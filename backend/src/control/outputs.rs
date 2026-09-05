use std::collections::HashSet;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use tracing::{debug, error, info, warn};

use crate::wiim::device::{DeviceManager, WiimDevice};

use super::models::SetEnabledRequest;
use super::state::ControlState;

#[derive(Debug)]
struct PlaybackSnapshot {
    uri: String,
    position: String,
    playing: bool,
}

/// The physical WiiM that owns the one logical Airwave playback stream.
/// Prefer the current group master, then a standalone enabled renderer.
pub fn playback_device(devices: &DeviceManager) -> Option<WiimDevice> {
    let mut enabled = devices
        .list_all()
        .into_iter()
        .filter(|device| {
            device.enabled && device.device_type == "wiim" && device.capabilities.av_transport
        })
        .collect::<Vec<_>>();
    enabled.sort_by(|a, b| a.id.cmp(&b.id));

    enabled
        .iter()
        .find(|device| device.is_master && device.group_id.as_deref() == Some(&device.id))
        .cloned()
        .or_else(|| {
            enabled
                .iter()
                .find(|device| device.group_id.is_none())
                .cloned()
        })
        .or_else(|| enabled.first().cloned())
}

pub async fn set_output_enabled(
    State(state): State<ControlState>,
    Path(id): Path<String>,
    Json(body): Json<SetEnabledRequest>,
) -> Result<StatusCode, StatusCode> {
    let _guard = state.output_lock.lock().await;
    let device = state.devices.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if device.device_type != "wiim" {
        return Err(StatusCode::NOT_FOUND);
    }
    if body.enabled && !device.capabilities.av_transport {
        return Err(StatusCode::CONFLICT);
    }
    let current_output = playback_device(&state.devices);
    let had_output = current_output.is_some();
    let preferred_master_id = current_output.map(|device| device.id);
    let snapshot = capture_playback(&state.devices).await;
    let previous = device.enabled;
    state
        .devices
        .update(&id, |entry| entry.enabled = body.enabled);

    if let Err(status) = reconcile_outputs_locked(
        &state,
        snapshot,
        !had_output && body.enabled,
        preferred_master_id.as_deref(),
    )
    .await
    {
        state.devices.update(&id, |entry| entry.enabled = previous);
        error!(
            "Failed to change output membership for {} to {}; desired state rolled back",
            id, body.enabled
        );
        return Err(status);
    }

    state.device_config.save_enabled(&id, body.enabled);
    state.events.publish(
        "device_state",
        &serde_json::json!({ "device_id": id, "enabled": body.enabled }),
    );
    if had_output && playback_device(&state.devices).is_none() {
        state.events.publish(
            "playback_stopped",
            &serde_json::json!({ "target_id": super::state::PLAYBACK_TARGET_ID }),
        );
    }
    publish_devices_changed(&state);
    Ok(StatusCode::OK)
}

/// Reconcile persisted output membership with the actual WiiM group topology.
/// Discovery calls this after it has refreshed the devices' physical group state.
pub async fn reconcile_outputs(state: &ControlState) -> Result<(), StatusCode> {
    let _guard = state.output_lock.lock().await;
    let preferred_master_id = playback_device(&state.devices).map(|device| device.id);
    let snapshot = capture_playback(&state.devices).await;
    reconcile_outputs_locked(state, snapshot, false, preferred_master_id.as_deref()).await
}

async fn reconcile_outputs_locked(
    state: &ControlState,
    mut snapshot: Option<PlaybackSnapshot>,
    restore_global_stream: bool,
    preferred_master_id: Option<&str>,
) -> Result<(), StatusCode> {
    let mut devices = state
        .devices
        .list_all()
        .into_iter()
        .filter(|device| device.device_type == "wiim")
        .collect::<Vec<_>>();
    devices.sort_by(|a, b| a.id.cmp(&b.id));

    let enabled = devices
        .iter()
        .filter(|device| device.enabled && device.capabilities.av_transport)
        .cloned()
        .collect::<Vec<_>>();
    let desired_master_id = select_master_id(&enabled, preferred_master_id);
    let mut restore_required = false;

    // Remove obsolete groups first. This also handles a disabled former master
    // and arbitrary legacy groups that do not match the one playing group.
    let physical_masters = devices
        .iter()
        .filter(|device| device.is_master)
        .map(|device| device.id.clone())
        .collect::<HashSet<_>>();
    for master_id in physical_masters {
        let keep = desired_master_id.as_deref() == Some(master_id.as_str()) && enabled.len() > 1;
        if !keep {
            dissolve_physical_group(state, &master_id).await?;
            restore_required = true;
        }
    }

    // Leave stale or undesired follower relationships without disturbing a
    // valid playing master.
    for device in state.devices.list_all() {
        if device.device_type != "wiim" || device.group_id.is_none() || device.is_master {
            continue;
        }
        let in_playing_group = device.enabled
            && desired_master_id.as_deref() == device.group_id.as_deref()
            && enabled.len() > 1;
        if !in_playing_group {
            leave_group(state, &device).await?;
        }
    }

    if let Some(master_id) = desired_master_id.as_deref() {
        if enabled.len() > 1 {
            for device in &enabled {
                if device.id == master_id {
                    continue;
                }
                let current = state.devices.get(&device.id).ok_or(StatusCode::NOT_FOUND)?;
                if current.group_id.as_deref() != Some(master_id) || current.is_master {
                    join_group(state, master_id, &current).await?;
                    restore_required = true;
                }
            }
            state.devices.update(master_id, |device| {
                device.group_id = Some(master_id.to_string());
                device.is_master = true;
            });
        } else {
            state.devices.update(master_id, |device| {
                device.group_id = None;
                device.is_master = false;
            });
        }
    }

    // A disabled output must have no active software transport. This is the
    // silence boundary: it does not change power, mute, or volume.
    for device in state
        .devices
        .list_all()
        .into_iter()
        .filter(|device| device.device_type == "wiim" && !device.enabled)
    {
        stop_transport(&device).await?;
    }

    if restore_global_stream && snapshot.is_none() {
        snapshot = global_playback_snapshot(state);
        restore_required = snapshot.is_some();
    }

    if restore_required {
        if let (Some(snapshot), Some(master)) = (snapshot, playback_device(&state.devices)) {
            restore_playback(&master, snapshot).await?;
        }
    }

    Ok(())
}

/// Reconnect the singleton's current track after its first output is enabled.
/// The stream is loaded paused; the user's next Play action resumes it.
fn global_playback_snapshot(state: &ControlState) -> Option<PlaybackSnapshot> {
    let session = state
        .sessions
        .get_or_create(super::state::PLAYBACK_TARGET_ID);
    let track_id = session
        .read()
        .as_ref()
        .and_then(|session| session.current_track_id().map(str::to_owned));

    let uri = if let Some(track_id) = track_id {
        let library = state.library.read();
        library
            .get(&track_id)
            .map(|_| format!("{}/media/{track_id}", state.base_url))
    } else {
        let queue = state.queues.get_or_create(super::state::PLAYBACK_TARGET_ID);
        let stream_url = queue.read().current().map(|track| {
            track
                .stream_url
                .clone()
                .unwrap_or_else(|| format!("{}/media/{}", state.base_url, track.id))
        });
        stream_url
    }?;

    Some(PlaybackSnapshot {
        uri,
        position: "00:00:00".to_string(),
        playing: false,
    })
}

fn select_master_id(enabled: &[WiimDevice], preferred_master_id: Option<&str>) -> Option<String> {
    enabled
        .iter()
        .find(|device| preferred_master_id == Some(device.id.as_str()))
        .or_else(|| {
            enabled
                .iter()
                .find(|device| device.is_master && device.group_id.as_deref() == Some(&device.id))
        })
        .or_else(|| enabled.iter().find(|device| device.group_id.is_none()))
        .or_else(|| enabled.first())
        .map(|device| device.id.clone())
}

async fn capture_playback(devices: &DeviceManager) -> Option<PlaybackSnapshot> {
    let preferred_id = playback_device(devices).map(|device| device.id);
    let mut candidates = devices
        .list_all()
        .into_iter()
        .filter(|device| {
            device.enabled && device.device_type == "wiim" && device.capabilities.av_transport
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        let left_preferred = preferred_id.as_deref() == Some(left.id.as_str());
        let right_preferred = preferred_id.as_deref() == Some(right.id.as_str());
        right_preferred
            .cmp(&left_preferred)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut paused_snapshot = None;
    for device in candidates {
        let Some((transport, position)) = device
            .av_transport
            .get_transport_info()
            .await
            .ok()
            .zip(device.av_transport.get_position_info().await.ok())
        else {
            continue;
        };
        if position.track_uri.is_empty() {
            continue;
        }
        let snapshot = PlaybackSnapshot {
            uri: position.track_uri,
            position: position.rel_time,
            playing: transport.current_transport_state == "PLAYING",
        };
        if snapshot.playing {
            return Some(snapshot);
        }
        if paused_snapshot.is_none() {
            paused_snapshot = Some(snapshot);
        }
    }
    paused_snapshot
}

async fn join_group(
    state: &ControlState,
    master_id: &str,
    follower: &WiimDevice,
) -> Result<(), StatusCode> {
    let master = state.devices.get(master_id).ok_or(StatusCode::NOT_FOUND)?;
    info!(
        "Joining output {} ({}) to playing master {} ({})",
        follower.name, follower.id, master.name, master.id
    );

    if let Some(https) = &follower.https_client {
        https.join_group_master(&master.ip).await.map_err(|error| {
            warn!("JoinGroupMaster failed for {}: {error:?}", follower.id);
            StatusCode::BAD_GATEWAY
        })?;
    } else {
        let master_info = format!("{}:{}", master.ip, master.port);
        follower
            .rendering
            .multiroom_join_group(&master_info)
            .await
            .map_err(|error| {
                warn!("MultiRoomJoinGroup failed for {}: {error:?}", follower.id);
                StatusCode::BAD_GATEWAY
            })?;
    }

    state.devices.update(&follower.id, |device| {
        device.group_id = Some(master_id.to_string());
        device.is_master = false;
    });
    Ok(())
}

async fn leave_group(state: &ControlState, follower: &WiimDevice) -> Result<(), StatusCode> {
    debug!(
        "Detaching output {} ({}) from group {:?}",
        follower.name, follower.id, follower.group_id
    );
    let used_master_api = if let Some(master_id) = follower.group_id.as_deref() {
        if let Some(master) = state.devices.get(master_id) {
            if let Some(https) = &master.https_client {
                https.slave_kickout(&follower.ip).await.map_err(|error| {
                    warn!("SlaveKickout failed for {}: {error:?}", follower.id);
                    StatusCode::BAD_GATEWAY
                })?;
                true
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    if !used_master_api {
        follower
            .rendering
            .multiroom_leave_group()
            .await
            .map_err(|error| {
                warn!("MultiRoomLeaveGroup failed for {}: {error:?}", follower.id);
                StatusCode::BAD_GATEWAY
            })?;
    }

    state.devices.update(&follower.id, |device| {
        device.group_id = None;
        device.is_master = false;
    });
    Ok(())
}

async fn dissolve_physical_group(state: &ControlState, master_id: &str) -> Result<(), StatusCode> {
    let master = state.devices.get(master_id).ok_or(StatusCode::NOT_FOUND)?;
    info!(
        "Dissolving obsolete output group led by {} ({})",
        master.name, master.id
    );

    if let Some(https) = &master.https_client {
        let slaves = https.get_slave_list().await.map_err(|error| {
            warn!("Failed to read slave list for {}: {error:?}", master.id);
            StatusCode::BAD_GATEWAY
        })?;
        for slave in slaves.slave_list {
            https.slave_kickout(&slave.ip).await.map_err(|error| {
                warn!("SlaveKickout failed for {}: {error:?}", slave.uuid);
                StatusCode::BAD_GATEWAY
            })?;
            let id = slave.uuid.replace("uuid:", "");
            state.devices.update(&id, |device| {
                device.group_id = None;
                device.is_master = false;
            });
        }
    } else {
        for follower in state.devices.list_all().into_iter().filter(|device| {
            device.id != master_id && device.group_id.as_deref() == Some(master_id)
        }) {
            follower
                .rendering
                .multiroom_leave_group()
                .await
                .map_err(|error| {
                    warn!("MultiRoomLeaveGroup failed for {}: {error:?}", follower.id);
                    StatusCode::BAD_GATEWAY
                })?;
            state.devices.update(&follower.id, |device| {
                device.group_id = None;
                device.is_master = false;
            });
        }
    }

    state.devices.update(master_id, |device| {
        device.group_id = None;
        device.is_master = false;
    });
    Ok(())
}

async fn stop_transport(device: &WiimDevice) -> Result<(), StatusCode> {
    let transport = device.av_transport.get_transport_info().await.ok();
    if transport.as_ref().is_some_and(|state| {
        matches!(
            state.current_transport_state.as_str(),
            "STOPPED" | "NO_MEDIA_PRESENT"
        )
    }) {
        return Ok(());
    }
    device.av_transport.stop().await.map_err(|error| {
        warn!("Failed to stop disabled output {}: {error:?}", device.id);
        StatusCode::BAD_GATEWAY
    })
}

async fn restore_playback(
    master: &WiimDevice,
    snapshot: PlaybackSnapshot,
) -> Result<(), StatusCode> {
    tokio::time::sleep(Duration::from_secs(2)).await;
    master
        .av_transport
        .set_av_transport_uri(&snapshot.uri, "")
        .await
        .map_err(|error| {
            warn!("Failed to restore stream on {}: {error:?}", master.id);
            StatusCode::BAD_GATEWAY
        })?;
    if !snapshot.position.is_empty() && snapshot.position != "00:00:00" {
        if let Err(error) = master.av_transport.seek(&snapshot.position).await {
            warn!(
                "Could not restore playback position on {}: {error:?}",
                master.id
            );
        }
    }
    if snapshot.playing {
        master.av_transport.play().await.map_err(|error| {
            warn!("Failed to resume stream on {}: {error:?}", master.id);
            StatusCode::BAD_GATEWAY
        })?;
    }
    info!("Restored the playing stream on output master {}", master.id);
    Ok(())
}

pub fn publish_devices_changed(state: &ControlState) {
    let mut devices = state.devices.list_all();
    devices.retain(|device| device.device_type == "wiim");
    devices.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let devices = devices
        .iter()
        .map(|device| {
            serde_json::json!({
                "id": device.id,
                "name": device.name,
                "ip": device.ip,
                "model": device.model,
                "firmware": device.firmware,
                "device_type": device.device_type,
                "enabled": device.enabled,
                "capabilities": device.capabilities,
                "volume": device.volume,
                "muted": device.muted,
                "channel": device.channel,
                "source": device.source,
                "group_id": device.group_id,
                "is_master": device.is_master,
            })
        })
        .collect::<Vec<_>>();
    state.events.publish(
        "devices_changed",
        &serde_json::json!({ "devices": devices }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiim::device::{DeviceCapabilities, DeviceParams, ServiceUrls};

    fn device(id: &str, enabled: bool, group_id: Option<&str>, is_master: bool) -> WiimDevice {
        let mut device = WiimDevice::new(DeviceParams {
            ip: format!("192.0.2.{}", id.len()),
            port: 49152,
            name: id.to_string(),
            model: Some("WiiM Mini".to_string()),
            firmware: None,
            udn: format!("uuid:{id}"),
            service_urls: ServiceUrls {
                av_transport: Some(format!("/wiim/{id}/upnp/av-transport")),
                rendering_control: Some(format!("/wiim/{id}/upnp/rendering-control")),
                play_queue: Some(format!("/wiim/{id}/upnp/play-queue")),
            },
            capabilities: DeviceCapabilities {
                av_transport: true,
                rendering_control: true,
                wiim_extended: true,
                https_api: false,
            },
            collector_url: "http://127.0.0.1:9".to_string(),
            collector_token: "test".to_string(),
        });
        device.enabled = enabled;
        device.group_id = group_id.map(str::to_string);
        device.is_master = is_master;
        device
    }

    #[test]
    fn current_enabled_master_remains_the_playing_master() {
        let enabled = vec![
            device("a", true, Some("a"), true),
            device("b", true, Some("a"), false),
        ];
        assert_eq!(select_master_id(&enabled, None).as_deref(), Some("a"));
    }

    #[test]
    fn disabled_master_is_not_selected() {
        let enabled = vec![device("b", true, Some("a"), false)];
        assert_eq!(select_master_id(&enabled, None).as_deref(), Some("b"));
    }

    #[test]
    fn current_standalone_output_remains_master_when_an_output_is_enabled() {
        let enabled = vec![
            device("a", true, None, false),
            device("b", true, None, false),
        ];
        assert_eq!(select_master_id(&enabled, Some("b")).as_deref(), Some("b"));
    }
}

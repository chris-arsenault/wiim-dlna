use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use thiserror::Error;
use tokio::sync::OwnedMutexGuard;
use tracing::{info, warn};

use crate::wiim::device::{DeviceManager, WiimDevice};

use super::models::SetEnabledRequest;
use super::state::ControlState;

const TOPOLOGY_POLL_INTERVAL: Duration = Duration::from_secs(5);
const TOPOLOGY_PHASE_TIMEOUT: Duration = Duration::from_secs(90);
const TOPOLOGY_STABLE_SAMPLES: u8 = 2;

#[derive(Debug, Clone)]
struct PlaybackSnapshot {
    uri: String,
    position: String,
    playing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PhysicalRole {
    Standalone,
    Master,
    Follower(String),
}

#[derive(Debug, Clone)]
struct PhysicalTopology {
    roles: HashMap<String, PhysicalRole>,
    followers_by_master: HashMap<String, HashSet<String>>,
}

#[derive(Debug, Default)]
struct ConvergenceTracker {
    consecutive_matches: u8,
}

impl ConvergenceTracker {
    fn observe(&mut self, matches: bool) -> bool {
        if matches {
            self.consecutive_matches = self.consecutive_matches.saturating_add(1);
        } else {
            self.consecutive_matches = 0;
        }
        self.consecutive_matches >= TOPOLOGY_STABLE_SAMPLES
    }

    fn reset(&mut self) {
        self.consecutive_matches = 0;
    }
}

impl PhysicalTopology {
    fn role(&self, device_id: &str) -> Option<&PhysicalRole> {
        self.roles.get(device_id)
    }

    fn group_members(&self, master_id: &str) -> Vec<String> {
        let mut members = self
            .followers_by_master
            .get(master_id)
            .cloned()
            .unwrap_or_default();
        members.insert(master_id.to_string());
        members.extend(
            self.roles
                .iter()
                .filter(|(_, role)| {
                    matches!(role, PhysicalRole::Follower(master) if master == master_id)
                })
                .map(|(id, _)| id.clone()),
        );
        members.into_iter().collect()
    }

    fn fully_joined(&self, follower_id: &str, master_id: &str) -> bool {
        matches!(
            self.role(follower_id),
            Some(PhysicalRole::Follower(id)) if id == master_id
        ) && self
            .followers_by_master
            .get(master_id)
            .is_some_and(|followers| followers.contains(follower_id))
    }

    fn fully_standalone(&self, device_id: &str) -> bool {
        matches!(self.role(device_id), Some(PhysicalRole::Standalone))
            && !self
                .followers_by_master
                .values()
                .any(|followers| followers.contains(device_id))
    }

    fn expected_group(&self, enabled_ids: &[String], master_id: &str) -> bool {
        enabled_ids.iter().all(|id| {
            if id == master_id {
                matches!(self.role(id), Some(PhysicalRole::Master))
            } else {
                self.fully_joined(id, master_id)
            }
        })
    }
}

#[derive(Debug, Error)]
enum TransitionError {
    #[error("speaker {0} is no longer available")]
    DeviceUnavailable(String),
    #[error("could not read topology from {device}: {message}")]
    TopologyRead { device: String, message: String },
    #[error("cannot group {0} because its Linkplay control API is unavailable")]
    LinkplayUnavailable(String),
    #[error("could not join {device} to the playing group: {message}")]
    Join { device: String, message: String },
    #[error("could not detach {device} from the playing group: {message}")]
    Detach { device: String, message: String },
    #[error("timed out waiting for WiiM hardware to {0}")]
    Timeout(String),
    #[error("could not stop {device}: {message}")]
    Stop { device: String, message: String },
    #[error("could not move playback to {device}: {message}")]
    Restore { device: String, message: String },
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

/// Accept one output change and let a background task observe the slow WiiM
/// topology transition. The task owns the output lock until convergence or a
/// bounded timeout, so another click can never queue behind it.
pub async fn set_output_enabled(
    State(state): State<ControlState>,
    Path(id): Path<String>,
    Json(body): Json<SetEnabledRequest>,
) -> Result<StatusCode, StatusCode> {
    let device = state.devices.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if device.device_type != "wiim" {
        return Err(StatusCode::NOT_FOUND);
    }
    if body.enabled && !device.capabilities.av_transport {
        return Err(StatusCode::CONFLICT);
    }
    if device.output_target.is_some() {
        return Err(StatusCode::CONFLICT);
    }
    if device.enabled == body.enabled {
        return Ok(StatusCode::NO_CONTENT);
    }

    let guard = Arc::clone(&state.output_lock)
        .try_lock_owned()
        .map_err(|_| StatusCode::CONFLICT)?;
    let previous_enabled = device.enabled;
    state.devices.update(&id, |entry| {
        entry.output_target = Some(body.enabled);
        entry.output_error = None;
    });
    publish_devices_changed(&state);

    let transition_state = state.clone();
    tokio::spawn(async move {
        finish_output_transition(transition_state, guard, id, previous_enabled, body.enabled).await;
    });

    Ok(StatusCode::ACCEPTED)
}

async fn finish_output_transition(
    state: ControlState,
    guard: OwnedMutexGuard<()>,
    device_id: String,
    previous_enabled: bool,
    target_enabled: bool,
) {
    let result = run_output_transition(&state, &device_id, target_enabled).await;

    let event_error = match result {
        Ok(topology) => {
            apply_physical_topology(&state.devices, &topology);
            state.devices.update(&device_id, |device| {
                device.enabled = target_enabled;
                device.output_target = None;
                device.output_error = None;
            });
            state.device_config.save_enabled(&device_id, target_enabled);
            info!(
                "WiiM output transition completed for {}: enabled={}",
                device_id, target_enabled
            );
            None
        }
        Err(error) => {
            if let Ok(topology) = read_physical_topology(&state.devices).await {
                apply_physical_topology(&state.devices, &topology);
            }
            let message = error.to_string();
            warn!(
                "WiiM output transition failed for {}: {}",
                device_id, message
            );
            state.devices.update(&device_id, |device| {
                device.output_target = None;
                device.output_error = Some(message.clone());
            });
            Some(message)
        }
    };

    // Make the next request admissible before publishing the non-pending
    // state. A fast client cannot observe an idle UI while the lock is held.
    drop(guard);
    let succeeded = event_error.is_none();
    state.events.publish(
        "device_state",
        &serde_json::json!({
            "device_id": device_id,
            "enabled": if succeeded { target_enabled } else { previous_enabled },
            "output_target": null,
            "output_error": event_error,
        }),
    );
    if succeeded && previous_enabled && !target_enabled && playback_device(&state.devices).is_none()
    {
        state.events.publish(
            "playback_stopped",
            &serde_json::json!({ "target_id": super::state::PLAYBACK_TARGET_ID }),
        );
    }
    publish_devices_changed(&state);
}

async fn run_output_transition(
    state: &ControlState,
    device_id: &str,
    target_enabled: bool,
) -> Result<PhysicalTopology, TransitionError> {
    let initial = read_physical_topology(&state.devices).await?;
    let current_master_id = playback_device(&state.devices).map(|device| device.id);

    if target_enabled {
        enable_output(state, device_id, &initial, current_master_id.as_deref()).await
    } else {
        disable_output(state, device_id, &initial, current_master_id.as_deref()).await
    }
}

async fn enable_output(
    state: &ControlState,
    device_id: &str,
    initial: &PhysicalTopology,
    current_master_id: Option<&str>,
) -> Result<PhysicalTopology, TransitionError> {
    let mut topology = initial.clone();
    let already_joined = current_master_id.is_some_and(|master_id| {
        matches!(
            topology.role(device_id),
            Some(PhysicalRole::Follower(id)) if id == master_id
        )
    });
    if !already_joined && !matches!(topology.role(device_id), Some(PhysicalRole::Standalone)) {
        topology = detach_device_once(state, device_id, &topology).await?;
    }

    if let Some(master_id) = current_master_id {
        if master_id != device_id && already_joined {
            topology = wait_for_topology(
                &state.devices,
                &format!("confirm {device_id} is joined to {master_id}"),
                |observed| observed.fully_joined(device_id, master_id),
            )
            .await?;
        } else if master_id != device_id {
            send_join_once(state, device_id, master_id).await?;
            topology = wait_for_topology(
                &state.devices,
                &format!("join {device_id} to {master_id}"),
                |observed| observed.fully_joined(device_id, master_id),
            )
            .await?;
        }
    } else if let Some(snapshot) = global_playback_snapshot(state) {
        let output = state
            .devices
            .get(device_id)
            .ok_or_else(|| TransitionError::DeviceUnavailable(device_id.to_string()))?;
        restore_playback(&output, snapshot).await?;
    }

    Ok(topology)
}

async fn disable_output(
    state: &ControlState,
    device_id: &str,
    initial: &PhysicalTopology,
    current_master_id: Option<&str>,
) -> Result<PhysicalTopology, TransitionError> {
    let remaining = enabled_ids_except(&state.devices, device_id);
    let moving_master = current_master_id == Some(device_id) && !remaining.is_empty();
    let snapshot = if moving_master {
        capture_playback(&state.devices).await
    } else {
        None
    };

    let topology = detach_device_once(state, device_id, initial).await?;
    let output = state
        .devices
        .get(device_id)
        .ok_or_else(|| TransitionError::DeviceUnavailable(device_id.to_string()))?;
    stop_transport(&output).await?;

    if !moving_master {
        return Ok(topology);
    }

    let new_master_id = remaining
        .first()
        .cloned()
        .ok_or_else(|| TransitionError::DeviceUnavailable(device_id.to_string()))?;
    let mut topology = topology;

    for follower_id in remaining.iter().filter(|id| **id != new_master_id) {
        if !matches!(topology.role(follower_id), Some(PhysicalRole::Standalone)) {
            topology = detach_device_once(state, follower_id, &topology).await?;
        }
    }

    if remaining.len() > 1 {
        for follower_id in remaining.iter().filter(|id| **id != new_master_id) {
            send_join_once(state, follower_id, &new_master_id).await?;
        }
        topology = wait_for_topology(
            &state.devices,
            &format!("form the playing group around {new_master_id}"),
            |observed| observed.expected_group(&remaining, &new_master_id),
        )
        .await?;
    }

    if let Some(snapshot) = snapshot {
        let new_master = state
            .devices
            .get(&new_master_id)
            .ok_or_else(|| TransitionError::DeviceUnavailable(new_master_id.clone()))?;
        restore_playback(&new_master, snapshot).await?;
    }

    Ok(topology)
}

fn enabled_ids_except(devices: &DeviceManager, excluded_id: &str) -> Vec<String> {
    let mut ids = devices
        .list_all()
        .into_iter()
        .filter(|device| {
            device.id != excluded_id
                && device.enabled
                && device.device_type == "wiim"
                && device.capabilities.av_transport
        })
        .map(|device| device.id)
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

async fn detach_device_once(
    state: &ControlState,
    device_id: &str,
    topology: &PhysicalTopology,
) -> Result<PhysicalTopology, TransitionError> {
    let role = topology
        .role(device_id)
        .cloned()
        .ok_or_else(|| TransitionError::DeviceUnavailable(device_id.to_string()))?;
    let affected_ids = match role {
        PhysicalRole::Standalone => return Ok(topology.clone()),
        PhysicalRole::Follower(master_id) => {
            send_kick_once(state, device_id, &master_id).await?;
            vec![device_id.to_string()]
        }
        PhysicalRole::Master => {
            let members = topology.group_members(device_id);
            send_dissolve_once(state, device_id).await?;
            members
        }
    };

    wait_for_topology(&state.devices, &format!("detach {device_id}"), |observed| {
        affected_ids.iter().all(|id| observed.fully_standalone(id))
    })
    .await
}

async fn send_join_once(
    state: &ControlState,
    follower_id: &str,
    master_id: &str,
) -> Result<(), TransitionError> {
    let follower = state
        .devices
        .get(follower_id)
        .ok_or_else(|| TransitionError::DeviceUnavailable(follower_id.to_string()))?;
    let master = state
        .devices
        .get(master_id)
        .ok_or_else(|| TransitionError::DeviceUnavailable(master_id.to_string()))?;
    let https = follower
        .https_client
        .as_ref()
        .ok_or_else(|| TransitionError::LinkplayUnavailable(follower_id.to_string()))?;

    info!(
        "Sending one join command to {} ({}) for master {} ({})",
        follower.name, follower.id, master.name, master.id
    );
    https
        .join_group_master(&master.ip)
        .await
        .map_err(|error| TransitionError::Join {
            device: follower_id.to_string(),
            message: error.to_string(),
        })
}

async fn send_kick_once(
    state: &ControlState,
    follower_id: &str,
    master_id: &str,
) -> Result<(), TransitionError> {
    let follower = state
        .devices
        .get(follower_id)
        .ok_or_else(|| TransitionError::DeviceUnavailable(follower_id.to_string()))?;
    let master = state
        .devices
        .get(master_id)
        .ok_or_else(|| TransitionError::DeviceUnavailable(master_id.to_string()))?;
    let https = master
        .https_client
        .as_ref()
        .ok_or_else(|| TransitionError::LinkplayUnavailable(master_id.to_string()))?;

    info!(
        "Sending one detach command for {} ({}) from master {} ({})",
        follower.name, follower.id, master.name, master.id
    );
    https
        .slave_kickout(&follower.ip)
        .await
        .map_err(|error| TransitionError::Detach {
            device: follower_id.to_string(),
            message: error.to_string(),
        })
}

async fn send_dissolve_once(state: &ControlState, master_id: &str) -> Result<(), TransitionError> {
    let master = state
        .devices
        .get(master_id)
        .ok_or_else(|| TransitionError::DeviceUnavailable(master_id.to_string()))?;
    let https = master
        .https_client
        .as_ref()
        .ok_or_else(|| TransitionError::LinkplayUnavailable(master_id.to_string()))?;
    let slaves = https
        .get_slave_list()
        .await
        .map_err(|error| TransitionError::Detach {
            device: master_id.to_string(),
            message: error.to_string(),
        })?;

    info!(
        "Sending one detach command per follower in group led by {} ({})",
        master.name, master.id
    );
    for slave in slaves.slave_list {
        https
            .slave_kickout(&slave.ip)
            .await
            .map_err(|error| TransitionError::Detach {
                device: slave.uuid,
                message: error.to_string(),
            })?;
    }
    Ok(())
}

async fn wait_for_topology<F>(
    devices: &DeviceManager,
    description: &str,
    converged: F,
) -> Result<PhysicalTopology, TransitionError>
where
    F: Fn(&PhysicalTopology) -> bool,
{
    let deadline = tokio::time::Instant::now() + TOPOLOGY_PHASE_TIMEOUT;
    let mut stability = ConvergenceTracker::default();
    loop {
        match read_physical_topology(devices).await {
            Ok(topology) => {
                if stability.observe(converged(&topology)) {
                    return Ok(topology);
                }
            }
            Err(error) => {
                stability.reset();
                warn!("Topology read while waiting to {description}: {error}");
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(TransitionError::Timeout(description.to_string()));
        }
        tokio::time::sleep(TOPOLOGY_POLL_INTERVAL).await;
    }
}

async fn read_physical_topology(
    devices: &DeviceManager,
) -> Result<PhysicalTopology, TransitionError> {
    let mut wiims = devices
        .list_all()
        .into_iter()
        .filter(|device| {
            device.device_type == "wiim"
                && device.capabilities.av_transport
                && (device.enabled || device.output_target.is_some())
        })
        .collect::<Vec<_>>();
    wiims.sort_by(|left, right| left.id.cmp(&right.id));
    let known_ids = wiims
        .iter()
        .map(|device| device.id.clone())
        .collect::<Vec<_>>();
    let mut roles = HashMap::new();
    let mut followers_by_master = HashMap::new();

    for device in wiims {
        let info = device.av_transport.get_info_ex().await.map_err(|error| {
            TransitionError::TopologyRead {
                device: device.id.clone(),
                message: error.to_string(),
            }
        })?;
        let role = physical_role_from_info(
            &info.slave_flag,
            &info.master_uuid,
            &info.slave_list,
            &known_ids,
        );
        if matches!(role, PhysicalRole::Master) {
            followers_by_master.insert(
                device.id.clone(),
                follower_ids_from_slave_list(&info.slave_list, &known_ids),
            );
        }
        roles.insert(device.id, role);
    }

    Ok(PhysicalTopology {
        roles,
        followers_by_master,
    })
}

fn physical_role_from_info(
    slave_flag: &str,
    master_uuid: &str,
    slave_list: &str,
    known_ids: &[String],
) -> PhysicalRole {
    if slave_flag == "1" {
        let master_id = normalize_device_id(master_uuid, known_ids);
        return PhysicalRole::Follower(master_id);
    }

    let slave_count = serde_json::from_str::<serde_json::Value>(slave_list)
        .ok()
        .and_then(|value| {
            value.get("slaves").and_then(|count| {
                count
                    .as_u64()
                    .or_else(|| count.as_str().and_then(|text| text.parse().ok()))
            })
        })
        .unwrap_or(0);
    if slave_count > 0 {
        PhysicalRole::Master
    } else {
        PhysicalRole::Standalone
    }
}

fn follower_ids_from_slave_list(slave_list: &str, known_ids: &[String]) -> HashSet<String> {
    serde_json::from_str::<serde_json::Value>(slave_list)
        .ok()
        .and_then(|value| value.get("slave_list").cloned())
        .and_then(|followers| followers.as_array().cloned())
        .into_iter()
        .flatten()
        .filter_map(|follower| {
            follower
                .get("uuid")
                .and_then(serde_json::Value::as_str)
                .map(|id| normalize_device_id(id, known_ids))
        })
        .collect()
}

fn normalize_device_id(id: &str, known_ids: &[String]) -> String {
    let normalized = id.trim().trim_start_matches("uuid:");
    known_ids
        .iter()
        .find(|known| known.eq_ignore_ascii_case(normalized))
        .cloned()
        .unwrap_or_else(|| normalized.to_string())
}

fn apply_physical_topology(devices: &DeviceManager, topology: &PhysicalTopology) {
    for (device_id, role) in &topology.roles {
        devices.update(device_id, |device| match role {
            PhysicalRole::Standalone => {
                device.group_id = None;
                device.is_master = false;
            }
            PhysicalRole::Master => {
                device.group_id = Some(device_id.clone());
                device.is_master = true;
            }
            PhysicalRole::Follower(master_id) => {
                device.group_id = Some(master_id.clone());
                device.is_master = false;
            }
        });
    }
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

async fn stop_transport(device: &WiimDevice) -> Result<(), TransitionError> {
    let transport = device.av_transport.get_transport_info().await.ok();
    if transport.as_ref().is_some_and(|state| {
        matches!(
            state.current_transport_state.as_str(),
            "STOPPED" | "NO_MEDIA_PRESENT"
        )
    }) {
        return Ok(());
    }
    device
        .av_transport
        .stop()
        .await
        .map_err(|error| TransitionError::Stop {
            device: device.id.clone(),
            message: error.to_string(),
        })
}

async fn restore_playback(
    master: &WiimDevice,
    snapshot: PlaybackSnapshot,
) -> Result<(), TransitionError> {
    master
        .av_transport
        .set_av_transport_uri(&snapshot.uri, "")
        .await
        .map_err(|error| TransitionError::Restore {
            device: master.id.clone(),
            message: error.to_string(),
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
        master
            .av_transport
            .play()
            .await
            .map_err(|error| TransitionError::Restore {
                device: master.id.clone(),
                message: error.to_string(),
            })?;
    }
    info!(
        "Moved the playing stream once to output master {}",
        master.id
    );
    Ok(())
}

pub fn transition_active(devices: &DeviceManager) -> bool {
    devices
        .list_all()
        .iter()
        .any(|device| device.output_target.is_some())
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
                "output_target": device.output_target,
                "output_error": device.output_error,
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
        let devices = DeviceManager::new();
        devices.register(device("a", true, Some("a"), true));
        devices.register(device("b", true, Some("a"), false));
        assert_eq!(
            playback_device(&devices).map(|device| device.id).as_deref(),
            Some("a")
        );
    }

    #[test]
    fn pending_target_does_not_change_the_stable_playback_owner() {
        let devices = DeviceManager::new();
        let mut master = device("a", true, Some("a"), true);
        master.output_target = Some(false);
        devices.register(master);
        devices.register(device("b", true, Some("a"), false));
        assert_eq!(
            playback_device(&devices).map(|device| device.id).as_deref(),
            Some("a")
        );
    }

    #[test]
    fn physical_role_uses_master_uuid_for_a_follower() {
        let ids = vec!["MASTER".to_string(), "follower".to_string()];
        assert_eq!(
            physical_role_from_info("1", "uuid:master", "{}", &ids),
            PhysicalRole::Follower("MASTER".to_string())
        );
    }

    #[test]
    fn physical_role_uses_slave_count_for_a_master() {
        assert_eq!(
            physical_role_from_info("0", "", r#"{"slaves":2}"#, &[]),
            PhysicalRole::Master
        );
    }

    #[test]
    fn master_slave_list_is_normalized_to_known_device_ids() {
        let ids = vec!["FOLLOWER".to_string()];
        let followers = follower_ids_from_slave_list(
            r#"{"slaves":"1","slave_list":[{"uuid":"uuid:follower"}]}"#,
            &ids,
        );
        assert_eq!(followers, HashSet::from(["FOLLOWER".to_string()]));
        assert_eq!(
            physical_role_from_info("0", "", r#"{"slaves":"1"}"#, &ids),
            PhysicalRole::Master
        );
    }

    #[test]
    fn physical_role_is_standalone_without_group_evidence() {
        assert_eq!(
            physical_role_from_info("0", "", "{}", &[]),
            PhysicalRole::Standalone
        );
    }

    #[test]
    fn group_members_include_master_and_known_followers() {
        let topology = PhysicalTopology {
            roles: HashMap::from([
                ("a".to_string(), PhysicalRole::Master),
                ("b".to_string(), PhysicalRole::Follower("a".to_string())),
                ("c".to_string(), PhysicalRole::Standalone),
            ]),
            followers_by_master: HashMap::from([(
                "a".to_string(),
                HashSet::from(["b".to_string()]),
            )]),
        };
        let mut members = topology.group_members("a");
        members.sort();
        assert_eq!(members, vec!["a", "b"]);
    }

    #[test]
    fn convergence_requires_master_and_follower_to_agree() {
        let mut topology = PhysicalTopology {
            roles: HashMap::from([
                ("a".to_string(), PhysicalRole::Master),
                ("b".to_string(), PhysicalRole::Follower("a".to_string())),
            ]),
            followers_by_master: HashMap::new(),
        };
        assert!(!topology.fully_joined("b", "a"));

        topology
            .followers_by_master
            .insert("a".to_string(), HashSet::from(["b".to_string()]));
        assert!(topology.fully_joined("b", "a"));
    }

    #[test]
    fn detach_is_not_complete_while_master_still_lists_follower() {
        let topology = PhysicalTopology {
            roles: HashMap::from([
                ("a".to_string(), PhysicalRole::Master),
                ("b".to_string(), PhysicalRole::Standalone),
            ]),
            followers_by_master: HashMap::from([(
                "a".to_string(),
                HashSet::from(["b".to_string()]),
            )]),
        };
        assert!(!topology.fully_standalone("b"));
    }

    #[test]
    fn convergence_requires_two_consecutive_matching_samples() {
        let mut stability = ConvergenceTracker::default();
        assert!(!stability.observe(true));
        assert!(stability.observe(true));
    }

    #[test]
    fn topology_mismatch_resets_convergence_hysteresis() {
        let mut stability = ConvergenceTracker::default();
        assert!(!stability.observe(true));
        assert!(!stability.observe(false));
        assert!(!stability.observe(true));
        assert!(stability.observe(true));
    }
}

use std::collections::HashMap;
use std::time::Duration;

use tracing::debug;

use super::events::EventBus;
use super::models::QueueTrackResponse;
use super::queue::QueueManager;
use super::state::{ControlState, PLAYBACK_TARGET_ID};
use crate::media::library::{LibraryObject, SharedLibrary};

/// Background task that monitors playback state, auto-advances
/// sessions/queues when a track finishes, and broadcasts state over SSE.
pub async fn run_playback_monitor(state: ControlState) {
    let ControlState {
        devices,
        queues,
        sessions,
        events,
        base_url,
        library,
        global_volume,
        volume_lock,
        device_config,
        output_recovery,
        ..
    } = state;
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    let mut last_states: HashMap<String, String> = HashMap::new();
    let mut initialized: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut last_playback_device: Option<String> = None;

    loop {
        interval.tick().await;

        let all_devices = devices.list_all();
        let current_device_ids: std::collections::HashSet<String> =
            all_devices.iter().map(|device| device.id.clone()).collect();
        initialized.retain(|id| current_device_ids.contains(id));
        let transition_active = all_devices
            .iter()
            .any(|device| device.output_target.is_some())
            || output_recovery.read().required;

        // Keep each WiiM's observed rendering state current. A physical volume
        // change is converted back to its base level only outside transitions.
        for device in &all_devices {
            if device.capabilities.rendering_control {
                let changed_base = {
                    let _volume_guard = volume_lock.lock().await;
                    if let Ok(vol) = device.rendering.get_volume().await {
                        let new_vol = vol as f64 / 100.0;
                        let infer_base_volume = !transition_active;
                        let mut changed_base = None;
                        devices.update(&device.id, |entry| {
                            changed_base = super::volume::reconcile_observed_volume(
                                entry,
                                new_vol,
                                *global_volume.read(),
                                infer_base_volume && entry.enabled,
                            );
                        });
                        changed_base
                    } else {
                        None
                    }
                };
                if let Some(volume) = changed_base {
                    device_config.save_volume(&device.id, volume);
                    events.publish(
                        "volume_changed",
                        &serde_json::json!({ "device_id": device.id, "volume": volume }),
                    );
                }
                if let Ok(muted) = device.rendering.get_mute().await {
                    if muted != device.muted {
                        devices.update(&device.id, |d| d.muted = muted);
                        events.publish(
                            "mute_changed",
                            &serde_json::json!({ "device_id": device.id, "muted": muted }),
                        );
                    }
                }
            }
        }

        // Group transitions can make the stable playback owner report
        // STOPPED for tens of seconds. Treat that as topology work, not a
        // track end, or the monitor will advance the global queue by itself.
        if transition_active {
            last_playback_device = None;
            last_states.clear();
            continue;
        }

        let Some(device) = super::outputs::playback_device(&devices) else {
            last_playback_device = None;
            last_states.clear();
            continue;
        };
        if last_playback_device.as_deref() != Some(&device.id) {
            last_states.clear();
            last_playback_device = Some(device.id.clone());
        }

        if initialized.insert(device.id.clone()) {
            if let Ok(settings) = device.av_transport.get_transport_settings().await {
                let (shuffle, repeat) = parse_upnp_play_mode(&settings.play_mode);
                let session_lock = sessions.get_or_create(PLAYBACK_TARGET_ID);
                let has_session = session_lock.read().is_some();
                if has_session {
                    let shuffle_mode: super::session::ShuffleMode =
                        serde_json::from_value(serde_json::Value::String(shuffle.to_string()))
                            .unwrap_or(super::session::ShuffleMode::Off);
                    let repeat_mode: super::session::RepeatMode =
                        serde_json::from_value(serde_json::Value::String(repeat.to_string()))
                            .unwrap_or(super::session::RepeatMode::Off);
                    let mut guard = session_lock.write();
                    if let Some(ref mut session) = *guard {
                        session.set_shuffle(shuffle_mode);
                        session.set_repeat(repeat_mode);
                    }
                } else {
                    let queue = queues.get_or_create(PLAYBACK_TARGET_ID);
                    let mut queue = queue.write();
                    queue.set_shuffle_mode(shuffle.to_string());
                    queue.set_repeat_mode(repeat.to_string());
                }
                debug!(
                    "Synced global transport settings from {}: {}",
                    device.id, settings.play_mode
                );
            }
        }

        let transport = device.av_transport.get_transport_info().await.ok();
        let position = device.av_transport.get_position_info().await.ok();

        let transport_state = transport
            .as_ref()
            .map(|transport| transport.current_transport_state.clone())
            .unwrap_or_default();
        let playing = transport_state == "PLAYING";
        let elapsed = position
            .as_ref()
            .map(|position| parse_duration(&position.rel_time))
            .unwrap_or(0.0);
        let duration = position
            .as_ref()
            .map(|position| parse_duration(&position.track_duration))
            .unwrap_or(0.0);

        let session_lock = sessions.get_or_create(PLAYBACK_TARGET_ID);
        let has_session = session_lock.read().is_some();

        let track_uri = position
            .as_ref()
            .map(|position| position.track_uri.clone())
            .unwrap_or_default();

        if has_session {
            handle_session_device(
                &device,
                &session_lock,
                &library,
                &base_url,
                &events,
                &mut last_states,
                &transport_state,
                &track_uri,
                playing,
                elapsed,
                duration,
            )
            .await;
        } else {
            handle_queue_device(
                &device,
                &queues,
                &base_url,
                &events,
                &mut last_states,
                &transport_state,
            )
            .await;
        }

        let allowed_actions = device
            .av_transport
            .get_current_transport_actions()
            .await
            .ok();

        broadcast_playback_state(
            &session_lock,
            &queues,
            &library,
            &base_url,
            &events,
            playing,
            elapsed,
            duration,
            allowed_actions.as_ref(),
            *global_volume.read(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_session_device(
    device: &crate::wiim::device::WiimDevice,
    session_lock: &parking_lot::RwLock<Option<super::session::PlaySession>>,
    library: &SharedLibrary,
    base_url: &str,
    events: &EventBus,
    last_states: &mut HashMap<String, String>,
    transport_state: &str,
    track_uri: &str,
    playing: bool,
    elapsed: f64,
    duration: f64,
) {
    let prev = last_states.get(&device.id).cloned().unwrap_or_default();

    // Detect seamless auto-transition: device used SetNextAVTransportURI and
    // transitioned without stopping (PLAYING → PLAYING). The device's track_uri
    // will no longer match the session's current track.
    if playing {
        let auto_transitioned = {
            let session = session_lock.read();
            if let Some(ref s) = *session {
                if s.is_next_sent() && !track_uri.is_empty() {
                    if let Some(current_id) = s.current_track_id() {
                        let expected_suffix = format!("/media/{}", current_id);
                        !track_uri.ends_with(&expected_suffix)
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };

        if auto_transitioned {
            debug!(
                "Detected seamless auto-transition on device {}, advancing session",
                device.id
            );
            let next_track_id = {
                let mut session = session_lock.write();
                if let Some(ref mut s) = *session {
                    s.advance() // also clears next_sent
                } else {
                    None
                }
            };
            if let Some(track_id) = next_track_id {
                let (title, artist) = {
                    let lib = library.read();
                    match lib.get(&track_id) {
                        Some(LibraryObject::Track(track)) => {
                            (track.meta.title.clone(), Some(track.meta.artist.clone()))
                        }
                        _ => (String::new(), None),
                    }
                };
                events.publish(
                    "track_changed",
                    &serde_json::json!({
                        "target_id": PLAYBACK_TARGET_ID,
                        "track": { "id": track_id, "title": title, "artist": artist }
                    }),
                );
            }
        }
    }

    // Pre-fetch: if playing and within 5s of track end, send next URI.
    if playing && duration > 0.0 && (duration - elapsed) <= 5.0 {
        // Resolve the next track URL while holding locks, then drop before await.
        let prefetch_url = {
            let session = session_lock.read();
            if let Some(ref s) = *session {
                if !s.is_next_sent() {
                    s.peek_next().and_then(|next_id| {
                        let lib = library.read();
                        if let Some(LibraryObject::Track(track)) = lib.get(&next_id) {
                            Some((next_id, format!("{}/media/{}", base_url, track.id)))
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((next_id, url)) = prefetch_url {
            let _ = device
                .av_transport
                .set_next_av_transport_uri(&url, "")
                .await;
            let mut session = session_lock.write();
            if let Some(ref mut s) = *session {
                s.mark_next_sent();
            }
            debug!(
                "Pre-fetched next track {} for device {}",
                next_id, device.id
            );
        }
    }

    // Detect track end: PLAYING/TRANSITIONING → STOPPED/NO_MEDIA_PRESENT.
    if (prev == "PLAYING" || prev == "TRANSITIONING")
        && (transport_state == "STOPPED" || transport_state == "NO_MEDIA_PRESENT")
    {
        debug!("Track ended on device {}, advancing session", device.id);

        let next_track_id = {
            let mut session = session_lock.write();
            if let Some(ref mut s) = *session {
                s.clear_next_sent();
                s.advance()
            } else {
                None
            }
        };

        if let Some(track_id) = next_track_id {
            let (stream_url, title, artist) = {
                let lib = library.read();
                match lib.get(&track_id) {
                    Some(LibraryObject::Track(track)) => (
                        format!("{}/media/{}", base_url, track.id),
                        track.meta.title.clone(),
                        Some(track.meta.artist.clone()),
                    ),
                    _ => {
                        last_states.insert(device.id.clone(), transport_state.to_string());
                        return;
                    }
                }
            };

            if device
                .av_transport
                .set_av_transport_uri(&stream_url, "")
                .await
                .is_ok()
            {
                let _ = device.av_transport.play().await;
            }

            events.publish(
                "track_changed",
                &serde_json::json!({
                    "target_id": PLAYBACK_TARGET_ID,
                    "track": { "id": track_id, "title": title, "artist": artist }
                }),
            );
        } else {
            events.publish(
                "session_ended",
                &serde_json::json!({ "target_id": PLAYBACK_TARGET_ID }),
            );
        }
    }

    last_states.insert(device.id.clone(), transport_state.to_string());
}

async fn handle_queue_device(
    device: &crate::wiim::device::WiimDevice,
    queues: &QueueManager,
    base_url: &str,
    events: &EventBus,
    last_states: &mut HashMap<String, String>,
    transport_state: &str,
) {
    let queue_lock = queues.get_or_create(PLAYBACK_TARGET_ID);

    // Skip devices with empty queues.
    {
        let q = queue_lock.read();
        if q.tracks().is_empty() {
            return;
        }
    }

    let prev = last_states.get(&device.id).cloned().unwrap_or_default();

    // Detect transition from PLAYING/TRANSITIONING to STOPPED.
    if (prev == "PLAYING" || prev == "TRANSITIONING")
        && (transport_state == "STOPPED" || transport_state == "NO_MEDIA_PRESENT")
    {
        debug!("Track ended on device {}, advancing queue", device.id);

        let next_track = {
            let mut q = queue_lock.write();
            q.advance().cloned()
        };

        if let Some(track) = next_track {
            let stream_url = track
                .stream_url
                .clone()
                .unwrap_or_else(|| format!("{}/media/{}", base_url, track.id));

            if device
                .av_transport
                .set_av_transport_uri(&stream_url, "")
                .await
                .is_ok()
            {
                let _ = device.av_transport.play().await;
            }

            events.publish(
                "track_changed",
                &serde_json::json!({
                        "target_id": PLAYBACK_TARGET_ID,
                    "track": {
                        "id": track.id,
                        "title": track.title,
                        "artist": track.artist,
                    }
                }),
            );
        } else {
            events.publish(
                "queue_ended",
                &serde_json::json!({ "target_id": PLAYBACK_TARGET_ID }),
            );
        }
    }

    last_states.insert(device.id.clone(), transport_state.to_string());
}

#[allow(clippy::too_many_arguments)]
fn broadcast_playback_state(
    session_lock: &parking_lot::RwLock<Option<super::session::PlaySession>>,
    queues: &QueueManager,
    library: &SharedLibrary,
    base_url: &str,
    events: &EventBus,
    playing: bool,
    elapsed: f64,
    duration: f64,
    allowed_actions: Option<&Vec<String>>,
    volume: f64,
) {
    let session_guard = session_lock.read();
    let (current_track, pos, queue_length, shuffle_mode, repeat_mode, session_info) =
        if let Some(ref session) = *session_guard {
            let track = session.current_track_id().and_then(|tid| {
                let lib = library.read();
                if let Some(LibraryObject::Track(t)) = lib.get(tid) {
                    Some(QueueTrackResponse {
                        id: t.id.clone(),
                        title: t.meta.title.clone(),
                        artist: Some(t.meta.artist.clone()),
                        album: Some(t.meta.album.clone()),
                        duration: t.meta.duration.map(|d| format_duration(d.as_secs_f64())),
                        stream_url: Some(format!("{}/media/{}", base_url, t.id)),
                    })
                } else {
                    None
                }
            });
            let info = serde_json::json!({
                "source_id": session.source.id,
                "label": session.source.label,
                "class": session.source.class,
                "artist": session.source.artist,
                "album": session.source.album,
                "shuffle_mode": session.shuffle_mode,
                "repeat_mode": session.repeat_mode,
                "total_tracks": session.total_tracks(),
                "position": session.flat_position(),
            });
            (
                track,
                session.flat_position(),
                session.total_tracks(),
                serde_json::to_value(session.shuffle_mode)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "off".to_string()),
                serde_json::to_value(session.repeat_mode)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "off".to_string()),
                Some(info),
            )
        } else {
            let queue = queues.get_or_create(PLAYBACK_TARGET_ID);
            let q = queue.read();
            (
                q.current().cloned(),
                q.position(),
                q.tracks().len(),
                q.shuffle_mode().to_string(),
                q.repeat_mode().to_string(),
                None,
            )
        };
    drop(session_guard);

    events.publish(
        "playback_state",
        &serde_json::json!({
            "target_id": PLAYBACK_TARGET_ID,
            "playing": playing,
            "volume": volume,
            "current_track": current_track,
            "position": pos,
            "queue_length": queue_length,
            "shuffle_mode": shuffle_mode,
            "repeat_mode": repeat_mode,
            "elapsed_seconds": elapsed,
            "duration_seconds": duration,
            "session": session_info,
            "allowed_actions": allowed_actions,
        }),
    );
}

fn format_duration(seconds: f64) -> String {
    let total = seconds as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Map UPnP PlayMode to app shuffle/repeat modes.
fn parse_upnp_play_mode(mode: &str) -> (&str, &str) {
    match mode {
        "SHUFFLE" | "SHUFFLE_NOREPEAT" | "RANDOM" => ("tracks", "off"),
        "REPEAT_ONE" => ("off", "track"),
        "REPEAT_ALL" => ("off", "all"),
        "SHUFFLE_REPEAT_ALL" => ("tracks", "all"),
        _ => ("off", "off"), // NORMAL or unknown
    }
}

fn parse_duration(s: &str) -> f64 {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        3 => {
            let h: f64 = parts[0].parse().unwrap_or(0.0);
            let m: f64 = parts[1].parse().unwrap_or(0.0);
            let s: f64 = parts[2].parse().unwrap_or(0.0);
            h * 3600.0 + m * 60.0 + s
        }
        2 => {
            let m: f64 = parts[0].parse().unwrap_or(0.0);
            let s: f64 = parts[1].parse().unwrap_or(0.0);
            m * 60.0 + s
        }
        _ => s.parse().unwrap_or(0.0),
    }
}

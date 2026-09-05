use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use parking_lot::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::models::{SleepTimerRequest, SleepTimerResponse};
use super::state::{ControlState, PLAYBACK_TARGET_ID};

#[derive(Clone)]
pub struct SleepTimerManager {
    inner: Arc<Mutex<HashMap<String, TimerEntry>>>,
}

struct TimerEntry {
    expires_at: Instant,
    handle: JoinHandle<()>,
}

impl Default for SleepTimerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SleepTimerManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn set(&self, minutes: u32, state: ControlState) {
        let duration = std::time::Duration::from_secs(minutes as u64 * 60);
        let expires_at = Instant::now() + duration;
        self.cancel();

        let manager = self.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            if let Some(device) = super::outputs::playback_device(&state.devices) {
                let _ = device.av_transport.stop().await;
            }
            state.events.publish(
                "sleep_timer_expired",
                &serde_json::json!({ "target_id": PLAYBACK_TARGET_ID }),
            );
            manager.inner.lock().remove(PLAYBACK_TARGET_ID);
        });

        self.inner.lock().insert(
            PLAYBACK_TARGET_ID.to_string(),
            TimerEntry { expires_at, handle },
        );
    }

    pub fn cancel(&self) {
        if let Some(entry) = self.inner.lock().remove(PLAYBACK_TARGET_ID) {
            entry.handle.abort();
        }
    }

    pub fn remaining_seconds(&self) -> Option<u64> {
        let lock = self.inner.lock();
        lock.get(PLAYBACK_TARGET_ID).map(|entry| {
            let now = Instant::now();
            if entry.expires_at > now {
                (entry.expires_at - now).as_secs()
            } else {
                0
            }
        })
    }
}

pub async fn set_sleep_timer(
    State(state): State<ControlState>,
    Json(body): Json<SleepTimerRequest>,
) -> StatusCode {
    let remaining = body.minutes as u64 * 60;
    state.sleep_timers.set(body.minutes, state.clone());
    state.events.publish(
        "sleep_timer_changed",
        &serde_json::json!({
            "target_id": PLAYBACK_TARGET_ID,
            "remaining_seconds": remaining,
        }),
    );
    StatusCode::OK
}

pub async fn get_sleep_timer(State(state): State<ControlState>) -> Json<SleepTimerResponse> {
    Json(SleepTimerResponse {
        remaining_seconds: state.sleep_timers.remaining_seconds(),
    })
}

pub async fn cancel_sleep_timer(State(state): State<ControlState>) -> StatusCode {
    state.sleep_timers.cancel();
    state.events.publish(
        "sleep_timer_changed",
        &serde_json::json!({
            "target_id": PLAYBACK_TARGET_ID,
            "remaining_seconds": null,
        }),
    );
    StatusCode::OK
}

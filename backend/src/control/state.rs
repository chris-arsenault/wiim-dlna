use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::media::art::ArtCache;
use crate::media::library::SharedLibrary;
use crate::wiim::device::DeviceManager;

use super::device_config::DeviceConfigStore;
use super::events::EventBus;
use super::playlists::PlaylistStore;
use super::queue::QueueManager;
use super::session::SessionManager;
use super::timer::SleepTimerManager;

pub const PLAYBACK_TARGET_ID: &str = "playing";
pub const OUTPUT_RECOVERY_STATE_KEY: &str = "output_recovery_state";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutputRecoveryState {
    pub required: bool,
    #[serde(default)]
    pub in_progress: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct ControlState {
    pub devices: Arc<DeviceManager>,
    pub device_config: Arc<DeviceConfigStore>,
    pub library: SharedLibrary,
    pub events: EventBus,
    pub playlists: Arc<PlaylistStore>,
    pub queues: Arc<QueueManager>,
    pub sessions: Arc<SessionManager>,
    pub art_cache: Arc<ArtCache>,
    pub sleep_timers: SleepTimerManager,
    pub output_lock: Arc<tokio::sync::Mutex<()>>,
    pub output_recovery: Arc<parking_lot::RwLock<OutputRecoveryState>>,
    pub volume_lock: Arc<tokio::sync::Mutex<()>>,
    pub global_volume: Arc<parking_lot::RwLock<f64>>,
    pub base_url: String,
    pub collector_ready: Arc<AtomicBool>,
}

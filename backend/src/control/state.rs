use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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
    pub base_url: String,
    pub collector_ready: Arc<AtomicBool>,
}

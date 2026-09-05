use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceResponse {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub device_type: String,
    pub enabled: bool,
    pub output_target: Option<bool>,
    pub output_error: Option<String>,
    pub capabilities: DeviceCapabilitiesResponse,
    pub volume: f64,
    pub muted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub source: Option<String>,
    pub group_id: Option<String>,
    pub is_master: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilitiesResponse {
    pub av_transport: bool,
    pub rendering_control: bool,
    pub wiim_extended: bool,
    pub https_api: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetEnabledRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryItemResponse {
    #[serde(rename = "type")]
    pub item_type: String,
    pub id: String,
    pub parent_id: Option<String>,
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<String>,
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BrowseResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerInfoResponse>,
    pub items: Vec<LibraryItemResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct ContainerInfoResponse {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueTrackResponse {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<String>,
    pub stream_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PlaybackStateResponse {
    pub target_id: String,
    pub playing: bool,
    pub current_track: Option<QueueTrackResponse>,
    pub position: usize,
    pub queue_length: usize,
    pub shuffle_mode: String,
    pub repeat_mode: String,
    pub elapsed_seconds: f64,
    pub duration_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionInfoResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_actions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfoResponse {
    pub source_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    pub shuffle_mode: String,
    pub repeat_mode: String,
    pub total_tracks: usize,
    pub position: usize,
}

#[derive(Debug, Serialize)]
pub struct QueueStateResponse {
    pub tracks: Vec<QueueTrackResponse>,
    pub position: usize,
}

#[derive(Debug, Deserialize)]
pub struct PlayRequest {
    pub track_id: Option<String>,
    pub track_ids: Option<Vec<String>>,
    pub container_id: Option<String>,
    pub start_index: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct VolumeRequest {
    pub volume: f64,
}

#[derive(Debug, Deserialize)]
pub struct SeekRequest {
    pub position_seconds: f64,
}

#[derive(Debug, Deserialize)]
pub struct ShuffleModeRequest {
    pub mode: String,
}

#[derive(Debug, Deserialize)]
pub struct RepeatModeRequest {
    pub mode: String,
}

#[derive(Debug, Deserialize)]
pub struct QueueAddRequest {
    pub track_ids: Vec<String>,
    #[serde(default = "default_position")]
    pub position: String,
}

fn default_position() -> String {
    "end".to_string()
}

#[derive(Debug, Deserialize)]
pub struct SessionPlayRequest {
    /// A library object ID, or `pl{id}` for a saved playlist.
    pub source_id: String,
    pub start_track_id: Option<String>,
    /// Shuffle mode to apply before the first track is chosen.
    pub shuffle: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceNameRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryPathEntry {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryStateRequest {
    pub path: Vec<LibraryPathEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryStateResponse {
    pub path: Vec<LibraryPathEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelRequest {
    pub channel: String,
}

#[derive(Debug, Deserialize)]
pub struct SleepTimerRequest {
    pub minutes: u32,
}

#[derive(Debug, Serialize)]
pub struct SleepTimerResponse {
    pub remaining_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct QueueMoveRequest {
    pub from_index: usize,
    pub to_index: usize,
}

#[derive(Debug, Deserialize)]
pub struct RateTrackRequest {
    pub track_id: String,
    pub rating: u8,
}

#[derive(Debug, Deserialize)]
pub struct PresetRequest {
    pub preset: String,
}

#[derive(Debug, Deserialize)]
pub struct EqBandRequest {
    pub index: u32,
    pub value: f64,
}

#[derive(Debug, Deserialize)]
pub struct SavePresetRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct BalanceRequest {
    pub balance: f64,
}

#[derive(Debug, Deserialize)]
pub struct CrossfadeRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct SourceRequest {
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct PlaylistResponse {
    pub id: i64,
    pub name: String,
    pub track_count: usize,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
    /// Track or container IDs; containers are expanded to their tracks.
    #[serde(default)]
    pub track_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddPlaylistTracksRequest {
    /// Track or container IDs; containers are expanded to their tracks.
    pub track_ids: Vec<String>,
}

import { config } from "../config";

const BASE = "/api";
let authToken = "";

export function setApiAuthToken(token: string) {
  authToken = token;
}

export function getApiAuthToken(): string {
  return authToken;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  headers.set("Content-Type", "application/json");
  if (authToken) headers.set("Authorization", `Bearer ${authToken}`);
  const res = await fetch(`${apiBase()}${path}`, { ...init, headers });
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
  const text = await res.text();
  if (!text) return undefined as T;
  return JSON.parse(text);
}

export function apiBase(): string {
  return config.apiBaseUrl ? `${config.apiBaseUrl.replace(/\/$/, "")}/api` : BASE;
}

export const api = {
  // Devices
  getDevices: () => request<Device[]>("/devices"),
  getDevice: (id: string) => request<Device>(`/devices/${id}`),
  setVolume: (id: string, volume: number) =>
    request("/devices/" + id + "/volume", { method: "POST", body: JSON.stringify({ volume }) }),
  toggleMute: (id: string) => request("/devices/" + id + "/mute", { method: "POST" }),
  setEnabled: (id: string, enabled: boolean) =>
    request("/devices/" + id + "/enabled", { method: "POST", body: JSON.stringify({ enabled }) }),
  renameDevice: (id: string, name: string) =>
    request("/devices/" + id + "/name", { method: "POST", body: JSON.stringify({ name }) }),
  getLibraryState: () => request<LibraryState>("/library/state"),
  setLibraryState: (path: BreadcrumbEntry[]) =>
    request("/library/state", {
      method: "POST",
      body: JSON.stringify({ path }),
    }),
  getChannel: (id: string) => request<{ channel: string }>(`/devices/${id}/channel`),
  setChannel: (id: string, channel: string) =>
    request("/devices/" + id + "/channel", { method: "POST", body: JSON.stringify({ channel }) }),

  // Library
  browse: (id = "0", start = 0, count = 0) =>
    request<BrowseResult>(`/library/browse?id=${id}&start=${start}&count=${count}`),
  search: (q: string, start = 0, count = 0) =>
    request<BrowseResult>(
      `/library/search?q=${encodeURIComponent(q)}&start=${start}&count=${count}`
    ),

  // Playback
  getPlaybackState: () => request<PlaybackState>("/playback"),
  play: (body: PlayRequest) =>
    request("/playback/play", { method: "POST", body: JSON.stringify(body) }),
  pause: () => request("/playback/pause", { method: "POST" }),
  resume: () => request("/playback/resume", { method: "POST" }),
  setPlaybackVolume: (volume: number) =>
    request("/playback/volume", { method: "POST", body: JSON.stringify({ volume }) }),
  next: () => request("/playback/next", { method: "POST" }),
  prev: () => request("/playback/prev", { method: "POST" }),
  seek: (positionSeconds: number) =>
    request("/playback/seek", {
      method: "POST",
      body: JSON.stringify({ position_seconds: positionSeconds }),
    }),
  seekForward: () => request("/playback/seek-forward", { method: "POST" }),
  seekBackward: () => request("/playback/seek-backward", { method: "POST" }),
  rateTrack: (trackId: string, rating: number) =>
    request("/playback/rate", {
      method: "POST",
      body: JSON.stringify({ track_id: trackId, rating }),
    }),
  setShuffle: (mode: string) =>
    request("/playback/shuffle", { method: "POST", body: JSON.stringify({ mode }) }),
  setRepeat: (mode: string) =>
    request("/playback/repeat", { method: "POST", body: JSON.stringify({ mode }) }),

  // Session-based playback
  sessionPlay: (body: SessionPlayRequest) =>
    request("/playback/session/play", { method: "POST", body: JSON.stringify(body) }),
  sessionNext: () => request("/playback/session/next", { method: "POST" }),
  sessionPrev: () => request("/playback/session/prev", { method: "POST" }),
  sessionSetShuffle: (mode: string) =>
    request("/playback/session/shuffle", {
      method: "POST",
      body: JSON.stringify({ mode }),
    }),
  sessionSetRepeat: (mode: string) =>
    request("/playback/session/repeat", {
      method: "POST",
      body: JSON.stringify({ mode }),
    }),

  // Queue
  getQueue: () => request<QueueState>("/playback/queue"),
  addToQueue: (trackIds: string[], position = "end") =>
    request("/playback/queue/add", {
      method: "POST",
      body: JSON.stringify({ track_ids: trackIds, position }),
    }),
  removeFromQueue: (index: number) => request(`/playback/queue/${index}`, { method: "DELETE" }),
  clearQueue: () => request("/playback/queue", { method: "DELETE" }),
  moveInQueue: (fromIndex: number, toIndex: number) =>
    request("/playback/queue/move", {
      method: "POST",
      body: JSON.stringify({ from_index: fromIndex, to_index: toIndex }),
    }),

  // Playlists
  getPlaylists: () => request<Playlist[]>("/playlists"),
  getPlaylist: (id: number) => request<PlaylistDetail>(`/playlists/${id}`),
  createPlaylist: (name: string, trackIds: string[] = []) =>
    request<{ id: number; name: string; track_count: number }>("/playlists", {
      method: "POST",
      body: JSON.stringify({ name, track_ids: trackIds }),
    }),
  deletePlaylist: (id: number) => request(`/playlists/${id}`, { method: "DELETE" }),
  /** Track or container IDs — containers (albums, artists, genres) are expanded server-side. */
  addToPlaylist: (id: number, trackIds: string[]) =>
    request<{ added: number }>(`/playlists/${id}/tracks`, {
      method: "POST",
      body: JSON.stringify({ track_ids: trackIds }),
    }),
  removeFromPlaylist: (id: number, position: number) =>
    request(`/playlists/${id}/tracks/${position}`, { method: "DELETE" }),
  playlistSourceId: (id: number) => `pl${id}`,

  // Metadata editing
  updateTrack: (trackId: string, update: TagUpdate) =>
    request(`/library/tracks/${trackId}`, { method: "PATCH", body: JSON.stringify(update) }),
  bulkSetAlbumArtist: (containerId: string, albumArtist: string) =>
    request<BulkResult>("/library/bulk/album-artist", {
      method: "POST",
      body: JSON.stringify({ container_id: containerId, album_artist: albumArtist }),
    }),
  bulkRenameArtist: (from: string, to: string, field = "both") =>
    request<BulkResult>("/library/bulk/rename-artist", {
      method: "POST",
      body: JSON.stringify({ from, to, field }),
    }),

  // Sleep timer
  setSleepTimer: (minutes: number) =>
    request("/playback/sleep-timer", {
      method: "POST",
      body: JSON.stringify({ minutes }),
    }),
  getSleepTimer: () => request<SleepTimerState>("/playback/sleep-timer"),
  cancelSleepTimer: () => request("/playback/sleep-timer", { method: "DELETE" }),

  // Device settings (HTTPS API)
  switchSource: (id: string, source: string) =>
    request("/devices/" + id + "/source", { method: "POST", body: JSON.stringify({ source }) }),
  getWifiStatus: (id: string) => request<WifiStatus>(`/devices/${id}/wifi`),

  // EQ
  getEqState: (id: string) => request<EqState>(`/eq/${id}/state`),
  getEqPresets: (id: string) => request<{ presets: string[] }>(`/eq/${id}/presets`),
  loadEqPreset: (id: string, preset: string) =>
    request<EqState>(`/eq/${id}/load`, { method: "POST", body: JSON.stringify({ preset }) }),
  enableEq: (id: string) => request(`/eq/${id}/enable`, { method: "POST" }),
  disableEq: (id: string) => request(`/eq/${id}/disable`, { method: "POST" }),
  setEqBand: (id: string, index: number, value: number) =>
    request(`/eq/${id}/band`, { method: "POST", body: JSON.stringify({ index, value }) }),
  saveEqPreset: (id: string, name: string) =>
    request(`/eq/${id}/save`, { method: "POST", body: JSON.stringify({ name }) }),
  deleteEqPreset: (id: string, name: string) =>
    request(`/eq/${id}/presets/${encodeURIComponent(name)}`, { method: "DELETE" }),
  getBalance: (id: string) => request<{ balance: number }>(`/eq/${id}/balance`),
  setBalance: (id: string, balance: number) =>
    request(`/eq/${id}/balance`, { method: "POST", body: JSON.stringify({ balance }) }),
  getCrossfade: (id: string) => request<{ enabled: boolean }>(`/eq/${id}/crossfade`),
  setCrossfade: (id: string, enabled: boolean) =>
    request(`/eq/${id}/crossfade`, { method: "POST", body: JSON.stringify({ enabled }) }),

  // Health
  health: () => request<{ status: string }>("/health"),

  // Art (returns image URL, not a JSON request)
  artUrl: (trackId: string) => `${apiBase()}/art/${trackId}`,
};

// Types
export interface DeviceCapabilities {
  av_transport: boolean;
  rendering_control: boolean;
  wiim_extended: boolean;
  https_api: boolean;
}

export interface Device {
  id: string;
  name: string;
  ip: string;
  model: string | null;
  firmware: string | null;
  device_type: string;
  enabled: boolean;
  capabilities: DeviceCapabilities;
  volume: number;
  muted: boolean;
  channel: string | null;
  source: string | null;
  group_id: string | null;
  is_master: boolean;
}

export interface BreadcrumbEntry {
  id: string;
  title: string;
}

export interface LibraryState {
  path: BreadcrumbEntry[];
}

export interface LibraryItem {
  type: "container" | "track";
  id: string;
  parent_id: string | null;
  title: string | null;
  artist?: string | null;
  album?: string | null;
  album_artist?: string | null;
  genre?: string | null;
  track_number?: string | null;
  class: string | null;
  child_count?: number;
  duration?: string | null;
  stream_url?: string | null;
  mime_type?: string | null;
  sample_rate?: string | null;
  bit_depth?: string | null;
}

export interface ContainerInfo {
  id: string;
  title: string;
  class?: string;
  artist?: string;
  album?: string;
}

export interface BrowseResult {
  container?: ContainerInfo;
  items: LibraryItem[];
  total: number;
}

export interface QueueTrack {
  id: string;
  title: string;
  artist: string | null;
  album: string | null;
  duration: string | null;
  stream_url: string | null;
}

export interface SessionInfo {
  source_id: string;
  label: string;
  class?: string;
  artist?: string;
  album?: string;
  shuffle_mode: string;
  repeat_mode: string;
  total_tracks: number;
  position: number;
}

export interface PlaybackState {
  target_id: string;
  playing: boolean;
  current_track: QueueTrack | null;
  position: number;
  queue_length: number;
  shuffle_mode: string;
  repeat_mode: string;
  elapsed_seconds: number;
  duration_seconds: number;
  session?: SessionInfo | null;
  allowed_actions?: string[] | null;
}

export interface SleepTimerState {
  remaining_seconds: number | null;
}

export interface QueueState {
  tracks: QueueTrack[];
  position: number;
}

export interface PlayRequest {
  track_id?: string;
  track_ids?: string[];
  container_id?: string;
  start_index?: number;
}

export interface SessionPlayRequest {
  /** A library object ID, or `pl{id}` for a saved playlist. */
  source_id: string;
  start_track_id?: string;
  /** Shuffle mode applied before the first track is chosen. */
  shuffle?: "off" | "tracks" | "groups" | "both";
}

export interface TagUpdate {
  title?: string;
  artist?: string;
  album?: string;
  album_artist?: string;
  genre?: string;
  track_number?: number;
  disc_number?: number;
}

export interface BulkResult {
  total: number;
  success: number;
  failed: number;
}

export interface Playlist {
  id: number;
  name: string;
  track_count: number;
  created_at: string | null;
  updated_at: string | null;
}

export interface PlaylistTrack {
  track_id: string;
  position: number;
  title?: string;
  artist?: string;
  album?: string;
  duration?: string | null;
  /** True when the track is no longer in the library. */
  missing?: boolean;
}

export interface PlaylistDetail extends Playlist {
  tracks: PlaylistTrack[];
}

export interface EqBand {
  index: number;
  param_name: string;
  value: number;
}

export interface EqState {
  enabled: boolean;
  preset_name: string;
  bands: EqBand[];
  channel_mode: string | null;
  source_name: string | null;
}

export interface WifiStatus {
  source: string | null;
  rssi: number | null;
  ssid: string | null;
}

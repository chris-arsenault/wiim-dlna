use dashmap::DashMap;
use parking_lot::RwLock;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::media::library::{Library, LibraryObject};

/// What the user originally chose to play.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSource {
    pub id: String,
    pub label: String,
    pub class: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

/// A group of tracks (typically an album) within a session.
#[derive(Debug, Clone)]
pub struct TrackGroup {
    #[allow(dead_code)]
    pub container_id: String,
    #[allow(dead_code)]
    pub label: String,
    pub track_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShuffleMode {
    Off,
    Tracks,
    Groups,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    All,
    Track,
}

/// A playback session that generates tracks on-the-fly from a library source.
#[derive(Debug)]
pub struct PlaySession {
    pub source: SessionSource,
    groups: Vec<TrackGroup>,
    /// Shuffled group ordering (indices into `groups`).
    group_order: Vec<usize>,
    /// Per-group shuffled track ordering (indices into each group's `track_ids`).
    track_orders: Vec<Vec<usize>>,
    /// Current position in the shuffled orderings.
    group_pos: usize,
    track_pos: usize,
    pub shuffle_mode: ShuffleMode,
    pub repeat_mode: RepeatMode,
    /// Whether SetNextAVTransportURI has been called for the upcoming track.
    next_sent: bool,
}

impl PlaySession {
    /// Create a new session from a library source.
    /// Returns None if the source has no playable tracks.
    pub fn new(source_id: &str, start_track_id: Option<&str>, library: &Library) -> Option<Self> {
        let source_obj = library.get(source_id)?;

        let (source, groups) = match source_obj {
            LibraryObject::Track(track) => {
                let source = SessionSource {
                    id: source_id.to_string(),
                    label: track.meta.title.clone(),
                    class: None,
                    artist: Some(track.meta.artist.clone()),
                    album: Some(track.meta.album.clone()),
                };
                let group = TrackGroup {
                    container_id: source_id.to_string(),
                    label: track.meta.title.clone(),
                    track_ids: vec![track.id.clone()],
                };
                (source, vec![group])
            }
            LibraryObject::Container(container) => {
                let source = build_source_info(container);
                let groups = build_groups(library, source_id, container);
                (source, groups)
            }
        };

        let mut session = Self::from_groups(source, groups)?;

        // If a start track was specified, seek to it.
        if let Some(start_id) = start_track_id {
            session.seek_to_track(start_id);
        }

        Some(session)
    }

    /// Create a session from an explicit list of track IDs (a saved playlist).
    /// Track IDs that are no longer in the library are skipped.
    pub fn from_tracks(
        source: SessionSource,
        track_ids: &[String],
        library: &Library,
    ) -> Option<Self> {
        let track_ids: Vec<String> = track_ids
            .iter()
            .filter(|id| matches!(library.get(id), Some(LibraryObject::Track(_))))
            .cloned()
            .collect();

        let group = TrackGroup {
            container_id: source.id.clone(),
            label: source.label.clone(),
            track_ids,
        };
        Self::from_groups(source, vec![group])
    }

    fn from_groups(source: SessionSource, groups: Vec<TrackGroup>) -> Option<Self> {
        if groups.is_empty() || groups.iter().all(|g| g.track_ids.is_empty()) {
            return None;
        }

        let group_order: Vec<usize> = (0..groups.len()).collect();
        let track_orders: Vec<Vec<usize>> = groups
            .iter()
            .map(|g| (0..g.track_ids.len()).collect())
            .collect();

        Some(Self {
            source,
            groups,
            group_order,
            track_orders,
            group_pos: 0,
            track_pos: 0,
            shuffle_mode: ShuffleMode::Off,
            repeat_mode: RepeatMode::Off,
            next_sent: false,
        })
    }

    /// Move back to the first track of the current ordering.
    pub fn restart(&mut self) {
        self.group_pos = 0;
        self.track_pos = 0;
        self.next_sent = false;
    }

    /// Returns the current track ID.
    pub fn current_track_id(&self) -> Option<&str> {
        let gi = *self.group_order.get(self.group_pos)?;
        let group = self.groups.get(gi)?;
        let ti = *self.track_orders.get(gi)?.get(self.track_pos)?;
        group.track_ids.get(ti).map(|s| s.as_str())
    }

    /// Advance to the next track. Returns the new track ID, or None if session ended.
    pub fn advance(&mut self) -> Option<String> {
        self.next_sent = false;

        if self.repeat_mode == RepeatMode::Track {
            return self.current_track_id().map(|s| s.to_string());
        }

        let gi = self.group_order[self.group_pos];
        let group_track_count = self.track_orders[gi].len();

        if self.track_pos + 1 < group_track_count {
            // Next track in current group.
            self.track_pos += 1;
        } else if self.group_pos + 1 < self.group_order.len() {
            // Next group.
            self.group_pos += 1;
            self.track_pos = 0;
        } else if self.repeat_mode == RepeatMode::All {
            // Wrap around.
            self.group_pos = 0;
            self.track_pos = 0;
            // Re-shuffle if needed for a fresh pass.
            if self.shuffle_mode == ShuffleMode::Groups || self.shuffle_mode == ShuffleMode::Both {
                self.reshuffle_groups();
            }
            if self.shuffle_mode == ShuffleMode::Tracks || self.shuffle_mode == ShuffleMode::Both {
                self.reshuffle_all_tracks();
            }
        } else {
            // Session ended.
            return None;
        }

        self.current_track_id().map(|s| s.to_string())
    }

    /// Go back to the previous track.
    pub fn go_back(&mut self) -> Option<String> {
        self.next_sent = false;

        if self.track_pos > 0 {
            self.track_pos -= 1;
        } else if self.group_pos > 0 {
            self.group_pos -= 1;
            let gi = self.group_order[self.group_pos];
            self.track_pos = self.track_orders[gi].len().saturating_sub(1);
        } else if self.repeat_mode == RepeatMode::All {
            self.group_pos = self.group_order.len().saturating_sub(1);
            let gi = self.group_order[self.group_pos];
            self.track_pos = self.track_orders[gi].len().saturating_sub(1);
        }
        // else: already at the very start, stay put.

        self.current_track_id().map(|s| s.to_string())
    }

    /// Peek at the next track without advancing.
    pub fn peek_next(&self) -> Option<String> {
        if self.repeat_mode == RepeatMode::Track {
            return self.current_track_id().map(|s| s.to_string());
        }

        let gi = self.group_order[self.group_pos];
        let group_track_count = self.track_orders[gi].len();

        let (next_gp, next_tp) = if self.track_pos + 1 < group_track_count {
            (self.group_pos, self.track_pos + 1)
        } else if self.group_pos + 1 < self.group_order.len() {
            (self.group_pos + 1, 0)
        } else if self.repeat_mode == RepeatMode::All {
            (0, 0)
        } else {
            return None;
        };

        let ngi = *self.group_order.get(next_gp)?;
        let group = self.groups.get(ngi)?;
        let nti = *self.track_orders.get(ngi)?.get(next_tp)?;
        group.track_ids.get(nti).map(|s| s.to_string())
    }

    /// Set shuffle mode and rebuild orderings.
    pub fn set_shuffle(&mut self, mode: ShuffleMode) {
        // Remember current track so we can stay on it.
        let current = self.current_track_id().map(|s| s.to_string());
        self.shuffle_mode = mode;

        match mode {
            ShuffleMode::Off => {
                // Reset to natural order.
                self.group_order = (0..self.groups.len()).collect();
                for (i, g) in self.groups.iter().enumerate() {
                    self.track_orders[i] = (0..g.track_ids.len()).collect();
                }
            }
            ShuffleMode::Tracks => {
                self.group_order = (0..self.groups.len()).collect();
                self.reshuffle_all_tracks();
            }
            ShuffleMode::Groups => {
                self.reshuffle_groups();
                for (i, g) in self.groups.iter().enumerate() {
                    self.track_orders[i] = (0..g.track_ids.len()).collect();
                }
            }
            ShuffleMode::Both => {
                self.reshuffle_groups();
                self.reshuffle_all_tracks();
            }
        }

        // Restore position to the track we were on.
        if let Some(ref tid) = current {
            self.seek_to_track(tid);
        }
    }

    /// Set repeat mode.
    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat_mode = mode;
    }

    pub fn mark_next_sent(&mut self) {
        self.next_sent = true;
    }

    pub fn clear_next_sent(&mut self) {
        self.next_sent = false;
    }

    pub fn is_next_sent(&self) -> bool {
        self.next_sent
    }

    /// Total number of tracks in the session.
    pub fn total_tracks(&self) -> usize {
        self.groups.iter().map(|g| g.track_ids.len()).sum()
    }

    /// Flat position of the current track (0-based).
    pub fn flat_position(&self) -> usize {
        let mut pos = 0;
        for gp in 0..self.group_pos {
            let gi = self.group_order[gp];
            pos += self.track_orders[gi].len();
        }
        pos + self.track_pos
    }

    /// Move to `track_id` if it is part of the session; otherwise stay put.
    pub fn seek_to_track(&mut self, track_id: &str) {
        for (gp, &gi) in self.group_order.iter().enumerate() {
            let group = &self.groups[gi];
            for (tp, &ti) in self.track_orders[gi].iter().enumerate() {
                if group.track_ids.get(ti).map(|s| s.as_str()) == Some(track_id) {
                    self.group_pos = gp;
                    self.track_pos = tp;
                    return;
                }
            }
        }
    }

    fn reshuffle_groups(&mut self) {
        let mut rng = rand::rng();
        self.group_order.shuffle(&mut rng);
    }

    fn reshuffle_all_tracks(&mut self) {
        let mut rng = rand::rng();
        for order in &mut self.track_orders {
            order.shuffle(&mut rng);
        }
    }
}

fn build_source_info(container: &crate::media::library::Container) -> SessionSource {
    let (artist, album) = match container.upnp_class {
        "object.container.album.musicAlbum" => (None, Some(container.title.clone())),
        "object.container.person.musicArtist" => (Some(container.title.clone()), None),
        _ => (None, None),
    };
    SessionSource {
        id: container.id.clone(),
        label: container.title.clone(),
        class: Some(container.upnp_class.to_string()),
        artist,
        album,
    }
}

fn build_groups(
    library: &Library,
    _source_id: &str,
    container: &crate::media::library::Container,
) -> Vec<TrackGroup> {
    let children = library.children_of(&container.id);

    // Check if children are all tracks (album/genre case) or containers (artist case).
    let has_sub_containers = children
        .iter()
        .any(|c| matches!(c, LibraryObject::Container(_)));

    if !has_sub_containers {
        // All children are tracks: single group.
        let track_ids: Vec<String> = children
            .iter()
            .filter_map(|c| match c {
                LibraryObject::Track(t) => Some(t.id.clone()),
                _ => None,
            })
            .collect();
        if track_ids.is_empty() {
            return Vec::new();
        }
        vec![TrackGroup {
            container_id: container.id.clone(),
            label: container.title.clone(),
            track_ids,
        }]
    } else {
        // Children are sub-containers (albums under an artist, etc.): each becomes a group.
        let mut groups = Vec::new();
        let mut loose_tracks = Vec::new();

        for child in &children {
            match child {
                LibraryObject::Container(sub) => {
                    let track_ids = collect_track_ids(library, &sub.id);
                    if !track_ids.is_empty() {
                        groups.push(TrackGroup {
                            container_id: sub.id.clone(),
                            label: sub.title.clone(),
                            track_ids,
                        });
                    }
                }
                LibraryObject::Track(t) => {
                    loose_tracks.push(t.id.clone());
                }
            }
        }

        // If there were loose tracks alongside sub-containers, add them as a group.
        if !loose_tracks.is_empty() {
            groups.push(TrackGroup {
                container_id: container.id.clone(),
                label: "Other".to_string(),
                track_ids: loose_tracks,
            });
        }

        groups
    }
}

/// Recursively collect every track ID under a container, in library order.
pub fn collect_track_ids(library: &Library, container_id: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for child in library.children_of(container_id) {
        match child {
            LibraryObject::Track(t) => ids.push(t.id.clone()),
            LibraryObject::Container(c) => {
                ids.extend(collect_track_ids(library, &c.id));
            }
        }
    }
    ids
}

/// Manages play sessions by logical playback target.
pub struct SessionManager {
    sessions: DashMap<String, Arc<RwLock<Option<PlaySession>>>>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    pub fn get_or_create(&self, target_id: &str) -> Arc<RwLock<Option<PlaySession>>> {
        self.sessions
            .entry(target_id.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(None)))
            .clone()
    }

    #[allow(dead_code)]
    pub fn clear_session(&self, target_id: &str) {
        if let Some(lock) = self.sessions.get(target_id) {
            *lock.write() = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::library::{Container, Library, LibraryObject, Track};
    use crate::media::metadata::TrackMetadata;
    use std::path::PathBuf;
    use std::time::Duration;

    fn make_track(id: &str, title: &str, track_num: u32) -> Track {
        Track {
            id: id.to_string(),
            parent_id: String::new(),
            path: PathBuf::from(format!("/music/{}.flac", id)),
            meta: TrackMetadata {
                title: title.to_string(),
                artist: "Test Artist".to_string(),
                album: "Test Album".to_string(),
                album_artist: "Test Artist".to_string(),
                genre: None,
                track_number: Some(track_num),
                disc_number: Some(1),
                duration: Some(Duration::from_secs(180)),
                sample_rate: None,
                bit_depth: None,
                mime_type: "audio/flac".to_string(),
                size_bytes: 0,
                year: None,
                channels: None,
            },
        }
    }

    fn build_test_library() -> Library {
        let mut lib = Library::new();

        // Create an album container with 3 tracks.
        let album = Container {
            id: "album1".to_string(),
            parent_id: "artist1".to_string(),
            title: "Test Album".to_string(),
            children: vec!["t1".to_string(), "t2".to_string(), "t3".to_string()],
            child_count: 3,
            upnp_class: "object.container.album.musicAlbum",
        };

        let t1 = make_track("t1", "Track One", 1);
        let t2 = make_track("t2", "Track Two", 2);
        let t3 = make_track("t3", "Track Three", 3);

        // Insert all objects via the internal objects map.
        // We need to access the internal BTreeMap, but Library doesn't expose insert.
        // Instead, we'll test through the public API by building a minimal lib.
        // For testing, we'll construct the library directly.
        use std::collections::BTreeMap;
        let mut objects = BTreeMap::new();
        objects.insert("album1".to_string(), LibraryObject::Container(album));
        objects.insert("t1".to_string(), LibraryObject::Track(t1));
        objects.insert("t2".to_string(), LibraryObject::Track(t2));
        objects.insert("t3".to_string(), LibraryObject::Track(t3));

        // Also add an artist with two albums.
        let album2 = Container {
            id: "album2".to_string(),
            parent_id: "artist1".to_string(),
            title: "Second Album".to_string(),
            children: vec!["t4".to_string(), "t5".to_string()],
            child_count: 2,
            upnp_class: "object.container.album.musicAlbum",
        };
        let artist = Container {
            id: "artist1".to_string(),
            parent_id: "vc_artists".to_string(),
            title: "Test Artist".to_string(),
            children: vec!["album1".to_string(), "album2".to_string()],
            child_count: 2,
            upnp_class: "object.container.person.musicArtist",
        };
        let t4 = make_track("t4", "Track Four", 1);
        let t5 = make_track("t5", "Track Five", 2);

        objects.insert("album2".to_string(), LibraryObject::Container(album2));
        objects.insert("artist1".to_string(), LibraryObject::Container(artist));
        objects.insert("t4".to_string(), LibraryObject::Track(t4));
        objects.insert("t5".to_string(), LibraryObject::Track(t5));

        // Use unsafe-ish approach: Library fields are private, so build via new() then replace.
        // Actually, let's just make a helper that constructs from parts.
        lib.inject_objects_for_test(objects);
        lib
    }

    // We need a way to inject test objects. Let's add a test-only method to Library.
    // For now, we'll test at the session level by using the scan function with test data.
    // Actually, it's simpler to just add a test helper.

    #[test]
    fn test_album_session_advance() {
        let lib = build_test_library();
        let mut session = PlaySession::new("album1", None, &lib).unwrap();

        assert_eq!(session.current_track_id(), Some("t1"));
        assert_eq!(session.advance(), Some("t2".to_string()));
        assert_eq!(session.advance(), Some("t3".to_string()));
        assert_eq!(session.advance(), None); // end of album
    }

    #[test]
    fn test_album_session_go_back() {
        let lib = build_test_library();
        let mut session = PlaySession::new("album1", None, &lib).unwrap();

        session.advance(); // t2
        session.advance(); // t3
        assert_eq!(session.go_back(), Some("t2".to_string()));
        assert_eq!(session.go_back(), Some("t1".to_string()));
        assert_eq!(session.go_back(), Some("t1".to_string())); // stay at start
    }

    #[test]
    fn test_artist_session_groups() {
        let lib = build_test_library();
        let mut session = PlaySession::new("artist1", None, &lib).unwrap();

        // Artist has 2 albums = 2 groups.
        assert_eq!(session.total_tracks(), 5);
        assert_eq!(session.current_track_id(), Some("t1"));
        assert_eq!(session.advance(), Some("t2".to_string()));
        assert_eq!(session.advance(), Some("t3".to_string()));
        // Crosses to second album.
        assert_eq!(session.advance(), Some("t4".to_string()));
        assert_eq!(session.advance(), Some("t5".to_string()));
        assert_eq!(session.advance(), None);
    }

    #[test]
    fn test_repeat_all() {
        let lib = build_test_library();
        let mut session = PlaySession::new("album1", None, &lib).unwrap();
        session.set_repeat(RepeatMode::All);

        session.advance(); // t2
        session.advance(); // t3
        assert_eq!(session.advance(), Some("t1".to_string())); // wraps
    }

    #[test]
    fn test_repeat_track() {
        let lib = build_test_library();
        let mut session = PlaySession::new("album1", None, &lib).unwrap();
        session.set_repeat(RepeatMode::Track);

        assert_eq!(session.advance(), Some("t1".to_string()));
        assert_eq!(session.advance(), Some("t1".to_string()));
    }

    #[test]
    fn test_start_at_track() {
        let lib = build_test_library();
        let session = PlaySession::new("album1", Some("t2"), &lib).unwrap();
        assert_eq!(session.current_track_id(), Some("t2"));
    }

    #[test]
    fn test_peek_next() {
        let lib = build_test_library();
        let session = PlaySession::new("album1", None, &lib).unwrap();

        assert_eq!(session.peek_next(), Some("t2".to_string()));
        // Position should not change.
        assert_eq!(session.current_track_id(), Some("t1"));
    }

    #[test]
    fn test_flat_position() {
        let lib = build_test_library();
        let mut session = PlaySession::new("artist1", None, &lib).unwrap();

        assert_eq!(session.flat_position(), 0);
        session.advance();
        assert_eq!(session.flat_position(), 1);
        session.advance(); // t3
        session.advance(); // t4 (second group)
        assert_eq!(session.flat_position(), 3);
    }

    fn playlist_source() -> SessionSource {
        SessionSource {
            id: "pl1".to_string(),
            label: "Mix".to_string(),
            class: Some("object.container.playlistContainer".to_string()),
            artist: None,
            album: None,
        }
    }

    #[test]
    fn test_playlist_session_plays_in_saved_order() {
        let lib = build_test_library();
        let ids = vec!["t4".to_string(), "t1".to_string(), "t3".to_string()];
        let mut session = PlaySession::from_tracks(playlist_source(), &ids, &lib).unwrap();

        assert_eq!(session.total_tracks(), 3);
        assert_eq!(session.current_track_id(), Some("t4"));
        assert_eq!(session.advance(), Some("t1".to_string()));
        assert_eq!(session.advance(), Some("t3".to_string()));
        assert_eq!(session.advance(), None);
    }

    #[test]
    fn test_playlist_session_skips_missing_tracks() {
        let lib = build_test_library();
        let ids = vec!["t1".to_string(), "gone".to_string()];
        let session = PlaySession::from_tracks(playlist_source(), &ids, &lib).unwrap();

        assert_eq!(session.total_tracks(), 1);
        assert!(PlaySession::from_tracks(playlist_source(), &["gone".to_string()], &lib).is_none());
    }

    #[test]
    fn test_playlist_shuffle_covers_every_track() {
        let lib = build_test_library();
        let ids = vec!["t1".to_string(), "t2".to_string(), "t3".to_string()];
        let mut session = PlaySession::from_tracks(playlist_source(), &ids, &lib).unwrap();

        session.set_shuffle(ShuffleMode::Tracks);
        session.restart();

        let mut played = vec![session.current_track_id().unwrap().to_string()];
        while let Some(next) = session.advance() {
            played.push(next);
        }
        played.sort();
        assert_eq!(played, ids);
    }
}

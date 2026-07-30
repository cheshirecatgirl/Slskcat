//! Domain types for the client.
//!
//! Nothing in here mentions the underlying protocol library. The adapter in
//! [`crate::live`] translates in both directions, so swapping the protocol
//! implementation never reaches past that one module.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

/// Identifies one search initiated by the user.
///
/// Searches are identified by our own counter rather than by query text so
/// that running the same query twice produces two independently cancellable
/// searches.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct SearchId(pub u64);

impl fmt::Display for SearchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "search:{}", self.0)
    }
}

/// Identifies one transfer. A peer cannot send us two different files with the
/// same path, so `(username, path)` is unique and stable across restarts —
/// which is what lets a resumed download reattach to its row in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferId {
    pub username: String,
    pub path: String,
}

impl TransferId {
    #[must_use]
    pub fn new(username: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            path: path.into(),
        }
    }
}

impl fmt::Display for TransferId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.username, self.path)
    }
}

/// A single file offered by a peer, as it appears in search results or in a
/// browse listing.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// The peer's own path for the file, which is what must be sent back when
    /// requesting it. Always in the peer's separator convention, so it is
    /// treated as an opaque key rather than a local path.
    pub path: String,
    pub size: u64,
    /// Decoded from the protocol's numeric attribute map where present.
    pub bitrate: Option<u32>,
    #[serde(with = "seconds_opt")]
    pub duration: Option<Duration>,
    /// True when the peer reported the file as variable bitrate.
    pub vbr: bool,
}

impl FileEntry {
    /// The final path component, which is what a user actually reads.
    ///
    /// Peers use both separator conventions, so both are honoured regardless
    /// of the platform we happen to be running on.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.path.rsplit(['\\', '/']).next().unwrap_or(&self.path)
    }

    /// The directory portion of the peer's path, used to group a peer's hits
    /// into albums.
    #[must_use]
    pub fn parent(&self) -> &str {
        let cut = self.path.rfind(['\\', '/']);
        cut.map_or("", |index| &self.path[..index])
    }

    /// Lowercased extension without the dot, or `""` when there is none.
    #[must_use]
    pub fn extension(&self) -> String {
        let name = self.file_name();
        name.rsplit_once('.')
            .map_or_else(String::new, |(_, ext)| ext.to_ascii_lowercase())
    }
}

/// One peer's response to a search, kept whole so the UI can show who is
/// offering what and how good the connection is likely to be.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub username: String,
    pub files: Vec<FileEntry>,
    /// Free upload slots the peer reported. Zero means queueing is likely.
    pub free_slots: u32,
    /// The peer's reported average speed, in bytes per second.
    pub speed: u32,
}

impl SearchHit {
    /// Whether the peer can start a transfer straight away.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.free_slots > 0
    }
}

/// Where a transfer currently is.
///
/// `Queued`, `Active` and `Paused` are live states; the rest are terminal.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum TransferState {
    /// Waiting in the peer's queue. Carries our position when the peer says.
    Queued {
        place: Option<u32>,
    },
    Active {
        transferred: u64,
        total: u64,
        bytes_per_sec: f64,
    },
    Paused {
        transferred: u64,
        total: u64,
    },
    Completed,
    Failed {
        reason: Option<String>,
    },
    Cancelled,
    TimedOut,
}

impl TransferState {
    /// Whether this state can still change on its own.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        matches!(
            self,
            Self::Queued { .. } | Self::Active { .. } | Self::Paused { .. }
        )
    }

    /// Completion in the range 0.0..=1.0, or `None` when the size is not yet
    /// known. A zero-byte file counts as complete rather than dividing by zero.
    #[must_use]
    pub fn progress(&self) -> Option<f32> {
        let ratio = |done: u64, total: u64| {
            if total == 0 {
                return 1.0;
            }
            // Both casts are deliberate: byte counts beyond f64's exact
            // range are not reachable, and the result is a 0..=1 ratio that
            // f32 represents to well beyond display precision.
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let ratio = (done as f64 / total as f64).clamp(0.0, 1.0) as f32;
            ratio
        };
        match *self {
            Self::Active {
                transferred, total, ..
            }
            | Self::Paused { transferred, total } => Some(ratio(transferred, total)),
            Self::Completed => Some(1.0),
            Self::Queued { .. } => Some(0.0),
            Self::Failed { .. } | Self::Cancelled | Self::TimedOut => None,
        }
    }
}

/// A transfer and everything the UI needs to draw its row.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub id: TransferId,
    pub size: u64,
    pub state: TransferState,
    /// Where the finished file is being written.
    pub destination: PathBuf,
}

impl Transfer {
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.id
            .path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&self.id.path)
    }
}

/// Where an upload has got to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum UploadState {
    /// Waiting for a free slot, at this 1-based place in our own queue.
    Queued {
        place: u32,
    },
    Active,
    Completed,
    Cancelled,
    Failed {
        reason: String,
    },
}

impl UploadState {
    /// Whether this state can still change on its own.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        matches!(self, Self::Queued { .. } | Self::Active)
    }
}

/// A file being served to a peer.
///
/// Uploads are what other people can see of you, so the interface shows them
/// in the same detail as downloads rather than hiding them behind a counter.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Upload {
    /// The peer receiving the file.
    pub username: String,
    /// The peer-facing path being served.
    pub path: String,
    pub size: u64,
    pub sent: u64,
    pub state: UploadState,
    pub bytes_per_sec: f64,
}

/// One directory from a peer's shared-file listing.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedDirectory {
    pub path: String,
    pub files: Vec<FileEntry>,
}

/// A public chat room and how busy it is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Room {
    pub name: String,
    pub user_count: u32,
}

/// A line of chat, from a room or a private conversation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub author: String,
    pub body: String,
}

/// Whether a peer is reachable, as far as the server knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Presence {
    Offline,
    Away,
    Online,
}

/// What is known about a peer. Fields stay `None` until their reply arrives,
/// because the server answers each part separately.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSummary {
    pub username: String,
    pub presence: Option<Presence>,
    pub shared_files: Option<u32>,
    pub shared_directories: Option<u32>,
    /// Average upload speed in bytes per second, as the server records it.
    pub average_speed: Option<u32>,
}

/// Everything needed to log in and serve files.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// User-tunable settings that the core needs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub credentials: Credentials,
    /// Where finished downloads land.
    pub download_dir: PathBuf,
    /// Directories offered to the network. Sharing nothing is antisocial but
    /// permitted, and some servers throttle accounts that share nothing.
    pub shared_dirs: Vec<PathBuf>,
    /// Concurrent uploads served before further requests are queued.
    pub upload_slots: usize,
    /// How long a search keeps collecting replies before it is closed.
    #[serde(with = "seconds")]
    pub search_timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            credentials: Credentials {
                username: String::new(),
                password: String::new(),
            },
            download_dir: default_download_dir(),
            shared_dirs: Vec::new(),
            upload_slots: 2,
            search_timeout: Duration::from_secs(12),
        }
    }
}

impl Config {
    /// Replace anything unusable with a working default.
    ///
    /// An empty download directory is the case that matters: it reaches the
    /// library as `""`, which is not a location, and every transfer would be
    /// written somewhere unintended. Callers assembling a config from a form
    /// or a settings file cannot be relied on to have filled it in, so the
    /// core refuses to trust it.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if self.download_dir.as_os_str().is_empty() {
            self.download_dir = default_download_dir();
        }
        if self.upload_slots == 0 {
            self.upload_slots = 1;
        }
        if self.search_timeout.is_zero() {
            self.search_timeout = Duration::from_secs(12);
        }
        self
    }
}

/// `$HOME/Downloads` where a home directory is known, else the working
/// directory, so the core never has to unwrap a missing path.
#[must_use]
pub fn default_download_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map_or_else(|| PathBuf::from("."), |home| home.join("Downloads"))
}

/// Serialise a `Duration` as whole seconds, which is the only precision the
/// interface needs and far easier to consume than serde's default struct.
mod seconds {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(value: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(value.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        u64::deserialize(d).map(Duration::from_secs)
    }
}

/// The same, for an optional duration.
mod seconds_opt {
    use serde::{Serialize, Serializer};
    use std::time::Duration;

    // serde's `with` contract hands the field by reference, so the signature
    // is fixed even though `Option<&Duration>` would read better.
    #[allow(clippy::ref_option)]
    pub fn serialize<S: Serializer>(value: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
        value.map(|d| d.as_secs()).serialize(s)
    }
}

/// Protocol file attributes are a numeric map; these are the codes we read.
pub(crate) mod attribute {
    pub const BITRATE: u32 = 0;
    pub const DURATION: u32 = 1;
    pub const VBR: u32 = 2;
}

/// Build a [`FileEntry`] from a peer path, size and raw attribute map.
pub(crate) fn file_entry(path: String, size: u64, attribs: &HashMap<u32, u32>) -> FileEntry {
    FileEntry {
        path,
        size,
        bitrate: attribs
            .get(&attribute::BITRATE)
            .copied()
            .filter(|rate| *rate > 0),
        duration: attribs
            .get(&attribute::DURATION)
            .copied()
            .filter(|secs| *secs > 0)
            .map(|secs| Duration::from_secs(u64::from(secs))),
        vbr: attribs.get(&attribute::VBR).copied().unwrap_or(0) != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_handles_both_separator_conventions() {
        let windows = FileEntry {
            path: r"@@music\Aphex Twin\SAW II\01 Rhubarb.flac".into(),
            size: 1,
            bitrate: None,
            duration: None,
            vbr: false,
        };
        assert_eq!(windows.file_name(), "01 Rhubarb.flac");
        assert_eq!(windows.parent(), r"@@music\Aphex Twin\SAW II");

        let unix = FileEntry {
            path: "music/track.mp3".into(),
            ..windows.clone()
        };
        assert_eq!(unix.file_name(), "track.mp3");
        assert_eq!(unix.parent(), "music");
    }

    #[test]
    fn file_name_falls_back_to_whole_path_when_unseparated() {
        let bare = FileEntry {
            path: "track.mp3".into(),
            size: 1,
            bitrate: None,
            duration: None,
            vbr: false,
        };
        assert_eq!(bare.file_name(), "track.mp3");
        assert_eq!(bare.parent(), "");
    }

    #[test]
    fn extension_is_lowercased_and_absent_when_missing() {
        let mut entry = FileEntry {
            path: "a/B.FLAC".into(),
            size: 1,
            bitrate: None,
            duration: None,
            vbr: false,
        };
        assert_eq!(entry.extension(), "flac");
        entry.path = "a/no-extension".into();
        assert_eq!(entry.extension(), "");
    }

    #[test]
    fn zero_byte_transfer_is_complete_rather_than_dividing_by_zero() {
        let state = TransferState::Active {
            transferred: 0,
            total: 0,
            bytes_per_sec: 0.0,
        };
        assert_eq!(state.progress(), Some(1.0));
    }

    #[test]
    fn progress_is_clamped_when_a_peer_overreports() {
        let state = TransferState::Active {
            transferred: 200,
            total: 100,
            bytes_per_sec: 0.0,
        };
        assert_eq!(state.progress(), Some(1.0));
    }

    #[test]
    fn terminal_states_are_not_live_and_have_no_progress() {
        for state in [
            TransferState::Completed,
            TransferState::Failed { reason: None },
            TransferState::Cancelled,
            TransferState::TimedOut,
        ] {
            assert!(!state.is_live(), "{state:?} should be terminal");
        }
        assert_eq!(TransferState::Cancelled.progress(), None);
        assert_eq!(TransferState::Completed.progress(), Some(1.0));
    }

    #[test]
    fn attributes_decode_and_drop_zero_placeholders() {
        let attribs = HashMap::from([
            (attribute::BITRATE, 0),
            (attribute::DURATION, 245),
            (attribute::VBR, 1),
        ]);
        let entry = file_entry("x/y.mp3".into(), 42, &attribs);
        assert_eq!(
            entry.bitrate, None,
            "a zero bitrate means unknown, not 0 kbps"
        );
        assert_eq!(entry.duration, Some(Duration::from_secs(245)));
        assert!(entry.vbr);
        assert_eq!(entry.size, 42);
    }

    #[test]
    fn hit_is_ready_only_with_a_free_slot() {
        let hit = SearchHit {
            username: "a".into(),
            files: vec![],
            free_slots: 0,
            speed: 0,
        };
        assert!(!hit.is_ready());
        assert!(
            SearchHit {
                free_slots: 1,
                ..hit
            }
            .is_ready()
        );
    }
}

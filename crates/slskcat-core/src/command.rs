//! Everything the UI can ask the core to do.

use crate::model::{Config, SearchId, TransferId};
use std::path::PathBuf;

/// A request from the UI to the core.
///
/// Commands are fire-and-forget: the outcome always comes back as an
/// [`Event`](crate::event::Event), never as a return value, so the UI never
/// blocks on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Connect to the server and log in using the supplied configuration.
    Connect(Box<Config>),
    /// Log out and drop the server connection.
    Disconnect,

    /// Start a search. Hits stream back tagged with `id`.
    Search {
        id: SearchId,
        query: String,
    },
    /// Stop collecting for a search. Hits already delivered stay valid.
    CancelSearch(SearchId),

    /// Replace the standing wishlist.
    ///
    /// A wish is a search the server re-runs on its own schedule, so the set
    /// is stated whole rather than added to one at a time.
    SetWishlist(Vec<String>),

    /// Queue a download from a peer.
    Download {
        username: String,
        path: String,
        size: u64,
    },
    PauseTransfer(TransferId),
    ResumeTransfer(TransferId),
    CancelTransfer(TransferId),

    /// Stop serving a file to a peer.
    CancelUpload(TransferId),

    /// Ask a peer for its full shared-file listing.
    BrowseUser(String),
    /// Ask the server what it knows about a peer.
    RequestUserInfo(String),

    /// Ask the server for the public room list.
    RequestRoomList,
    JoinRoom(String),
    LeaveRoom(String),
    SendRoomMessage {
        room: String,
        body: String,
    },
    SendPrivateMessage {
        username: String,
        body: String,
    },

    /// Replace the set of shared directories.
    SetSharedDirs(Vec<PathBuf>),
    /// Change how many uploads are served concurrently.
    SetUploadSlots(usize),
}

/// Hands out [`SearchId`]s that are unique within a run.
#[derive(Debug, Default)]
pub struct SearchIds {
    next: u64,
}

impl SearchIds {
    #[must_use]
    pub const fn new() -> Self {
        Self { next: 0 }
    }

    /// The next unused identifier.
    pub const fn next(&mut self) -> SearchId {
        let id = SearchId(self.next);
        self.next += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_ids_never_repeat() {
        let mut ids = SearchIds::new();
        let issued: Vec<_> = (0..4).map(|_| ids.next()).collect();
        assert_eq!(
            issued,
            vec![SearchId(0), SearchId(1), SearchId(2), SearchId(3)]
        );
    }
}

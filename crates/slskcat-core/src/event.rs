//! Everything the core reports back to the UI.

use crate::model::{
    ChatMessage, Room, SearchHit, SearchId, SharedDirectory, TransferId, TransferState, Upload,
    UserSummary,
};

/// Why a session ended.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Disconnect {
    /// The user asked to disconnect.
    Requested,
    /// The same account logged in elsewhere; the server drops the older session.
    LoggedInElsewhere,
    /// The connection dropped, with whatever detail was available.
    Lost(String),
}

/// Something that happened, delivered to the UI in the order it occurred.
///
/// Events are additive: `SearchHits` carries only hits not previously sent for
/// that search, so the UI appends rather than replacing.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Event {
    /// Logged in successfully.
    Connected {
        username: String,
    },
    /// Login was refused, with the server's reason where it gave one.
    LoginFailed {
        reason: String,
    },
    Disconnected(Disconnect),

    /// New hits for a search. Never empty.
    SearchHits {
        id: SearchId,
        hits: Vec<SearchHit>,
    },
    /// A search stopped collecting, either by timing out or by being cancelled.
    SearchFinished {
        id: SearchId,
    },

    /// A transfer appeared or changed state.
    TransferUpdated {
        id: TransferId,
        state: TransferState,
    },

    /// An upload appeared or changed state.
    UploadUpdated(Upload),

    /// A peer's shared-file listing arrived.
    BrowseReady {
        username: String,
        directories: Vec<SharedDirectory>,
    },
    /// Fresh information about a peer.
    UserUpdated(UserSummary),

    /// The full public room list, replacing any previous one.
    RoomList(Vec<Room>),
    RoomJoined {
        room: String,
        users: Vec<String>,
    },
    RoomLeft {
        room: String,
    },
    RoomMessage {
        room: String,
        message: ChatMessage,
    },
    /// Someone entered or left a room we are in.
    RoomPresence {
        room: String,
        username: String,
        joined: bool,
    },
    PrivateMessage(ChatMessage),

    /// Our own share statistics changed, as reported to the server.
    SharesUpdated {
        directories: u32,
        files: u32,
    },

    /// Something failed in a way the user should know about but which does not
    /// end the session.
    Warning(String),
}

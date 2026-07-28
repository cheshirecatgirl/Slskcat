//! The real backend, implemented over `soulseek-rs-lib`.
//!
//! This is the only module that names the protocol library. Everything it
//! knows about is translated into [`crate::model`] types before it leaves,
//! which is what makes the library replaceable.
//!
//! The library is synchronous, so the pattern throughout is the same: start
//! work, keep a handle to it, and surface progress from [`Backend::poll`]
//! rather than blocking the engine's worker thread.

use crate::backend::{Backend, EventSink};
use crate::command::Command;
use crate::event::{Disconnect, Event};
use crate::model::{
    ChatMessage, Config, FileEntry, Presence, Room, SearchHit, SearchId, SharedDirectory, Transfer,
    TransferId, TransferState, UserSummary, file_entry,
};

use soulseek_rs::{
    Client, ClientSettings, DownloadStatus, RoomEvent, SearchResult, SessionLoss, UserStatus,
};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// A search that is still collecting replies.
struct ActiveSearch {
    /// The library keys results by query text, so it is needed to read them.
    query: String,
    cancel: Arc<AtomicBool>,
    /// Set by the worker once its collection window closes.
    done: Arc<AtomicBool>,
    /// How many of the library's results have already been sent to the UI, so
    /// that polling emits only the new ones.
    delivered: usize,
    worker: Option<JoinHandle<()>>,
}

/// A download whose progress is arriving on a channel.
struct TransferWatch {
    updates: Receiver<DownloadStatus>,
    size: u64,
    destination: PathBuf,
    /// Latest known state, kept so pause/resume can report sensible byte
    /// counts without waiting for the next update from the library.
    state: TransferState,
}

/// Backend over the in-process Soulseek library.
#[derive(Default)]
pub struct LiveBackend {
    client: Option<Arc<Client>>,
    config: Config,
    searches: HashMap<SearchId, ActiveSearch>,
    transfers: HashMap<TransferId, TransferWatch>,
    /// Users whose browse listing has been requested but not yet returned.
    pending_browses: Vec<String>,
    /// Last reported share counts, so an unchanged count stays quiet.
    shares: Option<(u32, u32)>,
}

impl LiveBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The live client, or `None` with a warning already emitted.
    fn client(&self, out: &EventSink) -> Option<Arc<Client>> {
        let client = self.client.clone();
        if client.is_none() {
            out.warn("Not connected.");
        }
        client
    }

    fn connect(&mut self, config: Config, out: &EventSink) {
        if self.client.is_some() {
            self.disconnect(Disconnect::Requested, out);
        }

        // Whatever assembled this config — a form, a settings file — may have
        // left the download directory blank. Downloads must never be written
        // to an empty path, so it is repaired before anything uses it.
        let config = config.normalized();

        let settings = ClientSettings {
            username: config.credentials.username.clone(),
            password: config.credentials.password.clone(),
            shared_directories: config
                .shared_dirs
                .iter()
                .map(|dir| dir.to_string_lossy().into_owned())
                .collect(),
            ..ClientSettings::default()
        };

        // `connect` needs exclusive access; once it succeeds the client is
        // shared for the rest of the session and every other call takes `&self`.
        let mut client = Client::with_settings(settings);
        if let Err(error) = client.connect() {
            out.emit(Event::LoginFailed { reason: format!("Could not reach the server: {error}") });
            return;
        }

        match client.login() {
            Ok(true) => {}
            Ok(false) => {
                out.emit(Event::LoginFailed {
                    reason: "The server rejected that username or password.".into(),
                });
                return;
            }
            Err(error) => {
                out.emit(Event::LoginFailed { reason: format!("Login failed: {error}") });
                return;
            }
        }

        client.set_upload_slots(config.upload_slots);
        let username = client.username().to_owned();
        self.client = Some(Arc::new(client));
        self.config = config;
        out.emit(Event::Connected { username });
    }

    fn disconnect(&mut self, reason: Disconnect, out: &EventSink) {
        self.stop_all_searches();
        self.transfers.clear();
        self.pending_browses.clear();
        self.shares = None;
        if self.client.take().is_some() {
            out.emit(Event::Disconnected(reason));
        }
    }

    /// Signal every search to stop and wait for its worker.
    fn stop_all_searches(&mut self) {
        for search in self.searches.values_mut() {
            search.cancel.store(true, Ordering::Relaxed);
        }
        for (_, mut search) in self.searches.drain() {
            if let Some(worker) = search.worker.take() {
                let _ = worker.join();
            }
        }
    }

    fn start_search(&mut self, id: SearchId, query: String, out: &EventSink) {
        let Some(client) = self.client(out) else {
            out.emit(Event::SearchFinished { id });
            return;
        };

        // The library stores results per query string, so two live searches
        // for the same text would read each other's bucket. Retire the older
        // one rather than let them interfere.
        let clashing: Vec<SearchId> = self
            .searches
            .iter()
            .filter(|(_, search)| search.query == query)
            .map(|(other, _)| *other)
            .collect();
        for other in clashing {
            self.finish_search(other, out);
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let worker = thread::Builder::new()
            .name("slskcat-search".into())
            .spawn({
                let (client, query) = (Arc::clone(&client), query.clone());
                let (cancel, done) = (Arc::clone(&cancel), Arc::clone(&done));
                let timeout = self.config.search_timeout;
                move || {
                    // Results are read back by polling, so the return value is
                    // redundant here; only the collection window matters.
                    let _ = client.search_with_cancel(&query, timeout, Some(cancel));
                    done.store(true, Ordering::Release);
                }
            })
            .ok();

        if worker.is_none() {
            out.warn("Could not start the search.");
            out.emit(Event::SearchFinished { id });
            return;
        }

        self.searches.insert(id, ActiveSearch { query, cancel, done, delivered: 0, worker });
    }

    /// Emit any hits that have arrived for `id` since the last poll.
    fn drain_search(&mut self, id: SearchId, out: &EventSink) {
        let Some(client) = self.client.as_ref() else { return };
        let Some(search) = self.searches.get_mut(&id) else { return };

        // A `try_` read: the collecting worker holds the lock in bursts, and
        // skipping a contended poll simply defers the hits by one tick.
        let Some(results) = client.try_get_search_results(&search.query) else { return };
        if results.len() <= search.delivered {
            return;
        }

        let hits: Vec<SearchHit> = results[search.delivered..].iter().map(convert_hit).collect();
        search.delivered = results.len();
        if !hits.is_empty() {
            out.emit(Event::SearchHits { id, hits });
        }
    }

    /// Cancel a search, deliver whatever it already found, and close it out.
    fn finish_search(&mut self, id: SearchId, out: &EventSink) {
        if let Some(search) = self.searches.get(&id) {
            search.cancel.store(true, Ordering::Relaxed);
        }
        self.drain_search(id, out);
        if let Some(mut search) = self.searches.remove(&id) {
            if let Some(worker) = search.worker.take() {
                let _ = worker.join();
            }
            out.emit(Event::SearchFinished { id });
        }
    }

    fn start_download(&mut self, username: String, path: String, size: u64, out: &EventSink) {
        let Some(client) = self.client(out) else { return };
        let id = TransferId::new(username.clone(), path.clone());
        if self.transfers.get(&id).is_some_and(|watch| watch.state.is_live()) {
            out.warn(format!("Already downloading {}.", id.path));
            return;
        }

        let destination = self.config.download_dir.clone();
        match client.download(
            path,
            username,
            size,
            destination.to_string_lossy().into_owned(),
        ) {
            Ok((_, updates)) => {
                let state = TransferState::Queued { place: None };
                self.transfers.insert(
                    id.clone(),
                    TransferWatch { updates, size, destination, state: state.clone() },
                );
                out.emit(Event::TransferUpdated { id, state });
            }
            Err(error) => out.warn(format!("Could not start that download: {error}")),
        }
    }

    /// Forward every progress update that has arrived, for every transfer.
    fn drain_transfers(&mut self, out: &EventSink) {
        let mut closed = Vec::new();
        for (id, watch) in &mut self.transfers {
            loop {
                match watch.updates.try_recv() {
                    Ok(status) => {
                        watch.state = convert_status(&status);
                        out.emit(Event::TransferUpdated {
                            id: id.clone(),
                            state: watch.state.clone(),
                        });
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // The library dropped the sender. If it never reported
                        // a terminal state, the transfer died without saying so.
                        if watch.state.is_live() {
                            watch.state = TransferState::Failed {
                                reason: Some("The transfer stopped unexpectedly.".into()),
                            };
                            out.emit(Event::TransferUpdated {
                                id: id.clone(),
                                state: watch.state.clone(),
                            });
                        }
                        closed.push(id.clone());
                        break;
                    }
                }
            }
        }
        for id in closed {
            self.transfers.remove(&id);
        }
    }

    /// Snapshot of every transfer, for a UI rebuilding its list.
    #[must_use]
    pub fn transfers(&self) -> Vec<Transfer> {
        self.transfers
            .iter()
            .map(|(id, watch)| Transfer {
                id: id.clone(),
                size: watch.size,
                state: watch.state.clone(),
                destination: watch.destination.clone(),
            })
            .collect()
    }

    fn drain_rooms(client: &Client, out: &EventSink) {
        out.emit_all(client.take_room_events().into_iter().map(convert_room_event));
        out.emit_all(client.take_private_messages().into_iter().map(|message| {
            Event::PrivateMessage(ChatMessage {
                author: message.username().to_owned(),
                body: message.message().to_owned(),
            })
        }));
    }

    fn cancel_transfer(&mut self, id: TransferId, out: &EventSink) {
        if let Some(client) = self.client(out) {
            // A false result means the library had already retired it, which
            // is the state being asked for, so it is not an error.
            let _ = client.remove_download(&id.username, &id.path);
        }
        if let Some(watch) = self.transfers.get_mut(&id) {
            watch.state = TransferState::Cancelled;
        }
        out.emit(Event::TransferUpdated { id, state: TransferState::Cancelled });
    }

    fn browse(&mut self, username: String, out: &EventSink) {
        let Some(client) = self.client(out) else { return };
        match client.browse_user(&username) {
            Ok(()) => {
                if !self.pending_browses.contains(&username) {
                    self.pending_browses.push(username);
                }
            }
            Err(error) => out.warn(format!("Could not browse {username}: {error}")),
        }
    }

    fn set_shares(&mut self, dirs: Vec<PathBuf>, out: &EventSink) {
        // Kept even when disconnected, so the next login shares the right set.
        self.config.shared_dirs = dirs;
        let paths = self
            .config
            .shared_dirs
            .iter()
            .map(|dir| dir.to_string_lossy().into_owned())
            .collect();
        if let Some(client) = self.client(out)
            && let Err(error) = client.set_shared_directories(paths)
        {
            out.warn(format!("Could not update shares: {error}"));
        }
    }

    fn drain_browses(&mut self, client: &Client, out: &EventSink) {
        self.pending_browses.retain(|username| {
            let Some(directories) = client.take_browse_result(username) else {
                return true; // still waiting
            };
            out.emit(Event::BrowseReady {
                username: username.clone(),
                directories: directories.into_iter().map(convert_directory).collect(),
            });
            false
        });
    }

    fn report_shares(&mut self, client: &Client, out: &EventSink) {
        let counts = client.shared_counts();
        if self.shares != Some(counts) {
            self.shares = Some(counts);
            out.emit(Event::SharesUpdated { directories: counts.0, files: counts.1 });
        }
    }
}

impl Backend for LiveBackend {
    fn execute(&mut self, command: Command, out: &EventSink) {
        match command {
            Command::Connect(config) => self.connect(*config, out),
            Command::Disconnect => self.disconnect(Disconnect::Requested, out),

            Command::Search { id, query } => self.start_search(id, query, out),
            Command::CancelSearch(id) => self.finish_search(id, out),

            Command::Download { username, path, size } => {
                self.start_download(username, path, size, out);
            }
            Command::PauseTransfer(id) => {
                if let Some(client) = self.client(out)
                    && !client.pause_download(&id.username, &id.path)
                {
                    out.warn("That transfer could not be paused.");
                }
            }
            Command::ResumeTransfer(id) => {
                if let Some(client) = self.client(out)
                    && !client.resume_download(&id.username, &id.path)
                {
                    out.warn("That transfer could not be resumed.");
                }
            }
            Command::CancelTransfer(id) => self.cancel_transfer(id, out),

            Command::BrowseUser(username) => self.browse(username, out),
            Command::RequestUserInfo(username) => {
                if let Some(client) = self.client(out)
                    && let Err(error) = client.request_user_info(&username)
                {
                    out.warn(format!("Could not look up {username}: {error}"));
                }
            }

            Command::RequestRoomList => {
                if let Some(client) = self.client(out)
                    && let Err(error) = client.request_room_list()
                {
                    out.warn(format!("Could not fetch the room list: {error}"));
                }
            }
            Command::JoinRoom(room) => {
                if let Some(client) = self.client(out)
                    && let Err(error) = client.join_room(&room)
                {
                    out.warn(format!("Could not join {room}: {error}"));
                }
            }
            Command::LeaveRoom(room) => {
                if let Some(client) = self.client(out)
                    && let Err(error) = client.leave_room(&room)
                {
                    out.warn(format!("Could not leave {room}: {error}"));
                }
            }
            Command::SendRoomMessage { room, body } => {
                if let Some(client) = self.client(out)
                    && let Err(error) = client.say_in_room(&room, &body)
                {
                    out.warn(format!("Message not sent: {error}"));
                }
            }
            Command::SendPrivateMessage { username, body } => {
                if let Some(client) = self.client(out)
                    && let Err(error) = client.send_private_message(&username, &body)
                {
                    out.warn(format!("Message to {username} not sent: {error}"));
                }
            }

            Command::SetSharedDirs(dirs) => self.set_shares(dirs, out),
            Command::SetUploadSlots(slots) => {
                self.config.upload_slots = slots;
                if let Some(client) = self.client(out) {
                    client.set_upload_slots(slots);
                }
            }
        }
    }

    fn poll(&mut self, out: &EventSink) {
        let Some(client) = self.client.clone() else { return };

        if let Some(loss) = client.session_loss() {
            let reason = match loss {
                SessionLoss::Displaced => Disconnect::LoggedInElsewhere,
                SessionLoss::Disconnected => Disconnect::Lost(loss.to_string()),
            };
            self.disconnect(reason, out);
            return;
        }

        let active: Vec<SearchId> = self.searches.keys().copied().collect();
        for id in active {
            self.drain_search(id, out);
            let finished = self
                .searches
                .get(&id)
                .is_some_and(|search| search.done.load(Ordering::Acquire));
            if finished {
                self.finish_search(id, out);
            }
        }

        self.drain_transfers(out);
        Self::drain_rooms(&client, out);
        self.drain_browses(&client, out);
        self.report_shares(&client, out);
    }

    fn shutdown(&mut self) {
        self.stop_all_searches();
        self.transfers.clear();
        self.client = None;
    }
}

// --- translation from library types to domain types ---

fn convert_hit(result: &SearchResult) -> SearchHit {
    SearchHit {
        username: result.username.clone(),
        files: result
            .files
            .iter()
            .map(|file| file_entry(file.name.clone(), file.size, &file.attribs))
            .collect(),
        free_slots: u32::from(result.slots),
        speed: result.speed,
    }
}

fn convert_status(status: &DownloadStatus) -> TransferState {
    match *status {
        DownloadStatus::Queued => TransferState::Queued { place: None },
        DownloadStatus::InProgress { bytes_downloaded, total_bytes, speed_bytes_per_sec } => {
            TransferState::Active {
                transferred: bytes_downloaded,
                total: total_bytes,
                bytes_per_sec: speed_bytes_per_sec,
            }
        }
        DownloadStatus::Paused { bytes_downloaded, total_bytes } => {
            TransferState::Paused { transferred: bytes_downloaded, total: total_bytes }
        }
        DownloadStatus::Completed => TransferState::Completed,
        DownloadStatus::Failed(ref reason) => TransferState::Failed { reason: reason.clone() },
        DownloadStatus::TimedOut => TransferState::TimedOut,
    }
}

fn convert_room_event(event: RoomEvent) -> Event {
    match event {
        RoomEvent::List(rooms) => Event::RoomList(
            rooms
                .into_iter()
                .map(|room| Room { name: room.name, user_count: room.user_count })
                .collect(),
        ),
        RoomEvent::Joined { room, users } => Event::RoomJoined { room, users },
        RoomEvent::Left { room } => Event::RoomLeft { room },
        RoomEvent::Message { room, username, message } => Event::RoomMessage {
            room,
            message: ChatMessage { author: username, body: message },
        },
        RoomEvent::UserJoined { room, username } => {
            Event::RoomPresence { room, username, joined: true }
        }
        RoomEvent::UserLeft { room, username } => {
            Event::RoomPresence { room, username, joined: false }
        }
    }
}

fn convert_directory(directory: soulseek_rs::SharedDirectory) -> SharedDirectory {
    SharedDirectory {
        files: directory
            .files
            .into_iter()
            .map(|(name, size)| FileEntry {
                path: name,
                size,
                bitrate: None,
                duration: None,
                vbr: false,
            })
            .collect(),
        path: directory.name,
    }
}

/// Translate a library user snapshot, for the caller that asked about a peer.
#[must_use]
pub fn convert_user(info: &soulseek_rs::UserInfo) -> UserSummary {
    UserSummary {
        username: info.username.clone(),
        presence: info.presence.map(|presence| match presence.status {
            UserStatus::Offline => Presence::Offline,
            UserStatus::Away => Presence::Away,
            UserStatus::Online => Presence::Online,
        }),
        shared_files: info.stats.map(|stats| stats.shared_files),
        shared_directories: info.stats.map(|stats| stats.shared_folders),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap as Map;
    use std::sync::mpsc;
    use std::time::Duration;

    fn sink() -> (EventSink, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel();
        (EventSink::new(tx), rx)
    }

    #[test]
    fn commands_without_a_connection_warn_rather_than_panic() {
        let (out, rx) = sink();
        let mut backend = LiveBackend::new();

        backend.execute(Command::RequestRoomList, &out);
        assert!(matches!(rx.try_recv(), Ok(Event::Warning(_))));

        backend.execute(Command::JoinRoom("nicotine".into()), &out);
        assert!(matches!(rx.try_recv(), Ok(Event::Warning(_))));
    }

    #[test]
    fn a_search_without_a_connection_still_finishes() {
        let (out, rx) = sink();
        let mut backend = LiveBackend::new();

        backend.execute(Command::Search { id: SearchId(1), query: "boards".into() }, &out);

        assert!(matches!(rx.try_recv(), Ok(Event::Warning(_))));
        assert_eq!(rx.try_recv().unwrap(), Event::SearchFinished { id: SearchId(1) });
        assert!(
            backend.searches.is_empty(),
            "a search that never started must not be left registered"
        );
    }

    #[test]
    fn polling_while_disconnected_does_nothing() {
        let (out, rx) = sink();
        let mut backend = LiveBackend::new();
        backend.poll(&out);
        assert!(rx.try_recv().is_err(), "an idle disconnected backend should stay silent");
    }

    #[test]
    fn cancelling_a_transfer_reports_it_even_when_unknown() {
        let (out, rx) = sink();
        let mut backend = LiveBackend::new();
        let id = TransferId::new("peer", "music/a.flac");

        backend.execute(Command::CancelTransfer(id.clone()), &out);

        assert!(matches!(rx.try_recv(), Ok(Event::Warning(_))), "warns about no connection");
        assert_eq!(
            rx.try_recv().unwrap(),
            Event::TransferUpdated { id, state: TransferState::Cancelled }
        );
    }

    #[test]
    fn an_empty_download_directory_is_repaired() {
        // Kept free of `connect`, which would dial the real Soulseek server
        // and make this a slow, network-dependent test.
        let blank = Config { download_dir: PathBuf::new(), ..Config::default() };
        let fixed = blank.normalized();

        assert!(
            !fixed.download_dir.as_os_str().is_empty(),
            "a blank download directory must be replaced with a real one"
        );
        assert_eq!(fixed.download_dir, crate::model::default_download_dir());
    }

    #[test]
    fn zero_upload_slots_and_timeout_are_repaired() {
        let odd = Config { upload_slots: 0, search_timeout: Duration::ZERO, ..Config::default() }
            .normalized();
        assert_eq!(odd.upload_slots, 1, "serving zero uploads would stall every peer");
        assert!(!odd.search_timeout.is_zero(), "a zero search window finds nothing");
    }

    #[test]
    fn download_status_maps_onto_transfer_state() {
        assert_eq!(
            convert_status(&DownloadStatus::InProgress {
                bytes_downloaded: 50,
                total_bytes: 200,
                speed_bytes_per_sec: 1024.0,
            }),
            TransferState::Active { transferred: 50, total: 200, bytes_per_sec: 1024.0 }
        );
        assert_eq!(convert_status(&DownloadStatus::Completed), TransferState::Completed);
        assert_eq!(
            convert_status(&DownloadStatus::Failed(Some("refused".into()))),
            TransferState::Failed { reason: Some("refused".into()) }
        );
        assert_eq!(convert_status(&DownloadStatus::TimedOut), TransferState::TimedOut);
    }

    #[test]
    fn a_search_result_converts_with_its_attributes() {
        let result = SearchResult {
            token: 7,
            username: "peer".into(),
            slots: 1,
            speed: 4096,
            files: vec![soulseek_rs::File {
                username: "peer".into(),
                name: r"share\album\02 track.mp3".into(),
                size: 900,
                attribs: Map::from([(0, 320), (1, 210)]),
            }],
        };

        let hit = convert_hit(&result);
        assert_eq!(hit.username, "peer");
        assert!(hit.is_ready());
        assert_eq!(hit.files.len(), 1);
        assert_eq!(hit.files[0].file_name(), "02 track.mp3");
        assert_eq!(hit.files[0].bitrate, Some(320));
        assert_eq!(hit.files[0].duration, Some(std::time::Duration::from_secs(210)));
    }

    #[test]
    fn room_events_map_onto_ui_events() {
        assert_eq!(
            convert_room_event(RoomEvent::UserLeft {
                room: "nicotine".into(),
                username: "peer".into(),
            }),
            Event::RoomPresence { room: "nicotine".into(), username: "peer".into(), joined: false }
        );
        assert_eq!(
            convert_room_event(RoomEvent::Message {
                room: "nicotine".into(),
                username: "peer".into(),
                message: "hello".into(),
            }),
            Event::RoomMessage {
                room: "nicotine".into(),
                message: ChatMessage { author: "peer".into(), body: "hello".into() },
            }
        );
    }

    #[test]
    fn a_browse_listing_keeps_its_directory_path() {
        let converted = convert_directory(soulseek_rs::SharedDirectory {
            name: r"music\ambient".into(),
            files: vec![("one.flac".into(), 10), ("two.flac".into(), 20)],
        });
        assert_eq!(converted.path, r"music\ambient");
        assert_eq!(converted.files.len(), 2);
        assert_eq!(converted.files[1].size, 20);
    }
}

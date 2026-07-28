//! The desktop application: a Tauri shell around [`lark_core`].
//!
//! This layer is deliberately thin. It owns no protocol knowledge and no
//! application state beyond what is needed to route messages:
//!
//! - each user action becomes a `#[tauri::command]` that forwards a
//!   [`Command`] to the core;
//! - a single forwarding thread owns the core's [`Engine`] and republishes
//!   every event to the `WebView` on [`EVENT_CHANNEL`].
//!
//! Because the core is a plain Rust crate and Tauri's backend is Rust, there
//! is no sidecar process and no serialisation boundary between the interface
//! and the protocol library — only the one crossing into the `WebView`.

// Tauri's command macro takes every extractor and argument by value, so the
// handlers below cannot borrow them however little they consume.
#![allow(clippy::needless_pass_by_value)]

use lark_core::command::SearchIds;
use lark_core::model::{Config, SearchId, TransferId};
use lark_core::{Command, Commander, Engine, LiveBackend};

use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;

use tauri::{AppHandle, Emitter, Manager, State};

/// The single channel every core event is published on. The interface
/// subscribes once and switches on the event's `type` field.
pub const EVENT_CHANNEL: &str = "lark://event";

/// What the command handlers share.
struct App {
    core: Commander,
    /// Hands out search identifiers. Behind a mutex because Tauri invokes
    /// commands from a thread pool.
    searches: Mutex<SearchIds>,
}

impl App {
    /// Forward a command to the core.
    ///
    /// A refused send means the core worker has stopped, which the interface
    /// should surface rather than silently ignore.
    fn send(&self, command: Command) -> Result<(), String> {
        if self.core.send(command) {
            Ok(())
        } else {
            Err("The core has stopped. Restart Lark.".into())
        }
    }
}

/// Start the core and republish its events to the `WebView`.
///
/// The spawned thread owns the [`Engine`]: it is the single consumer of the
/// event channel, and keeping the engine alive here is what keeps the core
/// running for the lifetime of the app.
fn start_core(app: &AppHandle) -> Commander {
    let engine = Engine::spawn(LiveBackend::new());
    let commander = engine.commander();

    let handle = app.clone();
    thread::Builder::new()
        .name("lark-events".into())
        .spawn(move || {
            // `recv` ends only when the engine is dropped, which happens when
            // this thread exits — so the loop ends on app shutdown.
            while let Ok(event) = engine.events().recv() {
                if handle.emit(EVENT_CHANNEL, &event).is_err() {
                    break; // the window is gone; nothing left to publish to
                }
            }
        })
        .expect("spawning the event forwarding thread");

    commander
}

// --- commands: session ---

#[tauri::command]
fn connect(app: State<'_, App>, config: Config) -> Result<(), String> {
    app.send(Command::Connect(Box::new(config)))
}

#[tauri::command]
fn disconnect(app: State<'_, App>) -> Result<(), String> {
    app.send(Command::Disconnect)
}

// --- commands: search ---

/// Start a search and return the id its results will be tagged with.
#[tauri::command]
fn search(app: State<'_, App>, query: String) -> Result<SearchId, String> {
    let id = app.searches.lock().map_err(|_| "Search state is poisoned.")?.next();
    app.send(Command::Search { id, query })?;
    Ok(id)
}

#[tauri::command]
fn cancel_search(app: State<'_, App>, id: SearchId) -> Result<(), String> {
    app.send(Command::CancelSearch(id))
}

// --- commands: transfers ---

#[tauri::command]
fn download(
    app: State<'_, App>,
    username: String,
    path: String,
    size: u64,
) -> Result<(), String> {
    app.send(Command::Download { username, path, size })
}

#[tauri::command]
fn pause_transfer(app: State<'_, App>, username: String, path: String) -> Result<(), String> {
    app.send(Command::PauseTransfer(TransferId::new(username, path)))
}

#[tauri::command]
fn resume_transfer(app: State<'_, App>, username: String, path: String) -> Result<(), String> {
    app.send(Command::ResumeTransfer(TransferId::new(username, path)))
}

#[tauri::command]
fn cancel_transfer(app: State<'_, App>, username: String, path: String) -> Result<(), String> {
    app.send(Command::CancelTransfer(TransferId::new(username, path)))
}

// --- commands: peers ---

#[tauri::command]
fn browse_user(app: State<'_, App>, username: String) -> Result<(), String> {
    app.send(Command::BrowseUser(username))
}

#[tauri::command]
fn request_user_info(app: State<'_, App>, username: String) -> Result<(), String> {
    app.send(Command::RequestUserInfo(username))
}

// --- commands: chat ---

#[tauri::command]
fn request_room_list(app: State<'_, App>) -> Result<(), String> {
    app.send(Command::RequestRoomList)
}

#[tauri::command]
fn join_room(app: State<'_, App>, room: String) -> Result<(), String> {
    app.send(Command::JoinRoom(room))
}

#[tauri::command]
fn leave_room(app: State<'_, App>, room: String) -> Result<(), String> {
    app.send(Command::LeaveRoom(room))
}

#[tauri::command]
fn send_room_message(app: State<'_, App>, room: String, body: String) -> Result<(), String> {
    app.send(Command::SendRoomMessage { room, body })
}

#[tauri::command]
fn send_private_message(
    app: State<'_, App>,
    username: String,
    body: String,
) -> Result<(), String> {
    app.send(Command::SendPrivateMessage { username, body })
}

// --- commands: settings ---

#[tauri::command]
fn set_shared_dirs(app: State<'_, App>, dirs: Vec<PathBuf>) -> Result<(), String> {
    app.send(Command::SetSharedDirs(dirs))
}

#[tauri::command]
fn set_upload_slots(app: State<'_, App>, slots: usize) -> Result<(), String> {
    app.send(Command::SetUploadSlots(slots))
}

/// Build and run the application.
///
/// # Panics
/// If Tauri cannot create the window or the event thread cannot be spawned —
/// both unrecoverable at startup.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let core = start_core(app.handle());
            app.manage(App { core, searches: Mutex::new(SearchIds::new()) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            search,
            cancel_search,
            download,
            pause_transfer,
            resume_transfer,
            cancel_transfer,
            browse_user,
            request_user_info,
            request_room_list,
            join_room,
            leave_room,
            send_room_message,
            send_private_message,
            set_shared_dirs,
            set_upload_slots,
        ])
        .run(tauri::generate_context!())
        .expect("starting the Lark window");
}

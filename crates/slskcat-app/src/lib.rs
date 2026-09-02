//! The desktop application: a Tauri shell around [`slskcat_core`].
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

pub mod settings;

use slskcat_core::command::SearchIds;
use slskcat_core::model::{SearchId, TransferId};
use slskcat_core::{Command, Commander, Engine, LiveBackend};

use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;

use settings::Settings;
use tauri::{AppHandle, Emitter, Manager, State};

/// The single channel every core event is published on. The interface
/// subscribes once and switches on the event's `type` field.
pub const EVENT_CHANNEL: &str = "slskcat://event";

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
            Err("The core has stopped. Restart slsk.cat.".into())
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
        .name("slskcat-events".into())
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

/// Sign in, and persist the settings the session was started with.
///
/// Saving here rather than in the interface means the stored settings always
/// describe a configuration that was actually used to connect.
#[tauri::command]
fn connect(app: State<'_, App>, handle: AppHandle, settings: Settings) -> Result<Settings, String> {
    let config = settings.to_config();
    app.send(Command::Connect(Box::new(config)))?;
    // The wishlist is not part of the core's Config, so it is restated on
    // every sign-in; otherwise a saved wish would sit idle until edited.
    app.send(Command::SetWishlist(settings.wishlist.clone()))?;
    settings::save(&handle, &settings)
}

#[tauri::command]
fn load_settings(handle: AppHandle) -> Result<Settings, String> {
    settings::load(&handle)
}

#[tauri::command]
fn save_settings(handle: AppHandle, settings: Settings) -> Result<Settings, String> {
    settings::save(&handle, &settings)
}

/// Make another known account the current one, and return the settings it
/// describes. Signing in is a separate step, so the form is filled rather than
/// a session started behind the user's back.
#[tauri::command]
fn switch_account(handle: AppHandle, username: String) -> Result<Settings, String> {
    settings::switch(&handle, &username)
}

#[tauri::command]
fn forget_account(handle: AppHandle, username: String) -> Result<Settings, String> {
    settings::forget(&handle, &username)
}

#[tauri::command]
fn disconnect(app: State<'_, App>) -> Result<(), String> {
    app.send(Command::Disconnect)
}

// --- commands: search ---

/// Start a search and return the id its results will be tagged with.
#[tauri::command]
fn search(app: State<'_, App>, query: String) -> Result<SearchId, String> {
    let id = app
        .searches
        .lock()
        .map_err(|_| "Search state is poisoned.")?
        .next();
    app.send(Command::Search { id, query })?;
    Ok(id)
}

#[tauri::command]
fn cancel_search(app: State<'_, App>, id: SearchId) -> Result<(), String> {
    app.send(Command::CancelSearch(id))
}

#[tauri::command]
fn set_wishlist(app: State<'_, App>, queries: Vec<String>) -> Result<(), String> {
    app.send(Command::SetWishlist(queries))
}

// --- commands: transfers ---

#[tauri::command]
fn download(app: State<'_, App>, username: String, path: String, size: u64) -> Result<(), String> {
    app.send(Command::Download {
        username,
        path,
        size,
    })
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

#[tauri::command]
fn cancel_upload(app: State<'_, App>, username: String, path: String) -> Result<(), String> {
    app.send(Command::CancelUpload(TransferId::new(username, path)))
}

/// Classify a directory before it is offered to the network.
///
/// The core enforces this regardless, but the interface asks first so a
/// refusal can be explained at the moment of choosing rather than after.
#[tauri::command]
fn assess_share(path: PathBuf) -> ShareVerdict {
    let risk = slskcat_core::assess_share_path(&path);
    ShareVerdict {
        allowed: risk.is_allowed(),
        sensitive: matches!(risk, slskcat_core::ShareRisk::Sensitive(_)),
        reason: risk.reason().map(str::to_owned),
    }
}

/// The interface-facing form of [`slskcat_core::ShareRisk`].
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareVerdict {
    pub allowed: bool,
    pub sensitive: bool,
    pub reason: Option<String>,
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
fn send_private_message(app: State<'_, App>, username: String, body: String) -> Result<(), String> {
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

#[tauri::command]
fn set_download_slots(app: State<'_, App>, slots: usize) -> Result<(), String> {
    app.send(Command::SetDownloadSlots(slots))
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
            app.manage(App {
                core,
                searches: Mutex::new(SearchIds::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            load_settings,
            save_settings,
            switch_account,
            forget_account,
            search,
            cancel_search,
            set_wishlist,
            download,
            pause_transfer,
            resume_transfer,
            cancel_transfer,
            cancel_upload,
            assess_share,
            browse_user,
            request_user_info,
            request_room_list,
            join_room,
            leave_room,
            send_room_message,
            send_private_message,
            set_shared_dirs,
            set_upload_slots,
            set_download_slots,
        ])
        .run(tauri::generate_context!())
        .expect("starting the slsk.cat window");
}

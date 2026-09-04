/**
 * The one place that talks to Rust.
 *
 * Every function here is a thin typed wrapper over a `#[tauri::command]`, and
 * `onEvent` subscribes to the single channel the core publishes on. Keeping
 * this boundary in one file means the rest of the interface never imports
 * Tauri directly.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { hydrateSettings } from "./types";
import type { CoreEvent, LocalRoot, SearchId, Settings, ShareVerdict } from "./types";

/** Must match `slskcat_app::EVENT_CHANNEL`. */
const EVENT_CHANNEL = "slskcat://event";

/** Subscribe to core events. Resolves to an unsubscribe function. */
export function onEvent(handler: (event: CoreEvent) => void): Promise<UnlistenFn> {
  return listen<CoreEvent>(EVENT_CHANNEL, (message) => handler(message.payload));
}

export const core = {
  /** Signs in and persists the settings the session was started with. */
  connect: (settings: Settings) =>
    invoke<Partial<Settings>>("connect", { settings }).then(hydrateSettings),
  loadSettings: () => invoke<Partial<Settings>>("load_settings").then(hydrateSettings),
  saveSettings: (settings: Settings) =>
    invoke<Partial<Settings>>("save_settings", { settings }).then(hydrateSettings),
  /** Make another known account current. Signing in stays a separate step. */
  switchAccount: (username: string) =>
    invoke<Partial<Settings>>("switch_account", { username }).then(hydrateSettings),
  forgetAccount: (username: string) =>
    invoke<Partial<Settings>>("forget_account", { username }).then(hydrateSettings),
  disconnect: () => invoke<void>("disconnect"),

  /** Starts a search and resolves with the id its hits will carry. */
  search: (query: string) => invoke<SearchId>("search", { query }),
  cancelSearch: (id: SearchId) => invoke<void>("cancel_search", { id }),
  /** Replace the standing wishlist. Stated whole, not added to. */
  setWishlist: (queries: string[]) => invoke<void>("set_wishlist", { queries }),

  download: (username: string, path: string, size: number) =>
    invoke<void>("download", { username, path, size }),
  pauseTransfer: (username: string, path: string) =>
    invoke<void>("pause_transfer", { username, path }),
  resumeTransfer: (username: string, path: string) =>
    invoke<void>("resume_transfer", { username, path }),
  cancelTransfer: (username: string, path: string) =>
    invoke<void>("cancel_transfer", { username, path }),
  cancelUpload: (username: string, path: string) =>
    invoke<void>("cancel_upload", { username, path }),

  /** Ask whether a directory is safe to share, before offering it. */
  assessShare: (path: string) => invoke<ShareVerdict>("assess_share", { path }),

  browseUser: (username: string) => invoke<void>("browse_user", { username }),
  requestUserInfo: (username: string) =>
    invoke<void>("request_user_info", { username }),

  requestRoomList: () => invoke<void>("request_room_list"),
  joinRoom: (room: string) => invoke<void>("join_room", { room }),
  leaveRoom: (room: string) => invoke<void>("leave_room", { room }),
  sendRoomMessage: (room: string, body: string) =>
    invoke<void>("send_room_message", { room, body }),
  sendPrivateMessage: (username: string, body: string) =>
    invoke<void>("send_private_message", { username, body }),

  setSharedDirs: (dirs: string[]) => invoke<void>("set_shared_dirs", { dirs }),
  setUploadSlots: (slots: number) => invoke<void>("set_upload_slots", { slots }),
  setDownloadSlots: (slots: number) => invoke<void>("set_download_slots", { slots }),
  /** Everything already in the download folder, as name and size. */
  downloadedFiles: () => invoke<{ name: string; size: number }[]>("downloaded_files"),
  /** This machine's own folders: where downloads land, and what is shared. */
  localLibrary: () => invoke<LocalRoot[]>("local_library"),
};

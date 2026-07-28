/**
 * TypeScript mirrors of the core's domain types.
 *
 * These correspond one-to-one with `slskcat_core::model` and
 * `slskcat_core::event`. Enums carrying data are serialised adjacently tagged
 * (`{ type, data }`), which is what lets a `switch` on `type` narrow the
 * payload.
 */

export type SearchId = number;

export interface FileEntry {
  path: string;
  size: number;
  /** Kbps, or null when the peer did not report it. */
  bitrate: number | null;
  /** Whole seconds, or null when unknown. */
  duration: number | null;
  vbr: boolean;
}

export interface SearchHit {
  username: string;
  files: FileEntry[];
  freeSlots: number;
  /** Bytes per second, as the peer reports it. */
  speed: number;
}

export interface TransferId {
  username: string;
  path: string;
}

export type TransferState =
  | { type: "queued"; data: { place: number | null } }
  | {
      type: "active";
      data: { transferred: number; total: number; bytesPerSec: number };
    }
  | { type: "paused"; data: { transferred: number; total: number } }
  | { type: "completed" }
  | { type: "failed"; data: { reason: string | null } }
  | { type: "cancelled" }
  | { type: "timedOut" };

export interface SharedDirectory {
  path: string;
  files: FileEntry[];
}

export interface Room {
  name: string;
  userCount: number;
}

export interface ChatMessage {
  author: string;
  body: string;
}

export type Presence = "offline" | "away" | "online";

export interface UserSummary {
  username: string;
  presence: Presence | null;
  sharedFiles: number | null;
  sharedDirectories: number | null;
}

export interface Credentials {
  username: string;
  password: string;
}

export interface Config {
  credentials: Credentials;
  downloadDir: string;
  sharedDirs: string[];
  uploadSlots: number;
  /** Whole seconds. */
  searchTimeout: number;
}

/**
 * Persisted preferences, as `slskcat_app::settings::Settings`.
 *
 * `password` is never written to the settings file — it round-trips through
 * the OS credential store — and `keychainAvailable` reports whether that
 * store could actually be reached.
 */
export interface Settings {
  username: string;
  password: string;
  rememberPassword: boolean;
  downloadDir: string;
  sharedDirs: string[];
  uploadSlots: number;
  searchTimeoutSecs: number;
  keychainAvailable: boolean;
}

export type Disconnect =
  | { type: "requested" }
  | { type: "loggedInElsewhere" }
  | { type: "lost"; data: string };

export type CoreEvent =
  | { type: "connected"; data: { username: string } }
  | { type: "loginFailed"; data: { reason: string } }
  | { type: "disconnected"; data: Disconnect }
  | { type: "searchHits"; data: { id: SearchId; hits: SearchHit[] } }
  | { type: "searchFinished"; data: { id: SearchId } }
  | { type: "transferUpdated"; data: { id: TransferId; state: TransferState } }
  | {
      type: "browseReady";
      data: { username: string; directories: SharedDirectory[] };
    }
  | { type: "userUpdated"; data: UserSummary }
  | { type: "roomList"; data: Room[] }
  | { type: "roomJoined"; data: { room: string; users: string[] } }
  | { type: "roomLeft"; data: { room: string } }
  | { type: "roomMessage"; data: { room: string; message: ChatMessage } }
  | {
      type: "roomPresence";
      data: { room: string; username: string; joined: boolean };
    }
  | { type: "privateMessage"; data: ChatMessage }
  | { type: "sharesUpdated"; data: { directories: number; files: number } }
  | { type: "warning"; data: string };

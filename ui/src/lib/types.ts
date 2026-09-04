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

export type UploadState =
  | { type: "queued"; data: { place: number } }
  | { type: "active" }
  | { type: "completed" }
  | { type: "cancelled" }
  | { type: "failed"; data: { reason: string } };

export interface Upload {
  username: string;
  path: string;
  size: number;
  sent: number;
  state: UploadState;
  bytesPerSec: number;
}

/** What sharing a directory would expose, as judged by the core. */
export interface ShareVerdict {
  allowed: boolean;
  sensitive: boolean;
  reason: string | null;
}

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
  /** Bytes per second, as the server records it. */
  averageSpeed: number | null;
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
  /** Downloads allowed to run at once. */
  downloadSlots: number;
  /** Route the session through a proxy, or null to connect directly. */
  proxy: Proxy | null;
  searchTimeoutSecs: number;
  wishlist: string[];
  /** Interface scale as a percentage; 100 is the designed size. */
  uiScale: number;
  /** Rooms to rejoin on sign-in. The server remembers none between sessions. */
  rooms: string[];
  /** People kept across sessions. Local only; the network has no such list. */
  friends: string[];
  /** Known accounts on this machine, most recently used first. */
  accounts: string[];
  keychainAvailable: boolean;
}

/**
 * A settings object to fall back on when the stored one could not be loaded.
 *
 * Defined once: a hand-written literal at each call site silently goes stale
 * every time `Settings` gains a field, and the compiler only catches it where
 * the literal happens to be assigned to the full type.
 */
export function defaultSettings(): Settings {
  return {
    username: "",
    password: "",
    rememberPassword: false,
    downloadDir: "",
    sharedDirs: [],
    uploadSlots: 2,
    downloadSlots: 4,
    proxy: null,
    searchTimeoutSecs: 12,
    wishlist: [],
    uiScale: 100,
    rooms: [],
    friends: [],
    accounts: [],
    keychainAvailable: true,
  };
}

/**
 * A `Settings` with every field present, whatever the backend actually sent.
 *
 * The commands are typed as returning the whole shape, but a field the backend
 * omits arrives as `undefined` behind a type that promises a string — and the
 * first `.length` taken on it throws inside a `$derived`, which freezes
 * whatever was bound to it. That is not hypothetical: `password` was
 * `#[serde(skip)]` in Rust, and the sign-in button sat permanently disabled
 * because of it. Filling the gaps at the boundary keeps a backend mistake from
 * turning into a dead form.
 */
export function hydrateSettings(raw: Partial<Settings> | null | undefined): Settings {
  const base = defaultSettings();
  const from = raw ?? {};
  return {
    username: from.username ?? base.username,
    password: from.password ?? base.password,
    rememberPassword: from.rememberPassword ?? base.rememberPassword,
    downloadDir: from.downloadDir ?? base.downloadDir,
    sharedDirs: from.sharedDirs ?? base.sharedDirs,
    uploadSlots: from.uploadSlots ?? base.uploadSlots,
    downloadSlots: from.downloadSlots ?? base.downloadSlots,
    proxy: from.proxy ?? base.proxy,
    searchTimeoutSecs: from.searchTimeoutSecs ?? base.searchTimeoutSecs,
    wishlist: from.wishlist ?? base.wishlist,
    uiScale: from.uiScale ?? base.uiScale,
    rooms: from.rooms ?? base.rooms,
    friends: from.friends ?? base.friends,
    accounts: from.accounts ?? base.accounts,
    keychainAvailable: from.keychainAvailable ?? base.keychainAvailable,
  };
}

/** Which protocol a proxy speaks. */
export type ProxyKind = "http" | "socks4" | "socks5";

export interface Proxy {
  kind: ProxyKind;
  host: string;
  port: number;
  username: string;
  password: string;
}

/** An empty proxy, which the core reads as "connect directly". */
export function blankProxy(): Proxy {
  return { kind: "socks5", host: "", port: 1080, username: "", password: "" };
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
  | { type: "wishlistHits"; data: { query: string; hits: SearchHit[] } }
  | { type: "wishlistInterval"; data: { seconds: number } }
  | { type: "transferUpdated"; data: { id: TransferId; state: TransferState } }
  | { type: "uploadUpdated"; data: Upload }
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

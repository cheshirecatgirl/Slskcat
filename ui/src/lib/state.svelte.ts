/**
 * Application state, and the single place core events are applied.
 *
 * The core already guarantees ordering and delivers only deltas, so this is a
 * straightforward reducer: every field below is derived from the event stream
 * and nothing polls.
 */

import type {
  ChatMessage,
  CoreEvent,
  Room,
  SearchHit,
  SearchId,
  Settings,
  SharedDirectory,
  Upload,
  UploadState,
  TransferState,
  UserSummary,
} from "./types";

/** One row in the results table: a file, plus who is offering it. */
export interface ResultRow {
  username: string;
  freeSlots: number;
  speed: number;
  path: string;
  size: number;
  bitrate: number | null;
  duration: number | null;
}

/** A search and everything it has found. */
export class Search {
  readonly id: SearchId;
  readonly query: string;
  running = $state(true);
  rows = $state<ResultRow[]>([]);
  /** Distinct peers that have answered, for the summary line. */
  peers = $state(0);

  constructor(id: SearchId, query: string) {
    this.id = id;
    this.query = query;
  }

  /** Peer and path of every row held, so a repeat is recognised. */
  #seen = new Set<string>();
  /** Peers counted, so one answering twice is still one peer. */
  #answered = new Set<string>();

  add(hits: SearchHit[]) {
    const incoming: ResultRow[] = [];
    for (const hit of hits) {
      this.#answered.add(hit.username);
      for (const file of hit.files) {
        // A peer can answer the same search more than once, and a reply can
        // repeat a file already sent. Listing it twice reads as two copies of
        // something there is only one of.
        const seen = `${hit.username}\0${file.path}`;
        if (this.#seen.has(seen)) continue;
        this.#seen.add(seen);
        incoming.push({
          username: hit.username,
          freeSlots: hit.freeSlots,
          speed: hit.speed,
          path: file.path,
          size: file.size,
          bitrate: file.bitrate,
          duration: file.duration,
        });
      }
    }
    if (incoming.length > 0) {
      // Reassign rather than push: Svelte tracks the array reference.
      this.rows = [...this.rows, ...incoming];
    }
    this.peers = this.#answered.size;
  }
}

/** A transfer as the interface knows it. */
export interface Transfer {
  username: string;
  path: string;
  state: TransferState;
}

/** A transient message shown to the user and then forgotten. */
export interface Notice {
  id: number;
  text: string;
  tone: "warn" | "danger";
}

/**
 * The map key for one transfer or upload.
 *
 * NUL separates the halves because it is the one byte that can appear in
 * neither: with a space, the peer `bob` offering `a b` and the peer `bob a`
 * offering `b` produce the same key and overwrite each other's row.
 *
 * Written as the escape `\0`, never as a literal NUL byte. A raw NUL in the
 * source makes git classify this file as binary, and every change to the
 * reducer the whole interface is built on then shows up as "Binary files
 * differ" — invisible in a diff, a review, or a blame.
 */
const key = (username: string, path: string) => `${username}\0${path}`;

/** Hits kept per standing wish. Past this, the oldest are dropped. */
const WISH_HIT_LIMIT = 500;

/**
 * Whether a transfer can still change on its own.
 *
 * Defined once and shared: the badge count, the filters and the clearing all
 * have to agree on what "finished" means, and three copies of one `switch` is
 * how they stop agreeing.
 */
export const isLiveTransfer = (state: TransferState): boolean =>
  state.type === "queued" || state.type === "active" || state.type === "paused";

/** The same for an upload, which has no paused state. */
export const isLiveUpload = (state: UploadState): boolean =>
  state.type === "queued" || state.type === "active";

export class AppState {
  // --- settings ---
  /** Null until the first load resolves. */
  settings = $state<Settings | null>(null);
  settingsError = $state<string | null>(null);

  // --- session ---
  connected = $state(false);
  connecting = $state(false);
  username = $state("");
  loginError = $state<string | null>(null);
  /**
   * True when the server handed this account to another login.
   *
   * The one thing on this network that signs you out on purpose: the server
   * keeps a single session per name, so a second sign-in cuts the first. It is
   * kept apart from `loginError` because it is not a failure to fix — the
   * credentials were right — and the way back is one button, not a form.
   */
  displaced = $state(false);
  /**
   * True while signing in without anyone having pressed anything.
   *
   * Kept apart from `connecting` so the two look different: an automatic
   * sign-in can replace the form with a line, because there is nothing to fill
   * in, while a sign-in someone just asked for must leave their form on screen
   * — if the password was wrong, taking the form away and putting it back is
   * a flash of the wrong thing at the moment they need to read it.
   */
  resuming = $state(false);
  /** True while the sign-in form is open to add an account, session intact. */
  addingAccount = $state(false);
  shares = $state<{ directories: number; files: number } | null>(null);

  // --- search ---
  searches = $state<Search[]>([]);
  activeSearch = $state<SearchId | null>(null);

  // --- wishlist ---
  /** Hits found for each standing wish, newest first. */
  wishHits = $state<Record<string, ResultRow[]>>({});
  /** How often the server lets wishes be re-sent, in seconds. */
  wishInterval = $state<number | null>(null);

  // --- transfers ---
  /** Keyed by user+path so an update replaces its row rather than adding one. */
  transfers = $state<Record<string, Transfer>>({});

  /** Uploads, keyed the same way transfers are. */
  uploads = $state<Record<string, Upload>>({});

  // --- browse ---
  browsing = $state<string | null>(null);
  browseResult = $state<SharedDirectory[] | null>(null);

  // --- chat ---
  rooms = $state<Room[]>([]);
  joined = $state<string[]>([]);
  roomMessages = $state<Record<string, ChatMessage[]>>({});
  roomUsers = $state<Record<string, string[]>>({});
  /** Direct-message threads, keyed by the other person. */
  conversations = $state<Record<string, ChatMessage[]>>({});

  // --- peers ---
  users = $state<Record<string, UserSummary>>({});

  // --- what is already on disk ---
  /**
   * Files in the download folder, keyed exactly as a result would be.
   *
   * Name and size, because the protocol carries no hash for a search result
   * and never has. Two unrelated files agreeing on both an exact filename and
   * an exact byte count is possible; it is a rarer mistake than making the
   * user re-download something they already have.
   */
  downloaded = $state<Set<string>>(new Set());

  // --- notices ---
  notices = $state<Notice[]>([]);
  #nextNotice = 0;

  get transferList(): Transfer[] {
    return Object.values(this.transfers);
  }

  get uploadList(): Upload[] {
    return Object.values(this.uploads);
  }

  /** Uploads still in flight, which is what the Transfers badge counts. */
  get activeUploads(): number {
    return this.uploadList.filter((u) => isLiveUpload(u.state)).length;
  }

  /** Transfers that can still change, which is what the badge counts. */
  get activeTransfers(): number {
    return this.transferList.filter((t) => isLiveTransfer(t.state)).length;
  }

  /**
   * Forget every transfer and upload that can no longer change.
   *
   * Deliberately not a command. Clearing used to cancel each finished row
   * instead, which asked the core to cancel things that had already finished;
   * the core answered the only way it could, by reporting them cancelled, so
   * a completed download turned into a cancelled one and stayed on screen.
   * There is nothing to cancel here — the rows are dropped, and cancelled
   * ones go with them.
   */
  clearFinished() {
    this.transfers = Object.fromEntries(
      Object.entries(this.transfers).filter(([, t]) => isLiveTransfer(t.state)),
    );
    this.uploads = Object.fromEntries(
      Object.entries(this.uploads).filter(([, u]) => isLiveUpload(u.state)),
    );
  }

  /** Everything still moving, in either direction. */
  get activeWork(): number {
    return this.activeTransfers + this.activeUploads;
  }

  get search(): Search | null {
    return this.searches.find((s) => s.id === this.activeSearch) ?? null;
  }

  /**
   * Everyone this session has seen, sorted.
   *
   * The server publishes a room list but no user list — there is no message
   * that asks for one — so "everybody" is not available and is not what this
   * is. It is the people who have actually appeared: the members of rooms
   * joined, the peers who answered a search, anyone messaged, and anyone
   * transferred with. On a busy room that is hundreds of names the moment you
   * walk in, and none of it costs a request.
   */
  get knownUsers(): string[] {
    const seen = new Set<string>();
    for (const members of Object.values(this.roomUsers)) {
      for (const member of members) seen.add(member);
    }
    for (const peer of Object.keys(this.conversations)) seen.add(peer);
    for (const peer of Object.keys(this.users)) seen.add(peer);
    for (const search of this.searches) {
      for (const row of search.rows) seen.add(row.username);
    }
    for (const transfer of this.transferList) seen.add(transfer.username);
    for (const upload of this.uploadList) seen.add(upload.username);
    seen.delete(this.username);
    return [...seen].sort((a, b) => a.localeCompare(b));
  }

  /** The key a transfer is filed under, for looking one up by peer and path. */
  static key(username: string, path: string): string {
    return key(username, path);
  }

  /** The key a result and a local file have to agree on to be the same file. */
  static had(name: string, size: number): string {
    return `${name.toLowerCase()}\0${size}`;
  }

  notify(text: string, tone: Notice["tone"] = "warn") {
    const id = this.#nextNotice++;
    this.notices = [...this.notices, { id, text, tone }];
    // Notices are informational; leaving them on screen forever would turn
    // a busy network into a wall of text.
    setTimeout(() => this.dismiss(id), 6000);
  }

  dismiss(id: number) {
    this.notices = this.notices.filter((n) => n.id !== id);
  }

  /** Append a line to a direct-message thread. */
  addMessage(peer: string, message: ChatMessage) {
    const thread = this.conversations[peer] ?? [];
    this.conversations = { ...this.conversations, [peer]: [...thread, message] };
  }

  /**
   * Forget a search and everything it found.
   *
   * Searches were only ever added. Eight tabs are shown but every search ever
   * run stayed in memory with all of its rows, so an evening's use held tens
   * of thousands of results nobody was looking at any more — and there was no
   * way to put one down.
   */
  closeSearch(id: SearchId) {
    this.searches = this.searches.filter((search) => search.id !== id);
    if (this.activeSearch === id) {
      this.activeSearch = this.searches[0]?.id ?? null;
    }
  }

  startSearch(id: SearchId, query: string) {
    const search = new Search(id, query);
    this.searches = [search, ...this.searches];
    this.activeSearch = id;
  }

  /** Apply one core event. */
  apply(event: CoreEvent) {
    switch (event.type) {
      case "connected":
        this.connected = true;
        this.connecting = false;
        this.resuming = false;
        this.username = event.data.username;
        this.loginError = null;
        this.displaced = false;
        this.addingAccount = false;
        break;

      case "loginFailed":
        this.connected = false;
        this.connecting = false;
        this.resuming = false;
        // Whatever ended the last session, the current story is this refusal.
        // Leaving `displaced` set would keep a "go back online" button on
        // screen next to the reason it just failed to.
        this.displaced = false;
        this.loginError = event.data.reason;
        break;

      case "disconnected": {
        this.connected = false;
        this.connecting = false;
        this.resuming = false;
        const why = event.data;
        this.displaced = why.type === "loggedInElsewhere";
        if (why.type === "lost") {
          this.notify(why.data, "danger");
        }
        break;
      }

      case "searchHits":
        this.searches.find((s) => s.id === event.data.id)?.add(event.data.hits);
        break;

      case "searchFinished": {
        const search = this.searches.find((s) => s.id === event.data.id);
        if (search) search.running = false;
        break;
      }

      case "wishlistHits": {
        const { query, hits } = event.data;
        const rows = hits.flatMap((hit) =>
          hit.files.map((file) => ({
            username: hit.username,
            freeSlots: hit.freeSlots,
            speed: hit.speed,
            path: file.path,
            size: file.size,
            bitrate: file.bitrate,
            duration: file.duration,
          })),
        );
        // Newest first: a wish is checked periodically, so the interesting
        // hits are the ones that just turned up.
        // Newest first, and capped: a wish is re-run for as long as it is
        // kept, so an untended one would grow without limit at whatever
        // interval the server allows.
        const kept = [...rows, ...(this.wishHits[query] ?? [])].slice(0, WISH_HIT_LIMIT);
        this.wishHits = { ...this.wishHits, [query]: kept };
        break;
      }

      case "wishlistInterval":
        this.wishInterval = event.data.seconds;
        break;

      case "transferUpdated": {
        const id = event.data.id;
        this.transfers = {
          ...this.transfers,
          [key(id.username, id.path)]: {
            username: id.username,
            path: id.path,
            state: event.data.state,
          },
        };
        break;
      }

      case "uploadUpdated": {
        const upload = event.data;
        this.uploads = {
          ...this.uploads,
          [key(upload.username, upload.path)]: upload,
        };
        break;
      }

      case "browseReady":
        this.browsing = event.data.username;
        this.browseResult = event.data.directories;
        break;

      case "userUpdated":
        this.users = { ...this.users, [event.data.username]: event.data };
        break;

      case "roomList":
        this.rooms = event.data;
        break;

      case "roomJoined":
        if (!this.joined.includes(event.data.room)) {
          this.joined = [...this.joined, event.data.room];
        }
        this.roomUsers = { ...this.roomUsers, [event.data.room]: event.data.users };
        break;

      case "roomLeft":
        this.joined = this.joined.filter((r) => r !== event.data.room);
        break;

      case "roomMessage": {
        const room = event.data.room;
        const existing = this.roomMessages[room] ?? [];
        this.roomMessages = { ...this.roomMessages, [room]: [...existing, event.data.message] };
        break;
      }

      case "roomPresence": {
        const { room, username, joined } = event.data;
        const users = this.roomUsers[room] ?? [];
        this.roomUsers = {
          ...this.roomUsers,
          [room]: joined
            ? users.includes(username)
              ? users
              : [...users, username]
            : users.filter((u) => u !== username),
        };
        break;
      }

      case "privateMessage":
        this.addMessage(event.data.author, event.data);
        break;

      case "sharesUpdated":
        this.shares = event.data;
        break;

      case "warning":
        this.notify(event.data);
        break;
    }
  }
}

export const app = new AppState();

/**
 * Send a command and say so if it fails.
 *
 * `void promise` drops the rejection: the command silently did not happen, and
 * nothing on screen changed to admit it. Anything fired without waiting for an
 * answer should go through here.
 */
export function fire(action: Promise<unknown>) {
  void action.catch((error: unknown) => app.notify(String(error), "danger"));
}

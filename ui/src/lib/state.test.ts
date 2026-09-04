import { describe, expect, it } from "vitest";
import { AppState, isLiveTransfer, isLiveUpload } from "./state.svelte";
import type { CoreEvent, Upload } from "./types";

/** A search hit shaped the way the core sends one. */
function hit(username: string, path: string, size = 10) {
  return {
    username,
    freeSlots: 1,
    speed: 1000,
    files: [{ path, size, bitrate: 320, duration: 100, vbr: false }],
  };
}

function upload(username: string, path: string): Upload {
  return { username, path, size: 10, sent: 0, state: { type: "active" }, bytesPerSec: 1 };
}

describe("transfer keys", () => {
  it("cannot be confused by a space in a name or a path", () => {
    // With a space separator, peer `bob` offering `a b` and peer `bob a`
    // offering `b` produced the same key and evicted each other's row.
    expect(AppState.key("bob", "a b")).not.toBe(AppState.key("bob a", "b"));
  });

  it("files a transfer and an upload for the same peer separately", () => {
    const app = new AppState();
    app.apply({
      type: "transferUpdated",
      data: { id: { username: "peer", path: "x.flac" }, state: { type: "completed" } },
    });
    app.apply({ type: "uploadUpdated", data: upload("peer", "x.flac") });
    expect(app.transferList).toHaveLength(1);
    expect(app.uploadList).toHaveLength(1);
  });
});

describe("what counts as finished", () => {
  it("agrees across the badge, the filter and the clearing", () => {
    expect(isLiveTransfer({ type: "queued", data: { place: null } })).toBe(true);
    expect(isLiveTransfer({ type: "paused", data: { transferred: 1, total: 2 } })).toBe(true);
    expect(isLiveTransfer({ type: "completed" })).toBe(false);
    expect(isLiveTransfer({ type: "cancelled" })).toBe(false);
    expect(isLiveUpload({ type: "queued", data: { place: 1 } })).toBe(true);
    expect(isLiveUpload({ type: "completed" })).toBe(false);
  });
});

describe("clearing finished transfers", () => {
  it("removes them instead of cancelling them", () => {
    // Clearing used to ask the core to cancel finished rows, so a completed
    // download became a cancelled one and stayed exactly where it was.
    const app = new AppState();
    for (const [path, state] of [
      ["done.flac", { type: "completed" }],
      ["gone.flac", { type: "cancelled" }],
      ["going.flac", { type: "active", data: { transferred: 1, total: 2, bytesPerSec: 1 } }],
    ] as const) {
      app.apply({
        type: "transferUpdated",
        data: { id: { username: "peer", path }, state },
      } as CoreEvent);
    }
    app.apply({
      type: "uploadUpdated",
      data: { ...upload("peer", "sent.flac"), state: { type: "completed" } },
    });

    expect(app.transferList).toHaveLength(3);
    app.clearFinished();

    expect(app.transferList.map((t) => t.path)).toEqual(["going.flac"]);
    expect(app.uploadList, "uploads were counted but never cleared").toHaveLength(0);
  });
});

describe("search results", () => {
  it("does not list the same file twice when a peer answers twice", () => {
    const app = new AppState();
    app.startSearch(1, "aphex twin");
    app.apply({ type: "searchHits", data: { id: 1, hits: [hit("peer", "x.flac")] } });
    app.apply({ type: "searchHits", data: { id: 1, hits: [hit("peer", "x.flac")] } });

    expect(app.search?.rows).toHaveLength(1);
    expect(app.search?.peers, "nor count that peer twice").toBe(1);
  });

  it("keeps the same name from different peers", () => {
    // Two people having the same file is the normal case, and the whole point
    // of seeing more than one result.
    const app = new AppState();
    app.startSearch(1, "aphex twin");
    app.apply({
      type: "searchHits",
      data: { id: 1, hits: [hit("one", "x.flac"), hit("two", "x.flac")] },
    });
    expect(app.search?.rows).toHaveLength(2);
    expect(app.search?.peers).toBe(2);
  });
});

describe("closing a search", () => {
  it("frees it and moves to another", () => {
    // Searches were only ever added: every one ever run stayed in memory with
    // all its rows, and there was no way to put one down.
    const app = new AppState();
    app.startSearch(1, "first");
    app.startSearch(2, "second");
    app.apply({ type: "searchHits", data: { id: 2, hits: [hit("peer", "x.flac")] } });
    expect(app.activeSearch).toBe(2);

    app.closeSearch(2);

    expect(app.searches.map((s) => s.id)).toEqual([1]);
    expect(app.activeSearch, "the newest of what is left").toBe(1);
  });

  it("leaves nothing selected once they are all gone", () => {
    const app = new AppState();
    app.startSearch(1, "only");
    app.closeSearch(1);
    expect(app.searches).toHaveLength(0);
    expect(app.activeSearch).toBeNull();
  });
});

describe("standing wishes", () => {
  it("stops collecting hits without limit", () => {
    // A wish is re-run for as long as it is kept, so an untended one grew
    // forever at whatever interval the server allows.
    const app = new AppState();
    for (let batch = 0; batch < 60; batch++) {
      const files = Array.from({ length: 20 }, (_, index) => ({
        path: `b${batch}-f${index}.flac`,
        size: 1,
        bitrate: null,
        duration: null,
        vbr: false,
      }));
      app.apply({
        type: "wishlistHits",
        data: { query: "aphex", hits: [{ username: "peer", freeSlots: 1, speed: 1, files }] },
      });
    }
    const kept = app.wishHits["aphex"] ?? [];
    expect(kept.length).toBeLessThanOrEqual(500);
    expect(kept[0]?.path, "newest first").toBe("b59-f0.flac");
  });
});

describe("everyone seen", () => {
  it("gathers people from every place one can appear", () => {
    const app = new AppState();
    app.username = "me";
    app.apply({ type: "roomJoined", data: { room: "nicotine", users: ["in_room", "me"] } });
    app.startSearch(1, "aphex twin");
    app.apply({ type: "searchHits", data: { id: 1, hits: [hit("answered", "x.flac")] } });
    app.apply({ type: "privateMessage", data: { author: "messaged", body: "hi" } });
    app.apply({ type: "uploadUpdated", data: upload("uploading_to", "y.flac") });

    expect(app.knownUsers).toEqual(["answered", "in_room", "messaged", "uploading_to"]);
  });

  it("leaves you out of it", () => {
    const app = new AppState();
    app.username = "me";
    app.apply({ type: "roomJoined", data: { room: "nicotine", users: ["me"] } });
    expect(app.knownUsers).toEqual([]);
  });
});

describe("sessions ending", () => {
  it("tells being displaced apart from a bad password", () => {
    // One is not a failure to fix — the credentials were right — and the way
    // back is a button rather than a form.
    const app = new AppState();
    app.apply({ type: "disconnected", data: { type: "loggedInElsewhere" } });
    expect(app.displaced).toBe(true);
    expect(app.loginError).toBeNull();

    app.apply({ type: "loginFailed", data: { reason: "The server rejected that." } });
    expect(app.displaced).toBe(false);
    expect(app.loginError).toBe("The server rejected that.");
  });

  it("clears every in-flight flag when a session begins", () => {
    const app = new AppState();
    app.connecting = true;
    app.resuming = true;
    app.reconnecting = true;
    app.displaced = true;
    app.addingAccount = true;
    app.previousAccount = "someone";
    app.apply({ type: "connected", data: { username: "listener" } });
    expect([
      app.connecting,
      app.resuming,
      app.reconnecting,
      app.displaced,
      app.addingAccount,
    ]).toEqual([false, false, false, false, false]);
    expect(app.previousAccount).toBeNull();
  });

  it("keeps the wait on screen through the disconnect a reconnect causes", () => {
    // Applying a proxy rebuilds the session, so the old one drops partway
    // through. Letting that clear the wait put the sign-in form on screen for
    // the length of a handshake, which is the flicker this exists to stop.
    const app = new AppState();
    app.resuming = true;
    app.reconnecting = true;
    app.apply({ type: "disconnected", data: { type: "requested" } });
    expect(app.resuming).toBe(true);

    // A disconnect that no reconnect asked for still ends the wait.
    app.reconnecting = false;
    app.apply({ type: "disconnected", data: { type: "requested" } });
    expect(app.resuming).toBe(false);
  });
});

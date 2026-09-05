import { describe, expect, it } from "vitest";
import { Player, trackOf } from "./player.svelte";

/** Three tracks the way a folder hands them over. */
const one = (name: string) => trackOf(`/music/album/${name}`, `asset://${name}`, "downloads");
const first = one("a.flac");
const second = one("b.flac");
const last = one("c.flac");
const folder = [first, second, last];

describe("looping", () => {
  it("cycles off, track, folder, off", () => {
    const player = new Player();
    expect(player.loop).toBe("off");
    player.cycleLoop();
    expect(player.loop).toBe("track");
    player.cycleLoop();
    expect(player.loop).toBe("folder");
    player.cycleLoop();
    expect(player.loop).toBe("off");
  });

  it("stops at the end of a track when nothing is looping", () => {
    const player = new Player();
    player.track = first;
    player.playing = true;
    player.ended();
    expect(player.playing).toBe(false);
    expect(player.track).toBe(first);
  });

  it("moves to the next file in the folder, and round again from the last", () => {
    const player = new Player();
    player.loop = "folder";
    player.queue(folder);

    player.track = first;
    expect(player.nextInFolder()?.path).toBe(second.path);

    player.track = last;
    expect(player.nextInFolder()?.path).toBe(first.path);
  });

  it("has nowhere to go when only the track is looping", () => {
    const player = new Player();
    player.loop = "track";
    player.queue(folder);
    player.track = first;
    // The element repeats on its own, so `ended` never fires for this.
    expect(player.nextInFolder()).toBeNull();
  });

  it("sends a folder of one round again rather than off the end", () => {
    const player = new Player();
    player.loop = "folder";
    player.queue([first]);
    player.track = first;
    expect(player.nextInFolder()?.path).toBe(first.path);
  });
});

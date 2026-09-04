import { describe, expect, it } from "vitest";
import {
  bitrate,
  bytes,
  duration,
  eta,
  extension,
  fileName,
  format,
  parentPath,
  rate,
  tailPath,
} from "./format";

describe("bytes", () => {
  it("scales and keeps columns narrow", () => {
    expect(bytes(0)).toBe("0 B");
    expect(bytes(999)).toBe("999 B");
    expect(bytes(1024)).toBe("1.0 KB");
    expect(bytes(30_000_000)).toBe("29 MB");
  });

  it("refuses to render a size it was never given", () => {
    // Peers send whatever they like, and a search row builds a size out of it.
    expect(bytes(Number.NaN)).toBe("0 B");
    expect(bytes(-1)).toBe("0 B");
    expect(bytes(Number.POSITIVE_INFINITY)).toBe("0 B");
  });
});

describe("duration", () => {
  it("reads as a running time", () => {
    expect(duration(59)).toBe("0:59");
    expect(duration(60)).toBe("1:00");
    expect(duration(3661)).toBe("1:01:01");
  });

  it("says nothing rather than zero when it does not know", () => {
    expect(duration(null)).toBe("—");
    expect(duration(0)).toBe("—");
  });
});

describe("format", () => {
  it("reads the extension when there is one", () => {
    expect(format("music/01 Xtal.flac")).toBe("flac");
    expect(format("@@a1b2\\Video\\Show.MKV")).toBe("mkv");
  });

  it("does not mistake part of a name for a file type", () => {
    // The defect this exists for: peers name files `Show - 01v2.5` and
    // `Album Vol.2`, and taking whatever follows the last dot filled the
    // format menu with episode numbers and dates.
    expect(format("Show - 01v2.5")).toBe("");
    expect(format("Boards of Canada - Album Vol.2")).toBe("");
    expect(format("Live 1993.09.12")).toBe("");
    expect(format("no extension at all")).toBe("");
  });

  it("is not the same question as `extension`", () => {
    // The raw tail is still the honest answer to "what follows the last dot".
    expect(extension("Show - 01v2.5")).toBe("5");
    expect(format("Show - 01v2.5")).toBe("");
  });
});

describe("paths", () => {
  it("handles both separators, because peers use both", () => {
    expect(fileName("@@a1b2\\Music\\01 Xtal.flac")).toBe("01 Xtal.flac");
    expect(fileName("music/01 Xtal.flac")).toBe("01 Xtal.flac");
    expect(fileName("bare")).toBe("bare");
    expect(parentPath("@@a1b2\\Music\\01 Xtal.flac")).toBe("@@a1b2\\Music");
    expect(parentPath("bare")).toBe("");
  });

  it("trims by segment, never by character", () => {
    // Trimming with CSS reordered leading punctuation, so a real Soulseek
    // path beginning `@@` rendered with the `@@` at the end.
    expect(tailPath("@@a1b2\\Music\\FLAC\\Artist\\Album")).toBe("Artist · Album");
    expect(tailPath("@@a1b2\\Music", 3)).toBe("@@a1b2 · Music");
    expect(tailPath("")).toBe("");
  });
});

describe("rate and bitrate", () => {
  it("says nothing rather than zero when the peer did not", () => {
    expect(rate(0)).toBe("—");
    expect(bitrate(null)).toBe("—");
    expect(bitrate(0)).toBe("—");
    expect(bitrate(320)).toBe("320 kbps");
  });
});

describe("eta", () => {
  it("stays quiet until it can be honest", () => {
    expect(eta(0, 100, 0)).toBeNull();
    expect(eta(100, 100, 10)).toBeNull();
    expect(eta(0, 100, 10)).toBe("0:10");
  });
});

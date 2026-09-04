/** Display helpers. Every one of these is pure, so they are easy to trust. */

const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** A byte count at a readable scale — `4.7 MB`, `812 KB`. */
export function bytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const scale = Math.min(Math.floor(Math.log(value) / Math.log(1024)), UNITS.length - 1);
  const scaled = value / 1024 ** scale;
  // One decimal below 10 keeps columns narrow without losing useful precision.
  const digits = scale === 0 ? 0 : scaled < 10 ? 1 : 0;
  return `${scaled.toFixed(digits)} ${UNITS[scale]}`;
}

/** A transfer rate. */
export function rate(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return "—";
  return `${bytes(bytesPerSec)}/s`;
}

/** Whole seconds as `m:ss`, or `h:mm:ss` past an hour. */
export function duration(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds <= 0) return "—";
  const total = Math.round(seconds);
  const s = total % 60;
  const m = Math.floor(total / 60) % 60;
  const h = Math.floor(total / 3600);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

/** `320 kbps`, or an em dash when the peer did not say. */
export function bitrate(kbps: number | null): string {
  return kbps === null || kbps <= 0 ? "—" : `${kbps} kbps`;
}

/** The last component of a peer's path, which uses either separator. */
export function fileName(path: string): string {
  const cut = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return cut === -1 ? path : path.slice(cut + 1);
}

/** The directory portion of a peer's path. */
export function parentPath(path: string): string {
  const cut = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return cut === -1 ? "" : path.slice(0, cut);
}

/**
 * The last `segments` components of a path, joined with a middle dot.
 *
 * Peers share deeply nested paths (`@@a1b2\Music\FLAC\Artist\Album`) whose
 * useful part is at the end. Trimming here rather than with CSS is deliberate:
 * the `direction: rtl` trick that puts an ellipsis at the start also reorders
 * leading punctuation, so a real Soulseek path beginning `@@` renders with the
 * `@@` moved to the end — visibly wrong, and wrong in a way that looks like
 * corrupted data.
 */
export function tailPath(path: string, segments = 2): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.slice(-segments).join(" · ");
}

/** Lowercased text after the last dot, or `""`. Not necessarily a format. */
export function extension(path: string): string {
  const name = fileName(path);
  const cut = name.lastIndexOf(".");
  return cut === -1 ? "" : name.slice(cut + 1).toLowerCase();
}

/**
 * Formats worth offering as a filter.
 *
 * A closed set, because the open one is wrong. Peers name files anything:
 * `Show - 01v2.5`, `Album Vol.2`, `Live 1993.09.12` all end in a dot and
 * something, and taking whatever follows the last dot filled the format menu
 * with episode numbers and dates. Nothing here is guessed — a name whose tail
 * is not one of these has no format, and says so.
 */
const FORMATS = new Set([
  // lossless
  "flac", "wav", "aiff", "aif", "alac", "ape", "wv", "dsf", "dff", "tak", "tta",
  // lossy
  "mp3", "m4a", "aac", "ogg", "oga", "opus", "wma", "mpc", "ac3", "dts",
  // video
  "mkv", "mp4", "m4v", "avi", "mov", "webm", "wmv", "flv", "mpg", "mpeg", "ts", "vob",
  // things that travel with a rip
  "cue", "log", "txt", "nfo", "sfv", "m3u", "m3u8", "pdf",
  "jpg", "jpeg", "png", "gif", "webp",
  "zip", "rar", "7z", "iso",
]);

/**
 * The file's format, or `""` when its name does not end in one.
 *
 * Deliberately separate from [`extension`]: the raw tail is still the honest
 * answer to "what follows the last dot", and this is the answer to "what kind
 * of file is this", which is a different question with a different failure
 * mode.
 */
export function format(path: string): string {
  const tail = extension(path);
  return FORMATS.has(tail) ? tail : "";
}

/**
 * How long a transfer should still take, given progress and current rate.
 * Returns `null` while there is not enough information to be honest about it.
 */
export function eta(
  transferred: number,
  total: number,
  bytesPerSec: number,
): string | null {
  if (bytesPerSec <= 0 || total <= transferred) return null;
  return duration((total - transferred) / bytesPerSec);
}

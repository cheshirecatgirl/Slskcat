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

/** Lowercased extension without the dot, or `""`. */
export function extension(path: string): string {
  const name = fileName(path);
  const cut = name.lastIndexOf(".");
  return cut === -1 ? "" : name.slice(cut + 1).toLowerCase();
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

# Soulseek Desktop Client — Stack Research

Research date: 2026-07-28. Goal: a lightweight, fully-featured, modern-looking
Soulseek client as a **desktop app** (Windows/macOS/Linux).

The stack decision splits into two mostly-independent questions:

1. **Protocol core** — what speaks Soulseek. This is the hard, risky part.
2. **App shell / UI** — what draws the window. This is where "beautiful and
   light" is won or lost.

---

## 1. Protocol core

Soulseek is a closed, reverse-engineered protocol. Nobody has an official SDK.
The [Nicotine+ protocol documentation](https://nicotine-plus.org/doc/SLSKPROTOCOL.html)
is the de-facto spec, and it explicitly recommends reusing an existing
implementation rather than writing one — there are many subtle details required
for compatibility with other clients.

Writing our own from scratch is therefore rejected up front: it is months of
reverse-engineering work before a single file transfers.

### Candidates

| Library | Lang | Version | Last activity | Maturity signal | License |
|---|---|---|---|---|---|
| [Soulseek.NET](https://github.com/jpdillingham/Soulseek.NET) | C# | 10.0.2 | 2026-06-10 | ~6,257 commits; powers slskd | MIT |
| [soulseek-rs-lib](https://crates.io/crates/soulseek-rs-lib) | Rust | 12.0.0 | 2026-07-26 | 13k LOC, 131 tests, 719 downloads | MIT |
| [aioslsk](https://pypi.org/project/aioslsk/) | Python | 1.6.3 | 2026-01-18 | "Development Status :: 4 - Beta", good docs | GPL-3.0 |
| [soul](https://github.com/bh90210/soul) | Go | — | — | full message coverage + client pkg | — |
| [goose](https://github.com/a-cordier/goose) | Go | — | — | smaller/less complete | — |
| JS/TS | — | — | — | **no mature implementation exists** | — |

The absence of a credible JavaScript implementation is the single most
important finding: it removes "just build it in Electron with a Node backend"
from the table entirely.

### Detail: soulseek-rs-lib

Inspected the source directly (cloned `michel/soulseek-rs@master`).

- **MIT**, ~13,000 LOC across the library crate, 131 unit tests.
- **Zero dependencies** — `[dependencies]` is empty. Everything is std-only.
- **Synchronous/threaded**, not async: 12 `thread::spawn`, 0 `async fn`.
  Architecture is an actor model (`server_actor`, `peer_actor`,
  `peer_registry`, `dispatcher`).
- Clean facade: `Client` with `connect`, `login`, `search`,
  `search_with_cancel`, `get_search_results`, `download`,
  `download_with_metadata`, `pause_download`, `resume_download`,
  `browse_user`, `join_room`, `leave_room`, `say_in_room`,
  `send_private_message`, `request_user_info`, `set_shared_directories`,
  `set_upload_slots`, `enqueue_upload`, `place_in_queue`, `check_privileges`.
- Feature coverage per its README: search + download, wishlist, sharing,
  upload queue with privileged-user ordering, browse, **resumable downloads**
  (`.part` + offset request), chat rooms, private messages, **firewalled-peer
  fallback** via server-brokered indirect connections, and **UPnP-IGD /
  NAT-PMP** port mapping.

That last cluster matters: indirect connections and port mapping are exactly
the fiddly things a naive implementation skips, and without them a large
fraction of transfers silently fail.

**Risks.** It is a solo-maintainer project self-described as "a learning
exercise in Rust". It went 1.x → 12.0.0 in eight months, so the API breaks
often. Download counts are low, meaning little production hardening.

**Mitigation.** It is MIT and only 13k LOC — small enough to vendor and fork if
upstream stalls. We pin an exact version and put our own trait-shaped `core`
layer in front of it so the rest of the app never imports it directly. If it
has to be swapped for a hand-written implementation or a sidecar later, that
change stays behind one boundary.

**On sync-vs-async:** not a real problem here. The core runs on its own
threads, off the UI thread, and Soulseek peer counts are modest. It does mean
the core cannot share a tokio runtime with anything else.

### Detail: Soulseek.NET

The most battle-tested option by a wide margin — it is the engine inside
[slskd](https://github.com/slskd/slskd), the well-known headless
Soulseek server, and is still actively released (10.0.2 in June 2026). If
protocol reliability were the only axis, this wins.

The cost is that consuming it means shipping the .NET runtime (~70–100 MB
self-contained) and, in a Tauri/web-UI design, running it as a **second
process** with IPC. That directly contradicts the "lightweight" requirement.

---

## 2. App shell

| Option | Bundle | Idle RAM | UI ceiling | Verdict |
|---|---|---|---|---|
| **Tauri v2** (2.11.5) | 5–10 MB | ~45–100 MB | full modern CSS | **best fit** |
| Electron | 80–200 MB | ~120–180 MB | full modern CSS | fails "lightweight" |
| Avalonia (.NET) | ~70–100 MB | moderate | good, more effort | viable, heavier |
| Qt / GTK | small | low | dated unless heavily themed | poor UX-per-effort |
| Flutter | ~40 MB | moderate | excellent | no protocol library |

Tauri uses the OS's native WebView instead of bundling Chromium. Reference
point: Hoppscotch's Electron→Tauri migration reported 165 MB → 8 MB bundle and
~70% lower memory.

The trade-off Tauri asks for is a Rust backend — which, given that the best
footprint-compatible protocol library is *already Rust*, is a benefit here
rather than a cost. Webview inconsistency across platforms is the genuine
downside; it is manageable by targeting a conservative CSS baseline and
testing on all three platforms.

---

## Recommendation

**Tauri v2 + Rust core built on a pinned, wrapped `soulseek-rs-lib` + a web
frontend (Svelte 5 or React, with Tailwind v4).**

This is the only combination that satisfies both halves of the brief:

- The protocol library and the app shell are the **same language**, so there is
  one process, no sidecar, no cross-runtime IPC, and no second runtime to ship.
- Bundle lands in single-digit MB with idle RAM well under 100 MB.
- The UI is ordinary modern web tech, so "beautiful" is achievable at
  reasonable effort — which is not true of Qt/GTK, and is more work in Avalonia.

Architecture: the Soulseek client runs on its own thread(s) inside the Tauri
process; the frontend drives it through Tauri commands and receives streaming
updates (search hits, transfer progress) through Tauri's event channel.
Incremental search results come from kicking off `search_with_cancel` on a
worker and polling `get_search_results`, emitting deltas to the UI.

### The main fallback

If `soulseek-rs-lib` proves too unstable in practice, the escape hatch is a
**slskd sidecar**: keep the entire Tauri frontend, and re-point the `core`
boundary at slskd's HTTP API instead of in-process Rust. That buys the most
mature protocol stack in exchange for the .NET runtime and a second process.
Keeping the `core` layer trait-shaped from day one is what makes this a
contained change rather than a rewrite.

---

## Build environment note

This container has Rust 1.94.1, Node 22.22.2, pnpm 10.33, and bun 1.3.11.
It does **not** have `webkit2gtk-4.1`, so the Tauri GUI cannot be launched
here — the Rust core and the frontend can be compiled and tested, but visual
verification has to happen on a real desktop.

---

## Sources

- [Soulseek protocol documentation (Nicotine+)](https://nicotine-plus.org/doc/SLSKPROTOCOL.html)
- [Soulseek.NET](https://github.com/jpdillingham/Soulseek.NET) · [NuGet](https://www.nuget.org/packages/Soulseek)
- [soulseek-rs / soulseek-rs-lib](https://github.com/michel/soulseek-rs)
- [aioslsk](https://pypi.org/project/aioslsk/)
- [soul (Go)](https://github.com/bh90210/soul) · [goose (Go)](https://github.com/a-cordier/goose)
- [slskd](https://github.com/slskd/slskd)
- [Tauri vs Electron 2026 benchmarks (PkgPulse)](https://www.pkgpulse.com/blog/best-desktop-app-frameworks-2026)
- [Desktop stacks 2026 comparison (Digital Applied)](https://www.digitalapplied.com/blog/desktop-apps-web-stack-tauri-electron-deno-wails-2026)

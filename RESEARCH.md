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

**Constraint: native software, not a web app.** This rules out Electron
outright. It was initially read as also ruling out Tauri, on the grounds that
Tauri renders its UI in an OS WebView — but that reading was too strict and has
been revised; see [§3](#3-revisiting-tauri) for the reasoning and the final
decision. A Tauri app is an installed native binary with no browser, no server
and no localhost URL, which is what "software, not web based" was actually
asking for.

The toolkits below are still the right field to compare. Since the viable
protocol library is Rust (see §1), staying in Rust keeps the core in-process
with zero IPC either way.

| Option | Version | API stability | Look/feel ceiling | License | Verdict |
|---|---|---|---|---|---|
| **Iced** | 0.14.0 | 0.x, breaking | fully custom, proven | MIT | **recommended** |
| **Slint** | 1.17.1 | **stable 1.x** | excellent, DSL + live preview | GPL / royalty-free | strong runner-up |
| egui | 0.35.0 | 0.x | utilitarian, immediate-mode | MIT/Apache | fails "beautiful" |
| Freya | 0.4.0 | early | good (Skia, CSS-like) | MIT | too young |
| Xilem | 0.4.0 | experimental | unproven | Apache | not production-ready |
| Avalonia (.NET) | — | stable | good, more effort | MIT | needs .NET runtime + IPC to Rust |
| Qt / GTK | — | stable | dated unless heavily themed | LGPL/GPL | poor UX-per-effort, C++/C binding tax |
| **Tauri v2** | 2.11.5 | stable | full CSS | MIT | **chosen — see §3** |
| ~~Electron~~ | — | stable | full CSS | MIT | **rejected: 80–200 MB, bundles Chromium** |

### Iced vs Slint

These are the only two serious contenders.

**Iced 0.14** (Dec 2025) is pure Rust with an Elm-style architecture. Its
strongest credential is that **System76's COSMIC desktop is built on it** —
COSMIC shipped Epoch 1.0 and is now at 1.3, meaning an entire production
desktop environment (file manager, settings, terminal, app store) runs on this
toolkit. That is the most convincing possible answer to "can it handle a
complex, data-dense application". 0.14 added reactive rendering, hot reloading,
headless testing, and smarter scrollbars.

**Slint 1.17** has the better *authoring* story: a declarative UI DSL with an
LSP and live preview, so iterating on visual design is much faster, and it
carries a stable 1.x API guarantee where Iced is still 0.x and breaks between
releases. Against it: the DSL is deliberately limited for complex logic, its
`ListView` virtualization requires uniform row heights, and its centre of
gravity is embedded/automotive rather than data-dense desktop tools.

**Choosing Iced.** Two reasons dominate:

1. **The Elm architecture is an unusually good fit for this specific app.** A
   Soulseek client is almost entirely event-driven — search hits, transfer
   progress, room chatter and peer state all arrive asynchronously from the
   network. Iced's `Message` enum + `update()` loop is exactly that shape, and
   `Subscription` is the designed-in way to bridge a background thread's
   channel into the UI. The protocol core's threads feed messages in; the UI is
   a pure function of state.
2. **COSMIC proves the ceiling.** Complexity and polish at full-desktop scale
   are demonstrated, not hypothetical.

The accepted cost is Iced's 0.x churn — pinned exact versions and an upgrade
being a deliberate, scheduled task rather than a surprise.

---

---

## 3. Revisiting Tauri

The first pass rejected Tauri for rendering its UI in a WebView. On review that
rejection did not hold up, for two reasons.

**Tauri is native software by any practical definition.** It ships as an
installed binary — `.exe`, `.app`, `.deb`. There is no browser to open, no
server to run, no URL to visit. The web technology is a rendering detail
*inside* the window, not a deployment model. The thing "not web based" was
meant to exclude is an app like slskd, which you reach at `localhost:5030` in
Chrome. Tauri is not that.

**The footprint argument was wrong.** The first pass implied "lightweight"
favoured Iced. It does not. Tauri installs at roughly 600 KB–10 MB with
20–100 MB idle RAM, which is the same envelope as an Iced binary. Footprint
does not meaningfully separate them and should not have been used to decide.

What Tauri genuinely wins on is the thing that was asked for first: the
interface. Real CSS, real animation, grid and flexbox, instant hot-reload
iteration, and an ecosystem an order of magnitude larger than Iced's
(23.4M crate downloads against 2.4M). Polished shipping examples — Jan, Cap,
Spacedrive, Hoppscotch — show the ceiling is high.

### The real trade-off: Linux

Tauri uses whatever WebView the OS provides:

| Platform | Engine | Assessment |
|---|---|---|
| Windows | WebView2 (Chromium) | solid |
| macOS | WKWebView | solid |
| Linux | WebKitGTK | **the weak link** |

WebKitGTK is where the reported problems are, and they are precisely visual
ones: [CSS animations blurring the rest of the app and `contenteditable`
quirks](https://github.com/tauri-apps/tauri/discussions/9088), NVIDIA DMABUF
renderer failures, and [maintainers and users reporting it degrading with
successive releases](https://github.com/orgs/tauri-apps/discussions/8524).
Tauri ships a dedicated [Linux Graphics Issues](https://v2.tauri.app/develop/debug/linux-graphics/)
page, which is itself a signal about frequency.

Iced avoids this entirely by rendering everything itself, so it is
pixel-identical on all three platforms — at the cost of more effort to reach
the same visual polish, and a 0.x API that breaks between releases.

## Decision

**Tauri v2 + the existing Rust core, with a Svelte frontend.**

With all three platforms weighted equally, Tauri gives a better interface on
two of them and an occasionally-imperfect one on the third, against Iced's
consistent-but-harder-won interface on all three. Since interface quality is
the primary requirement, that trade favours Tauri. The accepted cost is
explicit: **Linux users may see occasional rendering artefacts**, and Linux
needs testing on real hardware before release.

Architecture is unchanged by this decision, which is the point of having built
the core behind a trait:

- `lark-core` stays exactly as it is — a plain Rust crate, `Command` in and
  `Event` out.
- Tauri's backend *is* Rust, so the core is used in-process with no sidecar and
  no serialisation boundary to the protocol library.
- UI commands map to `#[tauri::command]`; the core's event stream is forwarded
  to the WebView over Tauri's event channel.

### Fallbacks

- **If WebKitGTK proves unacceptable on Linux:** the view layer is replaceable
  with Iced 0.14 without touching `lark-core`. That is why the core was built
  first and kept free of UI concerns.
- **If `soulseek-rs-lib` proves too unstable:** re-point the `Backend` seam at
  an **slskd sidecar** over its HTTP API — the most battle-tested protocol
  stack — in exchange for the .NET runtime and a second process.

---

## Build environment note

This container has Rust 1.94.1, Node 22.22.2 and pnpm 10.33, and
`libwebkit2gtk-4.1-dev` 2.52.3 was installed so the Tauri backend compiles
here. It is headless with no display server, so the app cannot be *launched* —
everything compiles and unit-tests, but visual verification has to happen on a
real desktop.

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
- [Iced](https://github.com/iced-rs/iced) · [Iced 0.14 release coverage (Phoronix)](https://www.phoronix.com/news/Iced-0.14-Rust-GUI-LIbrary) · [Iced book/FAQ](https://book.iced.rs/faq.html)
- [Slint](https://github.com/slint-ui/slint) · [Slint vs Iced discussion](https://github.com/slint-ui/slint/discussions/2224)
- [COSMIC desktop (built on Iced)](https://en.wikipedia.org/wiki/COSMIC_desktop)
- [The Rust GUI Landscape in 2026](https://wrenlearnsrust.com/posts/2026-03-11-rust-gui-landscape-2026.html)
- [State of Rust GUI libraries (LogRocket)](https://blog.logrocket.com/state-rust-gui-libraries/)

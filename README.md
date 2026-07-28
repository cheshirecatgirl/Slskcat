# Lark

A lightweight Soulseek client for the desktop.

Native installed software — no browser, no server, no localhost URL. The
protocol core and the app backend are both Rust in a single process, with the
interface rendered by the OS WebView.

**4.5 MB binary, 104 KB of interface assets.**

> **Status:** early. The core and the app shell are built and compile clean;
> nothing has yet been tested against the live Soulseek network.
> See [RESEARCH.md](RESEARCH.md) for how the stack was chosen — including the
> reasoning that was revised along the way.

## Layout

```
crates/
  lark-core/     protocol core — Commands in, Events out
  lark-app/      Tauri backend — routes between the core and the WebView
ui/              Svelte 5 interface
```

### `lark-core`

- `model` — domain types (`SearchHit`, `Transfer`, `Room`, …). Mentions no
  protocol library.
- `command` / `event` — the two currencies the interface deals in.
- `backend` — the `Backend` trait, the seam the protocol library sits behind.
- `live` — the real backend, over [`soulseek-rs-lib`]. The only module that
  names that library.
- `engine` — owns the worker thread and the command/event channels.

### `lark-app`

Deliberately thin. Each user action is a `#[tauri::command]` forwarding a
`Command`; one thread owns the `Engine` and republishes every `Event` to the
WebView on a single channel. It holds no protocol knowledge of its own.

### `ui`

Svelte 5 with runes. `lib/core.ts` is the only file that imports Tauri;
`lib/state.svelte.ts` is the only place events are applied. Search results are
windowed, so a query returning tens of thousands of files renders a few dozen
rows.

## Design

The interface never blocks on the network. It sends a command and reacts to
events:

```rust
use lark_core::{Command, Engine, LiveBackend, model::Config};

let engine = Engine::spawn(LiveBackend::new());
engine.send(Command::Connect(Box::new(Config::default())));

for event in engine.drain() {
    println!("{event:?}");
}
```

Commands are fire-and-forget — failures come back as `Event::Warning` or a
specific failure event, never as a return value. The protocol library is
synchronous, so searches and transfers run on their own threads and report
progress through `Backend::poll` on a fixed tick.

### Why the `Backend` seam exists

`soulseek-rs-lib` is capable and actively developed, but it is a young
solo-maintained project that has broken its API often. It is pinned to an exact
version and confined to one module, so replacing it — with a fork, or with an
out-of-process daemon — stays a contained change.

The same seam is why the shell decision (Tauri, previously Iced) cost nothing
to reverse: the core has no idea what draws the window.

## Building

Requires a stable Rust toolchain, Node 20+, and pnpm. On Linux you also need
the Tauri system dependencies (`libwebkit2gtk-4.1-dev`, `libxdo-dev`,
`libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`).

```bash
pnpm --dir ui install

cargo test                        # 27 unit tests, no network needed
cargo clippy --all-targets        # clean under pedantic lints
pnpm --dir ui build               # typecheck + bundle

cargo tauri dev                   # run the app
```

## Known limitations

- **Not yet tested against the live network.** Login, transfers and browsing
  are unverified end to end.
- **Linux rendering.** Tauri uses WebKitGTK there, which is the weakest of the
  three platform WebViews; occasional visual artefacts are possible. Windows
  and macOS use Chromium-based and WebKit engines respectively and are solid.

## Licence

MIT.

[`soulseek-rs-lib`]: https://github.com/michel/soulseek-rs

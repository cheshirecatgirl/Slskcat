# Lark

A lightweight Soulseek client for the desktop — native software, not a web app
in a window.

Written in Rust throughout: the protocol core and the user interface are the
same language in the same process, so there is no sidecar, no bundled browser
engine and no second runtime to install.

> **Status:** early. The core is built and tested; the interface is next.
> See [RESEARCH.md](RESEARCH.md) for how the stack was chosen.

## Layout

```
crates/
  lark-core/     protocol core — commands in, events out
```

- `model` — domain types (`SearchHit`, `Transfer`, `Room`, …). Mentions no
  protocol library.
- `command` / `event` — the two currencies the interface deals in.
- `backend` — the `Backend` trait, the seam the protocol library sits behind.
- `live` — the real backend, over [`soulseek-rs-lib`]. The only module that
  names that library.
- `engine` — owns the worker thread and the command/event channels.

## Design

The interface never blocks on the network. It pushes a `Command` and drains
`Event`s whenever it is ready to redraw:

```rust
use lark_core::{Command, Engine, LiveBackend, model::Config};

let engine = Engine::spawn(LiveBackend::new());
engine.send(Command::Connect(Box::new(Config::default())));

for event in engine.drain() {
    println!("{event:?}");
}
```

Everything the network does arrives as an `Event`, in order. Commands are
fire-and-forget — failures come back as `Event::Warning` or a specific failure
event rather than as a return value.

The protocol library is synchronous, so long-running work (a search's
collection window, a file transfer) runs on its own thread and reports
progress through `Backend::poll`, which the engine calls on a fixed tick.

### Why the `Backend` seam exists

`soulseek-rs-lib` is capable and actively developed, but it is a young
solo-maintained project that has broken its API often. It is pinned to an exact
version and confined to one module, so replacing it — with a fork, or with an
out-of-process daemon — stays a contained change. Nothing above `Backend`
would notice.

## Building

Requires a recent stable Rust toolchain.

```bash
cargo test      # 25 unit tests, no network access needed
cargo clippy --all-targets
```

## Licence

MIT.

[`soulseek-rs-lib`]: https://github.com/michel/soulseek-rs

# Working on slsk.cat

A Soulseek client. Tauri v2 shell, Rust core, Svelte 5 interface. See
[README.md](README.md) for architecture and [VERIFY.md](VERIFY.md) for the
manual checklist.

## Why this file exists

Most of this project was built in a sandbox with **no display server and no
route to the Soulseek ports** (2416/2271 time out there). So a specific set of
things is built and unit-tested but has never run for real. If you are reading
this on a machine with a screen and a normal internet connection, **you can
close most of that gap** — that is the job.

## What is already verified — do not redo

- 93 Rust tests and 30 frontend tests, `cargo clippy` under
  pedantic-denied, `cargo fmt`, frontend
  typecheck. All green.
- The credential-store round trip, against a real `gnome-keyring`.
- The share guard, including a symlinked root resolving into a refused location.
- `cargo tauri build --bundles deb` — a 2.0 MB package installing
  `/usr/bin/slskcat`. Measured before `custom-protocol` was declared, so that
  bundle carried no frontend; the command works, the artefact needs remeasuring.
- Every screen, both colour schemes, in **headless Chromium** via
  `ui/tools/screenshots.mjs`.
- The shifter's output and the CORS trap that silences it, via
  `ui/tools/audio-check.mjs`. Chromium, not WebKitGTK.
- `cargo audit` and `pnpm audit` — zero vulnerabilities.

## What has never run

1. **Anything against the live network.** `crates/slskcat-core/examples/smoke.rs`
   is the instrument. It needs `SLSKCAT_USER` and `SLSKCAT_PASS` in the
   environment.
2. **The real Linux WebView.** Screenshots were Chromium; Linux ships
   WebKitGTK, which is the weak one. `cargo tauri dev` is the only way to see
   it.
3. **Bundles other than `.deb`.** `rpm`, `AppImage`, `msi`, NSIS, `dmg`.
4. **The release workflow.** `.github/workflows/release.yml` has never fired.

## Credentials

Never write a password into a file, a test, a commit, or a shell history you
then commit. Read it from the environment only. If you need one and it is not
set, **ask** — do not invent one, and do not register a new Soulseek account
(the server silently creates one from any unknown username, which is a real
footgun, not a convenience).

## Conventions this codebase holds to

- `unsafe_code = "forbid"` at the workspace level. If a test seems to need
  `unsafe` — mutating `std::env`, say — restructure so it does not. There is
  already a precedent: `guard.rs` takes the home directory as an argument
  instead of reading the environment, which made it deterministic too.
- Clippy runs with `all = deny` and `pedantic = warn`, and CI adds
  `-D warnings`. Fix findings; do not blanket-allow. Where an allow is genuinely
  right, scope it narrowly and say why in a comment.
- `cargo fmt --all --check` is a CI gate. Run `cargo fmt --all` before
  committing.
- Comments explain *why*, and are worth writing where a reader would otherwise
  wonder. Do not narrate what the code already says.
- The core knows nothing about the UI, and only `live.rs` names the protocol
  library. Keep it that way — it is why swapping the UI toolkit once cost
  nothing.
- Only `ui/src/lib/core.ts` imports Tauri; only `ui/src/lib/state.svelte.ts`
  applies events.

## Verifying visually

`ui/tools/screenshots.mjs` renders the built interface in headless Chromium
against a mocked Tauri bridge. It is fast, it needs no display, and it has
caught real defects that reading the code did not — a `direction: rtl`
truncation that reordered `@@`-prefixed paths, and table headers centred by
Chromium's internal button wrapper. **Use it, but remember it is Chromium.** It
says nothing about WebKitGTK.

```bash
pnpm --dir ui build
python3 -m http.server 8899 -d ui/dist &
node ui/tools/screenshots.mjs docs
```

Playwright is a dev dependency, so `pnpm --dir ui install` is the whole setup.
Where the browser binaries are already on the machine, point at one with
`CHROMIUM_PATH=/path/to/chrome` rather than downloading another.

## Verifying audio

`ui/tools/audio-check.mjs` renders tones through the shifter and through a
media element in headless Chromium, and measures what comes out. It starts its
own servers and needs no arguments:

```bash
node ui/tools/audio-check.mjs
```

`ui/tools/speed-probe.mjs` is the other half: it measures what time-stretching
costs, which is how the "keep pitch" control got its wording.

Silence is the failure that looks like success here — the track plays, the
clock runs, nothing is heard — so it is worth running after any change to
`player.svelte.ts`, `pitch-worklet.js`, or how files reach the element.

## Things that look like bugs and are not

- **Queue positions on downloads.** Normal Soulseek behaviour.
- **A peer never answering a browse request.** Also normal; some clients ignore
  it entirely.
- **Visual glitches on Linux specifically.** WebKitGTK. Worth noting, not worth
  chasing in our code first.

## Where a human is still better

Judging whether the interface *looks right*. Screenshots can be taken and
inspected, and should be — but "does this feel good to use" is not something to
sign off on someone's behalf. Report what you see and let them decide.

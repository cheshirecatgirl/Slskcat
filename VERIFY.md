# Verify, then publish

Everything that could be checked without real hardware already has been. This
is only the part that needs your machine.

**Total: about 20 minutes.** Steps 1–5 in order. Step 6 publishes.

> **Want Claude to drive most of this?** Install the Claude Code CLI, clone
> this repo, and run `claude` inside it. [CLAUDE.md](CLAUDE.md) tells a fresh
> session what is already verified and what has never run, so it can pick up
> from here. It can automate steps 1, 2, 4, 5 and 6; step 3 still wants your
> eyes.

---

## Setup (2 min)

```bash
git clone <this repo> && cd slsk.cat
pnpm --dir ui install
```

Linux also needs:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

---

## 1. Does it actually talk to Soulseek? (2 min)

**This is the important one.** Nothing in the repo has ever touched the network.

```bash
SLSKCAT_USER=yourname SLSKCAT_PASS=yourpassword \
  cargo run -p slskcat-core --example smoke
```

**Good:** every line says `ok`, ends with `all 5 steps passed`.

```
  ok  sign in        as yourname  (1.2s)
  ok  room list      412 rooms  (0.4s)
  ok  search         137 files from 24 peers  (6.1s)
  ok  browse a peer  86 folders from velvet_hare  (2.0s)
  ok  peer details   Some(Online), 18422 files shared  (0.3s)
```

**If sign in fails:** wrong password, or the server is down. Soulseek registers
a new name on first use, so a typo silently creates a new account.

**If search finds nothing:** your query is too narrow. `SLSKCAT_QUERY="jazz"`.

**If it times out:** `SLSKCAT_TIMEOUT=60`.

☐ Done

---

## 2. Move real bytes (2 min)

Same thing, but it downloads the smallest file it found.

```bash
SLSKCAT_USER=yourname SLSKCAT_PASS=yourpassword SLSKCAT_DOWNLOAD=1 \
  cargo run -p slskcat-core --example smoke
```

**Good:** `ok  download  <n> bytes received`.

**If it fails:** most likely nobody had a free slot. Run it again, or pick a
query with more results. A queue position is normal on Soulseek, not a bug.

☐ Done

---

## 3. Look at it (5 min)

```bash
cargo tauri dev
```

The screenshots in the README were taken in headless Chromium. **Linux uses
WebKitGTK instead, which is the one known-weak renderer** — so this step is
mostly about whether it looks right on your machine.

Click through, in this order:

☐ Sign in — check **Remember password**
☐ Search something — results stream in, columns line up
☐ Press **⌘K** / **Ctrl-K** — palette opens, typing offers to search
☐ **Library → Wishlist** — add a wish, it appears; **Files** lists media only
☐ **Transfers** — both tabs render
☐ **Rooms** — join one, send a line
☐ **Settings** — every section renders

**Watch for:** blurry text after an animation, a flickering window, text that
disappears. Those are the WebKitGTK problems, not layout bugs — note them and
move on.

☐ Done

---

## 4. Is the password really in the keychain? (1 min)

The round trip is already tested against `gnome-keyring`. This checks it on
*your* keyring.

1. Quit the app.
2. Reopen it: `cargo tauri dev`

**Good:** username **and** password are already filled in.

**Then confirm it is not on disk:**

```bash
grep -ri password ~/.config/cat.slsk.client/settings.json
```

**Good:** no output. The file should hold your username and folders, nothing
else.

☐ Done

---

## 5. Try to leak something (2 min)

The guard is unit-tested, but see it refuse in person.

In **Settings → Shared folders → Add folder…**:

☐ Pick your **home folder** → refused, red toast
☐ Pick **`~/Documents`** → amber strip, *Share anyway* / *Skip*
☐ Pick a **music folder** → added with no fuss

Show hidden folders in the picker (`Ctrl-H` on GTK) and pick `~/.ssh` →
refused.

☐ Done

---

## 6. Publish (5 min)

Only after 1–5 pass.

**One number to change.** `tauri.conf.json` reads the version from
`Cargo.toml`, so there is only one place:

```bash
# edit the workspace Cargo.toml: version = "0.1.0"  ->  "0.2.0"
cargo check                      # refreshes Cargo.lock
git commit -am "Release v0.2.0"
git tag v0.2.0
git push && git push --tags
```

The tag is what triggers it. Then:

1. Open the repo's **Actions** tab.
2. Watch **Release** — four jobs, one per platform.
3. When green, download the artifacts from the run.

**You get:** `.deb`, `.rpm`, `.AppImage`, `.msi`, NSIS `-setup.exe`, and a
`.dmg` for both Apple silicon and Intel.

**This workflow has never run.** If a job fails it will be in the bundling
step, not the compile — the Linux `.deb` is the only bundle built and verified
so far. Everything else compiles but has never been packaged.

☐ Published

---

## Already verified — don't redo these

- 90 Rust tests and 30 frontend tests, clippy under pedantic-denied, fmt,
  frontend typecheck
- The credential store round trip, against a real `gnome-keyring`
- The share guard, including a symlinked root pointing into a refused location
- `cargo tauri build --bundles deb` — a 2.0 MB package that installs
  `/usr/bin/slskcat`
- Every screen, in both colour schemes, in headless Chromium
- The pitch shifter's output, and that a media element needs `crossorigin` to
  be audible through a Web Audio graph (`node ui/tools/audio-check.mjs`)
- `cargo audit` and `pnpm audit` — zero vulnerabilities

## Known, expected, not worth reporting

- **Linux visual glitches.** WebKitGTK. Windows and macOS use better engines.
- **Queue positions on downloads.** Normal Soulseek behaviour.
- **A peer never answering a browse.** Also normal. Some clients ignore it.

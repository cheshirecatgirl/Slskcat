/**
 * Renders the built UI in Chromium against a mocked Tauri bridge, so the
 * interface can be looked at — and regressions caught — without a desktop
 * session. This is how the app is reviewed in headless environments.
 *
 * Usage, from `ui/`:
 *   pnpm build
 *   python3 -m http.server 8899 -d dist &
 *   node tools/screenshots.mjs ../docs
 *
 * Playwright is a dev dependency, so `pnpm install` is the whole setup. Set
 * CHROMIUM_PATH to use a browser that is already on the machine.
 */
import { chromium } from "playwright";

const URL = "http://127.0.0.1:8899/";
const OUT = process.argv[2] ?? ".";

// Stands in for `window.__TAURI_INTERNALS__`, which is what @tauri-apps/api
// talks to. Commands resolve with plausible values; events are pushed by
// calling the handler the app registered through `listen`.
const bridge = () => {
  const callbacks = new Map();
  let nextId = 1;
  let eventHandler = null;

  window.__TAURI_INTERNALS__ = {
    transformCallback(cb) {
      const id = nextId++;
      callbacks.set(id, cb);
      return id;
    },
    async invoke(cmd, args) {
      if (cmd === "plugin:event|listen") {
        eventHandler = callbacks.get(args.handler);
        return nextId++;
      }
      if (cmd === "plugin:event|unlisten") return null;
      if (cmd === "search") return 1;
      if (cmd === "load_settings") return { ...window.__settings };
      if (cmd === "local_library") return window.__library;
      if (cmd === "local_albums") return window.__albums;
      if (cmd === "downloaded_files") return [];
      if (cmd === "set_wishlist") return null;
      if (cmd === "assess_share") {
        // Mirrors the core's classification closely enough to drive the UI.
        if (/(^|\/)\.|^\/etc|^\/usr|^\/$/.test(args.path)) {
          return {
            allowed: false,
            sensitive: false,
            reason: "Hidden folders can hold keys and credentials.",
          };
        }
        if (/\/(Documents|Desktop|Pictures)$/.test(args.path)) {
          return {
            allowed: true,
            sensitive: true,
            reason: "This folder usually holds personal files, not music.",
          };
        }
        return { allowed: true, sensitive: false, reason: null };
      }
      if (cmd === "plugin:dialog|open") return window.__pick ?? null;
      if (cmd === "switch_account" || cmd === "remember_account") {
        window.__settings = { ...window.__settings, username: args.username };
        return { ...window.__settings };
      }
      if (cmd === "save_settings" || cmd === "connect") {
        window.__settings = { ...window.__settings, ...args.settings, keychainAvailable: true };
        return { ...window.__settings };
      }
      return null;
    },
  };

  // Stands in for what the settings file and credential store would hold.
  window.__settings = {
    username: "slsk_listener",
    password: "",
    rememberPassword: false,
    downloadDir: "/home/listener/Downloads",
    sharedDirs: ["/home/listener/Music/FLAC", "/home/listener/Music/Rips"],
    uploadSlots: 3,
    searchTimeoutSecs: 12,
    wishlist: ["boards of canada — geogaddi vinyl rip", "coil musick to play in the dark 2"],
    uiScale: 100,
    rooms: [],
    friends: [],
    proxy: null,
    accounts: ["slsk_listener", "night_shift", "tape_hiss"],
    keychainAvailable: true,
  };

  window.__library = [
    {
      path: "/home/listener/Downloads",
      downloads: true,
      files: [
        { path: "/home/listener/Downloads/01 Xtal.flac", folder: "Aphex Twin - SAW 85-92", name: "01 Xtal.flac", size: 30_000_000 },
        { path: "/home/listener/Downloads/02 Tha.flac", folder: "Aphex Twin - SAW 85-92", name: "02 Tha.flac", size: 41_000_000 },
        { path: "/home/listener/Downloads/folder.jpg", folder: "Aphex Twin - SAW 85-92", name: "folder.jpg", size: 240_000 },
      ],
    },
    {
      path: "/home/listener/Music/FLAC",
      downloads: false,
      files: [
        { path: "/home/listener/Music/FLAC/a.flac", folder: "Boards of Canada - Geogaddi", name: "01 Ready Lets Go.flac", size: 8_000_000 },
      ],
    },
  ];

  // Releases, with covers drawn rather than fetched: the real command reads
  // tags and writes artwork out, and neither exists behind a mocked bridge.
  const sleeve = (a, b, glyph) =>
    "data:image/svg+xml," +
    encodeURIComponent(
      `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 300">` +
        `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
        `<stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/>` +
        `</linearGradient></defs><rect width="300" height="300" fill="url(#g)"/>` +
        `<text x="150" y="196" font-size="132" text-anchor="middle" fill="#ffffffcc"` +
        ` font-family="sans-serif">${glyph}</text></svg>`,
    );

  const release = (key, title, artist, year, cover, names) => ({
    key,
    title,
    artist,
    year,
    cover,
    downloads: true,
    tracks: names.map((name, i) => ({
      path: `/home/listener/Downloads/${key}/${name}`,
      name,
      size: 30_000_000 + i * 2_000_000,
      title: name.replace(/^\d+ /, "").replace(/\.flac$/, ""),
      artist,
      number: i + 1,
      disc: null,
      seconds: 180 + i * 41,
    })),
  });

  window.__albums = [
    release("saw", "Selected Ambient Works 85-92", "Aphex Twin", 1992, sleeve("#5b4bd6", "#c05fa8", "△"), [
      "01 Xtal.flac", "02 Tha.flac", "03 Pulsewidth.flac", "04 Ageispolis.flac",
      "05 i.flac", "06 Green Calx.flac", "07 Heliosphan.flac",
    ]),
    release("geo", "Geogaddi", "Boards of Canada", 2002, sleeve("#a2582b", "#d8a24a", "◎"), [
      "01 Ready Lets Go.flac", "02 Music Is Math.flac", "03 Beware the Friendly Stranger.flac",
    ]),
    release("rdj", "Richard D James Album", "Aphex Twin", 1996, sleeve("#2f7d55", "#8fd18a", "✳"), [
      "01 4.flac", "02 Cornish Acid.flac", "03 Peek 824545201.flac",
    ]),
    release("mus", "Musick to Play in the Dark 2", "Coil", 2000, sleeve("#1f2b5c", "#5f7bd6", "☾"), [
      "01 Something.flac", "02 Tiny Golden Books.flac",
    ]),
    release("hex", "Hexadecimal Sunrise", "sparrowfall", null, null, [
      "01 Untitled.flac", "02 Untitled.flac",
    ]),
  ];

  // Push a core event into the app exactly as the Rust side would.
  // `convertFileSrc` reads this to build an asset URL. Outside Tauri it is
  // absent, and the library's play button would throw before the bar appeared.
  // Covers in this mock are drawn as data URIs rather than read off disk, and
  // wrapping one in an asset URL would only break it.
  window.__TAURI_INTERNALS__.convertFileSrc = (path) =>
    path.startsWith("data:") ? path : `asset://localhost/${encodeURIComponent(path)}`;

  window.__emit = (payload) => {
    eventHandler?.({ event: "slskcat://event", id: 1, payload });
  };
};

const hit = (username, album, n, slots, speed, ext = "flac") => ({
  username,
  freeSlots: slots,
  speed,
  files: Array.from({ length: n }, (_, i) => ({
    path: `@@music\\${album}\\${String(i + 1).padStart(2, "0")} ${
      ["Rhubarb", "Xtal", "Ageispolis", "Heliosphan", "Green Calx", "Tha"][i % 6]
    }.${ext}`,
    size: 28_000_000 + i * 3_100_000,
    bitrate: [320, 256, 192, 1411][i % 4],
    duration: 180 + i * 37,
    vbr: i % 3 === 0,
  })),
});

const run = async () => {
  // Honour a preinstalled browser when one is provided, so CI and sandboxes
  // do not have to download their own.
  const browser = await chromium.launch(
    process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {},
  );

  for (const scheme of ["dark", "light"]) {
    const context = await browser.newContext({
      viewport: { width: 1240, height: 820 },
      deviceScaleFactor: Number(process.env.SCALE ?? 2),
      colorScheme: scheme,
    });
    const page = await context.newPage();
    await page.addInitScript(bridge);
    await page.goto(URL, { waitUntil: "networkidle" });

    const shot = (name) => page.screenshot({ path: `${OUT}/${scheme}-${name}.png` });

    // 1. Sign-in
    await page.fill('input[autocomplete="username"]', "slsk_listener");
    await page.fill('input[type="password"]', "hunter2");
    await page.waitForTimeout(500);
    await shot("1-connect");

    // 2. Connected, with a search in flight
    await page.evaluate(() => {
      window.__emit({ type: "connected", data: { username: "slsk_listener" } });
      window.__emit({ type: "sharesUpdated", data: { directories: 212, files: 18422 } });
    });
    await page.waitForTimeout(300);

    await page.fill('input[placeholder="Search the network…"]', "aphex twin");
    await page.click('button[type="submit"]');
    await page.waitForTimeout(300);

    await page.evaluate(
      ([a, b, c]) => {
        window.__emit({ type: "searchHits", data: { id: 1, hits: [a, b, c] } });
      },
      [
        hit("velvet_hare", "Aphex Twin - Selected Ambient Works 85-92", 6, 2, 1_240_000),
        // One rip in a different format, because that is what the network
        // actually returns — and it is what gives the format filter something
        // to filter.
        hit("nightporter", "Aphex Twin - Richard D James Album", 5, 0, 480_000, "mp3"),
        hit("cassette_ghost", "Aphex Twin - Windowlicker EP", 4, 1, 2_100_000),
      ],
    );
    await page.waitForTimeout(400);
    await shot("2-search");

    // 2b. The format filter, which stays open across several choices
    await page.click('.picker > .field');
    await page.waitForTimeout(200);
    await page.click('.menu .row:has-text("FLAC")');
    await page.click('.menu .row:has-text("MP3")');
    await page.waitForTimeout(300);
    await shot("2b-formats");
    // Still open — that is the point of it — so clear it and close it.
    await page.click('.menu .row:has-text("All formats")');
    await page.click('.picker > .field');
    await page.waitForTimeout(200);

    // 3. Command palette
    await page.keyboard.press("Control+k");
    await page.waitForTimeout(200);
    await page.keyboard.type("boards of canada");
    await page.waitForTimeout(500);
    await shot("3-palette");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(300);

    // 3b. Wishlist: standing searches the server re-runs
    await page.evaluate(() => {
      window.__emit({ type: "wishlistInterval", data: { seconds: 720 } });
      window.__emit({
        type: "wishlistHits",
        data: {
          query: "boards of canada — geogaddi vinyl rip",
          hits: [
            {
              username: "owl_hours",
              freeSlots: 1,
              speed: 1_050_000,
              files: [
                { path: "@@rips/Boards of Canada - Geogaddi (2002, vinyl)/A1 Ready Lets Go.flac",
                  size: 24_100_000, bitrate: 1411, duration: 98, vbr: false },
                { path: "@@rips/Boards of Canada - Geogaddi (2002, vinyl)/A2 Music Is Math.flac",
                  size: 41_800_000, bitrate: 1411, duration: 313, vbr: false },
              ],
            },
          ],
        },
      });
    });
    // Wishes live under Library now, beside the files already here.
    await page.click('button:has-text("Library")');
    await page.waitForTimeout(300);
    await page.click('.segbtn:has-text("Wishlist")');
    await page.waitForTimeout(300);
    await page.click('.line:has-text("geogaddi")');
    await page.waitForTimeout(400);
    await shot("3b-wishlist");

    // 4. Transfers, mid-flight
    await page.evaluate(() => {
      const t = (username, path, state) =>
        window.__emit({ type: "transferUpdated", data: { id: { username, path }, state } });
      t("velvet_hare", "@@music\\SAW 85-92\\01 Xtal.flac", {
        type: "active",
        data: { transferred: 18_400_000, total: 31_200_000, bytesPerSec: 1_180_000 },
      });
      t("velvet_hare", "@@music\\SAW 85-92\\02 Tha.flac", {
        type: "queued",
        data: { place: 3 },
      });
      t("nightporter", "@@music\\RDJ Album\\04 Carn Marth.flac", {
        type: "paused",
        data: { transferred: 6_100_000, total: 22_800_000 },
      });
      t("cassette_ghost", "@@music\\Windowlicker\\01 Windowlicker.flac", { type: "completed" });
      t("ghost_radio", "@@music\\Drukqs\\09 Avril 14th.flac", {
        type: "failed",
        data: { reason: "The peer closed the connection." },
      });
    });
    await page.click('button:has-text("Transfers")');
    await page.waitForTimeout(500);
    await shot("4-transfers");

    // 4b. Uploads — what other people are taking from you
    await page.evaluate(() => {
      const up = (username, path, size, sent, state, speed = 0) =>
        window.__emit({
          type: "uploadUpdated",
          data: { username, path, size, sent, state, bytesPerSec: speed },
        });
      up("driftwood", "Music/FLAC/Talk Talk/Laughing Stock/01 Myrrhman.flac",
         44_000_000, 29_500_000, { type: "active" }, 840_000);
      up("kithara", "Music/FLAC/Talk Talk/Laughing Stock/02 Ascension Day.flac",
         61_000_000, 0, { type: "queued", data: { place: 1 } });
      up("owl_hours", "Music/Rips/Coil - Musick To Play In The Dark/03 Red Birds.flac",
         52_000_000, 52_000_000, { type: "completed" });
      up("sparrowfall", "Music/Rips/Broadcast - Tender Buttons/04 Corporeal.flac",
         31_000_000, 4_100_000, { type: "failed", data: { reason: "The peer went offline." } });
    });
    await page.click('button[role="tab"]:has-text("Uploads")');
    await page.waitForTimeout(500);
    await shot("4b-uploads");
    await page.click('button[role="tab"]:has-text("Downloads")');
    await page.waitForTimeout(200);

    // 4c. Discover: the people list, with the box filtering it
    await page.click('button:has-text("Discover")');
    await page.waitForTimeout(300);
    await page.click('.entry:has-text("velvet_hare")');
    await page.evaluate(() => {
      window.__emit({
        type: "browseReady",
        data: {
          username: "velvet_hare",
          directories: [
            {
              path: "@@a1b2\\Music\\FLAC\\Aphex Twin - Selected Ambient Works 85-92",
              files: [
                { path: "01 Xtal.flac", size: 30_000_000, bitrate: 1411, duration: 297, vbr: false },
                { path: "02 Tha.flac", size: 41_000_000, bitrate: 1411, duration: 365, vbr: false },
              ],
            },
            {
              path: "@@a1b2\\Music\\FLAC\\Boards of Canada - Geogaddi",
              files: [
                { path: "01 Ready Lets Go.flac", size: 8_000_000, bitrate: 1411, duration: 59, vbr: false },
              ],
            },
          ],
        },
      });
    });
    await page.waitForTimeout(400);
    await shot("4c-discover");

    // 4d. Library: this machine's own folders, with the player running
    await page.click('button:has-text("Library")');
    await page.waitForTimeout(300);
    await page.click('.segbtn:has-text("My list")');
    await page.waitForTimeout(400);
    await shot("4d-library");

    // 4d2. The same library as releases, from tags and embedded artwork.
    await page.click('.shapebtn[aria-label="As releases"]');
    await page.waitForTimeout(500);
    await shot("4d2-albums");
    await page.click(".card");
    await page.waitForTimeout(400);
    await shot("4d3-album");
    await page.click(".back");
    await page.waitForTimeout(300);
    await page.click('.shapebtn[aria-label="As files"]');
    await page.waitForTimeout(300);

    // 4e. The player bar, with its effects panel open. The mocked bridge has
    // no real file behind the path, so playback itself fails — the bar and its
    // controls are what this shot is for.
    await page.click('.file:has-text("01 Xtal.flac") .go');
    await page.waitForTimeout(300);
    // Two presses: the track, then the folder it came from.
    await page.click(".loop");
    await page.click(".loop");
    await page.click(".fx");
    await page.waitForTimeout(400);
    await shot("4e-player");
    await page.click('.fx');
    await page.waitForTimeout(200);

    // 5. Messages: the rooms list
    await page.evaluate(() => {
      window.__emit({
        type: "roomList",
        data: [
          { name: "nicotine", userCount: 412 },
          { name: "ambient", userCount: 208 },
          { name: "idm", userCount: 173 },
          { name: "jazz", userCount: 96 },
          { name: "vinyl rips", userCount: 61 },
        ],
      });
    });
    await page.click('button:has-text("Messages")');
    await page.waitForTimeout(300);
    await page.click('button:has-text("nicotine")');
    await page.waitForTimeout(200);
    await page.evaluate(() => {
      window.__emit({ type: "roomJoined", data: { room: "nicotine", users: ["a", "b", "c"] } });
      const say = (author, body) =>
        window.__emit({ type: "roomMessage", data: { room: "nicotine", message: { author, body } } });
      say("velvet_hare", "anyone have the 2001 remaster?");
      say("nightporter", "i think cassette_ghost was sharing it earlier");
      say("cassette_ghost", "still up, browse me");
    });
    await page.waitForTimeout(500);
    await shot("5-rooms");

    // 5b. A direct-message thread
    await page.evaluate(() => {
      const dm = (author, body) =>
        window.__emit({ type: "privateMessage", data: { author, body } });
      dm("cassette_ghost", "sent you the folder, should be queued");
      dm("cassette_ghost", "the 24bit one is the better transfer fwiw");
    });
    await page.waitForTimeout(300);
    // People and rooms are separate lists now, so the thread is under Users.
    await page.click('.segbtn:has-text("Users")');
    await page.waitForTimeout(200);
    await page.click('.entry:has-text("cassette_ghost") .pick');
    await page.waitForTimeout(200);
    await page.fill('section input.field', "got it, thanks — grabbing now");
    await page.waitForTimeout(400);
    await shot("5b-direct");

    // 5c. The account switcher, above the identity row
    await page.click('.name[aria-haspopup="menu"]');
    await page.waitForTimeout(400);
    await shot("5c-accounts");
    await page.keyboard.press("Escape");
    await page.click('.name[aria-haspopup="menu"]');
    await page.waitForTimeout(200);

    // 6. Settings, rendered from the persisted preferences
    await page.click('button:has-text("Settings")');
    await page.waitForTimeout(400);
    await shot("6-settings");

    // 6c. The proxy section, filled in
    await page.click('span:has-text("Connect through a proxy")');
    await page.waitForTimeout(300);
    await page.fill('.proxy input[placeholder="Host"]', "127.0.0.1");
    await page.waitForTimeout(300);
    await page.evaluate(() => document.querySelector("section:has(.proxy)")?.scrollIntoView());
    await page.waitForTimeout(300);
    await shot("6c-proxy");

    // 6b. The share guard: one path refused outright, one flagged for
    // confirmation. The picker is driven through the mocked dialog command.
    await page.evaluate(() => {
      window.__pick = ["/home/listener/.ssh", "/home/listener/Documents"];
    });
    await page.click('button:has-text("Add folder…")');
    await page.waitForTimeout(800);
    await shot("6b-share-guard");

    // 1b. An account that no longer works. Adding one ends the session that
    // was running, so a refusal lands on a form with nothing behind it — which
    // is why the way back has to be on this screen.
    await page.evaluate(() => {
      window.__emit({ type: "connected", data: { username: "slsk_listener" } });
    });
    await page.waitForTimeout(200);
    await page.click('.name[aria-haspopup="menu"]');
    await page.waitForTimeout(200);
    await page.click('button:has-text("Add another account")');
    await page.waitForTimeout(300);
    await page.fill('input[autocomplete="username"]', "owl_hours");
    await page.fill('input[type="password"]', "hunter2");
    await page.click('button[type="submit"]');
    await page.waitForTimeout(200);
    await page.evaluate(() => {
      window.__emit({ type: "disconnected", data: { type: "requested" } });
      window.__emit({
        type: "loginFailed",
        data: { reason: "The server refused the sign-in: unknown username or wrong password." },
      });
    });
    await page.waitForTimeout(400);
    await shot("1b-refused");

    // 1c. A proxy cannot be applied to an open socket, so changing one rebuilds
    // the session. The wait for that is the sign-in wait, not the form.
    await page.evaluate(() => {
      window.__emit({ type: "connected", data: { username: "slsk_listener" } });
    });
    await page.waitForTimeout(200);
    await page.click('button:has-text("Settings")');
    await page.waitForTimeout(300);
    await page.click('span:has-text("Connect through a proxy")');
    await page.waitForTimeout(200);
    await page.evaluate(() => {
      window.__emit({ type: "disconnected", data: { type: "requested" } });
    });
    await page.waitForTimeout(500);
    await shot("1c-reconnecting");

    await context.close();
  }

  await browser.close();
};

await run();
console.log("screenshots written");

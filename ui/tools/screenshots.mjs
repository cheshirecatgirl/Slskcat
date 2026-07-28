/**
 * Renders the built UI in Chromium against a mocked Tauri bridge, so the
 * interface can be looked at — and regressions caught — without a desktop
 * session. This is how the app is reviewed in headless environments.
 *
 * Usage:
 *   pnpm build
 *   npx serve dist -l 8899        # or: python3 -m http.server 8899 -d dist
 *   node tools/screenshots.mjs ../docs
 *
 * Playwright is not a dependency of this project; install it on demand:
 *   pnpm dlx playwright@latest
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
      return null;
    },
  };

  // Push a core event into the app exactly as the Rust side would.
  window.__emit = (payload) => {
    eventHandler?.({ event: "lark://event", id: 1, payload });
  };
};

const hit = (username, album, n, slots, speed) => ({
  username,
  freeSlots: slots,
  speed,
  files: Array.from({ length: n }, (_, i) => ({
    path: `@@music\\${album}\\${String(i + 1).padStart(2, "0")} ${
      ["Rhubarb", "Xtal", "Ageispolis", "Heliosphan", "Green Calx", "Tha"][i % 6]
    }.flac`,
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
    await page.fill('input[autocomplete="username"]', "lark_listener");
    await page.fill('input[type="password"]', "hunter2");
    await page.waitForTimeout(500);
    await shot("1-connect");

    // 2. Connected, with a search in flight
    await page.evaluate(() => {
      window.__emit({ type: "connected", data: { username: "lark_listener" } });
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
        hitData("velvet_hare", "Aphex Twin - Selected Ambient Works 85-92", 6, 2, 1_240_000),
        hitData("nightporter", "Aphex Twin - Richard D James Album", 5, 0, 480_000),
        hitData("cassette_ghost", "Aphex Twin - Windowlicker EP", 4, 1, 2_100_000),
      ],
    );
    await page.waitForTimeout(400);
    await shot("2-search");

    // 3. Command palette
    await page.keyboard.press("Control+k");
    await page.waitForTimeout(200);
    await page.keyboard.type("boards of canada");
    await page.waitForTimeout(500);
    await shot("3-palette");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(300);

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

    // 5. Rooms
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
    await page.click('button:has-text("Rooms")');
    await page.waitForTimeout(300);
    await page.click('button:has-text("nicotine")');
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

    await context.close();
  }

  await browser.close();
};

// Injected into the page scope by name, so it must be declared as a global.
function hitData(username, album, n, slots, speed) {
  return hit(username, album, n, slots, speed);
}

await run();
console.log("screenshots written");

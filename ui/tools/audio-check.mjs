/**
 * Renders audio through the player's graph in a headless browser and measures
 * what comes out.
 *
 * Silence is the failure mode that looks like success here: the track plays,
 * the clock runs, the meters move, and nothing reaches the speakers. Two
 * things cause it, and this checks both.
 *
 * The first is the shifter. A known tone goes through it and the zero
 * crossings of the result are counted, which says whether it passes audio and
 * whether it passes it at the right pitch.
 *
 * The second is CORS. Files come from Tauri's asset protocol, which is a
 * different origin from the page, and a Web Audio graph fed by a cross-origin
 * element that was not fetched with CORS outputs zeros — no error, no warning.
 * The two origins here stand in for the page and the asset protocol, one
 * fetched plainly and one with `crossorigin`, to show the difference.
 *
 * Usage, from `ui/`:
 *   node tools/audio-check.mjs
 *
 * Set CHROMIUM_PATH to use a browser that is already on the machine.
 */
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { chromium } from "playwright";

const TONE = 440;
/** Zero crossings are a coarse measure of frequency, so the tolerance is wide. */
const TOLERANCE = 0.06;
/** A full-scale sine renders at about 0.7 RMS. Anything near zero is silence. */
const AUDIBLE = 0.05;

/** Three seconds of 16-bit mono sine, as a WAV. */
function tone(hz, seconds = 3, rate = 44100) {
  const frames = rate * seconds;
  const buffer = Buffer.alloc(44 + frames * 2);
  buffer.write("RIFF", 0);
  buffer.writeUInt32LE(36 + frames * 2, 4);
  buffer.write("WAVEfmt ", 8);
  buffer.writeUInt32LE(16, 16);
  buffer.writeUInt16LE(1, 20);
  buffer.writeUInt16LE(1, 22);
  buffer.writeUInt32LE(rate, 24);
  buffer.writeUInt32LE(rate * 2, 28);
  buffer.writeUInt16LE(2, 32);
  buffer.writeUInt16LE(16, 34);
  buffer.write("data", 36);
  buffer.writeUInt32LE(frames * 2, 40);
  for (let i = 0; i < frames; i++) {
    buffer.writeInt16LE(Math.round(20000 * Math.sin((2 * Math.PI * hz * i) / rate)), 44 + i * 2);
  }
  return buffer;
}

const worklet = await readFile(new URL("../src/lib/pitch-worklet.js", import.meta.url));
const wav = tone(TONE);

/** Listen on whatever port is free, and report the origin it landed on. */
const serve = (handler) =>
  new Promise((ready) => {
    const server = createServer(handler);
    server.listen(0, "127.0.0.1", () =>
      ready({ server, origin: `http://127.0.0.1:${server.address().port}` }),
    );
  });

const page = await serve((request, response) => {
  if (request.url === "/pitch-worklet.js") {
    response.writeHead(200, { "content-type": "text/javascript" }).end(worklet);
  } else {
    response.writeHead(200, { "content-type": "text/html" }).end("<!doctype html><title>x</title>");
  }
});

// Stands in for the asset protocol, which answers every request with
// Access-Control-Allow-Origin set to the page's own origin. `/plain` is the
// same file without it, which is what an element fetched without `crossorigin`
// gets, whatever the server sends.
const assets = await serve((request, response) => {
  const headers = { "content-type": "audio/wav" };
  if (request.url !== "/plain.wav") headers["access-control-allow-origin"] = page.origin;
  response.writeHead(200, headers).end(wav);
});

const browser = await chromium.launch({
  ...(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {}),
  args: ["--autoplay-policy=no-user-gesture-required"],
});
const tab = await browser.newPage();
await tab.goto(page.origin);

/** Render a tone through the shifter offline, and report level and pitch. */
const shifted = (semitones) =>
  tab.evaluate(
    async ([hz, steps]) => {
      const rate = 44100;
      const context = new OfflineAudioContext({ numberOfChannels: 2, length: rate, sampleRate: rate });
      await context.audioWorklet.addModule("/pitch-worklet.js");

      const source = context.createOscillator();
      source.frequency.value = hz;
      const shifter = new AudioWorkletNode(context, "pitch-shifter", { outputChannelCount: [2] });
      shifter.parameters.get("ratio").value = 2 ** (steps / 12);
      source.connect(shifter).connect(context.destination);
      source.start();

      const samples = (await context.startRendering()).getChannelData(0);
      // The second half only: the first grain is the ring buffer filling.
      const from = Math.floor(samples.length / 2);
      let sum = 0;
      let crossings = 0;
      for (let i = from; i < samples.length; i++) {
        sum += samples[i] * samples[i];
        if (i > from && samples[i - 1] < 0 !== samples[i] < 0) crossings++;
      }
      const seconds = (samples.length - from) / rate;
      return { rms: Math.sqrt(sum / (samples.length - from)), hz: crossings / 2 / seconds };
    },
    [TONE, semitones],
  );

/** Play a file through a media element into a graph, and report the level. */
const throughElement = (url, cors) =>
  tab.evaluate(
    async ([url, cors]) => {
      const audio = new Audio();
      if (cors) audio.crossOrigin = "anonymous";
      audio.src = url;
      const ready = await new Promise((done) => {
        audio.onloadedmetadata = () => done(true);
        audio.onerror = () => done(false);
        setTimeout(() => done(false), 5000);
      });
      if (!ready) return { loaded: false, rms: 0 };

      const context = new AudioContext();
      const analyser = context.createAnalyser();
      context.createMediaElementSource(audio).connect(analyser).connect(context.destination);
      await context.resume();
      await audio.play();
      await new Promise((resume) => setTimeout(resume, 700));

      const samples = new Float32Array(analyser.fftSize);
      analyser.getFloatTimeDomainData(samples);
      audio.pause();
      await context.close();
      let sum = 0;
      for (const sample of samples) sum += sample * sample;
      return { loaded: true, rms: Math.sqrt(sum / samples.length) };
    },
    [url, cors],
  );

let failed = false;
const report = (ok, line) => {
  if (!ok) failed = true;
  console.log(`${ok ? "ok  " : "FAIL"}  ${line}`);
};

for (const semitones of [0, 12, -12, 7]) {
  const want = TONE * 2 ** (semitones / 12);
  const { rms, hz } = await shifted(semitones);
  const off = Math.abs(hz - want) / want;
  report(
    rms > 0.3 && off < TOLERANCE,
    `shifter ${String(semitones).padStart(3)} st: ${hz.toFixed(0).padStart(4)} Hz out of ` +
      `${want.toFixed(0).padStart(4)} wanted, rms ${rms.toFixed(3)}`,
  );
}

const plain = await throughElement(`${assets.origin}/plain.wav`, false);
report(
  plain.loaded && plain.rms < AUDIBLE,
  `cross-origin without CORS is silent: loaded ${plain.loaded}, rms ${plain.rms.toFixed(3)}`,
);

const asked = await throughElement(`${assets.origin}/tone.wav`, true);
report(
  asked.loaded && asked.rms > AUDIBLE,
  `cross-origin with CORS is audible:   loaded ${asked.loaded}, rms ${asked.rms.toFixed(3)}`,
);

await browser.close();
page.server.close();
assets.server.close();
process.exit(failed ? 1 : 0);

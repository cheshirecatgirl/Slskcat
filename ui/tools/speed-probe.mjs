/**
 * Measures what the two ways of changing tempo actually do to the sound.
 *
 * A media element can change tempo two ways: time-stretch it (`preservesPitch`
 * on, which is the browser's own WSOLA) or resample it and put the pitch back
 * with the worklet. Neither is free, and which is worse depends on the signal,
 * so this renders both and reports two numbers.
 *
 * `purity` is the share of spectral energy near the strongest partial: a
 * stretcher that warbles spreads energy into sidebands and this falls. `crest`
 * is peak over RMS: a stretcher that smears transients flattens the peaks and
 * this falls. A sine is the signal WSOLA reproduces perfectly, so run both.
 *
 * Usage, from `ui/`:
 *   node tools/speed-probe.mjs              # a 440 Hz tone
 *   SIGNAL=clicks node tools/speed-probe.mjs   # a click train
 *
 * Set CHROMIUM_PATH to use a browser that is already on the machine.
 *
 * As of writing, on Chromium: the browser's stretcher holds a sine perfectly
 * at every speed, and holds transients to about 1.5x before falling off — at
 * 2x it loses roughly half the transient peak, which is what "keep pitch"
 * being off exists to avoid.
 */

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { chromium } from "playwright";

/** A click train: sharp transients, which is what a stretcher actually has
 *  trouble with. A sine is the one signal WSOLA reproduces perfectly. */
function clicks(seconds = 8, rate = 44100, every = 0.1) {
  const frames = rate * seconds;
  const b = Buffer.alloc(44 + frames * 2);
  b.write("RIFF", 0); b.writeUInt32LE(36 + frames * 2, 4); b.write("WAVEfmt ", 8);
  b.writeUInt32LE(16, 16); b.writeUInt16LE(1, 20); b.writeUInt16LE(1, 22);
  b.writeUInt32LE(rate, 24); b.writeUInt32LE(rate * 2, 28);
  b.writeUInt16LE(2, 32); b.writeUInt16LE(16, 34); b.write("data", 36);
  b.writeUInt32LE(frames * 2, 40);
  const gap = Math.round(rate * every);
  for (let i = 0; i < frames; i++) {
    const into = i % gap;
    // A short decaying burst: loud, brief, and easy to find again.
    const v = into < 64 ? Math.round(28000 * Math.exp(-into / 12) * Math.sin(i * 0.9)) : 0;
    b.writeInt16LE(v, 44 + i * 2);
  }
  return b;
}

function tone(hz, seconds = 8, rate = 44100) {
  const frames = rate * seconds;
  const b = Buffer.alloc(44 + frames * 2);
  b.write("RIFF", 0); b.writeUInt32LE(36 + frames * 2, 4); b.write("WAVEfmt ", 8);
  b.writeUInt32LE(16, 16); b.writeUInt16LE(1, 20); b.writeUInt16LE(1, 22);
  b.writeUInt32LE(rate, 24); b.writeUInt32LE(rate * 2, 28);
  b.writeUInt16LE(2, 32); b.writeUInt16LE(16, 34); b.write("data", 36);
  b.writeUInt32LE(frames * 2, 40);
  for (let i = 0; i < frames; i++)
    b.writeInt16LE(Math.round(20000 * Math.sin((2 * Math.PI * hz * i) / rate)), 44 + i * 2);
  return b;
}

const worklet = await readFile(new URL("./src/lib/pitch-worklet.js", import.meta.url));
const wav = process.env.SIGNAL === "clicks" ? clicks() : tone(440);
const serve = (h) => new Promise((r) => { const s = createServer(h); s.listen(0, "127.0.0.1", () => r({ s, o: `http://127.0.0.1:${s.address().port}` })); });
const page = await serve((rq, rs) => {
  if (rq.url === "/pitch-worklet.js") rs.writeHead(200, { "content-type": "text/javascript" }).end(worklet);
  else rs.writeHead(200, { "content-type": "text/html" }).end("<!doctype html><title>x</title>");
});
const asset = await serve((rq, rs) => rs.writeHead(200, { "content-type": "audio/wav", "access-control-allow-origin": page.o }).end(wav));

const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH, args: ["--autoplay-policy=no-user-gesture-required"] });
const t = await b.newPage();
await t.goto(page.o);

// Purity: share of spectral energy inside a narrow band round the expected
// partial. A time-stretcher smears energy into sidebands, which shows up here.
const probe = (url, speed, preserve, semitones) =>
  t.evaluate(async ([url, speed, preserve, semitones]) => {
    const audio = new Audio();
    audio.crossOrigin = "anonymous";
    audio.src = url;
    await new Promise((d) => { audio.onloadedmetadata = d; setTimeout(d, 4000); });

    const ctx = new AudioContext();
    await ctx.audioWorklet.addModule("/pitch-worklet.js");
    const src = ctx.createMediaElementSource(audio);
    const shifter = new AudioWorkletNode(ctx, "pitch-shifter", { outputChannelCount: [2] });
    shifter.parameters.get("ratio").value = 2 ** (semitones / 12);
    const an = ctx.createAnalyser();
    an.fftSize = 16384;
    an.smoothingTimeConstant = 0;
    src.connect(shifter).connect(an).connect(ctx.destination);

    audio.preservesPitch = preserve;
    audio.playbackRate = speed;
    await ctx.resume();
    await audio.play();
    await new Promise((r) => setTimeout(r, 2500));

    // Crest factor of the waveform: peak over RMS. A smeared transient is a
    // lower peak for the same energy, so this falls as the stretcher works
    // harder. A clean signal path leaves it where the source put it.
    const wave = new Float32Array(an.fftSize);
    an.getFloatTimeDomainData(wave);
    let sq = 0, top = 0;
    for (const v of wave) { sq += v * v; top = Math.max(top, Math.abs(v)); }
    const crest = top / Math.sqrt(sq / wave.length);

    const bins = new Float32Array(an.frequencyBinCount);
    an.getFloatFrequencyData(bins);
    audio.pause(); await ctx.close();

    const hzPerBin = ctx.sampleRate / an.fftSize;
    let total = 0, peakBin = 0, peak = -Infinity;
    const lin = Array.from(bins, (db) => 10 ** (db / 10));
    for (let i = 1; i < lin.length; i++) { total += lin[i]; if (bins[i] > peak) { peak = bins[i]; peakBin = i; } }
    // Energy within ±25 Hz of the peak.
    const half = Math.max(1, Math.round(25 / hzPerBin));
    let near = 0;
    for (let i = Math.max(1, peakBin - half); i <= Math.min(lin.length - 1, peakBin + half); i++) near += lin[i];
    return { peakHz: peakBin * hzPerBin, purity: near / total, crest };
  }, [url, speed, preserve, semitones]);

const url = `${asset.o}/tone.wav`;
console.log("speed  approach                              peak Hz   purity   crest");
for (const speed of [1, 0.75, 1.25, 1.5, 2]) {
  const a = await probe(url, speed, true, 0);
  // Resample instead of stretch, then put the pitch back with the worklet.
  const correction = -12 * Math.log2(speed);
  const c = await probe(url, speed, false, correction);
  console.log(`${speed.toFixed(2)}   browser preservesPitch                ${a.peakHz.toFixed(0).padStart(5)}   ${a.purity.toFixed(4)}   ${a.crest.toFixed(2)}`);
  console.log(`       resample + worklet (${correction.toFixed(2).padStart(6)} st)     ${c.peakHz.toFixed(0).padStart(5)}   ${c.purity.toFixed(4)}   ${c.crest.toFixed(2)}`);
}
await b.close(); page.s.close(); asset.s.close();

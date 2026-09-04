/**
 * A real-time pitch shifter, as an audio worklet.
 *
 * An audio element gives you tempo for free — `playbackRate` with
 * `preservesPitch` on is a time-stretcher the browser already ships. It gives
 * you no way to move the pitch without the tempo, so that half is here.
 *
 * The method is the classic two-tap crossfading delay. Input goes into a ring
 * buffer; two read pointers chase the write pointer half a grain apart, both
 * advancing at the pitch ratio rather than at 1. Reading faster than you write
 * raises the pitch, and the drift that would otherwise run the pointer into
 * the writer is absorbed by wrapping it once per grain — the wrap is a
 * discontinuity, which is why there are two of them, crossfaded so that one is
 * always at full gain while the other is silent through its jump.
 *
 * `sin² + cos² = 1`, so the pair sums to unity gain at every phase and the
 * output does not pump.
 *
 * It smears transients, as every granular shifter does. That is the honest
 * trade for something that runs live on one thread and needs no library: the
 * alternative is a phase vocoder, which is an order of magnitude more code and
 * still smears, differently.
 */

/** Grain length in samples. Short enough not to slap back, long enough not to warble. */
const GRAIN = 1024;
/** Ring buffer length. Must exceed a grain plus a render quantum, comfortably. */
const SIZE = 8192;

class PitchProcessor extends AudioWorkletProcessor {
  static get parameterDescriptors() {
    return [
      {
        name: "ratio",
        defaultValue: 1,
        // Two octaves either way. Past that a granular shifter is an effect
        // rather than a transposition, and the controls do not offer it.
        minValue: 0.25,
        maxValue: 4,
        automationRate: "k-rate",
      },
    ];
  }

  constructor() {
    super();
    /** One ring buffer per channel, grown on first use. */
    this.buffers = [];
    this.write = 0;
    /** Position within the grain, 0 to 1. */
    this.phase = 0;
  }

  process(inputs, outputs, parameters) {
    const input = inputs[0];
    const output = outputs[0];
    if (!output || output.length === 0) return true;

    // No input yet — the element may still be loading. Stay alive and silent.
    if (!input || input.length === 0) {
      for (const channel of output) channel.fill(0);
      return true;
    }

    const ratio = parameters.ratio[0] ?? 1;
    const channels = output.length;
    const frames = output[0].length;

    while (this.buffers.length < channels) {
      this.buffers.push(new Float32Array(SIZE));
    }

    // At a ratio of exactly one there is nothing to do, and passing the signal
    // through untouched avoids a grain's worth of smearing for no benefit.
    if (ratio === 1) {
      for (let c = 0; c < channels; c++) {
        const source = input[c] ?? input[0];
        const buffer = this.buffers[c];
        for (let i = 0; i < frames; i++) {
          const sample = source ? source[i] : 0;
          buffer[(this.write + i) % SIZE] = sample;
          output[c][i] = sample;
        }
      }
      this.write = (this.write + frames) % SIZE;
      this.phase = 0;
      return true;
    }

    const step = (ratio - 1) / GRAIN;
    let phase = this.phase;

    for (let i = 0; i < frames; i++) {
      const at = (this.write + i) % SIZE;
      for (let c = 0; c < channels; c++) {
        const source = input[c] ?? input[0];
        this.buffers[c][at] = source ? source[i] : 0;
      }

      const second = phase < 0.5 ? phase + 0.5 : phase - 0.5;
      // `sin²` and `cos²` of the same angle: one rises exactly as the other
      // falls, and together they are always 1.
      const weightA = Math.sin(Math.PI * phase) ** 2;
      const weightB = 1 - weightA;

      const headA = at - GRAIN + phase * GRAIN;
      const headB = at - GRAIN + second * GRAIN;

      for (let c = 0; c < channels; c++) {
        const buffer = this.buffers[c];
        output[c][i] = read(buffer, headA) * weightA + read(buffer, headB) * weightB;
      }

      phase += step;
      phase -= Math.floor(phase);
    }

    this.write = (this.write + frames) % SIZE;
    this.phase = phase;
    return true;
  }
}

/** One sample at a fractional position, wrapped, linearly interpolated. */
function read(buffer, position) {
  const wrapped = ((position % SIZE) + SIZE) % SIZE;
  const index = Math.floor(wrapped);
  const fraction = wrapped - index;
  const a = buffer[index];
  const b = buffer[(index + 1) % SIZE];
  return a + (b - a) * fraction;
}

registerProcessor("pitch-shifter", PitchProcessor);

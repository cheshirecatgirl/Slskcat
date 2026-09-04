/**
 * Playback, and the effects applied to it.
 *
 * One `<audio>` element feeding a Web Audio graph:
 *
 *   element → shifter → dry ─────────────┐
 *                     → wet → convolver ─┴→ output → speakers
 *
 * The element does the decoding and the seeking, which keeps this light: no
 * decoding of whole files into memory, no dependency, and a four hundred
 * megabyte FLAC costs the same as a three minute one.
 *
 * Speed and pitch are independent, and both move while the track plays, but
 * they come from different places. Tempo is the element's own `playbackRate`
 * with `preservesPitch` on — a time-stretcher the browser already ships, and a
 * better one than is worth writing here. For pitch it offers nothing, so that
 * half is the worklet sitting between the source and the effects.
 */

import shifterUrl from "./pitch-worklet.js?url";

import { fileName } from "./format";

export interface Track {
  /** Absolute path on this machine, which is what identifies it. */
  path: string;
  /**
   * The URL the element loads.
   *
   * A path is not one: the webview cannot open a file by name, and Tauri's
   * asset scheme is what turns one into something an `<audio>` element will
   * accept — within the scope the app grants at startup.
   */
  src: string;
  name: string;
  /** Who it came from, when that is known. */
  from?: string;
}

/** Seconds of tail on the reverb. Longer than a room, shorter than a hall. */
const REVERB_SECONDS = 2.2;

export class Player {
  track = $state<Track | null>(null);
  playing = $state(false);
  /** Seconds. */
  position = $state(0);
  duration = $state(0);
  volume = $state(0.9);

  /** Tempo, 0.5 to 2. Does not touch the pitch. */
  speed = $state(1);
  /** Semitones, -12 to 12. Does not touch the tempo. */
  pitch = $state(0);
  /** 0 to 1, how much of the signal is reverberated. */
  reverb = $state(0);
  /**
   * False where the platform has no audio worklet, which leaves tempo and
   * reverb working and pitch not. Better to say so than to offer a slider
   * that does nothing.
   */
  pitchAvailable = $state(true);
  /**
   * False where the graph could not be built at all, which leaves the element
   * playing on its own: sound, but no speed-independent pitch and no reverb.
   */
  effectsAvailable = $state(true);

  #audio: HTMLAudioElement | null = null;
  #context: AudioContext | null = null;
  #dry: GainNode | null = null;
  #wet: GainNode | null = null;
  #shifter: AudioWorkletNode | null = null;
  /**
   * Whether the graph has been attempted.
   *
   * `createMediaElementSource` can only be called once per element, and it
   * takes the element's output away from the speakers, so a second attempt
   * after a half-built graph throws and leaves the track silent.
   */
  #built = false;

  /** Attach to the element once it exists. */
  attach(audio: HTMLAudioElement) {
    this.#audio = audio;
    audio.volume = this.volume;
  }

  /**
   * Build the graph on first play.
   *
   * Deferred because a browser will not start an `AudioContext` before a
   * gesture, and building one at load leaves it suspended and silent.
   */
  async #graph() {
    if (this.#built || !this.#audio) return;
    this.#built = true;

    const context = new AudioContext();
    const source = context.createMediaElementSource(this.#audio);
    try {
      // The shifter is optional: without a worklet everything downstream of
      // it still works and only pitch is lost.
      let head: AudioNode = source;
      try {
        await context.audioWorklet.addModule(shifterUrl);
        const shifter = new AudioWorkletNode(context, "pitch-shifter", {
          outputChannelCount: [2],
        });
        source.connect(shifter);
        head = shifter;
        this.#shifter = shifter;
      } catch {
        this.pitchAvailable = false;
      }

      const dry = context.createGain();
      const wet = context.createGain();
      const convolver = context.createConvolver();
      convolver.buffer = impulse(context, REVERB_SECONDS);

      head.connect(dry).connect(context.destination);
      head.connect(wet).connect(convolver).connect(context.destination);

      this.#context = context;
      this.#dry = dry;
      this.#wet = wet;
    } catch {
      // The element's output belongs to the graph now whether the rest of it
      // built or not, so send it to the speakers plainly. Losing the effects
      // is recoverable; losing the sound is not.
      source.disconnect();
      source.connect(context.destination);
      this.#context = context;
      this.pitchAvailable = false;
      this.effectsAvailable = false;
    }
  }

  async play(track: Track) {
    if (!this.#audio) return;
    if (this.track?.path !== track.path) {
      this.track = track;
      this.position = 0;
      this.#audio.src = track.src;
    }
    await this.#graph();
    await this.#context?.resume();
    this.apply();
    try {
      await this.#audio.play();
      this.playing = true;
    } catch {
      // A format the platform cannot decode, or a file that moved. Neither is
      // worth an exception reaching the interface.
      this.playing = false;
    }
  }

  toggle() {
    if (!this.#audio || !this.track) return;
    if (this.playing) {
      this.#audio.pause();
      this.playing = false;
    } else {
      void this.#audio.play().then(() => (this.playing = true));
    }
  }

  stop() {
    this.#audio?.pause();
    this.playing = false;
    this.track = null;
    this.position = 0;
  }

  seek(seconds: number) {
    if (!this.#audio || !Number.isFinite(seconds)) return;
    this.#audio.currentTime = Math.max(0, Math.min(seconds, this.duration));
    this.position = this.#audio.currentTime;
  }

  /** Push the current settings into the element and the graph. */
  apply() {
    const audio = this.#audio;
    if (audio) {
      audio.volume = this.volume;
      audio.playbackRate = this.speed;
      // Always on: pitch is its own control now, so the element's job is
      // tempo and nothing else.
      audio.preservesPitch = true;
    }
    if (this.#shifter) {
      // Twelve semitones to the octave, and an octave is a doubling.
      const ratio = 2 ** (this.pitch / 12);
      // Ramped rather than set: dragging the slider would otherwise step the
      // read rate between blocks, which is audible as a click.
      const target = this.#shifter.parameters.get("ratio");
      const now = this.#context?.currentTime ?? 0;
      target?.cancelScheduledValues(now);
      target?.linearRampToValueAtTime(ratio, now + 0.05);
    }
    if (this.#dry && this.#wet) {
      // Equal-power rather than linear, so the middle of the control is not a
      // dip in loudness.
      this.#dry.gain.value = Math.cos((this.reverb * Math.PI) / 2);
      this.#wet.gain.value = Math.sin((this.reverb * Math.PI) / 2);
    }
  }
}

/**
 * A reverb tail, generated rather than shipped.
 *
 * Decaying noise is the crude form of an impulse response, and for a control
 * that exists to smear a track rather than to model a concert hall it is the
 * honest amount of machinery. Two channels with independent noise gives it
 * width; the sixth-power decay is steeper than exponential and keeps the tail
 * from sounding like a hiss.
 */
function impulse(context: AudioContext, seconds: number): AudioBuffer {
  const length = Math.floor(context.sampleRate * seconds);
  const buffer = context.createBuffer(2, length, context.sampleRate);
  for (let channel = 0; channel < 2; channel++) {
    const samples = buffer.getChannelData(channel);
    for (let i = 0; i < length; i++) {
      samples[i] = (Math.random() * 2 - 1) * (1 - i / length) ** 6;
    }
  }
  return buffer;
}

/** A track from a path, named the way the rest of the interface names files. */
export function trackOf(path: string, src: string, from?: string): Track {
  return { path, src, name: fileName(path), from };
}

export const player = new Player();

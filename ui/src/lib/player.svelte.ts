/**
 * Playback, and the effects applied to it.
 *
 * One `<audio>` element feeding a Web Audio graph:
 *
 *   element → dry ─────────────┐
 *           → wet → convolver ─┴→ output → speakers
 *
 * The element does the decoding and the seeking, which is what keeps this
 * light: no decoding of whole files into memory, no dependency, and a four
 * hundred megabyte FLAC costs the same as a three minute one.
 *
 * Speed and pitch are the same control on an audio element — playing faster
 * raises the pitch, as tape does. `preservesPitch` separates them: with it on,
 * speed is tempo alone; with it off, the two move together the way a record at
 * the wrong speed does. Both are worth having, so it is a switch rather than a
 * decision made here.
 */

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

  /** 0.5 to 2. */
  speed = $state(1);
  /** Whether speed leaves the pitch alone. */
  keepPitch = $state(true);
  /** 0 to 1, how much of the signal is reverberated. */
  reverb = $state(0);

  #audio: HTMLAudioElement | null = null;
  #context: AudioContext | null = null;
  #dry: GainNode | null = null;
  #wet: GainNode | null = null;

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
  #graph() {
    if (this.#context || !this.#audio) return;
    const context = new AudioContext();
    const source = context.createMediaElementSource(this.#audio);

    const dry = context.createGain();
    const wet = context.createGain();
    const convolver = context.createConvolver();
    convolver.buffer = impulse(context, REVERB_SECONDS);

    source.connect(dry).connect(context.destination);
    source.connect(wet).connect(convolver).connect(context.destination);

    dry.gain.value = 1 - this.reverb;
    wet.gain.value = this.reverb;

    this.#context = context;
    this.#dry = dry;
    this.#wet = wet;
  }

  async play(track: Track) {
    if (!this.#audio) return;
    if (this.track?.path !== track.path) {
      this.track = track;
      this.position = 0;
      this.#audio.src = track.src;
    }
    this.#graph();
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
      audio.preservesPitch = this.keepPitch;
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

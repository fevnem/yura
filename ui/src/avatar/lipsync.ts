import * as THREE from 'three';

import type { Avatar } from './avatar';

/**
 * Mouth movement.
 *
 * There is no phoneme data available from the WebView2 speech engine, so this
 * works from a loudness envelope instead: an external source (the speech
 * controller, or later a real audio analyser) calls `setLevel`, and this turns
 * that scalar into plausible mouth shapes.
 *
 * Two details make it convincing rather than a volume meter:
 *  - asymmetric smoothing: the mouth opens fast and closes slowly, like a jaw;
 *  - shape varies with level, so quiet syllables read as `ih` and loud ones as
 *    `aa`, instead of everything pulsing through the same hole.
 */

/**
 * Mirrors `yura-avatar::lipsync::VISEMES`.
 * Each row sums to 1.0: a unit emphasis across the three bands.
 */
const VISEMES: { preset: string; low: number; mid: number; high: number }[] = [
  { preset: 'aa', low: 0.87, mid: 0.13, high: 0.0 },
  { preset: 'oh', low: 0.54, mid: 0.46, high: 0.0 },
  { preset: 'ou', low: 0.38, mid: 0.58, high: 0.04 },
  { preset: 'ee', low: 0.07, mid: 0.22, high: 0.71 },
  { preset: 'ih', low: 0.14, mid: 0.61, high: 0.25 },
];

export class LipSync {
  private level = 0;
  private target = 0;
  private emphasis = 0;

  constructor(
    private avatar: Avatar,
    private gain = 1,
  ) {}

  setGain(gain: number): void {
    this.gain = gain;
  }

  /** 0–1 loudness, called ~30×/s while audio is playing. */
  setLevel(level: number): void {
    this.target = THREE.MathUtils.clamp(level, 0, 1);
  }

  /** Punch the mouth open on a stressed syllable. */
  accent(): void {
    this.emphasis = Math.max(this.emphasis, 0.35);
  }

  stop(): void {
    this.target = 0;
    this.emphasis = 0;
  }

  update(dt: number, elapsed: number): void {
    const rising = this.target > this.level;
    const rate = rising ? 22 : 7; // fast open, slow close
    this.level += (this.target - this.level) * Math.min(1, dt * rate);
    this.emphasis *= Math.exp(-dt * 6);

    const level = Math.min(1, this.level * this.gain + this.emphasis);

    // Spectral guess: loud = open vowel, quiet = narrow vowel, plus a little
    // jitter so consecutive syllables are not identical.
    const jitter = 0.5 + 0.5 * Math.sin(elapsed * 11.3) * Math.sin(elapsed * 7.1);
    const low = Math.pow(level, 1.35) * (0.82 + 0.35 * jitter);
    const mid = level * (0.55 + 0.3 * (1 - jitter));
    const high = Math.max(0, level - 0.5) * 1.5 * jitter;

    if (level < 0.015) {
      this.avatar.setLayer('mouth', {});
      return;
    }

    const weights: Record<string, number> = {};
    for (const viseme of VISEMES) {
      const score = viseme.low * low + viseme.mid * mid + viseme.high * high;
      if (score > 0.02) weights[viseme.preset] = Math.min(1, score * 0.85);
    }
    this.avatar.setLayer('mouth', weights);
  }
}

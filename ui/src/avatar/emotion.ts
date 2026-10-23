import type { AvatarSpec, Emotion } from '../bridge/types';

/**
 * Mood → expression weights, with inertia.
 *
 * Emotions never snap. A damping factor of ~4/s means a shift from `neutral` to
 * `joy` lands over roughly 400 ms, which reads as a face *changing its mind*
 * rather than swapping textures. Every emotion row comes from the Rust avatar
 * spec, so retuning a mood is a one-line change in `yura-avatar`.
 *
 * Also tracks a hue per emotion so the procedural spirit — and the room tint —
 * can follow the mood.
 */

const EMOTION_HUE: Record<Emotion, number> = {
  neutral: 0,
  joy: 46,
  amused: 32,
  sad: 212,
  angry: 2,
  surprised: 56,
  shy: 330,
  thinking: 190,
  sleepy: 255,
  affection: 340,
};

export class EmotionMixer {
  private readonly table = new Map<Emotion, [string, number][]>();
  private target = new Map<string, number>();
  private current = new Map<string, number>();
  private intensity = 0;
  private targetIntensity = 0;
  private emotion: Emotion = 'neutral';

  constructor(spec: AvatarSpec) {
    for (const entry of spec.expressions) {
      this.table.set(entry.emotion, entry.weights);
    }
  }

  set(emotion: Emotion, intensity?: number): void {
    this.emotion = emotion;
    const weights = this.table.get(emotion) ?? [];
    this.target = new Map(weights);
    this.targetIntensity = intensity ?? (emotion === 'neutral' ? 0 : 0.75);
  }

  get currentEmotion(): Emotion {
    return this.emotion;
  }

  hue(): number {
    return EMOTION_HUE[this.emotion] ?? 0;
  }

  /** Damped weights, ready for `avatar.setLayer('mood', …)`. */
  update(dt: number): Record<string, number> {
    const k = 1 - Math.exp(-dt * 4.2);
    const keys = new Set([...this.target.keys(), ...this.current.keys()]);

    for (const key of keys) {
      const goal = this.target.get(key) ?? 0;
      const now = this.current.get(key) ?? 0;
      const next = now + (goal - now) * k;
      if (next < 0.002) this.current.delete(key);
      else this.current.set(key, next);
    }

    this.intensity += (this.targetIntensity - this.intensity) * k;

    const out: Record<string, number> = {};
    const gain = 0.35 + this.intensity * 0.75;
    for (const [key, value] of this.current) {
      out[key] = Math.min(1, value * gain);
    }
    return out;
  }

  /** 0–1, for tinting the room. */
  get strength(): number {
    return this.intensity;
  }
}

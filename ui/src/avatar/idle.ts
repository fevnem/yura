import * as THREE from 'three';

import type { IdleParams } from '../bridge/types';
import type { Avatar } from './avatar';

/**
 * The things that keep her alive while nothing is happening.
 *
 * Breathing, a slow weight shift, and blinks that are deliberately *not*
 * periodic. Blink timing uses a jittered interval plus a one-in-five chance of
 * a double blink — the pattern real eyes make. Get this wrong and a 3D
 * character reads as a mannequin no matter how good the model is.
 */
export class IdleAnimator {
  private params: IdleParams;
  private blinkCountdown: number;
  private blinkPhase = -1; // -1 = idle, otherwise seconds into the blink
  private doubleBlinkQueued = false;
  private swayPhase = Math.random() * Math.PI * 2;

  constructor(
    private avatar: Avatar,
    params: IdleParams,
    private options: { motion: boolean; blink: boolean },
  ) {
    this.params = params;
    this.blinkCountdown = this.nextInterval();
  }

  setParams(params: IdleParams): void {
    this.params = params;
  }

  setEnabled(options: { motion?: boolean; blink?: boolean }): void {
    if (options.motion !== undefined) this.options.motion = options.motion;
    if (options.blink !== undefined) this.options.blink = options.blink;
  }

  update(dt: number, elapsed: number): void {
    this.breathe(elapsed);
    this.blink(dt);
  }

  private breathe(elapsed: number): void {
    if (!this.options.motion) return;

    const chest = this.avatar.bone('chest');
    const spine = this.avatar.bone('spine');
    const { breath_period, breath_amplitude } = this.params;
    const phase = (elapsed / breath_period) * Math.PI * 2;

    if (chest) {
      chest.rotation.x = breath_amplitude * Math.sin(phase);
    }
    if (spine) {
      const sway = Math.sin(elapsed * 0.24 + this.swayPhase) * breath_amplitude * 0.4;
      spine.rotation.z = sway;
      spine.rotation.x = breath_amplitude * 0.18 * Math.sin(phase + 0.6);
    }
  }

  private blink(dt: number): void {
    if (!this.options.blink) {
      this.avatar.setLayer('blink', {});
      return;
    }

    if (this.blinkPhase < 0) {
      this.blinkCountdown -= dt;
      if (this.blinkCountdown <= 0) {
        this.blinkPhase = 0;
      }
      return;
    }

    // Close fast (0.07 s), open slower (0.13 s).
    const closeTime = 0.07;
    const openTime = 0.13;
    this.blinkPhase += dt;

    let weight: number;
    if (this.blinkPhase < closeTime) {
      weight = this.blinkPhase / closeTime;
    } else if (this.blinkPhase < closeTime + openTime) {
      weight = 1 - (this.blinkPhase - closeTime) / openTime;
    } else {
      weight = 0;
      this.blinkPhase = -1;
      if (this.doubleBlinkQueued) {
        this.doubleBlinkQueued = false;
        this.blinkCountdown = 0.09;
      } else {
        this.blinkCountdown = this.nextInterval();
        this.doubleBlinkQueued = Math.random() < 0.2;
      }
    }

    this.avatar.setLayer('blink', {
      blink: weight,
      blinkLeft: weight * 0.35,
      blinkRight: weight * 0.35,
    });
  }

  private nextInterval(): number {
    const { blink_interval, blink_jitter } = this.params;
    return blink_interval + Math.random() * blink_jitter;
  }

  /** Force a blink — used when the user sends a message; it reads as attention. */
  triggerBlink(): void {
    if (this.blinkPhase < 0) this.blinkCountdown = Math.min(this.blinkCountdown, 0.05);
  }

  /** A small head nod, for acknowledgement. */
  nod(): void {
    const head = this.avatar.bone('head');
    if (!head) return;
    const rest = this.avatar.restRotation();
    head.rotation.x = rest.x + 0.06;
    window.setTimeout(() => {
      if (head) head.rotation.x = rest.x;
    }, 180);
  }
}

export function clampAngle(value: number, limit: number): number {
  return THREE.MathUtils.clamp(value, -limit, limit);
}

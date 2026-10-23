import * as THREE from 'three';

import type { IdleParams } from '../bridge/types';
import type { Avatar } from './avatar';

/**
 * Where she looks.
 *
 * Two systems run in parallel: VRM's own `lookAt` drives the eyes and can track
 * something small and close (the cursor), while the head bone *partially*
 * follows so she never looks like a security camera. When the cursor has not
 * moved for a while, the target drifts on its own — a person thinking, not a
 * crosshair.
 */
export class GazeController {
  private pointer = new THREE.Vector2(0, 0);
  private aim = new THREE.Vector3(0, 1.32, 2.4);
  private wander = new THREE.Vector3();
  private spread = 0.0012;
  private holdTimer = 0;
  private idleSeconds = 0;
  private lastPointer = new THREE.Vector2(0, 0);

  constructor(
    private avatar: Avatar,
    private camera: THREE.PerspectiveCamera,
    private params: IdleParams,
    private enabled: boolean,
  ) {}

  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
  }

  setParams(params: IdleParams): void {
    this.params = params;
  }

  update(dt: number, elapsed: number): void {
    const head = this.avatar.bone('head');
    const rest = this.avatar.restRotation();

    if (!this.enabled) {
      if (head) {
        head.rotation.x += (rest.x - head.rotation.x) * Math.min(1, dt * 4);
        head.rotation.y += (rest.y - head.rotation.y) * Math.min(1, dt * 4);
      }
      return;
    }

    // Has the cursor moved recently?
    const moved = this.pointer.distanceToSquared(this.lastPointer) > 1e-6;
    this.lastPointer.copy(this.pointer);
    this.idleSeconds = moved ? 0 : this.idleSeconds + dt;

    // Target: the cursor plane while the user is active, a soft wander when not.
    this.holdTimer -= dt;
    const wanderOn = this.idleSeconds > 1.4;

    if (this.holdTimer <= 0) {
      this.holdTimer = this.params.gaze_hold + Math.random() * this.params.gaze_hold;
      const s = this.params.gaze_wander;
      const dx = (Math.random() - 0.5) * 2 * s;
      const dy = (Math.random() - 0.5) * s;
      const dz = (Math.random() - 0.5) * 2 * s;
      this.wander.lerp(new THREE.Vector3(dx, dy, dz), 0.6);
    }

    const point = new THREE.Vector3(this.pointer.x, this.pointer.y, 0.5).unproject(this.camera);
    const dir = point.sub(this.camera.position).normalize();
    const distance = 2.6;
    const cursorWorld = this.camera.position.clone().add(dir.multiplyScalar(distance));

    const goal = wanderOn
      ? this.aim.clone().lerp(cursorWorld, 0.15).add(this.wander)
      : cursorWorld;

    // Critically damped follow — eyes lead, head lags.
    const k = 1 - Math.exp(-dt * 9);
    this.aim.lerp(goal, k);
    this.spread += ((wanderOn ? 0.004 : 0.001) - this.spread) * Math.min(1, dt * 2);

    const target = this.avatar.lookTarget;
    target.position.copy(this.aim);
    target.position.x += Math.sin(elapsed * 1.7) * this.spread;
    target.position.y += Math.cos(elapsed * 2.1) * this.spread * 0.6;

    if (!head) return;

    // Head follows a fraction of the way, clamped so it never snaps around.
    const headWorld = new THREE.Vector3();
    head.getWorldPosition(headWorld);
    const to = this.aim.clone().sub(headWorld);
    const yaw = Math.atan2(to.x, Math.max(0.2, to.z));
    const pitch = -Math.atan2(to.y, Math.max(0.2, Math.hypot(to.x, to.z)));

    const follow = this.params.head_follow;
    const yawGoal = rest.y + THREE.MathUtils.clamp(yaw, -0.42, 0.42) * follow;
    const pitchGoal = rest.x + THREE.MathUtils.clamp(pitch, -0.26, 0.3) * follow;

    const hk = 1 - Math.exp(-dt * 5.5);
    head.rotation.y += (yawGoal - head.rotation.y) * hk;
    head.rotation.x += (pitchGoal - head.rotation.x) * hk;
  }

  setPointer(ndc: THREE.Vector2): void {
    this.pointer.copy(ndc);
  }
}

import * as THREE from 'three';

import { createBackdrop, type BackdropHandle } from './backdrop';
import { createParticles, type ParticlesHandle } from './particles';
import { createPostFX, type PostFXHandle } from './postfx';

/**
 * Owns the renderer, the camera and the frame loop.
 *
 * Nothing here knows what an avatar or a message is: subsystems register a tick
 * callback and the stage calls them in order. That keeps the update order
 * explicit (backdrop → avatar → particles) instead of emergent.
 */

export type Quality = 'low' | 'medium' | 'high';
export type Tick = (dt: number, elapsed: number) => void;

export interface StageInitial {
  hue: number;
  quality: Quality;
  /** World x of the character, so effects can avoid her silhouette. */
  focusX?: number;
}

export class Stage {
  readonly scene = new THREE.Scene();
  readonly camera: THREE.PerspectiveCamera;
  readonly renderer: THREE.WebGLRenderer;

  private postfx: PostFXHandle;
  private backdrop: BackdropHandle;
  private particles: ParticlesHandle;
  private clock = new THREE.Clock();
  private ticks: Tick[] = [];
  private frame = 0;
  private running = false;

  private pointer = new THREE.Vector2(0, 0);
  private cameraHome = new THREE.Vector3(0, 1.34, 3.15);
  private energy = 0;

  constructor(canvas: HTMLCanvasElement, initial: StageInitial) {
    this.renderer = new THREE.WebGLRenderer({
      canvas,
      alpha: true,
      antialias: false,
      powerPreference: 'high-performance',
      stencil: false,
    });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.06;
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.setClearColor(0x000000, 0);

    this.camera = new THREE.PerspectiveCamera(30, 1, 0.1, 100);
    this.camera.position.copy(this.cameraHome);
    this.camera.lookAt(0, 1.2, 0);

    this.backdrop = createBackdrop(initial.hue);
    this.scene.add(this.backdrop.mesh);

    this.particles = createParticles({
      hue: initial.hue,
      avoidX: initial.focusX,
      avoidRadius: 0.85,
    });
    this.scene.add(this.particles.points);

    this.postfx = createPostFX(this.renderer, this.scene, this.camera, initial.quality);

    this.resize();
    window.addEventListener('resize', this.resize);
    window.addEventListener('pointermove', this.onPointerMove);
  }

  /** Register a per-frame callback. Order of registration is update order. */
  addTick(tick: Tick): void {
    this.ticks.push(tick);
  }

  /** 0–1 loudness hint that lights the air and the rim. */
  setEnergy(value: number): void {
    this.energy = THREE.MathUtils.clamp(value, 0, 1);
  }

  setHue(hue: number): void {
    this.backdrop.setHue(hue);
    this.particles.setHue(hue);
  }

  setQuality(quality: Quality): void {
    this.particles.setQuality(quality);
    this.postfx.setQuality(quality);
  }

  start(): void {
    if (this.running) return;
    this.running = true;
    this.clock.start();
    this.loop();
  }

  stop(): void {
    this.running = false;
    cancelAnimationFrame(this.frame);
  }

  dispose(): void {
    this.stop();
    window.removeEventListener('resize', this.resize);
    window.removeEventListener('pointermove', this.onPointerMove);
    this.backdrop.dispose();
    this.particles.dispose();
    this.postfx.dispose();
    this.renderer.dispose();
  }

  private loop = (): void => {
    if (!this.running) return;
    this.frame = requestAnimationFrame(this.loop);

    // Clamp dt so tab-switching cannot fling the animations across the room.
    const dt = Math.min(this.clock.getDelta(), 1 / 20);
    const elapsed = this.clock.elapsedTime;

    this.backdrop.update(this.energy);
    for (const tick of this.ticks) tick(dt, elapsed);
    this.particles.update(dt, this.energy);

    this.driftCamera(dt);
    this.postfx.render(dt);
  };

  /** A few millimetres of parallax. Barely perceptible, entirely alive. */
  private driftCamera(dt: number): void {
    const targetX = this.cameraHome.x + this.pointer.x * 0.055;
    const targetY = this.cameraHome.y + this.pointer.y * 0.03;
    const k = 1 - Math.exp(-dt * 2.2);
    this.camera.position.x += (targetX - this.camera.position.x) * k;
    this.camera.position.y += (targetY - this.camera.position.y) * k;
    this.camera.lookAt(0, 1.2 + this.pointer.y * 0.01, 0);
  }

  private onPointerMove = (event: PointerEvent): void => {
    this.pointer.set(
      (event.clientX / window.innerWidth) * 2 - 1,
      -((event.clientY / window.innerHeight) * 2 - 1),
    );
  };

  private resize = (): void => {
    const width = window.innerWidth;
    const height = window.innerHeight;
    const dpr = Math.min(window.devicePixelRatio, 2);

    this.renderer.setPixelRatio(dpr);
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.postfx.setSize(width, height);
  };

  /** World-space point the cursor is nearest to, for gaze targets. */
  pointerNdc(): THREE.Vector2 {
    return this.pointer;
  }
}

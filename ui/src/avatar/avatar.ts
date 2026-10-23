import * as THREE from 'three';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { VRMLoaderPlugin, VRMUtils, type VRM } from '@pixiv/three-vrm';

import { createSpirit, type SpiritHandle } from './spirit';

/**
 * The body.
 *
 * Owns either a loaded VRM or the procedural spirit, and exposes one flat
 * interface so every animation system stays model-agnostic: write expression
 * weights, read bones, ask for a head position.
 *
 * Expression weights arrive in three separate *layers* — mood, blink, mouth —
 * because they are produced by three independent loops that must not stomp on
 * each other. They are summed and clamped once per frame, here.
 */

export type FaceLayer = 'mood' | 'blink' | 'mouth';
export type BoneName = 'head' | 'neck' | 'chest' | 'spine' | 'hips';

export interface AvatarLoadOptions {
  /** URL of a VRM file, or null to use the procedural spirit. */
  modelUrl: string | null;
  scale: number;
  hue: number;
}

const TARGET_HEIGHT = 1.62;

export class Avatar {
  readonly root = new THREE.Group();
  /** World-space point the eyes should track. */
  readonly lookTarget = new THREE.Object3D();

  private vrm: VRM | null = null;
  private spirit: SpiritHandle | null = null;
  private available = new Set<string>();

  private layers: Record<FaceLayer, Map<string, number>> = {
    mood: new Map(),
    blink: new Map(),
    mouth: new Map(),
  };
  private scratch = new Map<string, number>();

  private headNode: THREE.Object3D | null = null;
  private neckNode: THREE.Object3D | null = null;
  private chestNode: THREE.Object3D | null = null;
  private spineNode: THREE.Object3D | null = null;
  private hipsNode: THREE.Object3D | null = null;

  private headRest = new THREE.Euler();

  /** True when a real VRM is driving the face. */
  get isVrm(): boolean {
    return this.vrm !== null;
  }

  get hasFace(): boolean {
    return this.available.size > 0;
  }

  async load(options: AvatarLoadOptions): Promise<void> {
    this.root.add(this.lookTarget);

    if (options.modelUrl) {
      try {
        await this.loadVrm(options.modelUrl, options.scale);
        return;
      } catch (error) {
        console.warn('[avatar] VRM failed to load, falling back to spirit', error);
      }
    }
    this.loadSpirit(options.hue);
  }

  private async loadVrm(url: string, scale: number): Promise<void> {
    const loader = new GLTFLoader();
    loader.register((parser) => new VRMLoaderPlugin(parser));

    const gltf = await loader.loadAsync(url);
    const vrm = gltf.userData.vrm as VRM | undefined;
    if (!vrm) throw new Error('file is not a VRM');

    // VRM 0.x models face -Z; this rotates them to face the camera.
    const utils = VRMUtils as unknown as Record<string, unknown>;
    if (typeof utils.rotateVRM0 === 'function') {
      (utils.rotateVRM0 as (v: VRM) => void)(vrm);
    }
    if (typeof utils.removeUnnecessaryVertices === 'function') {
      (utils.removeUnnecessaryVertices as (s: THREE.Object3D) => void)(vrm.scene);
    }

    vrm.scene.traverse((object) => {
      object.frustumCulled = false;
    });

    // Fit to a standing height and plant the feet on y = 0.
    const box = new THREE.Box3().setFromObject(vrm.scene);
    const size = box.getSize(new THREE.Vector3());
    const factor = size.y > 0 ? (TARGET_HEIGHT / size.y) * scale : scale;
    vrm.scene.scale.setScalar(factor);

    const fitted = new THREE.Box3().setFromObject(vrm.scene);
    const center = fitted.getCenter(new THREE.Vector3());
    vrm.scene.position.x -= center.x;
    vrm.scene.position.z -= center.z;
    vrm.scene.position.y -= fitted.min.y;

    this.vrm = vrm;
    this.root.add(vrm.scene);

    vrm.lookAt && (vrm.lookAt.target = this.lookTarget);

    const humanoid = vrm.humanoid;
    this.headNode = humanoid?.getNormalizedBoneNode('head') ?? null;
    this.neckNode = humanoid?.getNormalizedBoneNode('neck') ?? null;
    this.chestNode = humanoid?.getNormalizedBoneNode('chest') ?? null;
    this.spineNode = humanoid?.getNormalizedBoneNode('spine') ?? null;
    this.hipsNode = humanoid?.getNormalizedBoneNode('hips') ?? null;
    if (this.headNode) this.headRest.copy(this.headNode.rotation);

    this.relaxPose();

    const expressions = vrm.expressionManager?.expressions ?? [];
    for (const expression of expressions) {
      if (expression.expressionName) this.available.add(expression.expressionName);
    }

    vrm.update(0);
  }

  /**
   * VRM ships in a T-pose; a companion must not. Drop the arms to a relaxed
   * A-pose and keep the resting values so idle animation can breathe on top of
   * them instead of fighting them.
   */
  private relaxPose(): void {
    const humanoid = this.vrm?.humanoid;
    if (!humanoid) return;

    const pose = (bone: string, x: number, y: number, z: number) => {
      const node = humanoid.getNormalizedBoneNode(bone as Parameters<typeof humanoid.getNormalizedBoneNode>[0]);
      if (!node) return;
      node.rotation.set(x, y, z);
    };

    // Left arm points +X, right arm points −X; rotating about Z drops them.
    pose('leftUpperArm', 0.06, 0.0, -1.16);
    pose('rightUpperArm', 0.06, 0.0, 1.16);
    pose('leftLowerArm', 0.02, -0.12, -0.16);
    pose('rightLowerArm', 0.02, 0.12, 0.16);
    pose('leftHand', 0, 0, -0.08);
    pose('rightHand', 0, 0, 0.08);
    pose('head', 0, 0, 0);
  }

  private loadSpirit(hue: number): void {
    this.spirit = createSpirit(hue);
    this.root.add(this.spirit.group);
  }

  /** Replace one animation layer's weights. Values are 0–1. */
  setLayer(layer: FaceLayer, weights: Record<string, number>): void {
    const map = this.layers[layer];
    map.clear();
    for (const [name, value] of Object.entries(weights)) {
      if (value > 0) map.set(name, value);
    }
  }

  clearLayer(layer: FaceLayer): void {
    this.layers[layer].clear();
  }

  bone(name: BoneName): THREE.Object3D | null {
    switch (name) {
      case 'head':
        return this.headNode;
      case 'neck':
        return this.neckNode;
      case 'chest':
        return this.chestNode;
      case 'spine':
        return this.spineNode;
      case 'hips':
        return this.hipsNode;
    }
  }

  /** Rest rotation of the head, so gaze can offset rather than overwrite it. */
  restRotation(): THREE.Euler {
    return this.headRest;
  }

  setEnergy(energy: number): void {
    this.spirit?.setEnergy(energy);
  }

  setHue(hue: number): void {
    this.spirit?.setHue(hue);
  }

  setEmotionTint(hue: number, intensity: number): void {
    this.spirit?.setEmotionTint(hue, intensity);
  }

  update(dt: number, elapsed: number): void {
    if (this.vrm) {
      this.vrm.update(dt);
      this.composeFace();
    }
    this.spirit?.update(dt, elapsed);
  }

  /** Sum the layers, clamp, and write to the expression manager. */
  private composeFace(): void {
    const manager = this.vrm?.expressionManager;
    if (!manager) return;

    this.scratch.clear();
    for (const layer of ['mood', 'blink', 'mouth'] as FaceLayer[]) {
      for (const [name, value] of this.layers[layer]) {
        this.scratch.set(name, (this.scratch.get(name) ?? 0) + value);
      }
    }

    for (const requested of this.scratch.keys()) {
      const actual = this.resolve(requested);
      if (!actual) continue;
      const value = this.scratch.get(requested) ?? 0;
      manager.setValue(actual, THREE.MathUtils.clamp(value, 0, 1));
    }
  }

  /**
   * Models do not always agree on casing — Sendagaya Shino ships a custom
   * group called `Surprised`, not the VRM 1.0 preset `surprised`. Match
   * case-insensitively so a mood is never silently dropped.
   */
  private resolve(name: string): string | null {
    if (this.available.has(name)) return name;
    const lower = name.toLowerCase();
    for (const candidate of this.available) {
      if (candidate.toLowerCase() === lower) return candidate;
    }
    return null;
  }

  dispose(): void {
    if (this.vrm) {
      const utils = VRMUtils as unknown as Record<string, unknown>;
      if (typeof utils.deepDispose === 'function') {
        (utils.deepDispose as (s: THREE.Object3D) => void)(this.vrm.scene);
      }
      this.root.remove(this.vrm.scene);
      this.vrm = null;
    }
    if (this.spirit) {
      this.root.remove(this.spirit.group);
      this.spirit.dispose();
      this.spirit = null;
    }
  }
}

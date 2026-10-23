import * as THREE from 'three';

/**
 * Three-point lighting, tuned for anime-styled characters.
 *
 * A warm key from camera-left, a cool rim from behind, and a broad fill keep
 * the face readable while the rim separates her from the dark backdrop. The rim
 * brightens with speech energy — the single cheapest trick that makes a
 * character feel *present* rather than pre-rendered.
 */
export interface LightingHandle {
  group: THREE.Group;
  setHue(hue: number): void;
  setEnergy(energy: number): void;
  update(dt: number): void;
  dispose(): void;
}

export function createLighting(hue: number): LightingHandle {
  const group = new THREE.Group();

  const hemi = new THREE.HemisphereLight(0x2b2438, 0x0b0810, 0.5);
  group.add(hemi);

  const key = new THREE.DirectionalLight(0xffe9d8, 1.15);
  key.position.set(1.9, 2.6, 2.4);
  group.add(key);

  const rim = new THREE.PointLight(0x9a86ff, 9, 7.5, 2);
  rim.position.set(-1.05, 1.85, -1.45);
  group.add(rim);

  const fill = new THREE.PointLight(0x8fc4ff, 2.2, 6, 2);
  fill.position.set(1.7, 1.15, 1.1);
  group.add(fill);

  // bounce from the "floor" so the underside of the jaw is never pure black
  const bounce = new THREE.PointLight(0xffb9a0, 1.1, 3.4, 2);
  bounce.position.set(0, 0.18, 0.85);
  group.add(bounce);

  const baseRim = rim.intensity;
  const baseFill = fill.intensity;

  const applyHue = (h: number) => {
    rim.color.setHSL(((h + 235) % 360) / 360, 0.72, 0.66);
    fill.color.setHSL(((h + 175) % 360) / 360, 0.55, 0.72);
    key.color.setHSL(((h + 18) % 360) / 360, 0.28, 0.94);
  };
  applyHue(hue);

  let energy = 0;

  return {
    group,
    setHue: applyHue,
    setEnergy(value: number) {
      energy = THREE.MathUtils.clamp(value, 0, 1);
    },
    update(dt: number) {
      const k = 1 - Math.exp(-dt * 6);
      rim.intensity += (baseRim * (1 + energy * 0.85) - rim.intensity) * k;
      fill.intensity += (baseFill * (1 + energy * 0.3) - fill.intensity) * k;
    },
    dispose() {
      group.clear();
    },
  };
}

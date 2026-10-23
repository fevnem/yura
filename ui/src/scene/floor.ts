import * as THREE from 'three';

/**
 * Contact shadow.
 *
 * A horizontal plane at y=0 is effectively invisible from a camera that looks
 * almost level at the character — it collapses to a line. So this is a
 * camera-facing sprite, squashed vertically, sitting at her feet: the shape the
 * eye actually expects under a standing figure, at one draw call.
 */
export interface FloorHandle {
  root: THREE.Group;
  dispose(): void;
}

function radialTexture(inner: string, outer: string): THREE.Texture {
  const size = 256;
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d');
  if (ctx) {
    const g = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
    g.addColorStop(0, inner);
    g.addColorStop(0.55, outer);
    g.addColorStop(1, 'rgba(0,0,0,0)');
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, size, size);
  }
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

export function createFloor(hue: number, focusX: number): FloorHandle {
  const root = new THREE.Group();
  root.position.set(focusX, 0.04, 0);

  // warm light spilling onto the ground
  const pool = new THREE.Sprite(
    new THREE.SpriteMaterial({
      map: radialTexture('rgba(255,214,196,0.30)', 'rgba(255,168,190,0.12)'),
      transparent: true,
      depthWrite: false,
      depthTest: false,
      blending: THREE.AdditiveBlending,
      color: new THREE.Color().setHSL(((hue + 12) % 360) / 360, 0.6, 0.5),
    }),
  );
  pool.scale.set(2.9, 0.85, 1);
  pool.position.set(0, 0.01, 0);
  root.add(pool);

  // the shadow itself: dark, tight, directly under her
  const shadow = new THREE.Sprite(
    new THREE.SpriteMaterial({
      map: radialTexture('rgba(0,0,0,0.92)', 'rgba(0,0,0,0.42)'),
      transparent: true,
      depthWrite: false,
      depthTest: false,
      opacity: 1.0,
    }),
  );
  shadow.scale.set(1.7, 0.46, 1);
  shadow.position.set(0, 0.02, 0);
  root.add(shadow);

  root.renderOrder = -3;

  return {
    root,
    dispose() {
      root.traverse((object) => {
        const sprite = object as THREE.Sprite;
        if (sprite.material) {
          sprite.material.map?.dispose();
          sprite.material.dispose();
        }
      });
    },
  };
}

import * as THREE from 'three';

/**
 * Fallback form: a small spirit of light.
 *
 * Used when no VRM file is present. It is not a placeholder — it is a
 * designed-in alternative so Yura has a body on first launch, before the user
 * has supplied an avatar. Colour follows emotion, brightness follows voice.
 */

export interface SpiritHandle {
  group: THREE.Group;
  setHue(hue: number): void;
  setEmotionTint(hue: number, intensity: number): void;
  setEnergy(energy: number): void;
  update(dt: number, elapsed: number): void;
  dispose(): void;
}

const coreVertex = /* glsl */ `
  varying vec3 vNormal;
  varying vec3 vView;
  varying vec3 vPos;

  void main() {
    vNormal = normalize(normalMatrix * normal);
    vec4 mv = modelViewMatrix * vec4(position, 1.0);
    vView = normalize(-mv.xyz);
    vPos = position;
    gl_Position = projectionMatrix * mv;
  }
`;

const coreFragment = /* glsl */ `
  precision highp float;

  uniform float uTime;
  uniform float uEnergy;
  uniform vec3  uInner;
  uniform vec3  uOuter;

  varying vec3 vNormal;
  varying vec3 vView;
  varying vec3 vPos;

  float hash(vec3 p) {
    return fract(sin(dot(p, vec3(17.13, 41.71, 91.7))) * 43758.5453);
  }

  float noise(vec3 p) {
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float n000 = hash(i);
    float n100 = hash(i + vec3(1.0, 0.0, 0.0));
    float n010 = hash(i + vec3(0.0, 1.0, 0.0));
    float n110 = hash(i + vec3(1.0, 1.0, 0.0));
    float n001 = hash(i + vec3(0.0, 0.0, 1.0));
    float n101 = hash(i + vec3(1.0, 0.0, 1.0));
    float n011 = hash(i + vec3(0.0, 1.0, 1.0));
    float n111 = hash(i + vec3(1.0, 1.0, 1.0));
    return mix(
      mix(mix(n000, n100, f.x), mix(n010, n110, f.x), f.y),
      mix(mix(n001, n101, f.x), mix(n011, n111, f.x), f.y),
      f.z
    );
  }

  void main() {
    float fresnel = pow(1.0 - clamp(dot(vNormal, vView), 0.0, 1.0), 2.2);
    float swirl = noise(vPos * 3.4 + uTime * 0.55);

    vec3 col = mix(uInner, uOuter, fresnel);
    col += uOuter * swirl * 0.28;
    col *= 0.75 + uEnergy * 0.65;

    float alpha = 0.30 + fresnel * 0.58;
    gl_FragColor = vec4(col, clamp(alpha, 0.0, 1.0));
  }
`;

const ringFragment = /* glsl */ `
  precision mediump float;
  uniform vec3 uColor;
  uniform float uOpacity;
  void main() {
    gl_FragColor = vec4(uColor, uOpacity);
  }
`;

function haloTexture(): THREE.Texture {
  const size = 256;
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d');
  if (ctx) {
    const g = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
    g.addColorStop(0, 'rgba(255,255,255,0.95)');
    g.addColorStop(0.18, 'rgba(255,255,255,0.42)');
    g.addColorStop(0.45, 'rgba(255,255,255,0.10)');
    g.addColorStop(1, 'rgba(255,255,255,0)');
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, size, size);
  }
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

/**
 * Soft round dot for the wisp cloud.
 *
 * `PointsMaterial` draws an axis-aligned *square* per point by default, which
 * on a dark scene reads as a grid of dead pixels. A radial alpha map is the
 * whole fix.
 */
function dotTexture(): THREE.Texture {
  const size = 64;
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d');
  if (ctx) {
    const g = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
    g.addColorStop(0, 'rgba(255,255,255,1)');
    g.addColorStop(0.35, 'rgba(255,255,255,0.55)');
    g.addColorStop(1, 'rgba(255,255,255,0)');
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, size, size);
  }
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

export function createSpirit(hue: number): SpiritHandle {
  const group = new THREE.Group();
  group.position.set(0, 1.05, 0);

  const uniforms = {
    uTime: { value: 0 },
    uEnergy: { value: 0 },
    uInner: { value: new THREE.Color() },
    uOuter: { value: new THREE.Color() },
  };

  const coreMaterial = new THREE.ShaderMaterial({
    vertexShader: coreVertex,
    fragmentShader: coreFragment,
    uniforms,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  });

  const core = new THREE.Mesh(new THREE.IcosahedronGeometry(0.34, 5), coreMaterial);
  group.add(core);

  const halo = new THREE.Sprite(
    new THREE.SpriteMaterial({
      map: haloTexture(),
      transparent: true,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
      opacity: 0.4,
    }),
  );
  halo.scale.setScalar(2.4);
  group.add(halo);

  const ringMaterialLow = new THREE.MeshBasicMaterial({
    color: 0xffffff,
    transparent: true,
    opacity: 0.14,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
  });
  const ringMaterialHigh = ringMaterialLow.clone();
  ringMaterialHigh.opacity = 0.09;

  const ringA = new THREE.Mesh(new THREE.TorusGeometry(0.78, 0.0035, 8, 160), ringMaterialLow);
  ringA.rotation.set(Math.PI / 2.4, 0.2, 0);

  const ringB = new THREE.Mesh(new THREE.TorusGeometry(1.02, 0.0025, 8, 160), ringMaterialHigh);
  ringB.rotation.set(Math.PI / 1.75, -0.35, 0.4);

  const ringC = new THREE.Mesh(new THREE.TorusGeometry(0.58, 0.003, 8, 120), ringMaterialHigh);
  ringC.rotation.set(Math.PI / 2, 0.9, 0.1);

  group.add(ringA, ringB, ringC);

  const wisps = 46;
  const wispPositions = new Float32Array(wisps * 3);
  for (let i = 0; i < wisps; i += 1) {
    // Never inside the core: overlapping additive sprites at the centre
    // saturate into a white block.
    const r = 0.82 + Math.random() * 0.6;
    const theta = Math.random() * Math.PI * 2;
    const phi = Math.acos(2 * Math.random() - 1);
    wispPositions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
    wispPositions[i * 3 + 1] = r * Math.cos(phi) * 0.8;
    wispPositions[i * 3 + 2] = r * Math.sin(phi) * Math.sin(theta);
  }
  const wispGeo = new THREE.BufferGeometry();
  wispGeo.setAttribute('position', new THREE.BufferAttribute(wispPositions, 3));
  const wispMat = new THREE.PointsMaterial({
    size: 0.026,
    map: dotTexture(),
    transparent: true,
    alphaTest: 0.02,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    color: 0xffffff,
  });
  const wispCloud = new THREE.Points(wispGeo, wispMat);
  group.add(wispCloud);

  let energy = 0;
  let targetEnergy = 0;
  let tintHue = hue;
  let tintAmount = 0;
  let targetTintAmount = 0;

  const applyHue = (h: number) => {
    uniforms.uInner.value.setHSL(h / 360, 0.6, 0.78);
    uniforms.uOuter.value.setHSL(((h + 40) % 360) / 360, 0.85, 0.62);
    ringMaterialLow.color.setHSL(h / 360, 0.55, 0.75);
    ringMaterialHigh.color.setHSL(((h + 200) % 360) / 360, 0.5, 0.75);
  };
  applyHue(hue);

  return {
    group,
    setHue: applyHue,
    setEmotionTint(h: number, intensity: number) {
      tintHue = h;
      targetTintAmount = THREE.MathUtils.clamp(intensity, 0, 1);
    },
    setEnergy(value: number) {
      targetEnergy = THREE.MathUtils.clamp(value, 0, 1);
    },
    update(dt: number, elapsed: number) {
      uniforms.uTime.value = elapsed;
      energy += (targetEnergy - energy) * Math.min(1, dt * 8);
      uniforms.uEnergy.value = energy;

      tintAmount += (targetTintAmount - tintAmount) * Math.min(1, dt * 2.5);
      const mixed = THREE.MathUtils.lerp(hue, tintHue, tintAmount * 0.85);
      applyHue(mixed);

      const breathe = 1 + Math.sin(elapsed * 0.9) * 0.022 + energy * 0.09;
      core.scale.setScalar(breathe);
      halo.scale.setScalar(2.9 * (1 + energy * 0.16));
      (halo.material as THREE.SpriteMaterial).opacity = 0.45 + energy * 0.3;

      ringA.rotation.z += dt * 0.14;
      ringB.rotation.y += dt * 0.09;
      ringC.rotation.x += dt * 0.17;
      wispCloud.rotation.y += dt * 0.06;
      wispCloud.rotation.x = Math.sin(elapsed * 0.25) * 0.08;

      group.position.y = 1.05 + Math.sin(elapsed * 0.55) * 0.018;
    },
    dispose() {
      coreGeoDispose(core);
      wispGeo.dispose();
      wispMat.dispose();
      coreMaterial.dispose();
      ringMaterialLow.dispose();
      ringMaterialHigh.dispose();
      (halo.material as THREE.SpriteMaterial).map?.dispose();
      (halo.material as THREE.SpriteMaterial).dispose();
      ringA.geometry.dispose();
      ringB.geometry.dispose();
      ringC.geometry.dispose();
    },
  };
}

function coreGeoDispose(mesh: THREE.Mesh): void {
  mesh.geometry.dispose();
}

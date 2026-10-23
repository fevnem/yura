import * as THREE from 'three';

/**
 * Dust motes.
 *
 * A few hundred points drifting upward, brightest where the key light falls.
 * They exist for two reasons: they sell depth in an otherwise flat frame, and
 * their brightness is the cheapest possible "she is speaking" cue — the air
 * reacts before the UI does.
 */
export interface ParticlesHandle {
  points: THREE.Points;
  update(dt: number, energy: number): void;
  setHue(hue: number): void;
  setQuality(quality: 'low' | 'medium' | 'high'): void;
  dispose(): void;
}

const vertex = /* glsl */ `
  attribute vec3  aSeed;
  attribute float aSize;

  uniform float uTime;
  uniform float uEnergy;
  uniform float uPixelRatio;

  varying float vSeed;
  varying float vFade;

  void main() {
    vSeed = aSeed.x;

    vec3 p = position;
    p.y += mod(uTime * 0.055 * (0.35 + aSeed.x) + aSeed.y * 7.0, 5.4) - 2.7;
    p.x += sin(uTime * 0.24 + aSeed.z * 18.0) * 0.14;
    p.z += cos(uTime * 0.19 + aSeed.x * 15.0) * 0.14;

    vec4 mv = modelViewMatrix * vec4(p, 1.0);
    gl_Position = projectionMatrix * mv;

    // Size in pixels, clamped hard. An unclamped perspective scale lets a mote
    // a metre closer than the rest bloom to ~100 px, which saturates into a
    // white block instead of reading as dust.
    float perspective = 1.6 + 2.4 / max(0.6, -mv.z);
    gl_PointSize = clamp(aSize * uPixelRatio * perspective, 1.0, 5.5);

    vFade = smoothstep(0.0, 1.2, -mv.z) * (1.0 - smoothstep(4.5, 6.5, -mv.z));
  }
`;

const fragment = /* glsl */ `
  precision mediump float;

  uniform vec3 uWarm;
  uniform vec3 uCool;
  uniform float uEnergy;

  varying float vSeed;
  varying float vFade;

  void main() {
    float d = length(gl_PointCoord - vec2(0.5));
    // Feathered, never a hard disc: at a few pixels wide a sharp edge reads as
    // a square artefact rather than a mote of dust.
    float alpha = pow(smoothstep(0.5, 0.0, d), 1.7);
    if (alpha <= 0.004) discard;

    vec3 col = mix(uCool, uWarm, vSeed);
    float glow = 0.10 + 0.26 * uEnergy;
    gl_FragColor = vec4(col, alpha * vFade * glow);
  }
`;

const COUNT: Record<'low' | 'medium' | 'high', number> = {
  low: 220,
  medium: 460,
  high: 820,
};

export interface ParticlesOptions {
  hue: number;
  /** Keep motes out of this vertical column: the character's silhouette. */
  avoidX?: number;
  avoidRadius?: number;
}

export function createParticles(hueOrOptions: number | ParticlesOptions): ParticlesHandle {
  const options: ParticlesOptions =
    typeof hueOrOptions === 'number' ? { hue: hueOrOptions } : hueOrOptions;
  const { hue } = options;
  const avoidX = options.avoidX ?? Number.NaN;
  const avoidRadius = options.avoidRadius ?? 0.9;
  const uniforms = {
    uTime: { value: 0 },
    uEnergy: { value: 0 },
    uPixelRatio: { value: Math.min(window.devicePixelRatio, 2) },
    uWarm: { value: new THREE.Color() },
    uCool: { value: new THREE.Color() },
  };

  const geometry = new THREE.BufferGeometry();
  const positions = new Float32Array(COUNT.high * 3);
  const seeds = new Float32Array(COUNT.high * 3);
  const sizes = new Float32Array(COUNT.high);

  for (let i = 0; i < COUNT.high; i += 1) {
    let px = (Math.random() - 0.5) * 7.5;
    // Push motes out of the character's column. Dust drifting across her face
    // reads as a dead pixel — it is the one place the eye is guaranteed to look.
    if (Number.isFinite(avoidX) && Math.abs(px - avoidX) < avoidRadius) {
      const side = px >= avoidX ? 1 : -1;
      px = avoidX + side * (avoidRadius + Math.random() * 2.2);
    }
    positions[i * 3 + 0] = px;
    positions[i * 3 + 1] = (Math.random() - 0.5) * 5.0;
    // Strictly behind the character plane. A mote drifting between the camera
    // and her face reads as a dead pixel, not as depth.
    positions[i * 3 + 2] = -2.7 + Math.random() * 2.1;

    seeds[i * 3 + 0] = Math.random();
    seeds[i * 3 + 1] = Math.random();
    seeds[i * 3 + 2] = Math.random();

    sizes[i] = 0.6 + Math.random() * 2.1;
  }

  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute('aSeed', new THREE.BufferAttribute(seeds, 3));
  geometry.setAttribute('aSize', new THREE.BufferAttribute(sizes, 1));
  geometry.setDrawRange(0, COUNT.high);

  const material = new THREE.ShaderMaterial({
    vertexShader: vertex,
    fragmentShader: fragment,
    uniforms,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  });

  const points = new THREE.Points(geometry, material);
  points.frustumCulled = false;
  points.renderOrder = 5;

  const setHue = (h: number) => {
    uniforms.uWarm.value.setHSL(((h + 12) % 360) / 360, 0.85, 0.72);
    uniforms.uCool.value.setHSL(((h + 210) % 360) / 360, 0.6, 0.66);
  };
  setHue(hue);

  return {
    points,
    update(dt: number, energy: number) {
      uniforms.uTime.value += dt;
      uniforms.uEnergy.value += (energy - uniforms.uEnergy.value) * 0.12;
    },
    setHue,
    setQuality(quality) {
      geometry.setDrawRange(0, COUNT[quality]);
    },
    dispose() {
      geometry.dispose();
      material.dispose();
    },
  };
}

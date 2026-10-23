import * as THREE from 'three';

/**
 * The room behind the character.
 *
 * One fullscreen-ish quad with a layered noise shader: cheap enough for a
 * laptop iGPU, and it gives the scene a slow organic drift that a static
 * gradient can never match. The warm pool behind where the character stands is
 * what makes her read as *lit* rather than pasted on.
 */
export interface BackdropHandle {
  mesh: THREE.Mesh;
  update(energy: number): void;
  setHue(hue: number): void;
  dispose(): void;
}

const vertex = /* glsl */ `
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`;

const fragment = /* glsl */ `
  precision highp float;

  varying vec2 vUv;

  uniform float uTime;
  uniform float uEnergy;
  uniform vec3  uDeep;
  uniform vec3  uMid;
  uniform vec3  uGlow;

  float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
  }

  float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = hash(i);
    float b = hash(i + vec2(1.0, 0.0));
    float c = hash(i + vec2(0.0, 1.0));
    float d = hash(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
  }

  float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    for (int i = 0; i < 5; i++) {
      v += a * noise(p);
      p *= 2.03;
      a *= 0.5;
    }
    return v;
  }

  void main() {
    vec2 uv = vUv;
    vec2 p = uv * vec2(1.62, 1.0);
    float t = uTime * 0.011;

    float n  = fbm(p * 1.7 + vec2(t, -t * 0.55));
    float n2 = fbm(p * 3.4 - vec2(t * 0.7, t * 0.35));

    vec3 col = mix(uDeep, uMid, smoothstep(0.22, 0.88, n));
    col += uMid * 0.18 * n2;

    // pool of warm light behind the character
    float d = distance(vec2(uv.x, uv.y * 0.92), vec2(0.5, 0.58));
    float pool = smoothstep(0.52, 0.0, d);
    col += uGlow * pool * (0.30 + 0.22 * n2 + 0.28 * uEnergy);

    // floor haze
    float floorHaze = smoothstep(0.42, 0.0, uv.y);
    col += uGlow * floorHaze * 0.10;

    // keep the corners genuinely dark
    col *= mix(0.55, 1.0, smoothstep(0.0, 0.55, uv.y));

    gl_FragColor = vec4(col, 1.0);
    #include <colorspace_fragment>
  }
`;

export function createBackdrop(hue: number): BackdropHandle {
  const uniforms = {
    uTime: { value: 0 },
    uEnergy: { value: 0 },
    uDeep: { value: new THREE.Color() },
    uMid: { value: new THREE.Color() },
    uGlow: { value: new THREE.Color() },
  };

  const material = new THREE.ShaderMaterial({
    vertexShader: vertex,
    fragmentShader: fragment,
    uniforms,
    depthWrite: false,
    depthTest: false,
  });

  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(80, 60), material);
  mesh.position.set(0, 4, -12);
  mesh.renderOrder = -100;

  const applyHue = (h: number) => {
    uniforms.uDeep.value.setHSL(h / 360, 0.42, 0.035);
    uniforms.uMid.value.setHSL(((h + 45) % 360) / 360, 0.5, 0.085);
    uniforms.uGlow.value.setHSL(h / 360, 0.62, 0.14);
  };
  applyHue(hue);

  return {
    mesh,
    update(energy: number) {
      uniforms.uTime.value += 0.016;
      uniforms.uEnergy.value += (energy - uniforms.uEnergy.value) * 0.06;
    },
    setHue: applyHue,
    dispose() {
      material.dispose();
      mesh.geometry.dispose();
    },
  };
}

import * as THREE from 'three';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js';
import { ShaderPass } from 'three/examples/jsm/postprocessing/ShaderPass.js';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js';

/**
 * Post-processing stack.
 *
 * Order matters: bloom picks up the highlights the character's rim light
 * creates, then a single grade pass adds vignette, grain and a whisper of
 * chromatic aberration. The grade pass sits *after* bloom so the grain is not
 * itself blurred into mush.
 *
 * Everything here is one draw call per stage — the whole stack costs well under
 * a millisecond at 1080p on an integrated GPU.
 */

const GradeShader = {
  uniforms: {
    tDiffuse: { value: null as THREE.Texture | null },
    uTime: { value: 0 },
    uVignette: { value: 1.0 },
    uGrain: { value: 0.032 },
    uAberration: { value: 0.35 },
    uResolution: { value: new THREE.Vector2(1, 1) },
  },
  vertexShader: /* glsl */ `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }
  `,
  fragmentShader: /* glsl */ `
    precision mediump float;

    uniform sampler2D tDiffuse;
    uniform float uTime;
    uniform float uVignette;
    uniform float uGrain;
    uniform float uAberration;
    uniform vec2  uResolution;

    varying vec2 vUv;

    void main() {
      vec2 uv = vUv;
      vec2 d = uv - 0.5;
      float r2 = dot(d, d);

      float ab = uAberration * (0.5 + r2 * 2.2) * 0.0035;
      vec3 col;
      col.r = texture2D(tDiffuse, uv + d * ab).r;
      col.g = texture2D(tDiffuse, uv).g;
      col.b = texture2D(tDiffuse, uv - d * ab).b;

      float vig = smoothstep(1.05, 0.28, length(d) * 1.28);
      col *= mix(1.0, vig, uVignette);

      // hash grain, animated so it never looks like a static texture
      float g = fract(sin(dot(uv * uResolution + uTime * 37.0, vec2(12.9898, 78.233))) * 43758.5453);
      col += (g - 0.5) * uGrain;

      gl_FragColor = vec4(col, 1.0);
    }
  `,
};

export interface PostFXHandle {
  composer: EffectComposer;
  setSize(width: number, height: number): void;
  setQuality(quality: 'low' | 'medium' | 'high'): void;
  render(dt: number): void;
  dispose(): void;
}

export function createPostFX(
  renderer: THREE.WebGLRenderer,
  scene: THREE.Scene,
  camera: THREE.Camera,
  quality: 'low' | 'medium' | 'high',
): PostFXHandle {
  const size = renderer.getSize(new THREE.Vector2());

  // MSAA on the composer's target: `antialias` on the renderer is ignored once
  // we render through a composer, and jagged rim light is the first thing the
  // eye catches on a dark scene.
  const target =
    quality === 'low'
      ? undefined
      : new THREE.WebGLRenderTarget(size.x, size.y, {
          type: THREE.HalfFloatType,
          samples: quality === 'high' ? 4 : 2,
        });

  const composer = new EffectComposer(renderer, target);
  composer.addPass(new RenderPass(scene, camera));

  const bloom = new UnrealBloomPass(
    new THREE.Vector2(size.x, size.y),
    quality === 'low' ? 0.22 : 0.30,
    0.7,
    0.92,
  );
  bloom.enabled = quality !== 'low';
  composer.addPass(bloom);

  const grade = new ShaderPass(GradeShader);
  grade.uniforms.uResolution.value.set(size.x, size.y);
  composer.addPass(grade);

  return {
    composer,
    setSize(width, height) {
      composer.setSize(width, height);
      bloom.resolution.set(width, height);
      grade.uniforms.uResolution.value.set(width, height);
    },
    setQuality(next) {
      bloom.enabled = next !== 'low';
    },
    render(dt) {
      grade.uniforms.uTime.value += dt;
      composer.render(dt);
    },
    dispose() {
      composer.dispose();
      target?.dispose();
    },
  };
}

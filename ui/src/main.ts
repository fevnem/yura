import './styles/base.css';

import { App } from './app';

/**
 * Entry point.
 *
 * Waits for the platform's synthesiser to publish its voice list before the
 * first paint — on Windows the list is empty for a few hundred milliseconds,
 * and a settings drawer opened in that window would show no voices at all.
 */
async function main(): Promise<void> {
  await warmVoices();

  const app = new App();
  try {
    await app.boot();
  } catch (error) {
    reportFatal(error);
  }
}

function warmVoices(): Promise<void> {
  if (!('speechSynthesis' in window)) return Promise.resolve();
  const synth = window.speechSynthesis;

  return new Promise((resolve) => {
    if (synth.getVoices().length > 0) {
      resolve();
      return;
    }
    const done = () => {
      synth.removeEventListener('voiceschanged', done);
      resolve();
    };
    synth.addEventListener('voiceschanged', done);
    // Never block the UI on a platform quirk.
    window.setTimeout(done, 900);
  });
}

function reportFatal(error: unknown): void {
  console.error('[yura] fatal', error);
  const message = error instanceof Error ? error.message : String(error);

  const banner = document.createElement('div');
  banner.style.cssText = [
    'position:fixed',
    'inset:auto 24px 24px 24px',
    'padding:16px 18px',
    'border-radius:14px',
    'background:rgba(24,10,14,0.92)',
    'border:1px solid rgba(255,120,120,0.35)',
    'color:#ffd9dc',
    'font:13px/1.5 ui-sans-serif,system-ui,sans-serif',
    'z-index:999',
  ].join(';');
  banner.textContent = `Yura could not start: ${message}`;
  document.body.append(banner);
}

void main();

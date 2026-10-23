/**
 * The only module that knows about Tauri.
 *
 * Everything else imports `invoke`/`on` from here. When the app is opened in a
 * plain browser (`npm run dev` without Tauri) the calls are routed to
 * `demo.ts`, so the renderer — the part that takes the longest to get right —
 * can be developed and screenshotted without a Windows build.
 */

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import { demoInvoke, demoListen } from './demo';

export const IS_TAURI: boolean =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!IS_TAURI) return demoInvoke<T>(command, args);
  return tauriInvoke<T>(command, args);
}

export async function on<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<() => void> {
  if (!IS_TAURI) return demoListen<T>(event, handler);
  const unlisten = await tauriListen<T>(event, (e) => handler(e.payload));
  return unlisten;
}

export type WindowAction = 'minimize' | 'toggle-maximize' | 'close';

export async function windowAction(action: WindowAction): Promise<void> {
  if (!IS_TAURI) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const win = getCurrentWindow();
  switch (action) {
    case 'minimize':
      await win.minimize();
      break;
    case 'toggle-maximize':
      await win.toggleMaximize();
      break;
    case 'close':
      await win.close();
      break;
  }
}

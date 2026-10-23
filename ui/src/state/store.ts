/**
 * A deliberately tiny observable store.
 *
 * The app has one screen and one owner of truth; a framework would add more
 * weight than it removes. Components subscribe to the key they care about and
 * re-render that slice only.
 */

import type { Bootstrap, Emotion, Settings } from '../bridge/types';

export interface AppState {
  bootstrap: Bootstrap | null;
  settings: Settings | null;
  configured: boolean;
  /** A reply is being generated. */
  streaming: boolean;
  /** Audio is coming out of the speakers. */
  speaking: boolean;
  /** Microphone is open. */
  listening: boolean;
  emotion: Emotion;
  settingsOpen: boolean;
  /** Live text of the reply currently being generated. */
  draft: string;
  error: string | null;
}

type Key = keyof AppState;
type Listener<K extends Key> = (value: AppState[K]) => void;

const initial: AppState = {
  bootstrap: null,
  settings: null,
  configured: false,
  streaming: false,
  speaking: false,
  listening: false,
  emotion: 'neutral',
  settingsOpen: false,
  draft: '',
  error: null,
};

class Store {
  private state: AppState = { ...initial };
  private listeners = new Map<Key, Set<Listener<Key>>>();

  get<K extends Key>(key: K): AppState[K] {
    return this.state[key];
  }

  snapshot(): AppState {
    return this.state;
  }

  set<K extends Key>(key: K, value: AppState[K]): void {
    if (Object.is(this.state[key], value)) return;
    this.state = { ...this.state, [key]: value };
    this.listeners.get(key)?.forEach((fn) => fn(value));
  }

  patch(values: Partial<AppState>): void {
    (Object.keys(values) as Key[]).forEach((key) => {
      this.set(key, values[key] as AppState[typeof key]);
    });
  }

  subscribe<K extends Key>(key: K, fn: Listener<K>): () => void {
    const set = this.listeners.get(key) ?? new Set();
    set.add(fn as Listener<Key>);
    this.listeners.set(key, set);
    return () => set.delete(fn as Listener<Key>);
  }
}

export const store = new Store();

/** Apply the theme and accent hue to the CSS palette. */
export function applyPalette(settings: Settings): void {
  const hue = ((settings.ui.accent_hue % 360) + 360) % 360;
  document.documentElement.style.setProperty('--hue', String(hue));
  document.documentElement.dataset.theme = settings.ui.theme || 'midnight';
  document.documentElement.dataset.reduceMotion = String(settings.ui.reduce_motion);
}

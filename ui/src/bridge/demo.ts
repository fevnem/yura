/**
 * Browser-only demo backend.
 *
 * Mirrors the Rust defaults so the renderer can be built, reviewed and
 * screenshotted in a normal browser. It is dead code inside the shipped app:
 * `IS_TAURI` routes every call to the real commands.
 */

import type {
  AvatarSpec,
  Bootstrap,
  ChatDeltaEvent,
  ChatDoneEvent,
  ChatStartEvent,
  Emotion,
  HistoryEntry,
  Chat,
  ChatSummary,
  PresetDto,
  Settings,
  Skill,
  Utterance,
} from './types';

type Handler = (payload: unknown) => void;

const subscribers = new Map<string, Set<Handler>>();
const timers = new Set<number>();

function emit(event: string, payload: unknown): void {
  subscribers.get(event)?.forEach((fn) => fn(payload));
}

export function demoListen<T>(event: string, handler: (p: T) => void): () => void {
  const set = subscribers.get(event) ?? new Set<Handler>();
  set.add(handler as Handler);
  subscribers.set(event, set);
  return () => set.delete(handler as Handler);
}

const PRESETS: PresetDto[] = [
  { id: 'openai', label: 'OpenAI', kind: 'openai_compatible', base_url: 'https://api.openai.com/v1', default_model: 'gpt-4o-mini', requires_key: true },
  { id: 'deepseek', label: 'DeepSeek', kind: 'openai_compatible', base_url: 'https://api.deepseek.com/v1', default_model: 'deepseek-chat', requires_key: true },
  { id: 'openrouter', label: 'OpenRouter', kind: 'openai_compatible', base_url: 'https://openrouter.ai/api/v1', default_model: 'anthropic/claude-3.5-sonnet', requires_key: true },
  { id: 'anthropic', label: 'Anthropic', kind: 'anthropic', base_url: 'https://api.anthropic.com/v1', default_model: 'claude-3-5-sonnet-latest', requires_key: true },
  { id: 'ollama', label: 'Ollama (local)', kind: 'openai_compatible', base_url: 'http://localhost:11434/v1', default_model: 'qwen2.5:7b', requires_key: false },
  { id: 'lmstudio', label: 'LM Studio (local)', kind: 'openai_compatible', base_url: 'http://localhost:1234/v1', default_model: 'local-model', requires_key: false },
  { id: 'custom', label: 'Custom / self-hosted', kind: 'openai_compatible', base_url: '', default_model: '', requires_key: false },
];

/** Same table as `yura-avatar::expression::EXPRESSIONS`. */
const AVATAR: AvatarSpec = {
  version: 1,
  expressions: [
    { emotion: 'neutral', weights: [] },
    { emotion: 'joy', weights: [['happy', 0.85], ['relaxed', 0.25], ['blink', 0.05]] },
    { emotion: 'amused', weights: [['happy', 0.55], ['relaxed', 0.35], ['aa', 0.12]] },
    { emotion: 'sad', weights: [['sad', 0.8], ['blink', 0.15]] },
    { emotion: 'angry', weights: [['angry', 0.75]] },
    { emotion: 'surprised', weights: [['surprised', 0.9], ['oh', 0.15]] },
    { emotion: 'shy', weights: [['happy', 0.3], ['relaxed', 0.3], ['blink', 0.2]] },
    { emotion: 'thinking', weights: [['relaxed', 0.2], ['blinkLeft', 0.1]] },
    { emotion: 'sleepy', weights: [['relaxed', 0.55], ['blink', 0.45], ['blinkLeft', 0.15]] },
    { emotion: 'affection', weights: [['happy', 0.45], ['relaxed', 0.4], ['blink', 0.25]] },
  ],
  idle: {
    breath_period: 4.2,
    breath_amplitude: 0.012,
    blink_interval: 4.6,
    blink_jitter: 3.2,
    gaze_wander: 0.09,
    gaze_hold: 1.8,
    head_follow: 0.28,
  },
};

const DEFAULT_SETTINGS: Settings = {
  version: 1,
  persona: {
    name: 'Yura',
    tagline: 'a quiet light that stays',
    system_prompt: '',
    speech_style: '',
  },
  llm: {
    provider_id: 'ollama',
    base_url: 'http://localhost:11434/v1',
    api_key: '',
    model: 'qwen2.5:7b',
    temperature: 0.85,
    top_p: 0.95,
    max_tokens: 512,
    context_turns: 24,
    context_window: 32768,
  },
  voice: {
    enabled: true,
    voice_uri: '',
    preferred_locale: 'en',
    rate: 1.04,
    pitch: 1.18,
    volume: 0.95,
    stream_speech: true,
    stt_enabled: true,
  },
  avatar: {
    model_file: '',
    scale: 1,
    gaze_follow: true,
    auto_blink: true,
    idle_motion: true,
    lipsync_gain: 1,
    quality: 'high',
  },
  ui: {
    always_on_top: false,
    theme: 'midnight',
    window_opacity: 0.98,
    accent_hue: 340,
    show_transcript: true,
    reduce_motion: false,
  },
};

const demoChats: Chat[] = [
  {
    id: 'c1',
    title: 'Kinetics before the exam',
    pinned: true,
    created: Date.now() - 9e5,
    updated: Date.now() - 8e5,
    messages: [
      { role: 'assistant', content: "You're back. I kept the light on.", ts: Date.now() - 9e5 },
      { role: 'user', content: 'long day', ts: Date.now() - 8.6e5 },
      { role: 'assistant', content: 'Then sit down. You do not have to explain it to me yet.', ts: Date.now() - 8.5e5 },
    ],
  },
  {
    id: 'c2',
    title: 'Paper: attention is all you need',
    pinned: false,
    created: Date.now() - 4e6,
    updated: Date.now() - 3.9e6,
    messages: [{ role: 'user', content: 'summarise the methods section', ts: Date.now() - 3.9e6 }],
  },
];

function demoSummaries(): ChatSummary[] {
  return demoChats
    .map((c) => ({
      id: c.id,
      title: c.title,
      pinned: c.pinned,
      updated: c.updated,
      turns: c.messages.length,
      preview: c.messages.find((m) => m.role === 'user')?.content ?? '',
    }))
    .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updated - a.updated);
}

const demoSkills: Skill[] = [
  {
    id: 'paper-digest',
    name: 'Paper digest',
    description: 'Summarise a paper as claims, method, evidence, limits.',
    enabled: true,
    body: 'When the user shares a paper, answer in this order:\n1. The claim in one sentence.\n2. Method, in three bullets.\n3. What the evidence actually supports.\n4. The strongest objection.\nNever pad with praise.',
  },
];

const DEMO_HISTORY: HistoryEntry[] = [
  { role: 'assistant', content: "You're back. I kept the light on.", ts: Date.now() - 900_000 },
  { role: 'user', content: 'long day', ts: Date.now() - 860_000 },
  { role: 'assistant', content: 'Then sit down. You do not have to explain it to me yet.', ts: Date.now() - 850_000 },
];

const DEMO_REPLIES: Utterance[][] = [
  [
    { emotion: 'joy', text: 'There you are.' },
    { emotion: 'amused', text: "I was starting to narrate the dust motes for company." },
    { emotion: 'affection', text: 'Tell me something true about your day.' },
  ],
  [
    { emotion: 'thinking', text: "Hm. That's a heavier answer than the question deserved." },
    { emotion: 'shy', text: 'I like that you told me anyway.' },
  ],
];

let demoReply = 0;
let settings: Settings = structuredClone(DEFAULT_SETTINGS);

function streamDemoReply(chatId?: string): void {
  const chat = demoChats.find((c) => c.id === chatId) ?? demoChats[0];
  const beats = DEMO_REPLIES[demoReply % DEMO_REPLIES.length];
  demoReply += 1;

  let delay = 220;
  emit('chat:start', { text: '' } satisfies Partial<ChatStartEvent>);

  beats.forEach((beat) => {
    const text = `${beat.text} `;
    let visible = '';
    for (const ch of text) {
      timers.add(
        window.setTimeout(() => {
          visible += ch;
          emit('chat:delta', { delta: ch, visible } satisfies ChatDeltaEvent);
        }, delay),
      );
      delay += 22;
    }
    timers.add(window.setTimeout(() => emit('chat:utterance', beat), delay));
    delay += 180;
  });

  timers.add(
    window.setTimeout(() => {
      const text = beats.map((b) => b.text).join(' ');
      emit('chat:done', {
        text,
        reply: { utterances: beats },
        context_tokens: 1840,
        context_window: 32768,
      } satisfies ChatDoneEvent);
    }, delay + 200),
  );
}

export async function demoInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  switch (command) {
    case 'bootstrap':
      return {
        settings,
        presets: PRESETS,
        avatar: AVATAR,
        history: DEMO_HISTORY,
        version: '0.1.0-demo',
        data_dir: 'browser demo',
        configured: true,
        skills: demoSkills,
        skills_dir: 'browser demo/skills',
        chats: demoSummaries(),
      } satisfies Bootstrap as T;

    case 'save_settings':
      settings = { ...(args?.settings as Settings) };
      emit('settings:changed', { settings });
      return settings as T;

    case 'apply_provider_preset': {
      const preset = PRESETS.find((p) => p.id === args?.id);
      if (preset) {
        settings = {
          ...settings,
          llm: {
            ...settings.llm,
            provider_id: preset.id,
            base_url: preset.base_url,
            model: preset.default_model,
            api_key: '',
          },
        };
      }
      emit('settings:changed', { settings });
      return settings as T;
    }

    case 'send_message':
      streamDemoReply(args?.chatId as string | undefined);
      return undefined as T;

    case 'cancel_generation':
      timers.forEach((t) => window.clearTimeout(t));
      timers.clear();
      emit('chat:cancelled', { text: '', reply: { utterances: [] } });
      return undefined as T;

    case 'list_chats':
      return demoSummaries() as T;

    case 'new_chat': {
      const chat: Chat = {
        id: `c${Date.now().toString(16)}`,
        title: 'New conversation',
        pinned: false,
        created: Date.now(),
        updated: Date.now(),
        messages: [],
      };
      demoChats.unshift(chat);
      return chat as T;
    }

    case 'load_chat':
      return (demoChats.find((c) => c.id === args?.id) ?? null) as T;

    case 'rename_chat': {
      const chat = demoChats.find((c) => c.id === args?.id);
      if (chat) chat.title = String(args?.title ?? chat.title);
      return undefined as T;
    }

    case 'pin_chat': {
      const chat = demoChats.find((c) => c.id === args?.id);
      if (chat) chat.pinned = Boolean(args?.pinned);
      return undefined as T;
    }

    case 'delete_chat': {
      const at = demoChats.findIndex((c) => c.id === args?.id);
      if (at >= 0) demoChats.splice(at, 1);
      return undefined as T;
    }

    case 'list_skills':
      return demoSkills as T;

    case 'save_skill': {
      const incoming = args?.skill as Skill;
      const id = incoming.id || incoming.name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
      const next: Skill = { ...incoming, id };
      const at = demoSkills.findIndex((s) => s.id === id);
      if (at >= 0) demoSkills[at] = next;
      else demoSkills.push(next);
      return next as T;
    }

    case 'delete_skill': {
      const id = args?.id as string;
      const at = demoSkills.findIndex((s) => s.id === id);
      if (at >= 0) demoSkills.splice(at, 1);
      return undefined as T;
    }

    case 'clear_history':
      DEMO_HISTORY.length = 0;
      return undefined as T;

    case 'recent_history':
      return DEMO_HISTORY as T;

    case 'set_always_on_top':
      return undefined as T;

    default:
      throw new Error(`demo backend: unknown command '${command}'`);
  }
}

export const DEMO_EMOTIONS: Emotion[] = [
  'neutral',
  'joy',
  'amused',
  'sad',
  'angry',
  'surprised',
  'shy',
  'thinking',
  'sleepy',
  'affection',
];

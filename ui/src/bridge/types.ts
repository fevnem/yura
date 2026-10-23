/**
 * Wire types. These mirror the Rust structs in `yura-core` and `yura-avatar`
 * field for field — serde's default naming is snake_case, so the names here are
 * deliberately not camelCased. One shape, two languages, no adapter layer.
 */

export type Emotion =
  | 'neutral'
  | 'joy'
  | 'amused'
  | 'sad'
  | 'angry'
  | 'surprised'
  | 'shy'
  | 'thinking'
  | 'sleepy'
  | 'affection';

export const EMOTIONS: Emotion[] = [
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

export interface Utterance {
  emotion: Emotion;
  text: string;
}

export interface Reply {
  utterances: Utterance[];
}

export interface Persona {
  name: string;
  tagline: string;
  system_prompt: string;
  speech_style: string;
}

export interface LlmSettings {
  provider_id: string;
  base_url: string;
  api_key: string;
  model: string;
  temperature: number;
  top_p: number;
  max_tokens: number;
  context_turns: number;
  context_window: number;
}

export interface Skill {
  id: string;
  name: string;
  description: string;
  body: string;
  enabled: boolean;
}

export interface VoiceSettings {
  enabled: boolean;
  voice_uri: string;
  preferred_locale: string;
  rate: number;
  pitch: number;
  volume: number;
  stream_speech: boolean;
  stt_enabled: boolean;
}

export type Quality = 'low' | 'medium' | 'high';

export interface AvatarSettings {
  model_file: string;
  scale: number;
  gaze_follow: boolean;
  auto_blink: boolean;
  idle_motion: boolean;
  lipsync_gain: number;
  quality: Quality;
}

export interface UiSettings {
  always_on_top: boolean;
  theme: string;
  window_opacity: number;
  accent_hue: number;
  show_transcript: boolean;
  reduce_motion: boolean;
}

export interface Settings {
  version: number;
  persona: Persona;
  llm: LlmSettings;
  voice: VoiceSettings;
  avatar: AvatarSettings;
  ui: UiSettings;
}

export interface PresetDto {
  id: string;
  label: string;
  kind: 'openai_compatible' | 'anthropic';
  base_url: string;
  default_model: string;
  requires_key: boolean;
}

export interface ExpressionDef {
  emotion: Emotion;
  weights: [string, number][];
}

export interface IdleParams {
  breath_period: number;
  breath_amplitude: number;
  blink_interval: number;
  blink_jitter: number;
  gaze_wander: number;
  gaze_hold: number;
  head_follow: number;
}

export interface AvatarSpec {
  version: number;
  expressions: ExpressionDef[];
  idle: IdleParams;
}

export interface HistoryEntry {
  role: 'user' | 'assistant' | 'system';
  content: string;
  ts: number;
}

export interface ChatTurn {
  role: 'user' | 'assistant' | 'system';
  content: string;
  ts: number;
}

export interface Chat {
  id: string;
  title: string;
  pinned: boolean;
  created: number;
  updated: number;
  messages: ChatTurn[];
}

export interface ChatSummary {
  id: string;
  title: string;
  pinned: boolean;
  updated: number;
  turns: number;
  preview: string;
}

export interface Bootstrap {
  settings: Settings;
  presets: PresetDto[];
  avatar: AvatarSpec;
  history: HistoryEntry[];
  version: string;
  data_dir: string;
  configured: boolean;
  skills: Skill[];
  skills_dir: string;
  chats: ChatSummary[];
}

/* ── event payloads ─────────────────────────────────────────── */

export interface ChatStartEvent {
  text: string;
}

export interface ChatDeltaEvent {
  delta: string;
  visible: string;
}

export interface ChatDoneEvent {
  text: string;
  reply: Reply;
  context_tokens: number;
  context_window: number;
}

export interface ChatThinkingEvent {
  delta: string;
}

export interface ChatCancelEvent {
  text: string;
  reply: Reply;
}

export interface ChatErrorEvent {
  message: string;
}

export interface SettingsChangedEvent {
  settings: Settings;
}

import { invoke, on, windowAction } from './bridge/ipc';
import type {
  Bootstrap,
  Chat,
  ChatSummary,
  ChatCancelEvent,
  ChatDeltaEvent,
  ChatDoneEvent,
  ChatErrorEvent,
  ChatStartEvent,
  Emotion,
  Settings,
  ChatThinkingEvent,
  SettingsChangedEvent,
  Skill,
  Utterance,
} from './bridge/types';
import { Avatar } from './avatar/avatar';
import { EmotionMixer } from './avatar/emotion';
import { GazeController } from './avatar/gaze';
import { IdleAnimator } from './avatar/idle';
import { LipSync } from './avatar/lipsync';
import { Composer } from './chat/composer';
import { ChatSidebar } from './chat/sidebar';
import { Welcome } from './chat/welcome';
import { Transcript } from './chat/transcript';
import { createLighting, type LightingHandle } from './scene/lights';
import { Stage } from './scene/stage';
import { SettingsPanel } from './settings/panel';
import { Telemetry } from './telemetry/hud';
import { applyPalette, store } from './state/store';
import { MicrophoneMeter, PlatformTranscriber } from './voice/mic';
import { SpeechController } from './voice/speech';

/**
 * Composition root.
 *
 * Owns the objects, wires the events, and is the only place that knows the
 * whole shape of the app. Subsystems stay ignorant of each other: the avatar
 * does not know about chat, the transcript does not know about audio, and the
 * stage does not know either exists.
 */
export class App {
  private stage!: Stage;
  private lighting!: LightingHandle;
  private avatar!: Avatar;
  private emotion!: EmotionMixer;
  private idle!: IdleAnimator;
  private gaze!: GazeController;
  private lipsync!: LipSync;
  private speech = new SpeechController();
  private meter = new MicrophoneMeter();
  private transcriber = new PlatformTranscriber();
  private transcript!: Transcript;
  private composer!: Composer;
  private panel!: SettingsPanel;
  private bootstrap!: Bootstrap;
  private telemetry!: Telemetry;
  private skills: Skill[] = [];
  private sidebar!: ChatSidebar;
  private welcome!: Welcome;
  private chats: ChatSummary[] = [];
  private activeChatId: string | null = null;
  private sidebarOpen = true;
  /** Horizontal resting position of the character in world units. */
  private characterX = -0.62;

  private beatQueue: Utterance[] = [];
  private draining = false;
  private beatTimer = 0;
  private models: string[] = [];
  private currentModel = '';

  async boot(): Promise<void> {
    this.bootstrap = await invoke<Bootstrap>('bootstrap');
    store.patch({
      bootstrap: this.bootstrap,
      settings: this.bootstrap.settings,
      configured: this.bootstrap.configured,
    });
    applyPalette(this.bootstrap.settings);

    const canvas = document.querySelector<HTMLCanvasElement>('#stage');
    if (!canvas) throw new Error('#stage canvas is missing');

    const settings = this.bootstrap.settings;

    // Composition: the character owns the left of the frame so the transcript
    // has a real column on the right instead of floating over dead space.
    // Lights and effects move with her, or the rim lands on thin air.
    const characterOffsetX = this.characterX;

    this.stage = new Stage(canvas, {
      hue: settings.ui.accent_hue,
      quality: settings.avatar.quality,
      focusX: characterOffsetX,
    });

    this.lighting = createLighting(settings.ui.accent_hue);
    this.lighting.group.position.x = characterOffsetX;
    this.stage.scene.add(this.lighting.group);

    this.avatar = new Avatar();
    this.avatar.root.position.x = characterOffsetX;
    this.stage.scene.add(this.avatar.root);

    this.models = await this.loadModelManifest();
    await this.loadCharacter(settings);

    this.emotion = new EmotionMixer(this.bootstrap.avatar);
    this.emotion.set('neutral', 0);

    this.idle = new IdleAnimator(this.avatar, this.bootstrap.avatar.idle, {
      motion: settings.avatar.idle_motion,
      blink: settings.avatar.auto_blink,
    });
    this.gaze = new GazeController(
      this.avatar,
      this.stage.camera,
      this.bootstrap.avatar.idle,
      settings.avatar.gaze_follow,
    );
    this.lipsync = new LipSync(this.avatar, settings.avatar.lipsync_gain);

    this.speech.configure({
      enabled: settings.voice.enabled,
      voiceUri: settings.voice.voice_uri,
      locale: settings.voice.preferred_locale,
      rate: settings.voice.rate,
      pitch: settings.voice.pitch,
      volume: settings.voice.volume,
    });

    this.mountUi();
    await this.wireEvents();
    await this.bootChats();
    this.registerTicks();
    this.stage.start();

    if (!this.bootstrap.configured) this.showSetup(true);
  }

  /* ── character ─────────────────────────────────────────────── */

  private async loadModelManifest(): Promise<string[]> {
    try {
      const response = await fetch('/assets/models/manifest.json', { cache: 'no-store' });
      if (!response.ok) return [];
      const data = (await response.json()) as { models?: { file: string }[] };
      return (data.models ?? []).map((m) => m.file);
    } catch {
      return [];
    }
  }

  private async loadCharacter(settings: Settings): Promise<void> {
    const wanted = settings.avatar.model_file || this.models[0] || '';
    this.currentModel = wanted;
    const url = wanted ? `/assets/models/${encodeURIComponent(wanted)}` : null;

    await this.avatar.load({
      modelUrl: url,
      scale: settings.avatar.scale,
      hue: settings.ui.accent_hue,
    });

    if (!this.avatar.isVrm) {
      console.info('[yura] no VRM loaded — using the spirit form');
    }
  }

  /* ── ui ────────────────────────────────────────────────────── */

  private mountUi(): void {
    const transcriptEl = must<HTMLElement>('#transcript');
    const form = must<HTMLFormElement>('#composer');
    const input = must<HTMLTextAreaElement>('#input');
    const send = must<HTMLButtonElement>('#btn-send');
    const mic = must<HTMLButtonElement>('#btn-mic');

    this.skills = this.bootstrap.skills ?? [];

    this.telemetry = new Telemetry(
      must<HTMLElement>('#telem-ctx'),
      must<HTMLElement>('#telem-ctx .telem-bar b'),
      must<HTMLElement>('#telem-ctx .telem-text'),
      must<HTMLElement>('#telem-tps'),
      must<HTMLElement>('#telem-tps .telem-text'),
      must<HTMLElement>('#thinking'),
      must<HTMLElement>('#thinking .thinking-text'),
    );
    this.telemetry.setContext(0, this.bootstrap.settings.llm.context_window);

    this.transcript = new Transcript(transcriptEl);

    this.welcome = new Welcome(must<HTMLElement>('#welcome'), (prompt) => {
      this.composer.setValue(prompt);
    });

    // The landing screen owns the left half, so she steps aside while it is up.
    this.setLanding(true);
    this.transcript.setVisible(this.bootstrap.settings.ui.show_transcript);

    this.composer = new Composer(form, input, send, mic, {
      onSend: (text) => void this.send(text),
      onStop: () => void this.stop(),
      onMicStart: () => void this.startListening(),
      onMicEnd: () => this.stopListening(),
    });

    this.panel = new SettingsPanel(must<HTMLElement>('#settings'), {
      getSettings: () => store.get('settings') ?? this.bootstrap.settings,
      getPresets: () => this.bootstrap.presets,
      getVoices: () => this.speech.rankedVoices(),
      getModels: () => this.models,
      onSave: (settings) => this.saveSettings(settings),
      onClearHistory: async () => {
        await invoke('clear_history');
        this.transcript.clear();
      },
      onTestVoice: (text, settings) => this.speakPreview(text, settings),
      getSkills: () => this.skills,
      onToggleSkill: async (id, enabled) => {
        const skill = this.skills.find((s) => s.id === id);
        if (!skill) return;
        const saved = await invoke<Skill>('save_skill', { skill: { ...skill, enabled } });
        this.skills = this.skills.map((s) => (s.id === id ? saved : s));
      },
      onSaveSkill: async (skill) => {
        const saved = await invoke<Skill>('save_skill', { skill });
        this.skills = [...this.skills.filter((s) => s.id !== saved.id), saved];
        return saved;
      },
      onDeleteSkill: async (id) => {
        await invoke('delete_skill', { id });
        this.skills = this.skills.filter((s) => s.id !== id);
      },
      onImportSkill: async (file) => {
        const text = await file.text();
        const stem = file.name.replace(/\.(md|markdown|txt)$/i, '');
        const saved = await invoke<Skill>('save_skill', {
          skill: { id: stem, name: stem.replace(/[-_]+/g, ' '), description: '', body: text, enabled: true },
        });
        this.skills = [...this.skills.filter((s) => s.id !== saved.id), saved];
      },
      onClose: () => undefined,
    });

    must<HTMLButtonElement>('#btn-settings').addEventListener('click', () => {
      this.panel.toggle();
      must<HTMLButtonElement>('#btn-settings').classList.toggle('active', this.panel.isOpen);
    });
    must<HTMLButtonElement>('#btn-min').addEventListener('click', () => void windowAction('minimize'));
    must<HTMLButtonElement>('#btn-max').addEventListener('click', () => void windowAction('toggle-maximize'));

    // Double-clicking the drag region should maximise, like any real title bar.
    const titlebar = document.querySelector('.titlebar');
    titlebar?.addEventListener('dblclick', (event) => {
      const target = event.target as HTMLElement;
      if (target.closest('.win-actions')) return;
      void windowAction('toggle-maximize');
    });
    must<HTMLButtonElement>('#btn-close').addEventListener('click', () => void windowAction('close'));
    must<HTMLButtonElement>('#btn-pin').addEventListener('click', () => void this.togglePin());
    must<HTMLButtonElement>('#setup-open').addEventListener('click', () => {
      this.showSetup(false);
      this.panel.show('model');
    });

    // Esc leaves the settings page, never the app.
    window.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && this.panel.isOpen) {
        event.preventDefault();
        this.panel.close();
        must<HTMLButtonElement>('#btn-settings').classList.remove('active');
      }
    });

    this.sidebar = new ChatSidebar(must<HTMLElement>('#chats'), {
      onSelect: (id) => void this.selectChat(id),
      onNew: () => void this.newChat(),
      onRename: (id, title) => {
        void invoke('rename_chat', { id, title }).then(() => this.refreshChats());
      },
      onPin: (id, pinned) => {
        void invoke('pin_chat', { id, pinned }).then(() => this.refreshChats());
      },
      onDelete: (id) => {
        void invoke('delete_chat', { id }).then(async () => {
          await this.refreshChats();
          if (this.activeChatId === id) {
            const next = this.chats[0];
            if (next) await this.selectChat(next.id);
            else await this.newChat();
          }
        });
      },
    });

    must<HTMLButtonElement>('#btn-sidebar').addEventListener('click', () => {
      this.sidebarOpen = !this.sidebarOpen;
      must<HTMLElement>('#app').dataset.sidebar = this.sidebarOpen ? 'open' : 'closed';
      must<HTMLButtonElement>('#btn-sidebar').classList.toggle('active', this.sidebarOpen);
    });

    if (!this.transcriber.available) {
      console.info('[yura] speech recognition unavailable in this engine; text input only');
    }

    this.composer.focus();
  }

  private registerTicks(): void {
    this.stage.addTick((dt, elapsed) => {
      this.avatar.setLayer('mood', this.emotion.update(dt));
      this.idle.update(dt, elapsed);
      this.gaze.setPointer(this.stage.pointerNdc());
      this.gaze.update(dt, elapsed);
      this.lipsync.update(dt, elapsed);
      this.avatar.update(dt, elapsed);
      this.lighting.update(dt);
      this.avatar.setEmotionTint(this.emotion.hue(), this.emotion.strength);
    });
  }

  /** Landing visible: she steps right, the meters get out of the way. */
  private setLanding(visible: boolean): void {
    if (this.welcome) {
      if (visible) this.welcome.show();
      else this.welcome.hide();
    }
    this.avatar.root.position.x = this.characterX + (visible ? 0.86 : 0);
    const telemetry = document.querySelector<HTMLElement>('.telemetry');
    if (telemetry) telemetry.hidden = visible;
  }

  /* ── conversations ─────────────────────────────────────────── */

  private async bootChats(): Promise<void> {
    this.chats = (await invoke<ChatSummary[]>('list_chats')) ?? [];
    if (this.chats.length === 0) {
      const chat = await invoke<Chat>('new_chat');
      this.chats = [{ id: chat.id, title: chat.title, pinned: false, updated: chat.updated, turns: 0, preview: '' }];
      await this.selectChat(chat.id);
      return;
    }
    await this.selectChat(this.chats[0].id);
  }

  private async refreshChats(): Promise<void> {
    this.chats = (await invoke<ChatSummary[]>('list_chats')) ?? [];
    this.sidebar?.setChats(this.chats, this.activeChatId);
  }

  private async selectChat(id: string): Promise<void> {
    const chat = await invoke<Chat | null>('load_chat', { id });
    this.activeChatId = id;
    const turns = chat?.messages ?? [];
    this.transcript.load(turns.map((m) => ({ role: m.role, content: m.content })));
    this.sidebar?.setChats(this.chats, id);
    this.setLanding(turns.length === 0);
  }

  private async newChat(): Promise<void> {
    const chat = await invoke<Chat>('new_chat');
    await this.refreshChats();
    this.activeChatId = chat.id;
    this.transcript.load([]);
    this.sidebar?.setChats(this.chats, chat.id);
    this.setLanding(true);
    this.composer.focus();
  }

  /* ── chat ──────────────────────────────────────────────────── */

  private async wireEvents(): Promise<void> {
    await on<ChatStartEvent>('chat:start', () => {
      this.telemetry.startStreaming();
      store.patch({ streaming: true, draft: '' });
      this.composer.setStreaming(true);
      this.setEmotion('thinking');
      this.idle.triggerBlink();
      this.setStatus('thinking');
    });

    await on<ChatThinkingEvent>('chat:thinking', (payload) => {
      this.telemetry.addThinking(payload.delta);
    });

    await on<ChatDeltaEvent>('chat:delta', (payload) => {
      this.telemetry.addText(payload.delta);
      store.set('draft', payload.visible);
      this.transcript.appendDelta(payload.visible);
      this.setStatus(null);
    });

    await on<Utterance>('chat:utterance', (utterance) => this.enqueueBeat(utterance));

    await on<ChatDoneEvent>('chat:done', (payload) => {
      this.telemetry.finishStreaming();
      if (payload.context_window) {
        this.telemetry.setContext(payload.context_tokens ?? 0, payload.context_window);
      }
      this.transcript.finishStreaming(payload.text, this.emotion.currentEmotion);
      store.patch({ streaming: false, draft: '' });
      this.composer.setStreaming(false);
      void this.refreshChats();
    });

    await on<ChatCancelEvent>('chat:cancelled', (payload) => {
      this.telemetry.cancel();
      this.transcript.finishStreaming(payload.text, this.emotion.currentEmotion);
      store.patch({ streaming: false });
      this.composer.setStreaming(false);
      this.setStatus(null);
    });

    await on<ChatErrorEvent>('chat:error', (payload) => {
      this.telemetry.cancel();
      this.transcript.addError(payload.message);
      store.patch({ streaming: false, error: payload.message });
      this.composer.setStreaming(false);
      this.setStatus(null);
      this.setEmotion('sad');
    });

    await on<SettingsChangedEvent>('settings:changed', (payload) => {
      store.patch({ settings: payload.settings, configured: true });
      applyPalette(payload.settings);
    });

    // The engine loads voices asynchronously on some Windows builds.
    if ('speechSynthesis' in window) {
      window.speechSynthesis.addEventListener?.('voiceschanged', () => undefined);
    }

    window.addEventListener('beforeunload', () => this.speech.cancel());
  }

  private async send(text: string): Promise<void> {
    this.setLanding(false);
    this.transcript.addUser(text);
    this.transcript.beginStreaming();
    this.setStatus('thinking');

    if (!this.activeChatId) await this.bootChats();

    try {
      await invoke('send_message', { chatId: this.activeChatId, text });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.transcript.addError(message);
      store.patch({ streaming: false, error: message });
      this.composer.setStreaming(false);
      this.setStatus(null);
    }
  }

  private async stop(): Promise<void> {
    this.speech.cancel();
    this.beatQueue = [];
    this.draining = false;
    await invoke('cancel_generation');
    this.setStatus(null);
  }

  /* ── speech ────────────────────────────────────────────────── */

  private enqueueBeat(utterance: Utterance): void {
    this.beatQueue.push(utterance);
    if (!this.draining) this.drain();
  }

  private drain(): void {
    window.clearTimeout(this.beatTimer);

    const next = this.beatQueue.shift();
    if (!next) {
      this.draining = false;
      this.setStatus(null);
      return;
    }

    this.draining = true;
    // The character reacts even when muted — the face is the primary channel.
    this.setEmotion(next.emotion);

    const settings = store.get('settings');
    const canSpeak = settings?.voice.enabled === true && this.speech.supported;

    if (!canSpeak) {
      const readingTime = Math.min(2600, 420 + next.text.length * 42);
      this.beatTimer = window.setTimeout(() => this.drain(), readingTime);
      return;
    }

    this.setStatus('speaking');
    this.speech.speak(next.text, {
      onStart: () => this.setSpeaking(true),
      onBoundary: () => this.lipsync.accent(),
      onLevel: (level) => {
        this.lipsync.setLevel(level);
        this.stage.setEnergy(level);
        this.avatar.setEnergy(level);
        this.lighting.setEnergy(level);
      },
      onEnd: () => {
        this.lipsync.stop();
        this.setSpeaking(false);
        this.drain();
      },
    });
  }

  private setSpeaking(speaking: boolean): void {
    store.set('speaking', speaking);
    if (!speaking) {
      this.stage.setEnergy(0);
      this.avatar.setEnergy(0);
      this.lighting.setEnergy(0);
    }
  }

  /** Speak a sample with the *draft* settings, then restore the saved ones. */
  private speakPreview(text: string, draft: Settings): void {
    const saved = store.get('settings');
    this.speech.configure({
      enabled: true,
      voiceUri: draft.voice.voice_uri,
      locale: draft.voice.preferred_locale,
      rate: draft.voice.rate,
      pitch: draft.voice.pitch,
      volume: draft.voice.volume,
    });
    this.speech.speak(text, {
      onLevel: (level) => {
        this.lipsync.setLevel(level);
        this.stage.setEnergy(level);
        this.lighting.setEnergy(level);
      },
      onBoundary: () => this.lipsync.accent(),
      onEnd: () => {
        this.lipsync.stop();
        if (saved) {
          this.speech.configure({
            enabled: saved.voice.enabled,
            voiceUri: saved.voice.voice_uri,
            locale: saved.voice.preferred_locale,
            rate: saved.voice.rate,
            pitch: saved.voice.pitch,
            volume: saved.voice.volume,
          });
        }
      },
    });
  }

  private setEmotion(emotion: Emotion): void {
    store.set('emotion', emotion);
    this.emotion.set(emotion);
    this.transcript.setStreamingEmotion(emotion);
  }

  /* ── microphone ────────────────────────────────────────────── */

  private async startListening(): Promise<void> {
    if (store.get('listening')) return;
    store.set('listening', true);

    try {
      await this.meter.start((level) => {
        this.lipsync.setLevel(level * 0.6);
        this.composer.setMicLevel(level, true);
      });

      if (this.transcriber.available) {
        this.transcriber.start({
          onText: (text, final) => {
            if (final) this.composer.appendTranscript(text);
            else this.composer.setValue(text);
          },
          onError: (message) => console.warn('[yura] transcription:', message),
        });
      } else {
        this.setStatus('listening — no offline recogniser, type instead');
      }
    } catch (error) {
      store.set('listening', false);
      console.warn('[yura] microphone unavailable', error);
    }
  }

  private stopListening(): void {
    if (!store.get('listening')) return;
    store.set('listening', false);
    this.meter.stop();
    this.transcriber.stop();
    this.lipsync.stop();
    this.composer.setMicLevel(0, false);
    this.setStatus(null);
  }

  /* ── settings ──────────────────────────────────────────────── */

  private async saveSettings(next: Settings): Promise<void> {
    const saved = await invoke<Settings>('save_settings', { settings: next });
    store.patch({ settings: saved, configured: true });
    applyPalette(saved);

    this.stage.setHue(saved.ui.accent_hue);
    this.stage.setQuality(saved.avatar.quality);
    this.lighting.setHue(saved.ui.accent_hue);
    this.avatar.setHue(saved.ui.accent_hue);
    this.lipsync.setGain(saved.avatar.lipsync_gain);
    this.idle.setEnabled({
      motion: saved.avatar.idle_motion,
      blink: saved.avatar.auto_blink,
    });
    this.gaze.setEnabled(saved.avatar.gaze_follow);
    this.transcript.setVisible(saved.ui.show_transcript);

    this.speech.configure({
      enabled: saved.voice.enabled,
      voiceUri: saved.voice.voice_uri,
      locale: saved.voice.preferred_locale,
      rate: saved.voice.rate,
      pitch: saved.voice.pitch,
      volume: saved.voice.volume,
    });

    if ((saved.avatar.model_file || this.models[0] || '') !== this.currentModel) {
      this.avatar.dispose();
      this.avatar = new Avatar();
      this.stage.scene.add(this.avatar.root);
      await this.loadCharacter(saved);
    }

    this.panel.close();
    must<HTMLButtonElement>('#btn-settings').classList.remove('active');
  }

  private async togglePin(): Promise<void> {
    const settings = store.get('settings');
    if (!settings) return;
    const next: Settings = structuredClone(settings);
    next.ui.always_on_top = !next.ui.always_on_top;
    await invoke('set_always_on_top', { on: next.ui.always_on_top });
    const saved = await invoke<Settings>('save_settings', { settings: next });
    store.set('settings', saved);
    must<HTMLButtonElement>('#btn-pin').classList.toggle('active', saved.ui.always_on_top);
  }

  /* ── chrome ────────────────────────────────────────────────── */

  private showSetup(visible: boolean): void {
    must<HTMLElement>('#setup').hidden = !visible;
  }

  private setStatus(text: string | null): void {
    const node = must<HTMLElement>('#status');
    const dot = must<HTMLElement>('#status-dot');

    if (!text) {
      node.classList.remove('on');
      node.textContent = '';
      dot.classList.remove('live');
      return;
    }

    // Built from DOM nodes only — the status vocabulary is fixed, never markup.
    const label = document.createElement('span');
    label.textContent = text;

    if (text === 'thinking' || text === 'speaking') {
      const dots = document.createElement('span');
      dots.className = 'dots';
      for (let i = 0; i < 3; i += 1) {
        const d = document.createElement('span');
        d.textContent = '·';
        dots.append(d);
      }
      node.replaceChildren(label, document.createTextNode(' '), dots);
    } else {
      node.replaceChildren(label);
    }

    node.classList.add('on');
    dot.classList.toggle('live', text !== 'listening');
  }
}

function must<T extends Element>(selector: string): T {
  const node = document.querySelector<T>(selector);
  if (!node) throw new Error(`missing element: ${selector}`);
  return node;
}

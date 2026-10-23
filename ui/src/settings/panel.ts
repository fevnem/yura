import type { PresetDto, Settings } from '../bridge/types';

/**
 * The settings drawer.
 *
 * A plain, boring form on purpose: it is the one screen where predictability
 * beats flourish. It edits a *copy* of the settings object and only writes on
 * Save, so a half-typed base URL never silently breaks the running app.
 *
 * Every control is generated from a tiny value/change pair — adding a setting
 * means adding one row, not one more block of DOM plumbing.
 */

export interface PanelOptions {
  getSettings: () => Settings;
  getPresets: () => PresetDto[];
  getVoices: () => SpeechSynthesisVoice[];
  getModels: () => string[];
  onSave: (settings: Settings) => Promise<void>;
  onClearHistory: () => Promise<void>;
  onClose: () => void;
}

type FieldKind = 'text' | 'password' | 'number' | 'range' | 'textarea' | 'select' | 'toggle';

interface FieldSpec {
  label?: string;
  kind: FieldKind;
  value: string | number | boolean;
  hint?: string;
  min?: number;
  max?: number;
  step?: number;
  suffix?: string;
  options?: { value: string; label: string }[];
  onChange: (value: string) => void;
}

export class SettingsPanel {
  private draft: Settings | null = null;
  private voiceList: SpeechSynthesisVoice[] = [];
  private models: string[] = [];
  private savedTimer = 0;

  constructor(
    private root: HTMLElement,
    private options: PanelOptions,
  ) {}

  get isOpen(): boolean {
    return !this.root.hidden;
  }

  open(): void {
    this.draft = structuredClone(this.options.getSettings());
    this.voiceList = this.options.getVoices();
    this.models = this.options.getModels();
    this.render();
    this.root.hidden = false;
  }

  close(): void {
    this.root.hidden = true;
    this.options.onClose();
  }

  toggle(): void {
    if (this.isOpen) this.close();
    else this.open();
  }

  private render(): void {
    const draft = this.draft;
    if (!draft) return;

    this.root.replaceChildren();

    const header = el('div', 'drawer-head');
    header.append(el('h2', '', 'Settings'));
    header.append(el('p', 'sub', 'Nothing leaves this machine unless you point it somewhere.'));
    this.root.append(header);

    this.root.append(
      this.group('Model', [
        this.field({
          label: 'Provider',
          kind: 'select',
          value: draft.llm.provider_id,
          options: this.options.getPresets().map((p) => ({ value: p.id, label: p.label })),
          onChange: (value) => {
            const preset = this.options.getPresets().find((p) => p.id === value);
            draft.llm.provider_id = value;
            if (preset) {
              draft.llm.base_url = preset.base_url;
              if (!draft.llm.model || preset.default_model) {
                draft.llm.model = preset.default_model;
              }
            }
            this.render();
          },
        }),
        this.field({
          label: 'Endpoint',
          kind: 'text',
          value: draft.llm.base_url,
          hint: 'Base URL. `/chat/completions` is appended automatically.',
          onChange: (value) => {
            draft.llm.base_url = value.trim();
          },
        }),
        this.field({
          label: 'API key',
          kind: 'password',
          value: draft.llm.api_key,
          hint: 'Stored locally in your config file. Never sent anywhere but your endpoint.',
          onChange: (value) => {
            draft.llm.api_key = value.trim();
          },
        }),
        this.field({
          label: 'Model',
          kind: 'text',
          value: draft.llm.model,
          onChange: (value) => {
            draft.llm.model = value.trim();
          },
        }),
        this.field({
          label: 'Temperature',
          kind: 'range',
          value: draft.llm.temperature,
          min: 0,
          max: 2,
          step: 0.05,
          onChange: (value) => {
            draft.llm.temperature = Number(value);
          },
        }),
        this.field({
          label: 'Max reply length',
          kind: 'range',
          value: draft.llm.max_tokens,
          min: 64,
          max: 2048,
          step: 32,
          suffix: ' tokens',
          onChange: (value) => {
            draft.llm.max_tokens = Number(value);
          },
        }),
        this.field({
          label: 'Memory',
          kind: 'range',
          value: draft.llm.context_turns,
          min: 0,
          max: 60,
          step: 1,
          suffix: ' turns',
          onChange: (value) => {
            draft.llm.context_turns = Number(value);
          },
        }),
      ]),
    );

    this.root.append(
      this.group('Persona', [
        this.field({
          label: 'Name',
          kind: 'text',
          value: draft.persona.name,
          onChange: (value) => {
            draft.persona.name = value;
          },
        }),
        this.field({
          label: 'System prompt',
          kind: 'textarea',
          value: draft.persona.system_prompt,
          onChange: (value) => {
            draft.persona.system_prompt = value;
          },
        }),
        this.field({
          label: 'Speech style',
          kind: 'textarea',
          value: draft.persona.speech_style,
          onChange: (value) => {
            draft.persona.speech_style = value;
          },
        }),
      ]),
    );

    const voiceOptions = [
      { value: '', label: 'Automatic (best available)' },
      ...this.voiceList.map((v) => ({ value: v.voiceURI, label: `${v.name} — ${v.lang}` })),
    ];
    if (!this.voiceList.length) {
      voiceOptions.push({ value: '', label: 'No system voices found' });
    }

    this.root.append(
      this.group('Voice', [
        this.field({
          label: 'Speak replies aloud',
          kind: 'toggle',
          value: draft.voice.enabled,
          onChange: (value) => {
            draft.voice.enabled = value === 'true';
          },
        }),
        this.field({
          label: 'Voice',
          kind: 'select',
          value: draft.voice.voice_uri,
          options: voiceOptions,
          onChange: (value) => {
            draft.voice.voice_uri = value;
          },
        }),
        this.field({
          label: 'Pitch',
          kind: 'range',
          value: draft.voice.pitch,
          min: 0.5,
          max: 2,
          step: 0.02,
          onChange: (value) => {
            draft.voice.pitch = Number(value);
          },
        }),
        this.field({
          label: 'Rate',
          kind: 'range',
          value: draft.voice.rate,
          min: 0.6,
          max: 1.8,
          step: 0.02,
          onChange: (value) => {
            draft.voice.rate = Number(value);
          },
        }),
        this.field({
          label: 'Volume',
          kind: 'range',
          value: draft.voice.volume,
          min: 0,
          max: 1,
          step: 0.05,
          onChange: (value) => {
            draft.voice.volume = Number(value);
          },
        }),
      ]),
    );

    const modelOptions = [{ value: '', label: 'Spirit form (no model)' }];
    for (const file of this.models) {
      modelOptions.push({ value: file, label: file.replace(/\.vrm$/i, '') });
    }

    this.root.append(
      this.group('Avatar', [
        this.field({
          label: 'Character',
          kind: 'select',
          value: draft.avatar.model_file,
          options: modelOptions,
          hint: 'Drop any .vrm into ui/public/assets/models/ — it appears here.',
          onChange: (value) => {
            draft.avatar.model_file = value;
          },
        }),
        this.field({
          label: 'Scale',
          kind: 'range',
          value: draft.avatar.scale,
          min: 0.6,
          max: 1.6,
          step: 0.02,
          onChange: (value) => {
            draft.avatar.scale = Number(value);
          },
        }),
        this.field({
          label: 'Follow the cursor',
          kind: 'toggle',
          value: draft.avatar.gaze_follow,
          onChange: (value) => {
            draft.avatar.gaze_follow = value === 'true';
          },
        }),
        this.field({
          label: 'Blink',
          kind: 'toggle',
          value: draft.avatar.auto_blink,
          onChange: (value) => {
            draft.avatar.auto_blink = value === 'true';
          },
        }),
        this.field({
          label: 'Idle motion',
          kind: 'toggle',
          value: draft.avatar.idle_motion,
          onChange: (value) => {
            draft.avatar.idle_motion = value === 'true';
          },
        }),
      ]),
    );

    this.root.append(
      this.group('Room', [
        this.field({
          label: 'Accent',
          kind: 'range',
          value: draft.ui.accent_hue,
          min: 0,
          max: 360,
          step: 2,
          suffix: '°',
          onChange: (value) => {
            draft.ui.accent_hue = Number(value);
            document.documentElement.style.setProperty('--hue', value);
          },
        }),
        this.field({
          label: 'Keep on top',
          kind: 'toggle',
          value: draft.ui.always_on_top,
          onChange: (value) => {
            draft.ui.always_on_top = value === 'true';
          },
        }),
        this.field({
          label: 'Show transcript',
          kind: 'toggle',
          value: draft.ui.show_transcript,
          onChange: (value) => {
            draft.ui.show_transcript = value === 'true';
          },
        }),
      ]),
    );

    const actions = el('div', 'drawer-actions');
    const save = btn('primary', 'Save');
    save.addEventListener('click', () => void this.save());

    const saved = el('span', 'saved-pill', 'saved');
    const clear = btn('ghost danger', 'Clear history');
    clear.addEventListener('click', () => void this.options.onClearHistory());

    actions.append(save, saved, el('span', 'spacer'), clear);
    this.root.append(actions);
    this.savedPill = saved;
  }

  private savedPill: HTMLElement | null = null;

  private async save(): Promise<void> {
    if (!this.draft) return;
    await this.options.onSave(structuredClone(this.draft));
    this.savedPill?.classList.add('on');
    window.clearTimeout(this.savedTimer);
    this.savedTimer = window.setTimeout(() => this.savedPill?.classList.remove('on'), 1600);
  }

  private group(label: string, fields: HTMLElement[]): HTMLElement {
    const wrap = el('section', 'group');
    wrap.append(el('span', 'group-label', label));
    fields.forEach((f) => wrap.append(f));
    return wrap;
  }

  private field(spec: FieldSpec): HTMLElement {
    if (spec.kind === 'toggle') {
      const label = el('label', 'toggle');
      label.append(el('span', '', spec.label ?? ''));
      const input = document.createElement('input');
      input.type = 'checkbox';
      input.checked = spec.value === true;
      input.addEventListener('change', () => spec.onChange(String(input.checked)));
      label.append(input);
      return label;
    }

    const wrap = el('label', 'field');
    if (spec.label) {
      const top = el('span', '', spec.label);
      if (spec.kind === 'range') {
        const val = el('span', 'val', `${spec.value}${spec.suffix ?? ''}`);
        top.append(val);
      }
      wrap.append(top);
    }

    if (spec.kind === 'select') {
      const select = document.createElement('select');
      for (const option of spec.options ?? []) {
        const opt = document.createElement('option');
        opt.value = option.value;
        opt.textContent = option.label;
        if (option.value === spec.value) opt.selected = true;
        select.append(opt);
      }
      select.addEventListener('change', () => spec.onChange(select.value));
      wrap.append(select);
    } else if (spec.kind === 'textarea') {
      const area = document.createElement('textarea');
      area.value = String(spec.value);
      area.spellcheck = false;
      area.addEventListener('input', () => spec.onChange(area.value));
      wrap.append(area);
    } else if (spec.kind === 'range') {
      const range = document.createElement('input');
      range.type = 'range';
      range.min = String(spec.min ?? 0);
      range.max = String(spec.max ?? 1);
      range.step = String(spec.step ?? 0.01);
      range.value = String(spec.value);
      const readout = wrap.querySelector('.val');
      range.addEventListener('input', () => {
        if (readout) readout.textContent = `${range.value}${spec.suffix ?? ''}`;
        spec.onChange(range.value);
      });
      wrap.append(range);
    } else {
      const input = document.createElement('input');
      input.type = spec.kind;
      input.value = String(spec.value);
      input.spellcheck = false;
      input.autocomplete = 'off';
      input.addEventListener('input', () => spec.onChange(input.value));
      wrap.append(input);
    }

    if (spec.hint) wrap.append(el('p', 'hint', spec.hint));
    return wrap;
  }
}

function el(tag: string, className = '', text = ''): HTMLElement {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

function btn(className: string, text: string): HTMLButtonElement {
  const node = document.createElement('button');
  node.type = 'button';
  node.className = className;
  node.textContent = text;
  return node;
}

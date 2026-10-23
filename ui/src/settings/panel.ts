import type { PresetDto, Settings, Skill } from '../bridge/types';

/**
 * Settings — a categorised accordion page.
 *
 * You see the categories first, each as one line. Tap a category and its
 * settings open underneath it; the others close. Nothing is a slide-over, and
 * the whole thing reads top to bottom like a real preferences page.
 *
 * Every control is generated from a small descriptor (label, help, input), so
 * adding a setting is one entry in a list rather than more DOM plumbing.
 */

export const THEMES: { id: string; label: string; ink: string; a: string; hue: number }[] = [
  { id: 'midnight', label: 'Midnight', ink: '#06050a', a: '#2a1420', hue: 340 },
  { id: 'dusk', label: 'Dusk', ink: '#05070f', a: '#13253f', hue: 212 },
  { id: 'ember', label: 'Ember', ink: '#0b0705', a: '#3a1f10', hue: 24 },
  { id: 'orchid', label: 'Orchid', ink: '#0a0612', a: '#31173f', hue: 288 },
  { id: 'mono', label: 'Mono', ink: '#0a0a0b', a: '#242426', hue: 0 },
];

type SectionId = 'model' | 'persona' | 'skills' | 'voice' | 'avatar' | 'appearance' | 'window';

interface SectionMeta {
  id: SectionId;
  label: string;
  summary: string;
  icon: string;
}

const SECTIONS: SectionMeta[] = [
  {
    id: 'model',
    label: 'Model',
    summary: 'Provider, endpoint, key, model',
    icon: '<circle cx="8" cy="8" r="2.2"/><path d="M8 1.8v2M8 12.2v2M14.2 8h-2M3.8 8h-2M12.5 3.5l-1.4 1.4M4.9 11.1l-1.4 1.4M12.5 12.5l-1.4-1.4M4.9 4.9 3.5 3.5"/>',
  },
  {
    id: 'persona',
    label: 'Persona',
    summary: 'Her name, character sheet and speech style',
    icon: '<path d="M8 2.2a4 4 0 0 1 4 4v2.6l1.2 2.2H2.8L4 8.8V6.2a4 4 0 0 1 4-4z"/><path d="M6.4 13.2a1.7 1.7 0 0 0 3.2 0"/>',
  },
  {
    id: 'skills',
    label: 'Skills',
    summary: 'Markdown instructions she follows — write or import your own',
    icon: '<path d="M4 2.6h5.2L12.4 5.8v7.6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V3.6a1 1 0 0 1 1-1z"/><path d="M9 2.8V6h3M5.6 8.6h4.8M5.6 11h3.2"/>',
  },
  {
    id: 'voice',
    label: 'Voice',
    summary: 'Which voice she speaks with, and how',
    icon: '<rect x="6" y="2" width="4" height="7" rx="2"/><path d="M3.5 7.5a4.5 4.5 0 0 0 9 0M8 12v2.2"/>',
  },
  {
    id: 'avatar',
    label: 'Avatar',
    summary: 'Her body, expression and idle behaviour',
    icon: '<circle cx="8" cy="6" r="2.6"/><path d="M3 14c0-2.5 2.2-4 5-4s5 1.5 5 4"/>',
  },
  {
    id: 'appearance',
    label: 'Appearance',
    summary: 'Theme, accent and motion',
    icon: '<circle cx="8" cy="8" r="5.4"/><path d="M8 2.6v10.8M2.6 8h10.8"/>',
  },
  {
    id: 'window',
    label: 'Window',
    summary: 'Always on top, and where your data lives',
    icon: '<rect x="2.4" y="3.2" width="11.2" height="9.6" rx="1.8"/><path d="M2.4 6.4h11.2"/>',
  },
];

export interface PanelOptions {
  getSettings: () => Settings;
  getPresets: () => PresetDto[];
  getVoices: () => SpeechSynthesisVoice[];
  getModels: () => string[];
  onSave: (settings: Settings) => Promise<void>;
  getSkills: () => Skill[];
  onToggleSkill: (id: string, enabled: boolean) => Promise<void>;
  onSaveSkill: (skill: Skill) => Promise<Skill>;
  onDeleteSkill: (id: string) => Promise<void>;
  onImportSkill: (file: File) => Promise<void>;
  onClearHistory: () => Promise<void>;
  onTestVoice: (text: string, settings: Settings) => void;
  onClose: () => void;
}

type RowInput =
  | { kind: 'text' | 'password' | 'number'; value: string; placeholder?: string; onChange: (v: string) => void }
  | { kind: 'textarea'; value: string; onChange: (v: string) => void }
  | { kind: 'select'; value: string; options: { value: string; label: string }[]; onChange: (v: string) => void }
  | { kind: 'range'; value: number; min: number; max: number; step: number; suffix?: string; onChange: (v: string) => void }
  | { kind: 'toggle'; value: boolean; onChange: (v: string) => void }
  | { kind: 'custom'; node: HTMLElement };

export class SettingsPanel {
  private draft: Settings | null = null;
  private open: SectionId | null = 'model';
  private voices: SpeechSynthesisVoice[] = [];
  private models: string[] = [];
  private skills: Skill[] = [];
  private editing: Skill | null = null;
  private savedPill: HTMLElement | null = null;
  private savedTimer = 0;
  private sections = new Map<SectionId, { wrap: HTMLElement; body: HTMLElement }>();

  constructor(
    private root: HTMLElement,
    private options: PanelOptions,
  ) {}

  get isOpen(): boolean {
    return !this.root.hidden;
  }

  show(section?: SectionId): void {
    this.draft = structuredClone(this.options.getSettings());
    this.voices = this.options.getVoices();
    this.models = this.options.getModels();
    this.skills = this.options.getSkills();
    this.editing = null;
    if (section) this.open = section;
    this.render();
    this.root.hidden = false;
  }

  close(): void {
    this.root.hidden = true;
    this.options.onClose();
  }

  toggle(): void {
    if (this.isOpen) this.close();
    else this.show();
  }

  /** Windows publishes its voice list asynchronously; redraw if it arrived. */
  refreshVoices(): void {
    const next = this.options.getVoices();
    if (next.length === 0 || next.length === this.voices.length) return;
    this.voices = next;
    if (this.isOpen) this.draw('voice');
  }

  /* ── shell ─────────────────────────────────────────────────── */

  private render(): void {
    this.root.replaceChildren();
    this.sections.clear();

    const page = document.createElement('div');
    page.className = 'settings-page';

    page.append(this.header());

    const list = document.createElement('div');
    list.className = 'settings-list';
    for (const meta of SECTIONS) list.append(this.section(meta));
    page.append(list);

    this.root.append(page);
  }

  private header(): HTMLElement {
    const head = document.createElement('header');
    head.className = 'settings-head';

    const back = document.createElement('button');
    back.type = 'button';
    back.className = 'ghost';
    back.textContent = '← Back';
    back.addEventListener('click', () => this.close());

    const title = document.createElement('div');
    title.className = 'settings-title';
    const h = document.createElement('h2');
    h.textContent = 'Settings';
    const sub = document.createElement('p');
    sub.textContent = 'Nothing leaves this machine unless you point it somewhere.';
    title.append(h, sub);

    this.savedPill = document.createElement('span');
    this.savedPill.className = 'saved-pill';
    this.savedPill.textContent = 'saved';

    const save = document.createElement('button');
    save.type = 'button';
    save.className = 'primary';
    save.textContent = 'Save changes';
    save.addEventListener('click', () => void this.save());

    head.append(back, title, this.savedPill, save);
    return head;
  }

  private section(meta: SectionMeta): HTMLElement {
    const wrap = document.createElement('section');
    wrap.className = 'acc';
    wrap.dataset.section = meta.id;
    wrap.dataset.open = String(this.open === meta.id);

    const head = document.createElement('button');
    head.type = 'button';
    head.className = 'acc-head';
    head.setAttribute('aria-expanded', String(this.open === meta.id));

    const icon = document.createElement('span');
    icon.className = 'acc-icon';
    // Static SVG from the constant table above; nothing user-supplied is injected.
    icon.innerHTML = `<svg viewBox="0 0 16 16" aria-hidden="true">${meta.icon}</svg>`;

    const text = document.createElement('span');
    text.className = 'acc-text';
    const label = document.createElement('span');
    label.className = 'acc-label';
    label.textContent = meta.label;
    const summary = document.createElement('span');
    summary.className = 'acc-summary';
    summary.textContent = meta.summary;
    text.append(label, summary);

    const chevron = document.createElement('span');
    chevron.className = 'acc-chevron';

    head.append(icon, text, chevron);
    head.addEventListener('click', () => this.pick(meta.id));

    const body = document.createElement('div');
    body.className = 'acc-body';

    wrap.append(head, body);
    this.sections.set(meta.id, { wrap, body });
    this.draw(meta.id);

    return wrap;
  }

  private pick(id: SectionId): void {
    this.open = this.open === id ? null : id;
    for (const [sid, { wrap }] of this.sections) {
      const isOpen = sid === this.open;
      wrap.dataset.open = String(isOpen);
      const head = wrap.querySelector('.acc-head');
      head?.setAttribute('aria-expanded', String(isOpen));
    }
  }

  private draw(id: SectionId): void {
    const target = this.sections.get(id);
    const draft = this.draft;
    if (!target || !draft) return;

    target.body.replaceChildren();
    for (const row of this.rows(id)) target.body.append(row);

    if (id === 'window') target.body.append(this.dangerZone());
  }

  private dangerZone(): HTMLElement {
    const zone = document.createElement('div');
    zone.className = 'acc-danger';

    const clear = document.createElement('button');
    clear.type = 'button';
    clear.className = 'ghost danger';
    clear.textContent = 'Clear conversation history';
    clear.addEventListener('click', () => void this.options.onClearHistory());

    zone.append(clear);
    return zone;
  }

  /* ── sections ──────────────────────────────────────────────── */

  private rows(section: SectionId): HTMLElement[] {
    const d = this.draft!;

    switch (section) {
      case 'model': {
        const presets = this.options.getPresets();
        return [
          this.row('Provider', 'Pick a vendor, or "Custom" for anything OpenAI-shaped.', {
            kind: 'select',
            value: d.llm.provider_id,
            options: presets.map((p) => ({ value: p.id, label: p.label })),
            onChange: (value) => {
              const preset = presets.find((p) => p.id === value);
              d.llm.provider_id = value;
              if (preset) {
                d.llm.base_url = preset.base_url;
                d.llm.model = preset.default_model;
                d.llm.api_key = '';
              }
              this.draw('model');
            },
          }),
          this.row('Endpoint', 'Base URL only — /chat/completions is appended.', {
            kind: 'text',
            value: d.llm.base_url,
            placeholder: 'https://api.example.com/v1',
            onChange: (v) => {
              d.llm.base_url = v.trim();
            },
          }),
          this.row('API key', 'Stored locally. Sent only to the endpoint above.', {
            kind: 'password',
            value: d.llm.api_key,
            placeholder: 'sk-…',
            onChange: (v) => {
              d.llm.api_key = v.trim();
            },
          }),
          this.row('Model', 'The exact model id your endpoint expects.', {
            kind: 'text',
            value: d.llm.model,
            placeholder: 'gpt-4o-mini',
            onChange: (v) => {
              d.llm.model = v.trim();
            },
          }),
          this.row('Temperature', 'Lower is steadier, higher is more playful. She reads well near 0.85.', {
            kind: 'range',
            value: d.llm.temperature,
            min: 0,
            max: 2,
            step: 0.05,
            onChange: (v) => {
              d.llm.temperature = Number(v);
            },
          }),
          this.row('Reply length', 'Tokens. Short answers keep a conversation alive.', {
            kind: 'range',
            value: d.llm.max_tokens,
            min: 64,
            max: 2048,
            step: 32,
            suffix: ' tokens',
            onChange: (v) => {
              d.llm.max_tokens = Number(v);
            },
          }),
          this.row('Memory', 'How many past turns she can still see.', {
            kind: 'range',
            value: d.llm.context_turns,
            min: 0,
            max: 60,
            step: 1,
            suffix: ' turns',
            onChange: (v) => {
              d.llm.context_turns = Number(v);
            },
          }),
        ];
      }

      case 'persona':
        return [
          this.row('Name', 'Shown in the title bar and used in her prompt.', {
            kind: 'text',
            value: d.persona.name,
            onChange: (v) => {
              d.persona.name = v;
            },
          }),
          this.row('Tagline', 'One quiet line under her name. Cosmetic.', {
            kind: 'text',
            value: d.persona.tagline,
            onChange: (v) => {
              d.persona.tagline = v;
            },
          }),
          this.row('System prompt', 'The character sheet — the whole personality lives here.', {
            kind: 'textarea',
            value: d.persona.system_prompt,
            onChange: (v) => {
              d.persona.system_prompt = v;
            },
          }),
          this.row('Speech style', 'Appended as hard rules: length, tone, what never to write.', {
            kind: 'textarea',
            value: d.persona.speech_style,
            onChange: (v) => {
              d.persona.speech_style = v;
            },
          }),
        ];

      case 'skills':
        return [
          this.row(
            'Active skills',
            'She reads these with every request. Toggle one off without deleting it.',
            { kind: 'custom', node: this.buildSkills() },
          ),
        ];

      case 'voice': {
        const voiceOptions = [
          {
            value: '',
            label: this.voices.length ? 'Automatic — best installed voice' : 'No system voices detected',
          },
          ...this.voices.map((v) => ({ value: v.voiceURI, label: `${v.name} · ${v.lang}` })),
        ];

        const test = document.createElement('div');
        test.className = 'row-inline';
        const testBtn = document.createElement('button');
        testBtn.type = 'button';
        testBtn.className = 'ghost';
        testBtn.textContent = 'Speak a test line';
        testBtn.addEventListener('click', () => {
          if (this.draft) this.options.onTestVoice("Hey. It's me.", structuredClone(this.draft));
        });
        test.append(testBtn);

        return [
          this.row('Speak replies', 'Read each beat aloud as it arrives.', {
            kind: 'toggle',
            value: d.voice.enabled,
            onChange: (v) => {
              d.voice.enabled = v === 'true';
            },
          }),
          this.row('Voice', 'The neural voices are usually the nicest.', {
            kind: 'select',
            value: d.voice.voice_uri,
            options: voiceOptions,
            onChange: (v) => {
              d.voice.voice_uri = v;
            },
          }),
          this.row('Pitch', 'Slightly high reads younger. Too high is a chipmunk.', {
            kind: 'range',
            value: d.voice.pitch,
            min: 0.6,
            max: 1.8,
            step: 0.02,
            onChange: (v) => {
              d.voice.pitch = Number(v);
            },
          }),
          this.row('Speed', 'Natural speech is around 1.0.', {
            kind: 'range',
            value: d.voice.rate,
            min: 0.6,
            max: 1.8,
            step: 0.02,
            onChange: (v) => {
              d.voice.rate = Number(v);
            },
          }),
          this.row('Volume', 'Room level, not a concert.', {
            kind: 'range',
            value: d.voice.volume,
            min: 0,
            max: 1,
            step: 0.05,
            onChange: (v) => {
              d.voice.volume = Number(v);
            },
          }),
          this.row('Try it', 'Speaks with these settings without saving.', {
            kind: 'custom',
            node: test,
          }),
        ];
      }

      case 'avatar': {
        const modelOptions = [{ value: '', label: 'Spirit form — no model file' }];
        for (const file of this.models) {
          modelOptions.push({ value: file, label: file.replace(/\.vrm$/i, '').replace(/[_-]+/g, ' ') });
        }

        return [
          this.row(
            'Character',
            this.models.length ? 'Bundled with Yura.' : 'No .vrm found — she uses her spirit form.',
            {
              kind: 'select',
              value: d.avatar.model_file,
              options: modelOptions,
              onChange: (v) => {
                d.avatar.model_file = v;
              },
            },
          ),
          this.row('Size', 'Auto-fitted to her standing height, then scaled.', {
            kind: 'range',
            value: d.avatar.scale,
            min: 0.6,
            max: 1.6,
            step: 0.02,
            onChange: (v) => {
              d.avatar.scale = Number(v);
            },
          }),
          this.row('Follow the cursor', 'Eyes track the pointer; the head follows at a fraction.', {
            kind: 'toggle',
            value: d.avatar.gaze_follow,
            onChange: (v) => {
              d.avatar.gaze_follow = v === 'true';
            },
          }),
          this.row('Blink', 'Jittered, with the occasional double blink.', {
            kind: 'toggle',
            value: d.avatar.auto_blink,
            onChange: (v) => {
              d.avatar.auto_blink = v === 'true';
            },
          }),
          this.row('Breathe and sway', 'Idle motion. Turn it off for a statue.', {
            kind: 'toggle',
            value: d.avatar.idle_motion,
            onChange: (v) => {
              d.avatar.idle_motion = v === 'true';
            },
          }),
        ];
      }

      case 'appearance': {
        const themes = document.createElement('div');
        themes.className = 'themes';

        for (const theme of THEMES) {
          const btn = document.createElement('button');
          btn.type = 'button';
          btn.className = 'swatch';
          btn.setAttribute('aria-pressed', String((d.ui.theme ?? 'midnight') === theme.id));
          btn.style.background = `linear-gradient(165deg, ${theme.a}, ${theme.ink})`;
          btn.title = theme.label;

          const dot = document.createElement('i');
          dot.style.color = theme.a;
          dot.style.background = theme.a;
          btn.append(dot, el('span', '', theme.label));

          btn.addEventListener('click', () => {
            d.ui.theme = theme.id;
            d.ui.accent_hue = theme.hue;
            document.documentElement.dataset.theme = theme.id;
            document.documentElement.style.setProperty('--hue', String(theme.hue));
            this.draw('appearance');
          });
          themes.append(btn);
        }

        return [
          this.row('Theme', 'Changes the room, not the character.', { kind: 'custom', node: themes }),
          this.row('Accent', 'One number drives every glow in the app.', {
            kind: 'range',
            value: d.ui.accent_hue,
            min: 0,
            max: 360,
            step: 2,
            suffix: '°',
            onChange: (v) => {
              d.ui.accent_hue = Number(v);
              document.documentElement.style.setProperty('--hue', v);
            },
          }),
          this.row('Show transcript', 'Hide the conversation column if you only want her face.', {
            kind: 'toggle',
            value: d.ui.show_transcript,
            onChange: (v) => {
              d.ui.show_transcript = v === 'true';
            },
          }),
          this.row('Reduce motion', 'Calms the camera drift and the idle sway.', {
            kind: 'toggle',
            value: d.ui.reduce_motion,
            onChange: (v) => {
              d.ui.reduce_motion = v === 'true';
            },
          }),
        ];
      }

      case 'window':
      default:
        return [
          this.row('Keep on top', 'She stays above other windows.', {
            kind: 'toggle',
            value: d.ui.always_on_top,
            onChange: (v) => {
              d.ui.always_on_top = v === 'true';
            },
          }),
          this.row('Where your data lives', 'Settings and history are plain files you can read or delete.', {
            kind: 'custom',
            node: el('p', 'row-help', '%APPDATA%\\Yura'),
          }),
        ];
    }
  }


  /* ── skills ────────────────────────────────────────────────── */

  private buildSkills(): HTMLElement {
    const wrap = document.createElement('div');

    const list = document.createElement('div');
    list.className = 'skill-list';

    if (this.skills.length === 0) {
      const empty = document.createElement('p');
      empty.className = 'row-help';
      empty.textContent =
        'No skills yet. A skill is a markdown file — write one here, or drop any .md into the skills folder.';
      list.append(empty);
    }

    for (const skill of this.skills) {
      const item = document.createElement('div');
      item.className = 'skill-item';
      item.dataset.on = String(skill.enabled);

      const toggle = document.createElement('label');
      toggle.className = 'switch';
      const box = document.createElement('input');
      box.type = 'checkbox';
      box.checked = skill.enabled;
      box.addEventListener('change', () => {
        skill.enabled = box.checked;
        item.dataset.on = String(box.checked);
        void this.options.onToggleSkill(skill.id, box.checked);
      });
      toggle.append(box);

      const main = document.createElement('div');
      main.className = 'skill-main';
      const name = document.createElement('div');
      name.className = 'skill-name';
      name.textContent = skill.name;
      const desc = document.createElement('div');
      desc.className = 'skill-desc';
      desc.textContent = skill.description || firstLine(skill.body) || 'No description';
      main.append(name, desc);

      const actions = document.createElement('div');
      actions.className = 'skill-actions';

      const edit = document.createElement('button');
      edit.type = 'button';
      edit.className = 'ghost';
      edit.textContent = 'Edit';
      edit.addEventListener('click', () => {
        this.editing = structuredClone(skill);
        this.draw('skills');
      });

      const del = document.createElement('button');
      del.type = 'button';
      del.className = 'ghost danger';
      del.textContent = 'Delete';
      del.addEventListener('click', () => {
        this.skills = this.skills.filter((s) => s.id !== skill.id);
        if (this.editing?.id === skill.id) this.editing = null;
        void this.options.onDeleteSkill(skill.id).then(() => this.draw('skills'));
        this.draw('skills');
      });

      actions.append(edit, del);
      item.append(toggle, main, actions);
      list.append(item);
    }

    wrap.append(list);

    // toolbar
    const bar = document.createElement('div');
    bar.className = 'row-inline';
    bar.style.marginTop = '12px';

    const add = document.createElement('button');
    add.type = 'button';
    add.className = 'ghost';
    add.textContent = this.editing && !this.editing.id ? 'Cancel' : 'New skill';
    add.addEventListener('click', () => {
      this.editing = this.editing ? null : { id: '', name: '', description: '', body: '', enabled: true };
      this.draw('skills');
    });

    const importBtn = document.createElement('button');
    importBtn.type = 'button';
    importBtn.className = 'ghost';
    importBtn.textContent = 'Import .md';
    const file = document.createElement('input');
    file.type = 'file';
    file.accept = '.md,.markdown,text/markdown,text/plain';
    file.hidden = true;
    file.addEventListener('change', () => {
      const picked = file.files?.[0];
      if (picked) void this.options.onImportSkill(picked).then((skills) => {
        this.skills = this.options.getSkills();
        this.draw('skills');
      });
      file.value = '';
    });
    importBtn.addEventListener('click', () => file.click());

    bar.append(add, importBtn, file);
    wrap.append(bar);

    if (this.editing) wrap.append(this.skillEditor());

    return wrap;
  }

  private skillEditor(): HTMLElement {
    const draft = this.editing!;
    const box = document.createElement('div');
    box.className = 'skill-editor';

    const nameRow = this.row(
      'Name',
      'Shown in this list.',
      {
        kind: 'text',
        value: draft.name,
        placeholder: 'Paper digest',
        onChange: (v) => {
          draft.name = v;
        },
      },
    );

    const descRow = this.row('Description', 'One line — also used in the prompt header.', {
      kind: 'text',
      value: draft.description,
      placeholder: 'Summarise a paper as claims, method, limits',
      onChange: (v) => {
        draft.description = v;
      },
    });

    const bodyRow = this.row(
      'Instructions',
      'Markdown. Written as rules she follows: what to do, in what order, what never to do.',
      {
        kind: 'textarea',
        value: draft.body,
        onChange: (v) => {
          draft.body = v;
        },
      },
    );

    const actions = document.createElement('div');
    actions.className = 'row-inline';

    const save = document.createElement('button');
    save.type = 'button';
    save.className = 'primary';
    save.textContent = 'Save skill';
    save.addEventListener('click', () => {
      if (!draft.name.trim() && !draft.body.trim()) return;
      void this.options.onSaveSkill(structuredClone(draft)).then((saved) => {
        this.skills = this.skills.filter((s) => s.id !== saved.id);
        this.skills.push(saved);
        this.editing = null;
        this.draw('skills');
      });
    });

    const cancel = document.createElement('button');
    cancel.type = 'button';
    cancel.className = 'ghost';
    cancel.textContent = 'Cancel';
    cancel.addEventListener('click', () => {
      this.editing = null;
      this.draw('skills');
    });

    actions.append(save, cancel);
    box.append(nameRow, descRow, bodyRow, actions);
    return box;
  }

  /* ── building blocks ───────────────────────────────────────── */

  private row(label: string, help: string, input: RowInput): HTMLElement {
    const row = document.createElement('div');
    row.className = 'row';

    const left = document.createElement('div');
    left.className = 'row-label';
    left.append(document.createTextNode(label));
    if (help) {
      const p = document.createElement('p');
      p.className = 'row-help';
      p.textContent = help;
      left.append(p);
    }

    const right = document.createElement('div');
    right.className = 'row-control';
    right.append(this.control(input));

    row.append(left, right);
    return row;
  }

  private control(input: RowInput): HTMLElement {
    if (input.kind === 'custom') return input.node;

    if (input.kind === 'toggle') {
      const wrap = document.createElement('label');
      wrap.className = 'switch';
      const box = document.createElement('input');
      box.type = 'checkbox';
      box.checked = input.value;
      box.addEventListener('change', () => input.onChange(String(box.checked)));
      wrap.append(box);
      return wrap;
    }

    if (input.kind === 'select') {
      const select = document.createElement('select');
      for (const option of input.options) {
        const opt = document.createElement('option');
        opt.value = option.value;
        opt.textContent = option.label;
        if (option.value === input.value) opt.selected = true;
        select.append(opt);
      }
      select.addEventListener('change', () => input.onChange(select.value));
      return select;
    }

    if (input.kind === 'textarea') {
      const area = document.createElement('textarea');
      area.value = input.value;
      area.spellcheck = false;
      area.addEventListener('input', () => input.onChange(area.value));
      return area;
    }

    if (input.kind === 'range') {
      const wrap = document.createElement('div');
      const range = document.createElement('input');
      range.type = 'range';
      range.min = String(input.min);
      range.max = String(input.max);
      range.step = String(input.step);
      range.value = String(input.value);
      const readout = document.createElement('div');
      readout.className = 'val';
      readout.textContent = `${input.value}${input.suffix ?? ''}`;
      range.addEventListener('input', () => {
        readout.textContent = `${range.value}${input.suffix ?? ''}`;
        input.onChange(range.value);
      });
      wrap.append(range, readout);
      return wrap;
    }

    const field = document.createElement('input');
    field.type = input.kind;
    field.value = input.value;
    field.spellcheck = false;
    field.autocomplete = 'off';
    if (input.placeholder) field.placeholder = input.placeholder;
    field.addEventListener('input', () => input.onChange(field.value));
    return field;
  }

  private async save(): Promise<void> {
    if (!this.draft) return;
    await this.options.onSave(structuredClone(this.draft));
    this.savedPill?.classList.add('on');
    window.clearTimeout(this.savedTimer);
    this.savedTimer = window.setTimeout(() => this.savedPill?.classList.remove('on'), 1800);
  }
}

function firstLine(markdown: string): string {
  return (
    markdown
      .split('\n')
      .map((line) => line.replace(/^#+\s*/, '').trim())
      .find((line) => line.length > 0) ?? ''
  );
}

function el(tag: string, className = '', text = ''): HTMLElement {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

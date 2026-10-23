import type { Emotion } from '../bridge/types';

/**
 * The conversation column.
 *
 * Owns only DOM. It does not know where text comes from — it is told to start a
 * message, append deltas, and finish it. That keeps streaming logic in `app.ts`
 * where the event handlers live.
 */

const EMOTION_LABEL: Record<Emotion, string> = {
  neutral: 'yura',
  joy: 'bright',
  amused: 'amused',
  sad: 'downcast',
  angry: 'sharp',
  surprised: 'startled',
  shy: 'shy',
  thinking: 'thinking',
  sleepy: 'sleepy',
  affection: 'warm',
};

export class Transcript {
  private streaming: HTMLElement | null = null;
  private streamingBody: HTMLElement | null = null;

  constructor(private root: HTMLElement) {}

  setVisible(visible: boolean): void {
    this.root.hidden = !visible;
  }

  addUser(text: string): void {
    this.root.append(this.build('user', text, 'you'));
    this.scroll();
  }

  addAssistant(text: string, emotion: Emotion = 'neutral'): void {
    if (!text.trim()) return;
    this.root.append(this.build('yura', text, EMOTION_LABEL[emotion], emotion));
    this.scroll();
  }

  beginStreaming(): void {
    const wrap = this.build('yura', '', EMOTION_LABEL.neutral, 'neutral');
    wrap.classList.add('streaming');
    this.streaming = wrap;
    this.streamingBody = wrap.querySelector('.msg-body');
    this.root.append(wrap);
    this.scroll();
  }

  appendDelta(visible: string): void {
    if (!this.streamingBody) return;
    this.streamingBody.textContent = visible;
    this.scroll();
  }

  setStreamingEmotion(emotion: Emotion): void {
    const meta = this.streaming?.querySelector('.msg-meta');
    if (!meta) return;
    const label = meta.querySelector('.msg-label');
    if (label) label.textContent = EMOTION_LABEL[emotion];
  }

  finishStreaming(text: string, emotion: Emotion = 'neutral'): void {
    if (!this.streaming) return;
    this.streaming.classList.remove('streaming');
    const body = this.streaming.querySelector('.msg-body');
    if (body) body.textContent = text;
    this.setStreamingEmotion(emotion);
    this.streaming = null;
    this.streamingBody = null;
    this.scroll();
  }

  addError(message: string): void {
    const wrap = this.build('yura', message, 'error');
    wrap.classList.add('error');
    this.root.append(wrap);
    this.scroll();
  }

  clear(): void {
    this.root.replaceChildren();
    this.streaming = null;
    this.streamingBody = null;
  }

  load(entries: { role: string; content: string }[]): void {
    this.clear();
    for (const entry of entries) {
      if (entry.role === 'user') this.addUser(entry.content);
      else if (entry.role === 'assistant') this.addAssistant(entry.content);
    }
  }

  private build(
    who: 'user' | 'yura',
    text: string,
    label: string,
    emotion?: Emotion,
  ): HTMLElement {
    const wrap = document.createElement('div');
    wrap.className = `msg ${who}`;

    const meta = document.createElement('div');
    meta.className = 'msg-meta';
    if (who === 'yura' && emotion && emotion !== 'neutral') {
      const dot = document.createElement('span');
      dot.className = 'msg-emotion';
      meta.append(dot);
    }
    const name = document.createElement('span');
    name.className = 'msg-label';
    name.textContent = label;
    meta.append(name);

    const body = document.createElement('div');
    body.className = 'msg-body';
    body.textContent = text;

    wrap.append(meta, body);
    return wrap;
  }

  private scroll(): void {
    this.root.scrollTop = this.root.scrollHeight;
  }
}

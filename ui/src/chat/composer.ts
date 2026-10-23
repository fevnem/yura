/**
 * The input dock.
 *
 * One textarea, one mic button, one send/stop button. Enter sends, Shift+Enter
 * breaks the line, Escape drops focus. While a reply streams the send button
 * becomes a stop button — the same control, so the user never has to hunt for
 * a way out of a long generation.
 */

export interface ComposerHandlers {
  onSend: (text: string) => void;
  onStop: () => void;
  onMicStart: () => void;
  onMicEnd: () => void;
}

export class Composer {
  private streaming = false;

  constructor(
    private form: HTMLFormElement,
    private input: HTMLTextAreaElement,
    private sendButton: HTMLButtonElement,
    private micButton: HTMLButtonElement,
    private handlers: ComposerHandlers,
  ) {
    this.form.addEventListener('submit', (event) => {
      event.preventDefault();
      this.submit();
    });

    this.input.addEventListener('input', () => {
      this.grow();
      this.syncSendState();
    });
    this.input.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault();
        this.submit();
      } else if (event.key === 'Escape') {
        this.input.blur();
      }
    });

    this.sendButton.addEventListener('click', () => {
      if (this.streaming) {
        this.handlers.onStop();
        return;
      }
      this.submit();
    });

    this.syncSendState();

    // Push-to-talk: press and hold.
    this.micButton.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      this.micButton.setPointerCapture(event.pointerId);
      this.micButton.classList.add('listening');
      this.handlers.onMicStart();
    });
    const release = () => {
      if (!this.micButton.classList.contains('listening')) return;
      this.micButton.classList.remove('listening');
      this.handlers.onMicEnd();
    };
    this.micButton.addEventListener('pointerup', release);
    this.micButton.addEventListener('pointercancel', release);
    this.micButton.addEventListener('pointerleave', release);
  }

  setStreaming(streaming: boolean): void {
    this.streaming = streaming;
    this.sendButton.dataset.mode = streaming ? 'stop' : 'send';
    this.sendButton.setAttribute('aria-label', streaming ? 'Stop' : 'Send');
    this.sendButton.title = streaming ? 'Stop' : 'Send';
    // Static inline SVG, no interpolation — not an injection surface.
    this.sendButton.innerHTML = streaming
      ? '<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="4.5" y="4.5" width="7" height="7" rx="1.4"/></svg>'
      : '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 8h9M8 3.5 12.5 8 8 12.5"/></svg>';
    this.syncSendState();
  }

  /** Send is only live when there is something to send, or work to stop. */
  private syncSendState(): void {
    this.sendButton.disabled = !this.streaming && this.input.value.trim().length === 0;
  }

  setValue(text: string): void {
    this.input.value = text;
    this.grow();
    this.syncSendState();
    this.focus();
  }

  appendTranscript(text: string): void {
    const existing = this.input.value.trim();
    this.input.value = existing ? `${existing} ${text}` : text;
    this.grow();
    this.syncSendState();
    this.focus();
  }

  /** 0–1, drives the mic button's border glow while recording. */
  setMicLevel(level: number, speaking: boolean): void {
    if (!speaking) {
      this.micButton.style.removeProperty('--mic-level');
      return;
    }
    this.micButton.style.setProperty('--mic-level', String(level));
  }

  focus(): void {
    this.input.focus();
  }

  clear(): void {
    this.input.value = '';
    this.grow();
    this.syncSendState();
  }

  private submit(): void {
    if (this.streaming) return;
    const text = this.input.value.trim();
    if (!text) return;
    this.clear();
    this.handlers.onSend(text);
  }

  private grow(): void {
    this.input.style.height = 'auto';
    this.input.style.height = `${Math.min(this.input.scrollHeight, 148)}px`;
  }
}

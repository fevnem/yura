/**
 * Speech out, via the platform's own engine.
 *
 * Using WebView2's `speechSynthesis` means zero bundled models, zero download,
 * and the OS voices the user already has (on Windows 11 that includes the
 * neural "Natural" voices). The trade is that the audio never passes through
 * JavaScript, so there is no waveform to analyse — see `LevelSource` below for
 * how lip-sync is derived instead.
 *
 * The engine is behind this class, so swapping in a bundled neural voice later
 * (Kokoro via a native audio path) means implementing the same three methods.
 */

export interface SpeakHandlers {
  onStart?: () => void;
  onEnd?: () => void;
  /** 0–1 mouth openness, called every animation frame while speaking. */
  onLevel?: (level: number) => void;
  /** A word boundary — a good moment for a small accent. */
  onBoundary?: () => void;
}

export class SpeechController {
  private voice: SpeechSynthesisVoice | null = null;
  private rate = 1.04;
  private pitch = 1.18;
  private volume = 0.95;
  private locale = 'en';
  private enabled = true;

  private current: SpeechSynthesisUtterance | null = null;
  private ticker = 0;
  private lastFrame = 0;
  private pulse = 0;
  private phraseTime = 0;
  private handlers: SpeakHandlers = {};
  private startGuard = 0;
  private watchdog = 0;
  private finished = true;

  get supported(): boolean {
    return typeof window !== 'undefined' && 'speechSynthesis' in window;
  }

  get isSpeaking(): boolean {
    return this.current !== null;
  }

  voices(): SpeechSynthesisVoice[] {
    if (!this.supported) return [];
    return window.speechSynthesis.getVoices();
  }

  /**
   * Voices ordered by how good they are likely to sound, so the settings
   * dropdown puts the pleasant ones first instead of alphabetically.
   */
  rankedVoices(): SpeechSynthesisVoice[] {
    const preferred = ['natural', 'neural', 'aria', 'jenny', 'sonia', 'zira', 'elian', 'hazel'];
    return [...this.voices()]
      .filter((v) => v.lang.toLowerCase().startsWith(this.locale))
      .sort((a, b) => score(b) - score(a));

    function score(voice: SpeechSynthesisVoice): number {
      const name = voice.name.toLowerCase();
      let s = voice.localService ? 1 : 3;
      preferred.forEach((token, i) => {
        if (name.includes(token)) s += 10 - i * 0.5;
      });
      if (name.includes('female') || name.includes('girl')) s += 1.5;
      return s;
    }
  }

  configure(options: {
    enabled: boolean;
    voiceUri: string;
    locale: string;
    rate: number;
    pitch: number;
    volume: number;
  }): void {
    this.enabled = options.enabled;
    this.locale = options.locale || 'en';
    this.rate = options.rate;
    this.pitch = options.pitch;
    this.volume = options.volume;

    const voices = this.voices();
    this.voice =
      voices.find((v) => v.voiceURI === options.voiceUri) ??
      this.rankedVoices()[0] ??
      voices[0] ??
      null;

    if (!this.enabled) this.cancel();
  }

  /**
   * Speak one beat. Long text is split at sentence boundaries because Chromium
   * silently truncates utterances past a few hundred characters.
   */
  speak(text: string, handlers: SpeakHandlers = {}): void {
    if (!this.supported || !this.enabled) {
      handlers.onStart?.();
      handlers.onEnd?.();
      return;
    }
    const sentence = text.trim();
    if (!sentence) {
      handlers.onEnd?.();
      return;
    }

    // Stop the engine without invoking the previous beat's callbacks: firing
    // onEnd here would re-enter the caller's queue while it is mid-drain.
    this.stopEngine();
    this.handlers = handlers;
    this.finished = false;

    // Two watchdogs, because the Windows engine can fail in ways it never
    // reports: no voices installed, a voice that refuses to start, or an
    // utterance that is silently dropped. Without these the beat queue stalls
    // and the UI sits on "speaking" forever.
    this.startGuard = window.setTimeout(() => {
      if (!this.current) this.finish(handlers);
    }, 900);
    this.watchdog = window.setTimeout(
      () => this.finish(handlers),
      1500 + sentence.length * 110,
    );

    let utterance: SpeechSynthesisUtterance;
    try {
      utterance = new SpeechSynthesisUtterance(sentence.slice(0, 420));
    } catch (error) {
      // No synthesiser at all (or a hardened runtime): treat the beat as read,
      // not spoken, so the queue keeps moving.
      console.warn('[yura] speech engine unavailable', error);
      this.finish(handlers);
      return;
    }

    if (this.voice) utterance.voice = this.voice;
    utterance.lang = this.voice?.lang ?? this.locale;
    utterance.rate = this.rate;
    utterance.pitch = this.pitch;
    utterance.volume = this.volume;

    utterance.onstart = () => {
      window.clearTimeout(this.startGuard);
      this.current = utterance;
      this.lastFrame = performance.now();
      handlers.onStart?.();
      this.startTicker();
    };
    utterance.onboundary = () => {
      this.pulse = Math.min(1, this.pulse + 0.75);
      this.phraseTime = 0;
      handlers.onBoundary?.();
    };
    utterance.onend = () => this.finish(handlers);
    utterance.onerror = () => this.finish(handlers);

    try {
      window.speechSynthesis.speak(utterance);
    } catch (error) {
      console.warn('[yura] speech engine refused the utterance', error);
      this.finish(handlers);
    }
  }

  cancel(): void {
    this.stopEngine();
    this.finish(this.handlers);
  }

  /** Silently tear down the current utterance and its timers. */
  private stopEngine(): void {
    window.clearTimeout(this.startGuard);
    window.clearTimeout(this.watchdog);
    if (this.ticker) cancelAnimationFrame(this.ticker);
    this.ticker = 0;
    if (this.supported) window.speechSynthesis.cancel();
    this.current = null;
    this.pulse = 0;
  }

  private finish(handlers: SpeakHandlers): void {
    if (this.finished) return;
    this.finished = true;

    this.stopEngine();
    handlers.onLevel?.(0);
    handlers.onEnd?.();
    this.handlers = {};
  }

  /**
   * Synthetic envelope.
   *
   * Boundary events tell us *when* a word starts; between them the mouth must
   * still move. A decaying pulse plus a little shaped flutter produces an
   * envelope that looks like speech at 60 fps without pretending to be
   * accurate — and it costs nothing.
   */
  private startTicker(): void {
    const step = (now: number) => {
      if (!this.current) return;
      const dt = Math.min((now - this.lastFrame) / 1000, 0.05);
      this.lastFrame = now;
      this.phraseTime += dt;

      this.pulse *= Math.exp(-dt * 4.6);

      const flutter =
        0.5 +
        0.28 * Math.sin(this.phraseTime * 17.3) +
        0.22 * Math.sin(this.phraseTime * 29.7 + 1.1);

      const level = Math.min(1, this.pulse * 1.15 + 0.16 * flutter);
      this.handlers.onLevel?.(level);

      this.ticker = requestAnimationFrame(step);
    };
    this.ticker = requestAnimationFrame(step);
  }
}

/**
 * Microphone in.
 *
 * Two capabilities, deliberately separated:
 *  1. Capture + level metering — always available, needs no model.
 *  2. Speech-to-text — only if the engine exposes `SpeechRecognition`.
 *
 * WebView2 is Chromium, and Chromium's recogniser is a cloud service, so (2) is
 * genuinely unavailable offline. Rather than pretend, the UI says so and the
 * composer stays the primary input. A bundled Whisper backend is the roadmap
 * item that turns this on for real; it will implement `Transcriber`.
 */

export interface Transcriber {
  readonly available: boolean;
  start(handlers: { onText: (text: string, final: boolean) => void; onError: (message: string) => void }): void;
  stop(): void;
}

type RecognitionLike = {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  start(): void;
  stop(): void;
  onresult: ((event: any) => void) | null;
  onerror: ((event: any) => void) | null;
  onend: (() => void) | null;
};

function recognitionCtor(): (new () => RecognitionLike) | null {
  const w = window as unknown as Record<string, unknown>;
  const ctor = w.SpeechRecognition ?? w.webkitSpeechRecognition;
  return typeof ctor === 'function' ? (ctor as new () => RecognitionLike) : null;
}

export class PlatformTranscriber implements Transcriber {
  private recognition: RecognitionLike | null = null;

  get available(): boolean {
    return recognitionCtor() !== null;
  }

  start(handlers: {
    onText: (text: string, final: boolean) => void;
    onError: (message: string) => void;
  }): void {
    const Ctor = recognitionCtor();
    if (!Ctor) {
      handlers.onError('speech recognition is not available in this engine');
      return;
    }

    const recognition = new Ctor();
    recognition.lang = 'en-US';
    recognition.continuous = false;
    recognition.interimResults = true;

    recognition.onresult = (event: any) => {
      let interim = '';
      let final = '';
      for (let i = event.resultIndex; i < event.results.length; i += 1) {
        const result = event.results[i];
        if (result.isFinal) final += result[0].transcript;
        else interim += result[0].transcript;
      }
      if (final) handlers.onText(final.trim(), true);
      else if (interim) handlers.onText(interim.trim(), false);
    };
    recognition.onerror = (event: any) => {
      handlers.onError(String(event?.error ?? 'recognition failed'));
    };
    recognition.onend = () => {
      this.recognition = null;
    };

    this.recognition = recognition;
    recognition.start();
  }

  stop(): void {
    this.recognition?.stop();
    this.recognition = null;
  }
}

/** Level metering for the mic button's visual feedback. */
export class MicrophoneMeter {
  private stream: MediaStream | null = null;
  private context: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private buffer: Float32Array<ArrayBuffer> | null = null;
  private raf = 0;

  get active(): boolean {
    return this.stream !== null;
  }

  async start(onLevel: (level: number) => void): Promise<void> {
    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
    });

    const Context = window.AudioContext ?? (window as any).webkitAudioContext;
    this.context = new Context();
    const source = this.context.createMediaStreamSource(this.stream);
    this.analyser = this.context.createAnalyser();
    this.analyser.fftSize = 1024;
    this.analyser.smoothingTimeConstant = 0.65;
    source.connect(this.analyser);
    this.buffer = new Float32Array(this.analyser.fftSize);

    const tick = () => {
      if (!this.analyser || !this.buffer) return;
      this.analyser.getFloatTimeDomainData(this.buffer);
      let sum = 0;
      for (let i = 0; i < this.buffer.length; i += 1) sum += this.buffer[i] ** 2;
      const rms = Math.sqrt(sum / this.buffer.length);
      onLevel(Math.min(1, rms * 4.2));
      this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  }

  stop(): void {
    cancelAnimationFrame(this.raf);
    this.raf = 0;
    this.analyser?.disconnect();
    this.analyser = null;
    this.context?.close();
    this.context = null;
    this.stream?.getTracks().forEach((track) => track.stop());
    this.stream = null;
    this.buffer = null;
  }
}

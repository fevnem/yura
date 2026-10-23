/**
 * The live readout: how full her memory is, how fast she is writing, and what
 * she is thinking while she writes it.
 *
 * The point is not decoration. A companion that shows its context budget and
 * its token rate is a tool you can reason about: you can see the moment a long
 * document eats the window, or a local model crawling at 4 tok/s.
 */

const TPS_WINDOW_MS = 900;

export class Telemetry {
  private streamStart = 0;
  private charsSinceStart = 0;
  private windowStart = 0;
  private windowChars = 0;
  private thinkingBuffer = '';
  private hideTimer = 0;
  private rafTimer = 0;

  constructor(
    private ctxEl: HTMLElement,
    private ctxBar: HTMLElement,
    private ctxText: HTMLElement,
    private tpsEl: HTMLElement,
    private tpsText: HTMLElement,
    private thinkingEl: HTMLElement,
    private thinkingText: HTMLElement,
  ) {}

  /** Tokens are estimated at ~4 characters each; that is honest enough to meter. */
  static tokens(chars: number): number {
    return Math.round(chars / 4);
  }

  setContext(tokens: number, window: number): void {
    const safeWindow = Math.max(1, window);
    const pct = Math.min(100, (tokens / safeWindow) * 100);
    this.ctxBar.style.width = `${pct.toFixed(1)}%`;
    this.ctxBar.dataset.level = pct > 85 ? 'hot' : pct > 60 ? 'warm' : 'cool';
    this.ctxText.textContent = `${formatTokens(tokens)} / ${formatTokens(safeWindow)}`;
    this.ctxEl.title = `${tokens.toLocaleString()} of ${safeWindow.toLocaleString()} context tokens used (${pct.toFixed(1)}%)`;
  }

  startStreaming(): void {
    window.clearTimeout(this.hideTimer);
    this.streamStart = performance.now();
    this.windowStart = this.streamStart;
    this.charsSinceStart = 0;
    this.windowChars = 0;
    this.tpsEl.hidden = false;
    this.tpsText.textContent = '0 tok/s';
    this.thinkingBuffer = '';
    this.thinkingText.textContent = '';
    this.thinkingEl.hidden = true;
    this.thinkingEl.dataset.live = 'true';
  }

  addText(delta: string): void {
    this.charsSinceStart += delta.length;
    this.windowChars += delta.length;
    this.publishRate(false);
  }

  addThinking(delta: string): void {
    this.thinkingBuffer += delta;
    // Keep the tail: the interesting part of a scratchpad is what it is
    // currently working on, not where it started.
    if (this.thinkingBuffer.length > 600) {
      this.thinkingBuffer = this.thinkingBuffer.slice(-600);
    }
    this.thinkingText.textContent = this.thinkingBuffer.trim();
    this.thinkingEl.hidden = false;
  }

  /** Called when the stream ends: freezes the rate and fades the scratchpad. */
  finishStreaming(): void {
    this.publishRate(true);
    this.thinkingEl.dataset.live = 'false';
    if (this.thinkingText.textContent) {
      this.hideTimer = window.setTimeout(() => {
        this.thinkingEl.hidden = true;
        this.thinkingText.textContent = '';
      }, 2600);
    }
    window.clearTimeout(this.rafTimer);
  }

  cancel(): void {
    window.clearTimeout(this.hideTimer);
    this.tpsEl.hidden = true;
    this.thinkingEl.hidden = true;
    this.thinkingText.textContent = '';
  }

  private publishRate(final: boolean): void {
    const now = performance.now();
    const elapsed = now - this.windowStart;
    if (!final && elapsed < TPS_WINDOW_MS) return;

    const tokens = Telemetry.tokens(this.windowChars);
    const rate = elapsed > 0 ? (tokens / elapsed) * 1000 : 0;

    if (final) {
      const total = now - this.streamStart;
      const overall = total > 0 ? (Telemetry.tokens(this.charsSinceStart) / total) * 1000 : rate;
      this.tpsText.textContent = `${overall.toFixed(0)} tok/s`;
    } else {
      this.tpsText.textContent = `${rate.toFixed(0)} tok/s`;
      this.windowStart = now;
      this.windowChars = 0;
      this.rafTimer = window.setTimeout(() => this.publishRate(false), TPS_WINDOW_MS);
    }
  }
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1000) return `${(n / 1000).toFixed(n >= 10_000 ? 0 : 1)}k`;
  return String(Math.max(0, Math.round(n)));
}

/**
 * The landing screen.
 *
 * A new conversation is the one screen with nothing to read, so it has to do
 * the teaching: who this is, what she is for, and four doors in. Suggestions are
 * concrete study tasks, not "ask me anything" — a blank box is a worse prompt
 * than a good example.
 */

const SUGGESTIONS: { label: string; prompt: string }[] = [
  {
    label: 'Explain a hard idea simply',
    prompt: 'Explain the hardest idea I am studying right now as if I have five minutes and no background. Ask me one question first.',
  },
  {
    label: 'Quiz me on what we covered',
    prompt: 'Quiz me on what we have covered so far. One question at a time, and tell me why each answer is right or wrong.',
  },
  {
    label: 'Digest a paper',
    prompt: 'I am going to paste a paper abstract. Give me the claim, the method, what the evidence actually supports, and the strongest objection.',
  },
  {
    label: 'Plan my week',
    prompt: 'Help me plan a week of study. Ask me what is due, then build a realistic day-by-day plan with rest built in.',
  },
];

export class Welcome {
  constructor(
    private root: HTMLElement,
    private onPick: (prompt: string) => void,
  ) {
    this.render();
  }

  show(): void {
    this.root.hidden = false;
  }

  hide(): void {
    this.root.hidden = true;
  }

  get visible(): boolean {
    return !this.root.hidden;
  }

  private render(): void {
    this.root.replaceChildren();

    const block = document.createElement('div');
    block.className = 'welcome-inner';

    const mark = document.createElement('span');
    mark.className = 'welcome-mark';

    const hello = document.createElement('h1');
    hello.className = 'welcome-title';
    hello.textContent = greeting();

    const sub = document.createElement('p');
    sub.className = 'welcome-sub';
    sub.textContent = 'What are we working through? Ask, paste a page, or start from one of these.';

    const grid = document.createElement('div');
    grid.className = 'welcome-grid';

    for (const item of SUGGESTIONS) {
      const card = document.createElement('button');
      card.type = 'button';
      card.className = 'welcome-card';
      card.textContent = item.label;
      card.addEventListener('click', () => this.onPick(item.prompt));
      grid.append(card);
    }

    block.append(mark, hello, sub, grid);
    this.root.append(block);
  }
}

function greeting(): string {
  const hour = new Date().getHours();
  if (hour < 5) return 'Still awake.';
  if (hour < 12) return 'Good morning.';
  if (hour < 18) return 'Good afternoon.';
  return 'Good evening.';
}

import type { ChatSummary } from '../bridge/types';

/**
 * The conversation rail.
 *
 * Grouped the way people actually think about chats: pinned first, then recent.
 * Rows are deliberately quiet — one line of title, one line of preview — and the
 * per-row actions only appear on hover, so a long list stays calm.
 *
 * Renaming happens in place: the title becomes an input, Enter commits, Escape
 * cancels. No modal, no dialog, no lost context.
 */

export interface SidebarHandlers {
  onSelect: (id: string) => void;
  onNew: () => void;
  onRename: (id: string, title: string) => void;
  onDelete: (id: string) => void;
  onPin: (id: string, pinned: boolean) => void;
}

export class ChatSidebar {
  private chats: ChatSummary[] = [];
  private activeId: string | null = null;
  private editing: string | null = null;
  private openMenu: string | null = null;

  constructor(
    private root: HTMLElement,
    private handlers: SidebarHandlers,
  ) {
    document.addEventListener('click', (event) => {
      if (this.openMenu && !(event.target as HTMLElement).closest('.chat-menu, .chat-more')) {
        this.openMenu = null;
        this.render();
      }
    });
  }

  setChats(chats: ChatSummary[], activeId: string | null): void {
    this.chats = chats;
    this.activeId = activeId;
    this.render();
  }

  setActive(activeId: string | null): void {
    this.activeId = activeId;
    this.render();
  }

  render(): void {
    this.root.replaceChildren();

    const head = document.createElement('div');
    head.className = 'chats-head';

    const label = document.createElement('span');
    label.className = 'chats-title';
    label.textContent = 'Conversations';

    const add = document.createElement('button');
    add.type = 'button';
    add.className = 'chats-new';
    add.title = 'New conversation';
    add.setAttribute('aria-label', 'New conversation');
    add.append(icon('<path d="M8 3.4v9.2M3.4 8h9.2"/>'));
    add.addEventListener('click', () => this.handlers.onNew());

    head.append(label, add);
    this.root.append(head);

    const list = document.createElement('div');
    list.className = 'chats-list';

    const pinned = this.chats.filter((c) => c.pinned);
    const rest = this.chats.filter((c) => !c.pinned);

    if (pinned.length) {
      list.append(this.groupLabel('Pinned'));
      pinned.forEach((chat) => list.append(this.row(chat)));
    }

    list.append(this.groupLabel(rest.length ? 'Recent' : 'Nothing yet'));

    if (!rest.length && !pinned.length) {
      const empty = document.createElement('p');
      empty.className = 'chats-empty';
      empty.textContent = 'Start a conversation — she keeps every one of them.';
      list.append(empty);
    }

    rest.forEach((chat) => list.append(this.row(chat)));

    this.root.append(list);
  }

  /* ── pieces ────────────────────────────────────────────────── */

  private groupLabel(text: string): HTMLElement {
    const node = document.createElement('div');
    node.className = 'chats-group';
    node.textContent = text;
    return node;
  }

  private row(chat: ChatSummary): HTMLElement {
    const row = document.createElement('div');
    row.className = 'chat-row';
    row.dataset.active = String(chat.id === this.activeId);
    row.dataset.pinned = String(chat.pinned);

    if (this.editing === chat.id) {
      const input = document.createElement('input');
      input.className = 'chat-rename';
      input.value = chat.title;
      input.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') {
          this.handlers.onRename(chat.id, input.value);
          this.editing = null;
          this.render();
        } else if (event.key === 'Escape') {
          this.editing = null;
          this.render();
        }
      });
      input.addEventListener('blur', () => {
        if (this.editing === chat.id) {
          this.handlers.onRename(chat.id, input.value);
          this.editing = null;
          this.render();
        }
      });
      row.append(input);
      queueMicrotask(() => {
        input.focus();
        input.select();
      });
      return row;
    }

    const main = document.createElement('button');
    main.type = 'button';
    main.className = 'chat-main';
    main.addEventListener('click', () => this.handlers.onSelect(chat.id));

    const title = document.createElement('span');
    title.className = 'chat-title';
    title.textContent = chat.title || 'Untitled';

    const preview = document.createElement('span');
    preview.className = 'chat-preview';
    preview.textContent = chat.preview || `${chat.turns} turns`;

    main.append(title, preview);

    const more = document.createElement('button');
    more.type = 'button';
    more.className = 'chat-more';
    more.setAttribute('aria-label', 'Conversation options');
    more.append(icon('<circle cx="3.2" cy="8" r="1.1"/><circle cx="8" cy="8" r="1.1"/><circle cx="12.8" cy="8" r="1.1"/>'));
    more.addEventListener('click', (event) => {
      event.stopPropagation();
      this.openMenu = this.openMenu === chat.id ? null : chat.id;
      this.render();
    });

    row.append(main, more);

    if (this.openMenu === chat.id) {
      const menu = document.createElement('div');
      menu.className = 'chat-menu';

      const rename = menuItem(chat.pinned ? 'Unpin' : 'Pin', () => {
        this.handlers.onPin(chat.id, !chat.pinned);
        this.openMenu = null;
        this.render();
      });
      // first entry pins; renaming is its own entry below
      const doRename = menuItem('Rename', () => {
        this.editing = chat.id;
        this.openMenu = null;
        this.render();
      });
      const del = menuItem('Delete', () => {
        this.openMenu = null;
        this.handlers.onDelete(chat.id);
      }, 'danger');

      menu.append(rename, doRename, del);
      row.append(menu);
    }

    return row;
  }
}

function menuItem(text: string, onPick: () => void, tone = ''): HTMLButtonElement {
  const b = document.createElement('button');
  b.type = 'button';
  b.className = `chat-menu-item ${tone}`.trim();
  b.textContent = text;
  b.addEventListener('click', (event) => {
    event.stopPropagation();
    onPick();
  });
  return b;
}

function icon(paths: string): SVGElement {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 16 16');
  svg.setAttribute('aria-hidden', 'true');
  // Static path data from this module only — never user content.
  svg.innerHTML = paths;
  return svg;
}

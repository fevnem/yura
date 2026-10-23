import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/**
 * The agent protocol.
 *
 * She asks for a tool the way she says anything else — in the reply — using a
 * fenced block:
 *
 *     ```yura:run_python
 *     {"code": "print(2 + 2)"}
 *     ```
 *
 * Plain text means it works with every provider, including local models with no
 * native tool-calling, and the user can read the request before approving it.
 */

export interface ToolCall {
  name: string;
  args: Record<string, unknown>;
  raw: string;
}

export interface ToolOutcome {
  ok: boolean;
  output: string;
}

const BLOCK = /```yura:([a-z_]+)\s*\n([\s\S]*?)```/g;

/** Split a reply into the prose the user sees and the calls she is asking for. */
export function extractToolCalls(text: string): { clean: string; calls: ToolCall[] } {
  const calls: ToolCall[] = [];
  let clean = text;

  for (const match of text.matchAll(BLOCK)) {
    const [raw, name, body] = match;
    let args: Record<string, unknown> = {};
    try {
      args = JSON.parse(body.trim());
    } catch {
      // A malformed block is still a request; run_tool reports the bad args.
      args = {};
    }
    calls.push({ name, args, raw });
    clean = clean.replace(raw, '');
  }

  return { clean: clean.replace(/\n{3,}/g, '\n\n').trim(), calls };
}

export async function runTool(name: string, args: Record<string, unknown>): Promise<ToolOutcome> {
  try {
    return await invoke<ToolOutcome>('run_tool', { name, args });
  } catch (error) {
    return { ok: false, output: error instanceof Error ? error.message : String(error) };
  }
}

/** One line the transcript can print for a tool run. */
export function describe(name: string, outcome: ToolOutcome): string {
  const verb: Record<string, string> = {
    run_python: 'ran python',
    read_file: 'read a file',
    write_file: 'wrote a file',
    list_files: 'listed the workspace',
    set_timer: 'set a timer',
    add_task: 'added a task',
    list_tasks: 'read the task list',
    complete_task: 'closed a task',
    notify: 'sent a notification',
  };
  const label = verb[name] ?? name;
  const verdict = outcome.ok ? label : `${label} — refused`;
  const first = outcome.output.split('\n')[0]?.slice(0, 90) ?? '';
  return first ? `${verdict} · ${first}` : verdict;
}

export interface ApprovalRequest {
  id: string;
  tool: string;
  summary: string;
  detail: string;
  kind: string;
}

/**
 * Approve/deny cards, drawn into the transcript as they arrive.
 *
 * The card is deliberately plain: what will run, in full, and two buttons. It
 * resolves itself when answered so a stale card cannot be clicked twice.
 */
export function bindApprovals(root: HTMLElement): () => void {
  let dispose: (() => void) | null = null;

  void listen<ApprovalRequest>('approval:request', (event) => {
    const request = event.payload;

    const card = document.createElement('div');
    card.className = 'approval';
    card.dataset.kind = request.kind;

    const title = document.createElement('p');
    title.className = 'approval-title';
    title.textContent = request.summary;

    const detail = document.createElement('pre');
    detail.className = 'approval-detail';
    detail.textContent = request.detail.slice(0, 1200);

    const row = document.createElement('div');
    row.className = 'approval-row';

    const approve = document.createElement('button');
    approve.type = 'button';
    approve.className = 'approval-yes';
    approve.textContent = 'Approve';

    const deny = document.createElement('button');
    deny.type = 'button';
    deny.className = 'approval-no';
    deny.textContent = 'Deny';

    const settle = async (approved: boolean) => {
      approve.disabled = true;
      deny.disabled = true;
      card.classList.add(approved ? 'is-approved' : 'is-denied');
      await invoke('respond_approval', { id: request.id, approved });
    };

    approve.addEventListener('click', () => void settle(true));
    deny.addEventListener('click', () => void settle(false));

    row.append(approve, deny);
    card.append(title, detail, row);
    root.append(card);
    root.scrollTop = root.scrollHeight;
  }).then((unlisten) => {
    dispose = unlisten;
  });

  return () => dispose?.();
}

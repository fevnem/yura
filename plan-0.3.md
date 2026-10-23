# Plan — 0.3 and beyond

Everything requested but not yet shipped, in the order it should be built, with
the design decided in advance so implementation is mechanical.

Guiding rule: **no feature ships without a way to see it fail.** Each item lists
how it is verified, because "it compiles" has already cost us a T-pose and a
transcript that could not scroll.

---

## 1. Abilities — code execution with an approval gate

**Why first:** it is the difference between a chat companion and a tool. It is
also the only feature that can damage the user's machine, so it needs the most
care and the least cleverness.

### Design

- Settings → **Abilities**
  - `Run Python` (off by default)
  - `Workspace folder` — one directory she may read and write. Nothing outside
    it, ever.
  - `Ask before running` (on, and not settable off for file writes)
- **Tool loop.** The model gets real tools, not prompt tricks:
  `run_python`, `read_file`, `write_file`, `list_dir`. Implemented once in
  `yura-llm` as an `openai` `tools` payload plus Anthropic `tools`, with a
  bounded loop (max 6 round trips) so a confused model cannot spin.
- **Approval.** Any tool that writes, or any `run_python`, emits
  `approval:request { id, tool, preview }` and waits on a `oneshot` channel.
  The chat shows a card: *what* it will run, *where*, and **Approve / Deny**.
  Denial returns a tool result of `"user denied"` so the model can adapt instead
  of hanging.
- **Execution.** `python -I -c <script>` with a 20 s timeout, cwd = workspace,
  stdout/stderr captured and truncated to 8 KB. `-I` (isolated) so the user's
  `PYTHONPATH` and site-packages cannot leak into the run.

### Files
`crates/yura-tools/` (new: `exec.rs`, `tools.rs`, `approval.rs`),
`crates/yura-llm/src/tools.rs`,
`ui/src/abilities/approval.ts`, settings section `abilities`.

### Verified by
A denied run must leave no file behind; a timed-out run must not hang the UI;
a 10 MB `print` must truncate instead of freezing the webview.

---

## 2. Resources — downloadable voices

**Decision:** ship no TTS weights in the installer. The Windows voices stay the
zero-download default; anything neural is a *resource* the user chooses.

- Settings → **Resources**, a list of installable packs:
  | Pack | Size | Notes |
  |---|---|---|
  | Windows system voices | 0 | already installed, offline |
  | **MOSS-TTS-Nano** | ~200 MB | [OpenMOSS/MOSS-TTS-Nano](https://github.com/OpenMOSS/MOSS-TTS-Nano), ONNX, cute/expressive |
  | Kokoro-82M | 86 MB | 54 voices, Apache-2.0 |
  | Whisper tiny / base | 75 / 142 MB | speech to text |
- Each row: state (not installed / downloading / ready / failed), a progress
  bar, and **Download** / **Remove**. Downloads go to
  `%APPDATA%\Yura\resources\<pack>\` with a SHA-256 check and resumable HTTP.
- The voice picker merges engine voices and installed packs. `TtsBackend` trait
  already exists in spirit: `speak(text) -> audio`, so MOSS and Kokoro become two
  implementations behind one seam, and real audio finally gives us FFT-driven
  lip-sync instead of an envelope.

### Files
`crates/yura-resources/` (manifest, download, verify), `ui/src/settings/resources.ts`,
`crates/yura-voice/` (new, the `TtsBackend` trait + system backend).

### Verified by
Kill a download mid-flight: resuming must not produce a corrupt file; the
checksum must reject a tampered pack.

---

## 3. Library — a folder she lives in

- Settings → **Library**: pick a folder with the native picker. Persisted as
  `library_path`. It doubles as the Abilities workspace.
- Commands: `list_library`, `read_library_file`, `write_library_file` —
  **path-sandboxed**: canonicalise, then reject anything not under the root.
- A bundled **librarian** skill: index the folder, answer from it, and when
  asked to save a note, write a markdown file with front matter.
- UI: a small library drawer listing files, with drop-to-add.

### Verified by
A path with `..` must be refused; a symlink pointing outside the root must be
refused after canonicalisation.

---

## 4. MCP servers

- Settings → **MCP**: add a server by
  - `command` + args (stdio), or
  - `url` (HTTP/SSE), plus optional headers.
- Store in `%APPDATA%\Yura\mcp.json`. Each server row shows status, tool count,
  and a **Connect / Disconnect** button.
- `crates/yura-mcp/`: a minimal JSON-RPC client — `initialize`, `tools/list`,
  `tools/call`, `notifications/tools/list_changed`. No SDK, no async runtime of
  its own; it rides the tokio we already have.
- Tools from connected servers join the built-in tools in the same loop, namespaced
  `server__tool`, and every call goes through the same approval gate.

### Verified by
A server that never answers must time out; a crashed stdio child must be
reported and restartable.

---

## 5. Presence — timers, reminders, tray, startup

- **Tools:** `set_timer(seconds, label)`, `add_task(text)`, `list_tasks`,
  `complete_task(id)`, `notify(title, body)` — the last one always asks first.
- **Scheduler:** one tokio task owning a `BinaryHeap<Instant>`; firing emits
  `reminder:due { id, label }` and a native notification via
  `tauri-plugin-notification`.
- **Tray:** icon + menu (Show, New conversation, Quit). Closing the window
  hides to tray when "Stay in tray" is on.
- **Startup:** `tauri-plugin-autostart`, toggled from Settings → Window.
- A **water ping** is just a default timer the user can enable: every 45 minutes,
  a quiet nudge.

### Verified by
A reminder set for 5 s must fire once, not twice; quitting from the tray must
not leave an orphan process.

---

## 6. Attachments and OCR

In flight: `crates/yura-docs` extracts PDF text and sniffs images by magic
bytes. Once it lands:
- Composer gets a paperclip; drag-and-drop onto the window works.
- PDFs become a prompt block; images attach as `data:` parts for multimodal
  models.
- **OCR** only where it is honest: rasterise → recognise. Pure-Rust OCR is not
  realistic at a size we would ship, so the plan is a WASM recogniser
  (`tesseract.js`) loaded on demand as a *resource*, never in the installer.

---

## 7. Design debt

- Settings: section headers are still a touch quiet; add a left icon column and
  a sticky section title.
- Sidebar: row hover reveal is invisible on touch; add a persistent ⋯ on the
  active row.
- Character: verify the A-pose against a second VRM (the sign of the shoulder
  rotation is model-dependent); fall back to a rest pose read from the file when
  available.

---

## Order of work

1. Attachments (landing) → 2. Abilities + approval → 3. Library →
4. Resources (MOSS/Kokoro/Whisper) → 5. Timers/tray/startup → 6. MCP → 7. polish.

Rationale: attachments make it a study tool today; abilities make it an agent;
library gives it a home; resources fix the voice; presence makes it a companion;
MCP is leverage on everything after.

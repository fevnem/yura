# Changelog

All notable changes to Yura. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [SemVer](https://semver.org/).

## [0.4.0] — 2026-10-23

### Added — MCP servers

- **Settings → MCP.** Add a server by name, command and arguments; connect it and
  the tool count appears. Servers are child processes speaking newline-delimited
  JSON-RPC on stdio — the protocol as it actually is, no SDK.
- **`list_mcp_tools` and `call_mcp`** join the agent protocol, so she can discover
  what a server offers and use it. Every MCP call goes through the same
  approve/deny card as Python, showing the exact arguments.
- Connections are owned by the app, not the settings page: switching sections or
  chats does not drop them, and removing a server kills its process.
- A server that fails to start, dies mid-handshake, or answers with an error is
  reported in plain words instead of hanging the UI.

### Not included

- HTTP/SSE transports, MCP resources and prompts. Tools over stdio is the part
  that makes an agent useful; the rest is designed in `docs/plan-0.3.md`.

## [0.3.0] — 2026-10-23

She can act.

### Added — abilities

- **Python.** Off by default. When on she writes and runs scripts in a workspace
  folder you choose: 20 s limit, `-I` isolation, output capped, hard kill on
  timeout.
- **Files.** Read, write and list inside that folder only. Paths are checked
  lexically *and* after canonicalisation, so `..` and escaping symlinks are both
  refused.
- **Approve / deny in the chat.** Every script, write and notification produces a
  card showing exactly what will happen. No answer in two minutes means no.
- **The agent protocol is plain text** — a fenced `yura:run_python` block — so it
  works with any provider, including local models with no tool-calling.
- **Timers, tasks, notifications.** `set_timer`, `add_task`, `list_tasks`,
  `complete_task`, `notify`. Timers survive a closed window and fire once.
- **Tray and start-on-login**, with close-to-tray.
- **Attachments** read through `yura-docs`: PDF text, magic-byte sniffing, and an
  honest "this looks like a scan" when there is no text layer.

### Added — settings

- **Abilities** — Python, file access, workspace folder, approval switch.
- **Library** — the files in that folder, and the data directory.
- **Resources** — voice and speech models are downloaded, never bundled:
  MOSS-TTS-Nano, Kokoro-82M, Whisper tiny. Interrupted downloads never become
  files.
- **General** — tray, close-to-tray, start on login, notifications.

### Changed

- The system prompt describes only the tools the user has enabled.
- Side effects (tray visibility, autostart) apply immediately, not on restart.

### Fixed

- A flaky ordering test that assumed millisecond resolution between creations.

### Known limits

- **MCP servers** are designed (`docs/plan-0.3.md` §4) but not in this build.
- Whisper downloads as a resource, but the microphone is not wired to it yet.

## [0.2.0] — 2026-10-23

Repositioned as a **study and research companion**, and given the structure a
working tool needs: many conversations, user-authored skills, and a live readout
of what she is doing.

### Added

**Conversations**
- A conversation rail: every chat is its own file, pinned first, newest next.
- Switch between chats without losing a draft, rename in place, pin, delete.
- Titles come from the first thing you say, and stay editable afterwards.
- The active chat is the model's context — no more one endless log.

**Skills**
- Skills are markdown files in `%APPDATA%\Yura\skills\`. Write one in Settings,
  or import any `.md` you already have.
- Enabled skills are compiled into the system prompt on every request; toggling
  one off is instant and does not delete it.
- Three starter skills ship installed: *Socratic tutor*, *Paper digest*,
  *Recall drill*.
- Format is optional front matter (`name`, `description`, `enabled`) plus
  instructions — a plain note dropped in the folder works too.

**Live telemetry**
- Context meter: tokens used against the model's window, warm at 60%, hot at 85%.
- Tokens per second, measured over a sliding window while she writes.
- A thinking strip that shows a reasoning model's scratchpad as it arrives
  (DeepSeek-R1, QwQ, Claude extended thinking) — never spoken, never saved.

**Character**
- Ships with a real anime avatar: **Sendagaya Shino**, the VRM Consortium's own
  sample, CC0 — bundled, attributed, and loading by default.
- Relaxed A-pose at load instead of the T-pose the file ships in.
- Expression lookup is case-insensitive, so models with custom groups still emote.

**Settings**
- A real settings page, not a slide-over: categories listed, each opening
  underneath itself.
- Six sections — Model, Persona, Skills, Voice, Avatar, Appearance.
- Themes: Midnight, Dusk, Ember, Orchid, Mono. Each one is a room, not a filter.
- "Test voice" speaks with the current slider values without saving.

**Window**
- Maximise button, and double-click on the title bar.
- The conversation rail can be collapsed.

### Fixed

- **Scrolling the transcript never worked.** `justify-content: flex-end` on a
  scroll container puts overflow above the scroll origin, where the wheel cannot
  reach it. Bottom-anchoring is now `margin-top: auto` on the first message.
- The speech queue could hang forever if the engine never fired `onstart` (no
  voices installed). Two watchdogs now guarantee the beat advances.
- Send was live on an empty composer.
- A dust mote near the camera rendered as a hard white block (unclamped
  `gl_PointSize`); motes also no longer cross the character's silhouette.

## [0.1.0] — 2026-10-23

First release: a window, a face, a voice, and a mind you supply.

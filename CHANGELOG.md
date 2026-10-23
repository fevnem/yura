# Changelog

All notable changes to Yura. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [SemVer](https://semver.org/).

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

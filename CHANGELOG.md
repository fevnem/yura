# Changelog

All notable changes to Yura. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [SemVer](https://semver.org/).

## [0.1.0] — 2026-10-23

The first shippable build: a window, a face, a voice, a mind you supply.

### Added

**Shell**
- Frameless Windows window with custom chrome, drag region, pin-on-top,
  minimise and close; window state restored from settings.
- NSIS installer (`Yura_0.1.0_x64-setup.exe`), cross-compiled from Linux.
- Single instance, ~7 MB binary, no bundled browser.

**Conversation**
- Streaming replies token by token over Tauri events.
- Provider abstraction: one `LlmProvider` trait, two protocol implementations
  (OpenAI-compatible and Anthropic) covering eleven providers out of the box.
- Provider, endpoint, API key, model, temperature, max tokens and memory depth
  all editable in Settings and persisted atomically.
- Local history in `%APPDATA%\Yura\history.jsonl`, replayed into the
  transcript on launch and used as model context.
- Stop mid-generation; the partial reply is kept rather than discarded.

**Character**
- VRM 0.x / 1.0 loader with automatic height fitting and grounding.
- Idle animation: chest breathing, spine sway, jittered blinks with occasional
  double blinks.
- Gaze: eyes track the cursor, head follows at a configurable fraction, with
  autonomous drift when the cursor is still.
- Emotion system: model tags each beat (`[joy]`, `[shy]`, …) and the face
  cross-fades between ten expressions with damping.
- Lip-sync driven by speech word boundaries plus a shaped envelope.
- Procedural "spirit form" when no avatar file is present.

**Voice**
- Offline speech through the platform engine (Windows neural voices included).
- Voice picker ranked by likely quality instead of alphabetically.
- Push-to-talk microphone with level metering; offline recognition is detected
  and reported honestly when the engine does not provide it.

**Look**
- Shader backdrop with drifting fBm noise and a warm pool behind the character.
- Three-point lighting; the rim brightens with speech energy.
- Post stack: MSAA target, Unreal bloom, grade pass with vignette, film grain
  and a touch of chromatic aberration.
- Depth-clamped dust motes that avoid the character's silhouette.
- Accent hue drives the entire palette from one number.

### Known limits

- Speech recognition is unavailable offline in WebView2; text input is primary
  until the bundled Whisper backend lands.
- Only one character at a time; the picker is roadmap.
- WebView2 CSP is currently unrestricted for first-run reliability.

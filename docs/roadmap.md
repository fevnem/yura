# Roadmap

Ordered by what a user would notice first, not by what is easiest.

## 0.1 — the shippable core (this build)

- [x] Frameless Windows window, pin-on-top, custom chrome
- [x] Streaming chat against any OpenAI-compatible endpoint + Anthropic
- [x] Provider/endpoint/key/model in Settings, persisted atomically
- [x] Emotion-tagged replies, parsed while streaming
- [x] 3D avatar: VRM loader, idle breathing, blink, gaze, head tracking
- [x] Lip-sync from the speech engine's boundary events
- [x] Procedural spirit form when no VRM is present
- [x] Offline TTS through the OS voices
- [x] Bloom, grain, vignette, drifting motes, parallax
- [x] Local history with per-turn recall

## 0.2 — voice becomes real

- [ ] **Bundled neural TTS (Kokoro-82M, 86 MB quantised).** Real audio means a
      real waveform, which means FFT-driven visemes instead of an envelope.
      Runs through `sherpa-onnx` as a sidecar process, keeping the C++ out of
      the main binary.
- [ ] **Offline STT (Whisper tiny/base via whisper.cpp).** Push-to-talk that
      actually transcribes without a cloud round trip.
- [ ] Voice mixer: per-character pitch, breath, and a room tone bed.
- [ ] Barge-in: speaking while she is talking cuts her off mid-word.

## 0.3 — a character, not a mannequin

- [ ] Animation clips: greet, listen, think, laugh, wave (VRMA playback).
- [ ] Expression blending from *text sentiment* between tagged beats, so long
      replies do not hold one face.
- [ ] Character picker with per-character persona and voice, first-class.
- [ ] Outfit/material variants via VRM material overrides.
- [ ] Spring-bone tuning per model, stored alongside the model choice.

## 0.4 — presence

- [ ] Always-on-top "corner" mode: a small, chrome-less 320×420 window.
- [ ] Global hotkey to open the composer from anywhere.
- [ ] Idle behaviours on a schedule: stretches, looks around, comments on the
      clock. Never interrupts.
- [ ] Do-not-disturb rules tied to fullscreen apps.
- [ ] Startup with Windows, minimised to tray.

## 0.5 — memory that earns its name

- [ ] SQLite backend behind the existing `History` trait.
- [ ] Summarised long-term memory: facts about the user, not a raw transcript.
- [ ] Explicit "remember this" and a small memory viewer/editor.
- [ ] Per-conversation threads instead of one endless log.

## Platform & hardening

- [ ] Tighten the WebView2 CSP (currently `null` for first-run reliability) and
      audit the capability list.
- [ ] Code signing for the NSIS installer (removes the SmartScreen warning).
- [ ] Optional CI: `cargo test` + `tsc --noEmit` + a Linux dev build.
- [ ] Crash reporting that is opt-in and local-only.

## Deliberately out of scope

- Accounts, sync, or any server of ours.
- Telemetry of any kind.
- Shipping third-party avatars inside the installer (licences forbid it — they
  are downloaded on the user's machine).
- A mobile build. The interaction model does not survive the translation.

# Yura

A lightweight AI companion for Windows. A 3D anime character who listens,
speaks and types back — running as a ~7 MB native window with no Electron, no
bundled browser, and no telemetry.

```
Rust (Tauri 2)  ──  window, LLM streaming, settings, history
WebView2        ──  three.js stage, VRM avatar, glass UI
```

Yura ships with no model weights. You point her at a provider — a cloud API,
Ollama on localhost, or any OpenAI-compatible endpoint — and she comes alive.

---

## What it does

- **Real 3D character.** Loads any `.vrm` (VRM 0.x or 1.0). Breathing, blinking,
  saccadic eye movement, head tracking toward your cursor, and lip-sync that
  follows her voice. Without a model file she appears as a designed spirit form.
- **Talks and types.** Replies stream token by token. Each reply is split into
  emotional beats (`[joy]`, `[shy]`, …) that drive her face mid-sentence.
- **Speaks offline.** Uses the Windows speech engine through WebView2 — the
  installed neural voices, no download, no network.
- **Any provider.** OpenAI, DeepSeek, OpenRouter, Groq, Mistral, Together, xAI,
  Anthropic, Ollama, LM Studio, or a custom base URL. One trait, two protocol
  implementations.
- **Remembers locally.** Settings and conversation history live in
  `%APPDATA%\Yura` as readable JSON. Nothing leaves the machine except the API
  call you configured.

## Build

Prerequisites: Node 20+, and either Windows with the Rust MSVC toolchain or a
Linux box cross-compiling with `cargo-xwin`.

```bash
npm install                     # task runner + Tauri CLI
npm --prefix ui install         # renderer dependencies

npm run avatar:fetch            # optional: download a default character
npm run tauri:dev               # run with hot reload
npm run build:windows           # NSIS installer → cross-compiled .exe
```

Output: `target/x86_64-pc-windows-msvc/release/bundle/nsis/`.

Cross-compiling from Linux needs `lld`, `llvm`, `nsis`, the
`x86_64-pc-windows-msvc` Rust target, and `cargo install --locked cargo-xwin`.
See [`docs/architecture.md`](docs/architecture.md#cross-compiling).

## Repository layout

```
crates/yura-core      domain: persona, emotions, messages, settings, reply parser
crates/yura-llm       streaming chat: trait + OpenAI-compatible + Anthropic
crates/yura-avatar    emotion → expression weights, viseme tables
crates/yura-store     settings.json (atomic) + history.jsonl (append-only)
src-tauri             the window: commands, state, IPC events
ui/src/scene          renderer, lighting, shaders, post-processing
ui/src/avatar         VRM rig, emotion mixer, idle, gaze, lip-sync
ui/src/chat           transcript + composer
ui/src/voice          speech out, microphone in
ui/src/settings       the settings drawer
docs                  architecture, roadmap, providers
scripts               avatar fetch, dev, cross-build
```

Each crate has one reason to change. Nothing in `crates/` knows Tauri exists,
and nothing in `ui/src/scene` knows what a message is.

## Add a provider

Add one row to `PRESETS` in `crates/yura-core/src/settings.rs`:

```rust
ProviderPreset {
    id: "myvendor",
    label: "My Vendor",
    kind: ProviderKind::OpenAiCompatible,
    base_url: "https://api.myvendor.com/v1",
    default_model: "my-model",
    requires_key: true,
},
```

It appears in the settings dropdown, gets endpoint editing and key storage for
free. A vendor with a non-OpenAI wire format needs one module in `yura-llm`
plus one match arm in `registry.rs`.

## Add an emotion

1. Add the variant in `crates/yura-core/src/emotion.rs` (plus its parse aliases).
2. Add one row to `EXPRESSIONS` in `crates/yura-avatar/src/expression.rs`.
3. Add a hue in `ui/src/avatar/emotion.ts`.

The tests in `yura-avatar` fail if an emotion has no expression row, so the
three tables cannot drift apart silently.

## Licence

MIT for the code. Avatar models and voice data are **not** included and are not
covered by that licence — see [`ATTRIBUTION.md`](ATTRIBUTION.md).

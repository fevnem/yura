# Architecture

## The shape

Three layers, one direction of dependency.

```
┌──────────────────────────────────────────────────────────┐
│ ui/  (TypeScript, runs in WebView2)                      │
│   scene → avatar → chat / voice / settings               │
└───────────────▲──────────────────────────────────────────┘
                │  invoke(command) / on(event)
┌───────────────┴──────────────────────────────────────────┐
│ src-tauri  (Rust)  commands · state · window             │
└───────────────▲──────────────────────────────────────────┘
                │  plain function calls
┌───────────────┴──────────────────────────────────────────┐
│ crates/    core · llm · avatar · store                   │
└──────────────────────────────────────────────────────────┘
```

The frontend never talks to a provider directly. That keeps the API key in one
process, out of the webview, and means the streaming protocol is a single
documented set of events rather than ad-hoc `fetch` calls.

## Why these choices

**Tauri over a native GPU renderer.** A hand-written `wgpu` character rig is
weeks of work before it looks like anything; `three.js` + `three-vrm` ships the
whole anime-shading, spring-bone and expression stack. Tauri uses the WebView2
runtime that already exists on Windows 11, so the cost is a ~7 MB binary, not a
150 MB Electron bundle.

**One trait for LLM backends.** Almost every vendor now speaks
`POST /chat/completions` with SSE deltas. `OpenAiCompatible` therefore covers
OpenAI, DeepSeek, OpenRouter, Groq, Mistral, Together, xAI, Ollama, LM Studio
and llama.cpp's server. `Anthropic` is separate only because its wire format
genuinely differs (top-level `system`, mandatory `max_tokens`, typed events).

**Emotions as a protocol, not a prompt trick.** The model is instructed to
prefix each beat with `[emotion]`. `ReplyParser` is a streaming state machine: it
emits a beat the moment the *next* tag arrives, so the first sentence is spoken
while the second is still being written, and a tag split across two SSE chunks
never leaks into the transcript.

**Expression weights live in Rust.** VRM preset names (`happy`, `aa`, `blink`)
are the one vocabulary the renderer and any future exporter must agree on, so
the table lives in `yura-avatar` and is shipped to the UI as JSON at boot.
`ui/src/bridge/demo.ts` mirrors it for browser-only development.

**Three expression layers.** Mood, blink and mouth are produced by three
independent loops that would otherwise overwrite each other. `Avatar` sums and
clamps them once per frame.

**JSONL history, not SQLite.** A companion's history is small, and an
append-only text file means no native dependency and no migrations. `History` is
the seam where SQLite can slot in later without touching callers.

**Speech through the OS.** `speechSynthesis` in WebView2 reaches the Windows
voices with zero bundling — including the neural ones on Windows 11. The cost is
that no waveform is available to JavaScript, so lip-sync is driven by word
boundary events plus a shaped envelope (`ui/src/voice/speech.ts`). A bundled
neural voice (Kokoro) would deliver real audio and real FFT, and slots in behind
the same three methods.

## The streaming protocol

`send_message` returns immediately and reports progress through events:

| Event | Payload | Meaning |
|---|---|---|
| `chat:start` | `{ text }` | user turn accepted |
| `chat:delta` | `{ delta, visible }` | raw token, plus cleaned text so far |
| `chat:utterance` | `{ emotion, text }` | completed beat, ready to speak |
| `chat:done` | `{ text, reply }` | finished |
| `chat:cancelled` | `{ text, reply }` | stopped by user |
| `chat:error` | `{ message }` | provider or network failure |
| `settings:changed` | `{ settings }` | UI should re-read state |

Cancellation is an `AtomicBool` checked between deltas; the generation task is
aborted and whatever was generated is kept as a partial reply rather than
discarded.

## Data on disk

`%APPDATA%\Yura\`

- `settings.json` — written to a temp file and renamed, so a crash mid-save can
  never truncate it. A corrupt file is moved to `settings.json.corrupt` and
  defaults are used; the app always starts.
- `history.jsonl` — one JSON object per line. A corrupt line is skipped, not
  fatal.

## Cross-compiling

Tauri officially targets MSVC. On Linux, `cargo-xwin` supplies the Windows SDK
and CRT, and NSIS builds the installer. The workspace deliberately depends on
`native-tls` rather than `rustls`, because on Windows that resolves to
`schannel` — no OpenSSL, no `ring`, nothing that needs a fragile cross C build.

```bash
sudo apt install lld llvm clang nsis
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin
npm run build:windows
```

`.msi` installers require WiX and therefore a Windows host; the NSIS
`-setup.exe` cross-builds fine and is what `build:windows` produces.

## Testing

Unit tests live next to the code they describe and focus on the parts where a
silent mistake is expensive: the reply parser (chunk boundaries, unknown tags),
SSE framing, settings normalisation, history truncation, and the emotion/expression
tables' consistency.

```bash
cargo test -p yura-core -p yura-llm -p yura-avatar -p yura-store
```

The renderer is verified by eye — `npm run dev` runs it in a plain browser with
a demo backend (`ui/src/bridge/demo.ts`), which is how the visuals are iterated.

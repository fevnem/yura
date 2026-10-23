# Providers

Yura speaks two wire formats. Everything else is configuration.

## OpenAI-compatible

`POST {base_url}/chat/completions`, `stream: true`, SSE frames.

| Provider | Base URL | Notes |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | |
| DeepSeek | `https://api.deepseek.com/v1` | cheap, strong persona work |
| OpenRouter | `https://openrouter.ai/api/v1` | one key, many models |
| Groq | `https://api.groq.com/openai/v1` | fastest tokens |
| Mistral | `https://api.mistral.ai/v1` | |
| Together | `https://api.together.xyz/v1` | open-weight models |
| xAI | `https://api.x.ai/v1` | |
| Ollama | `http://localhost:11434/v1` | no key needed |
| LM Studio | `http://localhost:1234/v1` | no key needed |
| Custom | anything | self-hosted vLLM, llama.cpp, gateways |

Local servers need no API key; Yura treats an empty key on a `requires_key:
false` preset as valid and never sends an `Authorization` header.

## Anthropic

`POST {base_url}/messages`, `x-api-key` header, `anthropic-version: 2023-06-01`,
typed SSE events (`content_block_delta` → `text_delta`).

Two API-specific rules are handled in `crates/yura-llm/src/anthropic.rs`:
`system` is a top-level field rather than a message, and `max_tokens` is
mandatory. Consecutive same-role messages are merged before sending, because the
Messages API rejects two `user` turns in a row.

## Picking a model for a companion

Yura is a *personality* workload: short turns, strong voice, fast first token.

- **Want it free and local:** `ollama pull qwen2.5:7b` — good at staying in
  character, runs on a 6 GB GPU or a decent CPU.
- **Want the best character work for the money:** `deepseek-chat`.
- **Want the fastest first token:** a Groq-hosted Llama.
- **Want the strongest reasoning:** `claude-3-5-sonnet` or `gpt-4o`.

Keep `max_tokens` low (300–500). A companion that monologues is worse company
than one that stops and asks.

## How keys are handled

- Stored in `%APPDATA%\Yura\settings.json`, in plain text, next to the rest of
  your config — the same trust model as an SSH config or a `.env`.
- Never logged. Never sent anywhere except the `base_url` you configured.
- Never part of any request to the frontend: the webview asks the Rust process
  to send a message; it never holds the key.
- Removing a provider's key from Settings and saving is enough to revoke Yura's
  access — nothing is cached elsewhere.

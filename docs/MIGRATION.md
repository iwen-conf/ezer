# Migrating from Grok Build (`grok`) to ezer

This repository is a standalone fork of Grok Build, rehomed as **ezer**.
The interactive TUI, session loop, tool orchestration, headless mode, and ACP
are the same architecture. The product defaults are not.

## Breaking changes

| Grok Build | ezer |
|---|---|
| Command `grok` | Command `ezer` |
| User state `~/.grok/` | User state `~/.ezer/` |
| Env override `$GROK_HOME` | Env override `$EZER_HOME` (`$GROK_HOME` is a deprecated alias only) |
| First launch opens grok.com / `auth.x.ai` OAuth | No xAI login required. BYOK / API key + base URL is enough |
| Default models `grok-4.6` / `grok-4.5` on `api.x.ai` | No bundled Grok catalog. Configure `[model.*]` (and usually `[models].default`) |
| Custom models default to Chat Completions | Custom / BYOK models default to **OpenAI Responses** (`POST /v1/responses`) |
| `XAI_API_KEY` | `EZER_API_KEY` or `OPENAI_API_KEY` (legacy `XAI_API_KEY` still accepted) |
| System config `/etc/grok` | `/etc/ezer` |
| Project config `<repo>/.grok/config.toml` | `<repo>/.ezer/config.toml` (`.grok` is still read as a fallback) |

ezer does **not** read or write `~/.grok` unless you explicitly set
`$EZER_HOME` or the deprecated `$GROK_HOME` to that path.

## Copying state (optional)

```sh
# Only if you want old sessions / skills / logs:
mkdir -p ~/.ezer
cp -a ~/.grok/config.toml ~/.ezer/   # then edit it
# sessions, skills, logs, caches are per-product; copy only what you need
```

Do not copy `~/.grok/auth.json` unless you still intend to use a grok.com
session. ezer is designed to run with a local API key instead.

## Minimum BYOK `~/.ezer/config.toml`

```toml
[models]
default = "my-model"

[model.my-model]
model = "my-model"
base_url = "http://192.168.0.63:8788/v1"
api_backend = "responses"
api_key = "sk-..."
```

`api_backend` may be omitted: **Responses is the default**. Set
`api_backend = "chat_completions"` only for gateways that do not speak
`POST /v1/responses`.

Equivalent environment:

```sh
export EZER_HOME="$HOME/.ezer"
export EZER_API_KEY="sk-..."
# or: export OPENAI_API_KEY="sk-..."
```

Then:

```sh
ezer
```

The TUI should start without a grok.com login prompt and send turns to
`POST {base_url}/responses`.

## What stayed the same

- Interactive TUI, slash commands, permissions, sandboxing
- Headless `-p` / `ezer agent`
- ACP (`ezer agent stdio`)
- Tools: files, shell, search, subagents, MCP, skills, hooks
- Internal Rust crate names (`xai-grok-*`) — only the public command is `ezer`

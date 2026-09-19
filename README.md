<div align="center">

<h1>ezer</h1>

**ezer** is a standalone terminal AI coding agent, forked from Grok Build.
It keeps the TUI, agent orchestration, tools, sessions, skills, headless mode,
and ACP — without requiring an xAI / grok.com account.

[Building from source](#building-from-source) ·
[Configuration](#configuration) ·
[Documentation](#documentation) ·
[Repository layout](#repository-layout) ·
[License](#license)

</div>

---

## What changed from Grok Build

- Config home is **`~/.ezer/`** (`EZER_HOME` override). ezer does **not** default-read or write `~/.grok`.
- First launch is **BYOK-first**: OpenAI-compatible Responses API, no browser login wall.
- Default wire protocol for custom models is **`POST /v1/responses`** (Chat Completions remains available).
- The CLI binary is **`ezer`**.

Optional xAI / grok.com OAuth still exists behind `EZER_ENABLE_XAI_LOGIN=1` (or `ezer login --force-login`) and never blocks startup.

## Building from source

Requirements:

- **Rust** — pinned by [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` installs it on first build.
- **[DotSlash](https://dotslash-cli.com)** — hermetic tools under [`bin/`](bin/) (notably [`bin/protoc`](bin/protoc)).
- **protoc** — via DotSlash, `$PATH`, or `$PROTOC`.

```sh
cargo run -p xai-grok-pager-bin              # build + launch the TUI (`ezer`)
cargo build -p xai-grok-pager-bin --release  # release binary: target/release/ezer
cargo check -p xai-grok-pager-bin            # fast validation
```

The package is still `xai-grok-pager-bin`; the default binary name is `ezer`.

Internal crate paths (`xai-grok-*`, functions like `grok_home()`) were left in place so TUI, tools, sessions, skills, headless, and ACP keep working. User-facing CLI, `~/.ezer`, and help/version say **ezer**.

## Configuration

On first launch (when `$EZER_HOME` is unset or points at an empty home), ezer writes a starter `~/.ezer/config.toml` for the WorkBuddy2API-Hub gateway.

### WorkBuddy2API-Hub / OpenAI Responses gateway

Default and primary wire protocol is **`POST /v1/responses`** (SSE). Session title/summary uses this same model — it does **not** call built-in `grok-4.6`.

```toml
# ~/.ezer/config.toml
[auth]
preferred_method = "api_key"

[endpoints]
models_base_url = "http://192.168.0.63:8788/v1"

[models]
default = "workbuddy"
session_summary = "workbuddy"

[model.workbuddy]
model = "deepseek-v4.1-flash"
name = "WorkBuddy DeepSeek"
base_url = "http://192.168.0.63:8788/v1"
api_backend = "responses"
context_window = 200000
reasoning_effort = "max"
supports_reasoning_effort = true
api_key = "SrdCiNW_1c1qkc--o6e_Btot7yCwa8JswK3N856Q6ck"
env_key = ["EZER_API_KEY", "XAI_API_KEY"]
```

Optional models on that hub (also seeded on first run): `deepseek-v4.1-flash-low`, `deepseek-v4.1-flash-high`, `deepseek-v4.1-flash-max`, `hy4-preview-f`, `hy3`.

```sh
ezer
# or: ezer -p "hello" --model workbuddy
```

**Auth headers:** every API-key request sends `Authorization: Bearer <key>` and also `x-api-key` / `api-key`.

**Wire protocol:** `api_backend = "responses"` → `POST /v1/responses` with streaming SSE (input items, function tools, reasoning). `function_call_arguments.delta` events that omit `item_id` and send `call_id` are accepted. Chat Completions remains secondary (`api_backend = "chat_completions"`). Empty `finish_reason` on chat streams is treated as unset.

### LAN smoke (run on the machine that can reach 192.168.0.63)

The cloud agent cannot reach this LAN address. On your Mac:

```sh
KEY='SrdCiNW_1c1qkc--o6e_Btot7yCwa8JswK3N856Q6ck'
BASE='http://192.168.0.63:8788/v1'

curl -sS "$BASE/models" \
  -H "Authorization: Bearer $KEY" \
  -H "x-api-key: $KEY" \
  -H "api-key: $KEY"

curl -sS -N "$BASE/responses" \
  -H "Authorization: Bearer $KEY" \
  -H "x-api-key: $KEY" \
  -H "api-key: $KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"deepseek-v4.1-flash","stream":true,"input":[{"role":"user","content":"ping"}]}'

EZER_HOME="$HOME/.ezer" ezer --version
EZER_HOME="$HOME/.ezer" ezer -p "Reply with the word pong only."
```

### Gateway wire notes (for WorkBuddy / `wb_proxy.py`)

Observed / handled on the ezer client (do **not** require a live 63 test from CI):

| Quirk | Client behavior |
| --- | --- |
| `response.function_call_arguments.delta` / `.done` omits `item_id`, sends `call_id` | Inject `item_id` from `call_id` (or `""`) before async-openai deserialize |
| Empty `finish_reason` on some Chat Completions streams | Treat as unset |
| Rich `/v1/models` (`id` only, dotted slugs) | Parse `id` as the wire model; default `api_backend = responses` |
| Session title using compiled `grok-4.6` | Use the active BYOK model (avoids 402 on free models) |

If you patch the hub on the LAN host, emitting `item_id` on function-call SSE (equal to `call_id` is fine) matches stock OpenAI Responses.

**Home override:**

```sh
export EZER_HOME="$HOME/.ezer"   # default
# $GROK_HOME is a deprecated alias and is not the default path
```

Project-local config is read from `.ezer/config.toml` first, then `.grok/config.toml`.

## Documentation

The user guide ships with the pager crate:
[`crates/codegen/xai-grok-pager/docs/user-guide/`](crates/codegen/xai-grok-pager/docs/user-guide/)
— getting started, keyboard shortcuts, slash commands, configuration, theming,
MCP servers, skills, plugins, hooks, headless mode, sandboxing, and more.

Start with [custom models](crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md) and [authentication](crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md).

## Repository layout

| Path | Contents |
|------|----------|
| `crates/codegen/xai-grok-pager-bin` | Composition-root package; builds the `ezer` binary |
| `crates/codegen/xai-grok-pager` | The TUI: scrollback, prompt, modals, rendering |
| `crates/codegen/xai-grok-shell` | Agent runtime + leader/stdio/headless entry points |
| `crates/codegen/xai-grok-tools` | Tool implementations (terminal, file edit, search, ...) |
| `crates/codegen/xai-grok-workspace` | Host filesystem, VCS, execution, checkpoints |
| `crates/codegen/...` | The rest of the CLI crate closure (config, MCP, markdown, sandbox, ...) |
| `crates/common/`, `crates/build/`, `prod/mc/` | Small shared leaf crates pulled in by the closure |
| `third_party/` | Vendored upstream source (Mermaid diagram stack) |

> [!IMPORTANT]
> The root `Cargo.toml` (workspace members, dependency versions, lints,
> profiles) is **generated** — treat it as read-only. Prefer editing per-crate
> `Cargo.toml` files.

## Development

```sh
cargo check -p <crate>        # always target specific crates; full-workspace builds are slow
cargo test -p xai-grok-config # per-crate tests
cargo clippy -p <crate>       # lint config: clippy.toml at the repo root
cargo fmt --all               # rustfmt.toml at the repo root
```

## License

First-party code in this repository is licensed under the **Apache License,
Version 2.0** — see [`LICENSE`](LICENSE).

This tree is a fork of SpaceXAI Grok Build. Third-party and vendored code remains under its original licenses. See:

- [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES)
- [`crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md`](crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md)
- [`third_party/NOTICE`](third_party/NOTICE)

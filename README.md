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

## Configuration

On first launch (when `$EZER_HOME` is unset or points at an empty home), ezer writes a starter `~/.ezer/config.toml`. Edit the API key and model id for your gateway.

### WorkBuddy2API-Hub / OpenAI Responses gateway

This is the supported default for a local OpenAI-compatible hub:

```toml
# ~/.ezer/config.toml
[auth]
preferred_method = "api_key"

[endpoints]
models_base_url = "http://192.168.0.63:8788/v1"

[models]
default = "workbuddy"

[model.workbuddy]
model = "deepseek-v4.1-flash"   # or hy4-preview-f / hy3 — whatever /v1/models lists
name = "WorkBuddy gateway"
base_url = "http://192.168.0.63:8788/v1"
api_backend = "responses"
context_window = 200000
env_key = ["EZER_API_KEY", "XAI_API_KEY"]
# api_key = "your-gateway-key"
```

```sh
export EZER_API_KEY="your-gateway-key"
ezer
```

**Auth headers:** every API-key request sends `Authorization: Bearer <key>` and also `x-api-key` / `api-key` (the gateway accepts any of these).

**Wire protocol:** default `api_backend = "responses"` → `POST /v1/responses` with streaming SSE (input items, function tools, reasoning fields). Set `api_backend = "chat_completions"` for the secondary Chat Completions path. Empty `finish_reason` on Chat Completions streams is treated as unset.

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

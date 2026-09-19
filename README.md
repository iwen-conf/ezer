<div align="center">

# ezer

**ezer** is a terminal-based AI coding agent. It runs as a full-screen TUI
that understands your codebase, edits files, executes shell commands, searches
the web, and manages long-running tasks — interactively, headlessly for
scripting/CI, or embedded in editors via the Agent Client Protocol (ACP).

It is a standalone fork of Grok Build. It does **not** require an xAI / X /
grok.com login. The default wire protocol is the **OpenAI Responses** API
(`POST /v1/responses`, streaming SSE).

[Building from source](#building-from-source) ·
[BYOK setup](#byok--custom-provider-setup) ·
[Documentation](#documentation) ·
[Migrating from grok](docs/MIGRATION.md) ·
[License](#license)

</div>

---

## Building from source

Requirements:

- **Rust** — the toolchain is pinned by [`rust-toolchain.toml`](rust-toolchain.toml);
  `rustup` installs it automatically on first build.
- **[DotSlash](https://dotslash-cli.com)** — required so hermetic tools under
  [`bin/`](bin/) (notably [`bin/protoc`](bin/protoc)) can download and run.
- **protoc** — proto codegen resolves [`bin/protoc`](bin/protoc) via DotSlash,
  or falls back to a `protoc` on `PATH` / `$PROTOC`.

```sh
cargo run -p xai-grok-pager-bin --bin ezer              # build + launch the TUI
cargo build -p xai-grok-pager-bin --release --bin ezer  # target/release/ezer
ezer --help
```

A convenience installer copies the release binary to `$EZER_HOME/bin`
(default `~/.ezer/bin`) and writes a starter config if none exists:

```sh
./crates/codegen/xai-grok-pager/scripts/install.sh
```

## BYOK / custom-provider setup

ezer starts from `~/.ezer/config.toml` (override the state root with
`EZER_HOME`). There is no grok.com browser login on first launch.

```toml
# ~/.ezer/config.toml
[models]
default = "my-model"

[model.my-model]
model = "my-model"
base_url = "http://192.168.0.63:8788/v1"
api_backend = "responses"   # default; Chat Completions is opt-in
api_key = "sk-..."
# env_key = "EZER_API_KEY"  # or OPENAI_API_KEY / XAI_API_KEY
```

```sh
export EZER_API_KEY="sk-..."
ezer
```

The client sends `POST /v1/responses` (SSE when streaming): input items,
function/tool calls, reasoning items when the gateway emits them, and both
stream and non-stream completions. Set `api_backend = "chat_completions"`
only if the gateway has no Responses implementation.

See [Custom Models](crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md)
and [Authentication](crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md).

## Documentation

The user guide ships with the pager crate:
[`crates/codegen/xai-grok-pager/docs/user-guide/`](crates/codegen/xai-grok-pager/docs/user-guide/)
— getting started, keyboard shortcuts, slash commands, configuration, theming,
MCP servers, skills, plugins, hooks, headless mode, sandboxing, and more.

Breaking changes from Grok Build (`~/.grok`, `grok login`, default models)
are listed in [`docs/MIGRATION.md`](docs/MIGRATION.md).

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

Internal crate names still use the historical `xai-grok-*` package ids. The
public command, help text, and user state directory are **ezer**.

> [!IMPORTANT]
> The root `Cargo.toml` (workspace members, dependency versions, lints,
> profiles) is **generated** — treat it as read-only. Prefer editing per-crate
> `Cargo.toml` files.

## Development

```sh
cargo check -p xai-grok-pager-bin
cargo test -p xai-dirs
cargo test -p xai-grok-login
cargo clippy -p xai-grok-pager-bin
cargo fmt --all
```

## License

First-party code in this repository is licensed under the **Apache License,
Version 2.0** — see [`LICENSE`](LICENSE).

Third-party and vendored code remains under its original licenses. See:

- [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES)
- [`crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md`](crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md)
- [`third_party/NOTICE`](third_party/NOTICE)

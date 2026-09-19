<div align="center">

<h1>ezer</h1>

**ezer** is a standalone terminal AI coding agent.
It keeps the TUI, agent orchestration, tools, sessions, skills, headless mode,
and ACP — without requiring an xAI account.

[Building from source](#building-from-source) ·
[Configuration](#configuration) ·
[Documentation](#documentation) ·
[Repository layout](#repository-layout) ·
[License](#license)

</div>

---

## Standalone defaults

- Config home is **`~/.ezer/`** (`EZER_HOME` override).
- First launch is **BYOK-first**: OpenAI-compatible Responses API, no browser login wall.
- Default wire protocol for custom models is **`POST /v1/responses`** (Chat Completions remains available).
- The CLI binary is **`ezer`**.
- Compiled catalog default is **`workbuddy`** (wire id `deepseek-v4.1-flash`). Startup validates catalog **`id`** (not only the wire `model` slug), so an alias default no longer panics.

Optional browser OAuth still exists behind `EZER_ENABLE_XAI_LOGIN=1` (or `ezer login --force-login`) and never blocks startup. BYOK does not require grok.com.

## Building from source

Requirements:

- **Rust** — pinned by [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` installs it on first build.
- **[DotSlash](https://dotslash-cli.com)** — hermetic tools under [`bin/`](bin/) (notably [`bin/protoc`](bin/protoc)).
- **protoc** — via DotSlash, `$PATH`, or `$PROTOC`.

```sh
cargo run -p ezer-pager-bin              # build + launch the TUI (`ezer`)
cargo build -p ezer-pager-bin --release  # release binary: target/release/ezer
cargo check -p ezer-pager-bin            # fast validation
```

The shipping package is `ezer-pager-bin`; the default binary name is `ezer`.

Crates that previously used the `xai-grok-*` package/directory names are now
`ezer-*` (workspace members, path deps, and Rust `use` paths). Historical
`xai-*` leaf crates that never had `grok` in the name were left as-is.
Internal function names such as `grok_home()` still exist so TUI, tools,
sessions, skills, headless, and ACP keep the same behavior. User-facing CLI,
`~/.ezer`, and help/version say **ezer**.

## Configuration

On first launch (when `$EZER_HOME` is unset or points at an empty home), ezer writes a starter `~/.ezer/config.toml` for the WorkBuddy2API-Hub gateway.

### WorkBuddy2API-Hub / OpenAI Responses gateway

Default and primary wire protocol is **`POST /v1/responses`** (SSE). Session title/summary uses the active BYOK model.

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

### Gateway wire notes (for WorkBuddy / `wb_proxy.py` on the LAN host)

The cloud agent cannot SSH to `192.168.0.63`. Patch the hub there if you want stock OpenAI Responses. ezer still ships a **defensive client inject** so a missing `item_id` does not drop the tool-call stream.

#### Mismatch that breaks typed Responses clients

async-openai requires `item_id: String` on:

- `response.function_call_arguments.delta`
- `response.function_call_arguments.done`

WorkBuddy2API-Hub often emits `call_id` only. Raw event (this is what the ezer mock fixture replays):

```json
{
  "type": "response.function_call_arguments.delta",
  "sequence_number": 2,
  "call_id": "call_wb_1",
  "output_index": 0,
  "delta": "{\"path\":\"README.md\"}"
}
```

Stock OpenAI Responses includes `item_id` (the function-call item id, e.g. `fc_…`). Copying `call_id` is enough for ezer:

```json
{
  "type": "response.function_call_arguments.delta",
  "sequence_number": 2,
  "item_id": "call_wb_1",
  "call_id": "call_wb_1",
  "output_index": 0,
  "delta": "{\"path\":\"README.md\"}"
}
```

Same for `.done` (`arguments` + optional `name` instead of `delta`).

#### Suggested `wb_proxy.py` SSE rewrite (LAN host only)

In the Responses SSE loop, after `json.loads` of each `data:` payload:

```python
# Keep call_id; fill item_id when the upstream omitted it.
# Optional: if you already tracked the function_call item id (fc_…) from
# response.output_item.added, prefer that over call_id.
FC_ARG_TYPES = (
    "response.function_call_arguments.delta",
    "response.function_call_arguments.done",
)
if payload.get("type") in FC_ARG_TYPES and not payload.get("item_id"):
    payload["item_id"] = payload.get("call_id") or ""
```

Also keep `item.id` and `item.call_id` on `response.output_item.added` / `.done` function_call items (those already look fine in the fixture).

ezer client inject (do not remove): `ezer_sampler::client::inject_item_id_from_call_id` copies `call_id` → `item_id` when `item_id` is missing, null, or `""`. Tests: `deserialize_function_call_arguments_*` and `tests/workbuddy_responses.rs`.

#### Other observed quirks

| Quirk | Client behavior | Optional gateway patch |
| --- | --- | --- |
| `function_call_arguments.*` omits `item_id`, sends `call_id` | Inject `item_id` from `call_id` (or `""`) | Emit `item_id` as above |
| Empty `finish_reason` on some Chat Completions streams | Treat as unset | Prefer Responses; or omit empty `finish_reason` |
| Rich `/v1/models` (`id` only, dotted slugs) | Parse `id` as the wire model; default `api_backend = responses` | Keep `id` as the wire slug (`deepseek-v4.1-flash`, `hy4-preview-f`, `hy3`) |
| Session title using a compiled-in aux slug | Use the active BYOK model (avoids 402 on free models) | n/a (client-only) |

**Home override:**

```sh
export EZER_HOME="$HOME/.ezer"   # default
```

Project-local config is read from `.ezer/config.toml` in the workspace, then `~/.ezer/config.toml`.

## Documentation

The user guide ships with the pager crate:
[`crates/codegen/ezer-pager/docs/user-guide/`](crates/codegen/ezer-pager/docs/user-guide/)
— getting started, keyboard shortcuts, slash commands, configuration, theming,
MCP servers, skills, plugins, hooks, headless mode, sandboxing, and more.

Start with [custom models](crates/codegen/ezer-pager/docs/user-guide/11-custom-models.md) and [authentication](crates/codegen/ezer-pager/docs/user-guide/02-authentication.md).

## Repository layout

| Path | Contents |
|------|----------|
| `crates/codegen/ezer-pager-bin` | Composition-root package; builds the `ezer` binary |
| `crates/codegen/ezer-pager` | The TUI: scrollback, prompt, modals, rendering |
| `crates/codegen/ezer-shell` | Agent runtime + leader/stdio/headless entry points |
| `crates/codegen/ezer-tools` | Tool implementations (terminal, file edit, search, ...) |
| `crates/codegen/ezer-workspace` | Host filesystem, VCS, execution, checkpoints |
| `crates/codegen/...` | The rest of the CLI crate closure (config, MCP, markdown, sandbox, ...) |
| `crates/common/`, `crates/build/`, `prod/mc/` | Small shared leaf crates pulled in by the closure |
| `third_party/` | Vendored upstream source (Mermaid diagram stack) |

> [!IMPORTANT]
> The root `Cargo.toml` (workspace members, dependency versions, lints,
> profiles) is **generated** — treat it as read-only. Prefer editing per-crate
> `Cargo.toml` files.

## De-branding leftovers

Shipping crate folders and package names are `ezer-*`. These `rg -i grok` hits remain on purpose and are **not** crate paths:

| Kind | Examples | In `ezer` binary? |
|------|----------|-------------------|
| Internal APIs | `grok_home()`, `GrokComConfig`, `GrokBuildEnvironment`, `ClientType::GrokPager` | Symbol / type names if the binary is unstripped |
| Theme ids | `groknight`, `grokday` | Yes — first-party theme names |
| Optional OAuth | `grok.com` copy behind `EZER_ENABLE_XAI_LOGIN=1` | Only if that login path is linked |
| Historical `xai-*` crates | `xai-dirs`, `xai-crash-handler`, … | Panic `file!()` paths (`crates/codegen/xai-…`) — no `grok` |
| npm platform packages | `crates/codegen/ezer-pager/npm/grok-*` | No — not linked into the Rust CLI |
| Docs | this README's fork/history notes | No |

## Development

```sh
cargo check -p <crate>        # always target specific crates; full-workspace builds are slow
cargo test -p ezer-config # per-crate tests
cargo clippy -p <crate>       # lint config: clippy.toml at the repo root
cargo fmt --all               # rustfmt.toml at the repo root
```

## License

First-party code in this repository is licensed under the **Apache License,
Version 2.0** — see [`LICENSE`](LICENSE).

Third-party and vendored code remains under its original licenses. See:

- [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES)
- [`crates/codegen/ezer-tools/THIRD_PARTY_NOTICES.md`](crates/codegen/ezer-tools/THIRD_PARTY_NOTICES.md)
- [`third_party/NOTICE`](third_party/NOTICE)

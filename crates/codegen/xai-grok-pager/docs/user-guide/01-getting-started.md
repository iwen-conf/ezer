# Getting Started

**ezer** is a terminal-based AI coding assistant (a standalone ezer fork). It runs as a TUI that understands your codebase, executes shell commands, edits files, searches the web, and manages tasks — using your own OpenAI-compatible gateway. No xAI / grok.com account is required.

You can use it interactively as a full-screen TUI, run it headlessly for scripting and CI/CD, or integrate it into editors via the Agent Client Protocol (ACP).

---

## Installation

Build from this tree and put `ezer` on your `PATH`:

```bash
cargo build -p xai-grok-pager-bin --release
# binary: target/release/ezer
# config / sessions: ~/.ezer  (override with EZER_HOME)
```

The leftover `install.sh` / `install.ps1` scripts still fetch published artifacts and install `ezer` as the primary command (`ezer` remains a compatibility name).

Verify the installation:

```bash
ezer --version
```

To fetch a repository through Grove (NFS on macOS, FUSE on Linux), enable
`ezer clone` with `[clone] enabled = true` in Grove config, `EZER_CLONE=1`,
or the enable-both convenience `EZER_GROVE=1` / `[cli] grove = true` in
`~/.ezer/config.toml`:

```bash
ezer clone <url> [dir]
```

The default is a depth-1 checkout of the selected branch. Pass `--full-history`
for a complete clone. Clone enablement is independent of session / `-w` Grove
worktrees (the convenience above turns both on; the specific knobs still win).
See [ezer clone](27-ezer-clone.md#authentication) and
[Configuration reference](26-config-reference.md).

---

## First Launch

Start ezer by running:

```bash
ezer
```

On first launch ezer writes `~/.ezer/config.toml` with a BYOK Responses example pointed at `http://192.168.0.63:8788/v1`. Put your gateway key in that file (`api_key`) or in `EZER_API_KEY`. There is no grok.com login wall.

```bash
export EZER_API_KEY="your-gateway-key"
ezer
```

See [Authentication](02-authentication.md) for the full set of auth options including OIDC, external auth providers, and device code flow.

---

## Basic Interaction

Once authenticated, ezer presents a full-screen TUI with two main areas:

- **Scrollback** -- the conversation history showing your prompts, ezer's responses, tool calls, file edits, and more.
- **Prompt** -- the input area at the bottom where you type messages.

Type a message and press `Enter` to send it. ezer reads files, runs commands, and edits code as needed. Each tool run streams into the scrollback in real time.

Press `Tab` to move focus between the prompt and the scrollback. While a turn is running, `Ctrl+C` cancels it once the composer is empty — with a draft, the first press only clears it. `Esc` never cancels a turn; mid-turn it shows a reminder to use `Ctrl+C`. Idle, press `Esc` twice within 800ms to clear a non-empty prompt, or (with an empty prompt and conversation messages) to open rewind — see [Keyboard Shortcuts](03-keyboard-shortcuts.md#escape). With the scrollback focused, use the arrow keys to select entries and to collapse or expand them. To navigate with `j`/`k` and fold with `h`/`l` instead, enable Vim mode.

### File References

Use `@` in your prompt to attach files:

```
@src/main.rs              # Attach a file
@src/main.rs:10-50        # Attach lines 10-50
@src/                     # Browse a directory
```

The `@` operator opens a fuzzy file picker. By default it respects `.gitignore` and hides dotfiles. Prefix with `!` to search hidden files:

```
@!.github                 # Search hidden files
@!.env                    # Attach a .env file
```

### Permissions

By default, ezer asks for permission before executing shell commands or editing files. You can approve individually or toggle always-approve mode:

- Press `Ctrl+O` to toggle always-approve mode
- Use the `--yolo` flag at launch: `ezer --yolo`
- Type `/always-approve` in the prompt to toggle the mode

---

## Key Concepts

### Sessions

Every conversation is a **session**. Sessions are automatically saved to `~/.ezer/sessions/` and can be resumed later. Each session tracks the full conversation history, tool calls, file edits, and task state.

- Start a new session: `Ctrl+N` or `/new`
- Resume a previous session: `/resume` in the TUI, or `--resume <ID>` from the CLI
- Continue the most recent session: `ezer -c`

### Scrollback

The scrollback is the main display area. It shows:

- **User prompts** -- your messages, rendered as sticky headers
- **Agent messages** -- ezer's responses with full markdown rendering and syntax highlighting
- **Thinking blocks** -- ezer's reasoning process (collapsible)
- **Tool calls** -- file edits (with inline diffs), command executions, search results, and more
- **Task lists** -- TODO items tracking progress

Collapse or expand the selected entry with the `Left`/`Right` arrow keys (or `h`/`l` and `e` in Vim mode). In Vim mode, press `y` to copy its content and `Y` to copy its metadata (for example, the command that ran). Press `Enter` to open it in the fullscreen viewer (in any mode).

### Tools

ezer has built-in tools for:

| Tool | Description |
|------|-------------|
| `read_file` / `search_replace` | Read and edit files with line-precise changes |
| `grep` | Regex search across your codebase (powered by ripgrep) |
| `list_dir` | List directory contents |
| `run_terminal_command` | Execute shell commands |
| `web_search` / `web_fetch` | Search the web and fetch URLs |
| `todo_write` | Create and manage task lists |
| `spawn_subagent` | Spawn parallel subagent sessions |
| `memory_search` | Search cross-session memory |

Tools can be extended with [MCP servers](05-configuration.md#mcp-servers) for integrations like GitHub, databases, and more.

### Slash Commands

Type `/` in the prompt to access commands. These provide quick actions without writing a full prompt:

```
/model workbuddy                # Switch model (wire id: deepseek-v4.1-flash)
/compact                          # Compress conversation history
/always-approve                   # Toggle always-approve mode
/new                              # Start a new session
```

See [Slash Commands](04-slash-commands.md) for the complete reference.

---

## Common Launch Options

```bash
# Launch the interactive TUI and submit an initial prompt as the first turn
ezer "fix the failing auth test and run it"

# Initial prompt in a new git worktree. Use --worktree=<name> (with `=`) so the
# prompt isn't swallowed as the worktree name — `ezer -w "refactor module X"`
# would treat "refactor module X" as the worktree label, not the prompt.
ezer --worktree=feat "refactor module X"

# Base the worktree on a specific branch (e.g. main) instead of the current HEAD:
ezer -w --ref main "implement feature from main"


# Start in a specific project directory
ezer --cwd ~/projects/my-app

# Add project-specific rules
ezer --rules "Always use TypeScript. Prefer functional components."

# Auto-approve all tool executions
ezer --yolo

# Use a specific model (catalog key; wire id is deepseek-v4.1-flash)
ezer -m workbuddy

# Resume a previous session
ezer --resume <session-id>

# Continue the most recent session
ezer -c

# Experimental scrollback-native render mode. Sticky: plain `ezer` reopens in
# the mode last chosen via --minimal/--fullscreen (or /minimal//fullscreen).
ezer --minimal

# Back to the standard fullscreen TUI (and make it sticky again)
ezer --fullscreen

# Headless mode (for scripts)
ezer -p "Explain this codebase"
```

---

## Headless Mode

Run ezer non-interactively for scripting, CI/CD, and automation:

```bash
ezer -p "Your prompt here"
```

Output formats:

| Format | Flag | Description |
|--------|------|-------------|
| `plain` | (default) | Human-readable text |
| `json` | `--output-format json` | Single JSON object with `text`, `stopReason`, `sessionId`, and `requestId` |
| `streaming-json` | `--output-format streaming-json` | NDJSON event stream for real-time processing |

Example CI/CD usage:

```bash
ezer -p "Review changes for bugs" --output-format json --yolo | jq -r '.text'
```

---

## Project Rules (AGENTS.md)

Add per-project instructions by creating an `AGENTS.md` file in your repository. ezer reads these files and injects their contents as a project-instructions message at the start of the conversation:

```
~/.ezer/AGENTS.md           # Global rules (apply to all projects)
<repo-root>/AGENTS.md       # Repository-level rules
<cwd>/AGENTS.md             # Directory-level rules (highest priority)
```

Deeper files take precedence. ezer also reads `CLAUDE.md` files for compatibility.

---

## Where to Go Next

| Document | What You Will Learn |
|----------|-------------------|
| [Authentication](02-authentication.md) | Browser login, API keys, OIDC, external auth, device code flow |
| [Keyboard Shortcuts](03-keyboard-shortcuts.md) | Complete reference for all key bindings |
| [Slash Commands](04-slash-commands.md) | All available `/` commands |
| [Configuration](05-configuration.md) | config.toml, pager.toml, environment variables |

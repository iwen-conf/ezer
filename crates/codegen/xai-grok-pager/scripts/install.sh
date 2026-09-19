#!/bin/bash
#
# ezer installer — build the Responses-first coding agent from this tree
# and install the `ezer` binary under $EZER_HOME/bin (default: ~/.ezer/bin).
#
# Usage (from a clone of this repository):
#   ./crates/codegen/xai-grok-pager/scripts/install.sh
#   EZER_HOME=/opt/ezer ./crates/codegen/xai-grok-pager/scripts/install.sh
#
# This fork does not download SpaceXAI / grok.com release artifacts and does
# not require a grok.com login. Configure a local Responses endpoint in
# ~/.ezer/config.toml (see README.md and docs/MIGRATION.md).

set -euo pipefail

EZER_HOME="${EZER_HOME:-${HOME}/.ezer}"
BIN_DIR="${EZER_BIN_DIR:-${EZER_HOME}/bin}"
REPO_ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required to build ezer. Install Rust from https://rustup.rs" >&2
    exit 1
fi

mkdir -p "$BIN_DIR"

echo "Building ezer from ${REPO_ROOT} ..." >&2
cargo build -p xai-grok-pager-bin --release --bin ezer --manifest-path "${REPO_ROOT}/Cargo.toml"

SRC="${CARGO_TARGET_DIR:-${REPO_ROOT}/target}/release/ezer"
if [ ! -x "$SRC" ]; then
    echo "Error: expected release binary at ${SRC}" >&2
    exit 1
fi

install -m 0755 "$SRC" "${BIN_DIR}/ezer"
echo "  Installed ${BIN_DIR}/ezer" >&2

mkdir -p "${EZER_HOME}/completions/bash" "${EZER_HOME}/completions/zsh"
"${BIN_DIR}/ezer" completions bash > "${EZER_HOME}/completions/bash/ezer.bash" 2>/dev/null || true
"${BIN_DIR}/ezer" completions zsh  > "${EZER_HOME}/completions/zsh/_ezer" 2>/dev/null || true
if mkdir -p "${HOME}/.config/fish/completions" 2>/dev/null; then
    "${BIN_DIR}/ezer" completions fish > "${HOME}/.config/fish/completions/ezer.fish" 2>/dev/null || true
fi

CONFIG_FILE="${EZER_HOME}/config.toml"
if [ ! -f "$CONFIG_FILE" ]; then
    cat > "$CONFIG_FILE" <<'EOF'
# ezer — Responses-first BYOK / custom-provider config
#
# Point this at any OpenAI Responses-compatible gateway
# (for example a local WorkBuddy2API-Hub at http://192.168.0.63:8788/v1).

[models]
default = "my-model"

[model.my-model]
model = "my-model"
base_url = "http://192.168.0.63:8788/v1"
api_backend = "responses"
# api_key = "sk-..."
# env_key = "EZER_API_KEY"

[cli]
auto_update = false
EOF
    echo "  Wrote starter config at ${CONFIG_FILE}" >&2
    echo "  Edit base_url / model id / api_key before the first launch." >&2
fi

path_has_dir() {
    case ":$PATH:" in *":$1:"*) return 0 ;; *) return 1 ;; esac
}

if ! path_has_dir "$BIN_DIR"; then
    for candidate in "${HOME}/.local/bin" "/usr/local/bin"; do
        if path_has_dir "$candidate" && [ -d "$candidate" ] && [ -w "$candidate" ]; then
            ln -sf "${BIN_DIR}/ezer" "${candidate}/ezer"
            echo "  Symlinked ${candidate}/ezer -> ${BIN_DIR}/ezer" >&2
            break
        fi
    done
fi

echo "" >&2
echo "ezer installed. Add ${BIN_DIR} to PATH if needed:" >&2
echo "  export PATH=\"${BIN_DIR}:\$PATH\"" >&2
echo "Then: ezer --help" >&2

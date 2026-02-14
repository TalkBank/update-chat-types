#!/bin/sh
# TalkBank bootstrap: install update-chat-types binary and pre-commit hook.
#
# Usage (local, build from source):
#   ./bootstrap.sh
#
# Usage (remote, pre-built binary):
#   scp update-chat-types user@machine:~/
#   ssh user@machine 'bash -s' < bootstrap.sh --binary ~/update-chat-types
set -e

BINARY_PATH=""
SCRIPT_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"

while [ $# -gt 0 ]; do
    case "$1" in
        --binary)
            BINARY_PATH="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1" >&2
            echo "Usage: $0 [--binary <path>]" >&2
            exit 1
            ;;
    esac
done

# --- Build or locate the binary ---
if [ -n "$BINARY_PATH" ]; then
    if [ ! -f "$BINARY_PATH" ]; then
        echo "error: binary not found at $BINARY_PATH" >&2
        exit 1
    fi
    echo "Using pre-built binary: $BINARY_PATH"
elif [ -f "$SCRIPT_DIR/Cargo.toml" ]; then
    echo "Building from source..."
    (cd "$SCRIPT_DIR" && cargo build --release)
    BINARY_PATH="$SCRIPT_DIR/target/release/update-chat-types"
else
    echo "error: not in the update-chat-types repo and no --binary given" >&2
    exit 1
fi

# --- Create directories ---
TALKBANK_DIR="$HOME/.talkbank"
BIN_DIR="$TALKBANK_DIR/bin"
HOOKS_DIR="$TALKBANK_DIR/hooks"
mkdir -p "$BIN_DIR" "$HOOKS_DIR"

# --- Install binary ---
cp "$BINARY_PATH" "$BIN_DIR/update-chat-types"
chmod +x "$BIN_DIR/update-chat-types"
echo "Installed binary to $BIN_DIR/update-chat-types"

# --- Install hook ---
HOOK_SRC="$SCRIPT_DIR/hooks/pre-commit"
if [ -f "$HOOK_SRC" ]; then
    cp "$HOOK_SRC" "$HOOKS_DIR/pre-commit"
else
    # When run via stdin (ssh pipe), the hook file isn't available locally.
    # Download it or create inline.
    echo "warning: hooks/pre-commit not found at $HOOK_SRC, writing inline copy"
    cat > "$HOOKS_DIR/pre-commit" << 'HOOK_EOF'
#!/bin/sh
# TalkBank pre-commit hook: auto-update @Types headers in CHAT files.
# Installed by bootstrap.sh — applies to all repos via core.hooksPath.

# Only activate in *-data repos.
repo_name=$(basename "$(git rev-parse --show-toplevel 2>/dev/null)" 2>/dev/null)
case "$repo_name" in
    *-data) ;;
    *)      exit 0 ;;
esac

# Graceful degradation: if the binary isn't available, warn and allow commit.
if ! command -v update-chat-types >/dev/null 2>&1; then
    echo "[TalkBank] warning: update-chat-types not found on PATH, skipping @Types check"
    exit 0
fi

repo_root=$(git rev-parse --show-toplevel)

# Run the update. If the tool itself errors (bad 0types.txt, I/O error),
# let the non-zero exit propagate to block the commit.
output=$(update-chat-types --chat-dir "$repo_root" 2>&1)
status=$?

if [ $status -ne 0 ]; then
    echo "[TalkBank] error updating @Types headers:"
    echo "$output"
    exit 1
fi

# Check if any files were updated (line starts with "Updated" and count > 0).
case "$output" in
    "Updated 0 CHAT files.")
        # Nothing changed, carry on.
        ;;
    "Updated "*)
        # Files were modified — stage the changes so they're included in this commit.
        git add -u -- '*.cha'
        echo "[TalkBank] $output"
        ;;
esac

exit 0
HOOK_EOF
fi
chmod +x "$HOOKS_DIR/pre-commit"
echo "Installed hook to $HOOKS_DIR/pre-commit"

# --- Set global hooksPath ---
git config --global core.hooksPath "$HOOKS_DIR"
echo "Set git core.hooksPath to $HOOKS_DIR"

# --- Add bin to PATH in shell profile ---
add_to_path() {
    local profile="$1"
    local line="export PATH=\"$BIN_DIR:\$PATH\""
    if [ -f "$profile" ] && grep -qF "$BIN_DIR" "$profile"; then
        echo "PATH already configured in $profile"
        return
    fi
    echo "" >> "$profile"
    echo "# TalkBank tools" >> "$profile"
    echo "$line" >> "$profile"
    echo "Added $BIN_DIR to PATH in $profile"
}

if [ -f "$HOME/.zshrc" ] || [ "$(basename "$SHELL")" = "zsh" ]; then
    add_to_path "$HOME/.zshrc"
elif [ -f "$HOME/.bashrc" ] || [ "$(basename "$SHELL")" = "bash" ]; then
    add_to_path "$HOME/.bashrc"
else
    # Fallback: try .profile
    add_to_path "$HOME/.profile"
fi

echo ""
echo "Done! Summary:"
echo "  Binary:    $BIN_DIR/update-chat-types"
echo "  Hook:      $HOOKS_DIR/pre-commit"
echo "  hooksPath: $HOOKS_DIR (global)"
echo ""
echo "Restart your shell or run:  export PATH=\"$BIN_DIR:\$PATH\""

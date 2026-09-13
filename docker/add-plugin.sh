#!/usr/bin/env bash
# Create a new Renzora plugin crate.
#
# Usage:
#   ./scripts/add-plugin.sh <name>             # runtime engine plugin
#   ./scripts/add-plugin.sh <name> --editor    # editor-only plugin

set -euo pipefail

usage() {
    cat <<EOF
Usage: $0 <name> [--editor]

Creates crates/renzora_<name>/ with a default plugin skeleton.

  <name>     snake_case suffix; e.g. 'cool_fx' -> crates/renzora_cool_fx/
  --editor   register under [features.editor] (Editor scope, optional dep)

Default (no flags) is a Runtime-scope engine plugin: an rlib baked into
the host binary, wired in by the build generator that reads its add! line.

To create an INSTALLABLE plugin instead — one shipped as source and
compiled on the machine that installs it — see
docs/r1-alpha8/extending/native-plugins.md. Those live outside this
repository entirely.
EOF
    exit "${1:-0}"
}

[ $# -lt 1 ] && usage 1
case "$1" in -h|--help) usage 0 ;; esac

NAME="$1"
shift
EDITOR=false
while [ $# -gt 0 ]; do
    case "$1" in
        --editor) EDITOR=true ;;
        *) echo "Unknown flag: $1" >&2; usage 1 ;;
    esac
    shift
done

if ! [[ "$NAME" =~ ^[a-z][a-z0-9_]*$ ]]; then
    echo "Error: name must match [a-z][a-z0-9_]*" >&2
    exit 1
fi

CRATE="renzora_$NAME"
CRATE_DIR="crates/$CRATE"
RUNTIME_TOML="crates/renzora_runtime/Cargo.toml"

# PascalCase plugin type name from snake_case input
PLUGIN_TYPE=""
IFS='_' read -ra PARTS <<<"$NAME"
for part in "${PARTS[@]}"; do
    PLUGIN_TYPE+="$(tr '[:lower:]' '[:upper:]' <<<"${part:0:1}")${part:1}"
done
PLUGIN_TYPE+="Plugin"

if [ -d "$CRATE_DIR" ]; then
    echo "Error: $CRATE_DIR already exists" >&2
    exit 1
fi
if ! $DYLIB && grep -q "^$CRATE\s*=" "$RUNTIME_TOML"; then
    echo "Error: $CRATE already in $RUNTIME_TOML" >&2
    exit 1
fi

# There is no "both" scope: a plugin is exclusively one or the other, and a
# feature needing editor tooling on top of runtime behaviour ships two plugins.
if $EDITOR; then SCOPE="Editor"; else SCOPE="Runtime"; fi

mkdir -p "$CRATE_DIR/src"

cat > "$CRATE_DIR/Cargo.toml" <<EOF
[package]
name = "$CRATE"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false }
EOF

cat > "$CRATE_DIR/src/lib.rs" <<EOF
use bevy::prelude::*;

#[derive(Default)]
pub struct $PLUGIN_TYPE;

impl Plugin for $PLUGIN_TYPE {
    fn build(&self, _app: &mut App) {
        info!("[$CRATE] $PLUGIN_TYPE loaded");
    }
}

renzora::add!($PLUGIN_TYPE, $SCOPE);
EOF

if $EDITOR; then
    # Append optional dep at end of file, then splice into features.editor.
    printf '\n%s = { path = "../%s", optional = true }\n' "$CRATE" "$CRATE" >> "$RUNTIME_TOML"
    # Insert "dep:renzora_<name>" right before the editor feature's closing ].
    awk -v entry="    \"dep:$CRATE\"," '
        /^editor = \[/ { in_editor=1; print; next }
        in_editor && /^\]$/ { print entry; in_editor=0; print; next }
        { print }
    ' "$RUNTIME_TOML" > "$RUNTIME_TOML.tmp" && mv "$RUNTIME_TOML.tmp" "$RUNTIME_TOML"
else
    # Engine plugin: non-optional dep. Append to end; cargo doesn't care
    # about TOML ordering as long as it's still in [dependencies].
    printf '%s = { path = "../%s" }\n' "$CRATE" "$CRATE" >> "$RUNTIME_TOML"
fi

echo "Created $CRATE_DIR"
echo "  Plugin type:  $PLUGIN_TYPE"
echo "  Scope:        $SCOPE"
echo "  Mode:         static (rlib, baked into binary)"
echo "  Registered:   $RUNTIME_TOML"
echo ""
echo "Next: cargo check -p renzora_runtime$([ $EDITOR = true ] && echo ' --features editor')"

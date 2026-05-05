#!/usr/bin/env bash
# Create a new Renzora plugin crate.
#
# Usage:
#   ./scripts/add-plugin.sh <name>             # engine plugin (Runtime+Editor)
#   ./scripts/add-plugin.sh <name> --editor    # editor-only plugin
#   ./scripts/add-plugin.sh <name> --dylib     # distribution plugin (.dll/.so/.dylib)

set -euo pipefail

usage() {
    cat <<EOF
Usage: $0 <name> [--editor] [--dylib]

Creates crates/renzora_<name>/ with a default plugin skeleton.

  <name>     snake_case suffix; e.g. 'cool_fx' -> crates/renzora_cool_fx/
  --editor   register under [features.editor] (Editor scope, optional dep)
  --dylib    distribution plugin: builds as a standalone cdylib that
             dynamic_plugin_loader picks up from plugins/. Adds the
             renzora "dlopen" feature so the macro emits FFI exports.
             Not added to renzora_runtime — loaded at runtime instead.

Default (no flags) is a statically-linked engine plugin: the crate
defaults to rlib, gets baked into the host binary, and self-registers
via inventory ctors at process start.
EOF
    exit "${1:-0}"
}

[ $# -lt 1 ] && usage 1
case "$1" in -h|--help) usage 0 ;; esac

NAME="$1"
shift
EDITOR=false
DYLIB=false
while [ $# -gt 0 ]; do
    case "$1" in
        --editor) EDITOR=true ;;
        --dylib) DYLIB=true ;;
        *) echo "Unknown flag: $1" >&2; usage 1 ;;
    esac
    shift
done

if $EDITOR && $DYLIB; then
    echo "Error: --editor and --dylib are mutually exclusive (a distribution" >&2
    echo "plugin doesn't share scope with the editor feature gate; if you" >&2
    echo "need editor-only behavior, use the Editor scope inside the plugin)." >&2
    exit 1
fi

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

if $EDITOR; then SCOPE="Editor"; else SCOPE="EditorAndRuntime"; fi

mkdir -p "$CRATE_DIR/src"

if $DYLIB; then
    cat > "$CRATE_DIR/Cargo.toml" <<EOF
[package]
name = "$CRATE"
version = "0.1.0"
edition = "2021"

# Distribution plugin: ships as a standalone .dll/.so/.dylib that
# dynamic_plugin_loader picks up from the engine's plugins/ directory at
# startup. cdylib keeps the export surface to the unmangled extern "C"
# symbols emitted by renzora::add! when this crate's "dlopen" feature
# is on. The macro's gate resolves to the *calling* crate's features,
# so the feature lives here, not on the renzora dep.
[lib]
crate-type = ["cdylib"]

[features]
default = ["dlopen"]
dlopen = []

[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false }
EOF
else
    cat > "$CRATE_DIR/Cargo.toml" <<EOF
[package]
name = "$CRATE"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false }
EOF
fi

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

if $DYLIB; then
    # Distribution plugin: nothing to register — the workspace already
    # auto-includes `crates/*`, and the host loads it at runtime via
    # dlopen, not at link time, so it does NOT go into renzora_runtime's
    # [dependencies].
    :
elif $EDITOR; then
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
if $DYLIB; then
    echo "  Mode:         distribution (cdylib, dlopen)"
    echo "  Workspace:    auto-included via crates/* glob"
    echo ""
    echo "Build with:   cargo build -p $CRATE --profile dist"
    echo "Output:       target/dist/$CRATE.{dll,so,dylib}"
    echo "Install:      copy that file into <renzora-binary>/plugins/"
else
    echo "  Mode:         static (rlib, baked into binary)"
    echo "  Registered:   $RUNTIME_TOML"
    echo ""
    echo "Next: cargo check -p renzora_runtime$([ $EDITOR = true ] && echo ' --features editor')"
fi

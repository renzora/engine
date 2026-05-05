#!/usr/bin/env bash
# Remove a Renzora plugin: deletes the crate directory and unregisters
# it from renzora_runtime's Cargo.toml ([dependencies] + features.editor).
#
# Usage: ./scripts/remove-plugin.sh <name>
#   <name> matches the suffix passed to add-plugin (without the renzora_ prefix).

set -euo pipefail

usage() {
    cat <<EOF
Usage: $0 <name>

Removes crates/renzora_<name>/ and unregisters it from renzora_runtime.
Idempotent: missing-but-referenced or referenced-but-missing both cleaned up.
EOF
    exit "${1:-0}"
}

[ $# -lt 1 ] && usage 1
case "$1" in -h|--help) usage 0 ;; esac

NAME="$1"
CRATE="renzora_$NAME"
CRATE_DIR="crates/$CRATE"
RUNTIME_TOML="crates/renzora_runtime/Cargo.toml"

REMOVED_DIR=false
REMOVED_DEP=false
REMOVED_FEATURE=false

# 1. Strip from renzora_runtime/Cargo.toml: drop the dependency line and any
#    "dep:renzora_<name>" entries inside features.
if grep -qE "^$CRATE\s*=" "$RUNTIME_TOML" || grep -qE "\"dep:$CRATE\"" "$RUNTIME_TOML"; then
    awk -v crate="$CRATE" '
        $0 ~ ("^" crate "[ \t]*=") { next }            # drop dep line
        $0 ~ ("\"dep:" crate "\"") { next }            # drop feature entry
        { print }
    ' "$RUNTIME_TOML" > "$RUNTIME_TOML.tmp"
    mv "$RUNTIME_TOML.tmp" "$RUNTIME_TOML"
    REMOVED_DEP=true
fi

# 2. Delete the crate directory.
if [ -d "$CRATE_DIR" ]; then
    rm -rf "$CRATE_DIR"
    REMOVED_DIR=true
fi

if ! $REMOVED_DIR && ! $REMOVED_DEP; then
    echo "Nothing to do: $CRATE not found in workspace or runtime deps."
    exit 0
fi

echo "✓ Removed $CRATE"
$REMOVED_DIR && echo "  Deleted:      $CRATE_DIR"
$REMOVED_DEP && echo "  Unregistered: $RUNTIME_TOML"

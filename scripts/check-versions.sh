#!/usr/bin/env bash
#
# One version string, checked everywhere it is written down a second time.
#
# `renzora::version::ENGINE_VERSION` is the source of truth (see
# docs/<version>/contributing/releases.md). Four other places have to agree with
# it, and none of them fails loudly when they don't:
#
#   * `docs/<version>/`            — missing, and the docs site 404s the default
#   * `docs/_versions.json`        — disagrees, and the site serves a version the
#                                    engine is not
#   * `RELEASE_NOTES.md` line 1    — stale, and the release publishes the previous
#                                    version's notes under this version's tag
#   * the marketplace's version list — behind, and a seller cannot declare the
#                                    version they built against
#
# Each of those has already gone wrong once. They are cheap to check and the
# check is exact, so it runs on every push rather than being remembered.
#
# Run it locally the same way CI does:  bash scripts/check-versions.sh
set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
err() { echo "  ✗ $*" >&2; fail=$((fail + 1)); }
ok()  { echo "  ✓ $*"; }

version=$(grep -oP 'ENGINE_VERSION: &str = "\K[^"]+' crates/renzora/src/version.rs || true)
if [ -z "$version" ]; then
    echo "✗ could not read ENGINE_VERSION from crates/renzora/src/version.rs" >&2
    exit 1
fi
echo "ENGINE_VERSION = $version"

# ── The docs directory the version names ────────────────────────────────────
if [ -d "docs/$version" ]; then
    ok "docs/$version/ exists"
else
    err "docs/$version/ does not exist — bump and docs fork must land together"
fi

sidebar="docs/$version/_sidebar.json"
if [ -f "$sidebar" ]; then
    sv=$(grep -oP '"version"\s*:\s*"\K[^"]+' "$sidebar" | head -1)
    if [ "$sv" = "$version" ]; then
        ok "$sidebar names $version"
    else
        err "$sidebar names '$sv', not '$version'"
    fi
else
    err "$sidebar is missing"
fi

# ── docs/_versions.json ─────────────────────────────────────────────────────
vfile=docs/_versions.json
default=$(grep -oP '"default"\s*:\s*"\K[^"]+' "$vfile" | head -1)
if [ "$default" = "$version" ]; then
    ok "$vfile defaults to $version"
else
    err "$vfile defaults to '$default' but the engine is '$version'
      Opening the next version's docs before its tag is pushed is what causes
      this: the release workflow reads ENGINE_VERSION as the version and
      compares it against the tag, so the constant cannot run ahead. Cut the
      release first, then bump and fork in one commit after it."
fi

if grep -q "\"id\"\s*:\s*\"$version\"" "$vfile"; then
    ok "$vfile lists $version"
else
    err "$vfile has no entry for $version"
fi

# Every version the site offers must be a directory that exists, or the
# selector has a dead option.
while read -r id; do
    [ -n "$id" ] || continue
    [ -d "docs/$id" ] || err "$vfile offers '$id' but docs/$id/ does not exist"
done < <(grep -oP '"id"\s*:\s*"\K[^"]+' "$vfile")

# ── RELEASE_NOTES.md ────────────────────────────────────────────────────────
if [ -f RELEASE_NOTES.md ]; then
    if head -n 5 RELEASE_NOTES.md | grep -qF "$version"; then
        ok "RELEASE_NOTES.md names $version in its opening lines"
    else
        err "RELEASE_NOTES.md does not name $version in its opening lines — it is
      still the previous release's notes. The notes carry the version in an HTML
      comment on line 1, which GitHub renders as nothing, so the published body
      does not have to repeat a title the release page already shows. The publish
      job refuses a release on this, but only after the tag is pushed; here it
      costs nothing to catch."
    fi
else
    err "RELEASE_NOTES.md is missing"
fi

# ── The marketplace's version list ──────────────────────────────────────────
# The current version is derived from ENGINE_VERSION, so it cannot fall behind;
# what can rot is the hand-written tail of past versions.
panel=crates/renzora_marketplace/src/upload_panel.rs
past=$(grep -oP 'PAST_ENGINE_VERSIONS: &\[&str\] = &\[\K[^]]*' "$panel" | tr -d '" ' || true)
if [ -z "$past" ]; then
    err "could not read PAST_ENGINE_VERSIONS from $panel"
else
    IFS=',' read -ra pv <<< "$past"
    for p in "${pv[@]}"; do
        [ -n "$p" ] || continue
        [ -d "docs/$p" ] || err "PAST_ENGINE_VERSIONS names '$p' with no docs/$p/"
    done
    ok "PAST_ENGINE_VERSIONS: ${past}"
fi

echo
if [ "$fail" -gt 0 ]; then
    echo "✗ $fail version inconsistency(ies)" >&2
    exit 1
fi
echo "✓ every copy of the version agrees with ENGINE_VERSION"

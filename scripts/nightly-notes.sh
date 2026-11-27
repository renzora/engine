#!/usr/bin/env bash
#
# Print what a nightly adds: the RELEASE_NOTES.md lines that are new since the
# previous nightly, under their timestamp headings.
#
#   scripts/nightly-notes.sh <current RELEASE_NOTES.md> [<previous nightly's copy>]
#
# Used by the "Create release" step of `.github/workflows/build-engine.yml`, which
# fetches the previous copy from the last nightly's tag. Run it by hand to preview
# a page:
#
#   git show r1-alpha8-nightly-26nov26:RELEASE_NOTES.md > /tmp/prev.md
#   scripts/nightly-notes.sh RELEASE_NOTES.md /tmp/prev.md
#
# ── Why a diff, not a marker ─────────────────────────────────────────────────
# The page used to list every timestamped section above the first heading that
# is not a timestamp, and the boundary was a `## <tag>` heading someone had to
# add by hand after each nightly. Nobody did for weeks, so every nightly from
# 29 Oct republished the same five hundred sections, and one outgrew GitHub's
# release-body limit. Comparing against the file as the previous nightly saw it
# needs no manual step, and it is right even when a step is skipped.
#
# ── What counts as new ───────────────────────────────────────────────────────
# A line is keyed by the timestamp heading it sits under, so the same wording
# under two different changes stays two lines. A line edited after it shipped
# counts as new, which is what a reader wants: it changed. A heading prints only
# when at least one line under it is new.
#
# The current file is still read only down to its first `##` heading that is not
# a timestamp (a published tag heading, or the prose of a curated release
# document), so a hand-added tag heading keeps working as an extra fence. The
# previous copy is read whole: anything that existed anywhere in it was shipped.
#
# Each timestamp is demoted two levels, so it nests under the page's
# "### Since the last nightly". With no previous copy every timestamped section
# prints, which is what the page did before.

set -euo pipefail

current="${1:?usage: nightly-notes.sh <current-notes> [<previous-notes>]}"
previous="${2:-/dev/null}"

awk -v prev="$previous" '
    FNR == 1 { heading = ""; stop = 0 }
    /^##[[:space:]]+[0-9]/ {
        if (stop) next
        heading = $0
        shown = 0
        next
    }
    /^##[[:space:]]/ {
        if (FILENAME != prev && heading != "") stop = 1
        heading = ""
        next
    }
    stop || heading == "" || /^[[:space:]]*$/ { next }
    FILENAME == prev { shipped[heading SUBSEP $0] = 1; next }
    !((heading SUBSEP $0) in shipped) {
        if (!shown) {
            if (any) print ""
            print "##" heading
            shown = 1
            any = 1
        }
        print
    }
' "$previous" "$current"

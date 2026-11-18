#!/usr/bin/env bash
set -euo pipefail

scripts="$(cd "$(dirname "$0")" && pwd)"
root="${GATE_ROOT:-$(cd "$scripts/../.." && pwd)}"
source "$scripts/tracked-libraries.sh"

scan_hits=""

scan() {
    local rc=0
    scan_hits="$(grep -rnE --exclude-dir=target "$@")" || rc=$?
    if [ "$rc" -ge 2 ]; then
        echo "::error::grep failed with status ${rc}: the gate could not scan, so it refuses to call the crate clean" >&2
    fi
    return "$rc"
}

fail=0
checked=0
for crate in ${GATE_CRATES-${GO_FREE_CRATES:-}}; do
    dir="$root/crates/$crate"
    if [ ! -d "$dir" ]; then
        echo "::error::GO_FREE_CRATES names ${crate} but ${dir} does not exist" >&2
        fail=1
        continue
    fi
    checked=$((checked + 1))

    hits=""
    scan_failed=0

    rc=0
    scan --include='*.rs' --include='*.toml' --include='*.sh' --include='*.yml' \
        -e '\b(go|make)[[:space:]]+(build|run|install|test|vet|generate|-C)\b' \
        -e 'ensure_go_available' \
        -e '[/"](go|make)"' \
        "$dir" || rc=$?
    case "$rc" in
        0) hits="$scan_hits" ;;
        1) ;;
        *) scan_failed=1 ;;
    esac

    rc=0
    scan --include='*.rs' --include='*.toml' --include='*.sh' --include='*.yml' \
        -e 'temp_dir|tempdir|tempfile' \
        "$dir" || rc=$?
    case "$rc" in
        0) hits="$hits"$'\n'"$scan_hits" ;;
        1) ;;
        *) scan_failed=1 ;;
    esac

    if [ "$scan_failed" -ne 0 ]; then
        fail=1
        continue
    fi
    if [ -n "$hits" ]; then
        echo "::error::${crate} compiles or builds a Go subject inside its own code; the subject binary must come from PATH or a release asset" >&2
        printf '%s\n' "$hits" | sed '/^$/d' >&2
        fail=1
        continue
    fi
    echo "✓ ${crate} builds no Go subject"
done

if [ "$checked" -eq 0 ] && [ "$fail" -eq 0 ]; then
    echo "::error::the crate list is empty: the gate checks nothing" >&2
    exit 1
fi
exit $fail

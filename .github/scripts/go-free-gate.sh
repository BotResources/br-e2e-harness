#!/usr/bin/env bash
set -euo pipefail

scripts="$(cd "$(dirname "$0")" && pwd)"
root="${GATE_ROOT:-$(cd "$scripts/../.." && pwd)}"
source "$scripts/tracked-libraries.sh"

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
    hits="$(grep -rnE --include='*.rs' --include='*.toml' --include='*.sh' --include='*.yml' \
        --exclude-dir=target \
        -e '\bgo (build|vet|install|run|test)\b' \
        -e 'Command::new\("go"\)' \
        -e 'ensure_go_available' \
        -e '"(go|make)"' \
        "$dir" || true)"
    hits_temp="$(grep -rnE --include='*.rs' --exclude-dir=target \
        -e 'temp_dir' "$dir/src" "$dir/tests" 2>/dev/null || true)"
    if [ -n "$hits$hits_temp" ]; then
        echo "::error::${crate} compiles or builds a Go subject inside its own code; the subject binary must come from PATH or a release asset" >&2
        printf '%s\n' "$hits" "$hits_temp" | sed '/^$/d' >&2
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

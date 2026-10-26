#!/usr/bin/env bash
set -euo pipefail

rev="${1:-}"
only="${2:-}"
scripts="$(cd "$(dirname "$0")" && pwd)"
source "$scripts/tracked-libraries.sh"

fail=0
for pair in ${COMPANION_CRATES:-}; do
  companion="${pair%%:*}"
  primary="${pair##*:}"
  if [ -n "$only" ] && [ "$primary" != "$only" ]; then
    continue
  fi
  companion_version=$(bash "$scripts/crate-version.sh" "$companion" "$rev")
  primary_version=$(bash "$scripts/crate-version.sh" "$primary" "$rev")
  if [ "$companion_version" != "$primary_version" ]; then
    echo "::error::${companion} declares ${companion_version} but ${primary} declares ${primary_version}: a companion crate is versioned with its library and must carry the same version" >&2
    fail=1
    continue
  fi
  bash "$scripts/check-changelog.sh" "$companion" "$rev" || fail=1
  echo "✓ ${companion} follows ${primary} at ${primary_version}"
done

exit $fail

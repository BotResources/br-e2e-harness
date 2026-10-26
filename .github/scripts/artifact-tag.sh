#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: $0 <crate> <tag>" >&2
    exit 2
fi

crate="$1"
tag="$2"
scripts="$(cd "$(dirname "$0")" && pwd)"
pattern="^${crate}/v([0-9]+\.[0-9]+\.[0-9]+)\$"

if [[ ! "$tag" =~ $pattern ]]; then
    echo "::error::'${tag}' is not a ${crate} release tag (${crate}/vMAJOR.MINOR.PATCH): nothing is built from it." >&2
    exit 1
fi
version="${BASH_REMATCH[1]}"

if ! git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
    echo "::error::${tag} does not exist in this clone." >&2
    exit 1
fi

declared="$(bash "$scripts/crate-version.sh" "$crate" "refs/tags/${tag}")"
if [[ "$declared" != "$version" ]]; then
    echo "::error::${tag} carries ${crate} ${declared} in Cargo.toml, not ${version}." >&2
    exit 1
fi

if ! gh release view "$tag" >/dev/null 2>&1; then
    echo "::error::${tag} has no GitHub release. The release is created by release-tags after the registry gate; an artifact is built only for a released tag." >&2
    exit 1
fi

echo "$version"

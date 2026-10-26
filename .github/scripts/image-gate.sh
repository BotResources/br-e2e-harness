#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: $0 <image> <version> <commit>" >&2
    exit 2
fi

image="$1"
version="$2"
commit="$3"
short="${commit:0:12}"

published() { docker manifest inspect "$1" >/dev/null 2>&1; }

if ! published "${image}:${version}"; then
    echo "skip=false"
    exit 0
fi

missing=()
for arch in amd64 arm64; do
    published "${image}:${version}-${short}-${arch}" || missing+=("${version}-${short}-${arch}")
done

if [[ ${#missing[@]} -gt 0 ]]; then
    echo "::error::${image}:${version} already exists but was not published from commit ${commit}: ${missing[*]} missing. A published tag is never overwritten; release a new version." >&2
    exit 1
fi

echo "::notice::${image}:${version} is already published from ${commit} - nothing to publish" >&2
echo "skip=true"

#!/usr/bin/env bash
set -euo pipefail

version=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/^version = "([^"]+)".*/\1/')
if [ -z "$version" ]; then
  echo "::error file=Cargo.toml::could not extract the workspace version" >&2
  exit 1
fi

fail=0
while IFS= read -r readme; do
  while IFS= read -r hit; do
    line=${hit#*:}
    number=${hit%%:*}
    tag=$(printf '%s\n' "$line" | sed -E 's/.*tag = "([^"]+)".*/\1/')
    if [ "$tag" != "v${version}" ]; then
      echo "::error file=${readme},line=${number}::pins br-e2e-harness at ${tag}, workspace version is ${version}; expected tag = \"v${version}\"" >&2
      fail=1
    fi
  done < <(grep -n 'br-e2e-harness.*tag = "v' "$readme" || true)
done < <(find . -name README.md -not -path './target/*')

if [ "$fail" -ne 0 ]; then
  exit 1
fi

echo "✓ every README br-e2e-harness pin is v${version}"

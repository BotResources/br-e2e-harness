#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: $0 <crate> <version> <line>" >&2
    exit 2
fi

crate="$1"
version="$2"
line="$3"
tag="${crate}/v${version}"

case "$crate" in
    oidc-test-idp) workflow="cd.yml" ;;
    conformance-scope) workflow="release-cli.yml" ;;
    conformance-passport)
        gh workflow run "release-anchor.yml" --ref "$line" -f "tag=${tag}"
        echo "dispatched release-anchor.yml for ${tag} on ${line}"
        workflow="release-cli-passport.yml"
        ;;
    *)
        echo "${crate}: no artifact beyond the tag and the release."
        exit 0
        ;;
esac

gh workflow run "$workflow" --ref "$line" -f "tag=${tag}"
echo "dispatched ${workflow} for ${tag} on ${line}"

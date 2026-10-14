#!/usr/bin/env bash
set -euo pipefail

LAW="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/release-line.sh"

export RELEASE_BRANCHES="
release/conformance-scope-0.5
release/oidc-test-idp-1.2
release/0.5
"

failures=0
cases=0

expect() {
    local want="$1" what="$2" got out rc=0
    shift 2
    cases=$((cases + 1))
    out="$(bash "$LAW" "$@" 2>&1)" || rc=$?
    case "$rc" in
        0) got=allowed ;;
        1) got=invalid ;;
        3) got=refused ;;
        *) got="exit-${rc}" ;;
    esac
    if [[ "$got" == "$want" ]]; then
        printf '  ok  %-8s %s\n' "$got" "$what"
    else
        printf 'FAIL  %-8s %s (expected %s)\n        %s\n' "$got" "$what" "$want" "${out//$'\n'/ | }"
        failures=$((failures + 1))
    fi
}

expect_out() {
    local want="$1" what="$2" got
    shift 2
    cases=$((cases + 1))
    got="$(bash "$LAW" "$@" 2>/dev/null || echo "<exit $?>")"
    if [[ "$got" == "$want" ]]; then
        printf '  ok  %-8s %s\n' "$got" "$what"
    else
        printf 'FAIL  %s (expected %s, got %s)\n' "$what" "$want" "$got"
        failures=$((failures + 1))
    fi
}

echo "== rule 4: only main moves the Latest pointer"
expect_out true  "main"                          may-move-latest refs/heads/main
expect_out false "release line"                  may-move-latest refs/heads/release/conformance-scope-0.5
expect_out false "a tag ref"                     may-move-latest refs/tags/conformance-scope/v0.5.3
expect_out false "a branch named like main"      may-move-latest refs/heads/mainline
expect_out false "no ref"                        may-move-latest ""

echo "== rule 1: one minor range, one branch (from main)"
expect allowed "main / conformance-scope 0.6.0: the next minor"                 check conformance-scope 0.6.0 refs/heads/main
expect allowed "main / conformance-scope 1.0.0: a later major"                  check conformance-scope 1.0.0 refs/heads/main
expect refused "main / conformance-scope 0.5.4: the minor its line owns"        check conformance-scope 0.5.4 refs/heads/main
expect refused "main / conformance-scope 0.4.2: an older minor"                 check conformance-scope 0.4.2 refs/heads/main
expect allowed "main / conformance 0.5.9: another crate that is a prefix"  check conformance 0.5.9 refs/heads/main
expect allowed "main / conformance-scope-cli 0.5.4: a longer name is another crate"  check conformance-scope-cli 0.5.4 refs/heads/main
expect refused "main / oidc-test-idp 1.2.1: its own line owns 1.2"      check oidc-test-idp 1.2.1 refs/heads/main
expect allowed "main / oidc-test-idp 1.3.0: the next minor"             check oidc-test-idp 1.3.0 main
expect allowed "main / a prerelease of the next minor"                        check conformance-scope 0.6.0-rc.1 refs/heads/main
expect refused "main / a prerelease inside the owned minor"                   check conformance-scope 0.5.9-rc.1 refs/heads/main

echo "== rules 1 and 2: from a release line"
expect allowed "release/conformance-scope-0.5 / conformance-scope 0.5.3: a patch"       check conformance-scope 0.5.3 refs/heads/release/conformance-scope-0.5
expect refused "release/conformance-scope-0.5 / conformance-scope 0.6.0: next minor"    check conformance-scope 0.6.0 refs/heads/release/conformance-scope-0.5
expect refused "release/conformance-scope-0.5 / conformance-scope 0.4.9: older minor"   check conformance-scope 0.4.9 refs/heads/release/conformance-scope-0.5
expect refused "release/conformance-scope-0.5 / conformance-scope 1.5.0: other major"   check conformance-scope 1.5.0 refs/heads/release/conformance-scope-0.5
expect refused "release/conformance-scope-0.5 / oidc-test-idp 1.2.4: another crate" check oidc-test-idp 1.2.4 refs/heads/release/conformance-scope-0.5

echo "== rule 3: only main and release lines publish"
expect invalid "release/0.5: the artifact-less name"          check conformance-scope 0.5.3 refs/heads/release/0.5
expect invalid "release/conformance-scope-0.5.1: a patch name"  check conformance-scope 0.5.1 refs/heads/release/conformance-scope-0.5.1
expect invalid "feat/x: a feature branch"                     check conformance-scope 0.6.0 refs/heads/feat/x
expect invalid "a tag ref"                                    check conformance-scope 0.5.3 refs/tags/conformance-scope/v0.5.3
expect invalid "a malformed version"                          check conformance-scope 0.5 refs/heads/main
expect allowed "a prerelease with build metadata"             check conformance-scope 0.6.0-rc.1+build.7 refs/heads/main
expect invalid "a version that would reshape a JSON matrix"   check conformance-scope '0.6.0-x","line":"main' refs/heads/main
expect invalid "a version carrying a command substitution"    check conformance-scope "0.6.0-\$(id)" refs/heads/main
expect invalid "a crate name with an underscore"              check foo_bar 0.1.0 refs/heads/main
expect invalid "a crate name starting with a digit"            check 9abc 0.1.0 refs/heads/main
expect allowed "a crate name outside the br- prefix"          check br-test-harness 0.1.0 refs/heads/main
expect invalid "an upper-case crate name"                     check Br-Foo 0.1.0 refs/heads/main
expect allowed "check-ref main"                               check-ref refs/heads/main
expect allowed "check-ref a release line"                     check-ref refs/heads/release/oidc-test-idp-1.2
expect invalid "check-ref release/0.5"                        check-ref refs/heads/release/0.5

echo "== branch names"
expect_out "oidc-test-idp 1 2" "parse release/oidc-test-idp-1.2" parse release/oidc-test-idp-1.2
expect invalid "parse release/conformance-scope-v0.5"                          parse release/conformance-scope-v0.5

echo "== no release branch at all"
RELEASE_BRANCHES="" expect allowed "main / conformance-scope 0.5.4 with no line cut" check conformance-scope 0.5.4 refs/heads/main

echo
if [[ "$failures" -gt 0 ]]; then
    echo "test-release-line: ${failures}/${cases} case(s) failed" >&2
    exit 1
fi
echo "test-release-line: ${cases} cases passed"

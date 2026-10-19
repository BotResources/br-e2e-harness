#!/usr/bin/env bash
set -euo pipefail

scripts="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-go-free-gate.XXXXXX")"
trap 'rm -rf "$work"' EXIT

failures=0
cases=0

expect() {
    local want="$1" what="$2"
    shift 2
    local rc=0 out
    out="$("$@" 2>&1)" || rc=$?
    cases=$((cases + 1))
    if [ "$rc" -eq "$want" ]; then
        printf '  ok  %s\n' "$what"
    else
        failures=$((failures + 1))
        printf 'FAIL  %s (exit %s, wanted %s)\n%s\n' "$what" "$rc" "$want" "$out"
    fi
}

gate() {
    GATE_ROOT="$work" GATE_CRATES="$1" bash "$scripts/go-free-gate.sh"
}

mkdir -p "$work/crates/clean/src" "$work/crates/clean/tests"
printf 'fn main() {}\n' >"$work/crates/clean/src/main.rs"
printf 'fn t() { let _ = std::process::Command::new("cargo"); }\n' >"$work/crates/clean/tests/t.rs"
printf '# go build is documented here\n' >"$work/crates/clean/README.md"
printf 'run: cargo build --release\n' >"$work/crates/clean/ci.yml"

echo "== the gate judges the listed crates"
expect 0 "a crate that builds nothing passes, prose in markdown is ignored" gate "clean"
expect 1 "an empty crate list checks nothing and fails" gate ""
expect 1 "a listed crate that does not exist fails" gate "missing"

echo "== every way of building a Go subject in a crate is refused"
plant() {
    local name="$1" file="$2" body="$3"
    mkdir -p "$work/crates/$name/src" "$work/crates/$name/tests"
    printf '%s\n' "$body" >"$work/crates/$name/$file"
    expect 1 "$name: refused" gate "clean $name"
}
plant go_command src/lib.rs 'fn b() { std::process::Command::new("go"); }'
plant go_build_arg tests/t.rs 'const A: &str = "go build -o x .";'
plant go_vet_script src/vet.sh 'go vet ./...'
plant go_workflow src/ci.yml 'run: go build ./...'
plant go_split_literals src/lib.rs 'fn b() { run_once("go", &["build", "-o", "x", "."]); }'
plant make_build src/lib.rs 'fn b() { std::process::Command::new("make").args(["-C", "d", "build"]); }'
plant go_const src/lib.rs 'const GO: &str = "go"; fn b() { std::process::Command::new(GO).arg("build"); }'
plant go_probe src/lib.rs 'pub fn ensure_go_available() {}'
plant temp_subject tests/t.rs 'fn d() { let _ = std::env::temp_dir().join("subject"); }'

echo "== a crate that is not listed is not judged"
expect 0 "an unlisted crate with a Go build passes" gate "clean"

echo "== $((cases - failures))/${cases} cases passed"
[ "$failures" -eq 0 ]

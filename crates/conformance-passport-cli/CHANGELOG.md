# Changelog

All notable changes to `conformance-passport-cli` are documented here. This crate
is not a registered library: it is versioned with `conformance-passport` (its
version always equals that crate's), and its release binaries ride the git tag
`conformance-passport/v{version}`. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Added

- First release of the CLI, with `conformance-passport` 0.1.1 under the tag `conformance-passport/v0.1.1`: the static binaries `conformance-passport-0.1.1-<target>.tar.gz` (linux musl x86_64, macOS arm64, each with its `.sha256`) are attached to that release by the `release-cli-passport` workflow, next to the `identity-passport` Go anchor assets. The CLI is not a registered library; its version always equals that of `conformance-passport`.
- `conformance-passport run --spawn <PATH>` spawns the caller's service binary against a throwaway `nats-server` with the `PUBLISHED_LANGUAGE` bucket and runs P1..P10 through `conformance_passport::run_spawn`; `--anchor` drives the Go reference subject `identity-passport`, looked up through `conformance_passport::locate_subject` (`PATH`, cache directory, release download). `--scenarios`, `--timeout`, `--subject-label`, `--format human|json|junit` and `--output` complete the surface; `--help` is the source of truth.
- Exit codes: 0 conformant, 1 non-conformant (a check failed or was skipped), 2 for an error that stopped the run (unknown scenario, bad flag, a binary that is not an executable file or not on `PATH`, `SubjectBinaryUnavailable` for a missing anchor, a subject that never reports `/readyz`). A panic inside the run (for example `br-test-harness` aborting because `nats-server` is not on `PATH`) is contained and reported as one `error:` line with exit 2; `--timeout` values that overflow are rejected with exit 2. A report that cannot be written (closed or full stdout, unwritable `--output`) is one `error:` line with exit 2; the panic hook is restored once the run ends.
- There is no attach mode: `conformance-passport` offers only `run_spawn`, and P8 destroys the `PUBLISHED_LANGUAGE` bucket, which no live service's broker may lose.
- It depends on `conformance-passport` through a workspace path dependency, not a tag pin: the CLI is released from the same commit as the library it drives, and no consumer depends on the CLI as a crate. The binary holds no protocol logic.

# Changelog

All notable changes to `conformance-identity-cli` are documented here. This crate
is not a registered library: it is versioned with `conformance-identity` (its
version always equals that crate's), and its release binaries ride the git tag
`conformance-identity/v{version}`. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Added

- First release of the CLI, with `conformance-identity` 0.1.1 under the tag `conformance-identity/v0.1.1`: the archives `conformance-identity-0.1.1-<target>.tar.gz` (linux musl x86_64, macOS arm64, each with its `.sha256`) are attached to that release by the `release-cli-identity` workflow, next to the `identity-acceptor` Go anchor assets. Each archive holds `conformance-identity` and `fabric-nats`, which the battery provisions its throwaway NATS with and looks up beside the running executable. The CLI is not a registered library; its version always equals that of `conformance-identity`.
- `conformance-identity run --spawn <PATH>` spawns the caller's acceptor binary against a throwaway `nats-server` through `conformance_identity::run_spawn`; `--anchor` drives the Go reference acceptor `identity-acceptor`, looked up through `conformance_identity::locate_subject` (`PATH`, cache directory, release download); `--attach --nats <URL> --readyz <URL>` drives a running acceptor through `conformance_identity::run_attach`. Exactly one mode is required. `--scenarios`, `--timeout`, `--subject-label`, `--format human|json|junit` and `--output` complete the surface; `--help` is the source of truth.
- Exit codes: 0 conformant, 1 non-conformant (a check failed or was skipped), 2 for an error that stopped the run (unknown scenario, bad flag, a binary that is not an executable file or not on `PATH`, `SubjectBinaryUnavailable` for a missing anchor, `fabric-nats` not beside the executable, `nats-server` not on `PATH`, an acceptor that never reports `/readyz`, absent fixed streams in attach mode). A panic inside the run is contained and reported as one `error:` line with exit 2. A `--timeout` that the platform clock cannot add to the current instant is rejected before any infra starts, with one `error: invalid --timeout ...: value too large` line and exit 2. A report that cannot be written (closed or full stdout, unwritable `--output`) is one `error:` line with exit 2; the panic hook is restored once the run ends.
- It depends on `conformance-identity` through a workspace path dependency, not a tag pin: the CLI is released from the same commit as the library it drives, and no consumer depends on the CLI as a crate. The binary holds no protocol logic.

### Fixed

- The JUnit and human reports stay well-formed whatever the text under judgment holds: a character that XML 1.0 forbids (C0 controls other than tab, line feed and carriage return, U+FFFE, U+FFFF) is written as U+FFFD in the JUnit file, and a control character other than newline and tab is written as U+FFFD in the human report. Before, a service answering with an escape character made the JUnit file unparsable, so the report was lost exactly when a non-conformant service was judged.

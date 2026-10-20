# Changelog

All notable changes to `conformance-directory-cli` are documented here. This crate
is not a registered library: it is versioned with `conformance-directory` (its
version always equals that crate's), and its release binaries ride the git tag
`conformance-directory/v{version}`. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Added

- First release of the CLI, with `conformance-directory` 0.1.1 under the tag `conformance-directory/v0.1.1`: the archives `conformance-directory-0.1.1-<target>.tar.gz` (linux musl x86_64, macOS arm64, each with its `.sha256`) are attached to that release by the `release-cli-directory` workflow, next to the `identity-directory` Go anchor assets. Each archive holds `conformance-directory` and `fabric-nats`, which the publisher and consumer checks provision their throwaway NATS with and look up beside the running executable. The CLI is not a registered library; its version always equals that of `conformance-directory`.
- `conformance-directory run` locates the Go anchor `identity-directory` through `conformance_directory::locate_and_emit` (`PATH`, cache directory, release download), or runs the file given to `--anchor-binary` through `conformance_directory::emit_snapshot`, and runs W1..W6, P1..P2 and C1..C6 through `conformance_directory::run_battery`. `--checks`, `--subject-label`, `--format human|json|junit` and `--output` complete the surface; `--help` is the source of truth. Checks always run in battery order, whatever the order of `--checks`.
- There is no spawn or attach mode: the battery drives the `br-util-directory` publisher and consumer kit and the frozen wire, and has no service binary to start. The consumer checks C1..C6 need a Postgres, read from `E2E_PG_ADMIN_URL` or `DATABASE_URL`; W1..W6 need only the anchor.
- Exit codes: 0 conformant, 1 non-conformant (a check failed or was skipped), 2 for an error that stopped the run (unknown check, bad flag, an anchor that is missing or cannot run, `fabric-nats` not beside the executable, `nats-server` not on `PATH`, no reachable Postgres). A panic inside the run is contained and reported as one `error:` line with exit 2. A report that cannot be written (closed or full stdout, unwritable `--output`) is one `error:` line with exit 2; the panic hook is restored once the run ends.
- It depends on `conformance-directory` through a workspace path dependency, not a tag pin. The binary holds no protocol logic: the code-to-check mapping and the run order live in the battery crate (`parse_checks`, `run_battery`, `ALL_CHECKS`).

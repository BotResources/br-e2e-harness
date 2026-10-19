# Changelog

All notable changes to `conformance-scope-cli` are documented here. This crate
is not a registered library: it is versioned with `conformance-scope` (its
version always equals that crate's), and its release binaries ride the git tag
`conformance-scope/v{version}`. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Fixed

- The aggregate `conformant` verdict of a report is true only when it holds at least one service report and every service report is conformant; with `conformance-scope` 0.1.1 a service report with no outcome or with a skipped check is no longer conformant. A run that selects no scenario is a usage error (`conformance-scope` returns `InvalidInput`).
- `run --attach` against a broker missing either handshake stream (`INTEGRATION_CMD` or `INTEGRATION_EVT`) or an unreachable broker exits `2` with a typed error and no longer creates the streams; it no longer reports that setup error as a non-conformant service (exit `1`).
- A malformed `--readyz` URL (no `http`/`https` scheme or no host) exits `2` naming the URL instead of exiting `1` with a failed S2. Pinned by `tests/exit_codes.rs`.
- `render::junit`, `render::human` and the XML escaping are pinned by unit tests, and `tests/exit_codes.rs` runs `--format junit --output <file>` against a real service and asserts the counts and the `<failure>` element, the output mode the Run in CI recipe prescribes.
- `--spawn` with an expectation whose scopes carry different `platform_only` flags is refused by `conformance-scope` before anything starts.
- The `spawn_cli` test locates the prebuilt `scope-service` binary (`PATH`, cache directory, release asset) instead of compiling it; the `release-cli` workflow attaches the `scope-service` assets next to the CLI binaries.

- `--spawn` without the `fabric-nats` binary beside the executable (the released archive holds only `conformance-scope`) or without the provisioning manifest exits `2` with an error naming it, no longer exit `101` (the harness panic text can still appear on stderr before the typed error); both READMEs name the prerequisite and the exit-code contract lists it.
- The `observed` field of a failed check holds what was seen, in the human, JSON and JUnit output.

- The manifest ignored unknown fields, so a typo such as `platform_onyl` read as a pass; `Manifest`, `ServiceSpec`, `ScopeSpec`, `AttachSpec` and `SpawnSpec` now refuse them (exit `2`, naming the field).
- `--platform-only key=bool` entries naming no listed scope, or given twice, and `--scopes` keys that no declaration can hold (invalid, not owned by `--service-key`, duplicated) exit `2` naming the key, no longer `0` (stray key) or `1` (blamed on the service). The CLI calls `ExpectedDeclaration::try_from_parts`; `conformance-scope` validates the rest before any I/O.
- `--timeout` (and the manifest `timeout`) refuses `0` in every unit and a value that overflows (`307445734561825862m` panicked in a debug build, wrapped in a release build); both exit `2` naming the value.

### Added

- `tests/exit_codes.rs`: assert_cmd tests of the exit-code contract against a real `nats-server`: exit `2` for an unreachable broker, a broker without the handshake streams (none is created), a missing `--nats` or `--readyz`, an unknown scenario, a spawn binary that is not a file, a missing `nats-server` on `PATH`, a released layout without `fabric-nats` and an unreadable or malformed manifest; exit `0` with a passing JSON report for an attached conformant service, and the manifest mode writing its aggregate report to `--output`; exit `2` for an unknown manifest field, a stray or duplicated `--platform-only` key, scope keys no declaration can hold, and a zero or overflowing `--timeout`, each against a broker that holds both streams.
- A "Run in CI" section in the README, with the pinned, checksum-verified download of the released binary and the attach-mode run against the service's own booted binary.

### Changed

- Version follows `conformance-scope` (0.1.1); the path dependency requires `conformance-scope` 0.1.1.

## 0.1.0

### Changed

- First release of the CLI, with `conformance-scope` 0.1.0 under the tag `conformance-scope/v0.1.0`: the static binaries `conformance-scope-0.1.0-<target>.tar.gz` are attached to that release. The CLI is not a registered library; its version always equals that of `conformance-scope`. The `v1.x` workspace tags are the legacy line and are untouched.
- It depends on `conformance-scope` through a workspace path dependency, not a tag pin: the CLI is released from the same commit as the library it drives, the tag `conformance-scope/v0.1.0` does not exist before that commit is merged, and no consumer depends on the CLI as a crate.
- The manifest YAML is read with `serde-saphyr` (pure Rust) instead of `serde_yaml_ng`, which pulled in the unmaintained `unsafe-libyaml` (RUSTSEC-2024-0449). The manifest format is unchanged.


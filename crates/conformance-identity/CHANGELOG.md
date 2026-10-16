# Changelog

All notable changes to `conformance-identity` are documented here. The git tag
`conformance-identity/v{version}` releases the crate; this crate is versioned
separately from the other crates of the repository. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Fixed

- `ConformanceReport::is_conformant` is true only for a report with at least one passed check and no failed one; an empty report or a report of skipped checks no longer gets a conformant verdict. `run_spawn` and `run_attach` return `ConformanceError::InvalidInput` when the scenario slice is empty, so a caller that selects nothing no longer gets a green result.
- A rejected confirmation is judged on the service it names as well as on its reason, like an accepted one: a subject that rejects a declaration but names another service now fails the scenario. The report text shows the service of the captured confirmation.
- A missing prerequisite is a typed `ConformanceError`, not a panic: `nats-server` not on `PATH`, the `fabric-nats` CLI not built beside the test executable (also from `provision`), an acceptor binary that is not a file, a throwaway NATS that does not start, an unreachable NATS in `run_attach`, a confirmation consumer that cannot bind, an acceptor that cannot be spawned, and an absent `identity-acceptor` directory in `build_subject` (`ConformanceError::Build`). A harness panic while a scenario observes the subject is contained and reported as a failed check.
- `expected_verdict` returns `ConformanceError::InvalidInput` on an empty sequence instead of panicking.
- In a failing multi-step scenario the `expected` text is the oracle verdict of the failing step, not of the last step; on a pass the `observed` text is rendered from the captured confirmation, not rebuilt from the oracle.
- `run_spawn` shuts the throwaway NATS and the capture down on every early failure.
- `subject_dir()` no longer panics when the directory is absent.

### Added

- `try_subject_dir()`, the non-panicking form of `subject_dir()`.
- `DecodedConfirmation`, `CapturedConfirmation::decode_confirmation`, `ConfirmationCapture::confirmation_decoded_for`, `expected_step_confirmations`, `oracle::confirmation_code`: the verdict together with the service the confirmation names.

### Changed

- Pins `br-test-harness/v0.1.1`; the `br-rust-common` pins stay on their latest tags (`0.1.0`).
- New dependency `futures-util` (default features off, `std`) to contain harness panics.

## 0.1.0

### Changed

- First registry release of `conformance-identity` as its own library, tagged `conformance-identity/v0.1.0`. The crate is the black-box conformance runner for the scope-acceptance side of the scope-declaration handshake (battery G2): it drives an Identity registry against the frozen Go declaring subject. It is a test fixture only. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line, `0.1`. The earlier `v1.x` workspace tags are the legacy line and are untouched.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-identity", tag = "conformance-identity/v0.1.0", version = "0.1.0" }`.
- The `br-rust-common` dependencies are pinned to the per-crate `0.1.0` tags (`br-core-integration`, `br-core-scope`, `br-identity-domain`, `br-scope-declaration-contract`, `br-util-nats-fabric`), each `{crate}/v0.1.0`.
- `br-test-harness` is pinned by its own tag, `{ git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.0", version = "0.1.0", default-features = false, features = ["nats", "spawned-nats", "nats-fabric"] }`, the same git url and tag string a consumer uses, so a consumer that also depends on `br-test-harness` resolves a single copy of its types.

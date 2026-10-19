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

- `ConformanceReport::is_conformant` is true only for a report with at least one outcome, no failed and no skipped outcome; an empty report, a skipped check or a failed check no longer gets a conformant verdict (same rule as `conformance-directory` 0.1.1). `run_spawn` and `run_attach` return `ConformanceError::InvalidInput` when the scenario slice is empty, so a caller that selects nothing no longer gets a green result.
- A rejected confirmation is judged on the service it names as well as on its reason, like an accepted one: a subject that rejects a declaration but names another service now fails the scenario. The report text shows the service of the captured confirmation.
- A missing prerequisite is a typed `ConformanceError`, not a panic: `nats-server` not on `PATH`, the `fabric-nats` CLI not built beside the test executable (also from `provision`), an acceptor binary that is not a file, a throwaway NATS that does not start, an unreachable NATS or absent fixed streams in `run_attach`, a confirmation consumer that cannot bind, an acceptor that cannot be spawned, and an `identity-acceptor` binary found nowhere (`ConformanceError::SubjectBinaryUnavailable`). A harness panic while a scenario observes the subject is contained and reported as a failed check.
- `run_attach` no longer creates the fixed streams `INTEGRATION_CMD` and `INTEGRATION_EVT` on the broker it attaches to (it used to get-or-create them with a default configuration, hiding a missing deployment stream): it attaches without provisioning, checks that both streams are present and returns `ConformanceError::Jetstream` naming them when they are not. Pinned by `tests/attach.rs` (bare broker stays empty; provisioned broker with a live acceptor is conformant, 7 passed).
- `expected_verdict` returns `ConformanceError::InvalidInput` on an empty sequence instead of panicking.
- In a failing multi-step scenario the `expected` text is the oracle verdict of the failing step, not of the last step; on a pass the `observed` text is rendered from the captured confirmation, not rebuilt from the oracle.
- A confirmation on the accepted or rejected subject that is not a JSON envelope with a readable correlation id is reported as a failed check naming the wire-shape violation (the capture of `br-test-harness/v0.1.2` records the cause instead of dropping the frame, and the harness panic is contained), no longer as "no confirmation arrived". An envelope whose payload does not deserialize was already a failed check naming the decode error. Both are pinned by `tests/malformed_confirmation.rs`.
- `ConfirmationCapture::verdict_for` and `confirmation_decoded_for` (return types carrying `Result`) no longer panic when the capture of `br-test-harness/v0.1.2` recorded a malformed frame or when the NATS client is gone: they return `Some(Err(ConformanceError::Jetstream(..)))` naming the cause. The judged checks poll `confirmation_decoded_for`, so a failed outcome names the cause without relying on containment.
- A judged check whose confirmation capture cannot be read (drain stopped, stream ended, broker gone, frame without a readable envelope) no longer reports "the confirmation did not deserialize into the real IntegrationEvent type": its observed text states that the capture failed before a confirmation could be read, and only a real `NonConformantConfirmation` keeps the deserialization text; the cause stays in the detail.
- `run_spawn` shuts the throwaway NATS and the capture down on every early failure.
- The tests never compile anything. The `identity-acceptor` binary is looked up on `PATH`, then in the cache directory `$XDG_CACHE_HOME/br-e2e-harness/conformance-identity-{version}`, then downloaded from this crate's own release (`conformance-identity/v{version}`, asset `identity-acceptor-<os>-<arch>.tar.gz` with its `.sha256`; `linux` and `darwin`, `amd64` and `arm64`), failing closed on a SHA-256 mismatch, a redirect to a host other than `github.com` / `objects.githubusercontent.com` / `release-assets.githubusercontent.com`, or an archive over 64 MiB. A binary found nowhere is `ConformanceError::SubjectBinaryUnavailable`, naming the binary and what to do. `IdentityHarness::start` uses this lookup instead of building the subject.
- The lookup, the download and the crate's own tests no longer write to the system temporary directory (the throwaway NATS store of `br-test-harness` stays that crate's own): the former build output is gone, and the scratch directories of the tests live under the test target directory. The tests that changed `PATH` through `std::env::set_var` run the check in a child process instead, with no `unsafe`.

### Removed

- The public items of the `build` module, removed with the module: `build_subject`, `ensure_go_available`, `subject_dir` and `run_dead_grammar_guard`, and the root re-exports of the first three. The crate no longer runs the Go toolchain or `make`, and no Go-toolchain check remains in it; the dead-grammar guard, the vet and the Go tests of the subject run in CI.
- `ConformanceError::GoUnavailable` and `ConformanceError::Build`.

### Added

- `locate_subject()` and `SUBJECT_BINARY` (the module `binary`) and `ConformanceError::SubjectBinaryUnavailable { binary, detail }`: the lookup above.
- `DecodedConfirmation`, `CapturedConfirmation::decode_confirmation`, `ConfirmationCapture::confirmation_decoded_for`, `expected_step_confirmations`, `oracle::confirmation_code`, `oracle::outcome_to_confirmation`: the verdict together with the service the confirmation names.

- `ConfirmationCapture::try_confirmation_for` and `ConfirmationCapture::try_count`: the non-panicking forms of `confirmation_for` and `count`.

### Changed

- `Subject::spawn` hands the acceptor a `PATH` led by the directory of its binary, through the child's environment.
- Pins `br-test-harness/v0.1.2`; the `br-rust-common` pins stay on their latest tags (`0.1.0`).
- A consumer that moves to 0.1.1 pins `br-test-harness/v0.1.2`, `br-util-nats-fabric/v0.1.0`, `br-core-integration/v0.1.0`, `br-core-scope/v0.1.0` and `br-identity-domain/v0.1.0` with the same git url and tag string as this crate: its public signatures name their types (`FabricTestNats`, `CapturedMessage`, `Fabric`, `CoordError`, `DeclareServiceScopes`, `DeclarationOutcome`), and two different tags give two copies and a type error. The README Install block lists the pins.
- The four crate tests that re-run the test binary on one filtered test (`tests/absent_subject_binary.rs`, `tests/absent_nats_server.rs`, `tests/absent_fabric_nats.rs`, `release_asset::tests::a_download_ignores_the_system_proxy`) now assert that the child ran exactly that test (`1 passed`), so a renamed or moved test fails the parent instead of running nothing.
- New dependencies `futures-util` (default features off, `std`) to contain harness panics and `sha2` to verify the release asset.

## 0.1.0

### Changed

- First registry release of `conformance-identity` as its own library, tagged `conformance-identity/v0.1.0`. The crate is the black-box conformance runner for the scope-acceptance side of the scope-declaration handshake (battery G2): it drives an Identity registry against the frozen Go declaring subject. It is a test fixture only. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line, `0.1`. The earlier `v1.x` workspace tags are the legacy line and are untouched.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-identity", tag = "conformance-identity/v0.1.0", version = "0.1.0" }`.
- The `br-rust-common` dependencies are pinned to the per-crate `0.1.0` tags (`br-core-integration`, `br-core-scope`, `br-identity-domain`, `br-scope-declaration-contract`, `br-util-nats-fabric`), each `{crate}/v0.1.0`.
- `br-test-harness` is pinned by its own tag, `{ git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.0", version = "0.1.0", default-features = false, features = ["nats", "spawned-nats", "nats-fabric"] }`, the same git url and tag string a consumer uses, so a consumer that also depends on `br-test-harness` resolves a single copy of its types.

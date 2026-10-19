# Changelog

All notable changes to `conformance-scope` are documented here. The git tag
`conformance-scope/v{version}` releases the crate; this crate is versioned
separately from the other crates of the repository. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Fixed

- `ConformanceReport::is_conformant` is true only for a report with at least one outcome, no failed and no skipped outcome; an empty report, a report of skipped checks or a failed check no longer gets a conformant verdict. `run_spawn` and `run_attach` return `ConformanceError::InvalidInput` for an empty scenario slice, so a caller that selects nothing no longer gets a green result. `conformance-scope-cli` takes its aggregate verdict from the same rule: no service report, or one non-conformant service report, is not conformant.
- S5 (duplicate confirmations) no longer passes at the first `200`: after the duplicate `accepted` it observes `/readyz` for `QUIET_WINDOW` (3 seconds) and fails as soon as the subject answers anything but `200` or stops answering. Pinned by `tests/duplicate_confirmations.rs` with a subject stand-in that drops to `503` one second after its first `200`, and a control stand-in that stays ready.
- A declare on the declare subject that is not an envelope with `metadata.correlation_id` is a failed check naming that cause, no longer a declare reported as absent, a harness panic, or a malformed first declare hidden behind a later valid re-publish. With `br-test-harness/v0.1.2` the capture records the cause and stops; the checks read the capture through the new `DeclareCapture::try_count`, `try_first` and `try_correlation_ids` and report a failed outcome. Pinned by `tests/malformed_declare.rs` (S1, `declaration-content`, S3, S6).
- `run_spawn` returns `ConformanceError::InvalidInput` for an expectation whose scopes carry different `platform_only` flags, before anything starts: spawn mode drives the subject with one flag for every scope, so such an expectation could only produce a `declaration-content` failure caused by the harness, not by the subject. Pinned by `tests/refused_input.rs`.
- `run_spawn` and `run_attach` shut the throwaway NATS and the capture down on every early failure. `ScopeHarness::start_with_binary` returns `InvalidInput` for a binary that is not a file. The README names `AttachTarget { nats_url, readyz_url }` as it is (it listed a `stream_name` field that does not exist).
- The tests never compile anything. The `scope-service` binary is looked up on `PATH`, then in the cache directory `$XDG_CACHE_HOME/br-e2e-harness/conformance-scope-{version}`, then downloaded from this crate's own release (`conformance-scope/v{version}`, asset `scope-service-<os>-<arch>.tar.gz` with its `.sha256`; `linux` and `darwin`, `amd64` and `arm64`), failing closed on a SHA-256 mismatch, a redirect to a host other than `github.com` / `objects.githubusercontent.com` / `release-assets.githubusercontent.com`, or an archive over 64 MiB. A binary found nowhere is `ConformanceError::SubjectBinaryUnavailable`, naming the binary and what to do. `ScopeHarness::start` uses this lookup instead of building the subject. The `release-cli` workflow now attaches the four `scope-service` assets to the `conformance-scope/v{version}` release, and the `infra-e2e` job builds the subject outside the tests and puts it on `PATH`.
- The lookup, the download and the crate's own tests no longer write to the system temporary directory (the throwaway NATS store of `br-test-harness` stays that crate's own): the former build output is gone, and the scratch directories of the tests live under the test target directory.

### Removed

- The public items of the `build` module, removed with the module: `build_subject`, `ensure_go_available`, `subject_dir` and `run_dead_grammar_guard`, and the root re-exports of the first three. The crate no longer runs the Go toolchain or `make`, and no Go-toolchain check remains in it; the dead-grammar guard, the vet and the Go tests of the subject run in CI.
- `ConformanceError::GoUnavailable` and `ConformanceError::Build`.

### Added

- `locate_subject()` and `SUBJECT_BINARY` (the module `binary`) and `ConformanceError::SubjectBinaryUnavailable { binary, detail }`: the lookup above.
- `DeclareCapture::try_count`, `DeclareCapture::try_first` and `DeclareCapture::try_correlation_ids`: the non-panicking forms of `count`, `first` and `correlation_ids`.

### Changed

- `Subject::spawn` hands the subject a `PATH` led by the directory of its binary, through the child's environment.
- Pins `br-test-harness/v0.1.2`; the `br-rust-common` pins stay on their latest tags (`0.1.0`).
- New dependencies `futures-util` (default features off, `std`) to contain harness panics and `sha2` to verify the release asset.
- `conformance-scope-cli` moves with this crate to 0.1.1.

## 0.1.0

### Changed

- First registry release of `conformance-scope` as its own library, tagged `conformance-scope/v0.1.0`. The crate is the black-box conformance runner for the scope-declaration wire handshake (batteries S1-S6), reusable against any service binary through `start_with_binary`. It is a test fixture only. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line, `0.1`. The earlier `v1.x` workspace tags are the legacy line and are untouched.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-scope", tag = "conformance-scope/v0.1.0", version = "0.1.0" }`.
- The `br-rust-common` dependencies are pinned to the per-crate `0.1.0` tags (`br-core-integration`, `br-core-scope`, `br-scope-declaration-contract`, `br-util-nats-fabric`), each `{crate}/v0.1.0`.
- `br-test-harness` is pinned by its own tag, `{ git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.0", version = "0.1.0", default-features = false, features = ["nats-fabric"] }`, the same git url and tag string a consumer uses, so a consumer that also depends on `br-test-harness` resolves a single copy of its types.

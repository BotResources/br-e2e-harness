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
- `run_attach` binds to the handshake streams without creating them: it used `FabricTestNats::connect`, which creates `INTEGRATION_CMD` and `INTEGRATION_EVT` when they are missing, against the documented rule that the attached service owns them. A missing stream (either of the two) or an unreachable broker is now `ConformanceError::Jetstream` naming both fixed streams (for a missing stream) or the broker URL (for an unreachable broker) (the harness panic is contained, its text can still appear on stderr), which the CLI reports with exit code `2`; `ScopeHarness::start_with_binary` likewise returns `Jetstream` instead of panicking when `nats-server` is not on `PATH`. Pinned by `tests/refused_input.rs` (a broker with only `INTEGRATION_CMD` is refused and `INTEGRATION_EVT` is not created) and `tests/absent_nats_server.rs`.
- `ReadyzProbe::new` (and so `run_attach`, whose `readyz_url` it parses before any broker is contacted) returns `ConformanceError::InvalidInput` naming the URL when it is not an `http` or `https` URL with a host; a malformed `--readyz` used to be probed as unreachable and blamed on the service as a failed S2. Pinned by `tests/refused_input.rs`.
- `run_spawn` and `run_attach` shut the throwaway NATS and the capture down on every early failure. `ScopeHarness::start_with_binary` returns `InvalidInput` for a binary that is not a file. The README names `AttachTarget { nats_url, readyz_url }` as it is (it listed a `stream_name` field that does not exist).
- The tests never compile anything. The `scope-service` binary is looked up on `PATH`, then in the cache directory `$XDG_CACHE_HOME/br-e2e-harness/conformance-scope-{version}`, then downloaded from this crate's own release (`conformance-scope/v{version}`, asset `scope-service-<os>-<arch>.tar.gz` with its `.sha256`; `linux` and `darwin`, `amd64` and `arm64`), failing closed on a SHA-256 mismatch, a redirect to a host other than `github.com` / `objects.githubusercontent.com` / `release-assets.githubusercontent.com`, or an archive over 64 MiB. A binary found nowhere is `ConformanceError::SubjectBinaryUnavailable`, naming the binary and what to do. `ScopeHarness::start` uses this lookup instead of building the subject. The `release-cli` workflow now attaches the four `scope-service` assets to the `conformance-scope/v{version}` release, and the `infra-e2e` job builds the subject outside the tests and puts it on `PATH`.
- The lookup, the download and the crate's own tests no longer write to the system temporary directory (the throwaway NATS store of `br-test-harness` stays that crate's own): the former build output is gone, and the scratch directories of the tests live under the test target directory.

- Spawn mode panicked (exit 101 in the CLI) when the `fabric-nats` binary was not next to the executable, as with the released CLI archive, which holds only `conformance-scope`. `provision` and `run_spawn` now return `ConformanceError::FabricProvisionUnavailable`, naming `fabric-nats` or the missing manifest and what to do, and the CLI exits `2`. Pinned by `tests/absent_fabric_nats.rs`, which re-runs the test executable copied into an empty directory.
- A failed check reported the wanted state as the observation (for example `observed: /readyz stayed 200` for a run that dropped to `503`, or `200 despite a duplicate acceptance` when `/readyz` never reached `200`). `observed` now holds what was seen (`/readyz answered 503 during the quiet window`, `/readyz did not answer after acceptance`, `3 declares captured, 2 at the rejection`) for S2 to S6, and `expected` keeps the wanted state, so the human, JSON and JUnit output of the CLI are honest. Pinned by `tests/duplicate_confirmations.rs` and `tests/readyz_observations.rs`.
- The README no longer says attach mode connects through `FabricTestNats::connect`: it uses `attach_without_provisioning` and never creates a stream.

- A `--platform-only` style per-scope entry that names no expected scope was dropped silently and a duplicate key kept the first value, so a typo (`my_service:admn=true`) read as a pass. `ExpectedDeclaration::try_from_parts` returns `InvalidInput` naming the stray or duplicated key. Pinned by `tests/refused_input.rs`.
- A service key or scope keys that no `ScopeDeclaration` can hold (invalid `ScopeKey`, not owned by the expected service, listed twice) were judged as a `declaration-content` failure of the service. `run_attach` and `run_spawn` now return `InvalidInput` naming the key before any I/O (spawn: before the binary is looked at). Pinned by `tests/refused_input.rs`; its `mixed()` fixture now uses scopes owned by its service.

- `Subject::start` returns `Result<Subject>`: after the fifth attempt in which the subject exits before its `/livez` answers it returns the new `ConformanceError::SubjectExitedAtBoot { binary, attempts, status, log_tail }` instead of the dead subject, so `run_spawn` fails with a setup error (exit `2` in the CLI) and no longer reports failed S1 or declaration-content checks against a process that never ran.
- Attach mode: the README and the `--scenarios` help said s3, s4 and s6 "cannot run against an already-booted service", yet `run_attach` runs any selected scenario. The documentation now states the real prerequisites: s3 holds for a service that is still unconfirmed and re-publishes within `--timeout`, s6 for a service booted with declaration disabled, s4 needs `--reject`; a failed s3 or s6 in `run_attach` names its prerequisite in its `detail` (`Scenario::attach_prerequisite`). Pinned by `tests/attach_lifecycle.rs`.
- The tests and `run_spawn` picked the subject's port by binding and releasing a free port, then the subject bound it itself; another process could take it in between ("address already in use", a 10-second timeout that read as a failed S3/S4). They now start the subject with `Subject::start`, which starts it again on a new port, up to five attempts, when the subject process exits before its `/livez` answers (`br-test-harness` `SpawnedProcess::await_boot`, whatever language the subject is written in and whatever it logs). After a `/livez` answer it waits 250 ms and checks again that the process is alive, because a subject that lost its port can be answered by the process that holds it; a subject whose `/livez` does not answer within one second is taken as started (best effort: a subject that loses its port later than that is not restarted). Pinned by `tests/port_race.rs` (the Go text, the capitalised operating-system text, an exit that says nothing, a silent subject that keeps running) and, through the CLI, by `conformance-scope-cli/tests/port_race.rs`.

### Removed

- The public items of the `build` module, removed with the module: `build_subject`, `subject_dir`, `run_dead_grammar_guard` and the Go-toolchain availability function, and the root re-exports of the first, second and fourth. The crate no longer runs the Go toolchain or `make`, and no Go-toolchain check remains in it; the dead-grammar guard, the vet and the Go tests of the subject run in CI.
- `ConformanceError::GoUnavailable` and `ConformanceError::Build`.

### Added

- `Subject::start`: `Subject::spawn` with a new port when the subject process exited before its `/livez` answered, up to five attempts (`spawn` is unchanged).
- `ConformanceError::SubjectExitedAtBoot { binary, attempts, status, log_tail }` and `Scenario::attach_prerequisite`.
- `ExpectedDeclaration::try_from_parts` (fallible form of `from_parts`, which is unchanged) and `ExpectedDeclaration::validate`.
- `ConformanceError::FabricProvisionUnavailable { detail }`: the typed error of a spawn-mode prerequisite (`fabric-nats` binary, provisioning manifest) that is missing.
- `locate_subject()` and `SUBJECT_BINARY` (the module `binary`) and `ConformanceError::SubjectBinaryUnavailable { binary, detail }`: the lookup above.
- `DeclareCapture::try_count`, `DeclareCapture::try_first` and `DeclareCapture::try_correlation_ids`: the non-panicking forms of `count`, `first` and `correlation_ids`.
- A "Run in CI" section in the README: a service downloads the released `conformance-scope` binary at a pinned version, verifies its SHA-256, boots its own binary against a `nats-server` carrying the two fixed streams and runs the battery in attach mode, the exit code failing the job.

### Changed

- `Subject::spawn` hands the subject a `PATH` led by the directory of its binary, through the child's environment.
- Pins `br-test-harness/v0.1.2` with its `http-client` feature (for `SpawnedProcess::await_boot`); the `br-rust-common` pins stay on their latest tags (`0.1.0`).
- New dependencies `futures-util` (default features off, `std`) to contain harness panics and `sha2` to verify the release asset.
- `conformance-scope-cli` moves with this crate to 0.1.1.

## 0.1.0

### Changed

- First registry release of `conformance-scope` as its own library, tagged `conformance-scope/v0.1.0`. The crate is the black-box conformance runner for the scope-declaration wire handshake (batteries S1-S6), reusable against any service binary through `start_with_binary`. It is a test fixture only. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line, `0.1`. The earlier `v1.x` workspace tags are the legacy line and are untouched.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-scope", tag = "conformance-scope/v0.1.0", version = "0.1.0" }`.
- The `br-rust-common` dependencies are pinned to the per-crate `0.1.0` tags (`br-core-integration`, `br-core-scope`, `br-scope-declaration-contract`, `br-util-nats-fabric`), each `{crate}/v0.1.0`.
- `br-test-harness` is pinned by its own tag, `{ git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.0", version = "0.1.0", default-features = false, features = ["nats-fabric"] }`, the same git url and tag string a consumer uses, so a consumer that also depends on `br-test-harness` resolves a single copy of its types.

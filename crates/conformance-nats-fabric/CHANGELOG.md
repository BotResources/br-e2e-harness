# Changelog

All notable changes to `conformance-nats-fabric` are documented here. The git tag
`conformance-nats-fabric/v{version}` releases the crate; this crate is versioned
separately from the other crates of the repository. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Fixed

- The per-check timeout bounds the check body only: for the two checks that need a server without the fixed streams, the throwaway server start is no longer inside the deadline, so a short `--timeout` no longer fails them on the server start time.
- A check selected more than once is refused with `ConformanceError::DuplicateCheck` by `parse_checks`, `run_spawn` and `run_attach`; before, it ran and was counted twice in the report.
- `assert_no_fixed_stream_captured` and `assert_dead_grammar_fails_loud` first require `INTEGRATION_CMD` and `INTEGRATION_EVT` to exist and panic otherwise. Before, the capture check read a missing fixed stream as "no message" and the dead-grammar check passed on a server with no fixed stream, because the publish then fails with `NoStream` for every subject. A broker that cannot answer is a panic, never "no message" (the `raw_message_absent` probe of `br-test-harness` 0.1.2 no longer reads a lookup error as "no message").
- `assert_bootstrap_then_watch_is_parallel_safe` proves the watch is live before it starts the bootstrap: it re-puts a canary key until the watch projects it (the watch delivers only entries put after it subscribes), so the bootstrap runs with the watch subscribed. The check panics, with the error, when the watch ended before the check stopped it; it no longer prints the error and stays green.
- `assert_prefix_watch_delivery_gap` proves the watch is live with the same canary instead of a 300 ms sleep, and panics when the watch ended instead of discarding its result.
- `assert_reconcile_drift_converges` and `assert_retract_orphan_deletes` compare the whole `PublishedUser` read back (email, first name, last name and extensions) with the desired one, and also check that an untouched value survives the reconcile. Before, only the email of the repaired and added values was compared.
- The `prefix_watch_delivers_slash_keyed_directory_puts` failure message and the README no longer cite a `br-util-nats-fabric` version that is not on the release line.

### Added

- `ConformanceError::AnchorBinaryUnavailable { binary, detail }` and `ConformanceError::Run`: the typed error for an absent anchor binary, naming it and what to do, and for a failing anchor run.
- `frozen_wire()` (unchanged signature) now finds the prebuilt `nats-fabric` binary on `PATH`, then in the cache directory, then by a verified download of the `nats-fabric-<os>-<arch>.tar.gz` asset (with its `.sha256`) of this crate's own GitHub release `conformance-nats-fabric/v{version}`. The download fails closed on a SHA-256 mismatch, a redirect to a host outside `github.com` / `objects.githubusercontent.com` / `release-assets.githubusercontent.com`, a non-https URL, or a size over 64 MiB, and honours the system proxy variables. A bare name is resolved only on the absolute entries of `PATH`, the `tar` that extracts a download likewise, and the cache root ignores a relative `XDG_CACHE_HOME` and a relative `HOME`, so no lookup depends on the current directory. The directory of the binary leads the child `PATH`, set on the child command only; the crate never calls `std::env::set_var`.
- The `release-anchor-nats-fabric` workflow builds and attaches the four per-platform `nats-fabric` assets to the `conformance-nats-fabric/v{version}` release. The `infra-e2e` job runs the anchor's source guard and Go tests, builds the binary onto `PATH` and runs the battery with `--all-targets`.

- A runner a wrapper can drive without Rust: `run_spawn(checks, timeout)` (throwaway `nats-server`, every check) and `run_attach(&AttachTarget, checks, timeout)` (an existing server) return a `ConformanceReport` of `CheckOutcome`s (`CheckId`, `Pass` / `Fail` / `Skipped`, expected, observed). A check that panics is reported as `Fail` with the panic message, and a check that outlives its timeout as `Fail` and is aborted, body included, together with the watch task a check started (a task spawned inside a check stops when the check is aborted, panics or returns). `CheckId` carries the 12 checks (the 12 tests of `tests/conformance.rs`) with a stable kebab-case code, `parse_checks` reads a CSV of codes, `spawn_default()` / `attach_default()` are the default sets.
- Attach mode never provisions: it binds with `attach_without_provisioning`, fails a check loud when the fixed streams or the `PUBLISHED_LANGUAGE` bucket are absent on the target, retracts every published-language key and deletes the durable it wrote (also when the check body outlives its timeout: the deadline bounds the body only, the cleanup runs afterwards under its own 30 s bound; a killed process is not cleaned), and leaves the two checks that need a server without the fixed streams (`missing-stream-fails-coverage-probe`, `missing-stream-fails-durable-bind`) out of `attach_default()` (a default attach run covers 10 of the 12 checks); selected explicitly, they are reported as `Skipped`. Spawn mode provisions the bucket in-process, so no `fabric-nats` binary has to sit beside the caller.
- A missing anchor binary stops the run with `ConformanceError::AnchorBinaryUnavailable` before any check runs; `ConformanceError::BrokerUnavailable` (no `nats-server` on `PATH` in spawn mode, an unanswering server in attach mode) and `ConformanceError::UnknownCheck` and `ConformanceError::DuplicateCheck` are the other new typed errors. An empty selection, or an attach run whose selection holds only spawn-only checks, is `ConformanceError::NoCheckToRun`: a run that checked nothing is never a green report. A server that does not answer the attach probe within the timeout is `BrokerUnavailable`. `redact_nats_url` removes the userinfo of a NATS URL (list) up to the last `@` of each entry, so a `/`, `?`, `#` or `,` in a password cannot end it early, and the runner applies it, plus a scrub of the userinfo and password literals, to every message it reports, so credentials in `--nats` never reach a report.
- `checks::integration::DEAD_COMMAND_SUBJECT`, `assert_anchor_subjects_match(&FrozenWire)` and `checks::published_language::assert_anchor_users_deserialize(&FrozenWire)`: the anchor comparisons of `tests/conformance.rs` as public checks, shared by the tests and the runner. They also fail on an anchor that renders no subject or no user.
- `tests/runner.rs`: the runner on real infra (every check in spawn mode; attach mode on a provisioned server, leaving no key behind; attach mode on a server without the fixed streams, creating nothing; an unreachable server; a timeout).
- The CI gate `.github/scripts/go-free-gate.sh` (shared with `conformance-passport`, listed in `GO_FREE_CRATES`) scans the `*.rs`, `*.toml`, `*.sh` and `*.yml` files of this crate and of `conformance-nats-fabric-cli` (not markdown) and fails the build on the former Go-toolchain probe helper, a `"go"` or `"make"` program literal (also ending a path, also inside `run_once(...)` or `Command::new(...)`), a `go` or `make` command line with `build`, `run`, `install`, `test`, `vet`, `generate` or `-C`, and `temp_dir`, `tempdir` or `tempfile`; it also fails when `grep` cannot scan a file. A program chosen by a variable (`Command::new(var)`) is not detected. `test-go-free-gate.sh` proves each refusal.

### Removed

- The crate no longer compiles anything. Removed public items: `build_anchor()`, `ensure_go_available()`, `anchor_dir()`, `run_dead_grammar_guard()`, and the variants `ConformanceError::GoUnavailable` and `ConformanceError::Build`. `frozen_wire()` stays and now locates the prebuilt binary. The dead-grammar source guard (`make guard`) moved from the test path to the CI step that builds the anchor.
- The crate no longer writes to the system temporary directory (the throwaway NATS store of `br-test-harness` stays that crate's own).

### Changed

- `br-test-harness` is pinned to `br-test-harness/v0.1.2`; the `br-rust-common` pins stay at their latest tags (`br-core-directory/v0.1.0`, `br-core-integration/v0.1.0`, `br-util-nats-fabric/v0.1.0`).
- A consumer that moves to 0.1.1 pins `br-test-harness/v0.1.2`, `br-util-nats-fabric/v0.1.0` and `br-core-directory/v0.1.0` with the same git url and tag string as this crate: its public signatures name their types (`FabricTestNats`, `BareFabricNats`, `WidenedDurable`, `KvKey`, `KvPrefix`, `FabricError`, `PublishErrorKind`, `ProjectionSink`, `PublishedUser`), and two different tags give two copies and a type error.
- The three crate tests that re-run the test binary on one filtered test now assert that the child ran exactly that test (`1 passed`), so a renamed or moved test fails the parent instead of running nothing.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-nats-fabric", tag = "conformance-nats-fabric/v0.1.1", version = "0.1.1" }`.

## 0.1.0

### Changed

- First registry release of `conformance-nats-fabric` as its own library, tagged `conformance-nats-fabric/v0.1.0`. The crate is the black-box conformance runner for the v1 `integration.*` subject grammar and the `PUBLISHED_LANGUAGE` KV, anchored against an independent Go renderer. It is a test fixture only. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line, `0.1`. The earlier `v1.x` workspace tags are the legacy line and are untouched.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-nats-fabric", tag = "conformance-nats-fabric/v0.1.0", version = "0.1.0" }`.
- The `br-rust-common` dependencies are pinned to the per-crate `0.1.0` tags (`br-core-directory`, `br-core-integration`, `br-util-nats-fabric`), each `{crate}/v0.1.0`.
- `br-test-harness` is pinned by its own tag, `{ git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.0", version = "0.1.0", default-features = false, features = ["nats-fabric"] }`, the same git url and tag string a consumer uses, so a consumer that also depends on `br-test-harness` resolves a single copy of its types.

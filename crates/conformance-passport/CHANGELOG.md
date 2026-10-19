# Changelog

All notable changes to `conformance-passport` are documented here. The git tag
`conformance-passport/v{version}` releases the crate; this crate is versioned
separately from the other crates of the repository. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Fixed

- The Go reference subject listened on `0.0.0.0` and answered `/internal/passport` without authentication on every interface of the test machine. It binds `127.0.0.1` only (`listenAddr` in `conformance-subjects/identity-passport`), pinned by a Go test and by the `the_subject_does_not_answer_on_a_non_loopback_address` real-infra test, which fails on the old binary.
- P8 passed when the call after the bucket loss failed (refused connection, crash, or no answer): it proved no answer, not a 5xx. P8 now passes only on a 5xx received within 10 seconds; any transport error fails the check, and the observed text carries the full error chain (`operation timed out`, `Connection refused`), so a hang and a crash read differently; this applies to the failure text of every check that calls the endpoint. The README contract and the P8 row say so.
- `run_spawn` returns `ConformanceError::InvalidInput` when a scenario is listed twice, before touching any infra; a repeated `p8` seeded into the bucket the first one deleted and panicked in the harness, losing the whole report.
- `PassportHarness::start_with_binary` and `run_spawn` return `ConformanceError::BinaryNotExecutable { path }` for a caller-supplied path that is not an executable file (the message names that path: build it and mark it executable), instead of a panic when the child is spawned.
- The tests never compile anything. The `identity-passport` binary is looked up on `PATH`, then in the cache directory `$XDG_CACHE_HOME/br-e2e-harness/conformance-passport-{version}`, then downloaded from this crate's own release (`conformance-passport/v{version}`, asset `identity-passport-<os>-<arch>.tar.gz` with its `.sha256`; `linux` and `darwin`, `amd64` and `arm64`), failing closed on a SHA-256 mismatch, a redirect to a host other than `github.com` / `objects.githubusercontent.com` / `release-assets.githubusercontent.com`, or an archive over 64 MiB. A binary found nowhere is `ConformanceError::SubjectBinaryUnavailable`, naming the binary and what to do. `PassportHarness::start` uses this lookup instead of building the subject.
- The crate writes nothing to the system temporary directory: the former build output is gone, and the scratch directories of the tests live under the test target directory. The one exception is outside the crate: every `PassportHarness::start*` starts a `br-test-harness` NATS server whose store `br-test-harness` creates in the system temporary directory.
- `PassportHarness::start_with_binary` and `run_spawn` resolve a bare program name (`identity-passport`) on `PATH`, as spawning it directly did in 0.1.0, and hand the absolute result to the child. A bare name found nowhere is `ConformanceError::BinaryNotOnPath { name }`, which names the caller's binary and not `identity-passport`; only a path with a directory part can be `BinaryNotExecutable`. The bare-name test is `#[ignore]` (real-infra: it starts `nats-server`) and its child run asserts it really ran.

- `run_spawn` returns `ConformanceError::InvalidInput` when `timeout` cannot be added to the current instant (for example `Duration::MAX`), before touching any infra; it used to start the broker and the subject, then panic in the harness wait.
- `run_spawn` reports a subject that never answers `/readyz` as `timed out waiting for the passport subject to report /readyz=200; logs: ...`; the message used to read `timed out waiting for the passport subject never reported /readyz=200`.

### Removed

- The public items of the `build` module, removed with the module: `build_subject`, `ensure_go_available`, `subject_dir` and `run_retired_model_guard`. The crate no longer runs the Go toolchain or `make`, and no Go-toolchain check remains in it; the retired-model guard, the vet and the Go tests of the subject run in CI.
- `ConformanceError::GoUnavailable` and `ConformanceError::Build`.

### Added

- The companion CLI `conformance-passport-cli` (binary `conformance-passport`) runs the battery against a service binary in any language and ships as release binaries of this library's tag; the README has a "Run in CI" section for it.
- `ConformanceError::BinaryNotOnPath { name }`: a caller-supplied bare program name that is not on `PATH`.
- `locate_subject()` and `SUBJECT_BINARY` (the module `binary`) and `ConformanceError::SubjectBinaryUnavailable { binary, detail }`: the lookup above, `SubjectBinaryUnavailable` is returned only by `locate_subject()` (`PATH`, cache, download).
- `ConformanceError::BinaryNotExecutable { path }`: a caller-supplied binary path that is not an executable file.

### Changed

- `Subject::spawn` hands the subject a `PATH` led by the directory of its binary, through the child's environment, when that directory is absolute, is valid UTF-8 and contains no `:`; otherwise the child inherits `PATH` unchanged.
- Pins `br-test-harness/v0.1.2`; the `br-rust-common` pins stay on their latest tags (`0.1.0`).
- New dependency `sha2` to verify the release asset.

## 0.1.0

### Changed

- First registry release of `conformance-passport` as its own library, tagged `conformance-passport/v0.1.0`. The crate is the black-box conformance runner for the bearer/PAT to Passport resolution endpoint (battery G1, `GET /internal/passport`), seeded from the committed Go-frozen wire vectors. It is a test fixture only. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line, `0.1`. The earlier `v1.x` workspace tags are the legacy line and are untouched.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-passport", tag = "conformance-passport/v0.1.0", version = "0.1.0" }`.
- The `br-rust-common` dependencies are pinned to the per-crate `0.1.0` tags (`br-core-auth`, `br-util-nats-fabric`), each `{crate}/v0.1.0`.
- `br-test-harness` is pinned by its own tag, `{ git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.0", version = "0.1.0", default-features = false, features = ["nats-fabric"] }`, the same git url and tag string a consumer uses, so a consumer that also depends on `br-test-harness` resolves a single copy of its types.

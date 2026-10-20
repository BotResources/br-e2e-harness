# Changelog

All notable changes to `conformance-directory` are documented here. The git tag
`conformance-directory/v{version}` releases the crate; this crate is versioned
separately from the other crates of the repository. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Fixed

- `ConformanceReport::is_conformant` is true only for a report with at least one outcome, no failed and no skipped outcome; an empty report or a report of skipped checks proved nothing and no longer gets a conformant verdict.
- `AnchorSource::from_snapshot` refuses a snapshot entry whose key is not `identity/users/{uuid}` (or `identity/groups/{uuid}`) and two entries resolving to the same id, with `ConformanceError::NonConformantWire` naming the key; it no longer drops them and shrinks the publisher and consumer batteries without a signal. W1 and W2 fail on the same malformed or duplicated keys.
- A missing prerequisite is a typed `ConformanceError`, not a panic: `nats-server` not on `PATH`, the `fabric-nats` CLI not built beside the test executable, no `E2E_PG_ADMIN_URL` / `DATABASE_URL`, an unreachable Postgres, a failing throwaway-database or throwaway-NATS start, an absent `identity-directory` binary (`ConformanceError::AnchorBinaryUnavailable`) and a missing `PUBLISHED_LANGUAGE` bucket when `read_users`, `read_groups` or `read_meta` run (`ConformanceError::Jetstream`). A harness panic during the start of the throwaway NATS or database is contained and returned as an error, and so is the panic of `br-test-harness` when it cannot spawn the `sh` orphan guard of a spawned process (the `nats-server --version` probe of `DirectoryHarness::start` and the consumer batteries, `provision::provision`, `emit_snapshot` and `locate_and_emit`, the `tar` extraction of `locate_anchor`): `ConformanceError::Jetstream`, `ConformanceError::Run` or `ConformanceError::AnchorBinaryUnavailable` naming the guard instead of a panic, and a NATS already started is shut down when the provisioning that follows fails.
- `read_users` and `read_groups` return `ConformanceError::NonConformantWire` for a stray key under the `identity/users/` or `identity/groups/` prefix of the bucket instead of dropping it.
- A check reports the behaviour of the system under test as a failed `CheckOutcome`, never as a harness `Err`. C3 reports a verdict, not `ConformanceError::Postgres`, when the projector wrote no `known_users` row for the first user; the same holds for every `DirectoryProjector::reconcile` in C1-C5 and every `DirectoryPublisher` open and `reconcile` in P1-P2, and for a `NonConformantWire` read back from the bucket in P1-P2 (a failed outcome naming the key). Only a missing prerequisite or a fixture failure (seeding, migration, probes, Jetstream, Postgres) stays an `Err`. `CheckOutcome::system_failed(id, expected, stage, error)` builds that outcome.
- W1, W2, `AnchorSource::from_snapshot`, `read_users` and `read_groups` require the whole key to equal `user_kv_key(id)` / `group_kv_key(id)` (hyphenated lowercase uuid). An uppercase, 32-hex, braced or `urn:uuid:` spelling of an id is `ConformanceError::NonConformantWire` naming the key (W1/W2 fail); two spellings of one id no longer read back as a single user.
- W3 requires the `identity/_meta` entry key to equal `META_KEY`; a snapshot with another meta key fails W3 naming the key instead of passing `run_wire_battery`.
- `ConformanceError::NonConformantWire` no longer says the value is "Go-frozen": it is also raised for a read-back of the bucket.
- The crate's own Postgres admin URL lookup (the probe and the unreachable-Postgres path) reads `E2E_PG_ADMIN_URL`, then `DATABASE_URL`, from the environment and falls back to the nearest `.env` file by reading it only; it never writes the process environment. `br-test-harness` 0.1.2 still loads the nearest `.env` into the process (`dotenvy::dotenv()`) when a battery creates its throwaway database, so a battery that reaches that step is not read-only.
- `read_users`, `read_groups` and `read_meta` return `ConformanceError::Jetstream` naming `PUBLISHED_LANGUAGE` when the NATS broker is dead or unreachable; they no longer panic (after about 10 s) from the bucket probe, `pl_list`, `pl_get_meta` or `pl_reader` of `br-test-harness`. `DirectoryHarness::start` contains the same bucket probe after provisioning and shuts the throwaway NATS down on failure. The panic guard is `catch_unwind` on the calling task (`futures-util`, `std` feature only), so it can wrap code that borrows the NATS handle.
- `emit_snapshot` with an explicit path (a path with a directory part) returns `ConformanceError::AnchorBinaryUnavailable` naming `identity-directory` and what to do when the path is missing, a directory or not executable; it no longer returns `ConformanceError::Run("failed to spawn ...")`.
- C5 (`users_only_narrows_projection`) aborts its spawned `watch()` task on every exit path, including the error returns after the spawn (`PublishedUser::new`, the live publish, the row poll); the task no longer keeps running against a pool and a broker that the battery then closes.
- The crate's own tests remove their scratch directories through one drop guard, also when an assertion fails; a failing test no longer leaves `scratch-*` directories under the target directory. The anchor `vet` and Go golden tests now run in CI before the anchor is built.
- `provision` no longer reads its manifest from the source-tree path that `CARGO_MANIFEST_DIR` baked in at compile time, a path that does not exist on the machine of a released binary: the `published_language.toml` fixture is embedded in the crate and, when the source-tree file is absent, written once to `$XDG_CACHE_HOME/br-e2e-harness/conformance-directory-{version}/manifests/` (or the platform cache directory), replaced when its content differs, and handed to `fabric-nats`. No cache directory, or one that cannot be written, is a typed `ConformanceError::Jetstream`. `manifest_path` and the signature of `provision` are unchanged; a manifest name the crate does not embed still resolves to `tests/fixtures/`.
- The embedded-manifest fallback of `provision` is pinned by tests through `readable_manifest_from`: an absent source tree writes the embedded manifest to the cache directory and returns that path, an absent source tree without a cache directory is a typed `ConformanceError::Jetstream` naming the missing cache directory, and a present source tree wins.

### Added

- `battery` module for the `conformance-directory` CLI: `ALL_CHECKS` (W1..W5, W6, P1..P2, C1..C6 in battery order), `parse_checks` (a comma-separated selection of check codes; an unknown code or an empty selection is `ConformanceError::InvalidInput`) and `run_battery(&snapshot, &checks)` (runs the selected checks in battery order whatever the order of the selection; an empty selection is `InvalidInput`, a missing prerequisite stays an `Err`). Additive; no existing item changed.
- `CheckOutcome::system_failed(id, expected, stage, error)`.
- The public module `binary` with `locate_anchor()` and `ANCHOR_BINARY`, and `locate_and_emit()` in `anchor`: the tests assume the Go anchor is already built and find the `identity-directory` binary on `PATH`, then in the cache directory, then by a verified download of the `identity-directory-<os>-<arch>.tar.gz` asset (with its `.sha256`) of this crate's own GitHub release `conformance-directory/v{version}`. The download fails closed on a SHA-256 mismatch, a redirect to a host outside `github.com` / `objects.githubusercontent.com` / `release-assets.githubusercontent.com`, a non-https URL, or a size over 64 MiB, and honours the system proxy variables. `emit_snapshot(binary)` prepends the directory of the binary to the child `PATH` only when that directory is absolute: a bare name is resolved only on the absolute entries of `PATH` and an absent name is `AnchorBinaryUnavailable`, so a relative `PATH` entry such as `.` never makes the anchor in the current directory run; a path with a directory part must be an executable file and is run as given, otherwise it is `AnchorBinaryUnavailable`. The `tar` that extracts a download is resolved the same way. The cache root ignores a relative `XDG_CACHE_HOME` (falls back to `HOME`) and a relative `HOME` (no cache directory, so the typed `AnchorBinaryUnavailable` error), so the cache lookup, the download target and the exec of the anchor never depend on the current directory.
- `ConformanceError::AnchorBinaryUnavailable { binary, detail }`: the typed error for an absent anchor binary, naming it and what to do.
- `.github/scripts/go-free-gate.sh` covers this crate in CI (`GO_FREE_CRATES`): it fails the build on a Go or `make` build in the crate's code, tests, `*.toml`, `*.sh` or `*.yml`, and on a system temporary directory in its `*.rs`.
- The `release-anchor` workflow builds and attaches the four per-platform anchor assets to the `conformance-directory/v{version}` release.

### Removed

- The crate no longer compiles anything. Removed public items: `build_anchor()`, `build_and_emit()` (replaced by `locate_and_emit()`), `ensure_go_available()`, `subject_dir()`, the `build` module, and the variants `ConformanceError::GoUnavailable` and `ConformanceError::Build`.
- The anchor lookup, the download and the crate's own tests no longer write to the system temporary directory (the throwaway NATS store of `br-test-harness` stays that crate's own).

### Changed

- `br-test-harness` is pinned to `br-test-harness/v0.1.2` and `br-util-directory` to `br-util-directory/v0.1.1`; the other `br-rust-common` pins stay at their latest tags (`br-core-directory/v0.1.0`, `br-util-nats-fabric/v0.1.0`).
- Every re-exec parent test of the crate now reads the child output and fails unless the child passed and ran exactly one test, so a renamed or stale filter can no longer pass with `running 0 tests`.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-directory", tag = "conformance-directory/v0.1.1", version = "0.1.1" }`.

## 0.1.0

### Changed

- First registry release of `conformance-directory` as its own library, tagged `conformance-directory/v0.1.0`. The crate is the black-box conformance runner for the Identity published-language (directory) wire: the Px publisher and Cx consumer batteries on real NATS and Postgres, plus the offline wire-deserialization gate on `br-core-directory`. It is a test fixture only. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line, `0.1`. The earlier `v1.x` workspace tags are the legacy line and are untouched.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-directory", tag = "conformance-directory/v0.1.0", version = "0.1.0" }`.
- The `br-rust-common` dependencies are pinned to the per-crate `0.1.0` tags (`br-core-directory`, `br-util-directory`, `br-util-nats-fabric`), each `{crate}/v0.1.0`.
- `br-test-harness` is pinned by its own tag, `{ git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.0", version = "0.1.0", default-features = false, features = ["nats-fabric", "e2e-db"] }`, the same git url and tag string a consumer uses, so a consumer that also depends on `br-test-harness` resolves a single copy of its types.

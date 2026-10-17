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

- `ConformanceReport::is_conformant` is true only for a report with at least one passed check and no failed one; an empty report or a report of skipped checks proved nothing and no longer gets a conformant verdict.
- `AnchorSource::from_snapshot` refuses a snapshot entry whose key is not `users/{uuid}` (or `groups/{uuid}`) and two entries resolving to the same id, with `ConformanceError::NonConformantWire` naming the key; it no longer drops them and shrinks the publisher and consumer batteries without a signal. W1 and W2 fail on the same malformed or duplicated keys.
- A missing prerequisite is a typed `ConformanceError`, not a panic: `nats-server` not on `PATH`, the `fabric-nats` CLI not built beside the test executable, no `E2E_PG_ADMIN_URL` / `DATABASE_URL`, an unreachable Postgres, a failing throwaway-database or throwaway-NATS start, an absent `identity-directory` binary (`ConformanceError::AnchorBinaryUnavailable`) and a missing `PUBLISHED_LANGUAGE` bucket when `read_users`, `read_groups` or `read_meta` run (`ConformanceError::Jetstream`). A harness panic during the start of the throwaway NATS or database is contained and returned as an error, and a NATS already started is shut down when the provisioning that follows fails.
- `read_users` and `read_groups` return `ConformanceError::NonConformantWire` for a stray key under the `users/` or `groups/` prefix of the bucket instead of dropping it.
- C3 reports a verdict, not `ConformanceError::Postgres`, when the projector wrote no `known_users` row for the first user.

### Added

- `locate_anchor()`, `locate_and_emit()` and `ANCHOR_BINARY`: the tests assume the Go anchor is already built and find the `identity-directory` binary on `PATH`, then in the cache directory, then by a verified download of the `identity-directory-<os>-<arch>.tar.gz` asset (with its `.sha256`) of this crate's own GitHub release `conformance-directory/v{version}`. The download fails closed on a SHA-256 mismatch, a redirect to a host outside `github.com` / `objects.githubusercontent.com` / `release-assets.githubusercontent.com`, a non-https URL, or a size over 64 MiB.
- `ConformanceError::AnchorBinaryUnavailable { binary, detail }`: the typed error for an absent anchor binary, naming it and what to do.
- The `release-anchor` workflow builds and attaches the four per-platform anchor assets to the `conformance-directory/v{version}` release.

### Removed

- The crate no longer compiles anything. Removed public items: `build_anchor()`, `build_and_emit()` (replaced by `locate_and_emit()`), `ensure_go_available()`, `subject_dir()`, the `build` module, and the variants `ConformanceError::GoUnavailable` and `ConformanceError::Build`.
- No file is written to the system temporary directory any more.

### Changed

- `br-test-harness` is pinned to `br-test-harness/v0.1.2` and `br-util-directory` to `br-util-directory/v0.1.1`; the other `br-rust-common` pins stay at their latest tags (`br-core-directory/v0.1.0`, `br-util-nats-fabric/v0.1.0`).
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-directory", tag = "conformance-directory/v0.1.1", version = "0.1.1" }`.

## 0.1.0

### Changed

- First registry release of `conformance-directory` as its own library, tagged `conformance-directory/v0.1.0`. The crate is the black-box conformance runner for the Identity published-language (directory) wire: the Px publisher and Cx consumer batteries on real NATS and Postgres, plus the offline wire-deserialization gate on `br-core-directory`. It is a test fixture only. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line, `0.1`. The earlier `v1.x` workspace tags are the legacy line and are untouched.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-directory", tag = "conformance-directory/v0.1.0", version = "0.1.0" }`.
- The `br-rust-common` dependencies are pinned to the per-crate `0.1.0` tags (`br-core-directory`, `br-util-directory`, `br-util-nats-fabric`), each `{crate}/v0.1.0`.
- `br-test-harness` is pinned by its own tag, `{ git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.0", version = "0.1.0", default-features = false, features = ["nats-fabric", "e2e-db"] }`, the same git url and tag string a consumer uses, so a consumer that also depends on `br-test-harness` resolves a single copy of its types.

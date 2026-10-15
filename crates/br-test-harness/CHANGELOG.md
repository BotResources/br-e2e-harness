# Changelog

All notable changes to `br-test-harness` are documented here. The git tag
`br-test-harness/v{version}` releases the crate; this crate is versioned
separately from the other crates of the repository. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Fixed

- `verdict::is_ack` (and so `expect_ack`) is true only for a response carrying a non-empty `data` object; `{}`, `{"data": null}`, a JSON string (the non-JSON body of `GraphqlClient::post_raw` and `query_with_passport_header`) and an HTTP error page no longer ack. `expect_rejected` no longer says "got ack" when there is no rejection code.
- `SseSubscription` frames on CRLF, CR and LF line ends, and carries a CRLF split across two chunks and a multi-byte character split across two chunks. A server ending lines with CRLF was never decoded: `next_outcome` read `Timeout` while events arrived and `expect_silence` passed.
- `SpawnedNats` binds `nats-server` to `127.0.0.1` (`-a 127.0.0.1`); the unauthenticated JetStream server no longer listens on every interface of the machine.
- `CommandAwaiter::await_correlation` panics when its subscription has ended, like the event awaiter, instead of returning the `None` of a passed deadline; a "no command was sent" assertion no longer passes on a dead subscription.

## 0.1.0

### Changed

- First registry release of `br-test-harness` as its own library, tagged `br-test-harness/v0.1.0`. The crate is the real-infrastructure end-to-end test harness of the platform: real Postgres and NATS (spawned or external), a spawned service binary, forged Passports, GraphQL, SSE and WebSocket clients, the mutation verdict and the `fabric-nats` CLI, with `oidc-test-idp` re-exported in-process behind the `oidc` feature. It is a test fixture only. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line, `0.1`. The earlier `v1.x` workspace tags are the legacy line and are untouched.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.0", version = "0.1.0" }`.
- The `br-rust-common` dependencies are pinned to the per-crate `0.1.0` tags (`br-core-auth`, `br-util-nats-fabric`, `br-core-integration`, `br-core-directory`, `br-scope-declaration-contract`), each `{crate}/v0.1.0`.
- The `oidc` feature depends on `oidc-test-idp` through its git tag `oidc-test-idp/v0.7.0`, the same git url and tag string a consumer uses, so a consumer that also depends on `oidc-test-idp` resolves a single copy of its types.

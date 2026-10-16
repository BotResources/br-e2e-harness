# Changelog

All notable changes to `br-test-harness` are documented here. The git tag
`br-test-harness/v{version}` releases the crate; this crate is versioned
separately from the other crates of the repository. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.2

### Fixed

- `GraphqlClient::query`, `query_unauthenticated`, `query_with_passport_header`, `post_raw` and `get_raw` panic with `graphql response body read failed` when the response body cannot be read (a body cut off by the server), instead of turning it into an empty string. `query` no longer reports a cut-off body as `graphql response not JSON`; that panic now carries the parse error and the body.
- `EventCapture` and `CommandCapture` panic with `fabric capture drain failed` on `count()`, `first()`, `for_correlation()` and `correlation_ids()` when their pull stream has ended (client drained, connection closed) or the NATS connection is not connected (the broker died), instead of reporting `count() == 0` and `first() == None`; a "no event / no command was published" assertion no longer passes on a dead or disconnected broker. The panic reads `fabric capture drain failed: the NATS connection is Disconnected`.
- `await_integration_event` returns `None` only when the deadline passes on a connected client. A missing stream, a consumer that cannot be created, a pull stream that ends or errors, a payload that is not JSON, and a deadline that passes on a dead or disconnected broker panic; they were all `None`, so a "no event arrived" assertion passed on a broken broker.
- `FabricTestNats::await_event` (`FabricAwaiter`), `FabricTestNats::await_command` (`CommandAwaiter`) and `DurableTap` (`next_within`, `expect_quiet`, `deliveries_within`) panic with `fabric capture drain failed: the NATS connection is Disconnected` when their deadline passes on a dead or disconnected broker, instead of returning `None` or `Timeout`, so `expect_quiet` no longer passes on a dead broker. `FabricAwaiter::new` keeps its signature; an awaiter built with it directly has no connection to watch, only the one `FabricTestNats::await_event` returns does.
- `BareFabricNats::{command_stream_absent, event_stream_absent, published_language_absent}` and `FabricTestNats::{raw_message_absent, durable_filter_subjects_if_present, fixed_streams_present, published_language_present, ephemeral_auth_present}` read "absent" only from the broker's own "not found" answer. A dead or timed-out broker panics; any error used to read as absent, so an absence assertion passed on a dead broker.
- `WsSubscription` returns `WsError::ErrorFrame` for a `next` frame whose payload carries `errors` and `WsError::Transport` for one with no `data` key, instead of `Ok(Null)`; an explicit `data: null` is still a push. Behaviour change: a `next` frame carrying both `data` and `errors` used to return `Ok(data)` and now returns `ErrorFrame`, so a consumer that relied on partial results must handle the error.
- `recreate_stream` and `recreate_kv` ignore only the broker's stream-not-found answer on the delete; any other delete failure is retried in the bounded loop and then panics, instead of the create silently returning the old stream or bucket with its data.
- `EventCapture` and `CommandCapture` also treat a drain task that finished without recording a cause as a dead drain (`fabric capture drain failed: the drain task stopped without recording a cause`), and `stop()` panics when the drain task itself panicked, instead of reading `count() == 0`.
- `verdict::is_ack` no longer acks a response whose `errors` member is present but not an array (`"errors": "boom"`); only `null`, an absent member and an empty array mean no errors.
- `SpawnedNats`, `SpawnedProcess::spawn` and `run_once` start a small `sh` guard that kills the `nats-server` or the spawned program (and, for `SpawnedNats`, removes its store directory) when the test process dies without running destructors (SIGKILL, a timeout kill); `kill_on_drop` alone left the server, or the service binary under test with its ports, pools and durable consumers, running after a killed suite (Unix only).
- `EventCapture` and `CommandCapture` reads of a dead drain all panic with `fabric capture drain failed`; the first panic no longer poisons the capture mutex so later reads do not panic with `capture mutex poisoned`.
- `SpawnedNats` start failures show a failed read of the server's stderr as `<stderr read failed: ...>` instead of an empty text.
- `SseSubscription` panics on a `next` frame whose data is missing or not JSON, whose payload is not a JSON object or has no `data` key (`{}`, `{"foo":1}`, `null`, `42`, `"x"`), like the WebSocket twin, instead of skipping it as if nothing was pushed (`expect_silence` passed while the service pushed a broken frame); an explicit `data: null` is still skipped. The `data:` lines of one frame are joined with a newline as the SSE format defines.
- `EventCapture` and `CommandCapture` stop their drain on a message that is not an envelope with `metadata.correlation_id`; every later read panics with `fabric capture drain failed: {subject}: payload is not an envelope ...` instead of skipping the message and reporting `count() == 0`. The capture replays the whole stream (deliver-all): on an attached shared broker, junk left on that coordinate by an earlier scenario makes the capture panic; clear it with `purge_command_subject` / `purge_event_subject`. `CommandAwaiter::await_correlation` panics on a non-envelope payload on its subject for the same reason.
- The `fabric-nats` CLI exits 3 with the cause of the failure (the panic text) instead of the fixed text `failed to connect to NATS`; a provisioning failure on a reachable broker no longer reads as a connection failure.
- `TestServer::spawn` panics when the server does not answer within 5 seconds (it used to return anyway), including a `/` handler that never resolves, and its serve task panics with `test server stopped serving` instead of ending silently.
- `SpawnedProcess` records a failed read of the child's pipe in `logs()` (`[harness] log capture stopped`) and keeps appending to, and `logs()` keeps reading, a poisoned log mutex, instead of dropping output without a trace.
- `E2eDatabase` teardown prints a warning when a `REVOKE` fails, like its other teardown steps.
- `fabric-nats verify` exits 4 with the broker's text (`probing durable ... : the broker did not answer "absent"`) when the durable probe gets a broker error, instead of reading it as an absent durable; exit 3 now means connect or fixed-stream provisioning failed. The README exit-code line says so.
- The copy-me `TestContext::kv_get` in the README panics on a KV transport error or a non-JSON value instead of reading both as an absent key.
- `TestServer::spawn` is covered by `tests/server.rs`: a router whose handler panics and a router whose handler never resolves both make the spawn panic after 5 seconds; a healthy router returns its base URL.
- `WsSubscription` returns `WsError::Transport` for a binary frame, for a text frame whose `type` is missing or unknown (`nxt`), instead of skipping it as if nothing was pushed (`Err(Timeout)`, so a "nothing was pushed" assertion passed while the service pushed a broken frame); `pong` and `connection_ack` stay ignored.
- `SseSubscription` panics on a block that carries `data:` lines under an `event:` that is not `next` or `complete` (`event: message`, a data-only block, `event: nxt`), instead of reading it as no push; comment-only blocks, blocks without `data:` and `complete` stay ignored.

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

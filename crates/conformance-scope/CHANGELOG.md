# Changelog

All notable changes to `conformance-scope` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.

## [0.2.0] — 2026-06-13

### Added

- Single-implementation check API: each S1–S6 check (plus a `declaration-content`
  content assertion) has one implementation in `checks/`, parameterized by a
  `CheckContext { js, readyz, capture, expected, service_key, behavior, timeout }`
  and returning a structured `CheckOutcome` instead of panicking. Both
  `tests/conformance.rs` and the new `conformance-scope-cli` binary call these —
  no second copy of the protocol logic.
- Structured outcome types: `CheckId`, `CheckStatus { Pass, Fail, Skipped }`,
  `CheckOutcome { id, status, expected, observed, detail }`, `ConformanceReport`
  with `passed()` / `failed()` / `skipped()` / `is_conformant()`.
- `ExpectedDeclaration` / `ExpectedScope` / `PlatformOnly` — the assertion input,
  with `platform_only` modeled **per scope** (faithful to `br-core-scope`'s
  `ScopeSpec.platform_only`); `assert_matches` renders an expected-vs-observed
  scope-set diff. Parsers `parse_scope_keys`, `parse_platform_only` (accepts a
  single bool or a per-scope `key=bool` CSV).
- `ReadyzProbe` — the readiness role decoupled from any spawned process, polling
  a given `/readyz` URL (spawn passes the subject's `base_url()`, attach passes
  the external URL).
- Two runners over the same checks: `run_spawn` (stands up `nats-server` + the
  subject binary, full `s1..s6`) and `run_attach` (zero host runtime deps:
  connects to a live service's NATS + `/readyz`, default `s1, s2` +
  `declaration-content`, never creates the stream — the service owns it, and the
  declare consumer fails loud if it is absent).
- `Scenario` / `AcceptorBehavior` with scenario parsing/defaults. In spawn mode
  the rejection scenario (s4) always exercises a rejection regardless of the
  global accept/reject flag; the global `--reject` reason flows through when
  given.
- `DEFAULT_TIMEOUT` (10s).

### Changed

- `tests/conformance.rs` now drives the single-implementation checks and asserts
  the returned `CheckOutcome`; the S1–S6 behavior is unchanged (a previously
  passing scenario still passes, a violation still fails). Adds two content
  assertions (matches-expected, flags-wrong-scopes).

## [0.1.0] — 2026-06-13

### Added

- Initial release: the G3 black-box conformance runner for the BotResources
  scope-declaration wire handshake. A **dev-dependency-only** crate that drives
  the frozen Go subject (`conformance-subjects/scope-service`) as a black box.
- `ScopeHarness` — `start()` builds the Go subject, and `start_with_binary` runs
  the same battery against any subject binary; both spawn a dedicated
  `SpawnedNats` and create the handshake JetStream stream (`identity.>`) up front
  (the platform never auto-provisions).
- `DeclareCapture` — a subscribe-first ephemeral consumer over the declare
  subject that drains declares; `decode()` validates the wire shape by
  deserializing the raw bytes into the **real**
  `IntegrationCommand<DeclareServiceScopes>` — deser success *is* the conformance
  check, with no hand-rolled shape guard. A broken drain fails loud: the cause is
  captured and re-raised through `count()` / `declares()` rather than masquerading
  as a silent subject.
- `accept` / `reject` — publish a confirmation built from the real
  `br-core-scope` payload types (`ServiceScopesAccepted` / `ServiceScopesRejected`),
  echoing the command's `correlation_id`, ack-confirmed.
- `Subject` / `SubjectConfig` — spawn the built binary with its env wiring and
  poll `/readyz` / `/livez` (status, plus `readyz_body()` for the rejection
  reason).
- `create_handshake_stream`, `build_subject`, and the three frozen subject
  constants.
- The S1–S6 conformance battery in `tests/conformance.rs`, `#[ignore]`-gated for
  real infra (`nats-server` + `go` + the spawned binary): declare-on-boot
  shape, readiness gating, re-publish-on-timeout, rejection handling, duplicate
  confirmations, and disabled mode.

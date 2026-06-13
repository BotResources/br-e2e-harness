# Changelog

All notable changes to `conformance-scope` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.

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

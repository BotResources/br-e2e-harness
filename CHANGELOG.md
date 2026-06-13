# Changelog

All notable changes to `br-e2e-harness` are documented here. The whole workspace
ships **one version**: every crate inherits `version.workspace = true`, and a
single git tag `v{version}` releases the set. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.

## [0.2.0] — 2026-06-13

### Changed

- **Unified workspace versioning.** The whole repository now ships **one
  version** behind a single `v{version}` git tag (this release: `v0.2.0`),
  replacing the prior per-crate tags (`<crate>-vX.Y.Z`). Every crate inherits
  `version.workspace = true` from the root `Cargo.toml`; a consumer pins the
  harness by the unified tag, never a per-crate version. Prior per-crate
  versions folded into this release: `br-test-harness` 0.1.0,
  `conformance-scope` 0.2.0, `conformance-scope-cli` 0.1.0, `oidc-test-idp`
  0.1.0.
- **Single root CHANGELOG.** History is consolidated here from the four former
  per-crate `crates/*/CHANGELOG.md` files, which are removed.
- **`br-rust-common` dependency bumped to `tag = "v0.8.0"`** (was an older git
  rev). All crates depending on `br-core-*` now pin the same `v0.8.0` tag, so
  Cargo resolves a single source and never duplicates `br-core-*` in the graph.
  `br_core_integration::MessageMetadata` was renamed upstream to `EventMetadata`.

### Added

#### `br-test-harness` — real-infra end-to-end test harness (was 0.1.0)

- The consolidated real-infra end-to-end test harness for BotResources platform
  services, distilled from the three divergent local copies (be-botresources
  `test-harness`, svc-identity's `e2e_support`, svc-notifier's `tests/common`).
  A **dev-dependency-only** crate.
- `E2eDatabase` — provisions a throwaway Postgres database + a non-superuser,
  CNPG-shaped owner role (explicit `BYPASSRLS` posture; suffixed or GitOps-exact
  names) for a spawned binary to migrate and run against under real RLS.
- `SpawnedNats` — a dedicated, ephemeral `nats-server -js` per test chain, so
  binaries that hardcode their bucket names still isolate.
- `TestNats` — a connection to a real NATS JetStream server plus isolated,
  per-test KV buckets; hands back **raw** `async_nats` handles (no project port
  trait), and forwards URL-embedded credentials (the async-nats workaround).
- `SpawnedProcess` / `run_once` — run the real service binary (or a one-shot
  CLI) as a child, draining stdout/stderr and polling an HTTP readiness URL,
  with captured logs surfaced on failure.
- `TestServer` — an in-process Axum server on a random port.
- `PassportBuilder` — forge a `Passport` (`Human` or `Service`), built directly
  on `br-core-auth` (no dependency on any project's private `infra` crate). It
  owns the `Passport` **structure** — typed setters for the canonical fields
  (`user_id`, `super_admin`, `active`, `pat` auth method, `impersonator`) —
  while the free-form `claims` keys are a **per-project seam**: the project
  passes its own via `.claim(key, value)` / `.claims(iter)`. The harness names
  no project claim key.
- `GraphqlClient` — GraphQL / REST client that attaches the forged `X-Passport`
  header (and an unauthenticated variant to pin rejection).
- `WsSubscription` — a real `graphql-transport-ws` subscription client with
  `next_data` (single push) and **`next_matching` drain-until-match**, the
  de-flake primitive for broadcast subscriptions; parameterizable WS path.
- `SseSubscription` — a Server-Sent-Events subscription client
  (`next_event` / `expect_event` / `expect_silence`), generalized so the caller
  indexes its own subscription root field.
- `wait_until` — poll an async condition up to a bounded deadline — the de-flake
  for "the side-effect lands a beat after the ack".
- `oidc` — re-export of the sibling `oidc-test-idp` fixture, so a service gets
  infra + a pilotable in-process OIDC IdP from this single dev-dependency.

#### `conformance-scope` — black-box scope-declaration conformance runner (was 0.2.0)

- The G3 black-box conformance runner for the BotResources scope-declaration
  wire handshake. A **dev-dependency-only** crate that drives the frozen Go
  subject (`conformance-subjects/scope-service`) as a black box.
- `ScopeHarness` — `start()` builds the Go subject, and `start_with_binary` runs
  the same battery against any subject binary; both spawn a dedicated
  `SpawnedNats` and create the handshake JetStream stream (`identity.>`) up front
  (the platform never auto-provisions).
- `DeclareCapture` — a subscribe-first ephemeral consumer over the declare
  subject that drains declares; `decode()` validates the wire shape by
  deserializing the raw bytes into the **real**
  `IntegrationCommand<DeclareServiceScopes>` — deser success *is* the
  conformance check, with no hand-rolled shape guard. A broken drain fails loud:
  the cause is captured and re-raised through `count()` / `declares()` rather
  than masquerading as a silent subject.
- `accept` / `reject` — publish a confirmation built from the real
  `br-core-scope` payload types (`ServiceScopesAccepted` /
  `ServiceScopesRejected`), echoing the command's `correlation_id`,
  ack-confirmed.
- `Subject` / `SubjectConfig` — spawn the built binary with its env wiring and
  poll `/readyz` / `/livez` (status, plus `readyz_body()` for the rejection
  reason).
- `create_handshake_stream`, `build_subject`, and the three frozen subject
  constants.
- The S1–S6 conformance battery in `tests/conformance.rs`, `#[ignore]`-gated for
  real infra (`nats-server` + `go` + the spawned binary): declare-on-boot
  shape, readiness gating, re-publish-on-timeout, rejection handling, duplicate
  confirmations, and disabled mode.
- Single-implementation check API: each S1–S6 check (plus a `declaration-content`
  content assertion) has one implementation in `checks/`, parameterized by a
  `CheckContext` and returning a structured `CheckOutcome` instead of panicking.
  Both `tests/conformance.rs` and the `conformance-scope-cli` binary call these —
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

#### `conformance-scope-cli` — language-agnostic CLI over the battery (was 0.1.0)

- The `conformance-scope` CLI, a language-agnostic wrapper over the
  `conformance-scope` S1–S6 battery. All protocol logic lives in the lib;
  the binary parses arguments, builds the run config, calls the lib runner,
  formats the report, and sets the exit code.
- `conformance-scope run` — drive a single service in **attach** mode (default,
  zero host deps: `--nats` + `--readyz` against a live service, `--stream` for
  the pre-existing handshake stream) or **spawn** mode (`--spawn <PATH>`, needs
  `nats-server` on PATH). Expected declaration via `--service-key`, `--scopes`
  (CSV), `--platform-only` (a single bool or a per-scope `key=bool` CSV).
  Acceptor behavior via `--accept` (default) / `--reject [REASON]`. Scenario
  selection `--scenarios`, `--timeout`, `--format`, `--output`.
- `conformance-scope manifest <FILE>` — run a YAML manifest of many services
  (each `attach` or `spawn`) and aggregate one report.
- Report formats: `human`, `json`, `junit` — each check shows its id, expected,
  observed, and a wire/file excerpt on failure; a wrong-scopes failure reads as
  a clear expected-vs-observed diff.
- Exit codes: `0` fully conformant, `1` at least one check failed, `2` a
  usage / connection / I/O error.

#### `oidc-test-idp` — pilotable OIDC test IdP (was 0.1.0)

- A pilotable OIDC test IdP, shipped both as a library fixture (re-exported by
  `br-test-harness`) and as a container image (`br-oidc-test-idp`).
- `GET /.well-known/openid-configuration` — minimal, honest discovery document
  (`issuer`, `jwks_uri`, RS256; no endpoints the fixture does not implement).
- `GET /jwks` — serves exactly the *published* subset of the pre-generated
  RSA key pool.
- `POST /admin/mint` — signs an RS256 id_token with any pool key (published
  or not), pilotable `aud`, email claim name, expiry (negative = already
  expired), extra/override claims, and optional `kid`-header omission.
- `POST /admin/rotate` — instantaneous key rotation (the whole pool is
  generated at startup): default gesture publishes the next key and makes it
  active; explicit form publishes/unpublishes/re-activates arbitrary kids.
- `POST /admin/reset` — restores the startup state and zeroes counters.
- `GET /admin/state` — snapshot including `jwks_fetches`/`discovery_fetches`
  counters, so tests assert refresh/cooldown behaviour without sleeping.
- `GET /health`, `--version` (CD smoke-test), fail-closed env parsing.

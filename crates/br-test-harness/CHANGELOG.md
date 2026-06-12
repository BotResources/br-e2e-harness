# Changelog

All notable changes to `br-test-harness` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.

## [0.1.0] — 2026-06-12

### Added

- Initial release: the consolidated real-infra end-to-end test harness for
  BotResources platform services, distilled from the three divergent local
  copies (be-botresources `test-harness`, svc-identity's `e2e_support`,
  svc-notifier's `tests/common`). A **dev-dependency-only** crate.
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
  (`user_id`, `super_admin`, `active`, `pat` auth method, `impersonator`) — while
  the free-form `claims` keys are a **per-project seam**: the project passes its
  own via `.claim(key, value)` / `.claims(iter)`. The harness names no project
  claim key.
- `GraphqlClient` — GraphQL / REST client that attaches the forged `X-Passport`
  header (and an unauthenticated variant to pin rejection).
- `WsSubscription` — a real `graphql-transport-ws` subscription client with
  `next_data` (single push) and **`next_matching` drain-until-match**, the
  de-flake primitive for broadcast subscriptions (harvested from svc-identity
  #64); parameterizable WS path.
- `SseSubscription` — a Server-Sent-Events subscription client
  (`next_event` / `expect_event` / `expect_silence`), generalized from
  svc-notifier so the caller indexes its own subscription root field.
- `wait_until` — poll an async condition up to a bounded deadline (harvested
  from svc-notifier) — the de-flake for "the side-effect lands a beat after the
  ack".
- `oidc` — re-export of the sibling `oidc-test-idp` fixture, so a service gets
  infra + a pilotable in-process OIDC IdP from this single dev-dependency.

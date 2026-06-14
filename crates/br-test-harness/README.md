# br-test-harness

The **real-infra end-to-end test harness** for BotResources platform services —
the "mock nothing" plumbing every service needs to write its *first* e2e test.

> ⚠️ **TEST FIXTURE ONLY.** Add it as a **dev-dependency**, never a runtime one.
> It forges `X-Passport` headers (the service is *inside* the trust boundary the
> gateway normally establishes), provisions throwaway databases with elevated
> roles, and spawns real binaries. Never link it into a production build.

The BR doctrine is *real* end-to-end tests: real Postgres, real NATS JetStream +
KV, the real service binary, a real `X-Passport` over the wire — **mock nothing
inside the system under test, and no test backdoor in any production binary**.
Test accommodations belong in the *harness*; you substitute a *dependency* with a
controllable equivalent, you never weaken production code. Standing up that
harness is, today, the single largest entry tax on a new service. This crate pays
it once.

Unlike a service's local copy, this harness binds to **no project's private
crate**. It depends on exactly one platform crate — `br-core-auth`, for the
`Passport` / `X-Passport` codec — and otherwise hands back **raw** `sqlx`,
`async_nats`, and `reqwest` handles. Any BR service, whatever its own KV or
persistence abstraction, can inject them.

## What it gives you

| Need | Helper | Feature |
|---|---|---|
| A throwaway Postgres DB + non-superuser, CNPG-shaped owner role | `E2eDatabase` | `e2e-db` |
| A dedicated, ephemeral `nats-server -js` for one test chain | `SpawnedNats` | `spawned-nats` |
| A connection to NATS + isolated KV buckets | `TestNats` | `nats` |
| Run the real service binary as a child (drained, readiness-polled) | `SpawnedProcess`, `run_once` | *(always on)* |
| An in-process Axum server on a random port | `TestServer` | `server` |
| A forged `Passport` (`Human` / `Service`) | `PassportBuilder` | `passport` |
| A GraphQL / REST client that sends the `X-Passport` header | `GraphqlClient` | `graphql` |
| A live GraphQL `graphql-transport-ws` subscription (with drain-until-match) | `WsSubscription` | `ws` |
| A live GraphQL Server-Sent-Events subscription | `SseSubscription` | `sse` |
| Poll an async condition until it holds or times out | `wait_until` | *(always on)* |
| A pilotable in-process OIDC IdP (discovery, JWKS, mint, rotate) | `oidc` | `oidc` |

### Claim keys are a per-project seam

`PassportBuilder` owns the `Passport` **structure** — it exposes typed setters
for `br-core-auth`'s canonical fields (`user_id`, `super_admin`, `active`, `pat`
for the auth method, `impersonator`) — but it names **none** of the free-form
`claims` keys. Those (`email`, `org_id`, roles, a tenant id, scopes, …) vary per
project, so each consuming **project supplies its own** via `.claim(key, value)`
(or `.claims(iter)`). The harness stays project-agnostic; there is no canonical
or "standard" key list to keep in lockstep, by design.

### The de-flake primitives

Two helpers exist specifically to kill the classic e2e flakes:

- **`WsSubscription::next_matching(predicate, timeout)`** — drain-until-match.
  A broadcast subscription is woken by *every* event in its class, so a socket
  opened mid-chain can receive an in-flight earlier-transition frame before the
  one the assertion wants. `next_matching` skips non-matching frames until the
  predicate holds, and on timeout reports every frame it skipped — so a *genuine*
  miss is still fully diagnosable. (`next_data` remains, for the single-push case.)
- **`wait_until(timeout, predicate)`** — poll an async condition (a projection
  row landed, an integration event was published, a NATS consumer count moved)
  up to a bounded deadline, instead of sleeping a fixed amount and hoping.

## Why — the non-obvious bits

The source is comment-free by design (it is read by agents, for which comments
are token overhead). The intent that the signatures don't make obvious lives
here, synthetically:

| Thing | Why it is the way it is |
|---|---|
| `E2eDatabase` owner = `NOSUPERUSER` + a `bypassrls` flag | The owner mirrors the CNPG GitOps owner: an RLS-bypassing agent for migrations/bootstrap, `NOSUPERUSER` so `FORCE ROW LEVEL SECURITY` behaves exactly as in prod. Pass `bypassrls = false` **only** for negative tests asserting a misdeclared owner is caught — it reproduces incident **2026-06-11**, where a no-bypass owner under `FORCE RLS` read zero rows and a deploy cascade failed. |
| `create*`'s `managed_roles` silently skips unknown roles | Only roles that **already exist** are granted to the owner `WITH ADMIN OPTION` (PG16: a `CREATEROLE` role needs `ADMIN` on a role to `ALTER` it). Roles the service creates itself at startup are deliberately left alone — this is not a typo guard. |
| `TestNats` always creates **two** KV buckets | The second (`bearer_kv`) stands in for the shared `bearer_tokens` bucket `svc-auth` reads for PAT lookup; inject it wherever production expects that bucket. |
| `SpawnedNats` *vs* `TestNats` | A binary that **hardcodes** its bucket names can't be isolated by per-bucket names → give it its own server (`SpawnedNats`, which lets `nats-server` self-assign its port — race-free under parallel `cargo test`). Tests using the harness's own **suffixed** buckets share one server (`TestNats`). |
| Subscriptions fail loud on a broken stream | A GraphQL `errors` payload, a transport error, or an `error` frame is a hard failure (SSE panics, WS returns `Err`) — never a frame to skip. `SseSubscription::next_event` returns `None` **only** for a genuine timeout or a clean stream end, so `expect_silence` can't be fooled into passing on a stream that actually broke. |
| `TestServer::spawn` readiness is best-effort | It polls `GET /` and treats **any** HTTP response — including a 404 — as "up": that proves the in-process server is serving, it is not a dependency-readiness gate, and after ~500 ms it returns anyway (the first real request surfaces a genuine failure). For real readiness against a spawned *binary*, use `SpawnedProcess::wait_for_http_ok` against the service's own health path. |
| `deny.toml` ignores `RUSTSEC-2023-0071` (Marvin timing attack in `rsa`) | The advisory is a side-channel in `rsa` private-key *decryption*. This workspace only ships test fixtures that **sign** short-lived tokens on isolated test networks — no decryption path, no timing-oracle adversary in the threat model. Same ignore as `svc-auth`'s CI. |
| `deny.toml` allows `CDLA-Permissive-2.0` | Mozilla's CA-root data set, vendored by `webpki-roots`, pulled in transitively by the rustls stack under `reqwest` / `sqlx` / `async-nats`. It is an OSI-recognized permissive *data* license with no copyleft and no patent traps — safe for a fixtures repo. Added when `br-test-harness` brought the rustls TLS stack in. |

## Cargo features

Everything is on by default (`default = ["full"]`), so a service e2e suite that
wants the whole toolbox depends on the crate and changes nothing. A consumer that
needs only a slice — e.g. a CLI that talks to NATS but touches no Postgres, Axum
or JWT — sets `default-features = false` and names the slice, keeping the heavy
transitive deps out of its binary:

| Feature | Unlocks | Headline heavy deps |
|---|---|---|
| `nats` | `TestNats` | `async-nats` |
| `spawned-nats` | `SpawnedNats` | `tempfile` |
| `e2e-db` | `E2eDatabase` | `sqlx` |
| `server` | `TestServer` | `axum`, `reqwest` |
| `passport` | `PassportBuilder` | `br-core-auth` |
| `graphql` | `GraphqlClient` | `reqwest` (+ `passport`) |
| `sse` | `SseSubscription` | `reqwest`, `futures-util` (+ `passport`) |
| `ws` | `WsSubscription` | `tokio-tungstenite`, `futures-util` (+ `passport`) |
| `oidc` | the in-process OIDC IdP | `oidc-test-idp` → `rsa` |

`SpawnedProcess` / `run_once` / `wait_until` are always compiled — their only deps
are `tokio` + std — so the smallest useful dependency is `default-features =
false` with no feature at all. `conformance-scope` rides exactly this: it takes
`["nats", "spawned-nats"]`, so its CLI binary carries no `sqlx` / `axum` / `rsa`.

## Install

It is a **dev-dependency**. Pin it to a release tag (git-tag distribution; no
crates.io — same model as the rest of the platform):

```toml
[dev-dependencies]
br-test-harness = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v0.3.0" }

# …or slim — only part of the toolbox, no `sqlx`/`axum`/`rsa` in your build:
br-test-harness = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v0.3.0", default-features = false, features = ["nats", "spawned-nats"] }
```

With the default `full` feature, `br-test-harness` depends on `br-core-auth`
pinned to the `br-rust-common` tag `v0.10.0` (it backs `PassportBuilder`; a slim
build that omits the passport-bearing features drops it). If your service already
pins `br-rust-common`, keep both on the **same tag** so Cargo resolves a single
source (two refs of one git URL are two distinct sources and duplicate
`br-core-*` in the graph).

## Running the tests it powers

The harness drives **real** infrastructure; it never mocks it to make a test
pass — it fails loud when infra is missing. A service whose suite uses it needs:

- **Postgres** reachable, via `E2E_PG_ADMIN_URL` (falling back to `DATABASE_URL`)
  with a role allowed to `CREATE DATABASE` / `CREATE ROLE` — e.g.
  `postgresql://postgres@localhost:5432/postgres`.
- **NATS JetStream** — either `NATS_URL` (e.g. `nats://localhost:4222`) for the
  shared-server / per-bucket isolation path, or `nats-server` on `PATH` for the
  `SpawnedNats` dedicated-server path.

`.env` is loaded automatically (a plain `#[tokio::test]` does not do this).

### The harness's own self-tests

The two load-bearing real-infra paths — `SpawnedNats` (spawns a real
`nats-server` and reads back its bound port) and `E2eDatabase` (provisions the
ephemeral owner role and the transaction-local RLS context) — have focused
self-tests in `tests/`. They are **`#[ignore]`-gated** so the default
`cargo test` stays green without infra; run them explicitly:

```sh
# SpawnedNats path — needs `nats-server` on PATH (it spawns its OWN server):
cargo test -p br-test-harness --test spawned_nats -- --ignored

# E2eDatabase path — needs an admin Postgres able to CREATE ROLE / CREATE DATABASE,
# via E2E_PG_ADMIN_URL (falling back to DATABASE_URL):
E2E_PG_ADMIN_URL=postgresql://postgres:postgres@localhost:5432/postgres \
  cargo test -p br-test-harness --test e2e_db -- --ignored
```

CI runs both in the `infra-e2e` job against a Postgres service container and a
runner-installed `nats-server`.

## Wiring it into a service's e2e tests

Two shapes, both real-infra:

**In-process** — provision infra, build the service's router/schema with the
harness pool + NATS, mount it on a `TestServer`, drive it with a `GraphqlClient`
and a forged `PassportBuilder`:

```rust,ignore
use br_test_harness::{TestNats, PassportBuilder, GraphqlClient, TestServer};

#[tokio::test]
async fn lists_only_my_rows() {
    let nats = TestNats::setup().await;                  // real NATS + isolated buckets
    let app  = my_service::build_router(pool, nats.kv().clone());
    let srv  = TestServer::spawn(app).await;

    // The harness owns the Passport structure; the project owns its claim keys.
    let passport = PassportBuilder::new().claim("email", "alice@example.com").claim("scopes", vec!["read"]).build();
    let gql = GraphqlClient::new(&srv.base_url);
    let res = gql.query(&passport, QUERY, serde_json::json!({})).await;
    assert_eq!(res["data"]["things"].as_array().unwrap().len(), 1);

    nats.cleanup().await;
}
```

**True end-to-end** — provision an `E2eDatabase` + a `SpawnedNats`, spawn the
real binary with `SpawnedProcess` (it runs its own migrations and connects as the
owner role), then drive it over HTTP / WS and assert against the DB + NATS:

```rust,ignore
use std::time::Duration;
use br_test_harness::{E2eDatabase, SpawnedNats, SpawnedProcess, WsSubscription, PassportBuilder};

let db   = E2eDatabase::create_named("identity", "identity_owner", true, &["identity_app"]).await;
let nats = SpawnedNats::start().await;
let mut svc = SpawnedProcess::spawn(
    env!("CARGO_BIN_EXE_my-service"),
    &["serve"],
    &[("DATABASE_URL", &db.owner_url()), ("NATS_URL", &nats.url())],
);
svc.wait_for_http_ok(&format!("{base}/health"), Duration::from_secs(30)).await.unwrap();

let passport = PassportBuilder::new().super_admin(true).build();
let mut sub  = WsSubscription::open(&base, &passport, SUBSCRIPTION).await.unwrap();
// … mutate …
let push = sub.next_matching(|d| d["thingChanged"]["status"] == "ACTIVE", Duration::from_secs(5)).await.unwrap();

svc.shutdown().await;
nats.shutdown().await;
db.cleanup().await;
```

## Relationship to `oidc-test-idp`

The sibling [`oidc-test-idp`](../oidc-test-idp/README.md) — the pilotable OIDC
test IdP — is **re-exported as `br_test_harness::oidc`**. A service that turns an
OIDC `id_token` into an internal credential gets infra **and** a controllable IdP
from this single dev-dependency: build `oidc::IdpState`, mount `oidc::router` on a
`TestServer`, point the system under test's `ISSUER` at its base URL, and mint /
rotate keys straight from the test — no second container. (The same fixture also
ships as `ghcr.io/botresources/br-oidc-test-idp` for the out-of-process case.)

## License

Apache-2.0. MSRV **1.88** (edition 2024).

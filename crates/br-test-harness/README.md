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
| A DB + owner + RLS-subject **app role** in one RLS-safe call (the default) | `E2eDatabase::create_with_app_role` | `e2e-db` |
| A second, RLS-subject **app role** on an existing DB (the runtime role) | `E2eDatabase::with_app_role` | `e2e-db` |
| A dedicated, ephemeral `nats-server -js` for one test chain | `SpawnedNats` | `spawned-nats` |
| A connection to NATS + isolated KV buckets | `TestNats` | `nats` |
| Await a producer's integration event on a named stream + subject | `await_integration_event` | `nats` |
| Reset a *named* service-contract stream / KV bucket clean per test | `recreate_stream`, `recreate_kv` | `nats` |
| Run the real service binary as a child (drained, readiness-polled) | `SpawnedProcess`, `run_once` | *(always on)* |
| Run the real service binary as a child (drained, readiness-polled, boot-classified) | `SpawnedProcess`, `BootOutcome`, `run_once` | *(always on; HTTP polling needs `http-client`)* |
| An in-process Axum server on a random port | `TestServer` | `server` |
| A forged `Passport` (`Human` / `Service`) | `PassportBuilder` | `passport` |
| A GraphQL / REST client that sends the `X-Passport` header | `GraphqlClient` | `graphql` |
| Verdict helpers over a GraphQL response — ack / rejection / stable-code | `verdict::*` | `graphql` |
| A live GraphQL `graphql-transport-ws` subscription (with drain-until-match) | `WsSubscription` | `ws` |
| A live GraphQL Server-Sent-Events subscription | `SseSubscription` | `sse` |
| Poll an async condition until it holds or times out | `wait_until` | *(always on)* |
| A pilotable in-process OIDC IdP (discovery, JWKS, mint, rotate) | `oidc` | `oidc` |

## The 4-channel observation pattern (the BR e2e doctrine)

A BR service e2e scenario **is** the executable functional spec — the verification
floor for every service, and the *primary* spec surface for **fatty** units, which
have no pure-domain `command → event` tests-as-spec. A scenario is a complete
functional flow (Given/When/Then, hosted in Outline; `tests/` is its executable
transcript), and it asserts that flow across **four observation channels** —
nothing else is a legitimate window into the running service:

| # | Channel | What it observes | Harness surface |
|---|---|---|---|
| 1 | **Mutation ack / error** | a mutation returns a *verdict*, never state: an ack on success, or a structured error carrying a **stable code** on rejection | `verdict::{expect_ack, is_ack, expect_rejected, expect_code_shaped, is_code_shaped, mutation_error_code}` over a `GraphqlClient` response |
| 2 | **Query + affordances** | the authoritative current state *and* the `{ action, allowed, reasonCode }` affordances the service computes for the caller's Passport | `GraphqlClient::query` + the same `verdict` helpers (the affordance-skip guarantee keeps an affordance `reasonCode` from reading as a mutation error) |
| 3 | **Subscriptions** | the near-unfiltered domain-event stream the frontend folds — every transition, with its code | `SseSubscription::{next_event, expect_event, expect_silence, drain}` |
| 4 | **Published contract** (KV + integration events) | the service's *published language* — its integration events on a named, GitOps-provisioned JetStream stream, and the KV mirror it writes for downstream consumers | `await_integration_event(js, stream, subject, deadline)` + `recreate_stream` / `recreate_kv` to reset the named bus clean per scenario |

The non-negotiable rule that makes a scenario a *spec* and not a *fixture*: **state
is built only through GraphQL mutations — never through DB seeds**. A seeded
scenario can pass while the mutation write path is broken; a mutation-built one
cannot. The `flows.rs` helpers a service writes (multi-step create → propose → vote
sequences) are therefore pure compositions of channel-1 mutations, never SQL.

Two supporting primitives underpin the four channels:

- **Two-role Postgres** — `E2eDatabase::create_with_app_role(owner, db, app, pw,
  bypassrls, …)` (or `create(bypassrls, …).with_app_role(name, pw)`): the
  `BYPASSRLS` **owner** runs the migrations (`owner_migration_url()`) and reads raw
  state, the RLS-subject **app role** is the runtime role the service connects as
  (`app_url()` — this is the binary's `DATABASE_URL`, never the owner). Channel 2's
  RLS assertions (a caller sees only its own rows) run as the app role while the
  owner — or the superuser `admin_url()` — reads the unfiltered truth.
- **Fail-loud boot** — `SpawnedProcess::await_boot(url, timeout) -> BootOutcome`
  (`Ready` / `Exited(status)` / `TimedOut`): the S0-style "the service refuses to
  boot when its declared GitOps infra is missing, and names the missing resource"
  scenario asserts an `Exited` outcome and greps `proc.logs()` for the named piece.
  `wait_for_http_ok` remains the happy-path "ready or fail the test" checker.

## Copy-me: the per-service `TestContext` assembly

The harness ships the **blocks**, deliberately **not** an opinionated composite — so
the small amount of glue that wires them into one running service stays the service's
own, expressed in its own vocabulary (its actors, its stream names, its env-var
contract, its readiness gate). That glue is the `TestContext` below: a new service
copies this ~80-line template, swaps the constants for its own, and writes its first
4-channel scenario **without re-implementing a single verdict/observation helper** —
every one of those is imported from `br-test-harness`.

The reference is `svc-charter` (the BR-fatty reference unit). What each service
**owns** is exactly: the named actors, the named contract streams + KV bucket, the
two PG role names, the binary's env-var contract, and the restart/boot helpers. What
it **imports** is everything else.

```rust,ignore
use std::time::Duration;

use br_test_harness::{
    await_integration_event, recreate_kv, recreate_stream, E2eDatabase, GraphqlClient,
    SpawnedNats, SpawnedProcess, SseSubscription,
};
use serde_json::Value;
use uuid::Uuid;

const READY_TIMEOUT: Duration = Duration::from_secs(20);
const PUSH_TIMEOUT: Duration = Duration::from_secs(5);

const CHARTER_STREAM: &str = "CHARTER";
const IDENTITY_STREAM: &str = "IDENTITY";
const CHARTER_KV_BUCKET: &str = "charter";

const OWNER_ROLE: &str = "charter_owner";
const APP_ROLE: &str = "charter_app";
const APP_PW: &str = "charter_app_test_pw";

pub struct Actors {
    pub su_a: Uuid,
    pub b: Uuid,
    pub c: Uuid,
    pub worker: Uuid,
}

pub struct TestContext {
    pub gql: GraphqlClient,
    pub base_url: String,
    pub actors: Actors,
    db: E2eDatabase,
    nats: SpawnedNats,
    js: async_nats::jetstream::Context,
    kv: async_nats::jetstream::kv::Store,
    service: SpawnedProcess,
}

impl TestContext {
    pub async fn setup() -> Self {
        let db =
            E2eDatabase::create_with_app_role(OWNER_ROLE, "charter_test", APP_ROLE, APP_PW, true, &[])
                .await;

        let nats = SpawnedNats::start().await;
        let client = br_test_harness::nats::connect(&nats.url()).await.unwrap();
        let js = async_nats::jetstream::new(client.clone());
        recreate_stream(&js, IDENTITY_STREAM, &["identity.>"]).await;
        recreate_stream(&js, CHARTER_STREAM, &["charter.>"]).await;
        let kv = recreate_kv(&js, CHARTER_KV_BUCKET).await;

        let port = free_port();
        let base_url = format!("http://127.0.0.1:{port}");
        let mut service = SpawnedProcess::spawn(
            env!("CARGO_BIN_EXE_svc-charter"),
            &["serve"],
            &[
                ("PORT", &port.to_string()),
                ("DATABASE_URL", &db.app_url()),
                ("DATABASE_URL_OWNER", &db.owner_migration_url()),
                ("NATS_URL", &nats.url()),
                ("SCOPE_DECLARATION_ENABLED", "false"),
            ],
        );
        let boot = service
            .await_boot(&format!("{base_url}/readyz"), READY_TIMEOUT)
            .await;
        assert!(boot.is_ready(), "svc-charter did not boot: {}", service.logs());

        Self {
            gql: GraphqlClient::new(&base_url),
            base_url,
            actors: Actors {
                su_a: Uuid::now_v7(),
                b: Uuid::now_v7(),
                c: Uuid::now_v7(),
                worker: Uuid::now_v7(),
            },
            db,
            nats,
            js,
            kv,
            service,
        }
    }

    pub async fn subscribe(&self, passport: &br_core_auth::Passport, query: &str) -> SseSubscription {
        SseSubscription::open(&self.base_url, passport, query).await
    }

    pub async fn await_event(&self, subject: &str) -> Option<Value> {
        await_integration_event(&self.js, CHARTER_STREAM, subject, PUSH_TIMEOUT).await
    }

    pub async fn kv_get(&self, key: &str) -> Option<Value> {
        match self.kv.get(key.to_string()).await {
            Ok(Some(bytes)) => serde_json::from_slice(&bytes).ok(),
            _ => None,
        }
    }

    pub async fn teardown(self) {
        self.service.shutdown().await;
        self.nats.shutdown().await;
        self.db.cleanup().await;
    }
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
    l.local_addr().expect("read port").port()
}
```

A scenario then reads as a flat transcript over the four channels:

```rust,ignore
#[tokio::test]
async fn s3_member_can_be_added_and_the_change_is_published() {
    let ctx = TestContext::setup().await;
    let su = PassportBuilder::new().user_id(ctx.actors.su_a).super_admin(true).build();

    let res = ctx.gql.query(&su, CHARTER_ADD_MEMBER,
        serde_json::json!({ "userId": ctx.actors.b.to_string() })).await;
    verdict::expect_ack(&res, "add college member");                     // channel 1

    let college = ctx.gql.query(&su, Q_COLLEGE, serde_json::json!({})).await;
    assert_eq!(college["data"]["charterCollege"]["members"].as_array().unwrap().len(), 1); // channel 2

    let event = ctx.await_event("charter.evt.college.member_added.v1").await
        .expect("the member-added integration event is published");      // channel 4
    assert_eq!(event["userId"], ctx.actors.b.to_string());

    ctx.teardown().await;
}
```

This is the **decided design (B.2.a of issue #55): blocks-only, charter's assembly
blessed as the documented copy-me template** — not an opinionated `TestContext`
builder shipped by the harness. The composite varies per service (actors, stream
names, role names, the env contract, whether the scope handshake is opted out); the
harness keeps its raw-handles philosophy and lets each service own the ~80 lines that
are genuinely its own.

### Claim keys are a per-project seam

`PassportBuilder` is **re-exported from `br-core-auth`** (its `test-support`
feature) — it lives next to the `Passport` type it forges, so it tracks every
field change with zero drift, and the harness re-exports it under the same
`br_test_harness::PassportBuilder` path. It exposes typed setters for
`br-core-auth`'s canonical fields (`user_id`, `super_admin`, `active`, `pat`
for the auth method, `impersonator`) — but it names **none** of the free-form
`claims` keys. Those (`email`, `org_id`, roles, a tenant id, scopes, …) vary per
project, so each consuming **project supplies its own** via `.claim(key, value)`
(or `.claims(iter)`). The harness stays project-agnostic; there is no canonical
or "standard" key list to keep in lockstep, by design.

### Two runtime roles, for RLS assertions

`E2eDatabase` provisions the **owner** (`NOSUPERUSER`, optionally `BYPASSRLS`)
that owns the schema and runs migrations. A service that enforces authZ via
Postgres RLS also needs the **runtime role** the app actually connects as — the
RLS *subject*, gated by `current_setting('app.current_user_id')`. The RLS-safe
two-role provisioning is the **default**, in one call:

```rust
let db = E2eDatabase::create_with_app_role(
    "svc_owner", "svc_db", "svc_app", "app_pw", true, &[],  // owner BYPASSRLS, app NOBYPASSRLS
).await;

let owner = PgConnection::connect(&db.owner_migration_url()).await?; // migrate + raw-state asserts
let app   = PgConnection::connect(&db.app_url()).await?;             // the runtime DATABASE_URL, sees only its rows
let admin = PgConnection::connect(db.admin_url()).await?;            // the superuser: posture / cluster asserts
```

The runtime binary's `DATABASE_URL` is **`app_url()`**, never
`owner_migration_url()`: the owner carries `BYPASSRLS` and exists only to run
migrations and read raw state. Wiring the owner DSN as `DATABASE_URL` runs the
service with RLS silently off — the misuse `owner_migration_url()` is named to
make obvious. `create(bypassrls, …).with_app_role(name, pw)` remains for the rare
case that adds an app role to an already-provisioned DB.

`with_app_role` is **idempotent and concurrency-safe** — it ensures the role via a
`DO … IF NOT EXISTS … $$` block run inside a transaction that first takes
`pg_advisory_xact_lock(hashtext(name))`, so two parallel binaries/worktrees that
ensure the **same shared role name** serialize on the server instead of writing the
same `pg_authid` row simultaneously and aborting the loser with
`tuple concurrently updated`. It then `GRANT CONNECT`s the role to the fresh DB and
runs `GRANT USAGE, CREATE ON SCHEMA public` plus the two `ALTER DEFAULT PRIVILEGES
… TO <app>` (tables + sequences) — each likewise under its advisory lock — so the
tables the owner's migrations *later* create are reachable by the runtime role.
Role **names** are caller-supplied (escaped via `quote_ident`); the grant dance is
the generic part. An **RLS isolation test** sets the principal transaction-locally
as the app role and asserts it sees only its own rows, while the `BYPASSRLS` owner
(or the superuser `admin_url()`) reads the raw, unfiltered state. `app_url()`
panics if `with_app_role` / `create_with_app_role` was never called; `cleanup`
drops only the per-test DB and the per-test owner — the app role persists (like
`managed_roles`), since a shared name may be in use by a parallel run. The owner is
created the same way (advisory-locked ensure), the shared `CREATE DATABASE` /
`DROP DATABASE` path is wrapped in a session-level `pg_advisory_lock`, and a `Drop`
net tears the DB + owner down even when a test panics before `cleanup` — so a
crashed run no longer leaks a role that dies the next run on `42710`.
### Observing the published contract (the 4th channel)

A BR service's published language — its integration events on a *named*,
GitOps-provisioned JetStream stream (`CHARTER`, `IDENTITY`, …) — is the fourth
e2e observation channel, alongside the mutation verdict, the query + affordances,
and the subscription. Three free functions on the `nats` feature observe and
reset it; **none lives on `TestNats`**, which owns only the harness's own
throwaway UUID-suffixed buckets, not a service-contract stream:

- **`await_integration_event(js, stream, subject, deadline)`** — opens an
  ephemeral pull consumer (`DeliverPolicy::All`, so an event published *before*
  the call is still caught) filtered to `subject`, awaits the first message
  within `deadline`, and returns the envelope as a `serde_json::Value`. Deserialize
  it against the **producer's** `contract-*` payload type to assert the wire shape.
  Returns `None` on a clean timeout — never hangs. The stream name is a parameter,
  so it is service-agnostic.
- **`recreate_stream(js, name, &subjects)`** / **`recreate_kv(js, bucket)`** —
  delete-then-create a named stream (with its subject filters, e.g. `charter.>`)
  or KV bucket, giving each serial scenario an empty bus on a shared server.
  Names are caller-supplied; the helpers carry zero service knowledge.

### The de-flake primitives

Three helpers exist specifically to kill the classic e2e flakes:

- **`WsSubscription::next_matching(predicate, timeout)`** — drain-until-match.
  A broadcast subscription is woken by *every* event in its class, so a socket
  opened mid-chain can receive an in-flight earlier-transition frame before the
  one the assertion wants. `next_matching` skips non-matching frames until the
  predicate holds, and on timeout reports every frame it skipped — so a *genuine*
  miss is still fully diagnosable. (`next_data` remains, for the single-push case.)
- **`SseSubscription::drain(max, timeout) -> usize`** — drain-to-quiescence.
  A scenario that has already asserted the push it cares about often needs to
  flush the *rest* of a known burst before opening the next leg, so a stale
  earlier-transition frame can't satisfy a later `expect_event`. `drain` pulls
  up to `max` events, stopping at the first that doesn't arrive within `timeout`
  (a clean stream end or genuine silence), and returns the count it drained. It
  reuses `next_event`, so a broken stream still panics — it never swallows an
  error frame. (`next_event` / `expect_event` / `expect_silence` remain, for the
  single-push and silence cases.)
- **`wait_until(timeout, predicate)`** — poll an async condition (a projection
  row landed, an integration event was published, a NATS consumer count moved)
  up to a bounded deadline, instead of sleeping a fixed amount and hoping.

### Boot classification — happy path *and* fail-loud

`SpawnedProcess` has two readiness paths against the service's own health URL,
both `http-client`-gated:

- **`wait_for_http_ok(url, timeout) -> Result<(), String>`** — the happy path. A
  non-`Ok` means "did not become ready", with the captured logs in the error
  string. Use it when a non-boot is a test failure.
- **`await_boot(url, timeout) -> BootOutcome`** — the fail-loud path. It
  *classifies* the boot into `BootOutcome::{ Ready, Exited(ExitStatus), TimedOut }`
  so a scenario can **assert** an outcome — including the deliberate crash of a
  service that refuses to start with its declared GitOps infra missing (the
  S0-style "fail loud, name the missing resource"). On `Exited` the captured pipes
  are drained to EOF before returning, so `proc.logs()` carries the full tail the
  binary printed before exiting — which is where the named missing resource lives.
  `BootOutcome::is_ready()` / `exit_status()` are the two convenience reads.

The process stays owned by the caller in every outcome (mirroring
`wait_for_http_ok`): pair the verdict with `proc.logs()`, and `shutdown()` (or
drop — `kill_on_drop`) reaps it.
### The verdict vocabulary (channel 1)

A BR mutation returns a **verdict, never state**: an `ack` on success, or a
structured error carrying a **stable code** (shaped `^[A-Z][A-Z0-9_]+$`,
≥2 characters, never English prose) on a rejection. The `verdict` module is the
assertion vocabulary for that
first observation channel — pure functions over the `serde_json::Value` a
`GraphqlClient` hands back, with **zero transport coupling** (they take a
response, not a client), so they work over any GraphQL response however obtained:

| Function | Verdict |
|---|---|
| `is_ack(&response) -> bool` | no top-level GraphQL `errors`, and no rejection code embedded under `data` |
| `expect_ack(&response, what)` | panics (with the response) unless the call acked |
| `mutation_error_code(&response) -> Option<String>` | the rejection code if rejected, else `None` — looks at the top-level error's `extensions.code`, then its `message`, then a `code` / `errorCode` / `reasonCode` under `data` |
| `expect_rejected(&response) -> String` | panics unless rejected; returns the code |
| `expect_code_shaped(&response, what) -> String` | `expect_rejected` **and** asserts the code matches the shape `^[A-Z][A-Z0-9_]+$` (so ≥2 chars — a single `"A"` is rejected); returns it |
| `is_code_shaped(&str) -> bool` | true iff the string matches the shape `^[A-Z][A-Z0-9_]+$` — uppercase-led, then `[A-Z0-9_]`, ≥2 chars — a stable code, not prose |

**The affordance-skip guarantee.** An affordance carries its own user-facing
`reasonCode` (why an action is blocked) — that is *not* a mutation rejection. The
private walker behind `mutation_error_code` therefore **skips any subtree under an
`affordances` key**, at any depth: a response whose payload acks but whose
affordances list a blocked action with a `reasonCode` reads as an **ack**, never a
rejection. When a payload-union rejection code and an affordance `reasonCode`
coexist, the **mutation code wins**. Without this, every affordance-aware service
would mis-read a blocked-affordance hint as a failed mutation.

## Why — the non-obvious bits

The source is comment-free by design (it is read by agents, for which comments
are token overhead). The intent that the signatures don't make obvious lives
here, synthetically:

| Thing | Why it is the way it is |
|---|---|
| `E2eDatabase` owner = `NOSUPERUSER` + a `bypassrls` flag | The owner mirrors the CNPG GitOps owner: an RLS-bypassing agent for migrations/bootstrap, `NOSUPERUSER` so `FORCE ROW LEVEL SECURITY` behaves exactly as in prod. Pass `bypassrls = false` **only** for negative tests asserting a misdeclared owner is caught — it reproduces incident **2026-06-11**, where a no-bypass owner under `FORCE RLS` read zero rows and a deploy cascade failed. |
| `owner_url()` is named `owner_migration_url()` | The owner carries `BYPASSRLS` and is the migration/bootstrap agent only. A service whose e2e `boot()` set `DATABASE_URL = owner_url()` ran the runtime as the RLS-bypassing owner → RLS silently off, and cross-tenant "isolation passes" proved nothing (masked two real production cross-tenant PII leaks, svc-identity 2026-06-15). The runtime DSN is `app_url()`; the longer owner name reads as obviously wrong at a runtime `DATABASE_URL` call site. |
| Role ensure / grant runs under `pg_advisory_xact_lock(hashtext(name))` | The ensure/grant statements (`ALTER ROLE`, `GRANT`, `ALTER DEFAULT PRIVILEGES`) are unconditional on every steady-state call and the role/ACL names are deliberately **shared and stable** across parallel binaries/worktrees. Two processes writing the same `pg_authid`/ACL row at once aborts the loser with `tuple concurrently updated` (`XX000`) — which the old `duplicate_object` (`42710`-only) guard missed and whose recovery re-collided. Each critical section now runs in a transaction that first takes an advisory lock keyed on the object name, serializing same-named callers on the server; the loop retries only on the concurrent-catalog race — an `XX000` whose message reports a tuple updated/deleted `concurrently` — plus deadlock (`40P01`) and lock-not-available (`55P03`), with bounded backoff. Any other `XX000` (a genuine internal error) fails immediately instead of being masked. The shared `CREATE/DROP DATABASE` (which cannot be transactional) uses a session-level `pg_advisory_lock`. The `ALTER` still resets the freshly-generated password, so `owner_migration_url()` authenticates even when it recovered a leaked role. |
| `E2eDatabase` has a `Drop` net that tears down on a dedicated thread | A test panicking before its explicit `cleanup`/`teardown` would otherwise leak the per-test DB + owner. `Drop` is synchronous and teardown is async, so the net spawns a dedicated OS thread with its own current-thread runtime and `join`s it — never touching the test's runtime (which may be current-thread, where `block_in_place`/`Handle::block_on` panics). Explicit `cleanup` is still the primary path and sets the `torn_down` flag so the net is a no-op after it; the net is best-effort (warns, never panics) so a teardown hiccup can't mask the original test failure. |
| `create*`'s `managed_roles` silently skips unknown roles | Only roles that **already exist** are granted to the owner `WITH ADMIN OPTION` (PG16: a `CREATEROLE` role needs `ADMIN` on a role to `ALTER` it). Roles the service creates itself at startup are deliberately left alone — this is not a typo guard. |
| `with_app_role` ensures-not-creates, and `cleanup` never drops the app role | The app role is a cluster-global object that a **shared name** (`svc_app`) leaves in use across parallel worktrees — so it is ensured idempotently and under the advisory lock above (so a true concurrent ensure serializes rather than racing), `ALTER`ed in place if present, and **left standing** on cleanup. Only the per-test DB + the per-test (unique-suffixed) owner are dropped. Same posture as `managed_roles`: a shared role is the cluster's, not one test's to delete. |
| Interpolated identifiers go through `quote_ident` / literals through `quote_literal` | The public `create_named` / `create_with_app_role` / `with_app_role` surface takes role and DB **names** as arbitrary `&str`. Names were previously interpolated raw into DDL (only the password was escaped); a `"`-bearing name broke the statement (and the `rolname = '…'` existence check). A single quote helper escapes every interpolated name and literal so the DDL is well-formed whatever the caller passes. |
| The owner-grant dance covers TABLES + SEQUENCES, not FUNCTIONS | `ALTER DEFAULT PRIVILEGES … GRANT … ON TABLES / ON SEQUENCES` makes the owner's later-migrated tables and sequences reachable by the app role — the universal projection-store needs. It matches the charter backend's own provisioning (`svc-charter/tests/common/infra/pg.rs`), which grants the same two and no `EXECUTE ON FUNCTIONS`. A service that exposes owner-created functions to the runtime role grants `EXECUTE` itself, in its own migrations — the harness does not assume that surface. |
| `TestNats` always creates **two** KV buckets | The second (`bearer_kv`) stands in for the shared `bearer_tokens` bucket `svc-auth` reads for PAT lookup; inject it wherever production expects that bucket. |
| `SpawnedNats` *vs* `TestNats` | A binary that **hardcodes** its bucket names can't be isolated by per-bucket names → give it its own server (`SpawnedNats`, which lets `nats-server` self-assign its port — race-free under parallel `cargo test`). Tests using the harness's own **suffixed** buckets share one server (`TestNats`). |
| `recreate_*` delete-then-create, not get-or-create | The harness is the **GitOps stand-in**: it provisions a named service-contract stream/bucket the service expects to already exist (the service itself never does — the lib never auto-provisions, it fails loud). Delete-then-create, never silent get-or-create, so each serial scenario starts from a truly empty bus — a get-or-create would leak the prior scenario's messages and the reset would pass while doing nothing. Delete-then-create over a **shared NATS server** is a cross-process TOCTOU, so the pair retries over a bounded loop to absorb a concurrent recreate; clean isolation across processes still wants a per-process `SpawnedNats` (its own server), which the copy-me template uses. |
| `await_integration_event` returns `Option`, not `Result` | A clean timeout (no matching message before the deadline) and a missing/unreadable stream both collapse to `None`: the caller's assertion is *"the event arrived"* / *"no event arrived"*, and `expect(...)` / `is_none()` reads better than threading an error. It can therefore back an `expect_silence`-style negative without a broker error masquerading as success — it only ever yields `Some` on a real, decodable envelope. |
| Subscriptions fail loud on a broken stream | A GraphQL `errors` payload, a transport error, or an `error` frame is a hard failure (SSE panics, WS returns `Err`) — never a frame to skip. `SseSubscription::next_event` returns `None` **only** for a genuine timeout or a clean stream end, so `expect_silence` can't be fooled into passing on a stream that actually broke. |
| `TestServer::spawn` readiness is best-effort | It polls `GET /` and treats **any** HTTP response — including a 404 — as "up": that proves the in-process server is serving, it is not a dependency-readiness gate, and after ~500 ms it returns anyway (the first real request surfaces a genuine failure). For real readiness against a spawned *binary*, use `SpawnedProcess::wait_for_http_ok` against the service's own health path. |
| `SpawnedProcess` has both `wait_for_http_ok` and `await_boot` | A nominal scenario wants "ready or fail the test" (`wait_for_http_ok` → `Result`); a fail-loud scenario wants to **assert** the boot *did not* succeed and inspect *why* (`await_boot` → `BootOutcome::{Ready, Exited(status), TimedOut}`). Both are kept — the classifier adds the three-outcome verdict, it does not replace the happy-path checker. |
| `await_boot` drains the pipes to EOF before returning `Exited` | The drain runs as background tasks, so `try_wait()` can observe the exit before those tasks have read the binary's final stderr/stdout. Awaiting them on `Exited` guarantees `proc.logs()` holds the full tail — the line naming the missing declared resource an S0-style scenario asserts on. The continuous-drain model means no kill-before-drain dance (the prod-side reference used a sync `std::process` and had to). |
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
| `nats` | `TestNats`, `await_integration_event`, `recreate_stream`, `recreate_kv` | `async-nats`, `futures-util` |
| `spawned-nats` | `SpawnedNats` | `tempfile` |
| `e2e-db` | `E2eDatabase` | `sqlx` |
| `server` | `TestServer` | `axum`, `reqwest` |
| `passport` | `PassportBuilder` | `br-core-auth` |
| `graphql` | `GraphqlClient`, `verdict::*` | `reqwest` (+ `passport`) |
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
br-test-harness = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v0.6.0" }

# …or slim — only part of the toolbox, no `sqlx`/`axum`/`rsa` in your build:
br-test-harness = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v0.6.0", default-features = false, features = ["nats", "spawned-nats"] }
```

With the default `full` feature, `br-test-harness` depends on `br-core-auth`
(its `test-support` feature, which ships `PassportBuilder`) pinned to
`br-rust-common` `tag = "v0.11.1"`. A slim build that omits the
passport-bearing features drops the dependency. If your service already pins
`br-rust-common`, keep both on the **same ref** so Cargo resolves a single
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

The load-bearing real-infra paths — `SpawnedNats` (spawns a real `nats-server`
and reads back its bound port), the `nats_assert` helpers (await an event on a
reset named stream / KV bucket), and `E2eDatabase` (provisions the ephemeral
owner role and the transaction-local RLS context) — have focused self-tests in
`tests/`. They are **`#[ignore]`-gated** so the default `cargo test` stays green
without infra; run them explicitly:

```sh
# SpawnedNats path — needs `nats-server` on PATH (it spawns its OWN server):
cargo test -p br-test-harness --test spawned_nats -- --ignored

# nats_assert path — needs `nats-server` on PATH (each test spawns its OWN server):
cargo test -p br-test-harness --test nats_assert -- --ignored

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
real binary with `SpawnedProcess` (it migrates as the owner, then serves as the
RLS-subject app role), then drive it over HTTP / WS and assert against the DB + NATS:

```rust,ignore
use std::time::Duration;
use br_test_harness::{E2eDatabase, SpawnedNats, SpawnedProcess, WsSubscription, PassportBuilder};

let db   = E2eDatabase::create_with_app_role(
    "identity_owner", "identity", "identity_app", "app_pw", true, &[],
).await;
let nats = SpawnedNats::start().await;
let mut svc = SpawnedProcess::spawn(
    env!("CARGO_BIN_EXE_my-service"),
    &["serve"],
    &[
        ("DATABASE_URL", &db.app_url()),                  // runtime: RLS-subject, never the owner
        ("DATABASE_URL_OWNER", &db.owner_migration_url()), // migrations: BYPASSRLS owner
        ("NATS_URL", &nats.url()),
    ],
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

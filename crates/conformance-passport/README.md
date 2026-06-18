# conformance-passport

A **black-box conformance runner** for the BotResources **bearer/PAT → Passport
resolution** contract (conformance group **G1**) — the `GET /internal/passport`
endpoint the GraphQL gateway calls before every authenticated request. It drives
the frozen Go test subject in `conformance-subjects/identity-passport` as a black
box — it never reads the subject's source — and asserts that the subject resolves
a bearer credential into a `Passport` exactly as the platform requires.

> ⚠️ **TEST FIXTURE ONLY.** Add it as a **dev-dependency**, never a runtime one.
> It builds and spawns a real binary, stands up a real `nats-server`, seeds a real
> `bearer_tokens` KV bucket, and calls the subject's HTTP surface on an isolated,
> throwaway test network.

## The frozen contract (authoritative)

`GET /internal/passport`:

- `Authorization: Bearer <token>` where `bearer_token_key(<token>)` exists in the
  JetStream KV bucket `bearer_tokens` → **200** with response header `X-Passport`
  = base64 of a `br_core_auth::Passport`.
- token absent / revoked, no `Authorization`, or a non-Bearer credential →
  **200, no `X-Passport`** (anonymous).
- a backend (KV) error → **500**.

The endpoint **resolves**, it does not **gate**: an unresolvable credential is an
anonymous request, never a 401 — services do authZ, never authN.

## Oracle — the real `br-rust-common` types, never hand-rolled

The runner seeds and decodes **using the real `br-core-auth` types** (branch
`v1.0.1`, with `test-support` for the G4 builder; the release flips it to
`tag = "v1.0.1"`), never hand-rolled JSON, never a re-stated wire shape:

- **Seeding** uses the real `bearer_token_key(raw)` for the KV key and
  serializes the real `BearerTokenEntry { email, token_id }` for the value. The
  subject independently recomputes the same key (its own Go SHA-256) and parses the
  same entry shape.
- **Decoding** the returned `X-Passport` is `PassportHeader::from_header(&str)`
  into the real `br_core_auth::Passport`. **Deserialization succeeding *is* the
  wire-shape check** — a malformed header is the assertion failing. There is no
  hand-written shape guard to drift from the types; `Passport` is
  `#[serde(deny_unknown_fields)]` so an extra field fails the decode.
- The resolved passport is asserted against the **seeded** values: `Passport::Human`,
  `auth_method == AuthMethod::Pat { token_id }` with the seeded `token_id`,
  `claims.email` == the seeded email. `user_id` is asserted **present and valid**
  (not the nil UUID), never a specific value — it is a subject-side stand-in (see
  the Why table).

> **The Go subject is a frozen, independent anchor of the external wire.** It is a
> faithful reimplementation of the key derivation, the entry parse and the Passport
> envelope in Go — not a binding to the Rust lib. When `br-rust-common` evolves, this
> crate bumps its lib pin and re-runs against the unchanged Go subject: **green means
> the external envelope didn't move**; **red is a real break** (to be fixed, or
> acknowledged as a deliberate major bump and a coordinated subject update).

## The conformance battery (P1–P5, G4)

P1–P5 are `#[ignore]`-gated real-infra tests (`tests/conformance.rs`); **G4** is a
pure round-trip through the lib types (a unit test in `src/scopes.rs`, no infra). G4
runs via `cargo test` only — it is an offline lib round-trip, never a black-box
spawned subject, so it is **not** selectable on the `--scenario` CLI runner
(`Scenario::from_code("g4")` returns `None` by design; only P1–P5 are spawn scenarios):

| Id | Asserts |
|---|---|
| **P1** | A seeded valid bearer resolves to **200 + `X-Passport`**, decoding to a `Passport::Human` whose `auth_method` is `Pat { token_id }` matching the seeded `token_id`, whose `claims.email` equals the seeded email, and whose `user_id` is a present, valid `Uuid`. |
| **P2** | A bearer seeded then **revoked** (KV key deleted) resolves to **200, no `X-Passport`** (anonymous). |
| **P3** | A bearer that was **never seeded** resolves to **200, no `X-Passport`** (anonymous). |
| **P4** | A request with **no `Authorization`** header resolves to **200, no `X-Passport`** (anonymous). |
| **P5** | Two distinct seeded entries (distinct email + token_id) resolve, **each to its own passport** — token_id and email match respectively, with no cross-talk. |
| **G4** | A `Passport` carrying a `scopes` claim survives the `X-Passport` base64 round-trip (`to_header` → `from_header`) **identically**, and the v1.0.0 typed-scopes API holds: `scopes()` yields the granted `ScopeKey`s, `has_scope` is true for a granted scope and false for an ungranted one, an absent claim yields no scopes, and a malformed entry is skipped while valid ones survive. |

The non-tautological property **P1 + P5** prove: the independent subject agrees with
the lib's `bearer_token_key` derivation and `BearerTokenEntry` shape. If the subject's
key derivation or entry parse diverged from the lib, the seeded key would never be
found → the bearer would resolve to anonymous → the battery goes red. That is the
backward-compat gate.

## Runner mechanics (seed via the lib, then drive the endpoint)

Per-test isolation: each test spawns its **own** `SpawnedNats`. The subject fails
loud if the bucket is missing, so the boot order is fixed:

1. `SpawnedNats::start()`.
2. Create the `bearer_tokens` KV bucket on it — **before** spawning the subject.
3. Spawn the subject with `NATS_URL`, `HTTP_ADDR=127.0.0.1:<free port>`,
   `BEARER_BUCKET=bearer_tokens`.
4. `wait_until` `/readyz` == 200.
5. Run the scenarios, then shut down the subject + NATS.

Seeding a token generates a raw `brk_<uuidv7>`, an email + `token_id = Uuid::now_v7()`,
and writes `serde_json::to_vec(&BearerTokenEntry { email, token_id })` at
`bearer_token_key(raw)`. Revoking deletes that key. Emails and tokens are
**namespaced per run** with a UUIDv7.

## Public helper surface

- `PassportHarness` — `start()` builds the Go subject (`build_subject`) and is the
  self-test entry point; `start_with_binary(binary)` runs the **same** battery
  against any passport-resolution binary, so a consuming Identity service drives it
  against its own. It spawns a dedicated `SpawnedNats` and creates the
  `bearer_tokens` bucket up front (the platform never auto-provisions, so the runner
  must). `seeder()` hands back a `BearerSeeder`.
- `BearerSeeder` — `seed(namespace, label)` / `revoke(token)` over the real
  `bearer_token_key` + `BearerTokenEntry`.
- `PassportEndpoint` — `resolve_bearer(raw)` / `resolve_anonymous()` driving
  `GET /internal/passport`; decodes `X-Passport` via the real `PassportHeader`.
- `Scenario` (P1–P5) with parsing/defaults.
- `Subject` / `SubjectConfig` — spawn the built binary with its env wiring
  (`NATS_URL`, `HTTP_ADDR`, `BEARER_BUCKET`) and poll `/readyz` / `/livez`.
- `run_spawn(SpawnTarget { binary }, scenarios, timeout)` — the core deliverable:
  stands up a throwaway `SpawnedNats` + the `bearer_tokens` bucket, launches the
  subject, waits for `/readyz=200`, and runs the full `p1..p5`. Needs `nats-server`
  on `PATH`. An **attach** runner (drive a live service's NATS + `/readyz`) is a
  future addition; G1 ships spawn only.

## Running it

Needs `nats-server` and the **Go toolchain** on `PATH` (the runner builds the
subject). The tests are `#[ignore]`-gated so the default `cargo test` stays green
without infra:

```sh
cargo test -p conformance-passport --test conformance -- --ignored
```

CI runs it in the `infra-e2e` job, which already has `go` and `nats-server` on the
runner (shared with G2/G3).

## Install

A **dev-dependency**, pinned to a release tag (git-tag distribution; no crates.io).
Keep its `br-rust-common` pin identical to `br-test-harness`'s (`v1.0.1`) so Cargo
resolves a single source and never duplicates `br-core-*`:

```toml
[dev-dependencies]
conformance-passport = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v1.0.0" }
```

## Why — the non-obvious bits

| Thing | Why it is the way it is |
|---|---|
| Oracle = the **real `br_core_auth` types** (`bearer_token_key`, `BearerTokenEntry`, `PassportHeader`, `Passport`, `AuthMethod`), never hand-rolled | The crate exists to detect drift between the platform contract and an independent subject. Seeding and decoding through the real lib means the only thing being conformance-checked is the *subject*; a hand-rolled key/entry/passport shape would be a second contract that drifts from the types — exactly what this crate prevents. |
| **P1 + P5 are the round-trip gate**: seed via the lib's `bearer_token_key` + `BearerTokenEntry`, independent subject must agree | The subject recomputes the KV key with its own Go SHA-256 and parses the entry with its own Go struct. If either diverges from the lib, the seeded key is never found and the bearer resolves to **anonymous** → red. That divergence-detection is the backward-compat property; a tautological "lib calls lib" check would prove nothing. |
| Revoked / unknown / absent credential → **200 anonymous**, never **401** | The endpoint **resolves**, it does not **gate**. An unresolvable credential yields an anonymous request that downstream authZ then refuses. A 401 here would conflate resolution with authorization and break the gateway's anonymous-passthrough path — services do authZ, never authN. |
| `claims.email` is asserted, but `user_id` is **not value-asserted** | The `BearerTokenEntry` carries only `email` + `token_id`; the subject has no database and synthesizes `user_id` as a deterministic stand-in (the real service loads it from Postgres). The battery asserts `user_id` is present and a valid non-nil `Uuid`, not a specific value, so it stays faithful to the contract without coupling to the stand-in. |
| `claims` is checked for `email` only, no project keys | `claims` is a per-project **seam** (org_id, scopes, is_admin are consumer deltas, not the platform contract). The anchor freezes only the generic envelope and the one generic claim the entry carries; asserting a fixed claim set would bake a project policy into the platform gate. |
| The bucket is created by the **runner**, before the subject is spawned | Mirrors the platform's fail-loud, never-auto-provision doctrine: the subject *binds* the bucket and refuses readiness if it is absent. The runner owns setup, so the boot order is bucket → subject → readiness → scenarios. |
| Per-test `SpawnedNats` (broker isolation) | spawn mode is isolated anyway; a dedicated throwaway broker per test chain keeps the hardcoded `bearer_tokens` bucket name from colliding across concurrent tests, and emails/tokens are namespaced per run with a UUIDv7 on top. |
| The whole battery is `#[ignore]`-gated | It drives real infra (`nats-server` + `go` + a spawned binary); the default `cargo test` must stay green on a machine without them, exactly like `br-test-harness`'s own self-tests. |
| Attach runner is not implemented (spawn only) | G1's contract is a stateless HTTP resolution over a seeded bucket; the value is the spawn round-trip against the frozen subject. An attach runner against a live `svc-identity` (driving its NATS + `/readyz`, seeding its bucket) is a future addition when a real subject exists to attach to. |

## License

Apache-2.0. MSRV **1.88** (edition 2024).

# conformance-passport

A **black-box conformance runner** for the BotResources **bearer/PAT → Passport
resolution** contract (conformance group **G1**) — the `GET /internal/passport`
endpoint the GraphQL gateway calls before every authenticated request. It drives
the frozen Go test subject in `conformance-subjects/identity-passport` as a black
box — it never reads the subject's source — and asserts that the subject resolves
a **sealed** bearer credential into a `Passport` exactly as the platform requires.

> ⚠️ **TEST FIXTURE ONLY.** Add it as a **dev-dependency**, never a runtime one.
> It builds and spawns a real binary, stands up a real `nats-server`, seeds a real
> sealed entry into the `PUBLISHED_LANGUAGE` KV bucket, and calls the subject's
> HTTP surface on an isolated, throwaway test network.

## The frozen contract (authoritative)

`GET /internal/passport`:

- `Authorization: Bearer <token>` whose `bearer_token_kv_key(<token>)` =
  `"identity/bearer_tokens/" + sha256hex(<token>)` exists in the JetStream KV
  bucket `PUBLISHED_LANGUAGE` → **200** with response header `X-Passport` =
  base64-std of a `br_core_auth::Passport`.
- token absent / revoked, no `Authorization`, a non-Bearer credential, an
  **unreadable envelope**, a **wrong seal key**, or a **tampered ciphertext** →
  **200, no `X-Passport`** (anonymous, fail-closed).
- a backend (KV) error → **500**.

The endpoint **resolves**, it does not **gate**: an unresolvable or non-openable
credential is an anonymous request, never a 401 — services do authZ, never authN.
Only genuine infra (a KV read failure) is a 500.

## The sealed wire

The KV value is a `br_auth_contract::SealedBearer { nonce, ciphertext }` (both
base64-std, `deny_unknown_fields`) — a **ChaCha20-Poly1305** AEAD envelope
(RFC 8439, 12-byte random nonce carried in the envelope, 16-byte tag). The
**AAD is the unprefixed SHA-256 digest** of the raw token
(`br_core_auth::bearer_token_key`), not the full KV key. The sealed cleartext is
`br_auth_contract::BearerEntry { actor, token_id }` — it carries **no email**.

The resolved `Passport::Human` therefore has `auth_method = Pat { token_id }`,
`user_id` = the `UserId` inside the sealed `Actor::Human(..)`, and **no `email`
claim**. The contract guarantee on `claims` is **email-absent** (the migration's
PII-removal property — the retired model put `email` in claims, the sealed model
must not), **not** fully-empty claims: a real subject MAY carry other claims (e.g.
`scopes`), so the battery asserts `claims.get("email").is_none()`, never `{}`. The
Go anchor emits `{}` only because it has no scopes source.

## Oracle — the real lib, never hand-rolled

The runner seeds and decodes **using the real types**, never a hand-restated wire
shape:

- **Seeding** uses `br_auth_identity_util::BearerPublisher` (the identity-side
  producer kit) to seal a `BearerEntry` and write it at `bearer_token_kv_key`,
  through the existing `br-test-harness` `with_published_language` PL seam — the
  in-crate `SealedSeeder`. The subject independently recomputes the same KV key and
  **opens** the envelope with its own Go AEAD.
- **Decoding** the returned `X-Passport` is `PassportHeader::from_header(&str)`
  into the real `br_core_auth::Passport`. Deserialization succeeding **is** the
  wire-shape check — `Passport` is `#[serde(deny_unknown_fields)]`, so a stale
  decode fails. There is no hand-written shape guard to drift from the types.

> **The Go subject is a frozen, independent anchor of the external wire — and it
> OPENS the Rust-sealed envelope.** It re-implements the KV-key/AAD derivation, the
> envelope parse, and the ChaCha20-Poly1305 **open** in Go, never importing the
> Rust lib. Seeding through the real Rust seal + the frozen Go open is genuine
> cross-language interop: it pins the whole crypto contract — key handling,
> AAD = unprefixed digest, cipher / nonce / tag, envelope JSON, `BearerEntry`
> shape, and `Passport` shape. The **random per-seal nonce** means the ciphertext
> bytes are not frozen; the crypto **contract** is. When `br-rust-common` /
> `svc-auth` evolve, this crate bumps its pins and re-runs against the unchanged Go
> subject: **green means the external envelope didn't move**; **red is a real
> break**.

## The conformance battery (P1–P8, G4)

P1–P8 are `#[ignore]`-gated real-infra tests (`tests/conformance.rs`); **G4** is a
pure round-trip through the lib types (a unit test in `src/scopes.rs`, no infra).
G4 runs via `cargo test` only — it is an offline lib round-trip, never a black-box
spawned subject, so it is **not** selectable on the spawn runner
(`Scenario::from_code("g4")` returns `None` by design; only P1–P8 are spawn
scenarios):

| Id | Asserts |
|---|---|
| **P1** | A seeded sealed bearer resolves to **200 + `X-Passport`**, decoding to a `Passport::Human` whose `auth_method` is `Pat { token_id }` matching the sealed `token_id`, whose `user_id` equals the **exact** sealed `Actor::Human` UserId, and whose `claims` carries **no email**. Resolved twice → deterministic. |
| **P2** | A bearer seeded then **revoked** (`delete_bearer`) resolves to **200, no `X-Passport`**. |
| **P3** | A bearer that was **never seeded** resolves to **200, no `X-Passport`**. |
| **P4** | A request with **no `Authorization`** resolves to **200, no `X-Passport`**. |
| **P5** | Two distinct sealed entries (distinct `user_id` + `token_id`) resolve, **each to its own passport**, no cross-talk. |
| **P6** | A bearer sealed under a **WRONG key** (a second `BearerPublisher` with a different 32-byte key) → the subject (correct key) AEAD-open fails → **anonymous**, never a wrong identity. |
| **P7** | A correctly-sealed bearer whose stored ciphertext is then **byte-flipped** (`pl_get_raw` → flip → `pl_put_raw`) → the AEAD tag fails → **anonymous**. |
| **P8** | With the `PUBLISHED_LANGUAGE` bucket **destroyed** under the live subject, resolution returns **500**, never silently anonymous. Destructive → `run_spawn` always runs it **last**. |
| **G4** | A `Passport` carrying a `scopes` claim survives the `X-Passport` base64 round-trip identically, and the typed-scopes API holds (`scopes()` / `has_scope` / absent = empty / malformed skipped). |

The non-tautological property **P1 + P5** prove: the independent subject opens what
the real lib sealed. If the subject's key derivation, AAD, cipher, or entry parse
diverged, the open would fail → anonymous → the battery goes red. That is the
backward-compat / interop gate.

`ConformanceReport::is_conformant()` means **"every check that ran passed, and at
least one ran"** — zero failures, zero skips, and a **non-empty** report. A gate
that greened on a skipped check (unproven, not passed) or on an empty report (zero
checks self-certifying) would be a false-green; both are closed. `run_spawn` itself
rejects an empty scenario set up front (`InvalidInput`) rather than stand up a
subject to assert nothing.

## Consuming service integration

A service exposing `GET /internal/passport` drives the battery against its own
binary. Pass `&ALL` and pin **full coverage** so a copy-paste consumer cannot
under-run the gate:

```rust
use conformance_passport::{ALL, DEFAULT_TIMEOUT, SpawnTarget, run_spawn};

let report = run_spawn(&SpawnTarget { binary: my_binary }, &ALL, DEFAULT_TIMEOUT)
    .await
    .expect("conformance run failed to start");
assert!(report.is_conformant(), "G1 failures: {:#?}", report.outcomes);
assert_eq!(report.passed(), ALL.len(), "every G1 spawn check must have run and passed");
```

`is_conformant()` proves no check failed or was skipped; the `passed() == ALL.len()`
assertion additionally proves the full set actually ran (the belt-and-braces against
a filtered-down selection). G4 is offline (`g4_scopes_claim_round_trips_through_the_header`)
and is asserted separately, not via the report.

## Subject env contract

`run_spawn` (and `Subject::spawn`) wire the subject with exactly:

| Var | Meaning |
|---|---|
| `NATS_URL` | the throwaway broker URL |
| `PORT` | the loopback port to bind; `base_url = http://127.0.0.1:$PORT` |
| `BEARER_SEAL_KEY` | base64-std of the fixed 32-byte seal key |

The KV bucket is the fixed `PUBLISHED_LANGUAGE` (not an env var). Readiness is
`/readyz`. This matches the gateway `examples/svc-identity` exactly, so the same
runner drives the Go anchor and a real reference with zero hand-driving.

## Runner mechanics

Per-test isolation: each test spawns its **own** `FabricTestNats`. The subject
fails loud if the bucket is missing, so the boot order is fixed:

1. `FabricTestNats::start()` (its own `nats-server`).
2. Provision the `PUBLISHED_LANGUAGE` bucket in-process via
   `with_published_language()` (no external binary, so the gate is a zero-ceremony
   drop-in) **before** the subject, which also binds it for seeding.
3. Spawn the subject with `NATS_URL` / `PORT` / `BEARER_SEAL_KEY`.
4. `wait_until` `/readyz` == 200.
5. Run the scenarios (P8 last), then shut down.

## Running it

Needs `nats-server` and the **Go toolchain** on `PATH` (the runner builds the
subject and runs its `make guard`). No external binary is required — the bucket is
provisioned in-process. The tests are `#[ignore]`-gated so the default `cargo test`
stays green:

```sh
# offline (G4 + unit tests)
cargo test -p conformance-passport

# full P1..P8 battery against the Go anchor (real infra)
cargo test -p conformance-passport --test conformance -- --ignored --test-threads=1
```

## Install

A **dev-dependency**, pinned to a release tag. Keep its `br-rust-common` pin
identical to `br-test-harness`'s (`v1.0.2`) so Cargo resolves a single source of
`br-core-*`:

```toml
[dev-dependencies]
conformance-passport = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v1.0.3" }
```

## License

Apache-2.0. MSRV **1.88** (edition 2024).

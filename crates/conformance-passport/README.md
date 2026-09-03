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

> **It depends on no BotResources code but `br-rust-common` and the sibling
> `br-test-harness`.** The sealed wire is frozen by the Go anchor, never by a Rust
> contract crate — so Rust and the wire cannot evolve together silently.

## The frozen contract (authoritative)

`GET /internal/passport`:

- `Authorization: Bearer <token>` whose `bearer_token_kv_key(<token>)` =
  `"identity/bearer_tokens/" + sha256hex(<token>)` exists in the JetStream KV
  bucket `PUBLISHED_LANGUAGE` → **200** with response header `X-Passport` =
  base64-std of a `br_core_auth::Passport`.
- token absent / revoked, no `Authorization`, a non-Bearer credential, an
  **unreadable envelope**, a **wrong seal key**, or a **tampered ciphertext** →
  **200, no `X-Passport`** (anonymous, fail-closed).
- a backend (KV) error → it **fails loud** (a **5xx**, or the resolver becomes
  unreachable), never a silent 200.

The endpoint **resolves**, it does not **gate**: an unresolvable or non-openable
credential is an anonymous request, never a 401 — services do authZ, never authN.
Catastrophic infra loss (the KV bucket/stream gone) is the one thing that must
**not** resolve to a silent 200: it surfaces loudly as a 5xx or by the resolver
dropping its connection / exiting (fail-loud).

## The sealed wire

The KV value is `{"nonce":"…","ciphertext":"…"}` (both base64-std, unknown fields
rejected) — a **ChaCha20-Poly1305** AEAD envelope (RFC 8439, 12-byte random nonce
carried in the envelope, 16-byte tag). The **AAD is the unprefixed SHA-256 digest**
of the raw token (`br_core_auth::bearer_token_key`), not the full KV key. The
sealed cleartext is `{"actor":{"kind":"human"|"service","id":"<uuid>"},"token_id":"<uuid>"}`
— it carries **no email**.

That wire is frozen in **Go**, in `conformance-subjects/identity-passport`
(`wire.go` + `seal.go`, pinned by `wire_test.go` + `seal_test.go`): a fixed-nonce
vector reproduces a frozen ciphertext, and further vectors freeze the exact
cleartext bytes and the exact envelope bytes. Rust ships no copy of it.

The resolved `Passport::Human` therefore has `auth_method = Pat { token_id }`,
`user_id` = the `UserId` inside the sealed `Actor::Human(..)`, and **no `email`
claim**. The contract guarantee on `claims` is **email-absent** (the migration's
PII-removal property — the retired model put `email` in claims, the sealed model
must not), **not** fully-empty claims: a real subject MAY carry other claims (e.g.
`scopes`), so the battery asserts `claims.get("email").is_none()`, never `{}`. The
Go anchor emits `{}` only because it has no scopes source.

## Oracle — the Go anchor freezes the wire, the lib decodes the Passport

Two roles, never conflated:

- **Seeding goes through the Go anchor.** `SealedSeeder` spawns the very binary
  under test in its one-shot `seal` mode, which prints the KV key and the exact
  bytes to store; the runner writes them **verbatim** through the `br-test-harness`
  `pl_put_raw` seam. Rust never builds, parses, or mutates the envelope — the
  adversarial seeds (wrong key, tampered ciphertext, unreadable envelope) are
  **anchor flags** (`--tamper`, `--unreadable`), not Rust byte-flipping. The seal
  side therefore cannot drift *with* the Rust lib: it is one Go implementation,
  frozen by its own vectors, sealing and opening.
- **Decoding** the returned `X-Passport` is `PassportHeader::from_header(&str)`
  into the real `br_core_auth::Passport`. Deserialization succeeding **is** the
  wire-shape check — `Passport` is `#[serde(deny_unknown_fields)]`, so a stale
  decode fails. There is no hand-written shape guard to drift from the types.

One lib-oracle cross-check survives on the seeding path: every KV key the anchor
emits must end in `br_core_auth::bearer_token_key(<token>)`, so the digest
derivation stays pinned to the real lib and a bare digest with no prefix is
rejected. The prefix itself is the anchor's.

> **The Rust-side interop guard belongs where the Rust seal lives.** Opening a
> Go-sealed vector through `br_auth_contract::open` proves Rust↔Go interop, and it
> is `svc-auth`'s test to own, next to the crate it guards. This battery is the
> **external-wire** gate: it depends on no service, so a `br-rust-common` bump
> never drags a consumer's tag along.

## The conformance battery (P1–P9, G4)

P1–P9 are `#[ignore]`-gated real-infra tests (`tests/conformance.rs`); **G4** is a
pure round-trip through the lib types (a unit test in `src/scopes.rs`, no infra).
G4 runs via `cargo test` only — it is an offline lib round-trip, never a black-box
spawned subject, so it is **not** selectable on the spawn runner
(`Scenario::from_code("g4")` returns `None` by design; only P1–P9 are spawn
scenarios). The codes are identifiers, not an order: **P8 is destructive and always
runs last**, whatever its number:

| Id | Asserts |
|---|---|
| **P1** | A seeded sealed bearer resolves to **200 + `X-Passport`**, decoding to a `Passport::Human` whose `auth_method` is `Pat { token_id }` matching the sealed `token_id`, whose `user_id` equals the **exact** sealed `Actor::Human` UserId, and whose `claims` carries **no email**. Resolved twice → deterministic. |
| **P2** | A bearer seeded then **revoked** (the KV key retracted) resolves to **200, no `X-Passport`**. |
| **P3** | A bearer that was **never seeded** resolves to **200, no `X-Passport`**. |
| **P4** | A request with **no `Authorization`** resolves to **200, no `X-Passport`**. |
| **P5** | Two distinct sealed entries (distinct `user_id` + `token_id`) resolve, **each to its own passport**, no cross-talk. |
| **P6** | A bearer sealed under a **WRONG key** (the anchor invoked with a different 32-byte `--key`) → the subject (correct key) AEAD-open fails → **anonymous**, never a wrong identity. The value is asserted **present** in the bucket first, so the anonymity is an open failure and not a missing key. |
| **P7** | A bearer that **resolved**, then had its stored ciphertext **byte-flipped** at the same key (anchor `--tamper ciphertext`) → the AEAD tag fails → **anonymous**. The before/after at one key is what makes the corruption the only difference. |
| **P9** | A bearer that **resolved**, then had its stored envelope replaced by one carrying an **unknown field** (anchor `--unreadable`) → the parse fails **before** the AEAD → **anonymous**. The replacement is a genuine, openable seal plus the extra field, so only the strict parse can reject it. |
| **P8** | With the `PUBLISHED_LANGUAGE` bucket **destroyed** under the live subject, resolution **fails loud** — a **5xx** *or* the resolver becoming **unreachable** (transport error) — never a silent **200** (anonymous or resolved). A pre-deletion health guard first confirms the subject resolves the seed, so a later unreachability is attributable to the infra loss alone. Destructive → `run_spawn` always runs it **last**. |
| **G4** | A `Passport` carrying a `scopes` claim survives the `X-Passport` base64 round-trip identically, and the typed-scopes API holds (`scopes()` / `has_scope` / absent = empty / malformed skipped). |

**What P1–P9 prove, and what they do not.** They prove the *endpoint* behaviour —
resolution, determinism, no cross-talk, and fail-closed on every unopenable input —
against a subject that is a black box to the runner. They do **not** prove
Rust↔Go crypto interop any more: seal and open are the same frozen Go
implementation, and that is deliberate (the wire must not move with the Rust lib).
Interop is pinned instead by the anchor's own frozen vectors and by the
`br-auth-contract` guard in `svc-auth`.

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

The same binary is also invoked one-shot as `<binary> seal …` to render each seed
(one JSON line: `kv_key` + `value_b64`), so a consuming service's binary must carry
that subcommand to be driven by this battery.

The KV bucket is the fixed `PUBLISHED_LANGUAGE` (not an env var). Readiness is
`/readyz`. This matches the gateway `examples/svc-identity` exactly, so the same
runner drives the Go anchor and a real reference with zero hand-driving.

## Runner mechanics

Per-test isolation: each test spawns its **own** `FabricTestNats`. The subject
fails loud if the bucket is missing, so the boot order is fixed:

1. `FabricTestNats::start()` (its own `nats-server`).
2. Provision the `PUBLISHED_LANGUAGE` bucket in-process via
   `with_published_language()` (no external binary, so the gate is a zero-ceremony
   drop-in) **before** the subject.
3. Spawn the subject with `NATS_URL` / `PORT` / `BEARER_SEAL_KEY`.
4. `wait_until` `/readyz` == 200.
5. Run the scenarios (P8 last), each seeding through a one-shot `seal` invocation
   of the same binary, then shut down.

## Running it

Needs `nats-server` and the **Go toolchain** on `PATH` (the runner builds the
subject and runs its `make guard`). No external binary is required — the bucket is
provisioned in-process. The tests are `#[ignore]`-gated so the default `cargo test`
stays green:

```sh
# offline (G4 + unit tests)
cargo test -p conformance-passport

# full P1..P9 battery against the Go anchor (real infra)
cargo test -p conformance-passport --test conformance -- --ignored --test-threads=1
```

## Install

A **dev-dependency**, pinned to a release tag. Keep its `br-rust-common` pin
identical to `br-test-harness`'s (`v1.3.0`) so Cargo resolves a single source of
`br-core-*`:

```toml
[dev-dependencies]
conformance-passport = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v1.2.0" }
```

## License

Apache-2.0. MSRV **1.88** (edition 2024).

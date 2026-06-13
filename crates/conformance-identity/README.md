# conformance-identity

A **black-box conformance runner** for the BotResources **Identity scope-acceptance
side** of the scope-declaration wire handshake (conformance group **G2**). It is the
exact mirror of [`conformance-scope`] (G3) with the role **inverted**: where G3
tested a scope-*declaring* service with the runner playing the acceptor, G2 tests a
scope-*accepting* service (the Identity registry) with the runner playing the
**declaring** side. It drives the frozen Go test subject in
`conformance-subjects/identity-acceptor` as a black box — it never reads the
subject's source — and asserts that an Identity registry decides every declaration
exactly as the platform requires.

> ⚠️ **TEST FIXTURE ONLY.** Add it as a **dev-dependency**, never a runtime one.
> It builds and spawns a real binary, stands up a real `nats-server`, and plays the
> declaring side of the handshake on an isolated, throwaway test network.

## What it proves — and why this is a backward-compat anchor

The runner plays the declaring side **using the real `br-rust-common` types**, and
judges the subject's verdict against the **real acceptance logic** as the oracle —
never hand-rolled JSON, never a re-stated decision table:

- It builds the declare as a real
  `IntegrationCommand<DeclareServiceScopes>` and publishes it on the contract
  subject; it decodes the reply **by deserialization** into the real
  `IntegrationEvent<ServiceScopesAccepted>` / `IntegrationEvent<ServiceScopesRejected>`
  types. **Deserialization succeeding *is* the wire-shape check** — a malformed reply
  is the assertion failing. There is no hand-written shape guard to drift from the
  types, and no `deny_unknown_fields`.
- **The oracle is the real `judge_declaration` / `ScopeRegistry`** from
  `br-identity-domain`. For each scenario the runner constructs a real `ScopeRegistry`,
  replays the **same** declaration sequence through the real lib, obtains the expected
  `DeclarationOutcome` per step, and asserts the subject's emitted event (accept vs
  reject, and the exact `ScopeDeclarationError`) **equals** the lib's verdict. The
  expected outcome is **computed from the lib, never hard-coded.**
- The handshake **subjects** are read from `br-scope-declaration-contract`
  (`declare_subject()` / `accepted_event_subject()` / `rejected_event_subject()`),
  not hardcoded — so the anchor covers the *subject strings* too: a renamed or
  re-versioned subject in `br-rust-common` diverges from the frozen Go wire and the
  battery goes red.

> **The Go subject is a frozen, independent anchor of the external wire + policy.**
> It is a faithful reimplementation of `judge_declaration`'s decision rules and
> precedence in Go — not a binding to the Rust lib. When `br-rust-common` evolves,
> this crate bumps its lib pin and re-runs against the unchanged Go subject:
> **green means the external envelope and the acceptance policy didn't move**; **red
> is a real break** (to be fixed, or acknowledged as a deliberate major bump and a
> coordinated subject update).

## The conformance battery (A1–A6)

Each is an `#[ignore]`-gated real-infra test (`tests/conformance.rs`). Every check
drives the scenario's declaration sequence at the subject and asserts each step's
verdict equals the `judge_declaration` oracle's verdict for that prefix:

| Id | Asserts |
|---|---|
| **A1** | A clean declaration is **accepted** — a well-formed `accepted` event echoing the command's `correlation_id`, carrying the declaring service. |
| **A2** | A second service claiming a first service's scope key is **rejected**. (Seed: service-A declares clean → accept; service-B claims an A-owned key → reject.) See the Why table — the oracle computes this as `scope_prefix_mismatch`, not `scope_owned_by_another_service`. |
| **A3** | A declaration with the same scope key listed twice is **rejected** with `duplicate_scope_in_declaration`. |
| **A4** | A declaration whose scope key is not prefixed by the declaring service is **rejected** with `scope_prefix_mismatch`. |
| **A5** | A declaration with a malformed scope key is **rejected** with `invalid_scope_key` (nesting the `KeyValidationError`). |
| **A6** | The same service re-declaring the same scopes is **accepted again** — idempotent, no false conflict. |

The acceptor keeps an in-memory `scope_key → owning_service` registry **across**
declarations within a run — state is the whole point: the runner seeds an ownership
context by driving a prior accepted declaration, then drives the contested one.

## Public helper surface (the reusable battery)

- `IdentityHarness` — `start()` builds the Go subject (`build_subject`) and is the
  self-test entry point; `start_with_binary(binary)` runs the **same** battery
  against any acceptor binary, so a consuming Identity service drives it against its
  own. It spawns a dedicated `SpawnedNats` and creates the handshake JetStream stream
  up front (the platform never auto-provisions, so the runner must).
  `declarer()` / `capture_confirmations()`.
- `Declarer` — builds a real `IntegrationCommand<DeclareServiceScopes>` and publishes
  it (ack-confirmed) via `br_core_integration::NatsIntegrationPublisher`.
- `ConfirmationCapture` — a subscribe-**first** ephemeral consumer
  (`DeliverPolicy::New`, `AckPolicy::None`) over both event subjects;
  `verdict_for(correlation_id)` decodes the reply by `from_slice` into the real
  `IntegrationEvent<…>` types — the type-oracle check.
- `oracle::expected_verdict` / `expected_step_verdicts` — replays a declaration
  sequence through the real `judge_declaration` / `ScopeRegistry` and returns the
  per-step `Verdict`. This is the authoritative expected outcome.
- `Scenario` (A1–A6) with `sequence(namespace)` — the declaration sequences,
  namespaced so each run uses unique service/scope keys.
- `Subject` / `SubjectConfig` — spawn the built binary with its env wiring
  (`NATS_URL`, `HTTP_ADDR`, `STREAM_NAME`, `SCOPE_ACCEPTANCE_ENABLED`) and poll
  `/readyz` / `/livez`.
- `create_handshake_stream`, and the three subject derivers, from
  `br-scope-declaration-contract`.

## The single-implementation check API

Each A1–A6 check shares **one** implementation (`run_judged_scenario`), returning a
structured `CheckOutcome` instead of panicking. Both `tests/conformance.rs` and the
programmatic runners call it.

- `CheckContext<'a>` — a `Declarer`, a `ConfirmationCapture`, the per-run
  `namespace`, and a per-step `timeout`.
- `CheckId` / `CheckStatus { Pass, Fail, Skipped }` / `CheckOutcome { id, status,
  expected, observed, detail }` / `ConformanceReport { outcomes }` with
  `passed()` / `failed()` / `skipped()` / `is_conformant()`.

## Two programmatic runners — spawn and attach

- `run_spawn(SpawnTarget { binary }, scenarios, timeout)` — the core deliverable.
  Stands up a throwaway `SpawnedNats`, creates the handshake stream, launches the
  acceptor binary, waits for `/readyz=200`, and runs the full `a1..a6`. Needs
  `nats-server` on `PATH`.
- `run_attach(AttachTarget { nats_url, readyz_url, stream_name }, scenarios, timeout)`
  — connects to an already-running Identity service's NATS and polls its `/readyz`
  directly; never spawns `nats-server` and never builds Go. It does **not** create the
  stream — the live service owns it; the confirmation consumer binds to the
  pre-existing stream and fails loud if it is absent. **Caveat:** driving declarations
  **mutates a live registry**; the runner uses **unique per-run service/scope keys**
  (a fresh UUIDv7 namespace) to bound pollution, so A1–A6 are safe to run repeatedly
  against the same live service.

## Running it

Needs `nats-server` and the **Go toolchain** on `PATH` (the runner builds the
subject). The tests are `#[ignore]`-gated so the default `cargo test` stays green
without infra:

```sh
cargo test -p conformance-identity --test conformance -- --ignored
```

CI runs it in the `infra-e2e` job, which already has `go` and `nats-server` on the
runner (shared with G3).

## Install

A **dev-dependency**, pinned to a release tag (git-tag distribution; no crates.io).
Keep its `br-rust-common` tag identical to `br-test-harness`'s (`v0.8.0`) so Cargo
resolves a single source and never duplicates `br-core-*`:

```toml
[dev-dependencies]
conformance-identity = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v0.3.0" }
```

## Why — the non-obvious bits

| Thing | Why it is the way it is |
|---|---|
| **A2's oracle verdict is `scope_prefix_mismatch`, not `scope_owned_by_another_service`** | The oracle is `judge_declaration`, which calls `command.validate()` **before** the registry's cross-owner check. A scope key whose prefix is service-A can only be declared by service-A (the prefix rule), so a cross-service claim is rejected at validation as `scope_prefix_mismatch` and never reaches the registry's `ScopeOwnedByAnotherService` branch. That branch is real but unreachable via `judge_declaration` — in production it is produced by the app-layer pipeline from a `UNIQUE(scope_key)` DB conflict, not by the domain handler. A2 asserts the **oracle-computed** verdict, never a hard-coded reason. |
| The verdict check is `from_slice` into the real `IntegrationEvent<…>`, with **no** extra no-extra-fields guard | Deser success against the real envelope + payload *is* the conformance check; a stricter hand-rolled guard would be a second contract that drifts from the types — exactly what this crate exists to prevent. |
| Scenarios carry **raw** `DeclareServiceScopes`, built from `RawScopeDeclaration` | A3/A4/A5 are intentionally malformed declarations that `ScopeDeclaration::new` would refuse to construct. The wire payload is `RawScopeDeclaration`, so the runner builds the raw shape and feeds the **same** raw payload to both the subject and the oracle — the oracle's `judge_declaration` runs the same validation the subject must. |
| The runner's declare metadata uses `Actor::Service` | A boot-time scope declaration is service-initiated; the real declarer (`br-util-scope-declaration`) emits `actor_kind:"service"`. The acceptor matches on `correlation_id` only and never validates the actor. |
| `ConfirmationCapture` subscribes **before** the subject is spawned and declares are sent | The capture uses `DeliverPolicy::New`; a reply published before its consumer exists is missed. Capture is armed first, then the subject boots and is awaited ready, then declares flow. |
| The runner waits for `/readyz=200` **before** declaring | The acceptor binds its declare consumer with `DeliverPolicy::New`; a declare published before the consumer exists would be missed. Readiness gates the consumer being live. |
| Keys are **namespaced per run** with a UUIDv7 | spawn mode is isolated anyway, but the same scenarios drive an attach run against a live, stateful registry; unique keys keep re-runs from colliding with each other or with prior state, and bound the pollution of a real registry. |
| The Go subject is a faithful reimpl of `judge_declaration`, not a binding | It is the tautology-break: an independent encoding of the wire + policy. If it merely called the lib, green would prove nothing. Its Go unit tests pin the rejection wire shapes to the frozen `scope-wire-v1` golden JSON. |
| The whole battery is `#[ignore]`-gated | It drives real infra (`nats-server` + `go` + a spawned binary); the default `cargo test` must stay green on a machine without them, exactly like `br-test-harness`'s own self-tests. |

[`conformance-scope`]: ../conformance-scope/README.md

## License

Apache-2.0. MSRV **1.88** (edition 2024).

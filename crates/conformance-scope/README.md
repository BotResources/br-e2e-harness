# conformance-scope

A **black-box conformance runner** for the BotResources **scope-declaration wire
handshake** (conformance group **G3**). It drives the frozen Go test subject in
`conformance-subjects/scope-service` as a black box — it never reads the
subject's source — and asserts that a scope-owning service performs the
handshake exactly as the platform requires.

> ⚠️ **TEST FIXTURE ONLY.** Add it as a **dev-dependency**, never a runtime one.
> It builds and spawns a real binary, stands up a real `nats-server`, and plays
> the Identity side of the handshake on an isolated, throwaway test network.

## What it proves — and why this is a backward-compat anchor

The runner's fake acceptor plays the Identity side **at the wire level, using the
real `br-rust-common` types as the oracle** — never hand-rolled JSON:

- It validates the subject's declare **by deserialization**: it takes the raw
  bytes of the declare message and runs
  `serde_json::from_slice::<IntegrationCommand<DeclareServiceScopes>>(bytes)`.
  **Deserialization succeeding *is* the wire-shape check** — the real envelope +
  the real `br-core-scope` payload accept the bytes — and a failure is the
  assertion failing (reject). There is no separate, hand-written shape guard to
  drift from the types.
- It emits confirmations through the same real types:
  `IntegrationEvent<ServiceScopesAccepted>` /
  `IntegrationEvent<ServiceScopesRejected>`, serialized with serde and published
  on the matching subject, echoing the command's `metadata.correlation_id`.
- The handshake **subjects** are read from `br-scope-declaration-contract`
  (`declare_subject()` / `accepted_event_subject()` / `rejected_event_subject()`),
  not hardcoded — so the anchor below covers the *subject strings* too, not just
  the payload shape: a renamed or re-versioned subject in `br-rust-common`
  diverges from the frozen Go wire and the battery goes red.

> **The Go subject is a frozen anchor of the external wire.** When
> `br-rust-common` evolves, this crate bumps its lib pin and re-runs against the
> unchanged Go subject: **green means the external envelope didn't move — the
> published-language wire is backward-compatible** with the new types; **red is a
> real break** (to be fixed, or to be acknowledged as a deliberate major bump and
> a coordinated subject update). This crate is a **backward-compatibility gate on
> the published-language scope-declaration wire.**

## The conformance battery (S1–S6)

Each is an `#[ignore]`-gated real-infra test (`tests/conformance.rs`), in the
`br-test-harness` `infra-e2e` style:

| Id | Asserts |
|---|---|
| **S1** | The subject declares on boot and the declare deserializes into `IntegrationCommand<DeclareServiceScopes>`; the declared scopes validate and are owned by the manifest service. |
| **S2** | `/readyz` is 503 before, 200 after the acceptor emits `accepted` (echoing the correlation_id). |
| **S3** | Withheld past `WAIT_TIMEOUT`, the subject re-publishes the **same** correlation_id (≥2 declares); accepting it then drives `/readyz` to 200. |
| **S4** | On `rejected`, the subject surfaces the rejection reason in its `/readyz` body (`scope declaration rejected: <code>`) — proving it received and processed the reject — then `/readyz` stays 503 and the subject stops re-publishing. |
| **S5** | Duplicate `accepted` events are tolerated — the subject reaches ready once and stays alive. |
| **S6** | With `SCOPE_DECLARATION_ENABLED=false`, no declare is ever published and `/readyz` is 200 immediately. |

## Public helper surface (the reusable battery)

A consuming service's e2e can drive the same battery against its own
binary by reusing the helpers, not just the tests:

- `ScopeHarness` — `start()` builds the Go subject (`build_subject`) and is the
  self-test entry point; `start_with_binary(binary)` runs the **same** battery
  against any subject binary, so a consuming service drives it against its own.
  Either way it spawns a dedicated `SpawnedNats` and creates the handshake
  JetStream stream up front (the platform never auto-provisions, so the runner
  must). `capture_declares()` starts a `DeclareCapture`.
- `DeclareCapture` — a **replaying** ephemeral consumer
  (`DeliverPolicy::All`, `AckPolicy::None`) over the declare subject that drains
  every declare on the stream into a shared buffer; `decode()` is the type-oracle
  check. Replay means a declare published before the capture exists (attach mode)
  is still caught.
- `accept` / `reject` — publish a confirmation built from the real
  `br-core-scope` payload types, echoing a correlation_id, ack-confirmed.
- `Subject` / `SubjectConfig` — spawn the built binary with its env wiring and
  poll `/readyz` / `/livez` (status, and `readyz_body()` for the rejection
  reason S4 corroborates).
- `create_handshake_stream`, and the three subject derivers
  (`declare_subject()` / `accepted_event_subject()` / `rejected_event_subject()`),
  read from `br-scope-declaration-contract` rather than hardcoded — so a subject
  drift in `br-rust-common` diverges from the frozen Go wire fixture and the
  battery goes red.

## The single-implementation check API

Each S1–S6 check (plus the `declaration-content` assertion) has **one**
implementation in `checks/`, returning a structured outcome instead of
panicking. Both `tests/conformance.rs` and the [`conformance-scope-cli`] binary
call these — there is no second copy of the protocol logic.

- `CheckContext<'a>` — the parameters a check needs: a `jetstream::Context`, a
  `ReadyzProbe`, a `DeclareCapture`, the `ExpectedDeclaration`, the declaring
  `ServiceKey`, the `AcceptorBehavior`, and a per-step `timeout`.
- `run_scenario(scenario, ctx) -> CheckOutcome` and the individual check
  functions (`declare_well_formed`, `declaration_content`, `readiness_gated`,
  `republishes_same_correlation_id`, `rejection_stops_readiness`,
  `duplicate_confirmations_tolerated`, `disabled_mode_ready_without_declare`).
- `CheckId` / `CheckStatus { Pass, Fail, Skipped }` / `CheckOutcome { id, status,
  expected, observed, detail }` / `ConformanceReport { outcomes }` with
  `passed()` / `failed()` / `skipped()` / `is_conformant()`.
- `ExpectedDeclaration` / `ExpectedScope` / `PlatformOnly` — the assertion input.
  `platform_only` is modeled **per scope** (faithful to `br-core-scope`'s
  `ScopeSpec.platform_only`); `PlatformOnly::All(bool)` is a convenience that
  expands one bool over every scope, `PlatformOnly::PerScope` carries a
  `key → bool` map. `assert_matches` renders a wire-faithful expected-vs-observed
  diff on mismatch.

## Two programmatic runners — spawn and attach

Both stand up a `CheckContext` and call the same checks; they differ only in how
the dependencies are obtained.

- `run_spawn(SpawnTarget { binary }, expected, behavior, scenarios, timeout)` —
  the convenience mode. Stands up a throwaway `SpawnedNats`, creates the
  handshake stream, and launches the subject binary with the env contract
  (`SERVICE_KEY`, `SCOPE_KEYS`, `PLATFORM_ONLY`, `SCOPE_DECLARATION_ENABLED`, …).
  Runs the full `s1..s6` default because it controls the subject's config and
  lifecycle. Needs `nats-server` on `PATH`.
- `run_attach(AttachTarget { nats_url, readyz_url, stream_name }, expected,
  behavior, scenarios, timeout)` — the primary mode, with **zero host runtime
  deps**: it connects to an already-running service's NATS and polls its
  `/readyz` URL directly, never spawning `nats-server` and never building Go. It
  **does not** create the stream — the live service owns the handshake stream;
  the declare consumer binds to the pre-existing stream and fails loud with a
  clear error if it is absent. Default scenarios are `s1, s2` + the
  `declaration-content` assertion (the lifecycle-controlling scenarios
  s3/s4/s6 cannot run against an already-booted service).

`ReadyzProbe::new(url)` is the readyz role decoupled from any spawned process —
spawn passes the spawned subject's `base_url()`, attach passes the external URL.

For a no-Rust-required CLI over both runners (`run`, `manifest`, exit codes,
human/json/junit reports), see the [`conformance-scope-cli`] crate.

[`conformance-scope-cli`]: ../conformance-scope-cli/README.md

## Running it

Needs `nats-server` and the **Go toolchain** on `PATH` (the runner builds the
subject). The tests are `#[ignore]`-gated so the default `cargo test` stays green
without infra:

```sh
cargo test -p conformance-scope --test conformance -- --ignored
```

CI runs it in the `infra-e2e` job — which now additionally requires **`go`** on
the runner (alongside `nats-server`), to build the conformance subject.

## Install

A **dev-dependency**, pinned to a release tag (git-tag distribution; no
crates.io). Keep its `br-rust-common` tag identical to `br-test-harness`'s
(`v0.11.1`) so Cargo resolves a single source and never duplicates `br-core-*`:

```toml
[dev-dependencies]
conformance-scope = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v0.5.0" }
```

## Why — the non-obvious bits

| Thing | Why it is the way it is |
|---|---|
| The shape check is `from_slice` into the real type, with **no** extra no-extra-fields / round-trip guard | The owner's ruling: deser success against the real envelope + payload *is* the conformance check. A stricter hand-rolled guard would be a second contract that drifts from the types — exactly what this crate exists to prevent. |
| The fake acceptor's reply `metadata` uses an `Actor::Human` | The wire spec's awaiter matches on `correlation_id` only and tolerates a defaulted human actor; emitting an unknown `actor_kind` is the one thing that hard-fails the real metadata deserializer, so the reply never invents one. |
| `DeclareCapture` uses `DeliverPolicy::All` on the declare-capture | Attach mode attaches the capture **after** the subject's boot declare is already on the stream; replay is the only way to catch it. `New` would skip that first declare and only converge on the ~10s re-publish, making the gate slow. The declarer's own confirmation awaiter still uses `New` (a confirmation pre-published before its consumer exists is missed), which is why `accept`/`reject` publish only after the subject is up. |
| Capture buffers via a lenient `correlation_id`-only probe, while `decode()` uses the strict real type | The buffer must record every declare to count re-publishes and read the id to echo it — that only needs the correlation_id, exactly as the real awaiter's `CorrelationProbe`. The conformance verdict (S1) is the separate, strict `decode()` against the real envelope + payload. Splitting them keeps the shape check honest without making the buffer depend on a conformant payload. |
| The Go build uses `go build -C <dir>` | `br_test_harness::run_once` sets env but not the child's working directory; `-C` builds the package without depending on cwd, race-free under parallel `cargo test`. |
| The stream is created by the runner, not the subject | The platform never auto-provisions (fail-loud); the subject does `get_stream`, so the harness owns stream setup. `identity.>` captures the declare command **and** both event subjects — the declare must be captured because the declarer awaits its publish ack. |
| The Go binary is built to a unique temp path per call | Each test builds its own subject; a shared output path would race under parallel `cargo test`. |
| The whole battery is `#[ignore]`-gated | It drives real infra (`nats-server` + `go` + a spawned binary); the default `cargo test` must stay green on a machine without them, exactly like `br-test-harness`'s own self-tests. |
| S4 reads the `/readyz` **body** before asserting no more declares | "It went quiet" alone is satisfiable by a reject that was never delivered. The subject writes `scope declaration rejected: <code>` into its `/readyz` body, so matching that body (the code is the real `ScopeDeclarationError` Display, not a literal) positively proves the reject was received and processed — only then is the tight `== count_at_reject` (no `+1` slack) sound. |
| In **spawn** mode s4 always rejects, regardless of the global accept/reject flag | s4 is intrinsically a rejection scenario while s2/s3/s5 are acceptance scenarios, so the full `s1..s6` battery cannot share one global behavior. In spawn mode each lifecycle scenario drives its own fresh subject, so s4 synthesizes a rejection (the global `--reject` reason if given, else a default derived from the first expected scope key) while the others accept. The global flag customizes s4's reason; it never lets s4 silently skip. |
| s2/s3/s5 accept unconditionally inside the check, ignoring `AcceptorBehavior` | These are acceptance scenarios; only `rejection_stops_readiness` reads `behavior`. The global `--reject` is meaningful for the rejection scenario (and in attach mode when the user selects it against their live service), not for the acceptance ones. |
| Attach default omits s3/s4/s6 | Those require controlling the subject's config and lifecycle (withholding to force re-publish, rejecting, disabled mode) — impossible against an already-running attached service. Attach proves the observable contract: a well-formed declare, its content, and the acceptance gating. |
| Attach mode assumes a **fresh** handshake stream per run (no leftover `declare`) | The capture replays from the start of the stream (`DeliverPolicy::All`) and takes `capture.first()` as the boot declare / correlation_id oracle; a stale cross-run declare would make the oracle pick a dead correlation_id. |

## License

Apache-2.0. MSRV **1.88** (edition 2024).

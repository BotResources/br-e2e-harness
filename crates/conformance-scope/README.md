# conformance-scope

A **black-box conformance runner** for the BotResources **scope-declaration wire
handshake** (conformance group **G3**). It drives the frozen Go test subject in
`conformance-subjects/scope-service` as a black box — it never reads the
subject's source — and asserts that a scope-owning service performs the
handshake exactly as the platform requires.

> ⚠️ **TEST FIXTURE ONLY.** Add it as a **dev-dependency**, never a runtime one.
> It spawns a prebuilt binary, stands up a real `nats-server`, and plays
> the Identity side of the handshake on an isolated, throwaway test network.

> **Wire — v1.0.0 `integration.*` grammar, migrated on both sides.**
> The Rust runner and `FabricTestNats` provision the two fixed streams
> `INTEGRATION_CMD` (`integration.cmd.>`, where the declare lands) and
> `INTEGRATION_EVT` (`integration.evt.>`, where both confirmations land), the
> acceptor publishes confirmations via the real `Fabric`, and `SubjectConfig`
> carries **both** stream names (injected to the subject as `COMMAND_STREAM_NAME`
> / `EVENT_STREAM_NAME`). The **Go anchor**
> (`conformance-subjects/scope-service`) freezes the same v1.0.0 wire as
> constants in `wire.go` — `integration.cmd.identity.service_scope.declare.v1` /
> `integration.evt.identity.service_scope.{accepted,rejected}.v1` and the
> `INTEGRATION_EVT` confirmation stream — and performs the two-stream split
> (publish the declare onto `INTEGRATION_CMD` by subject, await confirmations on
> `INTEGRATION_EVT`). The anchor freezes these names independently of the lib
> (the env injection is the runner's, not the anchor's, contract). The
> `#[ignore]`-gated real-infra tests in `tests/conformance.rs` run in the
> `infra-e2e` CI job against a real `nats-server` (lib-as-oracle / Go-as-anchor:
> the Go side freezes the wire independently of the lib).

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
| **S5** | Duplicate `accepted` events are tolerated — the subject reaches ready, and `/readyz` stays `200` for the 3-second quiet window after the duplicate. |
| **S6** | With `SCOPE_DECLARATION_ENABLED=false`, no declare is ever published and `/readyz` is 200 immediately. |

## Public helper surface (the reusable battery)

A consuming service's e2e can drive the same battery against its own
binary by reusing the helpers, not just the tests:

- `ScopeHarness` — `start()` locates the prebuilt `scope-service` subject
  (`locate_subject`: `PATH`, then the cache directory, then the release asset) and
  is the self-test entry point; `start_with_binary(binary)` runs the **same** battery
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
- The three subject derivers
  (`declare_subject()` / `accepted_event_subject()` / `rejected_event_subject()`)
  render the typed `br-scope-declaration-contract` coordinates through the
  `br-util-nats-fabric` Fabric rather than hardcoding strings — so a subject drift
  in `br-rust-common` diverges from the frozen Go wire fixture and the battery goes
  red. The fixed `INTEGRATION_CMD` / `INTEGRATION_EVT` streams are provisioned by
  the harness via `FabricTestNats`.

## The single-implementation check API

Each S1–S6 check (plus the `declaration-content` assertion) has **one**
implementation in `checks/`, returning a structured outcome instead of
panicking. Both `tests/conformance.rs` and the [`conformance-scope-cli`] binary
call these — there is no second copy of the protocol logic.

- `CheckContext<'a>` — the parameters a check needs: a `Fabric`, a
  `ReadyzProbe`, a `DeclareCapture`, the `ExpectedDeclaration`, the declaring
  `ServiceKey`, the `AcceptorBehavior`, and a per-step `timeout`. A check reads the
  declare capture through `DeclareCapture::try_count` / `try_first` /
  `try_correlation_ids`: a frame on the declare subject that is not an envelope with
  `metadata.correlation_id` is a failed check naming that cause, never an absent
  declare and never a panic.
- `run_scenario(scenario, ctx) -> CheckOutcome` and the individual check
  functions (`declare_well_formed`, `declaration_content`, `readiness_gated`,
  `republishes_same_correlation_id`, `rejection_stops_readiness`,
  `duplicate_confirmations_tolerated`, `disabled_mode_ready_without_declare`).
- `CheckId` / `CheckStatus { Pass, Fail, Skipped }` / `CheckOutcome { id, status,
  expected, observed, detail }` / `ConformanceReport { outcomes }` with
  `passed()` / `failed()` / `skipped()` / `is_conformant()`. `is_conformant()` is
  true only for a report with at least one outcome, no failed and no skipped outcome.
- `ExpectedDeclaration` / `ExpectedScope` / `PlatformOnly` — the assertion input.
  `platform_only` is modeled **per scope** (faithful to `br-core-scope`'s
  `ScopeSpec.platform_only`); `PlatformOnly::All(bool)` is a convenience that
  expands one bool over every scope, `PlatformOnly::PerScope` carries a
  `key → bool` map. `assert_matches` renders a wire-faithful expected-vs-observed
  diff on mismatch. Spawn mode drives the subject with one flag for every scope, so
  `run_spawn` returns `InvalidInput` for an expectation whose scopes carry different flags.
  `ExpectedDeclaration::try_from_parts` refuses a per-scope key that names no listed scope
  or appears twice; `ExpectedDeclaration::validate` (called first by `run_attach` and
  `run_spawn`, before any I/O) refuses a service key or a scope key that no
  `ScopeDeclaration` can hold (invalid `ScopeKey`, not owned by the service, listed
  twice). Both return `InvalidInput` naming the key, so a typo in the assertion is never
  reported as a failed check of the service.

## Two programmatic runners — spawn and attach

Both stand up a `CheckContext` and call the same checks; they differ only in how
the dependencies are obtained.

- `run_spawn(SpawnTarget { binary }, expected, behavior, scenarios, timeout)` —
  the convenience mode. Stands up a throwaway `FabricTestNats`, provisions the
  two fixed handshake streams (`INTEGRATION_CMD` / `INTEGRATION_EVT`), and
  launches the subject binary with the env contract (`COMMAND_STREAM_NAME`,
  `EVENT_STREAM_NAME`, `SERVICE_KEY`, `SCOPE_KEYS`, `PLATFORM_ONLY`,
  `SCOPE_DECLARATION_ENABLED`, …). Runs the full `s1..s6` default because it
  controls the subject's config and lifecycle. Needs `nats-server` on `PATH`.
  `run_spawn` and `run_attach` return `InvalidInput` for an empty scenario slice.
  The Go anchor freezes the v1.0.0 `integration.*` subjects and the
  `INTEGRATION_EVT` stream as constants, so this mode runs green against v1.0.0.
- `run_attach(AttachTarget { nats_url, readyz_url }, expected,
  behavior, scenarios, timeout)` — the primary mode, with **zero host runtime
  deps**: it connects to an already-running service's NATS and polls its
  `/readyz` URL directly, never spawning `nats-server` and never starting a subject.
  Both `INTEGRATION_CMD` and `INTEGRATION_EVT` must already exist: attach checks
  them before anything else, never creates them, and returns a typed
  `Jetstream` error when either is missing. A `readyz_url` that is not an
  `http` or `https` URL with a host is `InvalidInput`. Default scenarios are `s1, s2` + the
  `declaration-content` assertion. s3, s4 and s6 run when selected and hold under
  their prerequisite: s3 needs a service that is still unconfirmed and re-publishes
  within `timeout` (a `timeout` above the service's own wait timeout), s4 needs
  `AcceptorBehavior::Reject` and a service still awaiting acceptance, s6 needs a
  service booted with scope declaration disabled. A failed s3, s4 or s6 names its
  prerequisite in its `detail`; s4 with `AcceptorBehavior::Accept` is `skipped`.

`ReadyzProbe::new(url)` is the readyz role decoupled from any spawned process —
spawn passes the spawned subject's `base_url()`, attach passes the external URL.

For a no-Rust-required CLI over both runners (`run`, `manifest`, exit codes,
human/json/junit reports), see the [`conformance-scope-cli`] crate.

[`conformance-scope-cli`]: ../conformance-scope-cli/README.md

## Running it

The tests never compile anything. They need `nats-server` on `PATH` and the Go
subject as a **prebuilt binary named `scope-service`**:

1. on `PATH` (first match among the absolute `PATH` entries), or
2. in the cache directory `$XDG_CACHE_HOME/br-e2e-harness/conformance-scope-{version}`
   (`$HOME/Library/Caches/...` on macOS, `$HOME/.cache/...` elsewhere; a relative
   `XDG_CACHE_HOME` or `HOME` is ignored), or
3. downloaded into that cache from this crate's own GitHub release
   `conformance-scope/v{version}` of `BotResources/br-e2e-harness`: the asset
   `scope-service-<os>-<arch>.tar.gz` with its sibling `.sha256` (`linux` and
   `darwin`, `amd64` and `arm64`). The download fails closed on a SHA-256 mismatch,
   a redirect to a host other than `github.com` / `objects.githubusercontent.com` /
   `release-assets.githubusercontent.com`, or an archive over 64 MiB. It needs `tar` on
   `PATH`. The spawned subject receives a `PATH` led by the directory of its binary
   through the child's environment; the test process environment is never modified.

A binary found nowhere is `ConformanceError::SubjectBinaryUnavailable`, naming the
binary and what to do. To use your own build, put it on `PATH` before running:

```sh
PATH="$PWD/bin:$PATH" cargo test -p conformance-scope --all-targets -- --ignored --test-threads=1
```

The `#[ignore]`-gated tests drive real infra, so the default `cargo test` stays
green without it. The crate writes nothing to the system temporary directory (the
throwaway NATS store belongs to `br-test-harness`).

The suite **provisions the scope-declaration handshake topology by spawning the
`fabric-nats` CLI** (`fabric-nats provision --manifest
tests/fixtures/scope_declaration.toml`) — the conformance suite is the CLI's
real-life testbed. Spawn mode therefore needs the `fabric-nats` binary (the
`fabric-nats` bin of the `br-test-harness` package, built outside the tests) in the directory of the running executable, and
the manifest `tests/fixtures/scope_declaration.toml` of the conformance-scope source tree
the executable was built from (its path is fixed at build time). When either is missing,
`run_spawn` and `provision` return `ConformanceError::FabricProvisionUnavailable`, naming
the missing prerequisite and what to do; the error never unwinds out of the call, but the harness panic text can still appear on stderr. Attach mode needs neither. The
crate carries **no `async-nats` dependency**: the declare capture is a thin typed view over
the harness `CommandCapture`, the acceptor publishes accept/reject through a `Fabric`, and
attach mode connects through `FabricTestNats::attach_without_provisioning`, so attach mode
never creates a stream: a missing `INTEGRATION_CMD` or `INTEGRATION_EVT` is a typed
`ConformanceError::Jetstream`.

CI runs it in the `infra-e2e` job, which builds `scope-service` outside the tests
(dead-grammar guard, vet, Go tests, build) and puts it on `PATH` before the batteries.

## Run in CI

A service runs the battery in its own CI, against its own booted binary, with
the released `conformance-scope` binary pinned to a version. Nothing is compiled
and no Rust toolchain is needed. The CI mode is **attach**: the service is the
one under test, and the CLI plays the Identity side of the handshake against the
service's NATS and `/readyz`.

Before the CLI runs, the job must hold: a `nats-server` with JetStream, the two
fixed streams `INTEGRATION_CMD` (`integration.cmd.>`) and `INTEGRATION_EVT`
(`integration.evt.>`) already created (attach mode never creates them; a missing
stream exits `2`), and the service booted on that NATS, awaiting acceptance of
its declaration. The streams must be fresh: a leftover declare from an earlier
run makes the battery read a dead correlation id.

```yaml
- name: scope-declaration conformance
  env:
    CONFORMANCE_SCOPE_VERSION: "0.1.1"
    NATS_SERVER_VERSION: "v2.14.1"
    NATSCLI_VERSION: "0.3.1"
  run: |
    set -euo pipefail
    tools="$RUNNER_TEMP/conformance-tools"
    mkdir -p "$tools"

    target=x86_64-unknown-linux-musl
    asset="conformance-scope-${CONFORMANCE_SCOPE_VERSION}-${target}.tar.gz"
    base="https://github.com/BotResources/br-e2e-harness/releases/download/conformance-scope/v${CONFORMANCE_SCOPE_VERSION}"
    (cd "$tools" && curl -fsSLO "${base}/${asset}" && curl -fsSLO "${base}/${asset}.sha256" && sha256sum -c "${asset}.sha256" && tar -xzf "${asset}")

    nats_server="nats-server-${NATS_SERVER_VERSION}-linux-amd64"
    curl -fsSL "https://github.com/nats-io/nats-server/releases/download/${NATS_SERVER_VERSION}/${nats_server}.tar.gz" | tar -xz -C "$tools"
    natscli="nats-${NATSCLI_VERSION}-linux-amd64"
    curl -fsSL -o "$tools/natscli.zip" "https://github.com/nats-io/natscli/releases/download/v${NATSCLI_VERSION}/${natscli}.zip"
    unzip -q -o "$tools/natscli.zip" -d "$tools"

    "$tools/$nats_server/nats-server" -js -a 127.0.0.1 -p 4222 -m 8222 &
    until curl -fs http://127.0.0.1:8222/healthz >/dev/null; do sleep 0.2; done
    nats="$tools/$natscli/nats --server nats://127.0.0.1:4222"
    $nats stream add INTEGRATION_CMD --subjects 'integration.cmd.>' --defaults
    $nats stream add INTEGRATION_EVT --subjects 'integration.evt.>' --defaults

    NATS_URL=nats://127.0.0.1:4222 HTTP_ADDR=127.0.0.1:8080 ./target/release/my-service &
    until curl -fs http://127.0.0.1:8080/livez >/dev/null; do sleep 0.2; done

    "$tools/conformance-scope" run \
      --nats nats://127.0.0.1:4222 \
      --readyz http://127.0.0.1:8080/readyz \
      --service-key my_service \
      --scopes my_service:read,my_service:admin \
      --platform-only false \
      --format junit --output conformance-scope.xml
```

The step fails the job on a non-zero exit code: `1` when the service is not
conformant (a failed or skipped check, or no check ran), `2` when the run itself
could not happen (NATS unreachable, a missing stream, a bad argument). The
service's own environment variables are the service's; the CLI needs only its
NATS URL and its `/readyz` URL. On macOS use the `aarch64-apple-darwin` asset and
`shasum -a 256 -c`. Bump `CONFORMANCE_SCOPE_VERSION` deliberately, as for any
pinned tool; the `.sha256` check ties the download to the published asset. Poll the
service's `/livez`, never its `/readyz`: `/readyz` stays `503` until the CLI accepts
the declaration, so waiting on it would deadlock.

`--spawn <service-binary>` is not the CI mode: it stands up its own throwaway
`nats-server` and needs the `fabric-nats` binary and the provisioning manifest of
the source tree beside the executable, which the released archive does not hold.

## Install

A **dev-dependency**, pinned to a release tag (git-tag distribution; no
crates.io). Keep each `br-rust-common` crate tag identical to `br-test-harness`'s
(per-crate `<crate>/v0.1.0`) so Cargo resolves a single source and never duplicates `br-core-*`:

```toml
[dev-dependencies]
conformance-scope = { git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-scope", tag = "conformance-scope/v0.1.1", version = "0.1.1" }
```

## Why — the non-obvious bits

| Thing | Why it is the way it is |
|---|---|
| The shape check is `from_slice` into the real type, with **no** extra no-extra-fields / round-trip guard | The owner's ruling: deser success against the real envelope + payload *is* the conformance check. A stricter hand-rolled guard would be a second contract that drifts from the types — exactly what this crate exists to prevent. |
| The fake acceptor's reply `metadata` uses an `Actor::Human` | The wire spec's awaiter matches on `correlation_id` only and tolerates a defaulted human actor; emitting an unknown `actor_kind` is the one thing that hard-fails the real metadata deserializer, so the reply never invents one. |
| `DeclareCapture` uses `DeliverPolicy::All` on the declare-capture | Attach mode attaches the capture **after** the subject's boot declare is already on the stream; replay is the only way to catch it. `New` would skip that first declare and only converge on the ~10s re-publish, making the gate slow. The declarer's own confirmation awaiter still uses `New` (a confirmation pre-published before its consumer exists is missed), which is why `accept`/`reject` publish only after the subject is up. |
| Capture buffers via a lenient `correlation_id`-only probe, while `decode()` uses the strict real type | The buffer must record every declare to count re-publishes and read the id to echo it — that only needs the correlation_id, exactly as the real awaiter's `CorrelationProbe`. The conformance verdict (S1) is the separate, strict `decode()` against the real envelope + payload. Splitting them keeps the shape check honest without making the buffer depend on a conformant payload. |
| The streams are created by the runner, not the subject | The platform never auto-provisions (fail-loud); the subject does `get_stream`, so the harness owns stream setup. `FabricTestNats` provisions the two fixed streams — `INTEGRATION_CMD` (`integration.cmd.>`, the declare command) and `INTEGRATION_EVT` (`integration.evt.>`, both confirmations) — because the declarer awaits its publish ack and a missing stream fails the publish. |
| The tests never compile, and the subject is found by name | Compiling inside a test hides a toolchain dependency and races under parallel `cargo test`. The subject is a prebuilt `scope-service`, found on `PATH`, then in a cache directory, then downloaded from this crate's own release and verified; CI builds and vets it outside the tests. |
| The download is verified and the child's `PATH` is passed through `Command::env` | A fetched binary is executed, so a digest mismatch, a foreign redirect host or an oversized body fail closed; `std::env::set_var` is `unsafe` in edition 2024, so the cache directory reaches the child through its environment instead. |
| The whole battery is `#[ignore]`-gated | It drives real infra (`nats-server` + a spawned binary); the default `cargo test` must stay green on a machine without them, exactly like `br-test-harness`'s own self-tests. |
| A failed check's `observed` holds what was seen, `expected` what was wanted | The CLI prints both side by side; an `observed` that restates the wanted state (for example "stayed 200") reports a failure as a pass in the human, JSON and JUnit output. |
| S5 observes `/readyz` for the quiet window after the first 200 | Passing at the first 200 cannot tell a tolerant subject from one that crashes or drops to 503 on the duplicate `accepted`; the check holds the 200 for `QUIET_WINDOW` after the duplicate. |
| `is_conformant` needs an outcome and no skip | A report that verified nothing, or only skipped checks, must not read as a pass. |
| S4 reads the `/readyz` **body** before asserting no more declares | "It went quiet" alone is satisfiable by a reject that was never delivered. The subject writes `scope declaration rejected: <code>` into its `/readyz` body, so matching that body (the code is the real `ScopeDeclarationError` Display, not a literal) positively proves the reject was received and processed — only then is the tight `== count_at_reject` (no `+1` slack) sound. |
| In **spawn** mode s4 always rejects, regardless of the global accept/reject flag | s4 is intrinsically a rejection scenario while s2/s3/s5 are acceptance scenarios, so the full `s1..s6` battery cannot share one global behavior. In spawn mode each lifecycle scenario drives its own fresh subject, so s4 synthesizes a rejection (the global `--reject` reason if given, else a default derived from the first expected scope key) while the others accept. The global flag customizes s4's reason; it never lets s4 silently skip. |
| s2/s3/s5 accept unconditionally inside the check, ignoring `AcceptorBehavior` | These are acceptance scenarios; only `rejection_stops_readiness` reads `behavior`. The global `--reject` is meaningful for the rejection scenario (and in attach mode when the user selects it against their live service), not for the acceptance ones. |
| Attach default omits s3/s4/s6 | Those depend on how the attached service was booted (a wait timeout shorter than `timeout` for s3, a pending declare and a rejection for s4, declaration disabled for s6), which the battery cannot know. Attach proves the observable contract by default: a well-formed declare, its content, and the acceptance gating; the other three run when selected and name their prerequisite when they fail. |
| `Subject::start` restarts a subject that exits before its `/livez` answers | The signal does not depend on the subject's language or log text. A `/livez` answer is re-checked 250 ms later because a subject that lost its port can be answered by the process that holds it. After five exits it returns `SubjectExitedAtBoot`, a setup error, never a dead subject to judge. |
| Attach mode assumes a **fresh** handshake stream per run (no leftover `declare`) | The capture replays from the start of the stream (`DeliverPolicy::All`) and takes `capture.first()` as the boot declare / correlation_id oracle; a stale cross-run declare would make the oracle pick a dead correlation_id. |

## License

Apache-2.0. MSRV **1.88** (edition 2024).

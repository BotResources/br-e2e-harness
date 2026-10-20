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
> It spawns a prebuilt binary, stands up a real `nats-server`, and plays the
> declaring side of the handshake on an isolated, throwaway test network. The
> crate never compiles anything.

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
  reject, the service the event names, and the exact `ScopeDeclarationError`)
  **equals** the lib's verdict. A rejection that names another service than the
  declared one fails the check. The expected outcome is **computed from the lib,
  never hard-coded.**
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

## The conformance battery (A1–A7)

Each is an `#[ignore]`-gated real-infra test (`tests/conformance.rs`). Every check
drives the scenario's declaration sequence at the subject and asserts each step's
verdict equals the `judge_declaration` oracle's verdict for that prefix:

| Id | Asserts |
|---|---|
| **A1** | A clean declaration is **accepted** — a well-formed `accepted` event echoing the command's `correlation_id`, carrying the declaring service. |
| **A2** | **Registry state carries across declarations.** The only multi-step scenario: service-A declares a scope → **accepted** and *stays* accepted, then a second service re-declares that A-owned key → **rejected**. Its distinct value is proving the subject's registry state persists between declarations within a run. The rejection reason is `scope_prefix_mismatch` (a key prefixed by service-A can only be declared by service-A); the `scope_owned_by_another_service` branch is **not reachable** via the domain handler — see the Why table. |
| **A3** | A declaration with the same scope key listed twice is **rejected** with `duplicate_scope_in_declaration`. |
| **A4** | A declaration whose scope key is not prefixed by the declaring service is **rejected** with `scope_prefix_mismatch`. |
| **A5** | A declaration with a charset-invalid scope key is **rejected** with `invalid_scope_key` (nesting `KeyValidationError::InvalidCharset`). |
| **A6** | The same service re-declaring the same scopes is **accepted again** — idempotent, no false conflict. |
| **A7** | A declaration with a **structurally malformed** scope key (no `:` separator) is **rejected** with `invalid_scope_key` nesting `KeyValidationError::MalformedSegments` — closing the oracle loop on the frozen subject's `malformed_segments` validation path (A5 only covers `invalid_charset`). |

The acceptor keeps an in-memory `scope_key → owning_service` registry **across**
declarations within a run — state is the whole point: the runner seeds an ownership
context by driving a prior accepted declaration, then drives the contested one.

## Public helper surface (the reusable battery)

- `IdentityHarness` — `start()` locates the prebuilt `identity-acceptor` binary
  (`locate_subject`, see "The subject binary") and is the self-test entry point; `start_with_binary(binary)` runs the **same** battery
  against any acceptor binary, so a consuming Identity service drives it against its
  own. It stands up a `br_test_harness::FabricTestNats`, which provisions the two
  **fixed** Project-NATS-Fabric streams — `INTEGRATION_CMD` (`integration.cmd.>`) and
  `INTEGRATION_EVT` (`integration.evt.>`) — up front (the platform never
  auto-provisions, so the harness must). `command_stream_name()` /
  `event_stream_name()` / `declarer()` / `capture_confirmations()`.
- `Declarer` — builds a real `IntegrationCommand<DeclareServiceScopes>` and publishes
  it (ack-confirmed) via `br_util_nats_fabric::Fabric::publish_command` with the typed
  `br_scope_declaration_contract::declare_command_coords()` — no freestyle subject.
- `ConfirmationCapture` — a thin typed view over the harness `EventCapture`
  (a **replaying** background-drain consumer, `DeliverPolicy::All`,
  `AckPolicy::None`) filtered to both event subjects;
  `confirmation_decoded_for(correlation_id)` decodes the reply by `from_slice` into the
  real `IntegrationEvent<…>` types and returns a `DecodedConfirmation { service, verdict }`
  — the type-oracle check, and the authoritative decode: it keeps the service of a
  rejection. `verdict_for` is the `Verdict`-level view of the same decode; `Verdict::Rejected`
  carries no service, so it drops it.
- `oracle::expected_step_confirmations` — replays a declaration sequence through the
  real `judge_declaration` / `ScopeRegistry` and returns the per-step
  `DecodedConfirmation { service, verdict }`. Together with
  `ConfirmationCapture::confirmation_decoded_for` it is the authoritative pair:
  `run_scenario` compares exactly these two, so a rejection naming another service
  fails. The `Verdict`-level helpers — `verdict_for`, `expected_verdict`,
  `expected_step_verdicts`, `outcome_to_verdict` — drop the service of a rejection
  (`Verdict::Rejected { reason }` has none): a check built only from them accepts a
  rejection that names another service, so do not build a check from them.
- `Scenario` (A1–A7) with `sequence(namespace)` — the declaration sequences,
  namespaced so each run uses unique service/scope keys.
- `Subject` / `SubjectConfig` — spawn the built binary with its env wiring
  (`NATS_URL`, `HTTP_ADDR`, `SCOPE_ACCEPTANCE_ENABLED`) and poll
  `/readyz` / `/livez`. The acceptor hardcodes the command stream
  (`INTEGRATION_CMD`) and reads no `STREAM_NAME`, so the harness injects none.
- The three subject derivers (`declare_subject` / `accepted_event_subject` /
  `rejected_event_subject`) render the fixed six-segment grammar from the typed
  `br-scope-declaration-contract` coords through `br_util_nats_fabric`.

## The subject binary

The tests never compile anything. They assume the Go acceptor is already built and look
for an executable named `identity-acceptor` (`SUBJECT_BINARY`), in this order:

1. on `PATH` (absolute entries only);
2. in the cache directory `$XDG_CACHE_HOME/br-e2e-harness/conformance-identity-{version}`
   (`~/Library/Caches/...` on macOS and `~/.cache/...` on Linux when `XDG_CACHE_HOME` is
   unset; a relative `XDG_CACHE_HOME` or `HOME` is ignored, and without an absolute one
   there is no cache directory and the lookup ends in the typed error);
3. downloaded from this crate's own GitHub release,
   `BotResources/br-e2e-harness` tag `conformance-identity/v{version}`, asset
   `identity-acceptor-<os>-<arch>.tar.gz` with its sibling `.sha256`, into that cache
   directory.

Platforms with a release asset: `linux` and `darwin`, on `amd64` and `arm64`. The download
fails closed: the archive must match the SHA-256 published beside it, every redirect must
stay on an https `github.com` / `objects.githubusercontent.com` /
`release-assets.githubusercontent.com` host, and an archive over 64 MiB is refused. Only the
`identity-acceptor` entry is extracted, and only if it is a regular file, then marked
executable. The archive is extracted with the `tar` found on the absolute entries of `PATH`.
The lookup, the download and this crate's own tests write nothing to the system temporary
directory (the throwaway NATS store of `br-test-harness` is that crate's own).

`start_with_binary(binary)`, `SpawnTarget { binary }` and `Subject::spawn` accept a bare name such as
`identity-acceptor`: a path with no directory part is resolved on the absolute entries of `PATH` only, never
in the current directory, and the absolute result is checked and spawned. Found nowhere, the two
Result-returning entry points (`start_with_binary`, `run_spawn`) return `SubjectBinaryUnavailable`;
`Subject::spawn` returns `Self`, so it panics with that error's text instead, and an unresolved name
never reaches the operating system. A path with a directory part (`./identity-acceptor`, an absolute path)
is used as given.

`locate_subject()` returns the absolute path of the binary. The acceptor is spawned by
that path, and `Subject::spawn` hands the child a `PATH` led by the directory of the binary
when that directory is absolute, through the child's environment only: the crate never calls
`std::env::set_var`.

When none of the three succeeds, the call returns `ConformanceError::SubjectBinaryUnavailable`
naming the binary and what to do: put it on `PATH`, or build it yourself with
`make -C conformance-subjects/identity-acceptor build` and put
`conformance-subjects/identity-acceptor` on `PATH`. The dead-grammar guard of the subject
(`make guard`) and its Go tests run in CI, not in this crate.

## The single-implementation check API

Each A1–A7 check shares **one** implementation (`run_judged_scenario`), returning a
structured `CheckOutcome` instead of panicking. Both `tests/conformance.rs` and the
programmatic runners call it.

- `CheckContext<'a>` — a `Declarer`, a `ConfirmationCapture`, the per-run
  `namespace`, and a per-step `timeout`.
- `CheckId` / `CheckStatus { Pass, Fail, Skipped }` / `CheckOutcome { id, status,
  expected, observed, detail }` / `ConformanceReport { outcomes }` with
  `passed()` / `failed()` / `skipped()` / `is_conformant()`. `is_conformant()` is true
  only for a report with at least one outcome, no failed and no skipped outcome: an
  empty report, a skipped check or a failed check proved nothing.
- A check compares the confirmation to the lib oracle on the verdict **and** on the
  service the confirmation names (`DecodedConfirmation`); a rejection that names another
  service than the declaring one fails. The `expected` text of a failing outcome is the
  oracle verdict of the failing step, and `observed` is rendered from the captured
  confirmation.
- A missing prerequisite is a typed `ConformanceError`, never a panic: no scenario
  selected (`InvalidInput`), a path handed to `start_with_binary` / `run_spawn` that is
  not an executable file (`InvalidInput`), no `identity-acceptor` binary on `PATH`, in the cache or
  in the release (`SubjectBinaryUnavailable`), `nats-server` not on `PATH`, the
  `fabric-nats` CLI not built beside the test executable, a harness start or an
  unreachable NATS (`Jetstream`). A harness panic while observing a scenario is contained and reported as a failed check.
  `ConfirmationCapture::verdict_for` and `confirmation_decoded_for` return `Some(Err(Jetstream(..)))` when the capture recorded a malformed frame or the NATS client is gone; `try_confirmation_for` and `try_count` are the non-panicking forms of `confirmation_for` and `count`, which panic in those cases.

## Two programmatic runners — spawn and attach

- `run_spawn(SpawnTarget { binary }, scenarios, timeout)` — the core deliverable.
  Stands up a `FabricTestNats` (the two fixed fabric streams), launches the acceptor
  binary against `INTEGRATION_CMD`, waits for `/readyz=200`, and runs the full
  `a1..a7`. Needs `nats-server` on `PATH`.
- `run_attach(AttachTarget { nats_url, readyz_url }, scenarios, timeout)`
  — connects to an already-running Identity service's NATS and polls its `/readyz`
  directly; never spawns `nats-server` and never builds anything. It does **not** create the
  streams — the deployment owns `INTEGRATION_CMD` and `INTEGRATION_EVT`; the runner attaches
  without provisioning, checks that both are present and returns a typed
  `ConformanceError::Jetstream` naming them if either is absent. **Caveat:** driving declarations
  **mutates a live registry**; the runner uses **unique per-run service/scope keys**
  (a fresh UUIDv7 namespace) to bound pollution, so A1–A7 are safe to run repeatedly
  against the same live service.
  **New in 0.1.1 (through `br-test-harness` 0.1.2):** the confirmation capture replays the
  whole history of `INTEGRATION_EVT` for the two confirmation subjects (`DeliverPolicy::All`)
  and stops at the first frame that is not a JSON envelope with a readable correlation id.
  It fails closed and records the cause, and every later read of the capture fails: all
  seven checks then fail with the cause in the detail. One such frame already in a live stream
  therefore fails A1–A7 in every later run until the stream is purged (0.1.0 skipped the
  frame). Spawn mode is not affected: its broker is fresh.

## Running it

Needs `nats-server` and the `identity-acceptor` binary on `PATH` (see "The subject
binary") and the `fabric-nats` CLI built in the same target directory. The tests are `#[ignore]`-gated so the default `cargo test` stays green
without infra:

```sh
make -C conformance-subjects/identity-acceptor build
export PATH="$PWD/conformance-subjects/identity-acceptor:$PATH"
cargo b -p br-test-harness --bin fabric-nats --features nats-fabric
cargo test -p conformance-identity --test conformance -- --ignored
```

The suite **provisions its scope-declaration topology by spawning the
`fabric-nats` CLI** (`fabric-nats provision --manifest
tests/fixtures/scope_declaration.toml`) — the conformance suite is the CLI's
real-life testbed. The crate carries **no `async-nats` dependency**: the
confirmation capture is a thin typed view over the harness `EventCapture`, the
declarer publishes through a `Fabric`, and attach mode connects via
`FabricTestNats::attach_without_provisioning`.

CI runs `cargo test -p conformance-identity --all-targets -- --ignored --test-threads=1`
in the `infra-e2e` job. A workflow step builds `identity-acceptor` onto `PATH` after
the Go checks of the subject (the dead-grammar guard, the vet and the Go tests); `nats-server` is
installed on the runner (shared with G3).

## Run in CI

A service in any language runs this battery with the published `conformance-identity`
CLI ([`conformance-identity-cli`](../conformance-identity-cli/README.md)), with no Rust to
write: download the release binary at a pinned version, build the acceptor, run the CLI
against it. The archive holds `conformance-identity` and `fabric-nats`; keep them together.
The runner needs `nats-server` and nothing else (no Go).

```yaml
- name: download the identity conformance CLI
  run: |
    set -euo pipefail
    version=0.1.1
    asset="conformance-identity-${version}-x86_64-unknown-linux-musl.tar.gz"
    base="https://github.com/BotResources/br-e2e-harness/releases/download/conformance-identity/v${version}"
    curl -fsSLO "${base}/${asset}"
    curl -fsSLO "${base}/${asset}.sha256"
    sha256sum -c "${asset}.sha256"
    mkdir conformance-bin
    tar -xzf "${asset}" -C conformance-bin

- name: build the acceptor
  run: cargo build --release --locked

- name: scope-acceptance conformance battery
  run: ./conformance-bin/conformance-identity run --spawn target/release/my-identity --format junit --output identity-conformance.xml
```

`nats-server` must be on `PATH`. A non-zero exit code fails the job; the CLI README lists
the exit codes, the attach mode and the macOS asset.

## Install

A **dev-dependency**, pinned to a release tag (git-tag distribution; no crates.io).
Its public signatures name types of five sibling libraries: `br-test-harness`
(`FabricTestNats`, `CapturedMessage`, taken by `ConfirmationCapture::start` and the
`From<CapturedMessage>` conversion), `br-util-nats-fabric` (`Fabric`, taken by
`Declarer::new` and returned by `IdentityHarness::fabric`), `br-core-integration`
(`CoordError`, and the event types through its own pins), `br-core-scope`
(`DeclareServiceScopes` and the declaration types) and `br-identity-domain`
(`DeclarationOutcome`, taken by `oracle::outcome_to_confirmation`). Pin each of them with
the same git url and the same tag string as this crate, or the build holds two copies of
the type and fails with a type mismatch:

```toml
[dev-dependencies]
conformance-identity = { git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-identity", tag = "conformance-identity/v0.1.1", version = "0.1.1" }
br-test-harness = { git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.2", version = "0.1.2" }
br-util-nats-fabric = { git = "https://github.com/BotResources/br-rust-common", package = "br-util-nats-fabric", tag = "br-util-nats-fabric/v0.1.0", version = "0.1.0" }
br-core-integration = { git = "https://github.com/BotResources/br-rust-common", package = "br-core-integration", tag = "br-core-integration/v0.1.0", version = "0.1.0" }
br-core-scope = { git = "https://github.com/BotResources/br-rust-common", package = "br-core-scope", tag = "br-core-scope/v0.1.0", version = "0.1.0" }
br-identity-domain = { git = "https://github.com/BotResources/br-rust-common", package = "br-identity-domain", tag = "br-identity-domain/v0.1.0", version = "0.1.0" }
```

## Why — the non-obvious bits

| Thing | Why it is the way it is |
|---|---|
| **A2 proves cross-declaration *state carry*; its rejection reason is `scope_prefix_mismatch`, not `scope_owned_by_another_service`** | A2 is the only multi-step scenario, and that is its point: a prior declaration is accepted and **stays** accepted while a later declaration is rejected, proving the subject's `scope_key → owner` registry persists across declarations in a run. The reason is **not** an ownership conflict: the oracle is `judge_declaration`, which calls `command.validate()` **before** the registry's cross-owner check, and a key prefixed by service-A can only be declared by service-A (the prefix rule), so the reclaim is rejected at validation as `scope_prefix_mismatch` and never reaches the `ScopeOwnedByAnotherService` branch. That branch is real but unreachable via the domain handler — in production it is produced by the app-layer pipeline from a `UNIQUE(scope_key)` DB conflict. The unit test asserts A2's step verdicts explicitly (`[0]` Accepted, `[1]` prefix-mismatch Rejected) so the state-carry value is proven, not implied. |
| **The battery does not test service impersonation** | The acceptor trusts the manifest's self-asserted identity. The prefix rule is a coherence check, not authentication. Impersonation is out of the threat model: the trust boundary is the deployment scope — only first-party services run in it (Infra ARCHITECTURE.md, "the trust boundary is the scope itself"); the NATS bus runs without per-service auth today. G2 therefore does not test impersonation — it is an infrastructure property, not a domain one. |
| The verdict check is `from_slice` into the real `IntegrationEvent<…>`, with **no** extra no-extra-fields guard | Deser success against the real envelope + payload *is* the conformance check; a stricter hand-rolled guard would be a second contract that drifts from the types — exactly what this crate exists to prevent. |
| Scenarios carry **raw** `DeclareServiceScopes`, built from `RawScopeDeclaration` | A3/A4/A5 are intentionally malformed declarations that `ScopeDeclaration::new` would refuse to construct. The wire payload is `RawScopeDeclaration`, so the runner builds the raw shape and feeds the **same** raw payload to both the subject and the oracle — the oracle's `judge_declaration` runs the same validation the subject must. |
| The runner's declare metadata uses `Actor::Service` | A boot-time scope declaration is service-initiated; the real declarer (`br-util-scope-declaration`) emits `actor_kind:"service"`. The acceptor matches on `correlation_id` only and never validates the actor. |
| `ConfirmationCapture` starts **before** the subject is spawned | The capture and the acceptor both consume with `DeliverPolicy::All`, so nothing published early is missed; the order keeps the capture armed for the whole run and lets a boot failure surface as a `Timeout` carrying the subject logs. |
| The runner waits for `/readyz=200` **before** declaring | A subject that never reports ready is a `Timeout` with its logs, not a declare that goes unanswered. |
| Keys are **namespaced per run** with a UUIDv7 | spawn mode is isolated anyway, but the same scenarios drive an attach run against a live, stateful registry; unique keys keep re-runs from colliding with each other or with prior state, and bound the pollution of a real registry. |
| **attach drives all A1–A7, not a read-only subset like G3** | Unlike G3, whose attach default skips the scenarios that would stall a live service (timeout/re-publish, rejection-blocks-readiness), no G2 scenario durably mutes or blocks the acceptor — every one is a one-shot declare→verdict. With per-run key namespacing bounding the pollution, all seven are safe to drive repeatedly against a live registry, so `attach_default()` is the full set. One limit, new in 0.1.1 through `br-test-harness` 0.1.2: the capture replays the whole stream history and stops, fail-closed with the cause named, at the first non-envelope frame on the two confirmation subjects, so such a frame already in a live stream fails all seven checks in every later run until the stream is purged. Spawn mode is not affected (fresh broker). |
| The Go subject is a faithful reimpl of `judge_declaration`, not a binding | It is the tautology-break: an independent encoding of the wire + policy. If it merely called the lib, green would prove nothing. Its Go unit tests pin the rejection wire shapes to the frozen `scope-wire-v1` golden JSON. |
| `provision` embeds its manifest and writes it to the cache directory when the source tree has none | `CARGO_MANIFEST_DIR` is the source tree of the machine that compiled the crate; a released CLI binary runs elsewhere, where `tests/fixtures/` does not exist. `fabric-nats provision` reads a file, so when the source-tree file is absent (a released binary) the embedded fixture is written once under `$XDG_CACHE_HOME/br-e2e-harness/conformance-identity-{version}/manifests/` (never the system temporary directory) and replaced when its content differs. |
| The tests never compile and the binary comes from `PATH`, a cache or a verified release asset | Compiling in a test hid the toolchain, wrote a binary to the system temporary directory and made the importer's test run depend on a Go build. A prebuilt binary found by name is the same artifact the release publishes, and a missing one is a typed error that says what to do. |
| The whole battery is `#[ignore]`-gated | It drives real infra (`nats-server` + the prebuilt `identity-acceptor` binary); the default `cargo test` must stay green on a machine without them, exactly like `br-test-harness`'s own self-tests. |

[`conformance-scope`]: ../conformance-scope/README.md

## License

Apache-2.0. MSRV **1.88** (edition 2024).

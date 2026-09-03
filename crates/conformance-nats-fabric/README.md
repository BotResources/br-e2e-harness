# conformance-nats-fabric

A **black-box conformance runner** for the BotResources **NATS Fabric** — the v1
integration subject grammar (`integration.{cmd,evt}.{bc}.{aggregate}.{verb,fact}.vN`)
and the **published-language** KV (`PUBLISHED_LANGUAGE`). It drives the real
`br-util-nats-fabric` against a real `nats-server` (via `FabricTestNats`) and
anchors the wire against an **independent Go renderer** in
`conformance-subjects/nats-fabric`.

> ⚠️ **TEST FIXTURE ONLY.** Add it as a **dev-dependency**, never a runtime one.
> It builds and runs a real Go binary and stands up a real `nats-server` on an
> isolated, throwaway test network.

## lib-as-oracle / Go-as-anchor

The Go anchor **freezes the wire independently of the lib**: it renders the v1
subject grammar and the published-user KV shape by hand, with **no dependency on
`br-rust-common`**. The Rust runner then re-derives the same wire **through the
real lib types** and asserts they agree:

- **Subjects** — the runner builds `CommandCoords` / `EventCoords` from the
  anchor's segments and renders them with the lib's `command_subject` /
  `event_subject`; the result must equal the anchor's rendered subject
  **byte-for-byte**. If the lib's renderer drifts, the comparison fails.
- **Published-language values** — the runner deserializes the anchor's frozen
  user JSON **through `br_core_directory::PublishedUser`**. Deserialization
  succeeding *is* the shape check; a drifted/retyped field fails the deser.

The anti-drift mechanism is the **Go anchor's independence**, never a mirrored
copy of the lib's view in the test. A `make guard` target fails the anchor build
if the **dead `identity.cmd.` / `identity.evt.` grammar** ever appears in it.

## What it proves

### Integration messaging

- A durable **widened** to `integration.evt.>` (not the exact coordinate subject)
  is **narrowed back** to the exact coordinate filter when the lib create-or-binds
  it (`ensure_event_durable` returns `Ok`); reading the durable's effective filter
  back proves the widening was undone — a consumer cannot silently over-subscribe.
- A **missing fixed stream** fails loud on **both** lib paths — the
  `verify_*_durable` coverage probe *and* the `ensure_*_durable` durable bind —
  with `FabricError::Consume(NoStream)`, and the stream is still absent
  afterwards: the platform never auto-provisions it.
- The anchor's rendered subjects match the lib renderers **byte-for-byte**.
- The **dead `identity.*` grammar fails loud**: a publish on `identity.cmd.*`
  lands on no fixed stream (`PublishErrorKind::NoStream`) and no `INTEGRATION_*`
  stream captures it — no fallback, no silent drop.

### Published language (KV)

- `retract` **orphan-deletes** the key.
- `reconcile` **converges** drift: adds the missing, repairs the changed,
  deletes the orphaned.
- `bootstrap` is **parallel-safe** under a concurrently running `watch`: it
  projects the published seed and retracts a sink orphan with the watcher live.
- A live slash-keyed put is **delivered by `watch` within the deadline** —
  `prefix_watch_delivers_slash_keyed_directory_puts` asserts this on real infra
  (fixed in `br-util-nats-fabric` v1.0.1: the watch subject now matches the
  slash-delimited key scheme correctly).
- A malformed KV value **fails closed and names the offending key** in the
  `FabricError::Decode` error.

## Running

Add it as a dev-dependency, pinned to the same harness tag as `br-test-harness`:

```toml
[dev-dependencies]
conformance-nats-fabric = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v1.2.0" }
```

The real-infra tests in `tests/conformance.rs` are `#[ignore]`-gated and require
a `nats-server` on `PATH` and a `go` toolchain:

```sh
cargo test -p conformance-nats-fabric --test conformance -- --ignored
```

The published-language tests **provision their KV topology by spawning the
`fabric-nats` CLI** (`fabric-nats provision --manifest
tests/fixtures/published_language.toml`) rather than calling
`FabricTestNats::with_published_language()` in-process — the suite is the CLI's
real-life testbed. The crate carries **no `async-nats` dependency**: it observes
and mutates the fabric exclusively through the typed `FabricTestNats` surface
(`fabric_owned`, `pl_reader` / `pl_publisher` / `pl_put_raw`,
`assert_missing_stream` / `publish_dead_subject` / `raw_message_absent`).

CI runs this battery in the `infra-e2e` job (which installs `nats-server` on
`PATH` and provides a `go` toolchain), so the real-infra gate is enforced on
every PR, not just documented.

The non-infra unit tests (byte-for-byte renderer agreement, filter shape) run by
default:

```sh
cargo test -p conformance-nats-fabric
```

## Why table

| Thing | Why it is the way it is |
|---|---|
| Anchor renders subjects by hand, no `br-rust-common` dep | The Go side must freeze the wire **independently** of the lib; a shared dependency would let both drift together and blind the detector. |
| Runner re-derives through the lib types | The lib is the **oracle**: byte-for-byte subject equality and `PublishedUser` deser are the drift detectors. |
| `make guard` greps for `identity.cmd.`/`identity.evt.` | The dead pre-v1 grammar must never reappear in the live-wire anchor; `build_anchor` runs `make -C <dir> guard` before `go build` and fails loud on a hit, so the source-level guard runs on the real test path (in addition to the runtime fail-loud probe that proves the fabric rejects the dead subject). |
| Real-infra tests `#[ignore]`-gated | Matches the harness convention: CI installs `nats-server` on `PATH` in the `infra-e2e` job and runs the battery with `--ignored`; the default `cargo test` stays infra-free. |
| Each PL run is key-namespaced by `FabricTestNats::key_prefix()` | The shared `PUBLISHED_LANGUAGE` bucket is reused across parallel runs; the run-id prefix keeps reconcile/orphan scopes from colliding. |

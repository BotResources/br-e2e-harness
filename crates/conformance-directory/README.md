# conformance-directory

A **conformance runner** for the BotResources **identity Published Language**
(the directory KV projection) — the **Px** publisher and **Cx** consumer
batteries, plus the offline **wire-deser gate** that guards the `br-core-directory`
types against the frozen Go anchor.

> ⚠️ **TEST FIXTURE ONLY.** Add it as a **dev-dependency**, never a runtime one.
> The Px/Cx batteries stand up a real `nats-server` and a real Postgres on an
> isolated, throwaway test network; the wire gate builds and runs a real Go
> binary.

The on-wire contract these batteries enforce is documented in
[`../../docs/conformance/directory-wire-v1.md`](../../docs/conformance/directory-wire-v1.md),
authoritative for the shapes; this README describes how the runner exercises them.

## Three roles, never conflated

Per the platform wire-conformance doctrine — **Go freezes the wire · the
e2e-harness test imports the lib as oracle · conformance guards the
implementations**:

- **`conformance-subjects/identity-directory`** (Go) — an **independent
  re-implementation** of the directory wire. It prints the canonical KV snapshot
  to stdout and **never imports `br-core-directory`**. Its independence is what
  makes the detector trustworthy.
- **the wire gate (this crate)** — deserialises every Go-frozen value **through
  the real `br-core-directory` types**. A successful deserialise *is* the
  wire-shape check; a lib drift (a renamed/retyped core field, a changed serde, a
  broken `flatten`) makes the deserialise **fail** → red. This guards the lib.
- **Px / Cx (this crate)** — drive the real `br-util-directory` kit
  (`DirectoryPublisher` / `DirectoryProjector`) against real infra to prove the
  publisher and consumer implementations honour the contract.

## The batteries

### Wire-deser gate — W1–W5 (offline, the gate)

Builds + runs the Go anchor, then for each emitted `{key, value}` entry
deserialises the `value` through the lib oracle. **Offline** — no NATS, no PG;
only `go` on `PATH`. This is the gate that validates the lib's wire before its
tag is cut.

| Id | Asserts |
|---|---|
| **W1** | every `users/{uuid}` value deserialises through `br_core_directory::PublishedUser`. |
| **W2** | every `groups/{uuid}` value deserialises through `PublishedGroup`, and a memberless group proves `member_ids` is an array (never absent). |
| **W3** | `identity/_meta` deserialises through `DirectoryMeta` declaring users + groups. |
| **W4** | the neutral extension `x_custom` lands in `extensions` (flat), never leaking into a core field — the `#[serde(flatten)]` works. |
| **W5** | a users-only manifest deserialises and **auto-degrades** groups (`publishes_groups() == false`). |
| **W6** | an extensions map shadowing a reserved core key (`email`) is **rejected at `PublishedUser` construction** with `DirectoryError::ReservedExtensionKey` (fail-closed), never a silent overwrite. |

The same logic has pure unit tests (`src/wire.rs`, inline JSON mirroring the
anchor shape) so plain `cargo test` is green with **zero** toolchain.

### Px — publisher conformance (real NATS KV)

Drives `br_util_directory::DirectoryPublisher` (opened on the harness `Fabric`)
against the fixed `PUBLISHED_LANGUAGE` KV bucket, which `DirectoryHarness` provisions
by spawning the `fabric-nats` CLI on its throwaway NATS (a `[published_language]`
manifest) — the same CLI handshake every conformance suite uses, no in-process bucket
creation. KV reads go through the harness's typed `pl_list` / `pl_get_meta` surface.

| Id | Asserts |
|---|---|
| **P1** *(mandatory floor)* | `reconcile` writes `_meta` + every user; the published user wire round-trips identically to the source **through the lib types**; a second reconcile applies the **empty diff** (idempotent); dropping a user **orphan-deletes** its KV key (PII propagation). |
| **P2** *(optional)* | a users-only source publishes **no** group keys and a `_meta` that omits groups — groups are gated on `_meta`. |

### Cx — consumer conformance (real NATS KV + real PG)

Drives `br_util_directory::DirectoryProjector` (KV→PG) and the
`DirectorySnapshot` readers. **All opt-in, none mandatory** — a service may
consume nothing.

| Id | Asserts |
|---|---|
| **C1** | reconcile-on-boot projects KV users into PG; a `DirectorySnapshot` loaded from `known_users` resolves the carried user (`resolve_user`); retracting a KV user **orphan-deletes** its projection row. |
| **C2** | with groups in `_meta`, `is_member` / `group_name` resolve from the projected `known_groups` / `known_user_group`; with a users-only `_meta` they **auto-degrade** to empty. |
| **C3** | a published user carrying an extension the consumer's `extract_user_extensions` selects is projected into `known_users.extensions` and read back **intact** — the sink is lossless and never force-drops extensions. |
| **C4** | a user passing `filter_users` is projected; republishing it so it **fails** the filter makes the next `reconcile` **orphan-delete** its row (the copy filter is re-evaluated, a flip retracts). |
| **C5** | a `ConsumptionScope::UsersOnly` consumer against a schema that **lacks** the group tables reconciles, then **watches** a live PUT: a fresh user published into the bucket while `watch()` runs is projected into `known_users` (polled to a deadline), and the group tables still do **not** exist — so the scope narrows a *live* change with **no** group DML (any group write would error on the missing tables). Because directory keys are slash-delimited, the live PUT also exercises `br-rust-common` v1.0.1's `watch_all` + client-side prefix filter on real NATS. |

## Public helper surface (the reusable battery)

The check functions return a structured `CheckOutcome` rather than panicking, so a
consuming service's e2e can call them directly:

- `build_and_emit()` — builds the Go anchor and runs it once, returning the
  parsed `DirectorySnapshotWire`. `build_anchor()` / `emit_snapshot(binary)` split
  the two steps.
- `run_wire_battery(&snapshot) -> ConformanceReport` — the W1–W5 offline gate;
  `deserialize_user` / `deserialize_group` / `deserialize_meta` are the per-entry
  oracle deser, each erroring with the offending KV key and the lib type it failed
  to deserialise into.
- `publisher_floor(&snapshot)` / `publisher_groups_optional(&snapshot)` — the Px
  battery against a throwaway `DirectoryHarness` (a `FabricTestNats` whose
  `PUBLISHED_LANGUAGE` bucket is CLI-provisioned at `start`).
- `consumer_reads_users(&snapshot)` / `consumer_reads_groups(&snapshot)` — the Cx
  battery against a `DirectoryHarness` + a `ConsumerDb` (an `E2eDatabase` + a
  migrated pool).
- `extension_survives_projection(&snapshot)` / `filter_flip_orphan_deletes(&snapshot)`
  — the C3/C4 extension + copy-filter Cx scenarios, each driving a
  `DirectoryProjector::with_config` (a `DirectoryConsumerConfig` with a custom
  `extract_user_extensions` / `filter_users`).
- `users_only_narrows_projection(&snapshot)` — the C5 `ConsumptionScope::UsersOnly`
  scenario against a `ConsumerDb::apply_users_only_schema()` (a schema variant that
  omits the group tables; `group_tables_exist()` probes their absence).
- `reserved_key_rejected()` — the W6 offline guard; returns a `CheckOutcome` (no
  infra), asserting `PublishedUser::new` fails closed on a reserved-key extension.
- `AnchorSource` — a `DirectorySource` built **from the anchor snapshot** (its
  users/groups are the Go-frozen values re-deserialised through the lib), so the
  wire the publisher emits is exactly the frozen anchor shape; `.without_groups()`
  drops groups to drive P2's manifest-gating.

## Running it

The offline unit tests run with plain `cargo test -p conformance-directory`. The
real batteries are `#[ignore]`-gated:

```sh
# wire gate (needs `go`)
cargo test -p conformance-directory --test conformance \
  -- --ignored wire_battery

# the full Px + Cx + wire suite (needs `go`, `nats-server`, and a Postgres)
NATS_URL=nats://localhost:4222 \
E2E_PG_ADMIN_URL=postgresql://postgres@localhost:5432/postgres \
cargo test -p conformance-directory --test conformance -- --ignored --test-threads=1
```

`E2E_PG_ADMIN_URL` (or `DATABASE_URL`) must point at a Postgres where the role can
`CREATE ROLE` / `CREATE DATABASE` — the harness provisions a throwaway owner role
and database per Cx test and drops them on cleanup.

CI runs the full `--ignored` battery (`--test-threads=1`) in the `infra-e2e` job,
which provides the Postgres service container, `nats-server` on `PATH`, a `go`
toolchain, and `E2E_PG_ADMIN_URL` — so the real-infra gate is enforced on every
PR, not just documented.

## Install

A **dev-dependency**, pinned to a release tag (git-tag distribution; no
crates.io). Keep its `br-rust-common` tag identical to `br-test-harness`'s so
Cargo resolves a single source and never duplicates `br-core-*`:

```toml
[dev-dependencies]
conformance-directory = { git = "https://github.com/BotResources/br-e2e-harness", tag = "v1.0.1" }
```

## Why — the non-obvious bits

| Thing | Why it is the way it is |
|---|---|
| The shape check is `from_value` into the real `br-core-directory` type, with **no** extra no-extra-fields / round-trip guard | The owner's ruling (the scope/passport precedent): deser success against the real type *is* the conformance check. A stricter hand-rolled guard would be a second contract that drifts from the types — exactly what this crate exists to prevent. |
| The wire gate is offline (no NATS / no PG), only the Go anchor | The directory PL is a one-way KV projection, not a live handshake. WU8 froze the **bytes**; this gate pairs them against the lib types. A *live* projection is the Px/Cx batteries, a separate concern against real infra. |
| Px/Cx drive the **lib kit**, not a spawned Go service | Unlike scope/passport (live black-box services over NATS/HTTP), the publisher/consumer logic *is* `br-util-directory`. There is no external service to spawn; the conformance is the lib kit's behaviour against real infra. The Go anchor stays the wire freeze. |
| `AnchorSource` rebuilds its `PublishedUser`/`PublishedGroup` from the **anchor snapshot** | So the wire the publisher emits is the Go-frozen shape, not a hand-rolled fixture — the Px round-trip equality (`published == source`) is then a faithful published-wire ⇄ frozen-wire check through the lib. |
| Cx loads a `DirectorySnapshot` from the projected `known_*` rows, then exercises the readers | The kit's `DirectoryProjector` writes KV→PG; the readers (`resolve_user` / `is_member` / `group_name`) live on `DirectorySnapshot`, which a consumer service builds from its projection. Cx mirrors that: project to PG, read PG back into a snapshot, assert the readers. |
| Cx uses the `E2eDatabase` owner role for both migration and projection | The directory projection store carries no row-ownership dimension (reference data, not RLS-gated), so there is no second runtime role to model. The owner is a `NOSUPERUSER` least-privilege role; the projector reconciles as it. |
| W6 (reserved-key) drives `PublishedUser::new`, not a wire deser | The deser path consumes the three reserved core keys (`email` / `first_name` / `last_name`) out of the flatten bag *before* `new`, so a wire value can never leave a reserved key in the residual extensions — the guard is structurally unreachable via deser. It exists to defend the **publisher's** direct constructor path, which is exactly what W6 exercises (fail-closed, never a silent overwrite). |
| C5 (UsersOnly) proves the narrowing by a schema that **lacks** the group tables | A `UsersOnly` consumer that merely *suppressed* output would pass even against a full schema. Dropping the group tables turns any stray group DML into a hard error, so a green C5 proves the scope genuinely never opens the group consumer — the narrowing is real, not cosmetic. `watch()` is run under a short `timeout` (a live subscription never returns on its own); the assertion is that it does not error and materialises no group tables within the window. |
| The whole real battery is `#[ignore]`-gated | It drives real infra (`go` + `nats-server` + a Postgres); the default `cargo test` must stay green on a machine without them, exactly like the scope/passport conformance crates. |

## License

Apache-2.0. MSRV **1.88** (edition 2024).

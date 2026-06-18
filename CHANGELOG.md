# Changelog

All notable changes to `br-e2e-harness` are documented here. The whole workspace
ships **one version**: every crate inherits `version.workspace = true`, and a
single git tag `v{version}` releases the set. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.

## [Unreleased]

### Added

- **`conformance-directory` — extension, copy-filter and `UsersOnly` Cx
  scenarios (#77, #78).** Four new directory checks drive the real
  `br-util-directory` consumer kit (`DirectoryProjector::with_config`) against
  real NATS KV + real Postgres: **C3** proves an extracted extension survives the
  projection into `known_users.extensions` losslessly (`extract_user_extensions`);
  **C4** proves a user passing `filter_users` is projected, then **orphan-deleted**
  on republish when it fails the filter (the copy filter is re-evaluated, a flip
  retracts); **C5** proves a `ConsumptionScope::UsersOnly` consumer against a
  schema that **lacks** the group tables reconciles + watches cleanly and emits no
  group DML — the missing `known_groups` / `known_user_group` turn any stray group
  write into a hard error, so the scope genuinely narrows the projection. **W6**
  (offline) proves an extensions map shadowing a reserved core key is rejected at
  `PublishedUser` construction with `DirectoryError::ReservedExtensionKey`
  (fail-closed), never a silent overwrite. New public helpers
  (`extension_survives_projection`, `filter_flip_orphan_deletes`,
  `users_only_narrows_projection`, `reserved_key_rejected`) and a
  `ConsumerDb::apply_users_only_schema` / `group_tables_exist` schema variant; the
  C3/C4/C5 real-infra checks are `#[ignore]`-gated per the existing convention.
- **`FabricTestNats` — a typed, drift-proof NATS Fabric provisioner** (new
  `nats-fabric` feature). It generalises the hand-rolled `NatsEnv` that every
  Fabric service used to copy into `tests/common/mod.rs`: `start()` spawns a
  per-process `SpawnedNats`, provisions the two **fixed** Fabric streams
  (`INTEGRATION_CMD` → `integration.cmd.>`, `INTEGRATION_EVT` →
  `integration.evt.>`), mints a UUIDv7 `run_id`, and hands back a ready `Fabric`.
  Durables are bound from **typed `CommandCoords` / `EventCoords`** through the
  lib's own `command_subject` / `event_subject`, so a durable's `filter_subjects`
  is byte-identical to what the lib binds — subject-grammar drift now breaks the
  bind. `.with_published_language()` is **get-or-create, never wiped** (the #73
  never-wipe posture). `.durable(logical)`, `.key_prefix()` and `.correlation()`
  namespace each run. **Negative-path helpers are first-class**:
  `.with_widened_durable(...)` proves `FabricError::FilterMismatch`, and
  `BareFabricNats::{without_fixed_streams, with_only_command_stream,
  with_only_event_stream}` start a server missing a fixed stream / the bucket to
  prove the lib bind fails loud and never auto-provisions.
- **`conformance-nats-fabric` — black-box conformance for the NATS Fabric** (#76):
  a new crate driving `br-util-nats-fabric` v1.0.0 against real NATS via
  `FabricTestNats`, anchored by a new **independent Go subject renderer** in
  `conformance-subjects/nats-fabric` (lib-as-oracle / Go-as-anchor; a `make
  guard` fails the anchor build if the dead `identity.cmd.`/`identity.evt.`
  grammar reappears). It proves: a **widened durable** is rejected with
  `FabricError::FilterMismatch`; a **missing fixed stream** makes the bind fail
  loud (no auto-provision); the anchor's rendered subjects match the lib's
  `command_subject`/`event_subject` **byte-for-byte**; the **dead `identity.*`
  grammar fails loud** (publish lands on no fixed stream, no `INTEGRATION_*`
  stream captures it); and for the published-language KV — `retract`
  orphan-deletes, `reconcile` converges drift, `bootstrap` is parallel-safe
  under a running `watch`, and a malformed value **fails closed naming the
  offending key**. Real-infra tests are `#[ignore]`-gated per the harness
  convention.

### Changed

- **The workspace pins `br-rust-common` `v1.0.1`** (was `v1.0.0`), across
  `br-test-harness` and every `conformance-*` crate. v1.0.1 fixes the
  `br-util-nats-fabric` prefix-`watch` watch-subject so that incremental `watch`
  over slash-delimited directory keys delivers live puts on a real `nats-server`.
  `prefix_watch_does_not_deliver_slash_keyed_directory_puts` is renamed to
  `prefix_watch_delivers_slash_keyed_directory_puts` and its assertion is flipped
  (was `!delivered`; now `delivered`) — the limitation is resolved, not
  worked around.
- **The workspace pins `br-rust-common` `v1.0.0`** (was `v0.11.1`), across
  `br-test-harness` and every `conformance-*` crate. This pin is a coordinated
  migration: under v1.0.0 the `conformance-identity`, `conformance-scope` and
  `conformance-directory` crates each move onto the new surfaces in their own
  migration slice (the `NatsIntegrationPublisher` / `IntegrationPublisherExt` /
  `SubjectError` and `accepted_subject` / `command_subject` / `rejected_subject`
  surface is removed in v1.0.0; `DirectoryPublisher::new` is removed and
  `KnownUser` gained an `extensions` field). The slices converge on the
  `feat/harness-v1.0.0` integration branch — not directly on `main`.
- **`conformance-passport` reads `Passport` through its typed getters** —
  `actor_id()`, `auth_method()`, `claims()` — instead of destructuring the
  `Passport::Human` / `Passport::Service` variants. The checks now bind to the
  stable accessor surface rather than the variant field layout, and the
  `passport-wire-v1.md` schema reference is stamped at `v1.0.0`.
- **`conformance-identity` is migrated onto the Project NATS Fabric.** The
  declarer now publishes through `br_util_nats_fabric::Fabric::publish_command`
  with the typed `declare_command_coords()` (the removed
  `NatsIntegrationPublisher` / `IntegrationPublisherExt` and the freestyle subject
  builder are gone). `IdentityHarness` provisions the two **fixed** Fabric streams
  via `FabricTestNats` instead of a per-run `IDENTITY` stream, the confirmation
  capture binds to `INTEGRATION_EVT`, and the subject derivers render the fixed
  six-segment grammar (`integration.{cmd,evt}.identity.service_scope.…`) from the
  contract coords. The frozen Go anchor (`identity-acceptor`) moves to the same
  fixed grammar so it remains an independent oracle of the v1.0.0 wire. The
  removed-`SubjectError` and per-run-`stream_name` surface drops accordingly
  (`AttachTarget` no longer carries `stream_name`; `DEFAULT_STREAM_NAME` →
  `COMMAND_STREAM_NAME` / `EVENT_STREAM_NAME`).
- **`conformance-scope` spawn path carries both handshake streams.**
  `SubjectConfig::new` now takes `(nats_url, command_stream, event_stream,
  service_key)` and the spawned subject receives both `COMMAND_STREAM_NAME`
  (`INTEGRATION_CMD`, where the declare lands) and `EVENT_STREAM_NAME`
  (`INTEGRATION_EVT`, where confirmations arrive) instead of a single
  `STREAM_NAME`. Under the v1.0.0 `integration.*` grammar the declarer publishes
  on `INTEGRATION_CMD` but awaits confirmations on `INTEGRATION_EVT`, which one
  env value cannot express; the two-stream `FabricTestNats` provisioning is now
  matched by a two-stream subject contract (no more two-stream harness wired to a
  one-stream subject).
- **`conformance-scope-cli` drops the `--stream` flag and the manifest
  `attach.stream` field.** Fabric streams are fixed constants in `v1.0.0`
  (`INTEGRATION_CMD` / `INTEGRATION_EVT`), so the handshake stream is no longer
  caller-choosable: the CLI binds the declare consumer to the fixed
  `INTEGRATION_CMD` command stream. `AttachTarget` no longer carries
  `stream_name`.
- **`conformance-directory` migrates onto the v1.0.0 directory API + the Fabric
  KV.** The kit no longer takes a raw `kv::Store`: `DirectoryPublisher::open(&fabric)`
  and `DirectoryProjector::new(fabric, pool)` are now opened on the harness
  `Fabric`. `DirectoryHarness` drops its ad-hoc `identity_directory` bucket
  (`DEFAULT_DIRECTORY_BUCKET` is removed from the public surface) and instead spins
  a `FabricTestNats::with_published_language()`, so the publisher/projector exercise
  the **fixed `PUBLISHED_LANGUAGE` bucket** the lib actually targets. The Cx
  `load_snapshot` reads back the new `known_users.extensions` jsonb column into
  `KnownUser { extensions: PersistedExtensions }`. Verified against real NATS +
  real Postgres (P1/P2/C1/C2 + the W1–W5 wire battery all green).
- **`conformance-scope::AttachTarget` drops its `stream_name` field.** With the
  fixed-stream grammar the attach-mode declare consumer binds the fixed
  `COMMAND_STREAM_NAME` (`INTEGRATION_CMD`) directly, so the handshake stream is
  no longer caller-supplied; this reconciles the `runner` surface with the
  `conformance-scope-cli` call sites that already stopped passing it.

### Fixed

- **`conformance-identity::SubjectConfig` drops its inert `stream_name` field
  and the `STREAM_NAME` env injection.** The frozen Go `identity-acceptor`
  hardcodes `commandStream = "INTEGRATION_CMD"` and never reads `STREAM_NAME`, so
  the injection was dead wiring; `SubjectConfig::new` now takes `(nats_url)` only.
  Verified by the full A1–A7 identity conformance battery (green against real
  NATS + the Go anchor).
- **`FabricTestNats::correlation()` doc no longer reads as a stable per-run
  value.** It mints a **fresh** UUIDv7 on every call (one per flux/message),
  unlike the `run_id`-derived `durable()` / `key_prefix()`; the README now states
  this so a caller cannot expect two calls to match (doc=code).
- **Renamed the durable-provisioning test
  `start_provisions_..._a_byte_identical_durable` →
  `..._a_filter_identical_durable`.** It asserts only `filter_subjects` via
  `verify_command_durable`, not the whole `pull::Config` (`ack_wait` differs), so
  "byte-identical" over-promised.
- **The `nats-fabric` anchor `make guard` now actually fails on dead grammar.**
  The recipe wrapped its `exit 1` in a subshell whose non-zero status was then
  swallowed by a trailing `|| true`, so `make guard` (and `make check`) logged
  `DEAD GRAMMAR in the live-wire anchor` but exited `0` — the central anti-drift
  gate was decorative and the pre-v1 `identity.cmd.`/`identity.evt.` grammar
  could reappear silently while the README and CHANGELOG claimed it failed loud.
  Inverted to `! grep … || (echo … && exit 1)`: the nominal no-match case exits
  `0`, an injected `identity.cmd.`/`identity.evt.` match exits non-zero through
  `make guard` and `make check`.

### Known limitation

- **The `conformance-scope` spawn battery is migrated on the Rust side and the
  Go anchor is re-frozen on the `v1.0.0` wire, but its `#[ignore]`-gated
  real-infra tests have not been run in this integration** (no NATS server was
  available). The Go anchor (`conformance-subjects/scope-service`) now freezes
  the `integration.*` grammar — `wire.go` carries
  `integration.cmd.identity.service_scope.declare.v1` /
  `integration.evt.identity.service_scope.{accepted,rejected}.v1` and the
  two-stream split (publish the declare onto `INTEGRATION_CMD`, await
  confirmations on `INTEGRATION_EVT`) — so the wire matches the lib oracle. The
  remaining gap is purely execution: the spawn-mode tests need a running NATS to
  go green and have not been exercised here (lib-as-oracle / Go-as-anchor: the
  Go side freezes the wire independently of the lib).

## [0.6.0] — 2026-06-15

### Changed (breaking)

- **`E2eDatabase::owner_url()` is renamed `owner_migration_url()`.** The owner
  carries `BYPASSRLS` and is the **migration/bootstrap** agent, never the runtime
  pool: a service whose e2e `boot()` set `DATABASE_URL = owner_url()` silently ran
  the runtime as the RLS-bypassing owner, so RLS was **off** and cross-tenant
  isolation "passes" proved nothing (this masked two real production cross-tenant
  PII leaks, found 2026-06-15 in svc-identity). The new name reads as obviously
  wrong at a runtime `DATABASE_URL` call site. Migrate `owner_url()` →
  `owner_migration_url()`; the runtime DSN is `app_url()`.

### Added

- **`SseSubscription::expect_event_on(field, timeout)`** — the channel-3
  ergonomic that closes #55's SSE reconciliation (item B.1). `next_event` /
  `expect_event` already unwrap an SSE frame to its GraphQL `data` object and fail
  loud on an `errors` payload; `expect_event_on` pulls a *named* subscription root
  straight out of it (failing loud if the awaited frame omits that field), so a
  service drops its bespoke push-extraction boilerplate and `svc-charter` retires
  its service-local channel-based `sse.rs`. The harness keeps its stream-based
  flat API (the form of `open` / `verdict::*`, not a builder); the charter
  `expect_push` / `next_within` names map onto the harness `expect_event` /
  `next_event` without aliases. Feature-gated under `sse`.
- **`E2eDatabase::create_with_app_role(owner, db, app_role, app_pwd, bypassrls,
  managed_roles)`** — a first-class RLS-safe provisioning ctor that provisions the
  owner **and** the RLS-subject runtime app role in one call and hands back a
  ready, non-panicking `app_url()`. Two-role, RLS-active provisioning is now the
  ergonomic default rather than an easy-to-miss `.with_app_role(…)` opt-in next to
  a dangerous owner-DSN default. Converges `svc-identity`'s hand-rolled `boot_rls`,
  `svc-charter`'s bespoke `PgFixture` and #95's bespoke pool onto one lib path.

### Fixed

- **Shared-name provisioning no longer races under cross-process concurrency.**
  `ensure_owner_role` / `ensure_app_role` / the schema-grant and connect-grant
  dances ran unconditional `ALTER ROLE` / `GRANT` / `ALTER DEFAULT PRIVILEGES`
  on every steady-state call against shared, stable role names — two parallel test
  binaries or worktrees pointing at one Postgres wrote the same `pg_authid` / ACL
  row simultaneously and the loser aborted with `tuple concurrently updated`
  (`XX000`), which the old `duplicate_object` (`42710`-only) guard did not catch
  and whose recovery path re-collided. Each ensure/grant critical section now runs
  inside a transaction that first takes `pg_advisory_xact_lock(hashtext(name))`
  keyed on the shared object name, serializing same-named callers on the server,
  and retries with a bounded backoff only on the concurrent-catalog race itself —
  an `XX000` whose message reports a tuple updated/deleted `concurrently`, plus
  deadlock (`40P01`) and lock-not-available (`55P03`); any other `XX000` (a real
  internal error) fails immediately rather than being masked by the retry. The shared `CREATE DATABASE` / `DROP DATABASE` path —
  which cannot run inside a transaction — is wrapped in a session-level
  `pg_advisory_lock` for the same effect. `recreate_stream` / `recreate_kv` retry
  their delete-then-create over a bounded loop to absorb a concurrent recreate on
  a shared NATS server. A new `quote_ident` / `quote_literal` helper escapes every
  interpolated identifier and literal (the role/db names the public surface takes
  as arbitrary `&str`).

- **README doc bug (security invariant #1).** The README claimed the provisioning
  was "collision-safe when parallel worktrees share a role name" — true only for
  `CREATE`-vs-`CREATE`, never the `ALTER`/`GRANT` steady-state path that actually
  bit. The README + its "Why" table are corrected to the advisory-lock model.

- A concurrent-provisioning self-test (`Barrier` + N tasks on a shared role name,
  looped) is added — red on the pre-fix lib, green after.

## [0.5.2] — 2026-06-15

### Changed

- **Re-pinned `br-rust-common` from `v0.11.0` to `v0.11.1`** across every crate
  that links it (`br-test-harness` → `br-core-auth`; the `conformance-identity`,
  `conformance-directory`, `conformance-passport`, `conformance-scope` oracles →
  `br-core-*` / `br-identity-domain` / `br-scope-declaration-contract`). `v0.11.1`
  is a `br-util-graphql` SDL-name bugfix that changes no wire and no type this
  harness imports, so the conformance oracles still deserialize the Go-frozen
  golden vectors identically. The bump keeps a consumer that links **both** the
  harness (dev-dep) and `br-rust-common` v0.11.1 (prod) on a **single source**,
  preventing a diamond-skew duplication of `br-core-*` in the graph.

## [0.5.1] — 2026-06-14

### Fixed

- **`E2eDatabase` no longer leaks its owner role when a test panics before cleanup.** The
  owner role is now ensured **idempotently** (same `DO … IF NOT EXISTS … duplicate_object`
  shape as the app role), so a role left by a crashed run no longer makes the next run die
  on `42710 role already exists`, and the path is collision-safe under parallel worktrees.
  `E2eDatabase` also gains a best-effort **`Drop` net** that tears down the per-test DB +
  owner when a test panics before the explicit `cleanup`/`teardown`; the explicit path
  stays primary, and the net warns rather than panics, with a 10s bound on its admin
  connection so an unreachable Postgres can never hang teardown. `create` / `create_named`
  signatures are unchanged (`cleanup` stays by-value). See the README "Why" table for the
  mechanism. Real-PG tests cover re-run-after-leak recovery and the Drop-net teardown; an
  offline self-test proves the net survives an unreachable admin without panicking.

## [0.5.0] — 2026-06-14

### Added

- **`br-test-harness::nats_assert` — NATS published-contract observation + named-stream
  reset (the 4th e2e channel).** Three `nats`-gated free functions, promoted generic
  from `svc-charter`'s service-local `tests/common/` (issue #55, A.2 + A.3), taking a
  `&async_nats::jetstream::Context`:
  - **`await_integration_event(js, stream, subject, deadline) -> Option<Value>`** — opens
    an ephemeral pull consumer (`DeliverPolicy::All` + `filter_subject`) on the named
    stream, awaits the first matching message within `deadline`, and returns the envelope
    as a `serde_json::Value` for deserialization against the producer's `contract-*`
    payload. The **stream name is a parameter** (charter hard-coded `CHARTER_STREAM`),
    so it is service-agnostic; a clean timeout yields `None`, never a hang.
  - **`recreate_stream(js, name, &subjects) -> stream::Stream`** /
    **`recreate_kv(js, bucket) -> kv::Store`** — **delete-then-create** a *named*
    service-contract JetStream stream (with its subject filters, e.g. `charter.>`) or KV
    bucket, so each serial scenario starts from an empty bus on a shared server. Explicit
    delete-then-create, **never silent get-or-create** — the harness is the GitOps
    stand-in (the production service never provisions; the lib fails loud), and a
    get-or-create would leak the prior scenario's messages while the reset passed.
  - The helpers are **free functions, not methods on `TestNats`** — `TestNats` owns the
    harness's own throwaway UUID-suffixed buckets, not a named service-contract stream.
    `futures-util` joins the `nats` feature (the consumer message stream). Self-tests in
    `tests/nats_assert.rs` cover the offline reachability check plus four `#[ignore]`-gated
    real-`nats-server` cases (await-after-reset, timeout→`None`, delete-then-create drops
    stale messages / prior KV entries).
- **`SseSubscription::drain(max, timeout) -> usize`** — the third de-flake
  primitive, alongside `WsSubscription::next_matching` and `wait_until`. Pulls up
  to `max` events off an open SSE subscription, stops at the first that doesn't
  arrive within `timeout` (a clean stream end or genuine silence) leaving the
  rest for the next read, and returns the count drained. Lets a scenario flush
  the tail of a known burst before its next leg, so a stale earlier-transition
  frame can't satisfy a later `expect_event`. A broken stream still panics. Back-
  ports charter's local `drain_pushes(sub, max)` as a strict superset (explicit
  `timeout`, returned count): the harness's stream-based `SseSubscription`
  (`open` / `next_event` / `expect_event` / `expect_silence` / `drain`) now
  covers charter's full push/silence surface, and charter drops its channel-based
  local copy (B.1 of #55).

### Changed

- **`br-test-harness` re-exports `PassportBuilder` from `br-core-auth`.** The
  hand-rolled builder in `br-test-harness/src/passport.rs` is deleted; the crate
  now re-exports `br_core_auth::PassportBuilder` (its `test-support` feature)
  under the same `br_test_harness::PassportBuilder` path. The builder now lives
  next to the `Passport` it forges, tracking field changes with zero drift.
  API parity is exact (`.user_id() .super_admin() .active() .pat()
  .impersonator() .claim() .claims() .build() .build_service()` + `Default`),
  so current users (svc-notifier e2e, conformance crates) see no behavior change.
- **`br-core-auth` re-pinned to `br-rust-common` `v0.11.0`, with `test-support`
  enabled.** This is a **branch pin** (`branch = "v0.11.0"`) pending the
  `v0.11.0` tag — the release work flips it to `tag = "v0.11.0"`.

### Documentation

- **`br-test-harness` README documents the 4-channel observation pattern + the
  copy-me `TestContext` template** (issue #55, "Done when" #4 / B.2.a). The new
  "The 4-channel observation pattern" section frames the BR e2e doctrine — the
  scenario *is* the executable functional spec, state built **only** through
  GraphQL mutations (never DB seeds) — and maps each channel to the now-shipped
  primitives: mutation ack/error → `verdict::*`; query + affordances →
  `GraphqlClient::query` + the affordance-skip guarantee; subscriptions →
  `SseSubscription`; published contract (KV + integration events) →
  `await_integration_event` + `recreate_stream` / `recreate_kv`; plus the two-role
  Postgres (`E2eDatabase::with_app_role`) and fail-loud boot
  (`SpawnedProcess::await_boot` → `BootOutcome`) underpinnings. The "Copy-me: the
  per-service `TestContext` assembly" section embeds `svc-charter`'s assembly
  **adapted onto the shipped harness blocks** as the reference template a new
  service copies (actors + named streams + KV bucket + the two PG roles +
  restart/boot helpers), reflecting the decided **blocks-only** design (B.2.a):
  the harness ships the blocks, the service owns the ~80-line composite.

### Added

- **`E2eDatabase::with_app_role(name, password)` — two-role Postgres provisioning
  for RLS tests.** Alongside the existing schema **owner** (the migration agent),
  the harness now provisions the **runtime app role** an RLS-enforcing service
  connects as — the RLS *subject*, gated by `current_setting('app.current_user_id')`.
  The builder step is optional (`create` / `create_named` are unchanged): it ensures
  the role idempotently via a `DO … IF NOT EXISTS … $$` block (`NOBYPASSRLS`, `LOGIN`;
  collision-safe when parallel worktrees share a role name), `GRANT CONNECT`s it to
  the fresh DB, and on the owner connection runs `GRANT USAGE, CREATE ON SCHEMA public`
  plus the two `ALTER DEFAULT PRIVILEGES FOR ROLE <owner> … GRANT … TO <app>` (tables
  + sequences) so tables the owner's migrations later create are reachable by the
  runtime role. Role **names** are caller-supplied; the grant dance is the generic,
  load-bearing part lifted out of every service's hand-rolled `tests/common`.
  - New accessors: **`app_url()`** (the runtime DSN; panics if `with_app_role` was
    never called), **`admin_url()`** (the superuser DSN, surfaced for posture /
    raw-state assertions), and **`app_role()`** (`Option<&str>`).
  - `cleanup` drops only the per-test DB and the unique-suffixed owner; the app role
    persists (a shared name may be in use by a parallel run — same posture as
    `managed_roles`).
  - Real-PG tests (env-gated `#[ignore]`, `E2E_PG_ADMIN_URL` / `DATABASE_URL`): the
    app role authenticates and writes a fresh owner-created table via default
    privileges; provisioning is idempotent under a shared role name; an RLS-isolation
    proof asserts the app role sees only its principal's rows while the `BYPASSRLS`
    owner reads the raw, unfiltered state. Additive, public-surface minor.
- **`conformance-directory` — the identity Published Language (directory)
  conformance battery (epic #54, WU9).** A new crate that guards the
  `br-core-directory` wire and the `br-util-directory` publisher/consumer kit,
  driving the WU8 Go anchor `conformance-subjects/identity-directory`.
  - **Wire-deser gate (W1–W5, offline).** Builds + runs the Go anchor and
    deserialises every emitted `{key, value}` `value` **through the real
    `br-core-directory` types** (`PublishedUser` / `PublishedGroup` /
    `DirectoryMeta`). A successful deser *is* the wire-shape check; a lib drift
    (renamed/retyped core field, changed serde, broken `flatten`) makes the deser
    fail → red. Asserts: users/groups/meta deserialise, the neutral extension
    `x_custom` lands in `extensions` (flat) never in a core field, `member_ids` is
    always an array, and a users-only `_meta` auto-degrades groups. **W1 also
    positively binds the optional core names:** a populated user (`first_name` +
    `last_name` both non-null) must deserialise with both as `Some` — closing the
    blind spot where renaming an *optional* core field (`first_name` → `firstName`)
    would silently land in `extensions` and leave the field `None` undetected (the
    `flatten` makes the deser lenient, so a missing-required-field check alone never
    catches it). Pure unit tests of the same logic (inline JSON mirroring the anchor,
    including the `firstName` rename going red) keep plain `cargo test` green with
    **zero** toolchain.
  - **Px (publisher) — P1/P2, real NATS KV.** Drives `DirectoryPublisher` against a
    `SpawnedNats` KV bucket. **P1 (mandatory floor):** `reconcile` writes `_meta` +
    every user, the published wire round-trips identically to the source through the
    lib types, a second reconcile is the empty diff (idempotent), and dropping a user
    orphan-deletes its KV key (PII propagation). **P2 (optional):** a users-only
    source publishes no group keys and a `_meta` that omits groups (gated on `_meta`).
  - **Cx (consumer) — C1/C2, real NATS KV + real PG.** Drives `DirectoryProjector`
    (KV→PG over an `E2eDatabase`) and the `DirectorySnapshot` readers; all opt-in,
    none mandatory. **C1:** reconcile-on-boot projects users, `resolve_user` returns
    the carried fields, retracting a KV user orphan-deletes its projection row.
    **C2:** `is_member` / `group_name` resolve with groups in `_meta`, and
    auto-degrade to empty under a users-only `_meta`.
  - **Oracle = the real `br-core-directory` / `br-util-directory` types**, pinned to
    `br-rust-common` **branch `v0.11.0`** (`version = "0.11.0"` alongside, pending the
    `v0.11.0` tag; the release work flips it to `tag = "v0.11.0"`). The Go anchor
    never imports the lib — its independence is what makes the detector trustworthy.

- **`conformance-passport` — G4 (`scopes` claim round-trip) + the oracle bumped to
  `br-rust-common` v0.11.0.** The `br-core-auth` pin moves from `tag = "v0.10.0"` to
  `branch = "v0.11.0"` (`version = "0.11.0"`, `test-support` enabled) so the battery
  guards the new typed-scopes surface (`SCOPES_CLAIM_KEY`,
  `Passport::scopes() -> Vec<ScopeKey>`, `has_scope`). **G4** (a pure unit test, no
  infra) forges a `Passport` with a `scopes` claim, round-trips it through the
  `X-Passport` base64 header (`to_header` → `from_header`) and asserts the decoded
  Passport is identical, `scopes()` yields the granted typed keys, `has_scope` is
  correct for granted/ungranted scopes, an absent claim yields no scopes, and a
  malformed entry is skipped while valid ones survive. `CheckId` gains a
  `ScopesClaimRoundTrip` (`g4`) variant. (This oracle bump was deferred to WU9 by the
  WU7 review.)
- **`SpawnedProcess::await_boot` + `BootOutcome` — fail-loud boot classification.**
  Alongside the happy-path `wait_for_http_ok`, `await_boot(url, timeout)` polls the
  service's `/readyz` and classifies the boot into
  `BootOutcome::{ Ready, Exited(ExitStatus), TimedOut }` so a scenario can **assert**
  the verdict — including the deliberate crash of a service that refuses to start
  with its declared GitOps infra missing (S0-style "fail loud and name the missing
  resource"); `wait_for_http_ok` covers only the happy path. On `Exited` the captured
  pipes are drained to EOF before returning, so `proc.logs()` carries the full tail
  the binary printed (where the named missing resource lives). The process stays owned
  by the caller in every outcome. `BootOutcome::is_ready()` / `exit_status()` are the
  convenience reads. Both `BootOutcome` and `await_boot` are `http-client`-gated; the
  existing `wait_for_http_ok` / `logs()` surface is preserved unchanged. Promotes
  charter's service-local `await_boot` / `spawn_capturing_output` (issue #55, A.5);
  reconciled to the harness's continuous-drain `SpawnedProcess` (no kill-before-drain
  dance, the process is not consumed into the outcome).
- **`br-test-harness` — the GraphQL `verdict` module (channel-1 assertion
  vocabulary).** Pure functions over a `serde_json::Value` GraphQL response —
  `is_ack`, `expect_ack`, `expect_rejected`, `mutation_error_code`,
  `expect_code_shaped` (asserts the stable error-code shape `^[A-Z][A-Z0-9_]+$`,
  so **≥2 characters** — a single `"A"` is rejected), `is_code_shaped` — with
  **zero transport coupling** (they take a response, not a
  `GraphqlClient`). Feature-gated under `graphql`; promoted from `svc-charter`'s
  service-local `tests/common/gql.rs` (the BR-fatty reference unit) so every
  affordance-aware service stops re-inventing the most load-bearing observation
  helper (#55 A.1).
  - **Affordance-skip guarantee:** the rejection-code walker behind
    `mutation_error_code` skips any subtree under an `affordances` key at any
    depth, so an affordance's own `reasonCode` (a blocked-action hint) is never
    mistaken for a mutation rejection; a payload-union rejection code still wins
    when both coexist. Covered by unit vectors (no infra).

- **`conformance-passport` — the G1 conformance battery (bearer/PAT → Passport).**
  A black-box runner for the BotResources passport-resolution endpoint the GraphQL
  gateway calls before every authenticated request (`GET /internal/passport`). It
  drives a new frozen Go subject, `conformance-subjects/identity-passport`, as a
  black box: it stands up a `nats-server`, creates the `bearer_tokens` JetStream KV
  bucket, seeds entries, calls the endpoint with various `Authorization` headers,
  and decodes the returned `X-Passport`.
  - **Oracle = the real `br-core-auth` types** (tag `v0.10.0`): seeding uses the real
    `bearer_token_key(raw)` for the KV key and serializes the real
    `BearerTokenEntry { email, token_id }`; decoding the `X-Passport` is the real
    `PassportHeader::from_header` into `br_core_auth::Passport`. Deserialization
    succeeding *is* the wire-shape check (`Passport` is `deny_unknown_fields`); there
    is no hand-rolled JSON or shape guard to drift from the types.
  - Scenarios **P1–P5**: a valid seeded bearer → `Passport::Human` with
    `auth_method == Pat { token_id }` matching the seeded token_id, `claims.email`
    matching the seeded email, and a present valid `user_id` (not value-asserted, a
    subject-side stand-in); a revoked bearer → 200 anonymous (no `X-Passport`); an
    unknown bearer → 200 anonymous; no `Authorization` → 200 anonymous; two distinct
    seeded entries → each resolves to its own passport with no cross-talk.
  - **The non-tautological gate is P1 + P5**: the independent Go subject must agree
    with the lib's `bearer_token_key` derivation and `BearerTokenEntry` shape — a
    divergence means the seeded key is never found, the bearer resolves to anonymous,
    and the battery goes red. That is the backward-compat property.
  - **`run_spawn`** (the core deliverable) stands up a throwaway `nats-server` + the
    `bearer_tokens` bucket + the Go subject and runs the full P1–P5; per-test broker
    isolation (each test spawns its own `SpawnedNats`), with emails/tokens namespaced
    per run by a UUIDv7. The subject fails loud if the bucket is missing, so the boot
    order is bucket → subject → `/readyz=200` → scenarios. An attach runner against a
    live `svc-identity` is a future addition (G1 ships spawn only).
  - **Trust model:** the endpoint **resolves**, it does not **gate** — an unresolvable
    credential is a 200 anonymous request (never a 401), matching the platform
    posture that services do authZ, never authN.
- **`conformance-subjects/identity-passport` — the G1 Go anchor.** A minimal, frozen
  Go reimplementation of the passport-resolution wire: lowercase-hex SHA-256 KV key,
  `BearerTokenEntry` parse, and the `Passport::Human` envelope (base64 `X-Passport`).
  Its offline Go unit tests pin the key derivation, the deterministic `user_id`
  stand-in, the golden Passport shape, the exact top-level key set
  (`deny_unknown_fields` parity), and the bearer-header parsing.
- **`conformance-subjects/identity-directory` — the directory (identity Published
  Language) Go wire anchor (WU8, epic #54).** A minimal, frozen, **independent** Go
  reimplementation of the identity directory KV wire — the read-only projection
  identity publishes and generic services consume. It **imports no Rust** (the
  oracle is the lib, in the Rust runner WU9); it prints the canonical KV snapshot to
  stdout for that runner to deserialise through `br-core-directory`.
  - **What it freezes:** the KV-key conventions (`identity/_meta`,
    `identity/users/{uuid}`, `identity/groups/{uuid}`), the `DirectoryMeta` manifest
    (`{version, entities}`), the `PublishedUser` core (`email`, `first_name`,
    `last_name` — snake_case, no `rename_all`, names emitted as `null` not omitted),
    the `PublishedGroup` core (`name`, `member_ids` — always an array), and the
    **generic extension mechanism**: a neutral `x_custom` key riding **flat**
    alongside the core (mirroring the Rust `#[serde(flatten)]`).
  - **Tenancy-agnostic socle:** emits **no `organization_id`** / orgs / memberships —
    `organization_id` is a project extension, not core (epic #54); conformance covers
    the core + the generic extension mechanism only.
  - **Offline Go unit tests** pin the KV-key prefixes, the user/group/meta golden
    shapes, the exact core key sets, names-emitted-as-null, the flat-extension
    invariant (never nested under an `extensions` key), `member_ids`-always-array,
    and the users-only `_meta` auto-degrade.
  - **Oracle / backward-compat gate:** the Rust WU9 runner deserialises every emitted
    value through the real `br-core-directory` types; a renamed/retyped core field,
    a changed serde, or a broken flatten makes the Go-frozen value fail to
    deserialise → the battery goes red. The wire is frozen for `br-rust-common`
    `v0.11.0`; documented in `docs/conformance/directory-wire-v1.md`.

## [0.4.0] — 2026-06-14

### Fixed

- **`conformance-scope` declare-capture now replays the stream (`DeliverPolicy::All`).**
  In attach mode the capture consumer is created *after* the service under test has
  already published its boot `declare`; with `DeliverPolicy::New` it skipped that
  first declare and only converged on the service's re-publish cycle (~10s), making
  the G3 gate slow and hiding the scope-declaration boot speedup shipped in
  `br-rust-common` v0.10.0. Replaying from the start of the stream catches
  the boot declare immediately; the attach battery now converges sub-second. Safe
  across all modes — battery/spawn use a fresh per-run stream and start the capture
  before spawning, and attach runs against a fresh per-run JetStream store.
  Regression-locked by `attach_capture_replays_the_boot_declare_published_before_it`,
  which uses a long `wait_timeout` so only a replay (not a re-publish) can satisfy
  its bounded assertion.

### Changed

- **`br-rust-common` pin bumped `v0.8.0` → `v0.10.0`** (tag `v0.10.0`, commit
  `32c463adc19791205230de75cd603ae375b7633d`) across `conformance-scope`,
  `conformance-identity`, and `br-test-harness`. Both conformance batteries stay
  green against the bumped library — `conformance-scope` S1–S6 and
  `conformance-identity` A1–A7 — proving the published scope-declaration wire is
  backward-compatible across `0.8 → 0.10`, which is exactly what these batteries
  exist to prove. A mechanical bump: nothing in the harness uses any symbol the
  `0.10.0` release removed.

### Added

- **`conformance-identity` — the G2 conformance battery (the mirror of G3 with the
  role inverted).** Where `conformance-scope` (G3) tests a scope-*declaring* service
  with the runner playing the acceptor, `conformance-identity` (G2) tests a
  scope-*accepting* service (the Identity registry) with the runner playing the
  **declaring** side. It drives a new frozen Go subject,
  `conformance-subjects/identity-acceptor`, as a black box: it builds real
  `IntegrationCommand<DeclareServiceScopes>` and publishes them, then decodes the
  acceptor's reply **by deserialization** into the real
  `IntegrationEvent<ServiceScopesAccepted>` / `IntegrationEvent<ServiceScopesRejected>`
  types (no hand-rolled JSON, no `deny_unknown_fields`).
  - **Oracle = the real `judge_declaration` / `ScopeRegistry`** (`br-identity-domain`,
    tag `v0.10.0`): each scenario replays its declaration sequence through a real
    `ScopeRegistry` and the subject's emitted verdict is asserted **equal** to the
    lib's `DeclarationOutcome` per step — computed, never hard-coded.
  - Scenarios **A1–A7**: clean declaration → accepted; owned-scope reclaim after a
    prior accept → rejected (multi-step state carry); intra-declaration duplicate →
    `duplicate_scope_in_declaration`; prefix mismatch → `scope_prefix_mismatch`;
    charset-invalid scope key → `invalid_scope_key` (`invalid_charset`); idempotent
    re-declare → accepted again; structurally malformed scope key (no `:` separator) →
    `invalid_scope_key` (`malformed_segments`). The acceptor keeps an in-memory
    `scope_key → owner` registry across declarations; the runner seeds ownership by
    driving a prior accepted declaration.
  - **`run_spawn`** (the core deliverable) stands up a throwaway `nats-server` + the
    Go subject and runs the full A1–A7; **`run_attach`** drives a live acceptor's NATS
    + `/readyz` with unique per-run keys (bounding registry pollution). The Go subject
    is an independent reimplementation of the wire + `judge_declaration` policy — a
    backward-compat anchor, not a binding to the lib; its Go unit tests pin the
    rejection wire shapes (incl. `invalid_charset`, `too_long`, `malformed_segments`)
    to the frozen `scope-wire-v1` golden JSON.
  - **A2 proves cross-declaration state carry.** It is the only multi-step scenario:
    a prior declaration is accepted and stays accepted while a later reclaim of the
    same key is rejected — proving the subject's registry persists across declarations.
    Its rejection reason is `scope_prefix_mismatch`, **not**
    `scope_owned_by_another_service`: `judge_declaration` validates before the
    registry's cross-owner check, so a prefixed key can only be declared by its own
    service and the reclaim never reaches that branch (it is produced in production by
    the app-layer `UNIQUE(scope_key)` path). The unit test asserts A2's step verdicts
    explicitly (`[0]` Accepted, `[1]` prefix-mismatch Rejected).
  - **A7 closes the oracle loop on `malformed_segments`.** The Go subject already
    implements the `too_long` / `malformed_segments` / `empty` key-validation paths,
    but the battery only cross-checked `invalid_charset` (via A5); A7 adds a
    structurally malformed key so both the frozen subject and the real
    `ScopeKey::new` / `judge_declaration` oracle reject with
    `KeyValidationError::MalformedSegments`.
  - **Trust model (documented, not tested):** the acceptor trusts the manifest's
    self-asserted identity; the prefix rule is a coherence check, not authentication.
    Impersonation is out of the threat model — the trust boundary is the deployment
    scope (only first-party services run in it; the NATS bus runs without per-service
    auth today), so G2 tests no impersonation: it is an infrastructure property, not a
    domain one.
  - The `infra-e2e` CI job (already provisioned with `go` + `nats-server` for G3) now
    additionally runs the G2 battery.

## [0.3.0] — 2026-06-13

### Added

- **`br-test-harness` is now feature-gated** (`default = ["full"]`). Each heavy
  helper cluster is opt-in: `nats`, `spawned-nats`, `e2e-db`, `server`,
  `passport`, `graphql`, `sse`, `ws`, `oidc`; `SpawnedProcess` / `run_once` /
  `wait_until` stay always-compiled. A consumer can take `default-features =
  false` with a minimal feature set and drop `sqlx` / `axum` / `rsa` / `reqwest`
  / `tokio-tungstenite` from its build. Default consumers are unaffected (`full`
  = the whole toolbox). `conformance-scope` now takes only `["nats",
  "spawned-nats"]`, so the `conformance-scope` CLI binary no longer compiles the
  Postgres / Axum / JWT stacks (≈86 fewer crates in its dependency graph).

### Changed

- **`conformance-scope` derives its scope-declaration subjects from
  `br-scope-declaration-contract`** (`br-rust-common`, tag `v0.8.0`) instead of
  hardcoding the `declare` / `accepted` / `rejected` subjects (and their
  `event_type` strings). The frozen Go wire fixture stays the independence
  anchor, so the battery now also **detects subject drift**: if `br-rust-common`
  changes a subject, the derived value diverges from the Go fixture and a
  conformance test goes red. The throwaway capture stream's `identity.>` wildcard
  stays a literal — it is not a contract subject.
- **`conformance-scope` publishes accept/reject via
  `br_core_integration::NatsIntegrationPublisher`** instead of a hand-rolled
  `js.publish(serde_json::to_vec(…))` + ack — identical wire bytes, reusing the
  shared publisher instead of re-vendoring serialization.

### Removed

- **`conformance-scope`'s public subject constants** `DECLARE_SUBJECT` /
  `ACCEPTED_SUBJECT` / `REJECTED_SUBJECT` — replaced by the contract-derived
  functions `declare_subject()` / `accepted_event_subject()` /
  `rejected_event_subject()` (each `-> Result<String, SubjectError>`). A breaking
  change to the lib's symbol surface; it rides this `0.3.0` bump (a `0.x` minor is
  the breaking position under Cargo semver, so consumers move by bumping the git
  tag). No in-tree consumer referenced the constants.

## [0.2.0] — 2026-06-13

### Changed

- **Unified workspace versioning.** The whole repository now ships **one
  version** behind a single `v{version}` git tag (this release: `v0.2.0`),
  replacing the prior per-crate tags (`<crate>-vX.Y.Z`). Every crate inherits
  `version.workspace = true` from the root `Cargo.toml`; a consumer pins the
  harness by the unified tag, never a per-crate version. Prior per-crate
  versions folded into this release: `br-test-harness` 0.1.0,
  `conformance-scope` 0.2.0, `conformance-scope-cli` 0.1.0, `oidc-test-idp`
  0.1.0.
- **Single root CHANGELOG.** History is consolidated here from the four former
  per-crate `crates/*/CHANGELOG.md` files, which are removed.
- **`br-rust-common` dependency bumped to `tag = "v0.8.0"`** (was an older git
  rev). All crates depending on `br-core-*` now pin the same `v0.8.0` tag, so
  Cargo resolves a single source and never duplicates `br-core-*` in the graph.
  `br_core_integration::MessageMetadata` was renamed upstream to `EventMetadata`.

### Added

#### `br-test-harness` — real-infra end-to-end test harness (was 0.1.0)

- The consolidated real-infra end-to-end test harness for BotResources platform
  services, distilled from the three divergent local copies (be-botresources
  `test-harness`, svc-identity's `e2e_support`, svc-notifier's `tests/common`).
  A **dev-dependency-only** crate.
- `E2eDatabase` — provisions a throwaway Postgres database + a non-superuser,
  CNPG-shaped owner role (explicit `BYPASSRLS` posture; suffixed or GitOps-exact
  names) for a spawned binary to migrate and run against under real RLS.
- `SpawnedNats` — a dedicated, ephemeral `nats-server -js` per test chain, so
  binaries that hardcode their bucket names still isolate.
- `TestNats` — a connection to a real NATS JetStream server plus isolated,
  per-test KV buckets; hands back **raw** `async_nats` handles (no project port
  trait), and forwards URL-embedded credentials (the async-nats workaround).
- `SpawnedProcess` / `run_once` — run the real service binary (or a one-shot
  CLI) as a child, draining stdout/stderr and polling an HTTP readiness URL,
  with captured logs surfaced on failure.
- `TestServer` — an in-process Axum server on a random port.
- `PassportBuilder` — forge a `Passport` (`Human` or `Service`), built directly
  on `br-core-auth` (no dependency on any project's private `infra` crate). It
  owns the `Passport` **structure** — typed setters for the canonical fields
  (`user_id`, `super_admin`, `active`, `pat` auth method, `impersonator`) —
  while the free-form `claims` keys are a **per-project seam**: the project
  passes its own via `.claim(key, value)` / `.claims(iter)`. The harness names
  no project claim key.
- `GraphqlClient` — GraphQL / REST client that attaches the forged `X-Passport`
  header (and an unauthenticated variant to pin rejection).
- `WsSubscription` — a real `graphql-transport-ws` subscription client with
  `next_data` (single push) and **`next_matching` drain-until-match**, the
  de-flake primitive for broadcast subscriptions; parameterizable WS path.
- `SseSubscription` — a Server-Sent-Events subscription client
  (`next_event` / `expect_event` / `expect_silence`), generalized so the caller
  indexes its own subscription root field.
- `wait_until` — poll an async condition up to a bounded deadline — the de-flake
  for "the side-effect lands a beat after the ack".
- `oidc` — re-export of the sibling `oidc-test-idp` fixture, so a service gets
  infra + a pilotable in-process OIDC IdP from this single dev-dependency.

#### `conformance-scope` — black-box scope-declaration conformance runner (was 0.2.0)

- The G3 black-box conformance runner for the BotResources scope-declaration
  wire handshake. A **dev-dependency-only** crate that drives the frozen Go
  subject (`conformance-subjects/scope-service`) as a black box.
- `ScopeHarness` — `start()` builds the Go subject, and `start_with_binary` runs
  the same battery against any subject binary; both spawn a dedicated
  `SpawnedNats` and create the handshake JetStream stream (`identity.>`) up front
  (the platform never auto-provisions).
- `DeclareCapture` — a subscribe-first ephemeral consumer over the declare
  subject that drains declares; `decode()` validates the wire shape by
  deserializing the raw bytes into the **real**
  `IntegrationCommand<DeclareServiceScopes>` — deser success *is* the
  conformance check, with no hand-rolled shape guard. A broken drain fails loud:
  the cause is captured and re-raised through `count()` / `declares()` rather
  than masquerading as a silent subject.
- `accept` / `reject` — publish a confirmation built from the real
  `br-core-scope` payload types (`ServiceScopesAccepted` /
  `ServiceScopesRejected`), echoing the command's `correlation_id`,
  ack-confirmed.
- `Subject` / `SubjectConfig` — spawn the built binary with its env wiring and
  poll `/readyz` / `/livez` (status, plus `readyz_body()` for the rejection
  reason).
- `create_handshake_stream`, `build_subject`, and the three frozen subject
  constants.
- The S1–S6 conformance battery in `tests/conformance.rs`, `#[ignore]`-gated for
  real infra (`nats-server` + `go` + the spawned binary): declare-on-boot
  shape, readiness gating, re-publish-on-timeout, rejection handling, duplicate
  confirmations, and disabled mode.
- Single-implementation check API: each S1–S6 check (plus a `declaration-content`
  content assertion) has one implementation in `checks/`, parameterized by a
  `CheckContext` and returning a structured `CheckOutcome` instead of panicking.
  Both `tests/conformance.rs` and the `conformance-scope-cli` binary call these —
  no second copy of the protocol logic.
- Structured outcome types: `CheckId`, `CheckStatus { Pass, Fail, Skipped }`,
  `CheckOutcome { id, status, expected, observed, detail }`, `ConformanceReport`
  with `passed()` / `failed()` / `skipped()` / `is_conformant()`.
- `ExpectedDeclaration` / `ExpectedScope` / `PlatformOnly` — the assertion input,
  with `platform_only` modeled **per scope** (faithful to `br-core-scope`'s
  `ScopeSpec.platform_only`); `assert_matches` renders an expected-vs-observed
  scope-set diff. Parsers `parse_scope_keys`, `parse_platform_only` (accepts a
  single bool or a per-scope `key=bool` CSV).
- `ReadyzProbe` — the readiness role decoupled from any spawned process, polling
  a given `/readyz` URL (spawn passes the subject's `base_url()`, attach passes
  the external URL).
- Two runners over the same checks: `run_spawn` (stands up `nats-server` + the
  subject binary, full `s1..s6`) and `run_attach` (zero host runtime deps:
  connects to a live service's NATS + `/readyz`, default `s1, s2` +
  `declaration-content`, never creates the stream — the service owns it, and the
  declare consumer fails loud if it is absent).
- `Scenario` / `AcceptorBehavior` with scenario parsing/defaults. In spawn mode
  the rejection scenario (s4) always exercises a rejection regardless of the
  global accept/reject flag; the global `--reject` reason flows through when
  given.
- `DEFAULT_TIMEOUT` (10s).

#### `conformance-scope-cli` — language-agnostic CLI over the battery (was 0.1.0)

- The `conformance-scope` CLI, a language-agnostic wrapper over the
  `conformance-scope` S1–S6 battery. All protocol logic lives in the lib;
  the binary parses arguments, builds the run config, calls the lib runner,
  formats the report, and sets the exit code.
- `conformance-scope run` — drive a single service in **attach** mode (default,
  zero host deps: `--nats` + `--readyz` against a live service, `--stream` for
  the pre-existing handshake stream) or **spawn** mode (`--spawn <PATH>`, needs
  `nats-server` on PATH). Expected declaration via `--service-key`, `--scopes`
  (CSV), `--platform-only` (a single bool or a per-scope `key=bool` CSV).
  Acceptor behavior via `--accept` (default) / `--reject [REASON]`. Scenario
  selection `--scenarios`, `--timeout`, `--format`, `--output`.
- `conformance-scope manifest <FILE>` — run a YAML manifest of many services
  (each `attach` or `spawn`) and aggregate one report.
- Report formats: `human`, `json`, `junit` — each check shows its id, expected,
  observed, and a wire/file excerpt on failure; a wrong-scopes failure reads as
  a clear expected-vs-observed diff.
- Exit codes: `0` fully conformant, `1` at least one check failed, `2` a
  usage / connection / I/O error.

#### `oidc-test-idp` — pilotable OIDC test IdP (was 0.1.0)

- A pilotable OIDC test IdP, shipped both as a library fixture (re-exported by
  `br-test-harness`) and as a container image (`br-oidc-test-idp`).
- `GET /.well-known/openid-configuration` — minimal, honest discovery document
  (`issuer`, `jwks_uri`, RS256; no endpoints the fixture does not implement).
- `GET /jwks` — serves exactly the *published* subset of the pre-generated
  RSA key pool.
- `POST /admin/mint` — signs an RS256 id_token with any pool key (published
  or not), pilotable `aud`, email claim name, expiry (negative = already
  expired), extra/override claims, and optional `kid`-header omission.
- `POST /admin/rotate` — instantaneous key rotation (the whole pool is
  generated at startup): default gesture publishes the next key and makes it
  active; explicit form publishes/unpublishes/re-activates arbitrary kids.
- `POST /admin/reset` — restores the startup state and zeroes counters.
- `GET /admin/state` — snapshot including `jwks_fetches`/`discovery_fetches`
  counters, so tests assert refresh/cooldown behaviour without sleeping.
- `GET /health`, `--version` (CD smoke-test), fail-closed env parsing.

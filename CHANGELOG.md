# Changelog

All notable changes to `br-e2e-harness` are documented here. The whole workspace
ships **one version**: every crate inherits `version.workspace = true`, and a
single git tag `v{version}` releases the set. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.

## [Unreleased]

## 1.2.0 - 2026-09-03

### Added

#### `br-test-harness` — `FabricTestNats` adversarial provisioning, observation and purge

- **`DurableConfig` + `provision_command_durable_with` /
  `provision_event_durable_with`.** `#[non_exhaustive] DurableConfig { ack_wait,
  max_deliver: Option<i64>, max_ack_pending }` — `Default` reads the lib's
  `ConsumerTuning::default()` through `From<ConsumerTuning>` (plus unlimited
  deliver), so the lib's own defaults cannot drift away from it;
  `DurableConfig::harness()` is what `provision_*_durable` has always used (`2s`,
  server-default `max_ack_pending`, unlimited deliver), and `.ack_wait` /
  `.max_deliver` /
  `.unlimited_deliver` / `.max_ack_pending` narrow it. The frozen-as-contract
  config stays frozen (`ack_policy = Explicit`, `deliver_policy = All`,
  `replay_policy = Instant`, filter = the rendered coordinate). `max_deliver` is
  settable here — and only here — because a finite redelivery budget is
  deployment-declared on the durable bound to `INTEGRATION_CMD` — it is consumer
  config, not a stream property — while the lib freezes it at unlimited;
  adversarial provisioning is the harness's job. `provision_*_durable`
  and `with_*_durable` delegate with `DurableConfig::harness()`, behaviour
  unchanged.
- **`tap_durable(FixedStream, durable) -> DurableTap`** — pulls from an
  already-provisioned durable and **never acks**, so a frame redelivers until its
  budget is exhausted; `next_within(timeout) -> Option<TappedDelivery { subject,
  payload, delivered_count }>`, `deliveries_within(timeout, cap)`, `close()`. It
  exists because every lib `ensure_*` / `run_*` path create-or-updates the durable
  back to the lib's config, erasing the budget under test.
- **Typed read-only counters.** `consumer_pending(FixedStream, durable)`,
  `consumer_delivered(FixedStream, durable)`,
  `consumer_redelivered(FixedStream, durable)` (deliveries past the first),
  `command_stream_len()`,
  `event_stream_len()`, `stream_len(FixedStream)` — `consumer_info` / stream state
  without exposing a JetStream handle. `FixedStream::{Cmd, Evt}` is the typed
  stand-in for the two fixed stream names (`.name()` bridges to the `&str`-taking
  negative-path helpers).
- **Purge.** `purge_command_stream()` / `purge_event_stream()` and
  `purge_command_subject(&coords)` / `purge_event_subject(&coords)`, each
  returning the purged count — the between-scenarios reset on a shared
  `connect(url)` NATS; a purge never deletes the gitops-declared stream.
- **`publish_command_raw(&coords, bytes)`** — the command-side sibling of
  `publish_event_envelope` and the only intentional malformed-wire command
  publisher, so a lib consumer's decode failure and its handler's `Term`
  fail-closed path are reachable from a test. Typed `fabric().publish_command`
  stays the default.

  Together these are the four operations a service e2e had to hand-roll on a
  retained raw `jetstream::Context`; that `Context` can now leave `tests/common`.

#### `br-test-harness` — SSE and WS subscription handles

- **`SseOutcome` — the SSE handle says why it went quiet.**
  `SseSubscription::next_outcome(timeout) -> SseOutcome::{ Event(Value), Timeout, Closed }`
  splits the two cases `next_event` collapsed into a single `None`: a server that
  ended the stream now reads as `Closed`, a stream held open with nothing to say
  as `Timeout`. `next_event` is unchanged (`Event(v) => Some(v)`, otherwise
  `None`), so every existing suite compiles as-is and migrates on its own
  schedule.
- **`SseSubscription::drain_outcome(max, timeout) -> (usize, DrainStop)`** — the
  drain reports why it stopped (`DrainStop::{ Limit, Timeout, Closed }`);
  `drain(max, timeout) -> usize` keeps its signature and delegates.
- **`SseSubscription::with_logs(&SpawnedProcess)`** — opt-in attachment of the
  spawned service's captured output. When attached, a `Closed` panic from
  `expect_event` / `expect_event_on` / `expect_silence` carries the last 80 lines
  of the service log, so "the server closed the subscription" arrives with the
  reason the service printed — read **at panic time**, so a line the service
  prints after the attach still lands in the message. `Timeout` carries the tail
  too ("alive but pushed nothing" is the frequent diagnosis). Unattached, the
  panic still names the outcome.
- **`WsCredential` — `WsSubscription` takes a generic credential.**
  `#[non_exhaustive] enum WsCredential<'a> { Passport(&Passport), Cookie(&str),
  Anonymous }` plus `WsSubscription::open_with(base, credential, query)` and
  `open_at_with(base, ws_path, credential, query)`. `Passport` sends
  `X-Passport` (unchanged), `Cookie` sends `Cookie` and no `X-Passport`,
  `Anonymous` sends neither — so a suite can drive a subscription through the
  real edge, where a client-forged `X-Passport` is stripped, without
  hand-rolling its own WS client. `open` / `open_at(&Passport)` keep their exact
  signatures and delegate with `WsCredential::Passport`.
- **`WsError` — a typed WS outcome.** `#[non_exhaustive] enum WsError {
  Timeout, Closed, ServerClosed { code: u16, reason: String }, Completed,
  ErrorFrame(String), Transport(String) }` (with `Display` + `Error`) and
  `WsSubscription::next_data_outcome(timeout) -> Result<Value, WsError>`: a
  deadline with no push, a stream that ended, a server that refused with a close
  code, a `complete` before any push, an `error` frame and a broken exchange are
  now six distinct verdicts — the handle says *why* it went quiet.
  `ServerClosed` keeps the `graphql-transport-ws` rejection code (`4400`,
  `4401`, `4403`, `4409`, `4429`) an assertion needs. `next_data` /
  `next_matching` keep `Result<Value, String>` and render the variants through
  `Display`.
- **`WsSubscription::next_matching_outcome(predicate, timeout) -> Result<Value,
  WsError>`** — the typed sibling of `next_matching`, for a drain-until-match
  that needs the reason it stopped rather than the skipped-frames report.
  `next_matching` delegates to the same loop and its message — reason **and**
  skipped frames — is unchanged.

- **`WsSubscription::close(self) -> Result<(), WsError>`** — ends the
  subscription with a `complete` frame, then closes the socket, so the service
  observes an orderly unsubscribe rather than a dropped connection.
  Best-effort: both steps are attempted, a socket the peer already closed is
  `Ok(())`, and it never panics.

#### `br-test-harness` — `FabricTestNats` delivery-failure injection

- **`DeliveryOutage` — a real-broker delivery outage, no mock.**
  `FabricTestNats::withhold_event_subject(&withheld, &[&keep, ...])` and
  `withhold_command_subject(&withheld, &[&keep, ...])` rewrite the fixed
  stream's `subjects` to **exactly** the `keep` set, so the withheld coordinate
  is covered by no stream: the lib's own `Fabric::publish_event` /
  `publish_command` on it fails
  `FabricError::Publish { kind: PublishErrorKind::NoStream, .. }` while every
  listed coordinate is still stored. A narrowing, **not** a deny-list — a
  coordinate absent from `keep` also stops flowing, and an empty `keep`
  withholds the whole grammar.
- **`withhold_event_stream()` / `withhold_command_stream()`** — the coarse
  variant: the binding is replaced by a placeholder
  (`integration.evt.__withheld__.>` / `integration.cmd.__withheld__.>`) so
  **every** coordinate on that stream fails, and the other fixed stream is
  untouched.
- **`DeliveryOutage::restore(self)`** puts the **pre-outage** binding back (the
  one read at `withhold_*` time, not a pristine `integration.evt.>`, so nested
  outages restore LIFO) and fails loud if the broker refuses. The guard is
  `#[must_use]` and has **no `Drop` net**: a guard dropped without `restore()`
  leaves the stream narrowed — beyond the run on a persistent broker, until
  gitops re-declares the stream — which is a test bug.
  `stream()`, `live_subjects()` and `withheld_subjects()` are the assertion
  surface — the latter is the one concrete coordinate for `withhold_*_subject`
  and the previous binding patterns for `withhold_*_stream`.
- **Misuse panics before the stream is touched** — withholding a coordinate the
  stream does not currently carry (a `withhold_*_subject` nested inside
  another), the same coordinate in both `withheld` and `keep`, the same
  coordinate twice in `keep`, and a `withhold_*_stream` on a stream already
  bound to the placeholder. The coverage check copies the lib's
  `subject_covered` semantics as of br-rust-common v1.3.0 (inner `>` is a
  literal, `*` is exactly one token, an empty pattern or subject covers
  nothing); the lib's function is crate-private, so nothing gates the mirror.
- **A durable is untouched by an outage** — it keeps its filter and its
  position: a message stored before the narrowing survives it and is delivered
  after `restore()`, ahead of anything published since.
- **An outage rewrites a GLOBAL fixed stream**, so a withholding scenario needs
  its own `start()` server or a serialized `connect(url)` group — it makes every
  concurrent scenario's publishes fail, and a lost guard is never repaired
  (`get_or_create_fixed_stream` returns early on an existing stream) — on a
  persistent broker the narrowing outlives the run until gitops re-declares the
  stream; never run one against a broker whose streams gitops declares for
  anything but tests. The two `withhold_*_stream()` methods go beyond #97's
  per-subject toggle and are a deliberate addition (they overlap
  `withhold_*_subject(&c, &[])`); they are the practical form for a
  best-effort-delivery scenario, where the `_subject` form would require
  enumerating every coordinate to keep.

#### `br-test-harness` — the `spawned-nats` slice is provably fabric-free

- **A CI gate pins `spawned-nats` to zero `br-rust-common` crates.**
  `cargo tree -p br-test-harness --no-default-features --features spawned-nats
  -e normal` must not mention `br-rust-common`, and the slice must `cargo check`;
  both run in the `fmt-clippy-test` job. `SpawnedNats` needs only `tokio` +
  `tempfile` and already exposes `url()` / `shutdown()`, so br-rust-common can
  dev-depend on this crate by tag for a per-test broker with **no dependency
  cycle** — and no innocuous feature edit can quietly re-create one. Stated in
  the crate README beside the feature table. **Deferred:** actually moving
  br-rust-common's `fabric_e2e.rs` off its ambient `NATS_URL` onto `SpawnedNats`
  is a change in *that* repo and is not part of this release; only the harness
  side of the enabler ships here.

#### Workspace re-pin to `br-rust-common` v1.3.0

- **`FabricTestNats::attach_without_provisioning(url)`** — attaches to an
  existing NATS without get-or-creating the two fixed streams, for a caller whose
  job is to *observe* the topology rather than establish it.
- **`FabricTestNats::durable_filter_subjects_if_present(stream, durable) ->
  Option<Vec<String>>`** — the non-panicking sibling of
  `durable_filter_subjects`, so "the durable is absent" is a value rather than a
  panic.
- **`BareFabricNats::assert_missing_stream_on_bind` /
  `assert_missing_command_stream_on_bind` / `event_stream_absent`** — the bind-path
  guard of the never-auto-provision invariant. Until 1.3.0 the `verify_*_durable`
  sites covered it incidentally, because the probe created the consumer; now that
  the probe creates nothing, `ensure_*_durable` against a NATS with no fixed
  stream needs its own black-box assertion. Exercised by a new
  `conformance-nats-fabric` check
  (`a_missing_fixed_stream_fails_the_durable_bind_loud_and_provisions_nothing`)
  and a harness test, both asserting `Consume(NoStream)` **and** that the stream
  is still absent afterwards.
- **`conformance-directory` C6 — the directory stager path, black-box**
  (`stager_stages_in_the_projection_transaction`, `CheckId::ConsumerStagerTransaction`,
  code `c6`). It registers a real `ImpactStager` that writes every
  `Impact::ForeignChanged` into an **adopter-owned** `conformance_impacts`
  table on the very `PgConnection` the sink hands it, over real NATS + real
  Postgres, and pins five properties of br-rust-common v1.3.0's transactional
  sink: (a) **atomicity** — a committed roster write and its impacts are durable
  together, and the stager reads the still-uncommitted roster row through its own
  `conn`; (b) **rollback** — a stager that refuses one key leaves **every column**
  of that key's `known_users` row exactly as it was, while a lower-ordered sibling
  key stays converged and the refused value converges on the next accepting
  reconcile; (c) the **impact set** of the six roster writes the Go anchor can
  drive — user upsert stages that user; a group upsert that *adds* members stages
  the group only; one that *drops* a member stages the group **plus** the removed
  member; a name-only group upsert stages the group; a user delete stages that
  user **plus** every group still holding it; a group delete stages the group
  **plus** every member the cascade unlinks (the stager-only `GroupSink::retract`
  branch); (d) a converged mirror stages nothing; (e) a projector with **no**
  stager registered converges the roster and stages nothing at all. The
  service-account sink is **not** exercised — the anchor publishes no
  service-account key. `RecordingStager`, `StagerFault`, `StagedImpact`,
  `IMPACT_TABLE` and the `create_impact_table` / `staged_impacts` /
  `clear_impacts` helpers are exported beside the check, so an adopter can wire
  its own impact table. Runs under the crate's existing `--test-threads=1` mode.
- `tests/fabric_nats_cli.rs` — real-infra coverage of the `verify` subcommand:
  it fails loud without creating the fixed streams it probes or the KV bucket the
  manifest declares, reports every failing entry, fails while the durable is
  absent and passes once provisioned, and rejects a durable whose filter is not
  the coordinate.

#### `conformance-passport` — seal side frozen in the Go anchor

- **P9 — an unreadable envelope fails closed.** The frozen contract already required
  it; nothing covered it. A bearer that resolves is replaced, at its own KV key, by a
  genuine **openable** seal carrying one unknown field, so only the strict parse can
  reject it → anonymous.
- **P10 — a tampered nonce fails closed.** Same shape, with byte 0 of the envelope's
  nonce flipped: the carried nonce no longer matches the tag → anonymous.
- P7, P9 and P10 share one *resolved-then-corrupted* shape, and their corrupt vectors
  are **controlled twins**: each is generated from the exact sealed bytes of its
  faithful vector — same token, same key, same nonce, same ciphertext — with only the
  declared mutation applied (flip byte 0 of the base64-decoded ciphertext or nonce,
  or add one unknown field to the same envelope). The faithful half must resolve
  first, then the twin is written at the same KV key, so the mutation is provably the
  only difference between the two resolutions rather than one of many. Both languages
  enforce it: `vector_twins_test.go` re-applies each mutation to the faithful bytes
  and byte-compares the result with the committed corrupt half, and Rust re-checks
  the same rule while parsing the vector file, so a non-twin file panics before any
  scenario runs. The check closes the table rather than a hard-coded list of three:
  any entry declaring a `corruption` must be named as a corrupt half by the twin
  table, and a corrupt half must declare its own mutation. (P6's `wrong-key` is the
  one negative vector with no twin — a KV key holds a single value — so it rests on
  a `pl_get_raw` presence assertion instead.) (P6 is not of that shape — it seeds the wrong-key vector once and
  asserts the value is present before checking the endpoint went anonymous.)

### Changed

**Deliberate behaviour changes — a suite that newly fails is a false pass
surfacing, not a regression.**

1. **`SseSubscription::expect_silence` now panics when the server closed the
   stream** — the deliberate *semantic* change: a closed stream is not silence.
   In 1.1.3 a quiet window and a stream end were the same `next_event() -> None`,
   so every `expect_silence` (and every drain-to-quiet loop) on a subscription
   the service had hung up on passed **vacuously** — a test that asserted
   nothing now fails.
2. **An SSE block left unterminated when the stream ends now fails loud.** The
   reader frames on `\n\n` **only**, so anything still buffered when the server
   hangs up is a truncated push — and a CRLF-framed (`\r\n\r\n`) body, though
   legal SSE, is *entirely* residual because the splitter never cuts it. In 1.1.3
   both vanished into a `next_event() -> None` a scenario read as silence.
   `next_outcome` panics naming the residual bytes instead. Deliberately **not**
   CRLF support: the harness refuses to certify a silence it did not observe
   rather than guess at a framing it does not implement.
3. **Two `WsSubscription` messages move**, both from the single 1.1.3 close arm
   (`ws: server closed: {c:?}`, which covered a close frame with *and* without a
   payload): a close frame **with** a payload now renders as the stable
   `ws: server closed: code={code} reason={reason}` (never tungstenite's `Debug`,
   which a dependency bump could silently reword), and a close frame **without**
   one now renders as ``ws: socket closed before a `next` push`` — the same fact
   as an exhausted stream — instead of `ws: server closed: None`. Every other
   `next_data` / `next_matching` string is byte-identical to 1.1.3.
4. **`WsSubscription::open` / `open_at` / `open_with` / `open_at_with` trim a
   trailing `/` on the base URL.** In 1.1.3 a base ending in `/` — the shape a
   `GATEWAY_WS_URL`-style environment value often takes — built a `//graphql/ws`
   path, which no router matches, so the handshake failed with a connect error.
   Existing callers passing a slash-terminated base now reach the intended path;
   a base without a trailing slash is unaffected.
5. **The harness call sites that *assert* the anti-over-delivery narrow-back moved
   to `ensure_event_durable`** (`conformance-nats-fabric`'s
   `assert_widened_durable_converges` and the harness's own
   `a_widened_durable_is_converged_back_to_the_exact_filter`): br-rust-common
   v1.3.0 turned `verify_command_durable` / `verify_event_durable` into a
   stream-coverage probe that **creates nothing**, so under 1.3.0 only `ensure_*`
   converges a widened durable back to the coordinate filter. The assertions are
   unchanged — they are the only black-box guard of that guarantee.
   `start_provisions_the_two_fixed_streams_and_a_filter_identical_durable` keeps
   `verify_command_durable` as the coverage probe and now proves the filter
   identity through `durable_filter_subjects`. The two `BareFabricNats` negative
   paths keep `verify_*` (the `Consume(NoStream)` fail-loud is unchanged) with
   expect messages reworded to what they now prove.
6. **`fabric-nats verify` is a genuine read-only check.** Because the lib probe
   now creates nothing, the old `verify` would have printed `ok cmd <durable>`
   for a NATS carrying no durable at all. It now checks, per manifest entry,
   (a) the fixed stream exists and its `subjects` cover the rendered coordinate
   (the lib probe, `Consume(NoStream)` / `SubjectNotCovered`) **and** (b) the
   durable exists on that stream filtering **exactly** that coordinate; either
   miss exits `4` naming the stream, the durable and the filter found, and the
   `ok` line states both checks. It now covers the `[published_language]` /
   `[bearer_tokens]` manifest flags too (bucket presence, read-only), reports
   **every** failing entry rather than only the first, and discriminates an
   absent stream from an uncovered subject in the message.
7. **`conformance-passport`'s `ALL` grows from 8 to 10 mandatory scenarios**
   (P9 and P10). A consumer asserting `report.passed() == ALL.len()` picks them
   up automatically; one pinning the literal `8` must move to `ALL.len()`.

#### `br-test-harness` — SSE and WS subscription handles

Non-behavioural, text only:

- `expect_event` / `expect_event_on` panic messages name the outcome they got
  (`got Timeout: …` / `got Closed: …`) instead of `got none`. Observable only to
  a consumer asserting on the old text (`#[should_panic(expected = "got none")]`).

#### Workspace re-pin to `br-rust-common` v1.3.0

- **Every `br-rust-common` pin in the workspace moves from `v1.2.0` to
  `v1.3.0`** — 25 declarations across the 6 workspace-member `Cargo.toml`
  carrying one (`{ tag, version }` both bumped; no workspace-level pin
  introduced). `conformance-passport` is among them: it now pins
  `br-rust-common` alone (see its own entry below).
  `conformance-directory` C1–C5 therefore re-run against 1.3.0 on the
  **no-stager** path — where a single-statement projection still runs on a pooled
  connection, exactly as in 1.2.0 — and the new **C6** covers the stager
  path and the transactional sink. The workspace resolves to a single
  br-rust-common source.
- Lockfile: `chacha20` `0.10.0` → `0.10.2` (the resolved version was yanked;
  `cargo deny check` advisories were failing on it). It reaches the tree through
  `async-nats` → `rand`, unrelated to the re-pin itself.

#### `conformance-passport` — seal side frozen in the Go anchor

- **`conformance-passport` no longer depends on `svc-auth`.** The crate imported
  `br-auth-contract` + `br-auth-identity-util` for one job — seeding — which
  transitively pinned br-rust-common and made every lib bump break the crate (the
  1.1.2 exclude / 1.1.3 re-enter dance). Its only BotResources dependencies are now
  `br-rust-common` (`br-core-auth`, `br-util-nats-fabric`) and the sibling
  `br-test-harness`. The unused `br-core-kernel` dep is dropped with them, and the
  now-unused `svc-auth` entry leaves `deny.toml`'s git source allow-list.
- **The sealed wire is frozen as committed vectors produced by the Go anchor.** A
  contract that must stay constant is frozen by an independent implementation, never
  by a Rust contract crate — otherwise Rust and the wire evolve together silently.
  `crates/conformance-passport/vectors/passport-wire-v1.json` carries, per case, the
  token, the KV key, the sealed identity and the exact `value_b64` bytes; the
  `identity-passport` anchor generates it deterministically (`make vectors`: fixed
  keys, fixed ids, one fixed nonce per **token** — so distinct tokens have distinct
  nonces and a corrupted twin shares its faithful half's), and `vectors_test.go` regenerates it
  in memory and asserts **byte-equality** with the committed file, so anchor drift
  or a hand edit fails `make check` and CI. `SealedSeeder` `include_str!`s the file
  and writes `value_b64` verbatim through `pl_put_raw`; Rust never parses, builds or
  mutates a sealed envelope. A vector may declare `resolves: unasserted` — used for
  the service actor, whose *cleartext* is frozen but whose *resolution* this contract
  deliberately leaves to the subject.
- **The subject under test only ever serves.** The battery no longer asks any binary
  to seal, so a consuming service driven by `run_spawn` needs no credential-forging
  subcommand — `SubjectConfig` / `Subject::spawn` / `run_spawn` keep their contract,
  and `BEARER_SEAL_KEY` now comes straight out of the vector file. The anchor's
  `seal` subcommand remains a dev-time generator only and takes its key from
  `BEARER_SEAL_KEY`, exactly as serve mode does.
- `PassportHarness::seeder()` is now synchronous and infallible, and
  `wrong_key_seeder()` is gone (the wrong-key case is a vector, not a second
  publisher). `SealedSeeder::seed` takes a `Vector` and the harness; `SealedSeed`
  carries its `kv_key`. `CheckContext` loses its `namespace` field — vector tokens
  are fixed, so per-run namespacing no longer exists. `SEAL_KEY` / `WRONG_SEAL_KEY`
  / `seal_key()` / `wrong_seal_key()` are replaced by the vector file plus
  `vectors::seal_key_b64()`.
- **CI now runs the Go anchor's tests** — a new `identity-passport anchor tests` step
  in `infra-e2e` (`go vet` + `go test -count=1`), placed before the passport battery.
  Without it the byte-equality guard on the committed vectors never executed in CI,
  so the "a hand edit fails CI" claim in the READMEs was not yet true. The Makefile
  `test` target gains `-count=1` (Go caches across edits of the out-of-package vector
  file), and `check` no longer depends on `fmt` (which rewrote sources): it runs a
  `fmt-check` instead, with `fmt` kept as its own target.
- CI runs the passport battery with `--test-threads=1`, matching the README.

**Migration.** Every `br-rust-common` pin in a consuming repo must move to
`v1.3.0` in the same change that takes this tag: two refs of one git URL are two
distinct sources and duplicate `br-core-*` in the graph. No harness signature
changed, so a suite on 1.1.x compiles untouched — what it may hit is the class-B
false passes above surfacing as new failures (a vacuous `expect_silence` on a
closed stream, a drain-to-quiet loop over an ended stream). Those are the bug the
release exists to expose; budget for the assertions, not for the edits. A
consumer pinning `conformance-passport`'s scenario count as a literal moves to
`ALL.len()`; one driving a subject through `run_spawn` needs no change (the
battery no longer asks any binary to seal, and needs no `go` on `PATH`).

**Deferred.** The consumer-side follow-ups this release enables stay in their own
repos and are not part of it: svc-jobs dropping the `Budget` / `ended_early`
heuristic now that `Closed` is readable, svc-notifier's drain loops, bma-identity's
five drain-to-quiet loops and its `assert_stream_stays_open` helper (now
redundant), be-botresources.ai's svc-identity / svc-tasks helpers (optional:
better panic messages), the deletion of the gateway's duplicated
`crates/e2e/src/ws.rs` in favour of `WsCredential`, and un-ignoring svc-charter
`s19` on `DeliveryOutage`. The Rust-side guard for
svc-auth's `br-auth-contract` — the pin `conformance-passport` dropped — belongs
to the svc-auth repo. Migrating br-rust-common's own `fabric_e2e.rs` off an
ambient `NATS_URL` onto `SpawnedNats` is a br-rust-common change (the harness
side of it ships here, see Added).


### Fixed

- **`fabric-nats verify` no longer provisions the topology it is checking.** It
  attaches through the new `FabricTestNats::attach_without_provisioning`, so the
  subcommand stops get-or-creating the two fixed streams it exists to probe. This
  was a 1.1.3 bug — `verify` attached through `FabricTestNats::connect`, which
  get-or-creates the two fixed streams — not a consequence of the v1.3.0 re-pin;
  the re-pin only made it visible by turning the durable probe read-only.


## 1.1.3 - 2026-07-23

### Changed

- `conformance-passport` re-enters `[workspace] members` and its CI battery step
  is restored in its original slot — closing the temporary exclusion of 1.1.2.
  svc-auth has shipped `v1.0.4` (br-rust-common v1.2.0 aligned), so the whole
  tree resolves to a single br-rust-common source again. Its pins move to
  br-rust-common `v1.2.0` and svc-auth `v1.0.4` (crate versions unchanged);
  README/docs example pins de-frozen. The frozen G1 bearer→Passport wire
  contract is untouched — only the pins move.

## 1.1.2 - 2026-07-22

### Changed

- Bump `br-rust-common` pins from `v1.1.0` to `v1.2.0` (dependency-only patch:
  picks up the NoResponders-recoverable consumer fix and the supervised
  `PublishedLanguageConsumer::run()`; no harness surface change). Note the
  v1.2.0 `WatchHealth` semantics: the channel starts `Degraded` and only the
  supervised loop writes it — a battery asserting health on a raw `watch()`
  must drive `run()` instead.

### Removed (temporary)

- `conformance-passport` excluded from `[workspace] members` and its CI battery
  step dropped — same maneuver as harness 1.1.0: the crate bridges svc-auth's
  `br-auth-contract`/`br-auth-identity-util` (newest tag `v1.0.3`, transitively
  pinning br-rust-common **v1.1.0**) with the now-v1.2.0 workspace, and cargo
  cannot unify two git tags of one package. The crate stays in-tree with its
  pins frozen (br-rust-common v1.1.0 / svc-auth v1.0.3); it re-enters once
  svc-auth ships its v1.2.0-aligned tag.

## 1.1.1 - 2026-07-12

### Changed

- **`conformance-passport` re-enters the workspace and its CI battery step.** It
  was temporarily excluded in 1.1.0 because it bridges the `svc-auth`
  `br-auth-contract` / `br-auth-identity-util` crates (then pinned to
  `br-rust-common` `v1.0.2`) with the now-`v1.1.0` `br-test-harness`, and Cargo
  cannot resolve two git tags of one `br-rust-common` to a single source.
  `svc-auth` has since shipped `v1.0.3`, which pins `br-rust-common` `v1.1.0` and
  `br-test-harness` `v1.1.0` — so the whole tree now resolves to a single
  `br-rust-common` source. The battery's manifest moves to `br-rust-common`
  `v1.1.0` (`br-core-auth`, `br-core-kernel`, `br-util-nats-fabric`) and to
  `svc-auth` `v1.0.3` (`br-auth-contract` / `br-auth-identity-util`, both crate
  version `0.2.0`). The frozen G1 bearer→`Passport` wire contract is unchanged;
  only the pins move. The passport battery step is restored to
  `.github/workflows/ci.yml`.

## 1.1.0 - 2026-07-12

### Changed

- **The workspace pins `br-rust-common` `v1.1.0`** (was `v1.0.2`), across
  `br-test-harness` and the conformance batteries. v1.1.0 is an additive minor:
  the managed fabric run-loop now **auto-recovers transient consume errors**
  (rebind + backoff — a bounded budget for `Other`, unbounded retry for the new
  `ConsumeErrorKind::HeartbeatMissed`, while `NoStream` stays terminal), and the
  `br-core-values` reason codes moved to `lower_snake` (`locale_unknown`,
  `money_out_of_range`, `primary_content_missing`), with `Affordance` reason
  constructors now panicking on any non-`lower_snake` code. No harness or
  conformance code needed adapting: no suite asserted a heartbeat-driven loop
  death, referenced `ConsumerGone`, or emitted an upper-case reason code. The
  `NoStream` fail-loud assertions in `conformance-nats-fabric` are unchanged
  (`NoStream` remains terminal). The Go anchor's byte-for-byte subject/wire
  checks pass unchanged.

- **`conformance-passport` is temporarily removed from the workspace and its CI
  battery step** because it bridges the `svc-auth` `br-auth-contract` /
  `br-auth-identity-util` crates (still pinned to `br-rust-common` `v1.0.2`) with
  the now-`v1.1.0` `br-test-harness`, which cannot resolve to a single
  `br-rust-common` source. Its own manifest stays on `v1.0.2`. It re-enters the
  workspace once `svc-auth` ships a release pinned to `br-rust-common` `v1.1.0`
  (the next lockstep step); the code is source-compatible with `v1.1.0`.

## 1.0.5 - 2026-06-29

### Changed

- **conformance-passport P8 renamed `KvErrorFailsLoud` (was `KvErrorIs500`) and
  loosened from `== 500` to a fail-loud contract.** The property P8 actually guards
  is *no silent fail-open*: when the `PUBLISHED_LANGUAGE` bucket is destroyed under
  a live subject, the loss must surface **loudly** — either as a **5xx** or by the
  resolver becoming **unreachable** (the stream deletion drops its NATS connection /
  the process exits, a fail-loud outcome consistent with BR doctrine). Both are
  correct; only a **200** (anonymous or resolved) is the real failure, because it
  would let the request proceed as a valid anonymous call. The old strict `== 500`
  **reds on a loud connection-drop**: the real gateway reference
  (`example-svc-identity`) becomes unreachable on stream deletion in integrated CI,
  which is *not* a security failure, yet the strict check failed it. P8 now PASSes
  iff the re-resolve yields a 5xx **or** a transport error, and FAILs on any 2xx or
  other non-5xx status. A **pre-deletion health guard** was added: the subject must
  resolve the seed *before* the destructive step, so a later unreachability can only
  be attributed to the infra loss (never a boot failure). The `Ok(status)`-vs-`Err`
  verdict was extracted to a pure helper with offline unit tests (500/503/transport
  → pass; 200/401 → fail). The scenario code string `"p8"` is unchanged.
  conformance-passport is exempt from cargo-semver-checks, so the rename is safe.
- **`anyhow` 1.0.102 → 1.0.103** (Cargo.lock-only, transitive). Patches
  **RUSTSEC-2026-0190**, an unsoundness advisory in `Error::downcast_mut()`; the
  patched release exists, so no `deny.toml` ignore is added. `cargo deny` advisories
  are clean after the bump.

## 1.0.4 - 2026-06-29

### Changed

- conformance-passport provisions `PUBLISHED_LANGUAGE` in-process via
  `with_published_language()`, dropping the redundant `fabric-nats` CLI
  provisioning step. The battery no longer depends on the `fabric-nats` binary, so
  consuming services need neither a `nats-fabric` feature nor a fabric-nats build
  step — `import + run_spawn` is the whole integration. Pure refactor: no behavior
  or fidelity change (the subject still binds the pre-existing bucket; P8 still
  proves KV-error→500).

## 1.0.3 - 2026-06-29

### Changed

- **`conformance-passport` migrated to the sealed/AEAD bearer model.** The G1
  battery now resolves bearers stored as `br_auth_contract::SealedBearer`
  (ChaCha20-Poly1305, RFC 8439 — random 12-byte nonce, 16-byte tag) in the fixed
  `PUBLISHED_LANGUAGE` bucket at `bearer_token_kv_key(tok)` =
  `"identity/bearer_tokens/" + sha256hex(tok)`, **AAD = the unprefixed digest**.
  The sealed cleartext is `br_auth_contract::BearerEntry { actor, token_id }` — it
  carries **no email**, so the resolved `Passport::Human` has `user_id` = the
  sealed actor's `UserId` and **empty `claims`** (the retired plaintext
  `bearer_tokens` / `BearerTokenEntry { email, token_id }` model is gone).
- **Seeding goes through the real Rust lib.** A new in-crate `SealedSeeder` wraps
  `br_auth_identity_util::BearerPublisher` (seals + writes / retracts on the
  existing `with_published_language` PL seam) — no new `br-test-harness` seeding
  API. The Go anchor independently **opens** the Rust-sealed envelope with its own
  `golang.org/x/crypto/chacha20poly1305`: genuine Rust-seal / Go-open
  cross-language interop pins the whole crypto contract (the random nonce means the
  ciphertext bytes are not frozen, the *contract* is).
- **Subject env contract realigned** to `NATS_URL` + `PORT` + `BEARER_SEAL_KEY`
  (base64-std of the 32-byte key); dropped `HTTP_ADDR` and `BEARER_BUCKET` (the
  bucket is the fixed `PUBLISHED_LANGUAGE`). Matches the gateway `svc-identity`
  example exactly so `run_spawn` drives both the Go anchor and a real reference.

### Added

- **Fail-closed + infra-error checks.** P6 `WrongSealKeyFailsClosed` (an envelope
  sealed under a different key → anonymous, never a wrong identity), P7
  `TamperedEnvelopeFailsClosed` (the stored ciphertext is byte-flipped → AEAD tag
  fails → anonymous), and P8 `KvErrorIs500` (the `PUBLISHED_LANGUAGE` bucket is
  destroyed under the live subject → resolution returns 500, never silently
  anonymous). P8 is destructive, so `run_spawn` always runs it last. P1/P5 now
  assert `user_id` to the exact sealed value and that `claims` carries no email.
- **Go-anchor retired-model guard.** `make guard` in `identity-passport` fails loud
  if any retired marker (an `email` JSON tag, a `userIDFromEmail` / `uuid.NewSHA1`
  derivation, or the old plaintext entry type) reappears in source; the Rust build
  runs it before `go build`.
- **`FabricTestNats::delete_published_language()`** — additive harness helper (the
  P8 destructive hook), keeping `async-nats` confined to `br-test-harness`.

### Dependencies

- `conformance-passport` adds `br-auth-contract`, `br-auth-identity-util`
  (svc-auth `v1.0.2`), `br-core-kernel`, `br-util-nats-fabric` (`v1.0.2`), plus
  `base64` / `serde_json`.

## 1.0.2 - 2026-06-19

### Added

- **`FabricTestNats::with_ephemeral_auth()`** — provisions the sanctioned
  `EPHEMERAL_AUTH` KV bucket (mirroring `with_published_language()`) so a service
  e2e can stand it up without importing `async-nats`. The bucket is created with
  the lib's canonical config — `history = 8`, `max_age = 3600s`, and
  `limit_markers = 1s` (async-nats 0.48's `subject_delete_marker_ttl`) — so
  per-key `create_with_ttl` writes actually expire, matching `EphemeralAuthStore`'s
  prod behavior. New `connect::get_or_create_ephemeral_auth` sibling.
- **KV bucket inventory:** `FabricTestNats::kv_bucket_names()` enumerates the live
  `KV_*` JetStream streams and returns the stripped bucket-name set, and
  `assert_only_kv_buckets(&[&str])` panics with an `expected … got …` diff when the
  live set differs — the primitive a service uses to prove no stray bucket was
  created. Plus `ephemeral_auth_present()`.

## 1.0.1

### Changed

- **The workspace pins `br-rust-common` `v1.0.2`** (was `v1.0.1`), across
  `br-test-harness` and every `conformance-*` crate, to break the diamond skew
  with services that already consume `br-rust-common` `v1.0.2`. v1.0.2 is additive
  over v1.0.1 (fabric durable-consumer surface + the ephemeral-auth KV); the
  harness uses none of the new *positive* surface. The harness own-version moves to
  its own next patch `1.0.1` — it tracks `br-rust-common` only at the major level,
  not number-for-number.
- **The NATS-Fabric durable-bind conformance now proves convergence, matching
  v1.0.2's repurposed `verify_*_durable` / `FilterMismatch` contract.** In v1.0.2
  `verify_command_durable` / `verify_event_durable` became create-or-bind readiness
  gates delegating to `ensure_command_durable` / `ensure_event_durable`: a
  pre-existing durable left widened on `integration.evt.>` is **narrowed back** to
  the exact rendered coordinate filter (the anti-over-delivery guarantee) and the
  call returns `Ok` — it no longer errors. `FabricError::FilterMismatch` is
  repurposed to guard only the empty-coordinate-set case (which would vacuum the
  whole stream). The `br-test-harness` self-test
  `a_widened_durable_is_converged_back_to_the_exact_filter` (was
  `a_widened_durable_makes_the_lib_bind_fail_with_filter_mismatch`) now binds a
  widened durable, asserts `Ok`, and reads the durable's `filter_subject(s)` back
  from the broker to prove the filter was narrowed to the exact coordinate and is
  no longer `integration.evt.>`. The `conformance-nats-fabric`
  `a_widened_durable_is_rejected` check is likewise rewritten to
  `a_widened_durable_is_converged_back_to_the_exact_filter`. The
  `create_durable` → `ensure_*_durable` provisioning simplification stays
  **deferred** to lib gap `ws-cc-platform#93` and is intentionally not part of this
  bump.
- **`cargo semver-checks` now scopes to `br-test-harness`; the `conformance-*`
  battery crates are exempt.** A conformance battery is an executable spec: when the
  lib repurposes a behavior or error, the matching assert-helper *must* rename (here
  `conformance-nats-fabric::assert_widened_durable_rejected`), which `semver-checks`
  reads as a major break — a false demand that collides with the harness tracking
  `br-rust-common` at the major level only. The reusable library surface
  (`br-test-harness` fixtures) stays fully gated; the conformance assert-helpers,
  consumed through the battery runner rather than individually, are not.

## 1.0.0

### Added

- **`br-test-harness` — `FabricTestNats` connect-mode + the typed
  observation/publish/capture surface + the `fabric-nats` bin (#74, phase 1).**
  The Fabric provisioner gains `connect(url)` (attach to an already-running NATS,
  alongside `start()` which spawns its own) backed by a `NatsBacking { Owned |
  Attached }` so `shutdown()` only tears down a server the harness started — it
  never kills a shared NATS. **All provisioning is get-or-create** (fixed streams,
  PL bucket, bearer bucket), the structural fix for the #73 shared-bucket-wipe
  class. A typed surface so a test body never needs a raw handle: `fabric_owned()`,
  `capture_events` / `capture_commands` (background-drain, correlation-keyed:
  `count`/`first`/`for_correlation`/`correlation_ids`/`stop`, on one
  harness-internal consumer), `await_event` (wraps the lib `CorrelatedAwaiter`) +
  `await_command` (the command-stream counterpart), `pl_publisher` / `pl_reader`
  (delegate to the lib), `pl_list` / `pl_get_meta` (hand-rolled key-scans —
  `PublishedLanguageReader` has no list), `pl_put_raw` (the adversarial raw hatch),
  `with_bearer_tokens` + `bearer_seeder` (`BearerSeeder` moved in from
  `conformance-passport`, typed on `BearerTokenEntry`), and the negative methods
  `assert_missing_stream` / `publish_dead_subject` / `raw_message_absent`. A new
  `fabric-nats` bin (`required-features = ["nats-fabric"]`) is a thin shell over
  the same provisioner — `provision` / `verify` / `print-subjects` over a TOML
  manifest that speaks coords + durable names + a bucket flag, **never a raw
  subject** (exit codes 0/2/3/4). `CapturedMessage`, `FabricKvError`,
  `BearerSeedError` and `ManifestError` are harness-owned; the lib's
  `jetstream()` / `client()` handles stay public for now (phase-2 seal demotes
  them). The two hand-rolled lib gaps (command-side `await`, reader `keys()`) are
  noted for `br-util-nats-fabric` nice-to-haves.
- **`fabric-nats` manifest gains a `[bearer_tokens] enabled` flag (#74, phase 2b).**
  Additive, alongside `[published_language]`: when set, `provision` calls
  `with_bearer_tokens()` to get-or-create the `bearer_tokens` KV bucket and prints
  `kv bearer_tokens`. This lets a passport suite — which uses only the bearer bucket,
  not the Fabric streams — provision through the same CLI handshake as every other
  suite, with no in-binary special-casing. Exit codes and all other behavior are
  unchanged.
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
- **The three new real-infra batteries are wired into CI.** The `infra-e2e` job
  now runs `conformance-nats-fabric`, `conformance-passport` and
  `conformance-directory` with `-- --ignored` (directory at `--test-threads=1`,
  it provisions a throwaway role + database per Cx check), alongside the existing
  `br-test-harness` / `conformance-scope` / `conformance-scope-cli` /
  `conformance-identity` steps. The README "CI covers this" claims on those three
  crates are now enforced, not documentary.

### Changed

- **`br-test-harness` gains `workspace_bin(name)` + `spawn_fabric_provision(url,
  manifest)`; the four conformance crates stop hand-rolling the binary locator.**
  `workspace_bin` derives the built binary's path from `current_exe()` (pop, then
  pop a `deps/` layer) instead of guessing `../../target/<profile>/` from a
  `cfg!(debug_assertions)` profile — which broke under a custom `CARGO_TARGET_DIR`,
  a `--release` test run, or a relocated `target`. `spawn_fabric_provision` hoists
  the provision-spawn body (locate `fabric-nats`, `run_once`, status check) that
  `conformance-{scope,identity,passport,directory}` each duplicated; every
  `provision.rs` now keeps only its thin `map_err` into its own
  `ConformanceError`. The `conformance-nats-fabric` test binary-locator routes
  through `workspace_bin` too. Both helpers are always-compiled (std + `tokio`).
- **`fabric_nats::subscribe_command` (was `await_command_consumer`).** Internal
  rename only — the function is module-private to the Fabric capture path and was
  never re-exported; the public `await_command` / `CommandAwaiter` surface is
  unchanged. The name no longer implies a JetStream consumer: it opens a core-NATS
  `Subscriber` (no replay), because the lib's awaiter binds `INTEGRATION_EVT` only.
- **The seal: `async-nats` is confined to `br-test-harness`; the raw JetStream /
  client handles are removed from the typed surface (#74 complete, phase 2c).**
  `FabricTestNats::jetstream()` / `client()` and `BareFabricNats::jetstream()` are
  gone — no Fabric type hands back a raw `async-nats` handle. The last in-harness
  consumers (the harness's own `fabric_nats` / `fabric_smoke` self-tests) route
  through new typed observers instead: `fixed_streams_present`,
  `published_language_present`, `pl_get_raw`, `publish_event_envelope`, and on
  `BareFabricNats` `command_stream_absent` / `assert_missing_command_stream`. With
  this, **`br-test-harness` is the sole crate in the workspace that depends on
  `async-nats`**: every conformance battery provisions via the `fabric-nats` CLI
  and observes via the typed surface, so none can reach a bare handle — and the
  workspace compiling with the handles unexposed *is* the completeness proof. This
  closes #74: connect-mode + get-or-create provisioning, the typed
  capture/await/KV/bearer/negative surface, the `fabric-nats` bin
  (provision/verify/print-subjects over a coords-only TOML manifest including the
  bearer flag), and the six conformance suites standing as the CLI's testbed. The
  two hand-rolled lib stand-ins remain noted for `br-util-nats-fabric`
  nice-to-haves: the command-stream `await` and the PL reader `keys()`/`entries()`
  enumeration that `pl_list` / `pl_get_meta` hand-roll today.
- **`conformance-scope`, `conformance-identity` and `conformance-nats-fabric`
  provision their NATS topology by spawning the `fabric-nats` CLI, and drop all
  `async-nats` (#74, phase 2a).** Each suite is now the CLI's real-life testbed:
  it spawns `fabric-nats provision --manifest <crate>/tests/fixtures/*.toml`
  against the harness URL (a TOML manifest of coords + durable names + a
  `[published_language] enabled` flag, never a raw subject), then drives and
  observes the run through the frozen typed `FabricTestNats` surface only —
  `fabric_owned()`, `capture_events` / `capture_commands`, `pl_reader` /
  `pl_publisher` / `pl_put_raw`, and `assert_missing_stream` /
  `publish_dead_subject` / `raw_message_absent`. The three crates no longer depend
  on `async-nats` (nor the now-unused `futures-util`): their hand-rolled
  `DeclareCapture` / `ConfirmationCapture` consumers become thin typed views over
  the harness `CommandCapture` / `EventCapture`, the acceptor publisher takes a
  `&Fabric` instead of a raw `jetstream::Context`, and attach-mode runners connect
  via `FabricTestNats::connect`. `grep -rn async_nats` over the three crates is
  empty. The directory/passport suites still use the public raw handles
  (untouched here; the handle demotion is the phase-2c seal).
- **`conformance-directory` and `conformance-passport` provision via the
  `fabric-nats` CLI and drop all `async-nats` (#74, phase 2b).** Both suites now
  follow the phase-2a pattern: they spawn `fabric-nats provision --manifest
  <crate>/tests/fixtures/*.toml` against the harness URL and drive the run through
  the frozen typed `FabricTestNats` surface only. `conformance-directory`'s
  `read_users` / `read_groups` / `read_meta` re-home onto `pl_list` / `pl_get_meta`
  (no more raw `kv::Store` key-scan), `DirectoryHarness` drops its held `kv::Store`
  and CLI-provisions the `PUBLISHED_LANGUAGE` bucket (`[published_language]`
  manifest), and the C1 user-retract step orphan-deletes through a
  `DirectoryPublisher` reconcile instead of a raw `store().delete()` — same
  observable KV effect, via the typed publisher. `conformance-passport` deletes its
  local `BearerSeeder` (the harness owns it since phase 1), CLI-provisions the
  `bearer_tokens` bucket (`[bearer_tokens]` manifest), and seeds/revokes through
  `FabricTestNats::with_bearer_tokens()` + `bearer_seeder()`. Both crates drop
  `async-nats` (directory also `futures-util`, passport also `serde_json`);
  `grep -rn async_nats` over both is empty. The remaining public raw handles are
  untouched — their demotion is the phase-2c seal.
- **The dead-grammar guard is enforced on the real test path.** `build_anchor` /
  `build_subject` in `conformance-nats-fabric`, `conformance-identity` and
  `conformance-scope` now run `make -C <anchor-dir> guard` before `go build` and
  fail loud (surfacing stdout+stderr) on a hit, so the `make guard` grep for the
  dead pre-v1 `identity.cmd.`/`identity.evt.` grammar actually runs whenever the
  anchor is built — previously it was bypassed by the direct `go build` call.
- **C5 (`users_only_narrows_projection`) now proves *live* narrowing.** It runs
  `DirectoryProjector::watch()` as a concurrent task, publishes a fresh user
  (absent from the initial snapshot) into the Published-Language bucket while the
  watch runs, then asserts that user's row appears in `known_users` within a
  bounded deadline **and** the group tables still do not exist — turning a
  watch-timeout-as-success placeholder into a real live-PUT proof. Because the
  directory keys are slash-delimited, this also exercises `br-rust-common`
  v1.0.1's `watch_all` + client-side prefix filter on real NATS, making C5 a
  regression gate for that fix. New `publish_added_user` helper in
  `publish_fixture`.

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

- **`br-test-harness` Fabric get-or-create is idempotent — shared-NATS
  provisioning is race-safe, absorbs already-exists (#74).** The fixed-stream, PL
  and bearer get-or-create helpers did `get` → `create` → panic on any create
  error: a TOCTOU on a shared NATS where two processes both observed "absent" made
  the create-loser panic with the JetStream `stream name in use` / bucket-exists
  error, breaking the parallel-run guarantee. Create now matches the typed
  already-exists code (`ErrorCode::STREAM_NAME_EXIST`, 10058) and treats it as
  success — re-`get`ting the KV handle — and `published-language` reuses the
  bearer/bucket path. Still wipe-free; proven by the new real-infra
  `double_provisioning_a_shared_nats_is_idempotent_and_never_wipes` test.
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

## 0.6.0

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

## 0.5.2

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

## 0.5.1

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

## 0.5.0

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
  service-local `tests/common/gql.rs` (the BR reference service) so every
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

## 0.4.0

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

## 0.3.0

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

## 0.2.0

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

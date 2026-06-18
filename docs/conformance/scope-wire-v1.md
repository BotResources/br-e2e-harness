# G3 — Scope-Declaration Wire Contract (v1, FROZEN)

Authoritative on-wire spec for the BotResources scope self-declaration handshake.
Derived **from the real Rust types** in `br-rust-common` (`br-core-scope`, `br-core-integration`,
`br-core-events`, `br-util-scope-declaration`). All golden JSON below was produced by
serializing the actual crate values with `serde_json::to_string_pretty` (throwaway example,
since deleted), not hand-written.

This is the contract a real non-identity scope-owning service performs on boot, and the
contract the G3 Go test subject implements. A Rust conformance runner implements a **fake
acceptor** against this spec.

---

## 0. Roles

- **Declarer** (the subject under test, e.g. `svc-notifier`): on boot, publishes a
  `DeclareServiceScopes` command and waits for a confirmation, gating its `/readyz` on it.
- **Acceptor** (Identity / the fake): consumes the declare command and replies with either
  `ServiceScopesAccepted` or `ServiceScopesRejected`, echoing the command's `correlation_id`.

---

## 1. NATS subjects (literal, FROZEN)

Derived from `br-scope-declaration-contract` (`declare_command_coords` /
`accepted_event_coords` / `rejected_event_coords`) rendered by the
`br-util-nats-fabric` Fabric, with `receiver/producer="identity"`,
`aggregate="service_scope"`, `version=1`.
Format: command `integration.cmd.{receiver}.{aggregate}.{verb}.v{N}`,
event `integration.evt.{producer}.{aggregate}.{fact}.v{N}`. The `integration`
prefix is fixed and not caller-choosable.

| Role | Subject |
|---|---|
| **declare** (command, published by declarer) | `integration.cmd.identity.service_scope.declare.v1` |
| **accepted** (event, published by acceptor)  | `integration.evt.identity.service_scope.accepted.v1` |
| **rejected** (event, published by acceptor)  | `integration.evt.identity.service_scope.rejected.v1` |

`command_type` field value = `"service_scope.declare"` (`{aggregate}.{name}`, no bc, no version).
Event `event_type` values observed in the lib's stubs: `"service_scope.accepted"` /
`"service_scope.rejected"` — but note: the declarer does **not** inspect `event_type`. It routes
purely by the **NATS subject** the message arrived on (accepted-subject ⇒ Accepted,
rejected-subject ⇒ Rejected) and matches by `correlation_id`. The acceptor SHOULD set these
`event_type` values for fidelity, but only the subject + correlation_id are load-bearing.

---

## 2. Envelopes

### 2.1 `IntegrationCommand<T>` (br-core-integration/src/envelopes.rs)

Plain externally-untagged struct. Fields, in order:

| Field | JSON type | Notes |
|---|---|---|
| `command_id`   | string (UUID) | fresh **UUIDv7** per command instance (incl. each re-publish) |
| `command_type` | string | `"service_scope.declare"` |
| `version`      | number (u8) | `1` |
| `issued_at`    | string (RFC3339 / chrono `DateTime<Utc>`) | e.g. `"2023-11-14T22:13:20Z"` |
| `metadata`     | object | see §2.3 |
| `payload`      | object | the `DeclareServiceScopes`, see §3 |

### 2.2 `IntegrationEvent<T>` (same file)

| Field | JSON type | Notes |
|---|---|---|
| `event_id`    | string (UUID) | UUIDv7 |
| `event_type`  | string | `"service_scope.accepted"` / `"service_scope.rejected"` (informational) |
| `version`     | number (u8) | `1` |
| `occurred_at` | string (RFC3339) | |
| `metadata`    | object | see §2.3 — **must echo the command's `correlation_id`** |
| `payload`     | object | `ServiceScopesAccepted` / `ServiceScopesRejected`, see §4 |

### 2.3 `metadata` = `EventMetadata`/`MessageMetadata` (br-core-events/src/metadata.rs)

Custom `Serialize` — **flattened**, NOT nested under an `actor` object:

| Field | JSON type | Notes |
|---|---|---|
| `actor_id`       | string (UUID) | the actor's id |
| `actor_kind`     | string enum | `"human"` or `"service"` — **always emitted** on serialize |
| `correlation_id` | string (UUID) | **the matching key** (§5) |
| `causation_id`   | string (UUID) | **OMITTED entirely when `None`** (serde skips it) |

Deserialize is lenient: missing/`null` `actor_kind` ⇒ defaults to `"human"`; an *unknown*
`actor_kind` value (e.g. `"robot"`) is a hard error. The awaiter only deserializes `metadata`
(via a `CorrelationProbe` that reads `metadata.correlation_id`), so an acceptor's `metadata`
must at minimum carry a valid `correlation_id`; `actor_kind` may be omitted (defaults human).

The declarer's command metadata is built by `declaring_actor(service)` (br-util-scope-declaration/src/actor.rs):
`Actor::Service` with `actor_id = UUIDv5(ns=6f3a1c8e-4b27-4d59-9e10-a3f277c58d41, name=<service_key bytes>)`.
So a declarer's command has `actor_kind:"service"` and a deterministic `actor_id`. The acceptor
does NOT need to validate or reproduce this — it only echoes `correlation_id`.

---

## 3. Golden: declare command — `IntegrationCommand<DeclareServiceScopes>`

`DeclareServiceScopes` wraps a `RawScopeDeclaration` under a single field `declaration`
(messages.rs / raw.rs). `ScopeKey`/`ServiceKey` serialize as **plain strings**.
`ServiceManifest` = `{key, label_key, description_key}`; `ScopeSpec` =
`{key, label_key, description_key, platform_only}`.

Sample below uses service `notifier` with two scopes (`notifier:read` non-platform,
`notifier:admin` platform-only). **Variable fields** (`command_id`, `issued_at`,
`metadata.actor_id`, `metadata.correlation_id`) shown with concrete sample values; the
*structure* is the contract.

```json
{
  "command_id": "0190a1b2-0000-7000-8000-000000000001",
  "command_type": "service_scope.declare",
  "version": 1,
  "issued_at": "2023-11-14T22:13:20Z",
  "metadata": {
    "actor_id": "b10a8b19-5b18-53aa-b872-81dd00af0976",
    "actor_kind": "service",
    "correlation_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"
  },
  "payload": {
    "declaration": {
      "manifest": {
        "key": "notifier",
        "label_key": "label.notifier",
        "description_key": "desc.notifier"
      },
      "scopes": [
        {
          "key": "notifier:read",
          "label_key": "label.read",
          "description_key": "desc.read",
          "platform_only": false
        },
        {
          "key": "notifier:admin",
          "label_key": "label.admin",
          "description_key": "desc.admin",
          "platform_only": true
        }
      ]
    }
  }
}
```

Empty scope set is legal: `"scopes": []`.

---

## 4. Golden: confirmation events

### 4.1 Accepted — `IntegrationEvent<ServiceScopesAccepted>`

`ServiceScopesAccepted { service: ServiceKey }` → `{"service":"notifier"}`.

```json
{
  "event_id": "0190a1b2-0000-7000-8000-000000000002",
  "event_type": "service_scope.accepted",
  "version": 1,
  "occurred_at": "2023-11-14T22:13:20Z",
  "metadata": {
    "actor_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
    "actor_kind": "human",
    "correlation_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"
  },
  "payload": {
    "service": "notifier"
  }
}
```

The declarer's `resolve_match` for the accepted subject does **not** deserialize the payload at
all — it sets readiness UP on subject match alone. So `payload` shape on accepted is not strictly
load-bearing for the declarer, but the acceptor SHOULD emit the correct `{"service": "<key>"}`.

### 4.2 Rejected — `IntegrationEvent<ServiceScopesRejected>`

`ServiceScopesRejected { service: ServiceKey, reason: ScopeDeclarationError }`. The declarer
**does** deserialize the rejected payload (`IntegrationEvent<ServiceScopesRejected>`); a malformed
rejected payload is logged and ignored (readiness stays DOWN, keeps awaiting). So the `reason`
shape below IS load-bearing for a faithful acceptor.

`ScopeDeclarationError` is an **internally-tagged enum on key `"reason"`**, variants
`rename_all = "snake_case"`. Example (`ScopeOwnedByAnotherService`):

```json
{
  "event_id": "0190a1b2-0000-7000-8000-000000000003",
  "event_type": "service_scope.rejected",
  "version": 1,
  "occurred_at": "2023-11-14T22:13:20Z",
  "metadata": {
    "actor_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
    "actor_kind": "human",
    "correlation_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"
  },
  "payload": {
    "service": "notifier",
    "reason": {
      "reason": "scope_owned_by_another_service",
      "key": "notifier:read",
      "owner": "billing"
    }
  }
}
```

### 4.3 `reason` — all `ScopeDeclarationError` variants (golden, compact)

Tag key = `"reason"`. `InvalidScopeKey` nests a `KeyValidationError`, itself internally-tagged on
key `"validation"` (snake_case). Exact serializations:

```json
{"reason":"invalid_scope_key","key":"notifier:BAD","validation":{"validation":"invalid_charset"}}
{"reason":"invalid_scope_key","key":"notifierread","validation":{"validation":"malformed_segments"}}
{"reason":"invalid_scope_key","key":"<200 chars>","validation":{"validation":"too_long","max":128,"actual":200}}
{"reason":"scope_prefix_mismatch","scope_service":"billing","declaring_service":"notifier"}
{"reason":"duplicate_scope_in_declaration","key":"notifier:read"}
{"reason":"scope_owned_by_another_service","key":"notifier:read","owner":"billing"}
```

`KeyValidationError` variants (tag key `"validation"`):
`{"validation":"empty"}`, `{"validation":"invalid_charset"}`,
`{"validation":"malformed_segments"}`, `{"validation":"too_long","max":<n>,"actual":<n>}`.

The declarer logs `reason.reason` (the tag) and sets readiness DOWN with that string; it does
**not** branch on the specific variant. Any of the above is a valid "reject".

---

## 5. JetStream topology (what the declarer expects)

From `br-util-scope-declaration/src/handshake.rs` + `br-core-integration/src/awaiter.rs` +
`nats.rs`, and the lib's e2e harness (`tests/common/mod.rs`).

### 5.1 The two fixed streams (cmd vs evt)

- The declare command and the confirmation events live on **two different fixed streams**:
  `INTEGRATION_CMD` (subjects `integration.cmd.>`) carries the declare command, and
  `INTEGRATION_EVT` (subjects `integration.evt.>`) carries both event subjects. The names are
  the frozen `br-util-nats-fabric` constants and are **not caller-choosable** (no `STREAM_NAME`).
- The declarer **publishes the declare command to** `INTEGRATION_CMD` and **awaits confirmations
  on** `INTEGRATION_EVT`.
- The declarer does **NOT create either stream** — `br-rust-common` never auto-provisions
  (fail-loud doctrine). The awaiter calls `jetstream.get_stream(INTEGRATION_EVT)`; a missing
  stream is a hard error (readiness stays DOWN, `declare_scopes` returns `Err`).
- ⇒ **The acceptor / test harness MUST create both streams up front**:

  ```
  INTEGRATION_CMD   subjects: ["integration.cmd.>"]   (captures the declare command)
  INTEGRATION_EVT   subjects: ["integration.evt.>"]   (captures accepted + rejected)
  ```

  The declare command must be captured because the declarer publishes via JetStream and **awaits
  the publish ack** (`ack.await`) — publishing to a subject no stream captures fails the publish.

### 5.2 Publish path (declarer → declare subject)

`jetstream.publish(subject, bytes)` **then awaits the PubAck**. So the declare command is a
JetStream publish into `INTEGRATION_CMD`, ack-confirmed.

### 5.3 Await path (declarer ← confirmation events)

The awaiter creates a **pull consumer** on `INTEGRATION_EVT` with:

| Consumer setting | Value |
|---|---|
| `durable_name` | `None` → **ephemeral** consumer |
| `deliver_policy` | `DeliverPolicy::New` → only messages arriving **after** the consumer is created |
| `ack_policy` | `AckPolicy::None` → no acks |
| `filter_subjects` | `["integration.evt.identity.service_scope.accepted.v1", "integration.evt.identity.service_scope.rejected.v1"]` (both event subjects) |
| `inactive_threshold` | `AwaiterConfig.inactive_threshold`, default **300s** |

Consumer is consumed via `consumer.messages()` (push-style pull stream, parks at zero CPU —
never `fetch()` in a loop).

**Timing consequence of `DeliverPolicy::New`:** the consumer is created *before* the first
publish (in `declare_scopes`, the awaiter is created, then the publish loop starts). The acceptor
replies only *after* it sees a declare command, so its reply lands after the consumer exists ⇒
delivered. A fake acceptor that pre-publishes a confirmation before the declarer's consumer
exists would be missed.

---

## 6. Matching rule

- A confirmation matches the in-flight declaration **iff
  `event.metadata.correlation_id == command.metadata.correlation_id`**.
- The awaiter deserializes only `{ metadata: { correlation_id } }` from each message
  (`CorrelationProbe`); a message whose JSON does not have `metadata.correlation_id` is silently
  skipped. Non-matching correlation_ids are skipped (kept awaiting).
- **First match wins.** Subject decides outcome: arrival on the **accepted** subject ⇒ `Accepted`;
  on the **rejected** subject ⇒ `Rejected` (after decoding the reason; undecodable ⇒ ignored,
  keep awaiting).
- `causation_id` semantics: the declarer does not use `causation_id` for matching and does not
  set it on the command (omitted). In the BR envelope convention `causation_id` would point to the
  id of the message that caused this one; for a boot-time self-initiated declare there is no cause,
  hence absent. An acceptor MAY set `causation_id` to the command's `command_id` for traceability,
  but it is **not** required and **not** matched on. Matching is correlation_id only.

---

## 7. Declarer behavior contract (from `declare_scopes` / `ScopeDeclarationConfig`)

| Condition | Behavior |
|---|---|
| **enabled = true (boot)** | Build `DeclareServiceScopes`, wrap in `IntegrationCommand` with a **fresh `correlation_id`** (UUIDv7) generated **once**. Create the ephemeral awaiter (§5.3). Then loop. Readiness starts DOWN. |
| **publish** | Publish the command to the declare subject (JetStream, ack-confirmed). On publish error: log warn, do not abort — fall through to the await, retry next loop. |
| **await window** | `await_correlation(correlation_id, wait_timeout)`. `wait_timeout` = env `WAIT_TIMEOUT` (lib default 10s; tests use ~500ms). |
| **timeout (no confirmation in `wait_timeout`)** | Log, loop: **re-publish the SAME `correlation_id`** (do NOT mint a new one), await again. Repeats indefinitely while Identity is down. Readiness stays DOWN. |
| **Accepted received** | `readiness.set_ready()`; return `Accepted`; stop the loop. ⇒ `/readyz` 200. |
| **Rejected received (decodable)** | `readiness.set_not_ready("scope declaration rejected: <reason>")`; return `Rejected(reason)`; stop the loop, **no retry** (rejection is deterministic). ⇒ `/readyz` stays 503. |
| **Rejected received (undecodable payload)** | Log error, ignore, keep awaiting (readiness stays DOWN). |
| **missing stream** | `get_stream(INTEGRATION_EVT)` fails ⇒ `declare_scopes` returns `Err`, readiness never set ready (stays DOWN / fail-loud). |
| **enabled = false (disabled mode)** | `readiness.set_ready()` immediately, return `Disabled`, publish **nothing**. ⇒ `/readyz` 200 at once. |

Readiness liveness split (the G3 subject exposes both as HTTP):
- `/readyz`: 503 until `Accepted` (or immediately 200 if disabled); 200 after `Accepted`;
  503 forever on `Rejected`.
- `/livez`: always 200 (process is alive regardless of declaration state).

---

## 8. What the fake acceptor MUST do (checklist for the Rust runner)

1. **Create the two fixed JetStream streams first**: `INTEGRATION_CMD` (subjects
   `["integration.cmd.>"]`) and `INTEGRATION_EVT` (subjects `["integration.evt.>"]`). The subject
   will NOT create them and will fail loud.
2. Consume the declare subject `integration.cmd.identity.service_scope.declare.v1` from a
   consumer on `INTEGRATION_CMD` (a durable pull consumer filtered on it; explicit ack).
3. Parse the incoming command JSON; extract `metadata.correlation_id` (string UUID). Optionally
   validate/echo `payload.declaration` to decide accept vs reject.
4. **Reply on the matching subject**, echoing that **exact `correlation_id`** in
   `metadata.correlation_id` of an `IntegrationEvent`:
   - accept ⇒ publish to `integration.evt.identity.service_scope.accepted.v1`, payload `{"service":"<key>"}`.
   - reject ⇒ publish to `integration.evt.identity.service_scope.rejected.v1`, payload
     `{"service":"<key>","reason":{...}}` per §4.3.
5. Publish the reply via JetStream (so it lands in `INTEGRATION_EVT`, the stream the declarer's consumer reads).
6. For the timeout/re-publish test: ignore the first N declare messages, then reply — the
   declarer will re-publish with the same correlation_id until you answer.
7. Minimal viable `metadata` on the reply: `{"actor_id":"<any uuid>","correlation_id":"<echoed>"}`.
   `actor_kind` optional (defaults human). Do not emit an unknown `actor_kind`.

---

## 9. Trust model / out of scope

The acceptor trusts the manifest's self-asserted identity. The prefix rule
(`{service}:{capability}`, a scope key may only be declared by the service it names)
is a **coherence check, not authentication** — nothing binds the command's actor to the
declared manifest. The app pipeline discards the command metadata and judges only the
payload; the declaring actor is a self-derived `Uuid::new_v5(NAMESPACE, service_key)`,
which is forgeable.

Impersonation is **out of the threat model**: the trust boundary is the deployment
scope — only first-party services run in it (Infra `ARCHITECTURE.md`, "the trust
boundary is the scope itself") — and the NATS bus runs **without per-service auth
today**. The conformance batteries (G2/G3) therefore do **not** test impersonation: it
is an infrastructure property, not a domain one. A consumer must not read the prefix
rule as an authentication control.

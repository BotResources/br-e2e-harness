# Px/Cx — Identity Directory (Published Language) Wire Contract (v1, FROZEN)

Authoritative on-wire spec for the BotResources **identity Published Language** —
the read-only NATS KV projection identity publishes ("directory / display /
enumeration": who is user X, is X in group Y, the name of group Z) and that
generic services consume. This is **NOT authZ** — authZ stays the `scopes` claim,
resolved fresh per request inside identity.

The schemas below are the **real Rust types** in `br-core-directory`
(`PublishedUser`, `PublishedGroup`, `DirectoryMeta`, the KV-key helpers); the
golden vectors are the **real values** the Go anchor's
`conformance-subjects/identity-directory/wire_test.go` pins. Nothing here is a
hand-written shape that can drift from the types: the conformance oracle is the
lib itself (§6).

The contract is frozen for `br-rust-common` **v0.11.0** (the last evolution
before the lib feature-freeze).

---

## 0. Roles

- **Publisher** (identity): write-only into the KV bucket. On boot, reconciles
  the whole bucket against its Postgres source of truth (put new/changed, DELETE
  orphans = the PII-deletion guarantee), then incremental on event.
- **Consumer** (every other generic service): read-only. Projects KV diffs into
  local PG (`known_users` / `known_groups` / `known_user_group`), self-configures
  from `_meta` (no `_meta` entity ⇒ that reader degrades to empty).
- **Anchor** (`identity-directory`): an **independent Go re-implementation** of
  this wire. It freezes the bytes so the lib cannot drift *with* the Rust; it
  emits the canonical snapshot, the Rust conformance runner deserialises each
  value **through `br-core-directory`** (the oracle, §6).

KV is write-only by identity, read-only by everyone else; the trust boundary is
the deployment scope (network-isolation model), so there is no extra per-service
trust boundary on the bucket.

---

## 1. KV keys (literal, FROZEN)

From `br-core-directory/src/keys.rs`. The bucket name itself is a deployment
concern, not part of this contract; the **key conventions inside it** are frozen:

| Entity | KV key | Source |
|---|---|---|
| **manifest** | `identity/_meta` | `META_KEY` |
| **user** | `identity/users/{uuid}` | `user_kv_key(user_id)` = `USERS_KEY_PREFIX + uuid` |
| **group** | `identity/groups/{uuid}` | `group_kv_key(group_id)` = `GROUPS_KEY_PREFIX + uuid` |

`{uuid}` is the canonical hyphenated lowercase form. `user_id_from_kv_key` /
`group_id_from_kv_key` strip the prefix and parse the suffix as a `Uuid`; a wrong
prefix or a non-UUID suffix yields `None` (so `identity/_meta` is neither a user
nor a group key).

---

## 2. `DirectoryMeta` — the `identity/_meta` manifest

`br-core-directory::DirectoryMeta`. A plain struct, fields in order:

| Field | JSON type | Notes |
|---|---|---|
| `version` | number (u8) | `DIRECTORY_META_VERSION` = `1` |
| `entities` | array of string | declared published entities; `"users"` and/or `"groups"` |

`PublishedEntity` serialises as a **plain string** (custom `Serialize`):
`Users → "users"`, `Groups → "groups"`, and an **unknown future value is
captured, not dropped** (`Other(raw)` round-trips its raw string) — so a consumer
on an older lib tolerates a newer manifest.

**Auto-degrade:** a consumer reads `_meta` to self-configure. `entities` without
`"groups"` ⇒ group readers return empty; without `"users"` ⇒ user readers return
empty. Not a Helm flag — inferred from the published manifest.

Golden:

```json
{ "version": 1, "entities": ["users", "groups"] }
```

Users-only (groups auto-degraded):

```json
{ "version": 1, "entities": ["users"] }
```

---

## 3. `PublishedUser` — the `identity/users/{uuid}` value

`br-core-directory::PublishedUser`. **Core** fields are snake_case, **no
`rename_all`, no `skip`**; project extensions ride **flat** via
`#[serde(flatten)]`:

| Field | JSON type | Notes |
|---|---|---|
| `email` | string | required core |
| `first_name` | string \| **null** | `Option<String>`, **no `skip_serializing_if`** ⇒ emitted as `null` when absent, never omitted |
| `last_name` | string \| **null** | same |
| *(extensions)* | any | `#[serde(flatten)]` — every non-core key sits **flat at the top level** and lands in the lib's `extensions: BTreeMap<String, Value>`. **Never** nested under an `extensions` key. |

On deserialize, an **absent** `first_name`/`last_name` defaults to `None` (the
lib accepts a value that omitted them); on serialize the lib always emits the key
as `null`. The frozen anchor emits them.

Golden (core + the neutral extension `x_custom`):

```json
{
  "email": "ada@example.com",
  "first_name": "Ada",
  "last_name": "Lovelace",
  "x_custom": { "nested": "value" }
}
```

Core-only (names emitted as `null`):

```json
{
  "email": "grace@example.com",
  "first_name": null,
  "last_name": null
}
```

---

## 4. `PublishedGroup` — the `identity/groups/{uuid}` value

`br-core-directory::PublishedGroup`. Same flatten convention as the user:

| Field | JSON type | Notes |
|---|---|---|
| `name` | string | required core |
| `member_ids` | array of string (UUID) | `Vec<Uuid>`; **always an array** (empty `[]` for a memberless group, never absent). Membership (`has_member`) is **derivable** from this — no separate membership entity. |
| *(extensions)* | any | `#[serde(flatten)]`, flat at top level, as for the user |

Golden (core + the neutral extension):

```json
{
  "name": "engineering",
  "member_ids": [
    "01938c1f-0000-7000-8000-000000000001",
    "01938c1f-0000-7000-8000-000000000002"
  ],
  "x_custom": false
}
```

Core-only (empty membership):

```json
{ "name": "guilds", "member_ids": [] }
```

---

## 5. The extension mechanism (generic, tenancy-agnostic)

The PL is **core + extension**, like the Passport `claims` bag. Generic services
bind the **core only**; a project may publish extra fields, which ride **flat**
alongside the core (the `#[serde(flatten)]`), and only the relevant consumers
read them via `PublishedUser::extension(key)` / `PublishedGroup::extension(key)`.

The anchor freezes the **mechanism**, not any specific project field: it emits a
**neutral** extension key, `x_custom`, proving an arbitrary extra round-trips
flat. It deliberately does **not** emit `organization_id` / orgs / memberships /
any tenancy field:

> **`organization_id` is an extension, NOT core** (epic #54). Mono- vs
> multi-tenant is the dimension closest to domain → a seam (Hanshow has no orgs).
> Conformance covers the **core + the generic extension mechanism only**, never a
> project-specific extension — consistent with the tenancy-agnostic socle.

A consumer on the core sees `x_custom` (and any project field) in its
`extensions` map and ignores it unless it opts in. The core deserialises
identically whether or not extensions are present.

---

## 6. Oracle — how the lib is guarded

The conformance runner (`crates/conformance-directory`, the **W1–W5** wire gate)
validates **only by deserialising the Go-frozen wire into the real
`br-core-directory` types**:

```
from_value::<PublishedUser>(go_user_value)
from_value::<PublishedGroup>(go_group_value)
from_value::<DirectoryMeta>(go_meta_value)
```

A successful deserialise *is* the wire-shape check — there is no hand-written
shape guard to drift from the types. The Go anchor is an **independent
re-implementation** (its own structs, its own KV-key derivation), which breaks
the tautology and is the **backward-compat gate**:

> freeze the wire in Go → deserialise it through the lib.

If a core field is renamed/retyped, the serde changes, or the flatten breaks, the
Go-frozen value **fails to deserialise through the lib** → the battery goes
**red**. A real wire break is red; green means the directory envelope didn't move.
The anchor never imports `br-core-directory` — its independence is what makes the
detector trustworthy (CLAUDE.md "Wire-contract conformance": freeze the wire in
Go, import the lib in the test, never freeze the lib's view of the wire).

---

## 7. The anchor's emission

`conformance-subjects/identity-directory` prints one canonical JSON document to
stdout: the full KV snapshot, each entry a `{key, value}` pair where `value` is
the frozen wire for that key. The `{key, value}` envelope is **harness transport**
(it pairs a KV key with its stored value) and is never deserialised through the
lib — only each entry's `value` is. The snapshot:

- `_meta` = `{"version":1,"entities":["users","groups"]}`
- a user with `x_custom` + a core-only user with `null` names
- a group with `x_custom` + a core-only group with `member_ids: []`

The Go golden test (`wire_test.go`) pins the KV-key prefixes and every value's
shape (core keys exact, names-null-not-omitted, extension-rides-flat,
member_ids-always-array, the meta golden + the users-only auto-degrade) — the
offline mirror of the existing scope/passport anchors.

---

## 8. The live batteries — Px (publisher) and Cx (consumer)

The byte-freeze (§6–§7) guards the *types*. The **behaviour** of the
`br-util-directory` kit is guarded by `crates/conformance-directory` against real
infra:

- **Px** (`publisher_floor` / `publisher_groups_optional`) — drives
  `DirectoryPublisher` against a real NATS KV bucket. **P1 (mandatory floor):**
  `reconcile` writes `_meta` + every user, the published wire round-trips
  identically to the source through the lib types, a second reconcile is the empty
  diff (idempotent), and dropping a user orphan-deletes its KV key (PII
  propagation). **P2 (optional):** a users-only source publishes no group keys and
  a `_meta` that omits groups (gated on `_meta`).
- **Cx** (`consumer_reads_users` / `consumer_reads_groups`) — drives
  `DirectoryProjector` (KV→PG) and the `DirectorySnapshot` readers against real
  NATS KV + real Postgres. **All opt-in, none mandatory.** **C1:** reconcile-on-boot
  projects users into PG, `resolve_user` returns the carried fields, retracting a
  user orphan-deletes its row. **C2:** `is_member` / `group_name` resolve with
  groups in `_meta`, and auto-degrade to empty under a users-only `_meta`.

---

## 9. Deliberately OUT of the frozen contract

Not part of this wire; do not infer them:

- **The bucket name / KV bucket config** — a deployment concern; only the
  in-bucket key conventions are frozen.
- **`organization_id` / orgs / memberships / any tenancy field** — a project
  extension, not the core (§5).
- **The reconcile / orphan-delete / incremental-publish behaviour** — that is the
  publisher-kit (Px) and consumer-kit (Cx) conformance, exercised against real
  infra, not this byte-freeze.
- **The Passport / `scopes` claim** — a different layer (the Passport envelope);
  the PL is directory/display, never authZ.
- **Profile fields beyond `email`/`first_name`/`last_name`** — anything richer is
  an extension, never promoted to core unless project-invariant (tenancy is not).

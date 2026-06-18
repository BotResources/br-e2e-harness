# G1 — Bearer→Passport Wire Contract (v1, FROZEN)

The frozen projection that the `conformance-passport` battery enforces: a bearer/PAT
credential, looked up in the `bearer_tokens` NATS KV bucket, resolved into a
`br_core_auth::Passport` returned in the `X-Passport` header of `GET /internal/passport`.

**This doc does NOT own the contract.** The contract is OWNED by the consumer — the
GraphQL gateway — in `BotResources/br-graphql-gateway` README §"svc-identity convention"
and its `crates/e2e/tests/auth_propagation.rs`. That README is the source of truth for the
HTTP envelope (status codes, header injection, the nginx `auth_request` hop). This page
pins only the **two faces the G1 battery asserts** — the KV storage shape and the resolved
`Passport` shape — and defers to the gateway README for everything else, so there is **one**
source of truth, not a second drifting one.

The golden vectors below are the **real values** pinned in the Go anchor's
`conformance-subjects/identity-passport/wire_test.go`; the schemas are the **real Rust
types** in `br-core-auth` (tag `v1.0.0`). Nothing here is hand-written wire shape.

---

## 1. The two faces, pinned

### 1.1 `bearer_tokens` KV storage

| Face | Value |
|---|---|
| **KV key** | `br_core_auth::bearer_token_key(raw_token)` = **lowercase-hex SHA-256 of the FULL raw token**, 64 hex chars. The `brk_…` prefix is **included** in the hash, never stripped — the whole `Authorization: Bearer ` payload is SHA-256'd as-is. |
| **KV value** | `br_core_auth::BearerTokenEntry { email: String, token_id: Uuid }` as JSON. `#[serde(deny_unknown_fields)]` — an extra key fails the parse. No plaintext token is ever stored; the key *is* the hash (hash-once). |

Golden value example:

```json
{ "email": "alice@example.com", "token_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b" }
```

### 1.2 `GET /internal/passport`

| Condition | Response |
|---|---|
| `Authorization: Bearer <raw>` whose `bearer_token_key(<raw>)` exists in `bearer_tokens` | **200** + response header `X-Passport` = `base64.Std(JSON(Passport))` (RFC 4648 standard, padded) |
| key absent / revoked, no `Authorization`, non-Bearer credential, or empty token | **200 with NO `X-Passport`** (anonymous) |
| backend (KV) error | **500** |

The resolved `Passport` is the real `br_core_auth::Passport`
(`#[serde(tag = "kind", deny_unknown_fields)]`). For a PAT it is the `Human` variant with
`auth_method = { "method": "pat", "token_id": <uuid> }`, `impersonator: null`, and `claims`
a JSON object carrying at least `email`.

---

## 2. Golden: `Passport::Human`

Exactly as the anchor emits it (the value base64-decoded out of `X-Passport`). Top-level
keys are exactly these seven — `deny_unknown_fields` rejects any other.

```json
{
  "kind": "human",
  "user_id": "ec40195b-2bcc-58bb-b5d3-4db2e505cee5",
  "is_super_admin": false,
  "is_active": true,
  "auth_method": {"method": "pat", "token_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"},
  "impersonator": null,
  "claims": {"email": "alice@example.com"}
}
```

---

## 3. Golden vectors (pinned in `wire_test.go`)

| Vector | Input | Output |
|---|---|---|
| `bearer_token_key` | token `"brk_test_token_0001"` | `08b6b8ef9b27ca8d4561a519a9ab32cadb11ab16b66e2a47280dc55dccef8fd9` (64 hex) |
| `user_id` synthesis | email `"alice@example.com"` | `ec40195b-2bcc-58bb-b5d3-4db2e505cee5` (UUIDv5, version 5) |
| golden Passport | entry `{email: "alice@example.com", token_id: "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"}` | the JSON in §2 |

The anchor's `user_id` UUIDv5 namespace is `a7d4e2f0-3b91-4c6a-8f12-5e0c9d7b1a23` (the
subject's own, distinct from `br-util-scope-declaration`'s).

---

## 4. `user_id` is a subject-side identifier

`BearerTokenEntry` carries only `email` + `token_id` — no `user_id`. A real `svc-identity`
loads `user_id` from its store; the anchor has no database, so it synthesises a deterministic
stand-in: `UUIDv5(namespace, email)`. The battery therefore asserts `user_id` is **present,
a valid non-nil `Uuid`, and deterministic** (the same bearer resolves twice identically),
**never a specific value**. The golden `ec40195b-…` is documentary, not asserted.

---

## 5. Oracle

The battery validates **only by deserialising into the real `br-core-auth` types** — a
successful `Passport::from_header` *is* the shape check; there is no hand-written shape guard
to drift from the types. Seeding writes the real `bearer_token_key(raw)` and the real
`BearerTokenEntry` JSON.

The Go anchor is an **independent re-implementation** (its own Go SHA-256 in `wire.go`, its
own structs) — this breaks the tautology and is the **backward-compat gate**:

> seed via the lib → independent anchor resolves → decode via the lib.

If the anchor's key derivation or entry parse diverges from the lib, the seeded key is never
found → the bearer resolves to **anonymous** → the battery goes **red**. A real wire break is
red; green means the external envelope didn't move.

---

## 6. Trust note

- **Resolve ≠ gate.** An unresolvable credential yields **200 anonymous**, **never 401** —
  authZ-not-authN, matching the umbrella doctrine (services do authZ, never authN). A 401 here
  would conflate resolution with authorization and break the gateway's anonymous-passthrough.
- **No plaintext token is ever stored.** The KV key is the SHA-256 hash; the value is
  `{email, token_id}`. Hash-once: the raw token is never persisted.
- **The subject never auto-provisions the bucket.** It binds `bearer_tokens` and fails loud
  (`/readyz` stays 503) if absent. The runner/harness creates the bucket before the subject
  boots.

---

## 7. Deliberately OUT of the frozen contract

These are consumer seams or future work, **not** part of this contract — do not infer them:

- **GraphQL** — svc-identity's own app API; this contract is the resolution endpoint only.
- **svc-auth's `/_internal/auth_credentials` hop** — a separate upstream concern.
- **The `{"control": …}` response body** the real svc-identity adds — the gateway reads
  **only the status + the `X-Passport` header**, so the body is out of scope.
- **`claims` content** (org_id, is_admin, scopes) — a per-project seam; the battery checks
  only that `claims` is an object carrying `email`.
- **JIT-provisioning, cookie names** — consumer concerns.
- **`attach` mode** (drive a live service's NATS + `/readyz`) and a **500 / backend-error
  scenario** — future additions; G1 ships spawn-only over a seeded bucket.

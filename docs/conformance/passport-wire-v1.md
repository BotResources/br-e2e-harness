# G1 — Bearer→Passport Wire Contract (sealed model, FROZEN)

The frozen projection that the `conformance-passport` battery enforces: a bearer/PAT
credential, looked up as a **sealed AEAD envelope** in the `PUBLISHED_LANGUAGE` NATS
KV bucket, opened, and resolved into a `br_core_auth::Passport` returned in the
`X-Passport` header of `GET /internal/passport`.

> **The retired plaintext model is gone.** Earlier this contract stored
> `br_core_auth::BearerTokenEntry { email, token_id }` as cleartext JSON in a
> `bearer_tokens` bucket and synthesised `user_id` as a UUIDv5 of the email. That
> model is **retired**; this page describes the only live contract, the sealed one.

**This doc does NOT own the contract.** The contract is OWNED by the consumer — the
GraphQL gateway — in `BotResources/br-graphql-gateway`, and the sealed wire is
OWNED by `BotResources/svc-auth` (`br-auth-contract`). That README + the
`br-auth-contract` crate are the source of truth for the envelope. This page pins
only the **two faces the G1 battery asserts** — the KV storage shape and the
resolved `Passport` shape — and defers elsewhere, so there is **one** source of
truth, not a second drifting one.

The schemas below are the **real Rust types** (`br-auth-contract` / `br-core-auth`,
tag `v1.0.2`); the Go vectors are pinned in the anchor's `wire_test.go`. Nothing
here is hand-written wire shape.

---

## 1. The two faces, pinned

### 1.1 `PUBLISHED_LANGUAGE` KV storage

| Face | Value |
|---|---|
| **KV key** | `br_auth_contract::bearer_token_kv_key(raw_token)` = `"identity/bearer_tokens/"` + `br_core_auth::bearer_token_key(raw_token)` (lowercase-hex SHA-256 of the **full** raw token, prefix included). |
| **KV value** | `br_auth_contract::SealedBearer { nonce: String, ciphertext: String }` as JSON. Both fields are **base64-std (padded)**. `#[serde(deny_unknown_fields)]`, no version field. |
| **Cipher** | **ChaCha20-Poly1305** (RFC 8439): 12-byte nonce (random per seal, carried in the envelope), 16-byte tag. |
| **AAD** | `br_core_auth::bearer_token_key(raw_token)` — the **unprefixed** 64-char SHA-256 digest, as bytes. **NOT** the full KV key. |
| **Cleartext** | `serde_json(br_auth_contract::BearerEntry { actor: br_core_kernel::Actor, token_id: Uuid })`, `deny_unknown_fields`. `Actor` wire = `{"kind":"human","id":"<uuid>"}` / `{"kind":"service","id":"<uuid>"}`. **No email.** |
| **Seal key** | `BEARER_SEAL_KEY` env, base64-std → exactly 32 bytes (`BearerSealKey`, `BEARER_SEAL_KEY_LEN = 32`). Zeroize-on-drop. |

No plaintext token is ever stored; the KV key is the hash, the value is the sealed
envelope.

### 1.2 `GET /internal/passport`

| Condition | Response |
|---|---|
| `Authorization: Bearer <raw>` whose `bearer_token_kv_key(<raw>)` exists in `PUBLISHED_LANGUAGE` **and opens** under the seal key | **200** + header `X-Passport` = `base64.Std(JSON(Passport))` |
| key absent / revoked, no `Authorization`, non-Bearer, empty token, unreadable envelope, **wrong key**, or **tampered ciphertext** | **200 with NO `X-Passport`** (anonymous, fail-closed) |
| backend (KV) error — bucket/stream lost under a live subject | **fails loud**: a **5xx**, or the resolver becomes **unreachable** (drops its NATS connection / exits) — never a silent 200 |

The resolved `Passport` is the real `br_core_auth::Passport`
(`#[serde(tag = "kind", deny_unknown_fields)]`). For a sealed PAT it is the `Human`
variant with `auth_method = { "method": "pat", "token_id": <uuid> }`,
`impersonator: null`, and `user_id` = the `UserId` inside the sealed
`Actor::Human(..)`.

**The `claims` contract is "no `email` claim", not "empty `claims`".** The
sealed-model guarantee the battery enforces is the migration's PII-removal property:
the retired model put `email` in `claims`, the sealed model must **not** — there is
no email in the sealed `BearerEntry`. The battery therefore asserts **email-absent**
(`claims.get("email").is_none()`), deliberately, **not** fully-empty claims. A real
`svc-identity` MAY attach other claims (e.g. a `scopes` claim, cf.
`SCOPES_CLAIM_KEY` / G4); asserting `{}` would falsely reject it. `claims = {}` in
the golden below is the **Go anchor's emission** (it has no scopes source), not a
requirement on every conforming subject.

---

## 2. Golden: `Passport::Human` (the Go anchor's emission)

Exactly as the anchor emits it (the value base64-decoded out of `X-Passport`). The
seven top-level keys are exactly these — `deny_unknown_fields` rejects any other.
`claims = {}` here is the anchor's emission (no scopes source), not a contract
requirement on every subject — see §1.2.

```json
{
  "kind": "human",
  "user_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
  "is_super_admin": false,
  "is_active": true,
  "auth_method": {"method": "pat", "token_id": "0190c0de-c3d4-7e5f-8a9b-0c1d2e3f4a5b"},
  "impersonator": null,
  "claims": {}
}
```

`user_id` is the sealed actor's id, carried through verbatim — there is no email
and no derivation.

---

## 3. Golden vectors (pinned in `wire_test.go`)

| Vector | Input | Output |
|---|---|---|
| `kvKey` | token `"brk_test_token_0001"` | `identity/bearer_tokens/08b6b8ef9b27ca8d4561a519a9ab32cadb11ab16b66e2a47280dc55dccef8fd9` |
| `aad` | token `"brk_test_token_0001"` | `08b6b8ef9b27ca8d4561a519a9ab32cadb11ab16b66e2a47280dc55dccef8fd9` (the unprefixed digest) |
| frozen seal | key `[0x2a; 32]`, nonce `AAECAwQFBgcICQoL`, the golden entry | a frozen ciphertext (pins the AEAD wiring; the runtime nonce is random) |
| golden Passport | entry `{actor:{human, 0190a1b2-…}, token_id: 0190c0de-…}` | the JSON in §2 |

The frozen-ciphertext vector is **Go-internal** (Go seals with a fixed nonce and
opens its own) — it pins Go's AEAD wiring. The **cross-language** guarantee is
proven live: the battery seeds via the real Rust seal and the Go subject opens it.

---

## 4. Oracle & interop gate

The battery validates **only by the real types**: seeding via
`br_auth_identity_util::BearerPublisher` (real seal), decoding via
`br_core_auth::Passport::from_header` (a successful decode *is* the shape check).

The Go anchor is an **independent re-implementation that OPENS** the Rust-sealed
envelope (its own SHA-256, AAD, structs, and `chacha20poly1305`), never importing
the Rust lib — this breaks the tautology and is the **interop / backward-compat
gate**:

> seed via the real Rust seal → independent Go anchor opens it → decode via the lib.

If the anchor's key derivation, AAD, cipher, or entry parse diverges, the open
fails → the bearer resolves to **anonymous** → the battery goes **red**. The random
per-seal nonce means the ciphertext bytes are not frozen; the crypto **contract**
is.

---

## 5. Trust note

- **Resolve ≠ gate.** Unresolvable / non-openable → **200 anonymous**, **never
  401** (authZ-not-authN). A genuine KV backend loss is the one case that must
  **fail loud** — a **5xx** or an unreachable resolver — and must **never** become
  a silent 200.
- **No plaintext token, no email, ever stored.** The KV key is the SHA-256 hash;
  the value is a sealed envelope whose cleartext is `{actor, token_id}`.
- **The subject never auto-provisions the bucket.** It binds `PUBLISHED_LANGUAGE`
  and fails loud (`/readyz` stays 503) if absent. The runner/harness creates the
  bucket before the subject boots.

---

## 6. Deliberately OUT of the frozen contract

Consumer seams or future work — do not infer them:

- **GraphQL** — svc-identity's own app API; this contract is the resolution
  endpoint only.
- **svc-auth's seal-key provisioning / rotation** — a separate upstream concern;
  this contract takes the key as `BEARER_SEAL_KEY`.
- **The `{"control": …}` response body** the real svc-identity adds — the gateway
  reads only the status + the `X-Passport` header.
- **`claims` content** beyond "empty in the sealed PAT path" — per-project seam.
- **JIT-provisioning, cookie names** — consumer concerns.
- **`attach` mode** (drive a live service's NATS + `/readyz`) — a future addition;
  G1 ships spawn-only.

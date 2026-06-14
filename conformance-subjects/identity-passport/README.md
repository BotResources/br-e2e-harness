# identity-passport — G1 conformance subject

A minimal **Go** service that plays the BotResources **passport-resolution
endpoint** the GraphQL gateway calls before every authenticated request, exactly
as a real `svc-identity` does: it reads a bearer credential, resolves it against
the `bearer_tokens` NATS KV bucket, and returns the resolved **Passport** in the
`X-Passport` response header.

It is the **test SUBJECT** for conformance group **G1**. A separate Rust runner
drives it as a **black box**: it brings up a JetStream-enabled NATS with a
pre-created `bearer_tokens` bucket, seeds entries, calls
`GET /internal/passport` with various `Authorization` headers, and asserts the
returned `X-Passport` decodes under the real `br_core_auth::Passport`.

> This is a **test fixture**. It implements no authentication on its HTTP surface
> and is meant only for an isolated, throwaway test network.

## What it does

On boot:

1. Connects to NATS, binds the JetStream KV bucket named by `BEARER_BUCKET`
   (default `bearer_tokens`), then sets `/readyz` **200**. **The subject does NOT
   create the bucket** (the platform never auto-provisions). If the bucket is
   missing, the bind fails and `/readyz` stays **503**.
2. Serves `GET` on `/internal/passport` (the frozen contract is `GET`; the handler is a
   plain `net/http` mux entry, so other methods reach it too, but only `GET` is contractual):
   - Reads `Authorization`. Only `Authorization: Bearer <token>` is resolved.
   - Computes the KV key = **lowercase-hex SHA-256 of the raw `<token>`** (the
     full token after `Bearer `, including any `brk_` prefix — the whole string
     is hashed), matching the lib's `bearer_token_key`.
   - Looks the key up in the `bearer_tokens` bucket.
   - **Found** → the value is the lib's `BearerTokenEntry`
     (`{"email","token_id"}`). Builds a `Passport::Human`, base64-encodes its
     JSON, and returns **200** with header `X-Passport: <base64>`.
   - **Not found** (revoked/absent), **no `Authorization`**, **not a Bearer**, or
     **empty token** → **200 with no `X-Passport`** (anonymous).
   - KV backend failure → **500**.

`/livez` is always **200**.

## The frozen Passport wire (the `Human` variant)

The emitted `X-Passport` is standard base64 (RFC 4648, padded) of this exact JSON
— and only these top-level keys, because the Rust `Passport` is
`#[serde(deny_unknown_fields)]`:

```json
{
  "kind": "human",
  "user_id": "<uuidv5(email)>",
  "is_super_admin": false,
  "is_active": true,
  "auth_method": {"method": "pat", "token_id": "<entry.token_id>"},
  "impersonator": null,
  "claims": {"email": "<entry.email>"}
}
```

## Configuration (env only)

| Variable | Required | Default | Meaning |
|---|---|---|---|
| `NATS_URL` | no | `nats://127.0.0.1:4222` | JetStream-enabled NATS URL |
| `HTTP_ADDR` | no | `:8080` | bind addr for `/internal/passport` + `/readyz` + `/livez` |
| `BEARER_BUCKET` | no | `bearer_tokens` | JetStream KV bucket to resolve bearer tokens against (must already exist) |

## Build & run

```sh
# build
go build -o identity-passport .      # or: make build

# run against a local JetStream NATS with a pre-created bearer_tokens bucket
NATS_URL=nats://127.0.0.1:4222 \
HTTP_ADDR=127.0.0.1:8080 \
BEARER_BUCKET=bearer_tokens \
./identity-passport
```

```sh
make test       # go vet + go test (incl. the golden-shape + hashing + base64 tests)
```

The offline tests (key derivation, the Passport golden shape, the base64
round-trip, the bearer-header parsing) run with plain `go test` — no infra. The
full G1 e2e (found / revoked / absent-header / non-bearer / KV-failure /
readiness-gating) is the Rust conformance runner's job; it brings the real
`bearer_tokens` bucket and the real `Passport` deserialiser as the oracle.

## Why table

| Thing | Why it is the way it is |
|---|---|
| `user_id` is a UUIDv5 of the email under a fixed namespace | The `BearerTokenEntry` carries only `email` + `token_id`, and this anchor has **no database** — the real service loads `user_id` from Postgres. A deterministic v5-from-email is a stable stand-in; the battery asserts `user_id` is a present, valid UUID, not a specific value. The namespace is this subject's own, distinct from `br-util-scope-declaration`'s. |
| Revoked / absent token → **200 anonymous**, never **401** | The endpoint **resolves**, it does not **gate**: an unresolvable credential yields an anonymous request that downstream authZ then refuses. Returning 401 here would conflate resolution with authorization and break the gateway's anonymous-passthrough path. Matches the platform: services do authZ, never authN. |
| `claims` is minimal `{"email":…}`, no project keys | `claims` is a per-project **seam** (org_id, is_admin, scopes are consumer deltas, not the platform contract). The anchor freezes only the generic envelope, so it emits the single generic claim the entry actually carries. The Rust deserialiser requires `claims` to be a JSON object, so it cannot be null/array. |
| `impersonator: null` emitted (not omitted) | The contract value for a non-impersonated session; the Rust field is `Option<Uuid>` with `#[serde(default)]`, so `null` deserialises to `None` and is accepted by `deny_unknown_fields` as a known key. |
| `is_super_admin: false`, `is_active: true` fixed | Fixed anchor defaults — the conformance target is the **wire envelope**, not policy state the anchor has no source for. |
| Subject does not create the bucket | Mirrors the platform's fail-loud, never-auto-provision doctrine; it binds the bucket, it does not create it. The runner/harness owns bucket setup. |
| Full raw token (incl. `brk_` prefix) is hashed | Byte-matches the lib's `bearer_token_key`, which SHA-256s the plaintext as-is with no prefix stripping. |

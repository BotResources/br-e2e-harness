# identity-passport — G1 conformance subject (sealed model)

A minimal **Go** service that plays the BotResources **passport-resolution
endpoint** the GraphQL gateway calls before every authenticated request, exactly
as a real `svc-identity` does: it reads a bearer credential, looks up the
**sealed** entry in the shared `PUBLISHED_LANGUAGE` NATS KV bucket, **opens** it
with its own ChaCha20-Poly1305 AEAD, and returns the resolved **Passport** in the
`X-Passport` response header.

It is the **test SUBJECT** for conformance group **G1**. A separate Rust runner
drives it as a **black box**: it brings up JetStream NATS with a pre-created
`PUBLISHED_LANGUAGE` bucket, seeds **sealed** entries with the real Rust lib
(`br-auth-identity-util` → `br-auth-contract`), calls `GET /internal/passport`
with various `Authorization` headers, and asserts the returned `X-Passport`
decodes under the real `br_core_auth::Passport`.

> This is a **test fixture**. It implements no authentication on its HTTP surface
> and is meant only for an isolated, throwaway test network.

## The cross-language anchor

This subject is the **frozen, independent Go re-implementation** of the sealed
wire — its own SHA-256, its own AAD derivation, its own struct parse, and its own
`golang.org/x/crypto/chacha20poly1305` **open** path. It **never imports the Rust
lib**. The runner seeds with the *real* Rust seal (`BearerPublisher`); this subject
independently **decrypts** it. A green run is genuine Rust-seal / Go-open
cross-language interop: it pins the whole crypto contract — key handling, the
AAD = unprefixed digest, the cipher / nonce / tag, the envelope JSON, the
`BearerEntry` shape, and the `Passport` shape. The random per-seal nonce means the
**ciphertext bytes are not frozen** — the crypto **contract** is.

## What it does

On boot:

1. Reads `BEARER_SEAL_KEY` (base64-std of a 32-byte key), builds the
   ChaCha20-Poly1305 AEAD, connects to NATS, and **binds** the JetStream KV bucket
   `PUBLISHED_LANGUAGE`, then sets `/readyz` **200**. **The subject does NOT create
   the bucket** (the platform never auto-provisions). If the bucket is missing, the
   bind fails and `/readyz` stays **503**.
2. Serves `GET /internal/passport`:
   - Reads `Authorization`. Only `Authorization: Bearer <token>` is resolved.
   - Computes the KV key = `"identity/bearer_tokens/"` + **lowercase-hex SHA-256 of
     the raw `<token>`** (the whole string after `Bearer `, prefix included).
   - Looks the key up in `PUBLISHED_LANGUAGE`.
   - **Found** → the value is a sealed envelope (`{"nonce","ciphertext"}`, both
     base64-std). It opens the AEAD with **AAD = the unprefixed SHA-256 digest** of
     the token. On success the plaintext is the cleartext entry
     (`{"actor":{"kind","id"},"token_id"}`); it builds a `Passport::Human`,
     base64-encodes its JSON, and returns **200** with header `X-Passport`.
   - **Not found** (revoked/absent), **no `Authorization`**, **not a Bearer**,
     **empty token**, **unreadable envelope**, **wrong key**, or **tampered
     ciphertext** → **200 with no `X-Passport`** (anonymous, fail-closed).
   - **KV backend failure** (e.g. the bucket vanished) → **500**.

`/livez` is always **200**. The endpoint **resolves**, it does not **gate**: an
unresolvable credential is anonymous, never 401 — services do authZ, never authN.

## The frozen Passport wire (the `Human` variant)

The emitted `X-Passport` is standard base64 (RFC 4648, padded) of this exact JSON
— only these seven top-level keys, because the Rust `Passport` is
`#[serde(deny_unknown_fields)]`:

```json
{
  "kind": "human",
  "user_id": "<actor.id>",
  "is_super_admin": false,
  "is_active": true,
  "auth_method": {"method": "pat", "token_id": "<entry.token_id>"},
  "impersonator": null,
  "claims": {}
}
```

`user_id` is the actor's id taken **directly** from the sealed entry
(`actor: {"kind":"human","id":"<uuid>"}`) — there is **no email** in the sealed
model and **no UUIDv5-from-email derivation**. `claims` is the empty object.

## Configuration (env only)

| Variable | Required | Default | Meaning |
|---|---|---|---|
| `NATS_URL` | no | `nats://127.0.0.1:4222` | JetStream-enabled NATS URL |
| `PORT` | **yes** | — | port to bind (`0.0.0.0:$PORT`) for `/internal/passport` + `/readyz` + `/livez` |
| `BEARER_SEAL_KEY` | **yes** | — | base64-std of the 32-byte ChaCha20-Poly1305 seal key; must decode to exactly 32 bytes (fail loud otherwise) |

The fixed KV bucket is `PUBLISHED_LANGUAGE` (not an env var — the platform's fixed
Published-Language bucket).

## Build & run

```sh
go build -o identity-passport .      # or: make build
make check                           # fmt + vet + test + guard
```

The offline tests (`wire_test.go`) pin the KV-key/AAD vectors, a fixed-nonce
Go-internal seal+open round-trip (a frozen ciphertext that pins the AEAD wiring),
and the `Passport` golden shape. The full G1 e2e (found / revoked / unknown /
no-credential / wrong-key / tampered / KV-failure / readiness) is the Rust
conformance runner's job; it brings the real `PUBLISHED_LANGUAGE` bucket, the real
Rust seal, and the real `Passport` deserialiser as the oracle.

`make guard` fails loud if any retired-model marker (an `email` JSON tag, a
`userIDFromEmail` / `uuid.NewSHA1` derivation, or the old plaintext entry type)
reappears in non-test source.

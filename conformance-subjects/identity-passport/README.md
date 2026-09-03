# identity-passport — G1 conformance subject (sealed model)

A minimal **Go** service that plays the BotResources **passport-resolution
endpoint** the GraphQL gateway calls before every authenticated request, exactly
as a real `svc-identity` does: it reads a bearer credential, looks up the
**sealed** entry in the shared `PUBLISHED_LANGUAGE` NATS KV bucket, **opens** it
with its own ChaCha20-Poly1305 AEAD, and returns the resolved **Passport** in the
`X-Passport` response header.

It is the **test SUBJECT** for conformance group **G1**, and it is also the
**producer** of the seeds the runner writes: the `seal` subcommand (below) renders
the KV key and the exact stored bytes. A separate Rust runner drives it as a
**black box**: it brings up JetStream NATS with a pre-created `PUBLISHED_LANGUAGE`
bucket, asks this binary for the sealed bytes, writes them raw, calls
`GET /internal/passport` with various `Authorization` headers, and asserts the
returned `X-Passport` decodes under the real `br_core_auth::Passport`.

> This is a **test fixture**. It implements no authentication on its HTTP surface
> and is meant only for an isolated, throwaway test network.

## The cross-language anchor

This subject is the **frozen, independent Go re-implementation** of the sealed
wire — its own SHA-256, its own AAD derivation, its own struct parse, and both the
`golang.org/x/crypto/chacha20poly1305` **seal** and **open** paths. It **never
imports the Rust lib**, and no Rust crate ships the wire it freezes.

**Seal and open are frozen together, in one package.** The wire is pinned by
`wire_test.go` + `seal_test.go`, not by a Rust contract crate: a fixed-nonce vector
reproduces a **frozen ciphertext**, and a second vector freezes the exact
**cleartext bytes** (`{"actor":{"kind":"…","id":"…"},"token_id":"…"}`) and the exact
**envelope bytes** (`{"nonce":"…","ciphertext":"…"}`). Drift on either side turns
those tests red instead of moving Rust and the wire together silently.

The runner keeps one lib-oracle cross-check on the Rust side: the KV key this
binary emits must end in `br_core_auth::bearer_token_key(<token>)`, so the digest
derivation stays anchored to the real lib.

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

## The `seal` subcommand (seed production)

`identity-passport seal …` renders one seed and exits; with no subcommand the
binary serves (so the resolver path is unchanged). It prints **exactly one JSON
line** on stdout and exits non-zero with a message on stderr for any bad input.

```sh
identity-passport seal \
  --key <base64-std of 32 bytes> \
  --token <raw bearer token> \
  --actor human:<uuid> | service:<uuid> \
  --token-id <uuid> \
  [--tamper ciphertext|nonce] [--unreadable]
```

```json
{"kv_key":"identity/bearer_tokens/<sha256hex>","value_b64":"<base64-std of the exact bytes to store>"}
```

| Flag | Effect |
|---|---|
| *(none)* | a faithful envelope: fresh 12-byte nonce, AAD = the unprefixed digest, cleartext = the bearer entry |
| `--tamper ciphertext` | seals faithfully, then flips the first ciphertext byte — parses, **never opens** (AEAD tag) |
| `--tamper nonce` | seals faithfully, then flips the first nonce byte — parses, **never opens** |
| `--unreadable` | a faithful envelope **plus an unknown field** — would open, but the parser must reject it first |

`--key` is the key the seed is sealed **with**; the wrong-key case is simply a
different `--key` than the resolver's `BEARER_SEAL_KEY`. The KV key never depends
on the seal key. `--tamper` and `--unreadable` are mutually exclusive.

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

The offline tests pin the KV-key/AAD vectors, the frozen cleartext and envelope
bytes, a fixed-nonce seal that reproduces a frozen ciphertext, the `seal`
CLI contract (round-trip, service actor, fresh nonce per seal, wrong key,
both tamper modes, unreadable, and every rejected input), and the `Passport`
golden shape. The full G1 e2e (found / revoked / unknown / no-credential /
wrong-key / tampered / unreadable / KV-failure / readiness) is the Rust
conformance runner's job; it brings the real `PUBLISHED_LANGUAGE` bucket and the
real `Passport` deserialiser as the oracle.

`make guard` fails loud if any retired-model marker (an `email` JSON tag, a
`userIDFromEmail` / `uuid.NewSHA1` derivation, or the old plaintext entry type)
reappears in non-test source.

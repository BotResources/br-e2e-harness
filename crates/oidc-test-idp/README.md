# oidc-test-idp

A **pilotable OIDC identity provider for end-to-end tests**, published as
`ghcr.io/botresources/br-oidc-test-idp`.

> ⚠️ **TEST FIXTURE ONLY.** It signs any requested token, and its `/admin/*`
> surface has no authentication — by design. Never deploy it outside an
> isolated, throwaway test network.

It serves a real discovery document and a real JWKS, and signs real RS256
id_tokens — so the system under test exercises its **full** verification path
(discovery, JWKS fetch and caching, signature, issuer and audience
validation). Nothing in the system under test is bypassed; what makes this a
fixture is the control plane, which lets tests do what no real IdP allows:

- **Instant key rotation** — the whole RSA pool is pre-generated at startup;
  `/admin/rotate` only flips which keys the JWKS publishes. No waiting, ever.
- **Sign with an unpublished key** — mint a token whose `kid` will never
  appear in the JWKS, to prove unknown keys are rejected.
- **Fetch counters** — assert that a JWKS refresh actually happened (or that
  a cooldown suppressed one) by reading `/admin/state`, not by sleeping.

## Configuration (env)

| Var | Default | Meaning |
|---|---|---|
| `ISSUER` | — (required) | The URL by which the **system under test** reaches the fixture (e.g. `http://oidc-idp:9100`). Used as the `iss` claim, the discovery `issuer`, and the base of `jwks_uri`. |
| `PORT` | `9100` | Listen port (binds `0.0.0.0`). |
| `KEY_POOL_SIZE` | `6` | RSA keys generated at startup (`e2e-key-0` … `e2e-key-N-1`). |
| `INITIAL_PUBLISHED` | `2` | How many pool keys the JWKS publishes at startup. |
| `DEFAULT_CLIENT_ID` | `e2e-client` | Default `aud` for minted tokens. |

For a multi-provider test (routing by issuer), run two instances with two
different `ISSUER`s.

## Public surface (what the system under test sees)

- `GET /.well-known/openid-configuration` — minimal, honest document:
  `issuer`, `jwks_uri`, RS256. It does not advertise endpoints the fixture
  does not implement.
- `GET /jwks` — exactly the published subset of the pool.
- `GET /health` — liveness.

## Admin surface (what the test drives)

### `POST /admin/mint` → `{ "id_token": "...", "kid": "..." }`

```jsonc
{
  "email": "alice@example.com",      // required; becomes `sub` + the email claim
  "aud": "my-client",                // default: DEFAULT_CLIENT_ID
  "email_claim": "preferred_username", // default: "email" (Entra-shaped tokens)
  "kid": "e2e-key-5",                // default: active key; unpublished kids allowed
  "expires_in_secs": -60,            // default: 600; negative = already expired
  "claims": { "iss": "evil" },       // extra claims, merged LAST (can override iss/aud)
  "omit_kid_header": true            // default: false; exercises single-key fallbacks
}
```

### `POST /admin/rotate` → state snapshot

- Empty body `{}` — the common "the IdP rotated" gesture: publish the next
  unpublished pool key and make it the active signing key. `409` when the
  pool is exhausted.
- Explicit form:

```jsonc
{
  "publish":   ["e2e-key-3"],
  "unpublish": ["e2e-key-0"],   // unpublishing everything is allowed (empty JWKS)
  "active":    "e2e-key-3"      // may be an unpublished key
}
```

### `POST /admin/reset` → state snapshot

Back to the startup state (initial published set, `e2e-key-0` active),
counters zeroed. Use it between tests when a suite shares one instance.

### `GET /admin/state`

```jsonc
{
  "issuer": "http://oidc-idp:9100",
  "default_client_id": "e2e-client",
  "pool_kids": ["e2e-key-0", "..."],
  "published_kids": ["e2e-key-0", "e2e-key-1"],
  "active_kid": "e2e-key-0",
  "jwks_fetches": 3,            // GET /jwks count since start/reset
  "discovery_fetches": 1        // discovery document count since start/reset
}
```

## Test recipes

**Refresh-on-kid-miss** (the consumer cached the JWKS, then the IdP rotated):

1. Let the consumer start (it caches the initial JWKS).
2. `POST /admin/rotate` `{}` → new key published + active.
3. Mint and present a token → the consumer misses the new `kid`, re-fetches
   the JWKS, and accepts. Assert `jwks_fetches` grew by exactly 1.

**Unknown key is rejected**:

1. Mint with an unpublished `kid` (e.g. `"kid": "e2e-key-5"`).
2. Present it → the consumer refreshes (counter +1), still cannot find the
   key, and must reject.

**Refresh cooldown** (no hammering of the JWKS endpoint):

1. Trigger one kid-miss (counter +1).
2. Immediately present another unknown-`kid` token → rejection **without** a
   second fetch: assert the counter did not move.

## Running locally

```bash
ISSUER=http://localhost:9100 cargo run -p oidc-test-idp
curl -s localhost:9100/.well-known/openid-configuration | jq
curl -s -X POST localhost:9100/admin/mint \
  -H 'content-type: application/json' \
  -d '{"email":"alice@example.com"}' | jq -r .id_token
```

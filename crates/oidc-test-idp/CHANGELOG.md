# Changelog

All notable changes to `oidc-test-idp` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.

## [0.1.0] — 2026-06-10

### Added

- Initial release: a pilotable OIDC test IdP.
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

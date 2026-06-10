//! A pilotable OIDC identity provider for end-to-end tests.
//!
//! Serves a real discovery document and a real JWKS, and signs real RS256
//! id_tokens — so the system under test exercises its *full* verification
//! path (discovery, JWKS fetch and caching, signature, issuer and audience
//! validation). The admin surface lets tests do what no real IdP allows:
//! rotate keys instantly (the whole RSA pool is pre-generated at startup),
//! sign with a key that is not in the JWKS, and observe JWKS fetches through
//! counters instead of sleeps.
//!
//! ⚠️ **TEST FIXTURE ONLY** — it signs any requested token and exposes its
//! admin surface without authentication, by design. Never deploy it outside
//! an isolated, throwaway test network.

pub mod keys;
pub mod routes;
pub mod state;

pub use routes::router;
pub use state::{IdpConfig, IdpState};

//! Fixture state: the key pool, which keys the JWKS publishes, the active
//! signing key, and the fetch counters tests assert against.

use std::collections::BTreeSet;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::keys::{KeyEntry, generate_pool};

/// Startup configuration. All keys are generated here, once — rotation never
/// generates anything.
pub struct IdpConfig {
    /// The URL by which the *system under test* reaches this fixture
    /// (e.g. `http://oidc-idp:9100`). Used as the `iss` claim, the discovery
    /// `issuer`, and the base of `jwks_uri` — they must all match.
    pub issuer: String,
    pub key_pool_size: usize,
    /// How many pool keys the JWKS publishes at startup (`e2e-key-0` …).
    pub initial_published: usize,
    /// Default `aud` for minted tokens when the request does not set one.
    pub default_client_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("unknown kid '{kid}' — pool kids are {pool}")]
    UnknownKid { kid: String, pool: String },
    #[error("key pool exhausted: all {0} keys are already published — raise KEY_POOL_SIZE")]
    PoolExhausted(usize),
    #[error("token signing failed: {0}")]
    Signing(String),
}

#[derive(Deserialize)]
pub struct MintRequest {
    pub email: String,
    /// Defaults to the fixture's `default_client_id`.
    pub aud: Option<String>,
    /// Claim name carrying the email (e.g. `preferred_username` for
    /// Entra-shaped tokens). Defaults to `email`.
    pub email_claim: Option<String>,
    /// Signing key. Defaults to the active key. May name an *unpublished*
    /// pool key — that is how you mint a token the JWKS will never vouch for.
    pub kid: Option<String>,
    /// Defaults to 600. Negative values mint an already-expired token.
    pub expires_in_secs: Option<i64>,
    /// Extra claims, merged last — they override the generated ones, so a
    /// test can also forge a wrong `iss` or `aud` on a correctly-signed token.
    pub claims: Option<serde_json::Map<String, Value>>,
    /// Omit the `kid` JWT header to exercise single-key-JWKS fallbacks.
    #[serde(default)]
    pub omit_kid_header: bool,
}

#[derive(Serialize)]
pub struct MintResponse {
    pub id_token: String,
    pub kid: String,
}

#[derive(Deserialize, Default)]
pub struct RotateRequest {
    /// Kids to add to the JWKS.
    pub publish: Option<Vec<String>>,
    /// Kids to remove from the JWKS (removing all of them is allowed —
    /// an empty JWKS is a useful test state).
    pub unpublish: Option<Vec<String>>,
    /// New active signing key (may be unpublished).
    pub active: Option<String>,
}

pub struct IdpState {
    issuer: String,
    default_client_id: String,
    pool: Vec<KeyEntry>,
    initial_published: usize,
    published: RwLock<BTreeSet<usize>>,
    active: RwLock<usize>,
    jwks_fetches: AtomicU64,
    discovery_fetches: AtomicU64,
}

impl IdpState {
    /// Generates the whole key pool. Panics on inconsistent configuration —
    /// this runs once at startup and must fail loud, not limp along.
    pub fn new(config: IdpConfig) -> Self {
        assert!(
            config.key_pool_size >= 2,
            "KEY_POOL_SIZE must be >= 2 (rotation needs somewhere to go)"
        );
        assert!(
            (1..=config.key_pool_size).contains(&config.initial_published),
            "INITIAL_PUBLISHED must be between 1 and KEY_POOL_SIZE"
        );
        let issuer = config.issuer.trim_end_matches('/').to_string();
        Self {
            issuer,
            default_client_id: config.default_client_id,
            pool: generate_pool(config.key_pool_size),
            initial_published: config.initial_published,
            published: RwLock::new((0..config.initial_published).collect()),
            active: RwLock::new(0),
            jwks_fetches: AtomicU64::new(0),
            discovery_fetches: AtomicU64::new(0),
        }
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// The discovery document. Minimal on purpose: only what id_token
    /// verification flows need. We do not advertise endpoints (authorization,
    /// token) that this fixture does not implement.
    pub fn discovery_document(&self) -> Value {
        self.discovery_fetches.fetch_add(1, Ordering::Relaxed);
        json!({
            "issuer": self.issuer,
            "jwks_uri": format!("{}/jwks", self.issuer),
            "response_types_supported": ["id_token"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
        })
    }

    /// The JWKS: exactly the published subset of the pool.
    pub fn jwks_document(&self) -> Value {
        self.jwks_fetches.fetch_add(1, Ordering::Relaxed);
        let published = self.published.read().unwrap();
        let keys: Vec<&Value> = published.iter().map(|&i| &self.pool[i].jwk).collect();
        json!({ "keys": keys })
    }

    pub fn mint(&self, req: &MintRequest) -> Result<MintResponse, AdminError> {
        let key = match &req.kid {
            Some(kid) => self.key_index(kid).map(|i| &self.pool[i])?,
            None => &self.pool[*self.active.read().unwrap()],
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before 1970")
            .as_secs() as i64;
        let email_claim = req.email_claim.as_deref().unwrap_or("email");
        let aud = req.aud.as_deref().unwrap_or(&self.default_client_id);

        let mut claims = serde_json::Map::new();
        claims.insert("iss".into(), json!(self.issuer));
        claims.insert("sub".into(), json!(req.email));
        claims.insert("aud".into(), json!(aud));
        claims.insert("iat".into(), json!(now));
        claims.insert(
            "exp".into(),
            json!(now.saturating_add(req.expires_in_secs.unwrap_or(600))),
        );
        claims.insert(email_claim.to_string(), json!(req.email));
        if let Some(extra) = &req.claims {
            for (name, value) in extra {
                claims.insert(name.clone(), value.clone());
            }
        }

        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        if !req.omit_kid_header {
            header.kid = Some(key.kid.clone());
        }
        let id_token = jsonwebtoken::encode(&header, &claims, &key.encoding_key)
            .map_err(|e| AdminError::Signing(e.to_string()))?;

        Ok(MintResponse {
            id_token,
            kid: key.kid.clone(),
        })
    }

    /// With an empty body: publish the next unpublished pool key and make it
    /// the active signing key (the common "the IdP rotated" gesture).
    /// With explicit fields: apply exactly what was asked.
    pub fn rotate(&self, req: &RotateRequest) -> Result<Value, AdminError> {
        let explicit = req.publish.is_some() || req.unpublish.is_some() || req.active.is_some();
        if explicit {
            if let Some(kids) = &req.publish {
                let indices = self.key_indices(kids)?;
                let mut published = self.published.write().unwrap();
                published.extend(indices);
            }
            if let Some(kids) = &req.unpublish {
                let indices = self.key_indices(kids)?;
                let mut published = self.published.write().unwrap();
                for index in indices {
                    published.remove(&index);
                }
            }
            if let Some(kid) = &req.active {
                *self.active.write().unwrap() = self.key_index(kid)?;
            }
        } else {
            let next = {
                let published = self.published.read().unwrap();
                (0..self.pool.len())
                    .find(|i| !published.contains(i))
                    .ok_or(AdminError::PoolExhausted(self.pool.len()))?
            };
            self.published.write().unwrap().insert(next);
            *self.active.write().unwrap() = next;
        }
        Ok(self.snapshot())
    }

    /// Back to the startup state, counters zeroed — test isolation when a
    /// single fixture instance is shared by a sequential suite.
    pub fn reset(&self) -> Value {
        *self.published.write().unwrap() = (0..self.initial_published).collect();
        *self.active.write().unwrap() = 0;
        self.jwks_fetches.store(0, Ordering::Relaxed);
        self.discovery_fetches.store(0, Ordering::Relaxed);
        self.snapshot()
    }

    pub fn snapshot(&self) -> Value {
        let published = self.published.read().unwrap();
        json!({
            "issuer": self.issuer,
            "default_client_id": self.default_client_id,
            "pool_kids": self.pool.iter().map(|k| k.kid.as_str()).collect::<Vec<_>>(),
            "published_kids": published.iter().map(|&i| self.pool[i].kid.as_str()).collect::<Vec<_>>(),
            "active_kid": self.pool[*self.active.read().unwrap()].kid,
            "jwks_fetches": self.jwks_fetches.load(Ordering::Relaxed),
            "discovery_fetches": self.discovery_fetches.load(Ordering::Relaxed),
        })
    }

    fn key_index(&self, kid: &str) -> Result<usize, AdminError> {
        self.pool
            .iter()
            .position(|k| k.kid == kid)
            .ok_or_else(|| AdminError::UnknownKid {
                kid: kid.to_string(),
                pool: self
                    .pool
                    .iter()
                    .map(|k| k.kid.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            })
    }

    fn key_indices(&self, kids: &[String]) -> Result<Vec<usize>, AdminError> {
        kids.iter().map(|kid| self.key_index(kid)).collect()
    }
}

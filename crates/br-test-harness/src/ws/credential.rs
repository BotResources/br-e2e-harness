use br_core_auth::{Passport, PassportHeader};
use tokio_tungstenite::tungstenite::http::HeaderMap;

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum WsCredential<'a> {
    Passport(&'a Passport),
    Cookie(&'a str),
    Anonymous,
}

impl WsCredential<'_> {
    pub(super) fn apply(&self, headers: &mut HeaderMap) -> Result<(), String> {
        match self {
            Self::Passport(passport) => {
                headers.insert(
                    "X-Passport",
                    passport
                        .to_header()
                        .parse()
                        .map_err(|e| format!("ws: X-Passport header: {e}"))?,
                );
            }
            Self::Cookie(cookie) => {
                headers.insert(
                    "Cookie",
                    cookie
                        .parse()
                        .map_err(|e| format!("ws: Cookie header: {e}"))?,
                );
            }
            Self::Anonymous => {}
        }
        Ok(())
    }
}

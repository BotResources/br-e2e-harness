use std::sync::Arc;
use std::time::Instant;

use oidc_test_idp::{IdpConfig, IdpState, router};

fn required_env(name: &str, hint: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        eprintln!("{name} is required — {hint}");
        std::process::exit(1);
    })
}

/// Fail-closed env parsing: absent → default, garbage → exit. A fixture that
/// silently fell back on a default would lie to the test that configured it.
fn usize_env(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Err(_) => default,
        Ok(raw) => raw.parse().unwrap_or_else(|_| {
            eprintln!("{name} must be an integer, got '{raw}'");
            std::process::exit(1);
        }),
    }
}

#[tokio::main]
async fn main() {
    // CD smoke-tests the published image with `--version`.
    if std::env::args().any(|arg| arg == "--version") {
        println!("oidc-test-idp {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = IdpConfig {
        issuer: required_env(
            "ISSUER",
            "the URL by which the system under test reaches this fixture, e.g. http://oidc-idp:9100",
        ),
        key_pool_size: usize_env("KEY_POOL_SIZE", 6),
        initial_published: usize_env("INITIAL_PUBLISHED", 2),
        default_client_id: std::env::var("DEFAULT_CLIENT_ID")
            .unwrap_or_else(|_| "e2e-client".to_string()),
    };
    let port = usize_env("PORT", 9100);

    let started = Instant::now();
    let state = Arc::new(IdpState::new(config));
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        issuer = state.issuer(),
        "RSA key pool generated — rotations are now instantaneous"
    );
    tracing::warn!(
        "TEST FIXTURE ONLY — this IdP signs any requested token and its admin \
         surface has no authentication; never expose it outside an isolated test network"
    );

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap_or_else(|e| {
            eprintln!("failed to bind 0.0.0.0:{port}: {e}");
            std::process::exit(1);
        });
    tracing::info!(port, "oidc-test-idp listening");

    axum::serve(listener, router(state))
        .await
        .expect("server error");
}

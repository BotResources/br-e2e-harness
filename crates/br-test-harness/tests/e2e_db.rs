use br_test_harness::E2eDatabase;
use sqlx::{Connection, PgConnection, Row as _};

#[tokio::test]
#[ignore = "real-infra: needs an admin Postgres via E2E_PG_ADMIN_URL / DATABASE_URL"]
async fn provisions_an_owner_that_authenticates_with_its_generated_credentials() {
    let db = E2eDatabase::create(true, &[]).await;

    let mut owner = PgConnection::connect(&db.owner_url())
        .await
        .expect("the provisioned owner must authenticate with its generated credentials");

    let current: String = sqlx::query("SELECT current_user")
        .fetch_one(&mut owner)
        .await
        .expect("query as owner must succeed")
        .get(0);
    assert_eq!(current, db.owner_role());

    owner.close().await.ok();
    db.cleanup().await;
}

#[tokio::test]
#[ignore = "real-infra: needs an admin Postgres via E2E_PG_ADMIN_URL / DATABASE_URL"]
async fn rls_context_is_transaction_local_and_never_leaks_across_transactions() {
    let db = E2eDatabase::create(true, &[]).await;

    let mut conn = PgConnection::connect(&db.owner_url())
        .await
        .expect("owner connection for the RLS-locality proof");

    let principal = "user-42";

    let mut tx_a = conn.begin().await.expect("begin first transaction");
    sqlx::query("SELECT set_config('app.current_user_id', $1, true)")
        .bind(principal)
        .execute(&mut *tx_a)
        .await
        .expect("set transaction-local RLS context");
    let inside: String = sqlx::query("SELECT current_setting('app.current_user_id', true)")
        .fetch_one(&mut *tx_a)
        .await
        .expect("read context inside the same transaction")
        .get(0);
    assert_eq!(
        inside, principal,
        "the context must be visible inside the transaction that set it"
    );
    tx_a.commit().await.expect("commit first transaction");

    let mut tx_b = conn.begin().await.expect("begin second transaction");
    let leaked: Option<String> = sqlx::query("SELECT current_setting('app.current_user_id', true)")
        .fetch_one(&mut *tx_b)
        .await
        .expect("read context in a fresh transaction on the reused connection")
        .get(0);
    assert!(
        leaked.is_none() || leaked.as_deref() == Some(""),
        "transaction-local RLS context must NOT leak into another transaction on a reused connection — got {leaked:?}"
    );
    tx_b.rollback().await.expect("rollback second transaction");

    conn.close().await.ok();
    db.cleanup().await;
}

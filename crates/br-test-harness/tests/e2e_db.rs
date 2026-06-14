use br_test_harness::E2eDatabase;
use sqlx::{Connection, Executor as _, PgConnection, Row as _};

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

#[tokio::test]
#[ignore = "real-infra: needs an admin Postgres via E2E_PG_ADMIN_URL / DATABASE_URL"]
async fn provisions_an_app_role_that_authenticates_and_can_use_the_public_schema() {
    let app_name = format!("e2e_app_{}", uuid::Uuid::now_v7().simple());
    let db = E2eDatabase::create(true, &[])
        .await
        .with_app_role(&app_name, "app_test_pw")
        .await;

    assert_eq!(db.app_role(), Some(app_name.as_str()));

    let mut owner = PgConnection::connect(&db.owner_url())
        .await
        .expect("owner connection");
    owner
        .execute("CREATE TABLE widgets (id int primary key, owner_id text not null)")
        .await
        .expect("owner creates a table");
    owner.close().await.ok();

    let mut app = PgConnection::connect(&db.app_url())
        .await
        .expect("the provisioned app role must authenticate with its password");
    let current: String = sqlx::query("SELECT current_user")
        .fetch_one(&mut app)
        .await
        .expect("query as app role must succeed")
        .get(0);
    assert_eq!(current, app_name);

    app.execute("INSERT INTO widgets (id, owner_id) VALUES (1, 'alice')")
        .await
        .expect("owner default privileges must let the app role write the new table");
    let count: i64 = sqlx::query("SELECT count(*) FROM widgets")
        .fetch_one(&mut app)
        .await
        .expect("app role reads its own write")
        .get(0);
    assert_eq!(count, 1);

    app.close().await.ok();
    db.cleanup().await;
}

#[tokio::test]
#[ignore = "real-infra: needs an admin Postgres via E2E_PG_ADMIN_URL / DATABASE_URL"]
async fn app_role_provisioning_is_idempotent_under_a_shared_role_name() {
    let app_name = format!("e2e_app_shared_{}", uuid::Uuid::now_v7().simple());

    let first = E2eDatabase::create(true, &[])
        .await
        .with_app_role(&app_name, "app_test_pw")
        .await;
    let second = E2eDatabase::create(true, &[])
        .await
        .with_app_role(&app_name, "app_test_pw")
        .await;

    for db in [&first, &second] {
        let app = PgConnection::connect(&db.app_url())
            .await
            .expect("the shared app role authenticates against both databases");
        app.close().await.ok();
    }

    first.cleanup().await;
    second.cleanup().await;
}

#[tokio::test]
#[ignore = "real-infra: needs an admin Postgres via E2E_PG_ADMIN_URL / DATABASE_URL"]
async fn create_named_recovers_when_a_prior_crashed_run_leaked_the_owner_role() {
    let suffix = uuid::Uuid::now_v7().simple().to_string();
    let owner_name = format!("e2e_owner_leaked_{suffix}");
    let db_name = format!("e2e_leaked_{suffix}");

    let admin_url = std::env::var("E2E_PG_ADMIN_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("E2E_PG_ADMIN_URL / DATABASE_URL must be set for this real-infra test");
    let mut admin = PgConnection::connect(&admin_url)
        .await
        .expect("connect as admin to simulate a leaked owner role");
    admin
        .execute(
            format!("CREATE ROLE \"{owner_name}\" LOGIN PASSWORD 'stale_pw' NOSUPERUSER").as_str(),
        )
        .await
        .expect("pre-create the owner role to simulate a crashed run that never cleaned up");
    admin.close().await.ok();

    let db = E2eDatabase::create_named(&owner_name, &db_name, true, &[]).await;

    let mut owner = PgConnection::connect(&db.owner_url())
        .await
        .expect("after recovering the leaked owner, it authenticates with the fresh credentials");
    let current: String = sqlx::query("SELECT current_user")
        .fetch_one(&mut owner)
        .await
        .expect("query as the recovered owner must succeed")
        .get(0);
    assert_eq!(current, owner_name);
    owner.close().await.ok();

    db.cleanup().await;
}

#[tokio::test]
#[ignore = "real-infra: needs an admin Postgres via E2E_PG_ADMIN_URL / DATABASE_URL"]
async fn the_drop_net_tears_down_a_db_dropped_without_an_explicit_cleanup() {
    let suffix = uuid::Uuid::now_v7().simple().to_string();
    let owner_name = format!("e2e_owner_dropnet_{suffix}");
    let db_name = format!("e2e_dropnet_{suffix}");

    {
        let _db = E2eDatabase::create_named(&owner_name, &db_name, true, &[]).await;
    }

    let admin_url = std::env::var("E2E_PG_ADMIN_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("E2E_PG_ADMIN_URL / DATABASE_URL must be set for this real-infra test");
    let mut admin = PgConnection::connect(&admin_url)
        .await
        .expect("connect as admin to assert the Drop net tore the resources down");

    let db_left: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM pg_database WHERE datname = $1")
        .bind(&db_name)
        .fetch_optional(&mut admin)
        .await
        .expect("query pg_database");
    assert!(
        db_left.is_none(),
        "the Drop net must have dropped the test database left without an explicit cleanup"
    );

    let owner_left: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM pg_roles WHERE rolname = $1")
        .bind(&owner_name)
        .fetch_optional(&mut admin)
        .await
        .expect("query pg_roles");
    assert!(
        owner_left.is_none(),
        "the Drop net must have dropped the owner role left without an explicit cleanup"
    );

    admin.close().await.ok();
}

#[tokio::test]
#[ignore = "real-infra: needs an admin Postgres via E2E_PG_ADMIN_URL / DATABASE_URL"]
async fn rls_isolates_the_app_role_while_the_bypassrls_owner_sees_raw_rows() {
    let app_name = format!("e2e_app_rls_{}", uuid::Uuid::now_v7().simple());
    let db = E2eDatabase::create(true, &[])
        .await
        .with_app_role(&app_name, "app_test_pw")
        .await;

    assert!(
        !db.admin_url().is_empty(),
        "the admin url must be surfaced for posture / raw-state assertions"
    );

    let mut owner = PgConnection::connect(&db.owner_url())
        .await
        .expect("owner connection");
    for stmt in [
        "CREATE TABLE notes (id int primary key, owner_id text not null, body text not null)",
        "ALTER TABLE notes ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE notes FORCE ROW LEVEL SECURITY",
        "CREATE POLICY notes_owner ON notes USING (owner_id = current_setting('app.current_user_id', true))",
        "INSERT INTO notes (id, owner_id, body) VALUES (1, 'alice', 'a'), (2, 'bob', 'b')",
    ] {
        owner
            .execute(stmt)
            .await
            .expect("owner sets up RLS fixture");
    }

    let mut app = PgConnection::connect(&db.app_url())
        .await
        .expect("app connection (the RLS-subject role)");
    let mut tx = app.begin().await.expect("begin app transaction");
    sqlx::query("SELECT set_config('app.current_user_id', $1, true)")
        .bind("alice")
        .execute(&mut *tx)
        .await
        .expect("set transaction-local RLS principal");
    let visible: Vec<String> = sqlx::query("SELECT body FROM notes ORDER BY id")
        .fetch_all(&mut *tx)
        .await
        .expect("app role reads under RLS")
        .into_iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(
        visible,
        vec!["a".to_string()],
        "the app role must see only the rows its RLS principal owns"
    );
    tx.commit().await.ok();
    app.close().await.ok();

    let raw: i64 = sqlx::query("SELECT count(*) FROM notes")
        .fetch_one(&mut owner)
        .await
        .expect("the bypassrls owner reads raw, unfiltered state")
        .get(0);
    assert_eq!(
        raw, 2,
        "the rls-bypassing owner must see every row regardless of the RLS principal"
    );
    owner.close().await.ok();

    db.cleanup().await;
}

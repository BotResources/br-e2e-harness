use std::collections::BTreeMap;

use br_core_directory::{DirectoryError, PublishedUser};
use br_util_directory::{
    ConsumptionScope, DirectoryConsumerConfig, DirectoryProjector, PersistedExtensions, migrate,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::anchor::DirectorySnapshotWire;
use crate::check::{Check, Run, finish};
use crate::error::{ConformanceError, Result};
use crate::harness::{ConsumerEnv, DirectoryHarness};
use crate::outcome::{CheckId, CheckOutcome};
use crate::pg::ConsumerDb;
use crate::publish_fixture::publish_snapshot;
use crate::source::AnchorSource;

pub const MEMBERSHIP_FLAG: &str = "is_platform_member";

fn extract_locale(user: &PublishedUser) -> PersistedExtensions {
    match user.extension("locale") {
        Some(value) => PersistedExtensions::from_value(json!({ "locale": value.clone() })),
        None => PersistedExtensions::none(),
    }
}

fn config_keeping_locale(scope: ConsumptionScope) -> DirectoryConsumerConfig {
    DirectoryConsumerConfig::default()
        .scope(scope)
        .extract_user_extensions(extract_locale)
}

pub async fn extension_survives_projection(
    snapshot: &DirectorySnapshotWire,
) -> Result<CheckOutcome> {
    let id = CheckId::ConsumerExtensionSurvives;
    let expected = "a published user carrying an extension the consumer extracts is projected into \
                    known_users.extensions and read back intact (lossless sink)";
    let env = ConsumerEnv::start().await?;
    let outcome =
        finish(extension_survives_inner(id, expected, &env.harness, &env.db, snapshot).await);
    env.shutdown().await;
    outcome
}

async fn extension_survives_inner(
    id: CheckId,
    expected: &str,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    snapshot: &DirectorySnapshotWire,
) -> Run {
    let check = Check::new(id, expected);
    let mut source = AnchorSource::from_snapshot(snapshot)?;
    let Some((user_id, base)) = source.first_user() else {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "no user in the source",
            "the snapshot must carry a user",
        ));
    };
    let carried = with_extension(&base, "locale", json!("fr"))?;
    source.upsert_user(user_id, carried);

    publish_snapshot(harness.fabric(), &source).await?;
    migrate(db.pool())
        .await
        .map_err(|e| ConformanceError::Directory(format!("migrate: {e}")))?;

    let projector = DirectoryProjector::with_config(
        harness.fabric().clone(),
        db.pool().clone(),
        config_keeping_locale(ConsumptionScope::UsersAndGroups),
    );
    check.system("reconcile", projector.reconcile().await)?;

    match read_extensions(db, user_id).await? {
        Some(stored) if stored == json!({ "locale": "fr" }) => Ok(CheckOutcome::pass(
            id,
            expected,
            format!("extensions for {user_id} survived intact: {stored}"),
        )),
        Some(stored) => Ok(CheckOutcome::fail(
            id,
            expected,
            format!("stored extensions {stored}"),
            "the extracted extension was not projected losslessly into known_users.extensions",
        )),
        None => Ok(CheckOutcome::fail(
            id,
            expected,
            format!("no known_users row for {user_id}"),
            "the projector wrote no row for the published user, so no extension could survive",
        )),
    }
}

pub async fn filter_flip_orphan_deletes(snapshot: &DirectorySnapshotWire) -> Result<CheckOutcome> {
    let id = CheckId::ConsumerFilterFlipOrphans;
    let expected = "a user passing .filter_users is projected; on republish FAILING the filter the \
                    next reconcile orphan-deletes its row";
    let env = ConsumerEnv::start().await?;
    let outcome = finish(filter_flip_inner(id, expected, &env.harness, &env.db, snapshot).await);
    env.shutdown().await;
    outcome
}

async fn filter_flip_inner(
    id: CheckId,
    expected: &str,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    snapshot: &DirectorySnapshotWire,
) -> Run {
    let check = Check::new(id, expected);
    let mut source = AnchorSource::from_snapshot(snapshot)?;
    let Some((user_id, base)) = source.first_user() else {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "no user in the source",
            "the snapshot must carry a user",
        ));
    };
    let member = with_extension(&base, MEMBERSHIP_FLAG, json!(true))?;
    source.upsert_user(user_id, member);
    publish_snapshot(harness.fabric(), &source).await?;
    migrate(db.pool())
        .await
        .map_err(|e| ConformanceError::Directory(format!("migrate: {e}")))?;

    let config = DirectoryConsumerConfig::default()
        .filter_users(|user| user.extension(MEMBERSHIP_FLAG) == Some(&Value::Bool(true)));
    let projector = DirectoryProjector::with_config(
        harness.fabric().clone(),
        db.pool().clone(),
        config.clone(),
    );
    check.system("reconcile", projector.reconcile().await)?;
    if !user_row_exists(db, user_id).await? {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("user {user_id} absent after passing the filter"),
            "a filter-passing user must be projected on first reconcile",
        ));
    }

    let outsider = with_extension(&base, MEMBERSHIP_FLAG, json!(false))?;
    source.upsert_user(user_id, outsider);
    publish_snapshot(harness.fabric(), &source).await?;
    let projector_after =
        DirectoryProjector::with_config(harness.fabric().clone(), db.pool().clone(), config);
    check.system("reconcile after flip", projector_after.reconcile().await)?;
    if user_row_exists(db, user_id).await? {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("user {user_id} still projected after failing the filter"),
            "republishing a user that fails the copy filter must orphan-delete its known_users row",
        ));
    }

    Ok(CheckOutcome::pass(
        id,
        expected,
        format!("user {user_id} projected then orphan-deleted on the filter flip"),
    ))
}

pub fn reserved_key_rejected() -> CheckOutcome {
    let id = CheckId::WireReservedKeyRejected;
    let expected = "an extensions map shadowing a reserved core key (email) is rejected at \
                    PublishedUser construction with DirectoryError::ReservedExtensionKey, never a \
                    silent overwrite";
    let mut extensions = BTreeMap::new();
    extensions.insert("email".to_string(), json!("shadow@example.com"));
    match PublishedUser::new("real@example.com".to_string(), None, None, extensions) {
        Ok(_) => CheckOutcome::fail(
            id,
            expected,
            "construction succeeded",
            "a reserved-key extension must fail closed, never silently overwrite the core field",
        ),
        Err(DirectoryError::ReservedExtensionKey { entity, key }) if key == "email" => {
            CheckOutcome::pass(
                id,
                expected,
                format!("rejected: {entity} reserved key {key:?}"),
            )
        }
        Err(other) => CheckOutcome::fail(
            id,
            expected,
            format!("error {other}"),
            "expected DirectoryError::ReservedExtensionKey for the shadowed core key",
        ),
    }
}

fn with_extension(base: &PublishedUser, key: &str, value: Value) -> Result<PublishedUser> {
    let mut extensions = base.extensions().clone();
    extensions.insert(key.to_string(), value);
    PublishedUser::new(
        base.email.clone(),
        base.first_name.clone(),
        base.last_name.clone(),
        extensions,
    )
    .map_err(|e| ConformanceError::Directory(format!("build user with extension: {e}")))
}

async fn read_extensions(db: &ConsumerDb, user_id: Uuid) -> Result<Option<Value>> {
    let row: Option<(Value,)> =
        sqlx::query_as("SELECT extensions FROM known_users WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(db.pool())
            .await
            .map_err(|e| ConformanceError::Postgres(format!("read extensions: {e}")))?;
    Ok(row.map(|(extensions,)| extensions))
}

async fn user_row_exists(db: &ConsumerDb, user_id: Uuid) -> Result<bool> {
    let row: (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM known_users WHERE user_id = $1)")
            .bind(user_id)
            .fetch_one(db.pool())
            .await
            .map_err(|e| ConformanceError::Postgres(format!("probe user row: {e}")))?;
    Ok(row.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "real-infra: needs a Postgres"]
    async fn reading_the_extensions_of_an_unprojected_user_is_not_a_fixture_error() {
        let db = ConsumerDb::provision().await.expect("provision");
        br_util_directory::migrate(db.pool())
            .await
            .expect("migrate");
        let read = read_extensions(&db, Uuid::now_v7()).await;
        db.cleanup().await;
        assert_eq!(
            read.expect("a missing row is not a Postgres error"),
            None,
            "a user the projector wrote no row for reads as absent"
        );
    }

    #[test]
    fn reserved_key_rejection_is_fail_closed() {
        let outcome = reserved_key_rejected();
        assert!(
            outcome.is_pass(),
            "shadowing a reserved core key must be rejected: {:?}",
            outcome.detail
        );
    }

    #[test]
    fn extract_locale_selects_only_the_locale_extension() {
        let mut extensions = BTreeMap::new();
        extensions.insert("locale".to_string(), json!("fr"));
        extensions.insert("noise".to_string(), json!(true));
        let user = PublishedUser::new("a@b".to_string(), None, None, extensions).unwrap();
        assert_eq!(
            extract_locale(&user).into_value(),
            json!({ "locale": "fr" })
        );
    }

    #[test]
    fn extract_locale_is_empty_when_absent() {
        let user = PublishedUser::new("a@b".to_string(), None, None, BTreeMap::new()).unwrap();
        assert_eq!(extract_locale(&user), PersistedExtensions::none());
    }

    #[test]
    fn with_extension_preserves_core_and_adds_the_flag() {
        let base = PublishedUser::new(
            "a@b".to_string(),
            Some("Ada".to_string()),
            None,
            BTreeMap::new(),
        )
        .unwrap();
        let flagged = with_extension(&base, MEMBERSHIP_FLAG, json!(true)).unwrap();
        assert_eq!(flagged.email, "a@b");
        assert_eq!(flagged.first_name.as_deref(), Some("Ada"));
        assert_eq!(flagged.extension(MEMBERSHIP_FLAG), Some(&json!(true)));
    }
}

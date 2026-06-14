use br_core_directory::{DirectoryMeta, PublishedEntity, PublishedGroup, PublishedUser};
use serde_json::{Value, json};

use crate::anchor::{DirectorySnapshotWire, KvEntry};
use crate::error::{ConformanceError, Result};
use crate::outcome::{CheckId, CheckOutcome, ConformanceReport};

pub const NEUTRAL_EXTENSION_KEY: &str = "x_custom";

pub fn deserialize_user(entry: &KvEntry) -> Result<PublishedUser> {
    serde_json::from_value(entry.value.clone()).map_err(|e| ConformanceError::NonConformantWire {
        key: entry.key.clone(),
        ty: "PublishedUser",
        cause: e.to_string(),
    })
}

pub fn deserialize_group(entry: &KvEntry) -> Result<PublishedGroup> {
    serde_json::from_value(entry.value.clone()).map_err(|e| ConformanceError::NonConformantWire {
        key: entry.key.clone(),
        ty: "PublishedGroup",
        cause: e.to_string(),
    })
}

pub fn deserialize_meta(entry: &KvEntry) -> Result<DirectoryMeta> {
    serde_json::from_value(entry.value.clone()).map_err(|e| ConformanceError::NonConformantWire {
        key: entry.key.clone(),
        ty: "DirectoryMeta",
        cause: e.to_string(),
    })
}

pub fn run_wire_battery(snapshot: &DirectorySnapshotWire) -> ConformanceReport {
    let mut report = ConformanceReport::default();
    report.push(check_user_deserializes(snapshot));
    report.push(check_group_deserializes(snapshot));
    report.push(check_meta_deserializes(snapshot));
    report.push(check_extension_rides_flat(snapshot));
    report.push(check_meta_auto_degrades());
    report
}

fn check_user_deserializes(snapshot: &DirectorySnapshotWire) -> CheckOutcome {
    let id = CheckId::WireUserDeserializes;
    let expected = "every users/{uuid} value deserializes through br_core_directory::PublishedUser, and a populated user binds first_name + last_name (not silently None)";
    if snapshot.users.is_empty() {
        return CheckOutcome::fail(
            id,
            expected,
            "no users in the snapshot",
            "the anchor must emit at least one user",
        );
    }
    let mut populated_names = false;
    for entry in &snapshot.users {
        let user = match deserialize_user(entry) {
            Ok(user) => user,
            Err(e) => return CheckOutcome::fail(id, expected, "deser failed", e.to_string()),
        };
        if user.email.is_empty() {
            return CheckOutcome::fail(
                id,
                expected,
                "empty email",
                format!("{} carried an empty core email", entry.key),
            );
        }
        if user.first_name.is_some() && user.last_name.is_some() {
            populated_names = true;
        }
    }
    if !populated_names {
        return CheckOutcome::fail(
            id,
            expected,
            "no user binds both first_name and last_name",
            "the anchor must emit a user with non-null first_name + last_name so a rename of an optional core field (first_name -> firstName) lands it in extensions as None and fails this check",
        );
    }
    CheckOutcome::pass(
        id,
        expected,
        format!(
            "{} user value(s) deserialized, optional core names bound on a populated user",
            snapshot.users.len()
        ),
    )
}

fn check_group_deserializes(snapshot: &DirectorySnapshotWire) -> CheckOutcome {
    let id = CheckId::WireGroupDeserializes;
    let expected = "every groups/{uuid} value deserializes through br_core_directory::PublishedGroup with member_ids as an array";
    if snapshot.groups.is_empty() {
        return CheckOutcome::fail(
            id,
            expected,
            "no groups in the snapshot",
            "the anchor must emit at least one group",
        );
    }
    let mut saw_empty_membership = false;
    for entry in &snapshot.groups {
        match deserialize_group(entry) {
            Ok(group) => {
                if group.member_ids.is_empty() {
                    saw_empty_membership = true;
                }
            }
            Err(e) => return CheckOutcome::fail(id, expected, "deser failed", e.to_string()),
        }
    }
    if !saw_empty_membership {
        return CheckOutcome::fail(
            id,
            expected,
            "no core-only group with member_ids: []",
            "the anchor must emit a memberless group to prove member_ids is an array, never absent",
        );
    }
    CheckOutcome::pass(
        id,
        expected,
        format!("{} group value(s) deserialized", snapshot.groups.len()),
    )
}

fn check_meta_deserializes(snapshot: &DirectorySnapshotWire) -> CheckOutcome {
    let id = CheckId::WireMetaDeserializes;
    let expected =
        "identity/_meta value deserializes through DirectoryMeta declaring users + groups";
    let meta = match deserialize_meta(&snapshot.meta) {
        Ok(meta) => meta,
        Err(e) => return CheckOutcome::fail(id, expected, "deser failed", e.to_string()),
    };
    if !meta.publishes_users() {
        return CheckOutcome::fail(
            id,
            expected,
            "manifest omits users",
            "the canonical anchor declares users",
        );
    }
    if !meta.publishes_groups() {
        return CheckOutcome::fail(
            id,
            expected,
            "manifest omits groups",
            "the canonical anchor declares groups",
        );
    }
    CheckOutcome::pass(
        id,
        expected,
        format!(
            "version={}, entities={:?}",
            meta.version,
            render_entities(&meta.entities)
        ),
    )
}

fn check_extension_rides_flat(snapshot: &DirectorySnapshotWire) -> CheckOutcome {
    let id = CheckId::WireExtensionRidesFlat;
    let expected = format!(
        "the neutral extension {NEUTRAL_EXTENSION_KEY:?} lands in extensions (flat), never in a core field"
    );
    let extended = snapshot
        .users
        .iter()
        .find(|entry| entry.value.get(NEUTRAL_EXTENSION_KEY).is_some());
    let Some(entry) = extended else {
        return CheckOutcome::fail(
            id,
            expected,
            format!("no user carries {NEUTRAL_EXTENSION_KEY}"),
            "the anchor must emit a user with the neutral extension to prove the flatten",
        );
    };
    let user = match deserialize_user(entry) {
        Ok(user) => user,
        Err(e) => return CheckOutcome::fail(id, expected, "deser failed", e.to_string()),
    };
    if user.extension(NEUTRAL_EXTENSION_KEY).is_none() {
        return CheckOutcome::fail(
            id,
            expected,
            "extension absent from the flatten map",
            format!("{NEUTRAL_EXTENSION_KEY} did not land in extensions — the flatten broke"),
        );
    }
    let core_keys = ["email", "first_name", "last_name", "extensions"];
    for key in core_keys {
        if user.extension(key).is_some() {
            return CheckOutcome::fail(
                id,
                expected,
                format!("core key {key:?} leaked into extensions"),
                "a core field must never appear in the extensions map",
            );
        }
    }
    CheckOutcome::pass(
        id,
        &expected,
        format!("{NEUTRAL_EXTENSION_KEY} rides flat into extensions, core stays core"),
    )
}

fn check_meta_auto_degrades() -> CheckOutcome {
    let id = CheckId::WireMetaAutoDegrades;
    let expected =
        "a users-only manifest deserializes and auto-degrades groups (publishes_groups == false)";
    let wire: Value = json!({ "version": 1, "entities": ["users"] });
    let meta: DirectoryMeta = match serde_json::from_value(wire) {
        Ok(meta) => meta,
        Err(e) => {
            return CheckOutcome::fail(
                id,
                expected,
                "users-only meta failed to deserialize",
                e.to_string(),
            );
        }
    };
    if !meta.publishes_users() {
        return CheckOutcome::fail(
            id,
            expected,
            "users not published",
            "a users-only manifest must publish users",
        );
    }
    if meta.publishes_groups() {
        return CheckOutcome::fail(
            id,
            expected,
            "groups still published",
            "a manifest without 'groups' must auto-degrade group readers",
        );
    }
    CheckOutcome::pass(id, expected, "users-only manifest degrades groups")
}

fn render_entities(entities: &[PublishedEntity]) -> Vec<&str> {
    entities.iter().map(PublishedEntity::as_wire).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::KvEntry;

    fn entry(key: &str, value: Value) -> KvEntry {
        KvEntry {
            key: key.to_string(),
            value,
        }
    }

    fn canonical_snapshot() -> DirectorySnapshotWire {
        DirectorySnapshotWire {
            meta: entry(
                "identity/_meta",
                json!({ "version": 1, "entities": ["users", "groups"] }),
            ),
            users: vec![
                entry(
                    "identity/users/01938c1f-0000-7000-8000-000000000001",
                    json!({
                        "email": "ada@example.com",
                        "first_name": "Ada",
                        "last_name": "Lovelace",
                        "x_custom": { "nested": "value" }
                    }),
                ),
                entry(
                    "identity/users/01938c1f-0000-7000-8000-000000000002",
                    json!({ "email": "grace@example.com", "first_name": null, "last_name": null }),
                ),
            ],
            groups: vec![
                entry(
                    "identity/groups/01938c1f-0000-7000-8000-0000000000a1",
                    json!({
                        "name": "engineering",
                        "member_ids": [
                            "01938c1f-0000-7000-8000-000000000001",
                            "01938c1f-0000-7000-8000-000000000002"
                        ],
                        "x_custom": false
                    }),
                ),
                entry(
                    "identity/groups/01938c1f-0000-7000-8000-0000000000a2",
                    json!({ "name": "guilds", "member_ids": [] }),
                ),
            ],
        }
    }

    #[test]
    fn the_canonical_anchor_shape_is_conformant() {
        let report = run_wire_battery(&canonical_snapshot());
        assert!(report.is_conformant(), "{:#?}", report.outcomes);
        assert_eq!(report.passed(), 5);
    }

    #[test]
    fn a_renamed_core_user_field_fails_the_user_check() {
        let mut snapshot = canonical_snapshot();
        snapshot.users[0].value = json!({
            "e_mail": "ada@example.com",
            "first_name": "Ada",
            "last_name": "Lovelace"
        });
        let report = run_wire_battery(&snapshot);
        assert!(!report.is_conformant());
    }

    #[test]
    fn a_renamed_optional_core_user_field_fails_the_user_check() {
        let mut snapshot = canonical_snapshot();
        snapshot.users[0].value = json!({
            "email": "ada@example.com",
            "firstName": "Ada",
            "lastName": "Lovelace",
            "x_custom": { "nested": "value" }
        });
        let report = run_wire_battery(&snapshot);
        assert!(
            !report.is_conformant(),
            "renaming first_name -> firstName must drop it into extensions as None and fail W1: {:#?}",
            report.outcomes
        );
    }

    #[test]
    fn a_snapshot_with_only_null_name_users_fails_the_user_check() {
        let mut snapshot = canonical_snapshot();
        snapshot.users = vec![entry(
            "identity/users/01938c1f-0000-7000-8000-000000000002",
            json!({ "email": "grace@example.com", "first_name": null, "last_name": null }),
        )];
        let report = run_wire_battery(&snapshot);
        assert!(
            !report.is_conformant(),
            "a snapshot binding no populated names cannot exercise the optional-core-field guard: {:#?}",
            report.outcomes
        );
    }

    #[test]
    fn the_neutral_extension_lands_in_extensions_not_core() {
        let user = deserialize_user(&canonical_snapshot().users[0]).unwrap();
        assert!(user.extension(NEUTRAL_EXTENSION_KEY).is_some());
        assert!(user.extension("email").is_none());
        assert_eq!(user.email, "ada@example.com");
    }

    #[test]
    fn an_empty_member_ids_array_deserializes_to_no_members() {
        let group = deserialize_group(&canonical_snapshot().groups[1]).unwrap();
        assert_eq!(group.name, "guilds");
        assert!(group.member_ids.is_empty());
    }

    #[test]
    fn a_users_only_manifest_auto_degrades_groups() {
        let meta = deserialize_meta(&entry(
            "identity/_meta",
            json!({ "version": 1, "entities": ["users"] }),
        ))
        .unwrap();
        assert!(meta.publishes_users());
        assert!(!meta.publishes_groups());
    }
}

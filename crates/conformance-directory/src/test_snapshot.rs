use serde_json::{Value, json};

use crate::anchor::{DirectorySnapshotWire, KvEntry};

pub(crate) fn entry(key: &str, value: Value) -> KvEntry {
    KvEntry {
        key: key.to_string(),
        value,
    }
}

pub(crate) fn canonical_snapshot() -> DirectorySnapshotWire {
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

use conformance_directory::DirectorySnapshotWire;
use serde_json::json;

pub fn snapshot() -> DirectorySnapshotWire {
    serde_json::from_value(json!({
        "meta": {
            "key": "identity/_meta",
            "value": { "version": 1, "entities": ["users", "groups"] }
        },
        "users": [
            {
                "key": "identity/users/01938c1f-0000-7000-8000-000000000001",
                "value": {
                    "email": "ada@example.com",
                    "first_name": "Ada",
                    "last_name": "Lovelace",
                    "x_custom": { "nested": "value" }
                }
            },
            {
                "key": "identity/users/01938c1f-0000-7000-8000-000000000002",
                "value": { "email": "grace@example.com", "first_name": null, "last_name": null }
            }
        ],
        "groups": [
            {
                "key": "identity/groups/01938c1f-0000-7000-8000-0000000000a1",
                "value": {
                    "name": "engineering",
                    "member_ids": [
                        "01938c1f-0000-7000-8000-000000000001",
                        "01938c1f-0000-7000-8000-000000000002"
                    ]
                }
            },
            {
                "key": "identity/groups/01938c1f-0000-7000-8000-0000000000a2",
                "value": { "name": "guilds", "member_ids": [] }
            }
        ]
    }))
    .expect("the fixture snapshot has the anchor transport shape")
}

use std::collections::BTreeMap;

use br_core_directory::{
    DIRECTORY_META_VERSION, DirectoryMeta, PublishedEntity, PublishedGroup, PublishedUser,
};
use br_util_directory::{DirectoryError, DirectorySource};
use uuid::Uuid;

use crate::anchor::DirectorySnapshotWire;
use crate::error::Result;
use crate::wire::{deserialize_group, deserialize_user};

#[derive(Debug, Clone)]
pub struct AnchorSource {
    users: BTreeMap<Uuid, PublishedUser>,
    groups: BTreeMap<Uuid, PublishedGroup>,
    publish_groups: bool,
}

impl AnchorSource {
    pub fn from_snapshot(snapshot: &DirectorySnapshotWire) -> Result<Self> {
        let mut users = BTreeMap::new();
        for (id, entry) in snapshot.keyed_users()? {
            users.insert(id, deserialize_user(entry)?);
        }
        let mut groups = BTreeMap::new();
        for (id, entry) in snapshot.keyed_groups()? {
            groups.insert(id, deserialize_group(entry)?);
        }
        Ok(Self {
            users,
            groups,
            publish_groups: true,
        })
    }

    pub fn without_groups(mut self) -> Self {
        self.publish_groups = false;
        self.groups = BTreeMap::new();
        self
    }

    pub fn users(&self) -> &BTreeMap<Uuid, PublishedUser> {
        &self.users
    }

    pub fn groups(&self) -> &BTreeMap<Uuid, PublishedGroup> {
        &self.groups
    }

    pub fn drop_user(&mut self, user_id: &Uuid) {
        self.users.remove(user_id);
    }

    pub fn upsert_user(&mut self, user_id: Uuid, user: PublishedUser) {
        self.users.insert(user_id, user);
    }

    pub fn upsert_group(&mut self, group_id: Uuid, group: PublishedGroup) {
        self.groups.insert(group_id, group);
    }

    pub fn drop_group(&mut self, group_id: &Uuid) {
        self.groups.remove(group_id);
    }

    pub fn first_user(&self) -> Option<(Uuid, PublishedUser)> {
        self.users
            .iter()
            .next()
            .map(|(id, user)| (*id, user.clone()))
    }
}

#[async_trait::async_trait]
impl DirectorySource for AnchorSource {
    fn manifest(&self) -> DirectoryMeta {
        let mut entities = vec![PublishedEntity::Users];
        if self.publish_groups {
            entities.push(PublishedEntity::Groups);
        }
        DirectoryMeta {
            version: DIRECTORY_META_VERSION,
            entities,
        }
    }

    async fn desired_users(
        &self,
    ) -> std::result::Result<BTreeMap<Uuid, PublishedUser>, DirectoryError> {
        Ok(self.users.clone())
    }

    async fn desired_groups(
        &self,
    ) -> std::result::Result<BTreeMap<Uuid, PublishedGroup>, DirectoryError> {
        Ok(self.groups.clone())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::error::ConformanceError;
    use crate::test_snapshot::{canonical_snapshot, entry};

    #[test]
    fn the_canonical_snapshot_loads_every_entry() {
        let source = AnchorSource::from_snapshot(&canonical_snapshot()).unwrap();
        assert_eq!(source.users().len(), 2);
        assert_eq!(source.groups().len(), 2);
    }

    #[test]
    fn a_user_under_a_malformed_key_is_refused_not_dropped() {
        let mut snapshot = canonical_snapshot();
        snapshot.users[1].key = "identity/users/not-a-uuid".to_string();
        match AnchorSource::from_snapshot(&snapshot) {
            Err(ConformanceError::NonConformantWire { key, ty, .. }) => {
                assert_eq!(key, "identity/users/not-a-uuid");
                assert_eq!(ty, "PublishedUser");
            }
            other => panic!("a malformed user key must be refused: {other:?}"),
        }
    }

    #[test]
    fn a_group_under_a_foreign_key_is_refused_not_dropped() {
        let mut snapshot = canonical_snapshot();
        snapshot.groups.push(entry(
            "identity/teams/01938c1f-0000-7000-8000-0000000000a3",
            json!({ "name": "teams", "member_ids": [] }),
        ));
        match AnchorSource::from_snapshot(&snapshot) {
            Err(ConformanceError::NonConformantWire { key, ty, .. }) => {
                assert_eq!(key, "identity/teams/01938c1f-0000-7000-8000-0000000000a3");
                assert_eq!(ty, "PublishedGroup");
            }
            other => panic!("a foreign group key must be refused: {other:?}"),
        }
    }

    #[test]
    fn a_malformed_key_is_refused_even_when_its_value_is_garbage() {
        let mut snapshot = canonical_snapshot();
        snapshot
            .users
            .push(entry("identity/users/nope", json!("not a user")));
        assert!(matches!(
            AnchorSource::from_snapshot(&snapshot),
            Err(ConformanceError::NonConformantWire { .. })
        ));
    }

    #[test]
    fn two_entries_resolving_to_one_user_id_are_refused_not_merged() {
        let mut snapshot = canonical_snapshot();
        let twin = entry(
            "identity/users/01938C1F-0000-7000-8000-000000000001",
            snapshot.users[0].value.clone(),
        );
        snapshot.users.push(twin);
        assert!(matches!(
            AnchorSource::from_snapshot(&snapshot),
            Err(ConformanceError::NonConformantWire { .. })
        ));
    }
}

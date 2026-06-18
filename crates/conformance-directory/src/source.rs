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
        for entry in &snapshot.users {
            if let Some(id) = DirectorySnapshotWire::user_id(entry) {
                users.insert(id, deserialize_user(entry)?);
            }
        }
        let mut groups = BTreeMap::new();
        for entry in &snapshot.groups {
            if let Some(id) = DirectorySnapshotWire::group_id(entry) {
                groups.insert(id, deserialize_group(entry)?);
            }
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

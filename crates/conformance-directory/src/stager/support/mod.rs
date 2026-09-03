mod phase;
mod projection;
mod published;

use br_util_directory::{GROUP_NAMESPACE, USER_NAMESPACE};
use uuid::Uuid;

pub(crate) use phase::Phase;
pub(crate) use projection::{projected_first_name, projected_user};
pub(crate) use published::{
    groups_holding, members_of, published_group, published_user, with_first_name, with_name,
    without_member,
};

pub(crate) fn user_ref(user_id: Uuid) -> String {
    format!("{USER_NAMESPACE}/{user_id}")
}

pub(crate) fn group_ref(group_id: Uuid) -> String {
    format!("{GROUP_NAMESPACE}/{group_id}")
}

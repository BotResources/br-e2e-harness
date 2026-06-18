use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FrozenWire {
    pub command_subjects: Vec<FrozenCommandSubject>,
    pub event_subjects: Vec<FrozenEventSubject>,
    pub published_users: Vec<FrozenPublishedUser>,
    pub poison_user_key: String,
    pub poison_user_value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrozenCommandSubject {
    pub receiver: String,
    pub aggregate: String,
    pub verb: String,
    pub version: u8,
    pub subject: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrozenEventSubject {
    pub producer: String,
    pub aggregate: String,
    pub fact: String,
    pub version: u8,
    pub subject: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrozenPublishedUser {
    pub key: String,
    pub value: serde_json::Value,
}

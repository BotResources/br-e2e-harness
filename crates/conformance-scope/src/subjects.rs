use br_core_integration::SubjectError;
use br_scope_declaration_contract::{accepted_subject, command_subject, rejected_subject};

pub const STREAM_SUBJECTS: &str = "identity.>";

pub fn declare_subject() -> Result<String, SubjectError> {
    command_subject()
}

pub fn accepted_event_subject() -> Result<String, SubjectError> {
    accepted_subject()
}

pub fn rejected_event_subject() -> Result<String, SubjectError> {
    rejected_subject()
}

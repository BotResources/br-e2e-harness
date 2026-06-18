use br_scope_declaration_contract::{
    accepted_event_coords, declare_command_coords, rejected_event_coords,
};
use br_util_nats_fabric::{CoordError, command_subject, event_subject};

pub fn declare_subject() -> Result<String, CoordError> {
    Ok(command_subject(&declare_command_coords()?))
}

pub fn accepted_event_subject() -> Result<String, CoordError> {
    Ok(event_subject(&accepted_event_coords()?))
}

pub fn rejected_event_subject() -> Result<String, CoordError> {
    Ok(event_subject(&rejected_event_coords()?))
}

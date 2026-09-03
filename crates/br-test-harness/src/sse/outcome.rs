use serde_json::Value;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum SseOutcome {
    Event(Value),
    Timeout,
    Closed,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainStop {
    Limit,
    Timeout,
    Closed,
}

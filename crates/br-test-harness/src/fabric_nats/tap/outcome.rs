use super::TappedDelivery;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapOutcome {
    Delivery(TappedDelivery),
    Timeout,
    Closed,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapStop {
    Limit,
    Timeout,
    Closed,
}

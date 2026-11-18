#![doc = include_str!("../README.md")]

mod abort_on_drop;
pub mod anchor;
mod binary;
pub mod checks;
pub mod error;
pub mod harness;
mod release_asset;
pub mod report;
pub mod runner;
#[cfg(test)]
mod test_scratch;
pub mod wire;

pub use anchor::frozen_wire;
pub use error::{ConformanceError, Result};
pub use report::{
    CheckId, CheckOutcome, CheckStatus, ConformanceReport, attach_default, parse_checks,
    spawn_default,
};
pub use runner::{AttachTarget, DEFAULT_TIMEOUT, redact_nats_url, run_attach, run_spawn};
pub use wire::{FrozenCommandSubject, FrozenEventSubject, FrozenPublishedUser, FrozenWire};

#![doc = include_str!("../README.md")]

pub mod anchor;
pub mod checks;
pub mod error;
pub mod harness;
pub mod wire;

pub use anchor::{anchor_dir, build_anchor, ensure_go_available, frozen_wire};
pub use error::{ConformanceError, Result};
pub use wire::{FrozenCommandSubject, FrozenEventSubject, FrozenPublishedUser, FrozenWire};

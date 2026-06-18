pub mod acceptor;
pub mod build;
pub mod capture;
pub mod checks;
pub mod error;
pub mod expected;
pub mod harness;
pub mod outcome;
pub mod readyz;
pub mod runner;
pub mod scenario;
pub mod spawn;
pub mod subject;
pub mod subjects;

pub use acceptor::{accept, reject};
pub use build::{build_subject, ensure_go_available, subject_dir};
pub use capture::{CapturedDeclare, DeclareCapture};
pub use checks::{CheckContext, run_scenario};
pub use error::{ConformanceError, Result};
pub use expected::{
    ExpectedDeclaration, ExpectedScope, PlatformOnly, SAMPLE_FALLBACK_SCOPE, parse_platform_only,
    parse_scope_keys,
};
pub use harness::{COMMAND_STREAM_NAME, EVENT_STREAM_NAME, ScopeHarness};
pub use outcome::{CheckId, CheckOutcome, CheckStatus, ConformanceReport};
pub use readyz::ReadyzProbe;
pub use runner::{AttachTarget, DEFAULT_TIMEOUT, run_attach};
pub use scenario::{AcceptorBehavior, Scenario, attach_default, parse_scenarios, spawn_default};
pub use spawn::{SpawnTarget, run_spawn};
pub use subject::{Subject, SubjectConfig};
pub use subjects::{accepted_event_subject, declare_subject, rejected_event_subject};

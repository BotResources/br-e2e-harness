use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "conformance-scope",
    about = "Conformance-test a service against the BotResources scope-declaration wire handshake.",
    long_about = "Drive the frozen scope-declaration handshake against a service in any language.\n\
        In ATTACH mode (default) the CLI connects to a running service's NATS and /readyz and\n\
        plays the Identity side of the handshake with zero host dependencies. In SPAWN mode it\n\
        stands up a throwaway nats-server and launches a subject binary to run the full battery.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(Box<RunArgs>),
    Manifest(ManifestArgs),
}

#[derive(Debug, Args)]
#[command(about = "Run the conformance battery against a single service.")]
pub struct RunArgs {
    #[arg(
        long,
        conflicts_with = "spawn",
        help = "Attach to an already-running service (default). Requires --nats and --readyz."
    )]
    pub attach: bool,

    #[arg(
        long,
        value_name = "PATH",
        help = "Spawn this subject binary against a throwaway nats-server (needs `nats-server` on PATH)."
    )]
    pub spawn: Option<PathBuf>,

    #[arg(
        long,
        value_name = "URL",
        help = "NATS URL of the running service (attach mode), e.g. nats://127.0.0.1:4222."
    )]
    pub nats: Option<String>,

    #[arg(
        long,
        value_name = "URL",
        help = "The service's /readyz URL (attach mode), e.g. http://127.0.0.1:8080/readyz."
    )]
    pub readyz: Option<String>,

    #[arg(
        long,
        value_name = "NAME",
        help = "JetStream stream carrying the handshake subjects (attach mode). Defaults to the wire-contract stream."
    )]
    pub stream: Option<String>,

    #[arg(
        long,
        value_name = "KEY",
        help = "Expected declaring service key, e.g. example-service."
    )]
    pub service_key: String,

    #[arg(
        long,
        value_name = "CSV",
        default_value = "",
        help = "Expected scope keys, comma-separated, e.g. example:read,example:admin."
    )]
    pub scopes: String,

    #[arg(
        long,
        value_name = "BOOL|CSV",
        default_value = "false",
        help = "Expected platform_only: a single bool for all scopes, or a per-scope `key=bool` CSV."
    )]
    pub platform_only: String,

    #[arg(
        long,
        conflicts_with = "reject",
        help = "Play the acceptor as ACCEPT (default)."
    )]
    pub accept: bool,

    #[arg(
        long,
        value_name = "REASON",
        num_args = 0..=1,
        default_missing_value = "scope_owned_by_another_service",
        help = "Play the acceptor as REJECT, exercising the rejection path (s4). Optional reason code."
    )]
    pub reject: Option<String>,

    #[arg(
        long,
        value_name = "CSV",
        help = "Scenarios to run, e.g. s1,s2. Defaults: attach => s1,s2,declaration-content; spawn => s1..s6 + content."
    )]
    pub scenarios: Option<String>,

    #[arg(
        long,
        value_name = "DUR",
        default_value = "10s",
        help = "Per-step timeout, e.g. 10s, 500ms."
    )]
    pub timeout: String,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Debug, Args)]
#[command(
    about = "Run the battery against many services described in a YAML manifest, and aggregate."
)]
pub struct ManifestArgs {
    #[arg(
        value_name = "FILE",
        help = "Path to the YAML manifest describing the services to test."
    )]
    pub file: PathBuf,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Debug, Args)]
pub struct OutputArgs {
    #[arg(
        long,
        value_enum,
        default_value_t = Format::Human,
        help = "Report format."
    )]
    pub format: Format,

    #[arg(
        long,
        value_name = "PATH",
        help = "Write the report to a file instead of stdout."
    )]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Human,
    Json,
    Junit,
}

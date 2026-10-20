use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "conformance-identity",
    about = "Conformance-test an Identity service against the BotResources scope-acceptance side of the scope-declaration handshake.",
    long_about = "Drive the frozen scope-acceptance battery (A1..A7) against an Identity service in any language.\n\
        The CLI plays the declaring service: it publishes DeclareServiceScopes commands and judges\n\
        the ServiceScopes confirmations the acceptor answers, against the real contract types.\n\
        In SPAWN mode it stands up a throwaway nats-server and launches the acceptor binary with\n\
        NATS_URL, HTTP_ADDR and SCOPE_ACCEPTANCE_ENABLED. In ATTACH mode it connects to a running\n\
        acceptor's NATS and /readyz; the deployment owns the INTEGRATION_CMD and INTEGRATION_EVT\n\
        streams. ANCHOR mode spawns the Go reference acceptor `identity-acceptor`. Spawn mode needs\n\
        `nats-server` on PATH and the `fabric-nats` binary beside this executable.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(Box<RunArgs>),
}

#[derive(Debug, Args)]
#[command(
    about = "Run the conformance battery against one acceptor.",
    group(ArgGroup::new("mode").required(true).multiple(false).args(["spawn", "anchor", "attach"]))
)]
pub struct RunArgs {
    #[arg(
        long,
        value_name = "PATH",
        help = "Spawn this acceptor binary (a path, or a bare name resolved on PATH) against a throwaway nats-server."
    )]
    pub spawn: Option<PathBuf>,

    #[arg(
        long,
        help = "Spawn the Go reference acceptor `identity-acceptor` instead: looked up on PATH, then in the cache directory, then downloaded from the conformance-identity release."
    )]
    pub anchor: bool,

    #[arg(
        long,
        requires_all = ["nats", "readyz"],
        help = "Attach to an already-running acceptor. Requires --nats and --readyz."
    )]
    pub attach: bool,

    #[arg(
        long,
        value_name = "URL",
        requires = "attach",
        help = "NATS URL of the running acceptor (attach mode), e.g. nats://127.0.0.1:4222."
    )]
    pub nats: Option<String>,

    #[arg(
        long,
        value_name = "URL",
        requires = "attach",
        help = "The acceptor's /readyz URL (attach mode), e.g. http://127.0.0.1:8080/readyz."
    )]
    pub readyz: Option<String>,

    #[arg(
        long,
        value_name = "CSV",
        help = "Scenarios to run, e.g. a1,a2. Default: a1..a7."
    )]
    pub scenarios: Option<String>,

    #[arg(
        long,
        value_name = "DUR",
        default_value = "10s",
        help = "Timeout waiting for the acceptor's /readyz and for each confirmation, e.g. 10s, 500ms."
    )]
    pub timeout: String,

    #[arg(
        long,
        value_name = "LABEL",
        help = "Label of the acceptor in the report. Default: the file name of the binary, `identity-acceptor` for --anchor, the /readyz URL for --attach."
    )]
    pub subject_label: Option<String>,

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

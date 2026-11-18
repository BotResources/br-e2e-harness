use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "conformance-nats-fabric",
    about = "Conformance-test the BotResources NATS Fabric: the v1 subject grammar and the published-language KV.",
    long_about = "Drive the NATS Fabric conformance battery from any CI, in any language.\n\
        The battery checks the br-util-nats-fabric linked into this binary against a real NATS\n\
        server and against an independent Go anchor (the `nats-fabric` binary), which is found on PATH,\n\
        then in the cache directory, then downloaded and checksum-verified from the\n\
        conformance-nats-fabric/v{version} release. It starts no service: there is no service binary\n\
        to boot.\n\n\
        SPAWN mode (default) stands up throwaway nats-servers (needs `nats-server` on PATH) and runs\n\
        every check. ATTACH mode (--nats URL) runs against an existing server whose fixed streams and\n\
        PUBLISHED_LANGUAGE bucket already exist; it provisions nothing, removes what it wrote, and\n\
        does not run by default the two checks that need a server without the fixed streams (selected with --checks, they are reported skipped).\n\n\
        Exit codes: 0 conformant, 1 a check failed, 2 usage error, unknown check, repeated check, empty selection,\n\
        anchor binary unavailable, server unreachable,\n\
        a report that cannot be written (stdout or --output file).",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(Box<RunArgs>),
    #[command(about = "List the check ids and what each one expects.")]
    List,
}

#[derive(Debug, Args)]
#[command(about = "Run the conformance battery and report.")]
pub struct RunArgs {
    #[arg(
        long,
        value_name = "URL",
        help = "ATTACH to this NATS server, e.g. nats://127.0.0.1:4222. Without it, SPAWN throwaway servers."
    )]
    pub nats: Option<String>,

    #[arg(
        long,
        value_name = "CSV",
        help = "Check ids to run (see `list`). Defaults: spawn => all; attach => all but the spawn-only checks."
    )]
    pub checks: Option<String>,

    #[arg(
        long,
        value_name = "DUR",
        default_value = "60s",
        help = "Per-check timeout, above zero, e.g. 60s, 500ms. Bounds the check body only; the attach cleanup has its own 30s bound."
    )]
    pub timeout: String,

    #[arg(long, value_enum, default_value_t = Format::Human, help = "Report format.")]
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

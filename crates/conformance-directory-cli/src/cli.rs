use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "conformance-directory",
    about = "Conformance-test the BotResources identity Published Language (directory) kit and wire.",
    long_about = "Drive the frozen directory battery (W1..W6, P1..P2, C1..C6) in any language's CI.\n\
        The Go reference anchor `identity-directory` prints the frozen KV snapshot; the wire checks\n\
        deserialize it through the real contract types, and the publisher and consumer checks\n\
        replay it through the real kit against a throwaway nats-server and a throwaway Postgres.\n\
        There is no spawn or attach mode: the battery tests the kit and the frozen wire, not a\n\
        service binary. w1..w6 need nothing but the anchor. p1..p2 need `nats-server` on PATH and the\n\
        `fabric-nats` binary beside this executable. c1..c6 also need a Postgres whose admin URL\n\
        is in E2E_PG_ADMIN_URL or DATABASE_URL, read from the environment first, then from the\n\
        nearest `.env` found by walking up from the working directory.",
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
#[command(about = "Run the conformance battery.")]
pub struct RunArgs {
    #[arg(
        long,
        value_name = "PATH",
        help = "Run this `identity-directory` anchor binary. Default: looked up on PATH, then in the cache directory, then downloaded from the conformance-directory release."
    )]
    pub anchor_binary: Option<PathBuf>,

    #[arg(
        long,
        value_name = "CSV",
        help = "Checks to run, e.g. w1,p1,c3. Default: w1..w6, p1..p2, c1..c6, always in that order."
    )]
    pub checks: Option<String>,

    #[arg(
        long,
        value_name = "LABEL",
        default_value = "identity-directory",
        help = "Label of the subject in the report."
    )]
    pub subject_label: String,

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

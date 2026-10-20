use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "conformance-passport",
    about = "Conformance-test a service against the BotResources bearer/PAT to Passport contract (GET /internal/passport).",
    long_about = "Drive the frozen sealed-bearer to Passport battery (P1..P10) against a service in any language.\n\
        The CLI stands up a throwaway nats-server with the PUBLISHED_LANGUAGE bucket, spawns the\n\
        subject binary with NATS_URL, PORT and BEARER_SEAL_KEY, waits for /readyz, seeds the committed\n\
        Go-frozen wire vectors and asserts the answers of GET /internal/passport.\n\
        There is no attach mode: P8 destroys the PUBLISHED_LANGUAGE bucket, so the battery only runs\n\
        against a subject it spawns itself on its own broker. Needs `nats-server` on PATH.",
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
#[command(about = "Run the conformance battery against one subject binary.")]
pub struct RunArgs {
    #[arg(
        long,
        value_name = "PATH",
        required_unless_present = "anchor",
        conflicts_with = "anchor",
        help = "Spawn this service binary (a path, or a bare name resolved on PATH) as the subject."
    )]
    pub spawn: Option<PathBuf>,

    #[arg(
        long,
        help = "Drive the Go reference subject `identity-passport` instead: looked up on PATH, then in the cache directory, then downloaded from the conformance-passport release."
    )]
    pub anchor: bool,

    #[arg(
        long,
        value_name = "CSV",
        help = "Scenarios to run, e.g. p1,p2. Default: p1..p10 (P8 is destructive and always runs last). g4 is an offline lib test and is not selectable."
    )]
    pub scenarios: Option<String>,

    #[arg(
        long,
        value_name = "DUR",
        default_value = "10s",
        help = "Timeout waiting for the subject's /readyz, e.g. 10s, 500ms."
    )]
    pub timeout: String,

    #[arg(
        long,
        value_name = "LABEL",
        help = "Label of the subject in the report. Default: the file name of the subject binary."
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

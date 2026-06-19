use std::process::ExitCode;

use br_test_harness::{FabricTestNats, Manifest, ManifestError};
use br_util_nats_fabric::FabricError;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "fabric-nats",
    about = "Provision the Project NATS Fabric topology from typed coordinates (test/dev only)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Provision {
        #[arg(long)]
        nats: String,
        #[arg(long)]
        manifest: String,
        #[arg(long)]
        run_id: Option<String>,
    },
    Verify {
        #[arg(long)]
        nats: String,
        #[arg(long)]
        manifest: String,
        #[arg(long)]
        run_id: Option<String>,
    },
    PrintSubjects {
        #[arg(long)]
        manifest: String,
        #[arg(long)]
        run_id: Option<String>,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(Exit { code, message }) => {
            eprintln!("error: {message}");
            ExitCode::from(code)
        }
    }
}

struct Exit {
    code: u8,
    message: String,
}

impl From<ManifestError> for Exit {
    fn from(e: ManifestError) -> Self {
        Exit {
            code: 2,
            message: e.to_string(),
        }
    }
}

async fn run() -> Result<(), Exit> {
    let cli = Cli::parse();
    match cli.command {
        Command::Provision {
            nats,
            manifest,
            run_id,
        } => provision(&nats, &manifest, run_id.as_deref()).await,
        Command::Verify {
            nats,
            manifest,
            run_id,
        } => verify(&nats, &manifest, run_id.as_deref()).await,
        Command::PrintSubjects { manifest, run_id } => print_subjects(&manifest, run_id.as_deref()),
    }
}

async fn provision(nats: &str, manifest: &str, run_id: Option<&str>) -> Result<(), Exit> {
    let rendered = Manifest::parse(manifest)?.render(run_id)?;
    let mut harness = connect(nats).await?;
    if rendered.published_language {
        harness = harness.with_published_language().await;
        println!("kv PUBLISHED_LANGUAGE");
    }
    if rendered.ephemeral_auth {
        harness = harness.with_ephemeral_auth().await;
        println!("kv EPHEMERAL_AUTH");
    }
    if rendered.bearer_tokens {
        harness = harness.with_bearer_tokens().await;
        println!("kv bearer_tokens");
    }
    for command in &rendered.commands {
        harness
            .provision_command_durable(&command.coords, &command.durable)
            .await;
        println!("cmd {} -> {}", command.durable, command.subject);
    }
    for event in &rendered.events {
        harness
            .provision_event_durable(&event.coords, &event.durable)
            .await;
        println!("evt {} -> {}", event.durable, event.subject);
    }
    harness.shutdown().await;
    Ok(())
}

async fn verify(nats: &str, manifest: &str, run_id: Option<&str>) -> Result<(), Exit> {
    let rendered = Manifest::parse(manifest)?.render(run_id)?;
    let harness = connect(nats).await?;
    let fabric = harness.fabric_owned();
    let mut result = Ok(());
    for command in &rendered.commands {
        if let Err(e) = fabric
            .verify_command_durable(&command.coords, &command.durable)
            .await
        {
            result = result.and(Err(verify_exit(e, &command.durable)));
        } else {
            println!("ok cmd {} -> {}", command.durable, command.subject);
        }
    }
    for event in &rendered.events {
        if let Err(e) = fabric
            .verify_event_durable(&event.coords, &event.durable)
            .await
        {
            result = result.and(Err(verify_exit(e, &event.durable)));
        } else {
            println!("ok evt {} -> {}", event.durable, event.subject);
        }
    }
    harness.shutdown().await;
    result
}

fn print_subjects(manifest: &str, run_id: Option<&str>) -> Result<(), Exit> {
    let rendered = Manifest::parse(manifest)?.render(run_id)?;
    if rendered.published_language {
        println!("kv PUBLISHED_LANGUAGE");
    }
    if rendered.ephemeral_auth {
        println!("kv EPHEMERAL_AUTH");
    }
    if rendered.bearer_tokens {
        println!("kv bearer_tokens");
    }
    for command in &rendered.commands {
        println!("cmd {} -> {}", command.durable, command.subject);
    }
    for event in &rendered.events {
        println!("evt {} -> {}", event.durable, event.subject);
    }
    Ok(())
}

async fn connect(nats: &str) -> Result<FabricTestNats, Exit> {
    let url = nats.to_string();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let attempt = std::panic::AssertUnwindSafe(FabricTestNats::connect(&url));
    let result = futures_util::FutureExt::catch_unwind(attempt).await;
    std::panic::set_hook(previous);
    result.map_err(|_| Exit {
        code: 3,
        message: format!("failed to connect to NATS at {nats}"),
    })
}

fn verify_exit(err: FabricError, durable: &str) -> Exit {
    Exit {
        code: 4,
        message: format!("verify mismatch on durable '{durable}': {err}"),
    }
}

use std::future::Future;
use std::process::ExitCode;

use br_test_harness::{BEARER_BUCKET, FabricTestNats, Manifest, ManifestError, Rendered};
use br_util_nats_fabric::{
    ConsumeErrorKind, FabricError, INTEGRATION_CMD, INTEGRATION_EVT, KV_PUBLISHED_LANGUAGE,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "fabric-nats",
    about = "Provision, verify or print the Project NATS Fabric topology from typed coordinates (test/dev only)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Get-or-create the manifest's durables and KV buckets on a running NATS. Idempotent, never wipes."
    )]
    Provision {
        #[arg(long)]
        nats: String,
        #[arg(long)]
        manifest: String,
        #[arg(long)]
        run_id: Option<String>,
    },
    #[command(
        about = "Read-only check: each fixed stream covers its coordinate subject and carries the durable with exactly that filter. Creates nothing."
    )]
    Verify {
        #[arg(long)]
        nats: String,
        #[arg(long)]
        manifest: String,
        #[arg(long)]
        run_id: Option<String>,
    },
    #[command(
        about = "Render the manifest's coordinates to subjects and durable names. Contacts no NATS."
    )]
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
    let harness = attach_without_provisioning(nats).await?;
    let fabric = harness.fabric_owned();
    let mut failed = None;

    let buckets = harness.kv_bucket_names().await;
    for bucket in expected_buckets(&rendered) {
        if buckets.contains(bucket) {
            println!("ok kv {bucket} (bucket present)");
        } else {
            report(&mut failed, bucket_absent(bucket));
        }
    }

    for command in &rendered.commands {
        let coverage = fabric
            .verify_command_durable(&command.coords, &command.durable)
            .await;
        match check_stream_and_durable(
            &harness,
            INTEGRATION_CMD,
            &command.durable,
            &command.subject,
            coverage,
        )
        .await
        {
            Ok(()) => println!(
                "ok cmd {} -> {} (stream covers the subject; durable filter is exactly it)",
                command.durable, command.subject
            ),
            Err(e) => report(&mut failed, e),
        }
    }

    for event in &rendered.events {
        let coverage = fabric
            .verify_event_durable(&event.coords, &event.durable)
            .await;
        match check_stream_and_durable(
            &harness,
            INTEGRATION_EVT,
            &event.durable,
            &event.subject,
            coverage,
        )
        .await
        {
            Ok(()) => println!(
                "ok evt {} -> {} (stream covers the subject; durable filter is exactly it)",
                event.durable, event.subject
            ),
            Err(e) => report(&mut failed, e),
        }
    }

    harness.shutdown().await;
    match failed {
        Some(exit) => Err(exit),
        None => Ok(()),
    }
}

fn expected_buckets(rendered: &Rendered) -> Vec<&'static str> {
    let mut buckets = Vec::new();
    if rendered.published_language {
        buckets.push(KV_PUBLISHED_LANGUAGE);
    }
    if rendered.bearer_tokens {
        buckets.push(BEARER_BUCKET);
    }
    buckets
}

fn bucket_absent(bucket: &str) -> Exit {
    Exit {
        code: 4,
        message: format!("kv bucket '{bucket}' is absent; run `fabric-nats provision` first"),
    }
}

fn report(failed: &mut Option<Exit>, exit: Exit) {
    eprintln!("error: {}", exit.message);
    if failed.is_none() {
        *failed = Some(exit);
    }
}

async fn check_stream_and_durable(
    harness: &FabricTestNats,
    stream: &'static str,
    durable: &str,
    subject: &str,
    coverage: Result<(), FabricError>,
) -> Result<(), Exit> {
    coverage.map_err(|e| coverage_exit(e, stream, durable, subject))?;
    match harness
        .durable_filter_subjects_if_present(stream, durable)
        .await
    {
        Some(filters) if filters == [subject] => Ok(()),
        Some(filters) => Err(Exit {
            code: 4,
            message: format!(
                "durable '{durable}' on {stream} filters {filters:?}, expected exactly [\"{subject}\"]"
            ),
        }),
        None => Err(Exit {
            code: 4,
            message: format!(
                "durable '{durable}' is absent from {stream} (expected filter '{subject}'); run `fabric-nats provision` first"
            ),
        }),
    }
}

fn coverage_exit(err: FabricError, stream: &str, durable: &str, subject: &str) -> Exit {
    let message = match &err {
        FabricError::Consume {
            kind: ConsumeErrorKind::NoStream,
            ..
        } => format!("stream {stream} does not exist (durable '{durable}' expects '{subject}')"),
        FabricError::SubjectNotCovered { configured, .. } => format!(
            "stream {stream} does not cover '{subject}' (durable '{durable}'); it binds {configured:?}"
        ),
        _ => format!("probing {stream} for '{subject}' (durable '{durable}') failed: {err}"),
    };
    Exit { code: 4, message }
}

fn print_subjects(manifest: &str, run_id: Option<&str>) -> Result<(), Exit> {
    let rendered = Manifest::parse(manifest)?.render(run_id)?;
    if rendered.published_language {
        println!("kv PUBLISHED_LANGUAGE");
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
    catch_connect_panic(nats, FabricTestNats::connect(&url)).await
}

async fn attach_without_provisioning(nats: &str) -> Result<FabricTestNats, Exit> {
    let url = nats.to_string();
    catch_connect_panic(nats, FabricTestNats::attach_without_provisioning(&url)).await
}

async fn catch_connect_panic(
    nats: &str,
    attempt: impl Future<Output = FabricTestNats>,
) -> Result<FabricTestNats, Exit> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(attempt)).await;
    std::panic::set_hook(previous);
    result.map_err(|_| Exit {
        code: 3,
        message: format!("failed to connect to NATS at {nats}"),
    })
}

use conformance_scope::{run_attach, run_spawn};

use crate::error::{CliError, Result};
use crate::report::{AggregateReport, ServiceReport};

use super::prepare::{Prepared, Target, prepare};
use super::schema::Manifest;

pub struct ManifestRun {
    pub report: AggregateReport,
    pub failure: Option<CliError>,
}

pub async fn run(manifest: &Manifest) -> Result<ManifestRun> {
    let prepared = prepare(manifest)?;
    let mut services = Vec::with_capacity(prepared.len());
    let mut failure = None;
    for service in &prepared {
        match service.run().await {
            Ok(report) => services.push(report),
            Err(e) => {
                failure = Some(CliError::new(format!(
                    "service {:?}: {e}; the run stopped here: the services before it are in the report, the services after it did not run",
                    service.service_key
                )));
                break;
            }
        }
    }
    Ok(ManifestRun {
        report: AggregateReport::from_services(services),
        failure,
    })
}

impl Prepared<'_> {
    async fn run(&self) -> Result<ServiceReport> {
        let report = match &self.target {
            Target::Attach(target) => {
                run_attach(
                    target,
                    &self.expected,
                    &self.behavior,
                    &self.scenarios,
                    self.timeout,
                )
                .await?
            }
            Target::Spawn(target) => {
                run_spawn(
                    target,
                    &self.expected,
                    &self.behavior,
                    &self.scenarios,
                    self.timeout,
                )
                .await?
            }
        };
        Ok(ServiceReport::from_report(self.service_key, &report))
    }
}

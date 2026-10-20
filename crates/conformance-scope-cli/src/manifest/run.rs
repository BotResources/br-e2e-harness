use conformance_scope::{run_attach, run_spawn};

use crate::error::{CliError, Result};
use crate::report::{AggregateReport, IncompleteService, ServiceReport};

use super::prepare::{Prepared, Target, prepare};
use super::schema::Manifest;

pub struct ManifestRun {
    pub report: AggregateReport,
    pub failure: Option<CliError>,
}

pub async fn run(manifest: &Manifest) -> Result<ManifestRun> {
    let prepared = prepare(manifest)?;
    let mut services = Vec::with_capacity(prepared.len());
    for service in &prepared {
        match service.run().await {
            Ok(report) => services.push(report),
            Err(e) => {
                let failure = CliError::new(format!(
                    "service {:?}: {e}; the run stopped here: the services before it are in the report, the services after it did not run",
                    service.service_key
                ));
                let incomplete = IncompleteService {
                    service_key: service.service_key.to_string(),
                    error: e.to_string(),
                };
                return Ok(ManifestRun {
                    report: AggregateReport::interrupted(services, incomplete),
                    failure: Some(failure),
                });
            }
        }
    }
    Ok(ManifestRun {
        report: AggregateReport::from_services(services),
        failure: None,
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

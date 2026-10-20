use conformance_scope::{run_attach, run_spawn};

use crate::error::Result;
use crate::report::{AggregateReport, ServiceReport};

use super::prepare::{Prepared, Target, prepare};
use super::schema::Manifest;

pub async fn run(manifest: &Manifest) -> Result<AggregateReport> {
    let prepared = prepare(manifest)?;
    let mut services = Vec::with_capacity(prepared.len());
    for service in &prepared {
        services.push(service.run().await?);
    }
    Ok(AggregateReport::from_services(services))
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

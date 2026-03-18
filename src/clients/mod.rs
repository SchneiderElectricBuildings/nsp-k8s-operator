pub mod error;
pub mod mgmt;
pub mod prometheus;
use crate::Ctx;
use std::sync::Arc;

use crate::config::models::AppConfig;

pub fn log_clients(config: Arc<AppConfig>) {
    let mut enabled: Vec<&'static str> = Vec::new();
    let mut disabled: Vec<&'static str> = Vec::new();

    for (on, name) in [(config.mgmt.enabled, "MGMT"), (config.prometheus.enabled, "Prometheus")] {
        if on {
            enabled.push(name)
        } else {
            disabled.push(name)
        }
    }

    tracing::info!("Enabled clients: {:?}, Disabled clients: {:?}", enabled, disabled);
}

/// Helper struct for getting all clients and their enabled flag in one place
pub struct Clients<'a> {
    pub mgmt_client: &'a mgmt::MGMTClient,
    pub prometheus_client: &'a prometheus::PrometheusClient,

    pub mgmt_enabled: bool,
    pub prometheus_enabled: bool,
}

/// Convenience to get all clients and their enabled flag in one place
impl<'a> Clients<'a> {
    pub fn snapshot(ctx: &'a Ctx) -> Self {
        Self {
            mgmt_client: &ctx.mgmt_client,
            prometheus_client: &ctx.prometheus_client,

            mgmt_enabled: ctx.mgmt_client.is_enabled(),
            prometheus_enabled: ctx.prometheus_client.is_enabled(),
        }
    }
}

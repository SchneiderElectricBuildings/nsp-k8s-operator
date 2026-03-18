use super::PrometheusError;
use crate::config::AppConfig;
use crate::utils::hotreload::{ConfigView, SelectView};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrometheusConfig {
    pub enabled: bool,
    // optional
    #[serde(default)]
    pub api_url: String,
}

impl ConfigView for PrometheusConfig {
    fn enabled(&self) -> bool {
        self.enabled
    }
}

// implement from_env
impl PrometheusConfig {
    pub fn from_env() -> Result<Self, PrometheusError> {
        let enabled = std::env::var("PROMETHEUS_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";
        let api_url = std::env::var("PROMETHEUS_API_URL")
            .map_err(|_| PrometheusError::MissingEnvVar("PROMETHEUS_API_URL".to_string()))?;

        Ok(PrometheusConfig { enabled, api_url })
    }
}

pub struct PrometheusSel;
impl SelectView for PrometheusSel {
    type View = PrometheusConfig;
    fn select(app: &AppConfig) -> &Self::View {
        &app.prometheus
    }
}

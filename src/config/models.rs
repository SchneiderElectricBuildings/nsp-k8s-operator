use k8s_openapi::api::core::v1::Toleration;
use serde::Deserialize;
use serde_inline_default::serde_inline_default;
use std::{collections::BTreeMap, time::Duration};

use crate::clients::{mgmt::MGMTConfig, prometheus::PrometheusConfig};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub namespace_scope: Option<String>,
    pub ebo: EboConfig,
    pub mgmt: MGMTConfig,
    pub prometheus: PrometheusConfig,
}

impl AppConfig {
    /// Fallback used when reading from CONFIG_PATH fails (kept minimal).
    pub fn fallback() -> Self {
        tracing::warn!("Falling back to default config");
        Self {
            namespace_scope: None,
            ebo: EboConfig::default(),
            mgmt: MGMTConfig::default(),
            prometheus: PrometheusConfig::default(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        super::load_from_env().unwrap_or_else(|_| Self::fallback())
    }
}

/* --------------------------- ebo --------------------------- */

#[serde_inline_default]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EboConfig {
    // optional maps/vectors
    #[serde(default)]
    pub labels: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub annotations: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub node_selector: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub tolerations: Option<Vec<Toleration>>,

    // probes
    pub probes: Probes,

    // required
    pub registry: String,
    pub pull_secret: Option<String>,
    #[serde_inline_default("ebo-default-custom-config".to_string())]
    pub custom_config_secret: String,
    pub backups: i8,
    pub environment: String,

    pub utils: Utils,
    pub ingress: IngressConfig,
    pub proxy: Proxy,
    pub initial_password: InitialPassword,
    pub nsp_machine_id: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Utils {
    pub registry: String,
    pub image: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Probes {
    pub liveness: Probe,
    pub readiness: Probe,
}

#[serde_inline_default]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Probe {
    #[serde_inline_default(true)]
    pub enabled: bool,
    pub initial_delay_seconds: Option<i32>,
    pub period_seconds: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub failure_threshold: Option<i32>,
    pub path: Option<String>,
}

#[serde_inline_default]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IngressConfig {
    // Common for ingress-controllers and Istio Gateway
    #[serde_inline_default("example.com".to_string())]
    pub domain: String,
    #[serde(default)]
    pub hostname_separator: String,
    pub namespace_in_domain: bool,

    pub controller: IngressControllerConfig,
    pub istio_gateway: IstioGatewayConfig,
}

#[serde_inline_default]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IstioGatewayConfig {
    pub enabled: bool,
    #[serde_inline_default("ebo-gateway".to_string())]
    pub gateway_name: String,
    #[serde(default)]
    pub istio_namespace: String,
    #[serde_inline_default("letsencrypt-prod-issuer".to_string())]
    pub cert_issuer_name: String,
    #[serde_inline_default("ClusterIssuer".to_string())]
    pub cert_issuer_kind: String,
}

#[serde_inline_default]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IngressControllerConfig {
    pub enabled: bool,
    pub annotations: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub cert_secret_name: String,
    pub ingress_class: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub enabled: bool,
    // optional strings
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub no_proxy_list: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InitialPassword {
    pub enabled: bool,
    /// e.g. "15m" (humantime). If omitted, default to 0s.
    #[serde(with = "humantime_serde", default)]
    pub additional_time_limit: Duration,
}

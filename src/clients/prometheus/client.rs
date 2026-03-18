use super::error::PrometheusError;
use super::metrics::PrometheusMetrics;
use super::{PrometheusConfig, PrometheusSel};
use crate::clients::prometheus::models::{PromResponse, VolumeStats};
use crate::config::AppConfig;
use crate::config::ConfigHandle;
use crate::metrics::Metrics;
use crate::utils::hotreload::{HotReloadClient, SelectView};
use arc_swap::ArcSwapOption;
use reqwest::Client as HttpClient;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::TracingMiddleware;
use std::sync::Arc;
use tracing::instrument;

#[derive(Clone)]
pub struct PrometheusClientInner {
    pub cfg: PrometheusConfig,
    pub http: ClientWithMiddleware,
}

pub struct PrometheusClient {
    handle: ConfigHandle,
    inner: ArcSwapOption<PrometheusClientInner>,
    metrics: PrometheusMetrics,
}

impl PrometheusClient {
    pub fn new(handle: ConfigHandle, metrics: Arc<Metrics>) -> Self {
        let prometheus_metrics = metrics
            .registrar()
            .register(|reg| PrometheusMetrics::default().register(reg));

        let client = Self {
            handle,
            inner: ArcSwapOption::from(None),
            metrics: prometheus_metrics,
        };
        // prime once
        client.ensure_fresh();
        client
    }

    /// Query Prometheus for a single numeric value and parse as u64 bytes.
    /// Returns Ok(None) if the series is missing.
    pub async fn prom_query_single_value(&self, base_url: &str, query: &str) -> Result<Option<u64>, PrometheusError> {
        let url = format!("{}/api/v1/query", base_url.trim_end_matches('/'));
        let client = reqwest::Client::new();
        let req = client.get(&url).query(&[("query", query)]);
        let resp = req.send().await?.error_for_status()?.json::<PromResponse>().await?;

        if resp.status != "success" {
            return Ok(None);
        }
        if let Some(first) = resp.data.result.first() {
            // Value comes as string, possibly in scientific notation, e.g., "1.234e+06"
            let v = first.value.1.parse::<f64>()?;
            if v.is_finite() && v >= 0.0 {
                return Ok(Some(v.round() as u64));
            }
        }
        Ok(None)
    }

    /// Get PVC volume stats via Prometheus (kubelet volume metrics).
    /// - `prometheus_base`: e.g., "https://prometheus.example.com"
    /// - `namespace`: the PVC namespace
    /// - `pvc`: the PVC name    
    #[instrument(skip(self), fields(pvc = %pvc, ns = %ns))]
    pub async fn get_pvc_volume_stats_via_prometheus(
        &self, prometheus_base: &str, pvc: &str, ns: &str,
    ) -> Result<VolumeStats, PrometheusError> {
        // PromQL instant queries for the latest sample at evaluation time (now)
        let capacity_q =
            format!(r#"kubelet_volume_stats_capacity_bytes{{namespace="{ns}", persistentvolumeclaim="{pvc}"}}"#);
        let used_q = format!(r#"kubelet_volume_stats_used_bytes{{namespace="{ns}", persistentvolumeclaim="{pvc}"}}"#);
        let avail_q =
            format!(r#"kubelet_volume_stats_available_bytes{{namespace="{ns}", persistentvolumeclaim="{pvc}"}}"#);

        // Query in parallel for speed
        let (cap, used, avail) = tokio::join!(
            self.prom_query_single_value(prometheus_base, &capacity_q),
            self.prom_query_single_value(prometheus_base, &used_q),
            self.prom_query_single_value(prometheus_base, &avail_q),
        );

        Ok(VolumeStats {
            capacity_bytes: cap?,
            used_bytes: used?,
            available_bytes: avail?,
        })
    }

    #[instrument(skip(self), fields(name = %name, ns = %ns))]
    pub async fn ok_to_perform_backup(&self, name: &str, ns: &str, backups: i8) -> Result<bool, PrometheusError> {
        let Some(inner) = self.with_inner(|i| i.clone()) else {
            // disabled at the moment
            return Ok(false);
        };

        let prom = inner.cfg.api_url;
        let existing_backups = backups;
        let pvc = format!("{}-backup", name);

        let stats: VolumeStats = match self.get_pvc_volume_stats_via_prometheus(&prom, &pvc, ns).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to fetch PVC stats: {}", e);
                return Err(PrometheusError::PVCInfoError);
            }
        };

        // needs enough storage space to perform a backup!
        // calculating existing backups and target amount of backups from config
        let mut used_per_backup: f64 = 0.0;
        let mut current_ratio: f64 = 0.0;
        if let Some(ratio) = stats.used_ratio() {
            used_per_backup = ratio / (existing_backups as f64);
            current_ratio = ratio;
            tracing::debug!("Used per backup: {:.4}", used_per_backup);
        } else {
            tracing::error!("Used ratio is not available (metrics missing).");
        }

        tracing::debug!("Current used ratio: {:.4}", current_ratio);
        let total_free = 1.0 - current_ratio;
        tracing::debug!("Current free ratio: {:.4}", total_free);
        if used_per_backup < total_free {
            tracing::debug!("Ok to perform backup");
            return Ok(true);
        }
        tracing::debug!("NOT ok to perform backup");
        Ok(false)
    }

    // Returns current enabled status, updating the gauge.
    /// This will refresh from the latest config and (re)build/disable the inner as needed.
    pub fn is_enabled(&self) -> bool {
        // Refresh from current AppConfig (may rebuild or disable inner)
        self.ensure_fresh();

        // Check current state without cloning the inner
        let enabled = self.inner.load().is_some();

        // Reflect in metrics
        self.metrics.set_enabled(enabled);

        enabled
    }
}

impl HotReloadClient for PrometheusClient {
    type View = PrometheusConfig;
    type Inner = PrometheusClientInner;

    fn handle(&self) -> &ConfigHandle {
        &self.handle
    }
    fn inner(&self) -> &ArcSwapOption<Self::Inner> {
        &self.inner
    }

    fn build_inner(view: Self::View) -> Self::Inner {
        let http = ClientBuilder::new(HttpClient::new())
            .with(TracingMiddleware::default())
            .build();
        PrometheusClientInner { cfg: view, http }
    }

    fn select_view(app: &AppConfig) -> &Self::View {
        PrometheusSel::select(app)
    }

    // Avoid rebuilding if only unchanged
    fn inner_needs_rebuild(&self, cur: &Self::Inner, new_view: &Self::View) -> bool {
        &cur.cfg != new_view
    }
}

use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, exemplar::HistogramWithExemplars, family::Family, gauge::Gauge},
    registry::Registry,
};
use tokio::time::Instant;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct MethodLabels {
    pub method: String, // "get", "post", ...
}

#[derive(Clone, Debug)]
pub struct PrometheusMetrics {
    // High-level events you asked for:
    //pub registered_total: Counter,

    // Low-level HTTP telemetry:
    pub api_calls: Family<MethodLabels, Counter>,
    pub api_duration: HistogramWithExemplars<MethodLabels>,
    /// Gauge for client enabled/disabled status.
    pub client_enabled: Gauge,
}

impl Default for PrometheusMetrics {
    fn default() -> Self {
        Self {
            //registered_total: Counter::default(),
            api_calls: Family::default(),
            api_duration: HistogramWithExemplars::new([0.01, 0.1, 0.25, 0.5, 1.0, 5.0, 15.0, 60.0].into_iter()),
            client_enabled: Gauge::default(),
        }
    }
}

impl PrometheusMetrics {
    /// Register all Prometheus metrics.
    pub fn register(self, r: &mut Registry) -> Self {
        //r.register("registered_total", "Prometheus: registered", self.registered_total.clone());

        r.register(
            "prometheus_client_enabled",
            "Prometheus: client enabled status",
            self.client_enabled.clone(),
        );

        // Pre-seed common HTTP methods.
        for method in ["get", "post", "put", "delete"] {
            let _ = self.api_calls.get_or_create(&MethodLabels {
                method: method.to_string(),
            });
        }

        self
    }

    /*
    pub fn registration(&self) {
        self.registered_total.inc();
    }

    #[inline]
    pub fn backup_performed(&self) {
        self.backup_performed_total.inc();
    }
    */

    /// Set enabled Status
    pub fn set_enabled(&self, enabled: bool) {
        self.client_enabled.set(if enabled { 0 } else { -1 });
    }

    /// Start timing an HTTP call and bump api_calls(method).
    pub fn count_and_measure(&self, method: impl Into<String>) -> PrometheusDurationMeasurer {
        let label = MethodLabels { method: method.into() };
        self.api_calls.get_or_create(&label).inc();
        PrometheusDurationMeasurer {
            start: Instant::now(),
            labels: Some(label),
            metric: self.api_duration.clone(),
        }
    }
}

/// Observes duration on drop.
pub struct PrometheusDurationMeasurer {
    start: Instant,
    labels: Option<MethodLabels>,
    metric: HistogramWithExemplars<MethodLabels>,
}

impl PrometheusDurationMeasurer {
    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.labels = Some(MethodLabels { method: method.into() });
        self
    }
}

impl Drop for PrometheusDurationMeasurer {
    fn drop(&mut self) {
        #[allow(clippy::cast_precision_loss)]
        let duration = self.start.elapsed().as_millis() as f64 / 1000.0;
        let labels = self.labels.take();
        self.metric.observe(duration, labels, None);
    }
}

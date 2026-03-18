use prometheus_client::{encoding::EncodeLabelSet, metrics::counter::Counter, registry::Registry};

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct MethodLabels {
    pub method: String,
}

#[derive(Clone, Debug, Default)]
pub struct ConfigMetrics {
    /// Counts all successful loads.
    pub loads_total: Counter,
    /// Counts all errors.
    pub errors_total: Counter,
}

impl ConfigMetrics {
    /// Register BA Portal metrics to the Prometheus registry.
    pub fn register(self, r: &mut Registry) -> Self {
        r.register("config_loads", "Config: successful loads", self.loads_total.clone());

        r.register("config_errors", "Config: errors", self.errors_total.clone());

        self
    }

    /// Increment the loads counter after a successful load.
    pub fn load(&self) {
        self.loads_total.inc();
    }

    /// Increment the errors counter after a failed load.
    pub fn error(&self) {
        self.errors_total.inc();
    }
}

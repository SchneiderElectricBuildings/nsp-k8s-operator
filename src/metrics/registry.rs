use prometheus_client::registry::Registry;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Metrics {
    pub registry: Arc<Mutex<Registry>>,
    pub reconcile: super::reconcile::ReconcileMetrics,
}

impl Default for Metrics {
    fn default() -> Self {
        let mut registry = Registry::with_prefix("ebo_ctrl_reconcile");
        let reconcile = super::reconcile::ReconcileMetrics::default().register(&mut registry);

        Self {
            registry: Arc::new(Mutex::new(registry)),
            reconcile,
        }
    }
}

impl Metrics {
    /// Give modules a handle they can use to register metric families.
    pub fn registrar(&self) -> MetricsRegistrar {
        MetricsRegistrar {
            inner: Arc::clone(&self.registry),
        }
    }

    /// Read-only access for exporters/scrapers.
    pub fn with_registry<R>(&self, f: impl FnOnce(&Registry) -> R) -> R {
        let guard = self.registry.lock().expect("metrics registry poisoned");
        f(&guard)
    }
}

#[derive(Clone, Debug)]
pub struct MetricsRegistrar {
    pub(super) inner: Arc<Mutex<Registry>>,
}

impl MetricsRegistrar {
    /// Modules call this once in their constructors to register their metrics.
    pub fn register<R>(&self, f: impl FnOnce(&mut Registry) -> R) -> R {
        let mut guard = self.inner.lock().expect("metrics registry poisoned");
        f(&mut guard)
    }
}

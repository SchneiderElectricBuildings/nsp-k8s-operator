use crate::crd::v1alpha6::EboServer;
use crate::eboserver::EboServerError;
use kube::ResourceExt;
use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, exemplar::HistogramWithExemplars, family::Family},
    registry::{Registry, Unit},
};
use tokio::time::Instant;

#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelSet, Debug, Default)]
pub struct ReconcileLabels {
    pub ebo_name: String,
    pub ebo_ns: String,
}

#[derive(Clone, Debug)]
pub struct ReconcileMetrics {
    pub runs: Counter,
    pub failures: Family<ErrorLabels, Counter>,
    pub duration: HistogramWithExemplars<ReconcileLabels>,
}

impl Default for ReconcileMetrics {
    fn default() -> Self {
        Self {
            runs: Counter::default(),
            failures: Family::<ErrorLabels, Counter>::default(),
            duration: HistogramWithExemplars::new([0.01, 0.1, 0.25, 0.5, 1., 5., 15., 60.].into_iter()),
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct ErrorLabels {
    pub instance: String,
    pub error: String,
}

impl ReconcileMetrics {
    /// Register API metrics to start tracking them.
    pub fn register(self, r: &mut Registry) -> Self {
        r.register_with_unit("duration", "reconcile duration", Unit::Seconds, self.duration.clone());
        r.register("failures", "reconciliation errors", self.failures.clone());
        r.register("runs", "reconciliations", self.runs.clone());
        self
    }

    pub fn set_failure(&self, ebo: &EboServer, e: &EboServerError) {
        self.failures
            .get_or_create(&ErrorLabels {
                instance: ebo.name_any(),
                error: e.metric_label(),
            })
            .inc();
    }

    pub fn count_and_measure(&self, labels: Option<ReconcileLabels>) -> ReconcileMeasurer {
        self.runs.inc();
        ReconcileMeasurer {
            start: Instant::now(),
            labels,
            metric: self.duration.clone(),
        }
    }
}

/// Smart function duration measurer
pub struct ReconcileMeasurer {
    start: Instant,
    labels: Option<ReconcileLabels>,
    metric: HistogramWithExemplars<ReconcileLabels>,
}

impl Drop for ReconcileMeasurer {
    fn drop(&mut self) {
        #[allow(clippy::cast_precision_loss)]
        let duration = self.start.elapsed().as_millis() as f64 / 1000.0;
        let labels = self.labels.take();
        self.metric.observe(duration, labels, None);
    }
}

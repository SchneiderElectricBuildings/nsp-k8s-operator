use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, exemplar::HistogramWithExemplars, family::Family, gauge::Gauge},
    registry::{Registry, Unit},
};
use tokio::time::Instant;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct MethodLabels {
    pub method: String, // "get", "post", ...
}

#[derive(Clone, Debug)]
pub struct MgmtMetrics {
    // High-level events you asked for:
    pub registered_total: Counter,
    pub registered_errors_total: Counter,
    pub license_activated_total: Counter,
    pub license_returned_total: Counter,
    pub license_refreshed_total: Counter,
    pub license_errors_total: Counter,
    pub license_refresh_errors_total: Counter,
    pub password_expired_total: Counter,
    pub password_active_total: Counter,
    pub password_temporary_total: Counter,
    pub password_errors_total: Counter,
    pub backup_performed_total: Counter,
    pub backup_checked_total: Counter,
    pub backup_errors_total: Counter,
    backup_list_ok_total: Counter,
    pub backup_list_errors_total: Counter,

    // Low-level HTTP telemetry:
    pub api_calls: Family<MethodLabels, Counter>,
    pub api_duration: HistogramWithExemplars<MethodLabels>,
    /// Gauge for client enabled/disabled status.
    pub client_enabled: Gauge,
}

impl Default for MgmtMetrics {
    fn default() -> Self {
        Self {
            registered_total: Counter::default(),
            registered_errors_total: Counter::default(),
            license_activated_total: Counter::default(),
            license_returned_total: Counter::default(),
            license_refreshed_total: Counter::default(),
            license_errors_total: Counter::default(),
            license_refresh_errors_total: Counter::default(),
            password_expired_total: Counter::default(),
            password_active_total: Counter::default(),
            password_temporary_total: Counter::default(),
            password_errors_total: Counter::default(),
            backup_performed_total: Counter::default(),
            backup_checked_total: Counter::default(),
            backup_errors_total: Counter::default(),
            backup_list_ok_total: Counter::default(),
            backup_list_errors_total: Counter::default(),

            api_calls: Family::default(),
            api_duration: HistogramWithExemplars::new([0.01, 0.1, 0.25, 0.5, 1.0, 5.0, 15.0, 60.0].into_iter()),
            client_enabled: Gauge::default(),
        }
    }
}

impl MgmtMetrics {
    /// Register all MGMT metrics.
    pub fn register(self, r: &mut Registry) -> Self {
        r.register("registered_total", "MGMT: registered", self.registered_total.clone());
        r.register(
            "registered_errors",
            "MGMT: errors during registration ops",
            self.registered_errors_total.clone(),
        );

        r.register(
            "licences_activated",
            "MGMT: licenses activated",
            self.license_activated_total.clone(),
        );
        r.register(
            "licences_returned",
            "MGMT: licenses returned",
            self.license_returned_total.clone(),
        );
        r.register(
            "licences_refreshed",
            "MGMT: licenses refreshed",
            self.license_refreshed_total.clone(),
        );
        r.register(
            "licences_errors",
            "MGMT: errors during license ops",
            self.license_errors_total.clone(),
        );
        r.register(
            "licences_refresh_errors",
            "MGMT: errors during license refresh",
            self.license_refresh_errors_total.clone(),
        );

        r.register(
            "password_expired",
            "MGMT: licenses activated",
            self.password_expired_total.clone(),
        );
        r.register("password_active", "MGMT: licenses returned", self.password_active_total.clone());
        r.register(
            "password_temporary",
            "MGMT: errors during license ops",
            self.password_temporary_total.clone(),
        );
        r.register(
            "password_errors",
            "MGMT: errors during temp password ops",
            self.password_errors_total.clone(),
        );

        r.register(
            "backup_performed",
            "MGMT: backups performed",
            self.backup_performed_total.clone(),
        );
        r.register("backup_checked", "MGMT: backups checked", self.backup_checked_total.clone());
        r.register("backup_errors", "MGMT: errors during backups", self.backup_errors_total.clone());
        r.register("backup_list_ok", "MGMT: listing backups ok", self.backup_list_ok_total.clone());
        r.register(
            "backup_list_errors",
            "MGMT: errors listing backups",
            self.backup_list_errors_total.clone(),
        );

        r.register(
            "mgmt_client_enabled",
            "MGMT: client enabled status",
            self.client_enabled.clone(),
        );

        r.register("licences_api_calls", "MGMT: HTTP calls", self.api_calls.clone());
        r.register_with_unit(
            "licences_api_duration",
            "MGMT: HTTP call duration",
            Unit::Seconds,
            self.api_duration.clone(),
        );

        // Pre-seed common HTTP methods.
        for method in ["get", "post", "put", "delete"] {
            let _ = self.api_calls.get_or_create(&MethodLabels {
                method: method.to_string(),
            });
        }

        self
    }

    /// Increment the registration counter after a successful registration.
    pub fn registration(&self) {
        self.registered_total.inc();
    }

    #[inline]
    pub fn registered_error(&self) {
        self.registered_errors_total.inc();
    }

    #[inline]
    pub fn inc_activated(&self) {
        self.license_activated_total.inc();
    }

    #[inline]
    pub fn inc_returned(&self) {
        self.license_returned_total.inc();
    }

    #[inline]
    pub fn inc_refreshed(&self) {
        self.license_refreshed_total.inc();
    }

    #[inline]
    pub fn license_error(&self) {
        self.license_errors_total.inc();
    }

    #[inline]
    pub fn license_refresh_error(&self) {
        self.license_refresh_errors_total.inc();
    }

    #[inline]
    pub fn password_inc_expired(&self) {
        self.password_expired_total.inc();
    }

    #[inline]
    pub fn password_inc_active(&self) {
        self.password_active_total.inc();
    }

    #[inline]
    pub fn password_inc_temporary(&self) {
        self.password_temporary_total.inc();
    }

    #[inline]
    pub fn password_error(&self) {
        self.password_errors_total.inc();
    }

    #[inline]
    pub fn backup_performed(&self) {
        self.backup_performed_total.inc();
    }

    #[inline]
    pub fn backup_checked(&self) {
        self.backup_checked_total.inc();
    }

    #[inline]
    pub fn backup_error(&self) {
        self.backup_errors_total.inc();
    }

    #[inline]
    pub fn backup_list_ok(&self) {
        self.backup_list_ok_total.inc();
    }

    #[inline]
    pub fn backup_list_error(&self) {
        self.backup_list_errors_total.inc();
    }

    /// Set enabled Status
    pub fn set_enabled(&self, enabled: bool) {
        self.client_enabled.set(if enabled { 0 } else { -1 });
    }

    /// Start timing an HTTP call and bump api_calls(method).
    pub fn count_and_measure(&self, method: impl Into<String>) -> MgmtDurationMeasurer {
        let label = MethodLabels { method: method.into() };
        self.api_calls.get_or_create(&label).inc();
        MgmtDurationMeasurer {
            start: Instant::now(),
            labels: Some(label),
            metric: self.api_duration.clone(),
        }
    }
}

/// Observes duration on drop.
pub struct MgmtDurationMeasurer {
    start: Instant,
    labels: Option<MethodLabels>,
    metric: HistogramWithExemplars<MethodLabels>,
}

impl MgmtDurationMeasurer {
    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.labels = Some(MethodLabels { method: method.into() });
        self
    }
}

impl Drop for MgmtDurationMeasurer {
    fn drop(&mut self) {
        #[allow(clippy::cast_precision_loss)]
        let duration = self.start.elapsed().as_millis() as f64 / 1000.0;
        let labels = self.labels.take();
        self.metric.observe(duration, labels, None);
    }
}

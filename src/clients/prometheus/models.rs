use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default)]
pub struct VolumeStats {
    pub capacity_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
}

impl VolumeStats {
    /// Fraction used: used / capacity (0.0..=1.0)
    pub fn used_ratio(&self) -> Option<f64> {
        match (self.used_bytes, self.capacity_bytes) {
            (Some(u), Some(c)) if c > 0 => Some(u as f64 / c as f64),
            _ => None,
        }
    }
    /// Fraction available: available / capacity (0.0..=1.0)
    pub fn available_ratio(&self) -> Option<f64> {
        match (self.available_bytes, self.capacity_bytes) {
            (Some(a), Some(c)) if c > 0 => Some(a as f64 / c as f64),
            _ => None,
        }
    }
    /// Pretty-print helper
    pub fn fmt_gib(v: Option<u64>) -> String {
        v.map(|b| format!("{:.2} GiB", b as f64 / 1024.0 / 1024.0 / 1024.0))
            .unwrap_or_else(|| "<not found>".to_string())
    }
}

/// Prometheus HTTP API response (instant query)
#[derive(Debug, Deserialize)]
pub struct PromResponse {
    pub(crate) status: String,
    pub data: PromData,
}

#[derive(Debug, Deserialize)]
pub struct PromData {
    pub result: Vec<PromResult>,
}

#[derive(Debug, Deserialize)]
pub struct PromResult {
    // Instant-vector form: ("<unix_time_float>", "<value_as_string>")
    pub value: (f64, String),
}

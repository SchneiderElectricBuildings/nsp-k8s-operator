use crate::clients::{mgmt::MGMTError, prometheus::PrometheusError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EboServerError {
    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),

    #[error("Kube Error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<EboServerError>>),

    /// Error in user input or EboServer resource definition, typically missing fields.
    #[error("Invalid EboServer CRD: {0}")]
    UserInputError(String),

    /// Error in MGMT API
    #[error("MGMT API Error: {0}")]
    MgmtApiError(#[from] MGMTError),

    /// Error in Prometheus API
    #[error("Prometheus API Error: {0}")]
    PrometheusApiError(#[from] PrometheusError),

    /// Error server not found
    #[error("Server not found Error")]
    ServerNotFound,
}
pub type Result<T, E = EboServerError> = std::result::Result<T, E>;

impl EboServerError {
    pub fn metric_label(&self) -> String {
        match self {
            EboServerError::SerializationError(_) => "serialization_error".to_string(),
            EboServerError::KubeError(_) => "kube_error".to_string(),
            EboServerError::FinalizerError(_) => "finalizer_error".to_string(),
            EboServerError::UserInputError(_) => "user_input_error".to_string(),
            EboServerError::MgmtApiError(_) => "mgmt_api_error".to_string(),
            EboServerError::PrometheusApiError(_) => "prometheus_api_error".to_string(),
            EboServerError::ServerNotFound => "server_not_found".to_string(),
        }
    }
}

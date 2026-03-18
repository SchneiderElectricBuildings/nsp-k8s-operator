use crate::clients::error::HttpWithCode;
use thiserror::Error;

// Implement MgmtError enum
#[derive(Debug, Error)]
pub enum PrometheusError {
    #[error("{0}")]
    // #[error(transparent)]
    HttpError(#[from] HttpWithCode),

    #[error(transparent)]
    ParseFloat(#[from] std::num::ParseFloatError),

    #[error("Reqwest middleware error: {0}")]
    ReqwestMiddleware(#[from] reqwest_middleware::Error),

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Server not found")]
    ServerNotFound,

    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),

    #[error("PVC info error")]
    PVCInfoError,

    #[error("Client disabled")]
    ClientDisabled,
}

impl From<reqwest::Error> for PrometheusError {
    fn from(e: reqwest::Error) -> Self {
        PrometheusError::HttpError(HttpWithCode::from(e))
    }
}

use crate::clients::error::HttpWithCode;
use quick_xml::DeError;
use thiserror::Error;

// Implement MgmtError enum
#[derive(Debug, Error)]
pub enum MGMTError {
    #[error("{0}")]
    // #[error(transparent)]
    HttpError(#[from] HttpWithCode),

    #[error("Reqwest middleware error: {0}")]
    ReqwestMiddleware(#[from] reqwest_middleware::Error),

    #[error("Registration failed: {0}")]
    RegistrationFailed(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Server not found")]
    ServerNotFound,

    #[error("Error with admin password check: {0}")]
    CheckAdminPasswordFailed(String),

    #[error("Backup failed: {0}")]
    BackupFailed(String),

    #[error("Backup status error: {0}")]
    BackupStatusError(String),

    #[error("Backup listing error: {0}")]
    BackupListError(String),

    #[error("License refresh failed: {0}")]
    LicenseRefreshFailed(String),

    /*
    #[error("Failed to fetch Server info: {0}")]
    FetchError(String),

    #[error("Failed to serialize request body")]
    SerializationError(String),
    */
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),

    #[error("Parsing response from mgmt")]
    XmlParseError,

    #[error("Client disabled")]
    ClientDisabled,
}

impl From<reqwest::Error> for MGMTError {
    fn from(e: reqwest::Error) -> Self {
        MGMTError::HttpError(HttpWithCode::from(e))
    }
}

impl From<DeError> for MGMTError {
    fn from(_e: DeError) -> Self {
        MGMTError::XmlParseError
    }
}

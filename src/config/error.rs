use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    /// `CONFIG_PATH` env var not set or unreadable.
    #[error("missing or invalid env var `{0}` for config path")]
    MissingConfigPathEnv(String),

    /// Building the layered `config::Config` failed.
    #[error("failed to build configuration: {0}")]
    ConfigBuild(#[from] config::ConfigError),

    /// Notify errors
    #[error("failed to notify config change: {0}")]
    Notify(#[from] notify::Error),
}

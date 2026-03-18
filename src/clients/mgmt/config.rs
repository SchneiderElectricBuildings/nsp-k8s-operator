use super::MGMTError;
use crate::config::AppConfig;
use crate::utils::hotreload::{ConfigView, SelectView};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MGMTConfig {
    pub enabled: bool,
    // optional
    #[serde(default)]
    pub api_url: String,
}

impl ConfigView for MGMTConfig {
    fn enabled(&self) -> bool {
        self.enabled
    }
}

// implement from_env
impl MGMTConfig {
    pub fn from_env() -> Result<Self, MGMTError> {
        let enabled = std::env::var("MGMT_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";
        let api_url =
            std::env::var("MGMT_API_URL").map_err(|_| MGMTError::MissingEnvVar("MGMT_API_URL".to_string()))?;

        Ok(MGMTConfig { enabled, api_url })
    }
}

pub struct MGMTSel;
impl SelectView for MGMTSel {
    type View = MGMTConfig;
    fn select(app: &AppConfig) -> &Self::View {
        &app.mgmt
    }
}

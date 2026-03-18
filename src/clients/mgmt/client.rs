use super::error::MGMTError;
use super::metrics::MgmtMetrics;
use super::{MGMTConfig, MGMTSel};
use crate::config::AppConfig;
use crate::config::ConfigHandle;
use crate::crd::v1alpha6::EboAdminPasswordStatus;
use crate::metrics::Metrics;
use crate::utils::hotreload::{HotReloadClient, SelectView};
use arc_swap::ArcSwapOption;
use reqwest::{Client as HttpClient, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::TracingMiddleware;
use std::sync::Arc;
use tracing::instrument;

#[derive(Clone)]
pub struct MGMTClientInner {
    pub cfg: MGMTConfig,
    pub http: ClientWithMiddleware,
}

pub struct MGMTClient {
    handle: ConfigHandle,
    inner: ArcSwapOption<MGMTClientInner>,
    metrics: MgmtMetrics,
}

impl MGMTClient {
    pub fn new(handle: ConfigHandle, metrics: Arc<Metrics>) -> Self {
        let mgmt_metrics = metrics.registrar().register(|reg| MgmtMetrics::default().register(reg));

        let client = Self {
            handle,
            inner: ArcSwapOption::from(None),
            metrics: mgmt_metrics,
        };
        // prime once
        client.ensure_fresh();
        client
    }

    // Refreshing EBO licenses
    #[instrument(skip(self), fields(name = %name, ns = %ns))]
    pub async fn refresh_licenses_ebo(&self, name: &str, ns: &str) -> Result<bool, MGMTError> {
        let Some(inner) = self.with_inner(|i| i.clone()) else {
            // disabled at the moment
            return Err(MGMTError::ClientDisabled);
        };

        let mgmt_endpoint = &format!("http://{}.{}.svc.cluster.local:8080/licenses/refresh", &name, &ns);
        let response = inner.http.get(mgmt_endpoint).send().await;
        match response {
            Ok(resp) => {
                tracing::debug!("EBO license refresh response: {:?}", resp);
                tracing::debug!("EBO license refresh status: {:?}", resp.status());
                if resp.status() != reqwest::StatusCode::OK {
                    // METRICS: error on non-200
                    self.metrics.license_refresh_error();
                    tracing::error!("Error license refresh response: {:?}", resp);
                    Err(MGMTError::LicenseRefreshFailed(
                        resp.error_for_status().unwrap_err().to_string(),
                    ))
                } else {
                    self.metrics.inc_refreshed();
                    Ok(true)
                }
            }
            Err(e) => {
                // METRICS: transport error
                self.metrics.license_refresh_error();
                tracing::error!("Error license refresh: {}", e);
                Err(MGMTError::LicenseRefreshFailed(e.to_string()))
            }
        }
    }

    #[instrument(skip(self), fields(name = %name, ns = %ns))]
    pub async fn check_admin_password_status(&self, name: &str, ns: &str) -> Result<EboAdminPasswordStatus, MGMTError> {
        let Some(inner) = self.with_inner(|i| i.clone()) else {
            return Err(MGMTError::ClientDisabled);
        };

        let url = format!("http://{}.{}.svc.cluster.local:8080/adminpassword/status", name, ns);
        let resp = match inner.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                self.metrics.password_error();
                tracing::error!(error = %e, "Transport error while calling admin password status");
                return Err(MGMTError::CheckAdminPasswordFailed(e.to_string()));
            }
        };

        let status = resp.status();
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                self.metrics.password_error();
                tracing::error!(error = %e, "Failed to read admin password status response body");
                return Err(MGMTError::CheckAdminPasswordFailed(e.to_string()));
            }
        };

        if !status.is_success() {
            self.metrics.password_error();
            tracing::error!(%status, body = %body, "Non-success status from admin password status");
            return Err(MGMTError::CheckAdminPasswordFailed(format!("status {status}: {body}")));
        }

        let normalized = body.trim();
        let status_enum = match normalized {
            "Expired" => {
                self.metrics.password_inc_expired();
                EboAdminPasswordStatus::Expired
            }
            "Active" => {
                self.metrics.password_inc_active();
                EboAdminPasswordStatus::Active
            }
            // Treat any other (including "Temporary") as Temporary
            other => {
                self.metrics.password_inc_temporary();
                tracing::debug!(raw_body = %other, "Treating password status as Temporary");
                EboAdminPasswordStatus::Temporary
            }
        };

        Ok(status_enum)
    }

    // Create backup, backup name as input, retention set in config file
    #[instrument(skip(self), fields(name = %name, ns = %ns))]
    pub async fn create_backup(&self, name: &str, ns: &str, backups: i8, backup_name: &str) -> Result<bool, MGMTError> {
        let Some(inner) = self.with_inner(|i| i.clone()) else {
            return Err(MGMTError::ClientDisabled);
        };

        let mut url =
            Url::parse(&format!("http://{}.{}.svc.cluster.local:8080/external/backup", name, ns)).map_err(|e| {
                self.metrics.backup_error();
                tracing::error!(error = %e, "Failed to construct backup URL");
                MGMTError::BackupFailed(e.to_string())
            })?;
        url.query_pairs_mut().append_pair("name", backup_name);
        url.query_pairs_mut().append_pair("backups", &backups.to_string());

        let resp = match inner.http.get(url.as_str()).send().await {
            Ok(r) => r,
            Err(e) => {
                self.metrics.backup_error();
                tracing::error!(error = %e, "Transport error while calling create_backup");
                return Err(MGMTError::BackupFailed(e.to_string()));
            }
        };

        let status = resp.status();
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                self.metrics.backup_error();
                tracing::error!(error = %e, "Failed to read create_backup response body");
                return Err(MGMTError::BackupFailed(e.to_string()));
            }
        };

        if !status.is_success() {
            self.metrics.backup_error();
            tracing::error!(%status, body = %body, "Non-success status from create_backup");
            return Err(MGMTError::BackupFailed(format!("status {status}: {body}")));
        }

        self.metrics.backup_performed();
        tracing::debug!(backup_name = %backup_name, "Backup request accepted/succeeded");
        Ok(true)
    }

    #[instrument(skip(self), fields(name = %name, ns = %ns))]
    pub async fn backup_status(&self, name: &str, ns: &str) -> Result<bool, MGMTError> {
        let Some(inner) = self.with_inner(|i| i.clone()) else {
            return Err(MGMTError::ClientDisabled);
        };

        let url = format!("http://{}.{}.svc.cluster.local:8080/external/backup_status", &name, &ns);
        let resp = match inner.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                self.metrics.backup_error();
                tracing::error!(error = %e, "Transport error while calling backup_status");
                return Err(MGMTError::BackupStatusError(e.to_string()));
            }
        };

        let status = resp.status();
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                self.metrics.backup_error();
                tracing::error!(error = %e, "Failed to read backup_status response body");
                return Err(MGMTError::BackupStatusError(e.to_string()));
            }
        };

        if !status.is_success() {
            self.metrics.backup_error();
            tracing::error!(%status, body = %body, "Non-success status from backup_status");
            return Err(MGMTError::BackupStatusError(format!("status {status}: {body}")));
        }

        let normalized = body.trim();
        let is_ok = if let Some(rest) = normalized.strip_prefix("Status:") {
            rest.trim() == "1"
        } else {
            normalized == "1"
        };

        self.metrics.backup_checked();
        tracing::debug!(raw_body = %body, result = is_ok, "Parsed backup status");
        Ok(is_ok)
    }

    #[instrument(skip(self), fields(name = %name, ns = %ns))]
    pub async fn list_backups(&self, name: &str, ns: &str) -> Result<Vec<String>, MGMTError> {
        let Some(inner) = self.with_inner(|i| i.clone()) else {
            // disabled at the moment
            return Err(MGMTError::ClientDisabled);
        };

        let url = format!("http://{}.{}.svc.cluster.local:8080/external/backup_list", &name, &ns);
        let resp = match inner.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                self.metrics.backup_list_error();
                tracing::error!(error = %e, "Transport error while calling backup_list");
                return Err(MGMTError::BackupListError(e.to_string()));
            }
        };

        let status = resp.status();
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                self.metrics.backup_list_error();
                tracing::error!(error = %e, "Failed to read backup_list response body");
                return Err(MGMTError::BackupListError(e.to_string()));
            }
        };

        if !status.is_success() {
            self.metrics.backup_list_error();
            tracing::error!(%status, body = %body, "Non-success status from backup_list");
            return Err(MGMTError::BackupListError(format!("status {status}: {body}")));
        }

        let backups: Vec<String> = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                self.metrics.backup_list_error();
                tracing::error!(error = %e, body = %body, "Failed to parse JSON for backup_list");
                return Err(MGMTError::BackupListError(e.to_string()));
            }
        };

        self.metrics.backup_list_ok();
        Ok(backups)
    }

    // Returns current enabled status, updating the gauge.
    /// This will refresh from the latest config and (re)build/disable the inner as needed.
    pub fn is_enabled(&self) -> bool {
        // Refresh from current AppConfig (may rebuild or disable inner)
        self.ensure_fresh();

        // Check current state without cloning the inner
        let enabled = self.inner.load().is_some();

        // Reflect in metrics
        self.metrics.set_enabled(enabled);

        enabled
    }

    fn ascii_to_hex(c: char) -> u8 {
        match c {
            '0'..='9' => c as u8 - b'0',
            'a'..='f' => c as u8 - b'a' + 10,
            'A'..='F' => c as u8 - b'A' + 10,
            _ => 0,
        }
    }

    fn hex_to_ascii(hex: u8) -> char {
        match hex {
            0..=9 => (hex + b'0') as char,
            10..=15 => (hex - 10 + b'a') as char,
            _ => '0',
        }
    }

    pub fn encode_server_id(guid: &str) -> String {
        const HASH_CODE_SIZE: usize = 8;
        let mut check_sum: u16 = 0;
        let mut no_of_one_bits: u8 = 0;
        let mut xor_byte: u8 = 0;
        let mut hex_array = [0u8; 16];
        let mut hash_code = [0u8; HASH_CODE_SIZE];
        let sequence = [0, 2, 4, 6, 9, 11, 14, 16, 19, 21, 24, 26, 28, 30, 32, 34];

        for i in 0..16 {
            hex_array[i] = (Self::ascii_to_hex(guid.chars().nth(sequence[i] + 1).unwrap()) << 4)
                + Self::ascii_to_hex(guid.chars().nth(sequence[i]).unwrap());
        }

        for &byte in &hex_array {
            xor_byte ^= byte;
            check_sum += byte as u16;
        }

        for &byte in &hex_array {
            for j in 0..8 {
                if (byte >> j) & 0x01 != 0 {
                    no_of_one_bits += 1;
                }
            }
        }

        hash_code[0] = Self::hex_to_ascii(xor_byte & 0x0f) as u8;
        hash_code[1] = Self::hex_to_ascii((xor_byte >> 4) & 0x0f) as u8;
        hash_code[2] = Self::hex_to_ascii((check_sum & 0x000f).try_into().unwrap()) as u8;
        hash_code[3] = Self::hex_to_ascii(((check_sum >> 4) & 0x000f).try_into().unwrap()) as u8;
        hash_code[4] = Self::hex_to_ascii(((check_sum >> 8) & 0x000f).try_into().unwrap()) as u8;
        hash_code[5] = Self::hex_to_ascii(((check_sum >> 12) & 0x000f).try_into().unwrap()) as u8;
        hash_code[6] = Self::hex_to_ascii(no_of_one_bits & 0x0f) as u8;
        hash_code[7] = Self::hex_to_ascii((no_of_one_bits >> 4) & 0x0f) as u8;

        hash_code.iter().map(|&b| b as char).collect()
    }
}

impl HotReloadClient for MGMTClient {
    type View = MGMTConfig;
    type Inner = MGMTClientInner;

    fn handle(&self) -> &ConfigHandle {
        &self.handle
    }
    fn inner(&self) -> &ArcSwapOption<Self::Inner> {
        &self.inner
    }

    fn build_inner(view: Self::View) -> Self::Inner {
        let http = ClientBuilder::new(HttpClient::new())
            .with(TracingMiddleware::default())
            .build();
        MGMTClientInner { cfg: view, http }
    }

    fn select_view(app: &AppConfig) -> &Self::View {
        MGMTSel::select(app)
    }

    // Avoid rebuilding if only unchanged
    fn inner_needs_rebuild(&self, cur: &Self::Inner, new_view: &Self::View) -> bool {
        &cur.cfg != new_view
    }
}

// The unit test module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_ascii() {
        // Test all digits from 0 to 9
        assert_eq!(MGMTClient::hex_to_ascii(0), '0');
        assert_eq!(MGMTClient::hex_to_ascii(1), '1');
        assert_eq!(MGMTClient::hex_to_ascii(5), '5');
        assert_eq!(MGMTClient::hex_to_ascii(9), '9');
        assert_eq!(MGMTClient::hex_to_ascii(10), 'a');
        assert_eq!(MGMTClient::hex_to_ascii(11), 'b');
        assert_eq!(MGMTClient::hex_to_ascii(14), 'e');
        assert_eq!(MGMTClient::hex_to_ascii(15), 'f');
        assert_eq!(MGMTClient::hex_to_ascii(16), '0');
        assert_eq!(MGMTClient::hex_to_ascii(u8::MAX), '0'); // Test the maximum u8 value
    }

    #[test]
    fn test_ascii_to_hex() {
        // Digits
        assert_eq!(MGMTClient::ascii_to_hex('0'), 0);
        assert_eq!(MGMTClient::ascii_to_hex('1'), 1);
        assert_eq!(MGMTClient::ascii_to_hex('5'), 5);
        assert_eq!(MGMTClient::ascii_to_hex('9'), 9);
        // Lowercase
        assert_eq!(MGMTClient::ascii_to_hex('a'), 10);
        assert_eq!(MGMTClient::ascii_to_hex('b'), 11);
        assert_eq!(MGMTClient::ascii_to_hex('e'), 14);
        assert_eq!(MGMTClient::ascii_to_hex('f'), 15);
        // Uppercase
        assert_eq!(MGMTClient::ascii_to_hex('A'), 10);
        assert_eq!(MGMTClient::ascii_to_hex('B'), 11);
        assert_eq!(MGMTClient::ascii_to_hex('E'), 14);
        assert_eq!(MGMTClient::ascii_to_hex('F'), 15);
    }

    #[test]
    fn test_ascii_to_hex_invalid_input() {
        // Test inputs that are not valid hex characters
        assert_eq!(MGMTClient::ascii_to_hex('g'), 0);
        assert_eq!(MGMTClient::ascii_to_hex('G'), 0);
        assert_eq!(MGMTClient::ascii_to_hex('*'), 0);
        assert_eq!(MGMTClient::ascii_to_hex(' '), 0);
    }

    #[test]
    fn test_encode_server_id() {
        // test valid encode
        assert_eq!(MGMTClient::encode_server_id("4f4819d7-a343-4f3e-a889-474f363bbe37"), "f59c9044");
        assert_eq!(MGMTClient::encode_server_id("92aeb7e6-3bb7-44ba-bf59-f8a632db4d82"), "81e78084");
    }
}

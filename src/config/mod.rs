//! Config module: loading + env-based default.
//! Reads CONFIG_PATH (YAML/TOML/JSON supported by `config`).

pub mod error;
pub mod metrics;
pub mod models;

use arc_swap::ArcSwap;
pub use error::ConfigError;
use notify::event::ModifyKind::Name;
use notify::{event::RenameMode, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::clients::log_clients;
pub use crate::metrics::Metrics; // your app-wide metrics aggregator
pub use metrics::ConfigMetrics;
pub use models::AppConfig;

pub const CONFIG_ENV: &str = "CONFIG_PATH";
pub type ConfigHandle = Arc<ArcSwap<AppConfig>>;

/// A tiny manager that holds the config handle, the watcher (to keep it alive),
/// and registered Prometheus counters.
pub struct ConfigRuntime {
    handle: ConfigHandle,
    _watcher: Option<RecommendedWatcher>,
    metrics: ConfigMetrics,
}

impl ConfigRuntime {
    pub fn init_and_watch(metrics: Arc<Metrics>) -> Result<Self, ConfigError> {
        // register our counters with the app registry
        let cfg_metrics = metrics
            .registrar()
            .register(|reg| ConfigMetrics::default().register(reg));

        // initial load (counts success/error)
        let initial = match load_from_env() {
            Ok(cfg) => {
                cfg_metrics.load();
                cfg
            }
            Err(e) => {
                cfg_metrics.error();
                tracing::error!("Config initial load failed: {e}");
                panic!("Failed to load config")
            }
        };

        let handle: ConfigHandle = Arc::new(ArcSwap::from_pointee(initial));

        // if CONFIG_PATH missing, nothing to watch
        let Ok(file_path_str) = env::var(CONFIG_ENV) else {
            return Ok(Self {
                handle,
                _watcher: None,
                metrics: cfg_metrics,
            });
        };
        let file_path = PathBuf::from(file_path_str);

        // kubelet updates the parent dir (..data swap)
        let dir_to_watch = file_path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| ConfigError::MissingConfigPathEnv(CONFIG_ENV.to_string()))?;

        let handle_for_cb = Arc::clone(&handle);
        let file_for_cb = file_path.clone();

        let metrics_for_reload = cfg_metrics.clone();
        let metrics_for_watcher = cfg_metrics.clone();

        let try_reload = move || {
            // file can be briefly missing during symlink swap
            if fs::read(&file_for_cb).is_err() {
                return;
            }
            match load_from_path(&file_for_cb) {
                Ok(new_cfg) => {
                    handle_for_cb.store(Arc::new(new_cfg));
                    tracing::info!("config hot-reloaded from {:?}", file_for_cb);
                    log_clients(handle_for_cb.load_full());
                    metrics_for_reload.load(); // count successful reload
                }
                Err(err) => {
                    tracing::warn!("config reload failed: {err}");
                    metrics_for_reload.error(); // count reload error
                }
            }
        };

        // watch parent directory; react on the same event you had before
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if let EventKind::Modify(Name(RenameMode::Both)) = event.kind {
                        // don’t filter by paths; kubelet often reports ..data
                        try_reload();
                    }
                }
                Err(err) => {
                    tracing::warn!("config watcher error: {err}");
                    metrics_for_watcher.error(); // count watcher error
                }
            }
        })?;
        watcher.watch(&dir_to_watch, RecursiveMode::NonRecursive)?;

        Ok(Self {
            handle,
            _watcher: Some(watcher),
            metrics: cfg_metrics,
        })
    }

    /// Lock-free snapshot for readers.
    pub fn snapshot(&self) -> Arc<AppConfig> {
        self.handle.load_full()
    }

    /// Expose the handle if some code already expects `ConfigHandle`.
    pub fn handle(&self) -> &ConfigHandle {
        &self.handle
    }

    /// Access metrics (e.g., tests).
    pub fn metrics(&self) -> &ConfigMetrics {
        &self.metrics
    }
}

/// Load config from a file.
pub fn load_from_path(path: impl Into<PathBuf>) -> Result<AppConfig, ConfigError> {
    let path = path.into();
    let builder = config::Config::builder().add_source(config::File::from(path.clone()));
    tracing::info!("Loading config from {:?}", path);

    let built = builder.build()?;
    let res = built.try_deserialize();
    match res {
        Ok(cfg) => Ok(cfg),
        Err(err) => {
            tracing::warn!("Failed to deserialize config: {}", err);
            Err(ConfigError::ConfigBuild(err))
        }
    }
}

/// Load config from environment variables.
pub fn load_from_env() -> Result<AppConfig, ConfigError> {
    tracing::info!("Loading config from env var {}", CONFIG_ENV);
    let path = env::var(CONFIG_ENV).map_err(|_| ConfigError::MissingConfigPathEnv(CONFIG_ENV.into()))?;
    load_from_path(path)
}

/// Take a snapshot of the current config.
pub fn snapshot(handle: &ConfigHandle) -> Arc<AppConfig> {
    handle.load_full()
}

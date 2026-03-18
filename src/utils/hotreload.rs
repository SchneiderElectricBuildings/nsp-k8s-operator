use arc_swap::ArcSwapOption;
use std::sync::Arc;

use crate::config::{AppConfig, ConfigHandle};

/// Minimal per-client view requirements (a slice of AppConfig the client cares about)
pub trait ConfigView: Clone + PartialEq + Send + Sync + 'static {
    fn enabled(&self) -> bool;
}

/// Selects the view from AppConfig for a given client
pub trait SelectView {
    type View: ConfigView;
    fn select(app: &AppConfig) -> &Self::View;
}

/// Trait that gives you hot-reload behavior
pub trait HotReloadClient: Send + Sync + 'static {
    type View: ConfigView;
    type Inner: Clone + Send + Sync + 'static;

    /// Where to read the latest config from
    fn handle(&self) -> &ConfigHandle;
    /// Where we keep the current, ready-to-use inner (or None when disabled)
    fn inner(&self) -> &ArcSwapOption<Self::Inner>;

    /// Build a fresh Inner from current view
    fn build_inner(view: Self::View) -> Self::Inner;
    /// How to get the client-specific view from AppConfig
    fn select_view(app: &AppConfig) -> &Self::View;

    /// Ensure inner matches latest config; disable if not enabled.
    fn ensure_fresh(&self) {
        let app = self.handle().load_full();
        let view = Self::select_view(&app).clone();

        if !view.enabled() {
            self.inner().store(None);
            return;
        }

        // If stored inner is based on a different view, rebuild.
        let needs_rebuild = match self.inner().load().as_ref() {
            None => true,
            Some(cur) => self.inner_needs_rebuild(cur, &view),
        };

        if needs_rebuild {
            let new_inner = Self::build_inner(view);
            self.inner().store(Some(Arc::new(new_inner)));
        }
    }

    /// Compare current inner to the new view (default: always rebuild).
    /// Override if your Inner stores the view and you can compare cheaply.
    fn inner_needs_rebuild(&self, _cur: &Self::Inner, _new_view: &Self::View) -> bool {
        true
    }

    /// Get a stable inner snapshot if enabled (refreshes first).
    fn with_inner<T>(&self, f: impl FnOnce(&Self::Inner) -> T) -> Option<T> {
        self.ensure_fresh();
        self.inner().load().as_ref().map(|i| f(i))
    }
}

// src/health.rs
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct Component {
    pub last_tick: Option<Instant>,
    pub last_ok: Option<Instant>,
    pub last_error: Option<String>,
    pub disabled: bool,
}

#[derive(Debug, Serialize)]
pub struct ComponentView {
    pub disabled: bool,
    pub tick_age_ms: Option<u128>,
    pub ok_age_ms: Option<u128>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthView {
    pub controller_started: bool,
    pub components: HashMap<String, ComponentView>,
}

#[derive(Clone, Default)]
pub struct Health {
    controller_started: Arc<AtomicBool>,
    inner: Arc<RwLock<HashMap<&'static str, Component>>>,
}

impl Health {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn controller_started(&self) -> bool {
        self.controller_started.load(Ordering::Acquire)
    }

    pub fn mark_controller_started(&self) {
        self.controller_started.store(true, Ordering::Release);
    }

    pub async fn tick(&self, name: &'static str) {
        let mut g = self.inner.write().await;
        let e = g.entry(name).or_default();
        e.disabled = false;
        e.last_tick = Some(Instant::now());
    }

    pub async fn ok(&self, name: &'static str) {
        let mut g = self.inner.write().await;
        let e = g.entry(name).or_default();
        e.disabled = false;
        e.last_ok = Some(Instant::now());
        e.last_error = None;
    }

    pub async fn error(&self, name: &'static str, err: impl ToString) {
        let mut g = self.inner.write().await;
        let e = g.entry(name).or_default();
        e.disabled = false;
        e.last_error = Some(err.to_string());
    }

    pub async fn disabled(&self, name: &'static str) {
        let mut g = self.inner.write().await;
        let e = g.entry(name).or_default();
        e.disabled = true;
        e.last_tick = None;
        e.last_ok = None;
        e.last_error = None;
    }

    pub async fn is_alive(&self, name: &'static str, max_staleness: Duration) -> bool {
        let g = self.inner.read().await;
        match g.get(name) {
            Some(c) if c.disabled => true,
            Some(c) => c.last_tick.map(|t| t.elapsed() <= max_staleness).unwrap_or(false),
            None => false,
        }
    }

    pub async fn is_ok_recent(&self, name: &'static str, max_age: Duration) -> bool {
        let g = self.inner.read().await;
        match g.get(name) {
            Some(c) if c.disabled => true,
            Some(c) => c.last_ok.map(|t| t.elapsed() <= max_age).unwrap_or(false),
            None => false,
        }
    }

    pub async fn view(&self) -> HealthView {
        let snap = self.inner.read().await.clone();
        let now = Instant::now();

        let components = snap
            .into_iter()
            .map(|(name, c)| {
                let tick_age_ms = c.last_tick.map(|t| now.duration_since(t).as_millis());
                let ok_age_ms = c.last_ok.map(|t| now.duration_since(t).as_millis());
                (
                    name.to_string(),
                    ComponentView {
                        disabled: c.disabled,
                        tick_age_ms,
                        ok_age_ms,
                        last_error: c.last_error,
                    },
                )
            })
            .collect();

        HealthView {
            controller_started: self.controller_started(),
            components,
        }
    }
}

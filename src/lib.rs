pub const OPERATOR_NAME: &str = env!("CARGO_PKG_NAME");

/// Expose all controller components used by main
pub mod controller;

pub use crate::controller::*;

/// Metrics
pub mod metrics;
pub use metrics::telemetry::{init_tracing, shutdown_tracing, Telemetry};
pub use metrics::Metrics;

pub mod clients;
pub mod config;
pub mod crd;
pub mod eboserver;
pub mod resources;
pub mod utils;

pub mod events;

pub use crate::events::Level;

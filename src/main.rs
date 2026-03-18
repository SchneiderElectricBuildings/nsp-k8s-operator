use actix_web::{get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder};
use controller::Metrics;
use opentelemetry::trace::TracerProvider;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;
use tracing::subscriber::Subscriber;
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

pub use controller::config::snapshot;
pub use controller::metrics::telemetry;
pub use controller::{self, State};

#[get("/metrics")]
async fn metrics_enpoint(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let metrics = c.metrics();
    HttpResponse::Ok()
        .content_type("application/openmetrics-text; version=1.0.0; charset=utf-8")
        .body(metrics)
}

#[get("/livez")]
pub async fn livez(c: Data<State>) -> impl Responder {
    let h = c.health();
    let ok = h.controller_started() && h.is_alive("controller", Duration::from_secs(20)).await;

    if ok {
        HttpResponse::Ok().json(json!({"status":"live"}))
    } else {
        HttpResponse::ServiceUnavailable().json(json!({
            "status":"not_live",
            "health": h.view().await
        }))
    }
}

#[get("/readyz")]
pub async fn readyz(c: Data<State>) -> impl Responder {
    let h = c.health();

    let controller_ok = h.controller_started() && h.is_alive("controller", Duration::from_secs(20)).await;

    let ok = controller_ok;

    if ok {
        HttpResponse::Ok().json(json!({
            "status":"ready"
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(json!({
            "status":"not_ready",
            "health": h.view().await
        }))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let telemetry = telemetry::init_tracing()?;

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();
    // its env_filter + , + some specific overrides for noisy dependencies
    let otel_filter = EnvFilter::new(format!("{env_filter},kube=info,hyper=warn,h2=warn,rustls=warn,tower=warn"));

    let fmt_layer = tracing_subscriber::fmt::layer().compact().json();

    let subscriber: Box<dyn Subscriber + Send + Sync> = match &telemetry {
        telemetry::Telemetry::Enabled(provider) => {
            let tracer = provider.tracer("nsp-k8s-operator");
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

            Box::new(
                Registry::default()
                    .with(fmt_layer.with_filter(env_filter))
                    .with(otel_layer.with_filter(otel_filter)),
            )
        }
        telemetry::Telemetry::Disabled => Box::new(Registry::default().with(fmt_layer.with_filter(env_filter))),
    };

    subscriber.init();

    info!("Starting nsp-k8s-operator...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    info!("Build: {}", env!("GIT_SHA"));
    info!("Build time: {}", env!("BUILD_TIME"));

    let metrics_ctx = Arc::new(Metrics::default());
    let state = State::new(metrics_ctx);
    let controller = controller::run(state.clone());

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(state.clone()))
            .wrap(
                middleware::Logger::default()
                    .exclude("/readyz")
                    .exclude("/livez")
                    .exclude("/metrics"),
            )
            .service(readyz)
            .service(livez)
            .service(metrics_enpoint)
    })
    .bind("0.0.0.0:8080")?
    .shutdown_timeout(5);

    let result = tokio::join!(controller, server.run()).1;

    telemetry::shutdown_tracing(telemetry);

    result?;
    Ok(())
}

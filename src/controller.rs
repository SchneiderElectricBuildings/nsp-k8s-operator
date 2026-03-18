use crate::metrics::telemetry;
use crate::utils::hash_ring::{spawn_membership_heartbeat, spawn_ring_manager, RingHandle};
use crate::utils::leader_election;
use futures::StreamExt;
use kube::runtime::events::Reporter;
use kube::runtime::watcher;
use kube::ResourceExt;
use kube::{
    api::{Api, ListParams},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event as Finalizer},
    },
};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::time::Duration;
use tracing::Instrument;
use tracing::{error, info, info_span, instrument, warn};

use crate::clients::{log_clients, mgmt::MGMTClient, prometheus::PrometheusClient};
use crate::config::{snapshot, ConfigHandle, ConfigRuntime};
use crate::crd::v1alpha6::EboServer;
use crate::eboserver::EboServerExt;
use crate::utils::health::Health;
use crate::Metrics;

pub use crate::OPERATOR_NAME;

use crate::eboserver::EboServerError;
use crate::eboserver::Result;

pub static EBO_FINALIZER: &str = "nsp-k8s-operator.digbld.em.se.com";

pub struct Ctx {
    pub client: Client,
    pub metrics: Arc<Metrics>,
    pub me: String,
    pub config: ConfigHandle,
    pub is_leader: Arc<AtomicBool>,
    pub ring: RingHandle,
    pub mgmt_client: Arc<MGMTClient>,
    pub prometheus_client: Arc<PrometheusClient>,
    pub reporter: Reporter,
    pub health: Health,
}

pub async fn run(state: State) {
    let config = snapshot(&state.config_handle());

    let me = std::env::var("POD_NAME").unwrap_or_else(|_| "local-dev".into());
    let ns = std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "default".into());

    let label_key = "app".to_string();
    let label_value = "nsp-k8s-operator-member".to_string();

    let h = state.health();
    h.mark_controller_started();
    h.tick("controller").await;

    let hb = tokio::spawn({
        let h = h.clone();
        async move {
            let mut i = tokio::time::interval(Duration::from_secs(5));
            loop {
                i.tick().await;
                h.tick("controller").await;
            }
        }
    });

    let mgmt_client = MGMTClient::new(state.config_handle(), Arc::clone(&state.metrics));
    let prometheus_client = PrometheusClient::new(state.config_handle(), Arc::clone(&state.metrics));

    log_clients(config.clone());

    let client: Client = Client::try_default().await.expect("missing kubeconfig");

    let watched_namespaces = config.namespace_scope.clone();
    let ebo_api: Api<EboServer> = if let Some(watch_ns) = watched_namespaces {
        info!("Watching namespace: {watch_ns}");
        Api::namespaced(client.clone(), &watch_ns)
    } else {
        info!("Watching all namespaces");
        Api::all(client.clone())
    };

    if let Err(e) = ebo_api.list(&ListParams::default().limit(1)).await {
        error!(?e, "CRD is not queryable");
        std::process::exit(1);
    }

    let context: Arc<Ctx> = state.to_context(client, mgmt_client, prometheus_client).await;

    leader_election::spawn_leader_manager(
        context.client.clone(),
        ns.clone(),
        me.clone(),
        Arc::clone(&context.is_leader),
    );

    spawn_ring_manager(
        context.client.clone(),
        ns.clone(),
        context.ring.clone(),
        label_key.clone(),
        label_value.clone(),
        128,
        Duration::from_secs(5),
        Duration::from_secs(5),
    );

    spawn_membership_heartbeat(
        context.client.clone(),
        ns.clone(),
        me.clone(),
        "lease-".into(),
        Duration::from_secs(30),
        Duration::from_secs(10),
        label_key,
        label_value,
    );

    info!("Controller loop starting");

    Controller::new(ebo_api, watcher::Config::default())
        .shutdown_on_signal()
        .run(reconcile, on_error, context)
        .for_each(|result| async move {
            if let Err(e) = result {
                error!(?e, "Reconciliation error");
            }
        })
        .await;

    hb.abort();
}

#[instrument(
    name = "controller.reconcile",
    skip(ctx, ebo_server),
    fields(
        trace_id = %telemetry::get_trace_id(),
        ebo_name = %ebo_server.name_any(),
        namespace = %ebo_server.namespace().unwrap_or_default(),
        uid = %ebo_server.metadata.uid.clone().unwrap_or_default(),
        ring_owner = %ctx.me
    )
)]
async fn reconcile(ebo_server: Arc<EboServer>, ctx: Arc<Ctx>) -> Result<Action, EboServerError> {
    ctx.health.tick("reconcile").await;
    let ns = ebo_server.namespace().unwrap();
    let ebos: Api<EboServer> = Api::namespaced(ctx.client.clone(), &ns);
    let uuid = ebo_server.metadata.uid.as_ref().unwrap();

    if !ctx.ring.is_owner(&ctx.me, uuid) && ctx.me != "local-dev" {
        return Ok(Action::requeue(Duration::from_secs(60)));
    }

    let span = info_span!("finalizer");

    let res = finalizer(&ebos, EBO_FINALIZER, ebo_server, |event| async {
        match event {
            Finalizer::Apply(ebo) => ebo.reconcile(ctx.clone()).await,
            Finalizer::Cleanup(ebo) => ebo.cleanup(ctx.clone()).await,
        }
    })
    .instrument(span)
    .await
    .map_err(|e| EboServerError::FinalizerError(Box::new(e)))?;

    ctx.health.ok("reconcile").await;

    Ok(res)
}

#[instrument(
    name = "controller.reconcile_error",
    skip(error, context, ebo_server),
    fields(
        ebo = %ebo_server.name_any(),
        ns = %ebo_server.namespace().unwrap_or_default(),
    )
)]
fn on_error(ebo_server: Arc<EboServer>, error: &EboServerError, context: Arc<Ctx>) -> Action {
    context.metrics.reconcile.set_failure(&ebo_server, error);

    let h = context.health.clone();
    let msg = format!("{error:?}");
    tokio::spawn(async move {
        h.error("reconcile", msg).await;
    });

    Action::requeue(Duration::from_secs(5))
}

/// State shared between the controller and the web server
#[derive(Clone)]
pub struct State {
    /// Metrics
    metrics: Arc<Metrics>,
    /// Config
    config_runtime: Arc<ConfigRuntime>,
    /// Health
    health: Health,
    /// Leader
    is_leader: Arc<AtomicBool>,
}

/// State wrapper around the controller outputs for the web server
impl State {
    /// Build a State with a live config watcher.
    pub fn new(metrics: Arc<Metrics>) -> Self {
        let runtime = ConfigRuntime::init_and_watch(Arc::clone(&metrics)).unwrap_or_else(|e| {
            tracing::error!("Failed to init config: {e}");
            std::process::exit(1);
        });

        Self {
            metrics,
            config_runtime: Arc::new(runtime),
            health: Health::new(),
            is_leader: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Metrics getter
    pub fn metrics(&self) -> String {
        let mut buf = String::new();
        self.metrics.with_registry(|reg| {
            let _ = prometheus_client::encoding::text::encode(&mut buf, reg);
        });
        buf
    }

    /// Expose a clone of the health tracker
    pub fn health(&self) -> Health {
        self.health.clone()
    }

    /// Expose is leader
    pub fn is_leader(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.is_leader)
    }

    /// Expose a clone of the hot-reload handle
    pub fn config_handle(&self) -> ConfigHandle {
        self.config_runtime.handle().clone()
    }

    // Create a Controller Context that can update State
    #[allow(clippy::too_many_arguments)]
    pub async fn to_context(
        &self, client: Client, mgmt_client: MGMTClient, prometheus_client: PrometheusClient,
    ) -> Arc<Ctx> {
        Arc::new(Ctx {
            client,
            metrics: Arc::clone(&self.metrics),
            me: std::env::var("POD_NAME").unwrap_or_else(|_| "local-dev".into()),
            config: self.config_handle(),
            is_leader: Arc::new(AtomicBool::new(false)),
            ring: RingHandle::new(),
            mgmt_client: Arc::new(mgmt_client),
            prometheus_client: Arc::new(prometheus_client),
            reporter: Reporter {
                controller: OPERATOR_NAME.to_owned(),
                instance: std::env::var("HOSTNAME").ok(),
            },
            health: self.health.clone(),
        })
    }
}

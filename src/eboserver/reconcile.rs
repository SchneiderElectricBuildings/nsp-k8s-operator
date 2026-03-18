use std::sync::Arc;

use kube::{runtime::controller::Action, ResourceExt};

use crate::config::models::EboConfig;
use tracing::{info_span, instrument, Instrument};

use crate::eboserver::manager::EboServerManager;

use crate::clients::Clients;
use crate::config::snapshot;
use crate::controller::Ctx;
use crate::eboserver::fsm;
use crate::eboserver::handles::Handles;
use crate::eboserver::EboServerError;
use crate::events::definitions::*;

use crate::resources::ensure_apply;
use crate::resources::*;

// Pull in the CRD types
use crate::crd::v1alpha6::{EboServer, EboServerPhase};
use async_trait::async_trait;

#[async_trait]
pub trait EboServerExt {
    async fn create_resources(&self, h: &Handles, ctx: &Ctx) -> Result<(), EboServerError>;
    fn get_system_id(&self) -> Option<String>;
    fn get_system_instance_id(&self) -> Option<String>;
    fn get_server_id(&self) -> Option<String>;
    fn get_server_instance_id(&self) -> Option<String>;
    fn get_nsp_machine_id(&self, cfg: &EboConfig) -> Option<String>;
    fn get_deployed_by(&self) -> String;
    async fn reconcile(&self, ctx: Arc<Ctx>) -> Result<Action, EboServerError>;
    async fn cleanup(&self, ctx: Arc<Ctx>) -> Result<Action, EboServerError>;
}

// ---- Implementation of controller logic on the CRD type ----

#[async_trait]
impl EboServerExt for EboServer {
    #[instrument(
        skip(self, h, ctx),
        fields(
            ebo = %self.name_any(),
            ns = %self.namespace().unwrap_or_default()
        )
    )]
    async fn create_resources(&self, h: &Handles, ctx: &Ctx) -> Result<(), EboServerError> {
        let config = snapshot(&ctx.config);
        let istio_namespace = config.ebo.ingress.istio_gateway.istio_namespace.as_str();

        ensure_apply(&h.services, &service::gen_svc(self)).await?;
        ensure_apply(&h.cronjobs, &cronjob::gen_cronjob(self, &config.ebo)).await?;
        ensure_apply(&h.pvcs, &pvc::gen_pvc(self, "backup")).await?;
        ensure_apply(&h.pvcs, &pvc::gen_pvc(self, "data")).await?;

        if config.ebo.ingress.controller.enabled {
            ensure_apply(&h.ingresses, &ingress::gen_ingress(self, &config.ebo)).await?;
        } else {
            delete_if_exists(&h.ingresses, self.name_any().as_str()).await?;
        }

        if config.ebo.ingress.istio_gateway.enabled {
            ensure_apply(&h.httproutes, &httproute::gen_httproute(self, &config.ebo)).await?;
            ensure_apply(&h.xlistenersets, &xlistenerset::generate_xlistener_set(self, &config.ebo)).await?;
            ensure_apply(&h.certificates, &certificate::gen_certificate(self, &config.ebo)).await?;
        } else if h.namespaces.get_opt(istio_namespace).await?.is_some() {
            delete_if_exists(&h.httproutes, self.name_any().as_str()).await?;
            delete_if_exists(
                &h.certificates,
                &format!(
                    "{}-{}-eboserver-tls-cert",
                    self.name_any(),
                    self.namespace().unwrap_or_default()
                ),
            )
            .await?;
            delete_if_exists(
                &h.xlistenersets,
                &format!("{}-{}-listener", self.name_any(), self.namespace().unwrap_or_default()),
            )
            .await?;
        }

        Ok(())
    }

    fn get_system_id(&self) -> Option<String> {
        self.metadata
            .labels
            .as_ref()
            .and_then(|labels| {
                labels
                    .get("eboserver.digbld.em.se.com/ebo-system-id")
                    .or_else(|| labels.get("projectId"))
            })
            .map(|s| s.to_string())
    }

    fn get_system_instance_id(&self) -> Option<String> {
        self.metadata
            .labels
            .as_ref()
            .and_then(|labels| {
                labels
                    .get("eboserver.digbld.em.se.com/ebo-system-instance-id")
                    .or_else(|| labels.get("projectId"))
            })
            .map(|s| s.to_string())
    }

    fn get_server_id(&self) -> Option<String> {
        self.metadata
            .labels
            .as_ref()
            .and_then(|labels| {
                labels
                    .get("eboserver.digbld.em.se.com/ebo-server-id")
                    .or_else(|| labels.get("eboserverId"))
                    .or_else(|| labels.get("systemId"))
            })
            .map(|s| s.to_string())
    }

    fn get_server_instance_id(&self) -> Option<String> {
        self.metadata
            .labels
            .as_ref()
            .and_then(|labels| {
                labels
                    .get("eboserver.digbld.em.se.com/ebo-server-instance-id")
                    .or_else(|| labels.get("eboserverId"))
                    .or_else(|| labels.get("systemId"))
            })
            .map(|s| s.to_string())
    }

    fn get_nsp_machine_id(&self, cfg: &EboConfig) -> Option<String> {
        //only set nsp-machine-id if enabled
        if cfg.nsp_machine_id {
            self.metadata
                .labels
                .as_ref()
                .and_then(|labels| {
                    labels
                        .get("eboserver.digbld.em.se.com/nsp-machine-id")
                        .or_else(|| labels.get("nspMachineId"))
                })
                .map(|s| s.to_string())
                .or_else(|| self.get_server_id())
        } else {
            None
        }
    }

    fn get_deployed_by(&self) -> String {
        self.metadata
            .labels
            .as_ref()
            .and_then(|labels| labels.get("deployedBy"))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "<unknown>".to_string())
    }

    #[instrument(
        skip(self, ctx),
        parent = None,
        fields(
            ebo = %self.name_any(),
            ns = %self.namespace().unwrap_or_default(),
            deployed_by = %self.get_deployed_by()
        )
    )]
    async fn reconcile(&self, ctx: Arc<Ctx>) -> Result<Action, EboServerError> {
        let ns = self.metadata.namespace.as_ref().unwrap();
        let _timer = ctx.metrics.reconcile.count_and_measure(None);
        let clients = Clients::snapshot(&ctx);
        let config = snapshot(&ctx.config);
        let istio_ns = config.ebo.ingress.istio_gateway.istio_namespace.as_str();
        let h = Handles::new(ctx.client.clone(), ns, istio_ns);
        let mut mgr = EboServerManager::new(self, ctx.clone());

        self.create_resources(&h, &ctx).await?;

        if mgr.phase().is_none() {
            tracing::info!(
                target: "controller::eboserver::phase",
                prev_phase = "None",
                next_phase = ?EboServerPhase::Starting,
                "phase transition"
            );
            mgr.publish_event(OperatorEvent::EboCreated).await?;
            mgr.move_to(EboServerPhase::Starting).await?;
            return Ok(Action::requeue(fsm::SHORT_REQUEUE));
        }

        // Determine current phase safely
        let phase = mgr.phase().as_ref().unwrap().clone();

        // Flags passed into the FSM (pure knobs, no I/O)
        let flags = fsm::types::Flags {
            initial_password_enabled: config.ebo.initial_password.enabled,
        };

        // 1) Observe
        let observed = fsm::observe::observe(self, &h, &ctx, &clients, &mut mgr).await?;

        // 2) Decide (dispatch to per-phase handler)
        let decision = fsm::step(phase, &observed, flags);

        // 3) Act (apply effects and extract forced transition if any)
        let forced = fsm::act::apply(self, &h, &ctx, &clients, &mut mgr, &observed, &decision).await?;

        // 4) Apply transition recorded in decision (or forced one if present)
        if let Some(t) = forced {
            fsm::act::apply_transition(&mut mgr, &t).await?;
        } else {
            fsm::act::apply_transition(&mut mgr, &decision.transition).await?;
        }

        Ok(Action::requeue(decision.requeue))
    }

    #[instrument(
        skip(self, ctx),
        parent = None,
        fields(
            ebo = %self.name_any(),
            ns = %self.namespace().unwrap_or_default(),
            deployed_by = %self.get_deployed_by()
        )
    )]
    async fn cleanup(&self, ctx: Arc<Ctx>) -> Result<Action, EboServerError> {
        let config = snapshot(&ctx.config);
        let istio_enabled = config.ebo.ingress.istio_gateway.enabled;
        let ebo_mgr = EboServerManager::new(self, ctx.clone());
        let ns = self.metadata.namespace.as_ref().unwrap();
        let istio_ns = config.ebo.ingress.istio_gateway.istio_namespace.as_str();
        let h = Handles::new(ctx.client.clone(), ns, istio_ns);

        let span_event = info_span!("cleanup.publish_deleting");
        async {
            ebo_mgr.publish_event(OperatorEvent::EboDeleting).await?;
            Ok::<(), EboServerError>(())
        }
        .instrument(span_event)
        .await?;

        if istio_enabled {
            let span_cleanup = info_span!("cleanup.istio_resources");
            async {
                delete_if_exists(&h.httproutes, self.name_any().as_str()).await?;
                delete_if_exists(
                    &h.certificates,
                    &format!(
                        "{}-{}-eboserver-tls-cert",
                        self.name_any(),
                        self.namespace().unwrap_or_default()
                    ),
                )
                .await?;
                delete_if_exists(
                    &h.xlistenersets,
                    &format!("{}-{}-listener", self.name_any(), self.namespace().unwrap_or_default()),
                )
                .await?;
                Ok::<(), EboServerError>(())
            }
            .instrument(span_cleanup)
            .await?;
        }
        Ok(Action::await_change())
    }
}

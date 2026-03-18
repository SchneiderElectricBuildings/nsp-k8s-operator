use croner::Cron;
use k8s_openapi::api::batch::v1::{CronJob, CronJobSpec, JobSpec, JobTemplateSpec};
use k8s_openapi::api::core::v1::{Container, LocalObjectReference, PodSpec, PodTemplateSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::Resource;
use std::collections::BTreeMap;
use std::str::FromStr;

use crate::config::models::EboConfig;
use crate::crd::v1alpha6::EboServer;
use crate::eboserver::EboServerExt;
use crate::resources::util::{gen_labels, set_hash_annotation, spec_hash};

pub fn gen_cronjob(ebo_server: &EboServer, config: &EboConfig) -> CronJob {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
    let default_schedule = "0 12 * * 1".to_string();
    let schedule_str = ebo_server.spec.backup_schedule.as_ref().unwrap_or(&default_schedule);
    let utils_registry = config.utils.registry.clone();
    let utils_image = config.utils.image.clone();
    let pull_secret: Option<Vec<LocalObjectReference>> = config
        .pull_secret
        .clone()
        .map(|s| vec![LocalObjectReference { name: s }]);
    let backups = config.backups;

    let schedule = Cron::from_str(schedule_str).unwrap_or_else(|_| {
        tracing::error!("Invalid cron schedule: {}", schedule_str);
        Cron::from_str("0 12 * * 1").unwrap()
    });

    let mut cj = CronJob {
        metadata: ObjectMeta {
            name: Some(format!("{name}-backup")),
            labels: Some(gen_labels(name, server_id.clone(), system_id.clone())),
            owner_references: Some(vec![owner_ref.clone()]),
            ..ObjectMeta::default()
        },
        spec: Some(CronJobSpec {
            schedule: schedule.pattern.to_string(),
            job_template: JobTemplateSpec {
                metadata: Some(ObjectMeta {
                    name: Some(format!("{name}-backup")),
                    labels: Some(gen_labels(name, server_id, system_id)),
                    owner_references: Some(vec![owner_ref]),
                    ..ObjectMeta::default()
                }),
                spec: Some(JobSpec {
                    template: PodTemplateSpec {
                        metadata: Some(ObjectMeta {
                            annotations: Some(BTreeMap::from([
                                ("linkerd.io/inject".to_owned(), "enabled".to_owned()),
                                ("config.linkerd.io/proxy-await".to_owned(), "enabled".to_owned()),
                                ("config.linkerd.io/default-inbound-policy".to_owned(), "deny".to_owned()),
                            ])),
                            ..ObjectMeta::default()
                        }),
                        spec: Some(PodSpec {
                            node_selector: config.node_selector.clone(),
                            tolerations: config.tolerations.clone(),
                            image_pull_secrets: pull_secret,
                            containers: vec![Container {
                                name: "nsp-servers".to_owned(),
                                image: Some(format!("{utils_registry}{utils_image}")),
                                command: Some(vec![
                                    "/bin/sh".to_string(),
                                    "-c".to_string(),
                                    format!(
                                        r#"DATE="$(date +'%d%m%y-%H%M')"; \
                                        curl -sSsf -X GET http://{name}:8080/external/backup?name={name}_${{DATE}}\&backups={backups} -vvv; \
                                        curl -sSsf -X POST http://localhost:4191/shutdown -vvv"#
                                    ),
                                ]),
                                ..Container::default()
                            }],
                            restart_policy: Some("OnFailure".to_owned()),
                            ..PodSpec::default()
                        }),
                    },
                    backoff_limit: Some(3),
                    ttl_seconds_after_finished: Some(180),
                    ..JobSpec::default()
                }),
            },
            ..CronJobSpec::default()
        }),
        ..CronJob::default()
    };

    if let Some(spec) = cj.spec.as_ref() {
        let h = spec_hash(spec);
        set_hash_annotation(&mut cj.metadata, h);
    }
    cj
}

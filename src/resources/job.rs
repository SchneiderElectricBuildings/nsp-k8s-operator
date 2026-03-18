use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::{
    Container, LocalObjectReference, PersistentVolumeClaimVolumeSource, PodSecurityContext, PodSpec, PodTemplateSpec,
    Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::Resource;
use std::collections::BTreeMap;

use crate::config::models::EboConfig;
use crate::crd::v1alpha6::EboServer;
use crate::eboserver::EboServerExt;
use crate::resources::util::{gen_labels, set_hash_annotation, spec_hash};

pub fn gen_upgrade_job(ebo_server: &EboServer, job_name: String, config: &EboConfig) -> Job {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
    let image = ebo_server.status.as_ref().unwrap().pod_image.clone().unwrap();
    let pull_secret: Option<Vec<LocalObjectReference>> = config
        .pull_secret
        .clone()
        .map(|s| vec![LocalObjectReference { name: s }]);

    let mut job = Job {
        metadata: ObjectMeta {
            name: Some(job_name),
            labels: Some(gen_labels(name, server_id, system_id)),
            owner_references: Some(vec![owner_ref]),
            annotations: None,
            ..ObjectMeta::default()
        },
        spec: Some(JobSpec {
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    annotations: Some(BTreeMap::from([
                        ("linkerd.io/inject".to_owned(), "disabled".to_owned()),
                        ("config.linkerd.io/proxy-await".to_owned(), "disabled".to_owned()),
                    ])),
                    ..ObjectMeta::default()
                }),
                spec: Some(PodSpec {
                    security_context: Some(PodSecurityContext {
                        fs_group: Some(60606),
                        ..PodSecurityContext::default()
                    }),
                    volumes: Some(vec![Volume {
                        name: "data".to_owned(),
                        persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                            claim_name: format!("{name}-data"),
                            ..PersistentVolumeClaimVolumeSource::default()
                        }),
                        ..Volume::default()
                    }]),
                    node_selector: config.node_selector.clone(),
                    tolerations: config.tolerations.clone(),
                    image_pull_secrets: pull_secret,
                    containers: vec![Container {
                        name: "nsp-servers".to_owned(),
                        image: Some(image),
                        volume_mounts: Some(vec![VolumeMount {
                            name: "data".to_owned(),
                            mount_path: "/var/sbo".to_owned(),
                            ..VolumeMount::default()
                        }]),
                        command: Some(vec!["/opt/sbo/bin/prepare-upgrade".to_owned()]),
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
        ..Job::default()
    };

    if let Some(spec) = job.spec.as_ref() {
        let h = spec_hash(spec);
        set_hash_annotation(&mut job.metadata, h);
    }
    job
}

pub fn gen_reset_password_job(ebo_server: &EboServer, job_name: String, config: &EboConfig) -> Job {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
    let image = ebo_server.status.as_ref().unwrap().pod_image.clone().unwrap();
    let pull_secret: Option<Vec<LocalObjectReference>> = config
        .pull_secret
        .clone()
        .map(|s| vec![LocalObjectReference { name: s }]);

    let mut job = Job {
        metadata: ObjectMeta {
            name: Some(job_name),
            labels: Some(gen_labels(name, server_id, system_id)),
            owner_references: Some(vec![owner_ref]),
            annotations: None,
            ..ObjectMeta::default()
        },
        spec: Some(JobSpec {
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    annotations: Some(BTreeMap::from([
                        ("linkerd.io/inject".to_owned(), "disabled".to_owned()),
                        ("config.linkerd.io/proxy-await".to_owned(), "disabled".to_owned()),
                    ])),
                    ..ObjectMeta::default()
                }),
                spec: Some(PodSpec {
                    security_context: Some(PodSecurityContext {
                        fs_group: Some(60606),
                        ..PodSecurityContext::default()
                    }),
                    volumes: Some(vec![Volume {
                        name: "data".to_owned(),
                        persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                            claim_name: format!("{name}-data"),
                            ..PersistentVolumeClaimVolumeSource::default()
                        }),
                        ..Volume::default()
                    }]),
                    node_selector: config.node_selector.clone(),
                    tolerations: config.tolerations.clone(),
                    image_pull_secrets: pull_secret,
                    containers: vec![Container {
                        name: "nsp-servers".to_owned(),
                        image: Some(image),
                        volume_mounts: Some(vec![VolumeMount {
                            name: "data".to_owned(),
                            mount_path: "/var/sbo".to_owned(),
                            ..VolumeMount::default()
                        }]),
                        command: Some(vec![
                            "/bin/sh".to_string(),
                            "-c".to_string(),
                            "mkdir -p /var/sbo/db/reset && touch /var/sbo/db/reset/r0.txt && ls -lah /var/sbo/db/reset/;".to_string(),
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
        ..Job::default()
    };

    if let Some(spec) = job.spec.as_ref() {
        let h = spec_hash(spec);
        set_hash_annotation(&mut job.metadata, h);
    }
    job
}

pub fn gen_restore_job(ebo_server: &EboServer, job_name: String, backup_name: String, config: &EboConfig) -> Job {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
    let image = ebo_server.status.as_ref().unwrap().pod_image.clone().unwrap();
    let pull_secret: Option<Vec<LocalObjectReference>> = config
        .pull_secret
        .clone()
        .map(|s| vec![LocalObjectReference { name: s }]);

    // backup to restore
    // TODO: Maybe check that it exists? -> if not exists take latest with same version?
    let backup_name_short = backup_name.strip_suffix(".xbk").unwrap_or(&backup_name);

    let cmd = format!(
        "cp /mnt/backup/{} /var/sbo/db_backup/LocalBackup/ && echo -e \"\'\'\'StruxureWare\n{}\nAllData\nRequestTypeRestore\'\'\'\" >> /var/sbo/db_backup/restore", backup_name, backup_name_short
    );

    let mut job = Job {
        metadata: ObjectMeta {
            name: Some(job_name),
            labels: Some(gen_labels(name, server_id, system_id)),
            owner_references: Some(vec![owner_ref]),
            annotations: None,
            ..ObjectMeta::default()
        },
        spec: Some(JobSpec {
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    annotations: Some(BTreeMap::from([
                        ("linkerd.io/inject".to_owned(), "disabled".to_owned()),
                        ("config.linkerd.io/proxy-await".to_owned(), "disabled".to_owned()),
                    ])),
                    ..ObjectMeta::default()
                }),
                spec: Some(PodSpec {
                    security_context: Some(PodSecurityContext {
                        fs_group: Some(60606),
                        ..PodSecurityContext::default()
                    }),
                    volumes: Some(vec![
                        Volume {
                            name: "data".to_owned(),
                            persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                                claim_name: format!("{name}-data"),
                                ..PersistentVolumeClaimVolumeSource::default()
                            }),
                            ..Volume::default()
                        },
                        Volume {
                            name: "backup".to_owned(),
                            persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                                claim_name: format!("{name}-backup"),
                                ..PersistentVolumeClaimVolumeSource::default()
                            }),
                            ..Volume::default()
                        },
                    ]),
                    node_selector: config.node_selector.clone(),
                    tolerations: config.tolerations.clone(),
                    image_pull_secrets: pull_secret,
                    containers: vec![Container {
                        name: "nsp-servers".to_owned(),
                        image: Some(image),
                        volume_mounts: Some(vec![
                            VolumeMount {
                                name: "data".to_owned(),
                                mount_path: "/var/sbo".to_owned(),
                                ..VolumeMount::default()
                            },
                            VolumeMount {
                                name: "backup".to_owned(),
                                mount_path: "/mnt/backup".to_owned(),
                                ..VolumeMount::default()
                            },
                        ]),
                        command: Some(vec!["sh".to_owned(), "-c".to_owned(), cmd]),
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
        ..Job::default()
    };

    if let Some(spec) = job.spec.as_ref() {
        let h = spec_hash(spec);
        set_hash_annotation(&mut job.metadata, h);
    }
    job
}

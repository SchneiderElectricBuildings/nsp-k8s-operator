use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, HTTPGetAction, LocalObjectReference, PersistentVolumeClaimVolumeSource, Pod,
    PodSecurityContext, PodSpec, Probe, ResourceRequirements, SecretVolumeSource, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::{apis::meta::v1::ObjectMeta, util::intstr::IntOrString};
use kube::Resource;
use std::collections::BTreeMap;

use crate::config::models::EboConfig;
use crate::crd::v1alpha6::EboServer;
use crate::eboserver::EboServerExt;
use crate::resources::util::{
    calc_pod_image, gen_hostname, gen_pod_annotations, gen_pod_labels, set_hash_annotation, spec_hash,
};

pub fn gen_pod(ebo_server: &EboServer, config: &EboConfig) -> Pod {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let nsp_machine_id = ebo_server.get_nsp_machine_id(config);
    let namespace = ebo_server.metadata.namespace.as_ref().unwrap();
    let eboservertype = ebo_server.spec.type_.to_string();
    let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
    let image = calc_pod_image(ebo_server, &config.registry);
    let utils_registry = config.utils.registry.clone();
    let utils_image = config.utils.image.clone();
    let host = gen_hostname(name, namespace, true, config);
    let ebo_id = ebo_server.get_server_id();

    let pull_secret: Option<Vec<LocalObjectReference>> = config
        .pull_secret
        .clone()
        .map(|s| vec![LocalObjectReference { name: s }]);

    let mut init_command = "set -ex
         mkdir -p /mnt/data/db/Configuration && mkdir -p /mnt/data/db/System/Names/ && mkdir -p /mnt/data/db/etc"
        .to_string();

    if let Some(ebo_id) = ebo_id {
        init_command.push_str(&format!(
            "
         test ! -f /mnt/data/db/System/Names/me && echo -n {ebo_id} > /mnt/data/db/System/Names/me || true
         test ! -f /mnt/data/db/System/Names/{ebo_id} && echo -n {name} > /mnt/data/db/System/Names/{ebo_id} || true"
        ));
    }

    if ebo_server.spec.inject_license.unwrap_or(true) {
        init_command.push_str(
            "
         test -d /mnt/data/db/etc/licensing || test -f /mnt/lic/license.tgz && tar -xzvf /mnt/lic/license.tgz -C /mnt/data/db/etc && chown -R admin:admin /mnt/data/db/etc/licensing"
        );
    }

    let mut volumes = Some(vec![
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
        Volume {
            name: "license".to_owned(),
            secret: Some(SecretVolumeSource {
                secret_name: Some("ebo-embedded-demo-license".to_owned()),
                default_mode: Some(420),
                optional: Some(true),
                ..SecretVolumeSource::default()
            }),
            ..Volume::default()
        },
        Volume {
            name: "custom-config".to_owned(),
            secret: Some(SecretVolumeSource {
                secret_name: Some(config.custom_config_secret.clone()),
                default_mode: Some(420),
                optional: Some(true),
                ..SecretVolumeSource::default()
            }),
            ..Volume::default()
        },
    ]);

    let volume_mounts = Some(vec![
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
        VolumeMount {
            name: "custom-config".to_owned(),
            mount_path: "/var/sbo/db/etc/custom_config".to_owned(),
            ..VolumeMount::default()
        },
    ]);

    let mut init_container_mounts = Some(vec![
        VolumeMount {
            name: "data".to_owned(),
            mount_path: "/mnt/data".to_owned(),
            ..VolumeMount::default()
        },
        VolumeMount {
            name: "license".to_owned(),
            mount_path: "/mnt/lic".to_owned(),
            ..VolumeMount::default()
        },
    ]);

    if config.initial_password.enabled {
        volumes.as_mut().unwrap().push(Volume {
            name: "password".to_owned(),
            secret: Some(SecretVolumeSource {
                secret_name: Some(format!("{name}-init-pass")),
                default_mode: Some(420),
                ..SecretVolumeSource::default()
            }),
            ..Volume::default()
        });

        init_container_mounts.as_mut().unwrap().push(VolumeMount {
            name: "password".to_owned(),
            mount_path: "/var/init_pass".to_owned(),
            ..VolumeMount::default()
        });

        init_command.push_str(
            "
         cp -n /var/init_pass/password /mnt/data/db/System/password",
        );
    }

    let init_containers = Some(vec![Container {
        name: "populate-db-volume".to_owned(),
        image: Some(format!("{utils_registry}{utils_image}")),
        volume_mounts: init_container_mounts,
        command: Some(vec!["/bin/sh".to_string(), "-c".to_string(), init_command]),
        security_context: Some(k8s_openapi::api::core::v1::SecurityContext {
            run_as_user: Some(60606),
            run_as_group: Some(60606),
            allow_privilege_escalation: Some(false),
            ..k8s_openapi::api::core::v1::SecurityContext::default()
        }),
        ..Container::default()
    }]);

    let mut env_vars = vec![
        EnvVar {
            name: "NSP_ACCEPT_EULA".to_owned(),
            value: Some("Yes".to_owned()),
            ..EnvVar::default()
        },
        EnvVar {
            name: "SERVER_TYPE".to_owned(),
            value: Some(eboservertype),
            ..EnvVar::default()
        },
        EnvVar {
            name: "NSP_HOST".to_owned(),
            value: Some(host),
            ..EnvVar::default()
        },
        EnvVar {
            name: "NSP_HOST_SUFFIX".to_owned(),
            value: Some("/".to_owned()),
            ..EnvVar::default()
        },
        EnvVar {
            name: "NSP_REQUEST_PREFIX".to_owned(),
            value: Some("/".to_owned()),
            ..EnvVar::default()
        },
        EnvVar {
            name: "NSP_USE_MANAGER_WEBSERVER".to_owned(),
            value: Some("true".to_owned()),
            ..EnvVar::default()
        },
    ];

    if let Some(nsp_machine_id) = nsp_machine_id {
        env_vars.push(EnvVar {
            name: "NSP_MACHINE_ID".to_owned(),
            value: Some(nsp_machine_id.to_string()),
            ..EnvVar::default()
        });
    }

    let proxy_address = config.proxy.address.clone();
    let no_proxy_list = config.proxy.no_proxy_list.clone();
    if config.proxy.enabled {
        let mut proxy_envs = vec![
            EnvVar {
                name: "http_proxy".to_owned(),
                value: Some(proxy_address.clone()),
                ..EnvVar::default()
            },
            EnvVar {
                name: "https_proxy".to_owned(),
                value: Some(proxy_address.clone()),
                ..EnvVar::default()
            },
            EnvVar {
                name: "HTTP_PROXY".to_owned(),
                value: Some(proxy_address.clone()),
                ..EnvVar::default()
            },
            EnvVar {
                name: "HTTPS_PROXY".to_owned(),
                value: Some(proxy_address.clone()),
                ..EnvVar::default()
            },
            EnvVar {
                name: "no_proxy".to_owned(),
                value: Some(no_proxy_list.clone()),
                ..EnvVar::default()
            },
        ];
        env_vars.append(&mut proxy_envs);
    }

    let mut pod = Pod {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            labels: Some(gen_pod_labels(name, server_id, system_id, config.clone())),
            owner_references: Some(vec![owner_ref]),
            annotations: Some(gen_pod_annotations(name, config.clone())),
            ..ObjectMeta::default()
        },
        spec: Some(PodSpec {
            image_pull_secrets: pull_secret,
            security_context: Some(PodSecurityContext {
                fs_group: Some(60606),
                ..PodSecurityContext::default()
            }),
            volumes,
            init_containers,
            containers: vec![Container {
                name: "nsp-servers".to_owned(),
                image: Some(image),
                env: Some(env_vars),
                volume_mounts,
                readiness_probe: if config.probes.readiness.enabled {
                    Some(Probe {
                        http_get: Some(HTTPGetAction {
                            path: Some(
                                config
                                    .probes
                                    .readiness
                                    .path
                                    .clone()
                                    .unwrap_or_else(|| "/nsp/healthz/readiness".to_owned()),
                            ),
                            port: IntOrString::Int(80),
                            ..HTTPGetAction::default()
                        }),
                        initial_delay_seconds: config.probes.readiness.initial_delay_seconds,
                        period_seconds: config.probes.readiness.period_seconds,
                        timeout_seconds: config.probes.readiness.timeout_seconds,
                        failure_threshold: config.probes.readiness.failure_threshold,
                        ..Probe::default()
                    })
                } else {
                    None
                },
                liveness_probe: if config.probes.liveness.enabled {
                    Some(Probe {
                        http_get: Some(HTTPGetAction {
                            path: Some(
                                config
                                    .probes
                                    .liveness
                                    .path
                                    .clone()
                                    .unwrap_or_else(|| "/nsp/healthz/liveness".to_owned()),
                            ),
                            port: IntOrString::Int(80),
                            ..HTTPGetAction::default()
                        }),
                        initial_delay_seconds: config.probes.liveness.initial_delay_seconds,
                        period_seconds: config.probes.liveness.period_seconds,
                        timeout_seconds: config.probes.liveness.timeout_seconds,
                        failure_threshold: config.probes.liveness.failure_threshold,
                        ..Probe::default()
                    })
                } else {
                    None
                },
                ports: Some(vec![
                    ContainerPort {
                        name: Some("http".to_owned()),
                        container_port: 80,
                        ..ContainerPort::default()
                    },
                    ContainerPort {
                        name: Some("https".to_owned()),
                        container_port: 443,
                        ..ContainerPort::default()
                    },
                    ContainerPort {
                        name: Some("tcp".to_owned()),
                        container_port: 4444,
                        ..ContainerPort::default()
                    },
                    ContainerPort {
                        name: Some("manager".to_owned()),
                        container_port: 8080,
                        ..ContainerPort::default()
                    },
                ]),
                resources: Some(ResourceRequirements {
                    limits: Some(BTreeMap::from([
                        ("cpu".to_owned(), ebo_server.spec.resources.limits.cpu.to_owned()),
                        ("memory".to_owned(), ebo_server.spec.resources.limits.memory.to_owned()),
                    ])),
                    requests: Some(BTreeMap::from([
                        ("cpu".to_owned(), ebo_server.spec.resources.requests.cpu.to_owned()),
                        ("memory".to_owned(), ebo_server.spec.resources.requests.memory.to_owned()),
                    ])),
                    ..ResourceRequirements::default()
                }),
                ..Container::default()
            }],
            node_selector: config.node_selector.clone(),
            tolerations: config.tolerations.clone(),
            ..PodSpec::default()
        }),
        ..Pod::default()
    };

    if let Some(spec) = pod.spec.as_ref() {
        let h = spec_hash(spec);
        set_hash_annotation(&mut pod.metadata, h);
    }
    pod
}

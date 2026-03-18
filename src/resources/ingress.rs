use k8s_openapi::api::networking::v1::{
    HTTPIngressPath, HTTPIngressRuleValue, Ingress, IngressBackend, IngressRule, IngressServiceBackend, IngressSpec,
    IngressTLS, ServiceBackendPort,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::Resource;

use crate::config::models::EboConfig;
use crate::crd::v1alpha6::EboServer;
use crate::eboserver::EboServerExt;
use crate::resources::util::{gen_hostname, gen_labels, set_hash_annotation, spec_hash};

pub fn gen_ingress(ebo_server: &EboServer, config: &EboConfig) -> Ingress {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let namespace = ebo_server.metadata.namespace.as_ref().unwrap();
    let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
    let host = gen_hostname(name, namespace, true, config);

    let cert_secret_name = if !config.ingress.controller.cert_secret_name.is_empty() {
        config.ingress.controller.cert_secret_name.clone()
    } else {
        format!("{name}-{namespace}-eboserver-tls-secret")
    };

    let mut ing = Ingress {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            labels: Some(gen_labels(name, server_id, system_id)),
            annotations: config.ingress.controller.annotations.clone(),
            owner_references: Some(vec![owner_ref]),
            ..ObjectMeta::default()
        },
        spec: Some(IngressSpec {
            ingress_class_name: Some(config.ingress.controller.ingress_class.clone()),
            rules: Some(vec![IngressRule {
                host: Some(host.clone()),
                http: Some(HTTPIngressRuleValue {
                    paths: vec![HTTPIngressPath {
                        path_type: "Prefix".to_string(),
                        path: Some("/".to_owned()),
                        backend: IngressBackend {
                            service: Some(IngressServiceBackend {
                                name: name.clone(),
                                port: Some(ServiceBackendPort {
                                    name: Some("https".to_string()),
                                    ..ServiceBackendPort::default()
                                }),
                            }),
                            ..IngressBackend::default()
                        },
                    }],
                }),
            }]),
            tls: Some(vec![IngressTLS {
                hosts: Some(vec![host.clone()]),
                secret_name: Some(cert_secret_name),
            }]),
            ..IngressSpec::default()
        }),
        ..Ingress::default()
    };
    if let Some(spec) = ing.spec.as_ref() {
        let h = spec_hash(spec);
        set_hash_annotation(&mut ing.metadata, h);
    }
    ing
}

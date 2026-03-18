use crate::eboserver::reconcile::EboServerExt;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kcr_cert_manager_io::v1::certificates::{Certificate, CertificateIssuerRef, CertificateSpec};

use crate::config::models::EboConfig;
use crate::crd::v1alpha6::EboServer;
use crate::resources::util::{gen_hostname, gen_labels, set_hash_annotation, spec_hash};

pub fn gen_certificate(ebo_server: &EboServer, config: &EboConfig) -> Certificate {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let namespace = ebo_server.metadata.namespace.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let host = gen_hostname(name, namespace, true, config);

    let mut cert = Certificate {
        metadata: ObjectMeta {
            name: Some(format!("{name}-{namespace}-eboserver-tls-cert")),
            labels: Some(gen_labels(name, server_id, system_id)),
            ..ObjectMeta::default()
        },
        spec: CertificateSpec {
            secret_name: format!("{name}-{namespace}-eboserver-tls-secret"),
            dns_names: Some(vec![host]),
            issuer_ref: CertificateIssuerRef {
                name: config.ingress.istio_gateway.cert_issuer_name.clone(),
                kind: Some(config.ingress.istio_gateway.cert_issuer_kind.clone()),
                group: Some("cert-manager.io".to_string()),
            },
            ..CertificateSpec::default()
        },
        ..Certificate::default()
    };

    let h = spec_hash(&cert.spec);
    set_hash_annotation(&mut cert.metadata, h);
    cert
}

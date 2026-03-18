use k8s_openapi::api::core::v1::{Service, ServicePort, ServiceSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::Resource;

use crate::crd::v1alpha6::EboServer;
use crate::eboserver::EboServerExt;
use crate::resources::util::{gen_labels, set_hash_annotation, spec_hash};

pub fn gen_svc(ebo_server: &EboServer) -> Service {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
    let mut service = Service {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            labels: Some(gen_labels(name, server_id.clone(), system_id.clone())),
            owner_references: Some(vec![owner_ref]),
            ..ObjectMeta::default()
        },
        spec: Some(ServiceSpec {
            type_: Some(ebo_server.spec.service_type.to_string()),
            selector: Some(gen_labels(name, server_id, system_id)),
            ports: Some(vec![
                ServicePort {
                    name: Some("http".to_owned()),
                    port: 80,
                    target_port: Some(IntOrString::String("http".to_owned())),
                    ..ServicePort::default()
                },
                ServicePort {
                    name: Some("https".to_owned()),
                    port: 443,
                    target_port: Some(IntOrString::String("https".to_owned())),
                    ..ServicePort::default()
                },
                ServicePort {
                    name: Some("tcp".to_owned()),
                    port: 4444,
                    target_port: Some(IntOrString::String("tcp".to_owned())),
                    ..ServicePort::default()
                },
                ServicePort {
                    name: Some("manager".to_owned()),
                    port: 8080,
                    target_port: Some(IntOrString::String("manager".to_owned())),
                    ..ServicePort::default()
                },
            ]),
            ..ServiceSpec::default()
        }),
        ..Service::default()
    };
    if let Some(spec) = service.spec.as_ref() {
        let h = spec_hash(spec);
        set_hash_annotation(&mut service.metadata, h);
    }
    service
}

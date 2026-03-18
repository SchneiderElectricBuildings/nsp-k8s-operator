use crate::eboserver::reconcile::EboServerExt;
use kube::Resource;

use gateway_api::apis::standard::{
    common::ParentReference,
    httproutes::{
        HTTPBackendReference, HTTPRoute, HttpRouteRule, HttpRouteRulesMatchesPathType, HttpRouteSpec, PathMatch,
        RouteMatch,
    },
};

use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

use crate::config::models::EboConfig;
use crate::crd::v1alpha6::EboServer;
use crate::resources::util::{gen_hostname, gen_labels, set_hash_annotation, spec_hash};

pub fn gen_httproute(ebo_server: &EboServer, config: &EboConfig) -> HTTPRoute {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let namespace = ebo_server.metadata.namespace.as_ref().unwrap();
    let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
    let host = gen_hostname(name, namespace, true, config);

    let mut route = HTTPRoute {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            labels: Some(gen_labels(name, server_id, system_id)),
            annotations: None,
            owner_references: Some(vec![owner_ref]),
            ..Default::default()
        },
        spec: HttpRouteSpec {
            parent_refs: Some(vec![ParentReference {
                name: format!("{name}-{namespace}-listener"),
                namespace: Some(config.ingress.istio_gateway.istio_namespace.clone()),
                group: Some("gateway.networking.x-k8s.io".to_string()),
                kind: Some("XListenerSet".to_string()),
                // optional fields present in ParentReference:
                port: None,
                section_name: None,
            }]),
            hostnames: Some(vec![host]),
            rules: Some(vec![HttpRouteRule {
                // default match: all requests
                matches: Some(vec![RouteMatch {
                    path: Some(PathMatch {
                        r#type: Some(HttpRouteRulesMatchesPathType::PathPrefix),
                        value: Some("/".to_string()),
                    }),
                    ..Default::default()
                }]),
                backend_refs: Some(vec![HTTPBackendReference {
                    name: name.clone(),
                    port: Some(80),
                    kind: Some("Service".to_string()),
                    weight: Some(1),
                    group: None,
                    namespace: None,
                    filters: None,
                }]),
                ..Default::default()
            }]),
        },
        ..Default::default()
    };

    let h = spec_hash(&route.spec);
    set_hash_annotation(&mut route.metadata, h);
    route
}

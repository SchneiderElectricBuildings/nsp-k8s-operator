use crate::eboserver::reconcile::EboServerExt;
use std::collections::BTreeMap;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kcr_gateway_networking_x_k8s_io::v1alpha1::xlistenersets::{
    XListenerSet, XListenerSetListeners, XListenerSetListenersAllowedRoutes,
    XListenerSetListenersAllowedRoutesNamespaces, XListenerSetListenersAllowedRoutesNamespacesFrom,
    XListenerSetListenersAllowedRoutesNamespacesSelector, XListenerSetListenersTls,
    XListenerSetListenersTlsCertificateRefs, XListenerSetListenersTlsMode, XListenerSetSpec,
};

use crate::config::models::EboConfig;
use crate::crd::v1alpha6::EboServer;
use crate::resources::util::{gen_hostname, gen_labels, set_hash_annotation, spec_hash};

pub fn generate_xlistener_set(ebo_server: &EboServer, config: &EboConfig) -> XListenerSet {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let namespace = ebo_server.metadata.namespace.as_ref().unwrap();
    let gateway_name = config.ingress.istio_gateway.gateway_name.clone();
    let host = gen_hostname(name, namespace, true, config);

    let mut xlistener_set = XListenerSet {
        metadata: ObjectMeta {
            name: Some(format!("{name}-{namespace}-listener")),
            labels: Some(gen_labels(name, server_id, system_id)),
            ..ObjectMeta::default()
        },
        spec: XListenerSetSpec {
            parent_ref: kcr_gateway_networking_x_k8s_io::v1alpha1::xlistenersets::XListenerSetParentRef {
                group: Some("gateway.networking.k8s.io".to_string()),
                kind: Some("Gateway".to_string()),
                name: gateway_name,
                namespace: Some(config.ingress.istio_gateway.istio_namespace.clone()),
            },
            listeners: vec![XListenerSetListeners {
                name: name.clone(),
                hostname: Some(host),
                port: 443,
                protocol: "HTTPS".to_string(),
                tls: Some(XListenerSetListenersTls {
                    mode: Some(XListenerSetListenersTlsMode::Terminate),
                    certificate_refs: Some(vec![XListenerSetListenersTlsCertificateRefs {
                        name: format!("{name}-{namespace}-eboserver-tls-secret"),
                        kind: Some("Secret".to_string()),
                        group: None,
                        ..XListenerSetListenersTlsCertificateRefs::default()
                    }]),
                    ..XListenerSetListenersTls::default()
                }),
                allowed_routes: Some(XListenerSetListenersAllowedRoutes {
                    namespaces: Some(XListenerSetListenersAllowedRoutesNamespaces {
                        from: Some(XListenerSetListenersAllowedRoutesNamespacesFrom::Selector),
                        selector: Some(XListenerSetListenersAllowedRoutesNamespacesSelector {
                            match_labels: Some(BTreeMap::from([(
                                "kubernetes.io/metadata.name".to_string(),
                                namespace.clone(),
                            )])),
                            ..XListenerSetListenersAllowedRoutesNamespacesSelector::default()
                        }),
                    }),
                    ..XListenerSetListenersAllowedRoutes::default()
                }),
            }],
        },
        ..XListenerSet::default()
    };

    let h = spec_hash(&xlistener_set.spec);
    set_hash_annotation(&mut xlistener_set.metadata, h);
    xlistener_set
}

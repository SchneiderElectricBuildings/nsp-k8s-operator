use gateway_api::apis::standard::httproutes::HTTPRoute;
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{Namespace, PersistentVolumeClaim, Pod, Secret, Service};
use k8s_openapi::api::networking::v1::Ingress;
use kcr_cert_manager_io::v1::certificates::Certificate;
use kcr_gateway_networking_x_k8s_io::v1alpha1::xlistenersets::XListenerSet;

use kube::api::Api;

/// Handles for Kubernetes API resources used by the EboServer controller.
///
/// Keeping this in its own module avoids circular dependencies when your FSM
/// (observe/act) needs access to Kubernetes APIs.
#[derive(Clone)]
pub struct Handles {
    pub namespaces: Api<Namespace>,
    pub pods: Api<Pod>,
    pub pvcs: Api<PersistentVolumeClaim>,
    pub services: Api<Service>,
    pub ingresses: Api<Ingress>,
    pub cronjobs: Api<CronJob>,
    pub jobs: Api<Job>,
    pub secrets: Api<Secret>,
    pub httproutes: Api<HTTPRoute>,
    pub xlistenersets: Api<XListenerSet>,
    pub certificates: Api<Certificate>,
}

impl Handles {
    pub fn new(client: kube::Client, ns: &str, istio_ns: &str) -> Self {
        Self {
            namespaces: Api::all(client.clone()),
            pods: Api::namespaced(client.clone(), ns),
            pvcs: Api::namespaced(client.clone(), ns),
            services: Api::namespaced(client.clone(), ns),
            ingresses: Api::namespaced(client.clone(), ns),
            cronjobs: Api::namespaced(client.clone(), ns),
            jobs: Api::namespaced(client.clone(), ns),
            secrets: Api::namespaced(client.clone(), ns),
            httproutes: Api::namespaced(client.clone(), ns),
            xlistenersets: Api::namespaced(client.clone(), istio_ns),
            certificates: Api::namespaced(client, istio_ns),
        }
    }
}

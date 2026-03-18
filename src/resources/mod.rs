use crate::{controller::OPERATOR_NAME, resources::util::get_hash_annotation};
use kube::core::GroupVersionKind;
use kube::discovery::Discovery;
use kube::{
    api::{Patch, PatchParams},
    Api, Resource, ResourceExt,
};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use tracing::{debug, info, instrument};

pub mod certificate;
pub mod cronjob;
pub mod httproute;
pub mod ingress;
pub mod job;
pub mod pod;
pub mod pvc;
pub mod secret;
pub mod service;
pub mod xlistenerset;

pub mod util;

pub async fn is_served<K>(api: &Api<K>) -> Result<bool, kube::Error>
where
    K: Clone + Serialize + DeserializeOwned + Debug + Send + Sync + 'static + Resource<DynamicType = ()>,
{
    let client = api.clone().into_client();
    let discovery = Discovery::new(client).run().await?;

    let gvk = GroupVersionKind::gvk(K::group(&()).as_ref(), K::version(&()).as_ref(), K::kind(&()).as_ref());

    Ok(discovery.resolve_gvk(&gvk).is_some())
}

/// Server-Side Apply desired state. If nothing changed, apiserver no-ops.
/// Works for any typed Kubernetes object.
#[instrument(
    skip(api, desired),
    fields(
        kind = %K::kind(&()),
        name = %desired.name_any()
    )
)]
pub async fn ensure_apply<K>(api: &Api<K>, desired: &K) -> Result<(), kube::Error>
where
    K: Clone + Serialize + DeserializeOwned + Debug + Send + Sync + 'static + Resource<DynamicType = ()>,
{
    let name = desired.name_any();
    let pp = PatchParams::apply(OPERATOR_NAME).force();
    debug!("applying desired state via server-side apply");
    api.patch(&name, &pp, &Patch::Apply(desired)).await?;
    Ok(())
}

#[tracing::instrument(skip(api), fields(kind = %K::kind(&()), name = %name))]
pub async fn delete_if_exists<K>(api: &Api<K>, name: &str) -> Result<(), kube::Error>
where
    K: Clone + Serialize + DeserializeOwned + Debug + Send + Sync + 'static + Resource<DynamicType = ()>,
{
    if !is_served(api).await? {
        return Ok(());
    }

    match api.delete(name, &Default::default()).await {
        Ok(_) => {
            info!("deleted resource");
            Ok(())
        }

        // normal: object does not exist
        Err(kube::Error::Api(ae)) if ae.code == 404 => {
            debug!("not found, nothing to delete");
            Ok(())
        }

        // CRD / API group not installed (router 404 -> kube can't parse Status)
        Err(kube::Error::SerdeError(_)) => {
            debug!("api type not served on this cluster, skipping delete");
            Ok(())
        }

        // also sometimes happens depending on reverse proxies / apiserver path
        Err(kube::Error::HyperError(_)) => {
            debug!("api endpoint not present, skipping delete");
            Ok(())
        }

        Err(e) => Err(e),
    }
}

/// Detect drift by comparing spec-hash annotations.
/// Returns true if they differ, or if current is missing the annotation.
#[instrument(skip(current, desired))]
pub fn detect_drift<K>(current: &K, desired: &K) -> bool
where
    K: Resource,
{
    let current_hash = get_hash_annotation(current.meta());
    let desired_hash = get_hash_annotation(desired.meta());

    let drift = match (current_hash.clone(), desired_hash.clone()) {
        (Some(c), Some(d)) => c != d,
        (None, Some(_)) => true,
        _ => true,
    };

    debug!(?current_hash, ?desired_hash, drift, "drift check");
    drift
}

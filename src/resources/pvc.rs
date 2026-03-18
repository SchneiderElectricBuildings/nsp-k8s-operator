use k8s_openapi::api::core::v1::{PersistentVolumeClaim, PersistentVolumeClaimSpec, VolumeResourceRequirements};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::Resource;
use std::collections::BTreeMap;

use crate::crd::v1alpha6::{EboServer, EboServerRetentionPolicy};
use crate::eboserver::EboServerExt;
use crate::resources::util::{gen_labels, set_hash_annotation, spec_hash};

pub fn gen_pvc(ebo_server: &EboServer, volume_name: &str) -> PersistentVolumeClaim {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let storage = match volume_name {
        "data" => &ebo_server.spec.db_storage,
        "backup" => &ebo_server.spec.bckp_storage,
        _ => &ebo_server.spec.db_storage,
    };
    let capacity = storage.capacity.clone().unwrap_or_else(|| Quantity("4Gi".to_owned()));

    let mut pvc = PersistentVolumeClaim {
        metadata: ObjectMeta {
            name: Some(format!("{name}-{volume_name}")),
            labels: Some(gen_labels(format!("{name}-{volume_name}").as_str(), server_id, system_id)),
            ..ObjectMeta::default()
        },
        spec: Some(PersistentVolumeClaimSpec {
            access_modes: Some(vec!["ReadWriteOnce".to_owned()]),
            resources: Some(VolumeResourceRequirements {
                requests: Some(BTreeMap::from([("storage".to_owned(), capacity)])),
                ..VolumeResourceRequirements::default()
            }),
            storage_class_name: storage.storage_class_name.clone(),
            ..PersistentVolumeClaimSpec::default()
        }),
        ..PersistentVolumeClaim::default()
    };
    if storage.retention_policy == EboServerRetentionPolicy::Delete {
        let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
        pvc.metadata.owner_references = Some(vec![owner_ref]);
    }

    if let Some(spec) = pvc.spec.as_ref() {
        let h = spec_hash(spec);
        set_hash_annotation(&mut pvc.metadata, h);
    }
    pvc
}

pub fn pvc_needs_update(current: &PersistentVolumeClaim) -> bool {
    let requested = current
        .spec
        .as_ref()
        .and_then(|s| s.resources.as_ref())
        .and_then(|r| r.requests.as_ref())
        .and_then(|req| req.get("storage"));

    let allocated = current
        .status
        .as_ref()
        .and_then(|s| s.capacity.as_ref())
        .and_then(|cap| cap.get("storage"));

    match (requested, allocated) {
        (Some(req), Some(cap)) => req != cap,
        (Some(_), None) => true,
        _ => false,
    }
}

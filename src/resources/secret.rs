use chrono::{NaiveTime, Timelike};
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::ByteString;
use kube::Resource;
use std::collections::BTreeMap;

use crate::config::models::EboConfig;
use crate::crd::v1alpha6::EboServer;
use crate::eboserver::EboServerExt;
use crate::resources::util::{gen_labels, gen_temp_password};
pub fn gen_pass_secret(
    ebo_server: &EboServer, temp_pass: (Option<String>, Option<String>), config: &EboConfig,
) -> Secret {
    let name = ebo_server.metadata.name.as_ref().unwrap();
    let server_id = ebo_server.get_server_id();
    let system_id = ebo_server.get_system_id();
    let owner_ref = ebo_server.controller_owner_ref(&()).unwrap();
    let initial_password = temp_pass.0.unwrap_or_else(|| gen_temp_password(12));
    let password_time_limit = temp_pass.1.unwrap_or("00:30:00".to_string());
    let password_time_limit = match NaiveTime::parse_from_str(&password_time_limit, "%H:%M:%S") {
        Ok(time) => time.num_seconds_from_midnight(),
        Err(_) => {
            tracing::error!("Invalid password time limit format: {}", password_time_limit);
            1800
        }
    };

    let additional_time_limit = config.initial_password.additional_time_limit;
    let password_time_limit = password_time_limit + additional_time_limit.as_secs() as u32;

    let initial_password = format!("{initial_password}\n{password_time_limit}\ntrue");
    let mut data = BTreeMap::new();
    data.insert("password".to_owned(), ByteString(initial_password.as_bytes().to_vec()));

    let secret = Secret {
        metadata: ObjectMeta {
            name: Some(format!("{name}-init-pass")),
            labels: Some(gen_labels(name, server_id, system_id)),
            owner_references: Some(vec![owner_ref]),
            ..ObjectMeta::default()
        },
        data: Some(data),
        ..Secret::default()
    };
    secret
}

use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use rand::seq::{IndexedRandom, SliceRandom};
use rand::{distr::Alphanumeric, RngExt};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::config::models::EboConfig;
use crate::crd::v1alpha6::{EboServer, EboServerType};

pub const SPEC_HASH_ANNOTATION: &str = "eboserver.digbld.em.se.com/spec-hash";

pub fn calc_pod_image(ebo_server: &EboServer, registry: &str) -> String {
    let image_name = match ebo_server.spec.type_ {
        EboServerType::EdgeServer => "ebo-edge-server".to_owned(),
        EboServerType::EnterpriseServer => "ebo-enterprise-server".to_owned(),
        EboServerType::EnterpriseCentral => "ebo-enterprise-central".to_owned(),
        EboServerType::EnterpriseCentralCloud | EboServerType::EnterpriseServerCloud => "ebo-server".to_owned(),
    };
    format!("{}{}:{}", registry, image_name, ebo_server.spec.version.to_owned())
}

pub fn spec_hash<T: Serialize>(spec: &T) -> String {
    let bytes = serde_json::to_vec(spec).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn set_hash_annotation(meta: &mut ObjectMeta, hash: String) {
    let anns = meta.annotations.get_or_insert(Default::default());
    anns.insert(SPEC_HASH_ANNOTATION.to_string(), hash);
}

pub fn get_hash_annotation(meta: &ObjectMeta) -> Option<String> {
    meta.annotations
        .as_ref()
        .and_then(|a| a.get(SPEC_HASH_ANNOTATION).cloned())
}

pub fn gen_labels(name: &str, server_id: Option<String>, system_id: Option<String>) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::from([
        ("app.kubernetes.io/name".to_owned(), "eboserver".to_owned()),
        ("app.kubernetes.io/instance".to_owned(), format!("eboserver-{name}")),
    ]);
    if let Some(server_id) = server_id {
        labels.insert("eboserver.digbld.em.se.com/ebo-server-id".to_owned(), server_id.to_owned());
    }
    if let Some(system_id) = system_id {
        labels.insert("eboserver.digbld.em.se.com/ebo-system-id".to_owned(), system_id.to_owned());
    }
    labels
}

pub fn gen_pod_labels(
    name: &str, server_id: Option<String>, system_id: Option<String>, config: EboConfig,
) -> BTreeMap<String, String> {
    let ebo_labels = config.labels.unwrap_or_default();
    let mut labels = gen_labels(name, server_id, system_id);
    labels.extend(ebo_labels);
    labels
}

pub fn gen_pod_annotations(_name: &str, config: EboConfig) -> BTreeMap<String, String> {
    let ebo_annotations = config.annotations.unwrap_or_default();
    let mut annotations = BTreeMap::new();
    annotations.extend(ebo_annotations);
    annotations
}

pub fn gen_hostname(name: &str, namespace: &str, add_domain: bool, cfg: &EboConfig) -> String {
    // Ingress knobs
    let namespace_in_domain: bool = cfg.ingress.namespace_in_domain;
    let separator: &str = &cfg.ingress.hostname_separator; // e.g. "." or "-" or "_"
    let domain: &str = &cfg.ingress.domain;

    let mut host = String::with_capacity(name.len() + namespace.len() + 32);
    host.push_str(name);

    if namespace_in_domain {
        host.push_str(separator);
        host.push_str(namespace);
    }
    if !cfg.environment.is_empty() {
        host.push_str(separator);
        host.push_str(&cfg.environment);
    }
    if add_domain {
        host.push('.');
        host.push_str(domain);
    }
    host
}

pub fn gen_temp_password(length: usize) -> String {
    assert!(length >= 5, "Password length must be at least 5");

    let special_chars = [
        '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '-', '_', '=', '+', '[', ']', '{', '}', '.', ',', '?',
    ];
    let mut rng = rand::rng();

    // Ensure at least one character from each required category
    let mut password = vec![
        rng.random_range('a'..='z'),              // Lowercase
        rng.random_range('A'..='Z'),              // Uppercase
        rng.random_range('0'..='9'),              // Digit
        rng.sample(Alphanumeric) as char,         // Alphanumeric (letter or digit)
        *special_chars.choose(&mut rng).unwrap(), // Special character
    ];

    // Fill the rest of the password
    for _ in 5..length {
        let ch = match rng.random_range(0..5) {
            0 => rng.random_range('a'..='z'),              // Lowercase
            1 => rng.random_range('A'..='Z'),              // Uppercase
            2 => rng.random_range('0'..='9'),              // Digit
            3 => rng.sample(Alphanumeric) as char,         // Alphanumeric
            _ => *special_chars.choose(&mut rng).unwrap(), // Special character
        };
        password.push(ch);
    }

    password.shuffle(&mut rng);
    password.iter().collect()
}

//write test for gen_temp_password function
#[cfg(test)]
mod tests {
    use crate::config::models::{IngressConfig, InitialPassword, Probes, Proxy, Utils};

    use super::*;

    #[test]
    fn test_gen_temp_password() {
        let password = gen_temp_password(12);
        println!("Generated password: {}", password);
        assert_eq!(password.len(), 12);
        assert!(password.chars().any(|c| c.is_lowercase()));
        assert!(password.chars().any(|c| c.is_uppercase()));
        assert!(password.chars().any(|c| c.is_digit(10)));
        assert!(password.chars().any(|c| "!@#$%^&*()-_=+[]{}.,?".contains(c)));
    }
    // tiny helper to build only the fields we care about; everything else uses Default
    fn mk_cfg(ns_in_domain: bool, sep: &str, env: &str, domain: &str) -> EboConfig {
        EboConfig {
            // optional stuff we don't use in these tests
            labels: None,
            annotations: None,
            backups: 5,
            node_selector: None,
            tolerations: None,
            nsp_machine_id: false,
            probes: Probes::default(),

            // required fields (fill with harmless values)
            registry: "dummy".into(),
            pull_secret: None,
            custom_config_secret: "x".into(),

            // top-level fields used by gen_hostname
            environment: env.into(),

            // structs we don't exercise → Default
            utils: Utils::default(),
            proxy: Proxy::default(),
            initial_password: InitialPassword::default(),

            // ingress bits we do exercise
            ingress: IngressConfig {
                namespace_in_domain: ns_in_domain,
                hostname_separator: sep.into(),
                domain: domain.into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn gen_hostname_variants() {
        // Case 1: namespace in domain, '.' sep, default domain
        let cfg1 = mk_cfg(true, ".", "", "eboaas.se.com");
        assert_eq!(gen_hostname("ebo", "dev", true, &cfg1), "ebo.dev.eboaas.se.com");
        assert_eq!(gen_hostname("ebo", "dev", false, &cfg1), "ebo.dev");

        // Case 2: no namespace in domain, '-' sep, custom domain
        let cfg2 = mk_cfg(false, "-", "prod", "example.com");
        assert_eq!(gen_hostname("ebo", "ns", true, &cfg2), "ebo-prod.example.com");
        assert_eq!(gen_hostname("ebo", "ns", false, &cfg2), "ebo-prod");

        // Case 3: namespace in domain, '_' sep, custom domain
        let cfg3 = mk_cfg(true, "_", "", "d.io");
        assert_eq!(gen_hostname("ebo", "ns", true, &cfg3), "ebo_ns.d.io");
        assert_eq!(gen_hostname("ebo", "ns", false, &cfg3), "ebo_ns");
    }
}

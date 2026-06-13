use br_core_scope::{DeclareServiceScopes, RawScopeDeclaration, RawScopeSpec, RawServiceManifest};

pub fn raw_manifest(service_key: &str) -> RawServiceManifest {
    RawServiceManifest {
        key: service_key.to_string(),
        label_key: format!("service.{service_key}.label"),
        description_key: format!("service.{service_key}.desc"),
    }
}

pub fn raw_spec(scope_key: &str, platform_only: bool) -> RawScopeSpec {
    RawScopeSpec {
        key: scope_key.to_string(),
        label_key: format!("scope.{scope_key}.label"),
        description_key: format!("scope.{scope_key}.desc"),
        platform_only,
    }
}

pub fn declare(service_key: &str, scope_keys: &[&str]) -> DeclareServiceScopes {
    let scopes = scope_keys.iter().map(|k| raw_spec(k, false)).collect();
    from_raw(RawScopeDeclaration {
        manifest: raw_manifest(service_key),
        scopes,
    })
}

pub fn from_raw(declaration: RawScopeDeclaration) -> DeclareServiceScopes {
    let payload = serde_json::json!({ "declaration": declaration });
    serde_json::from_value(payload)
        .expect("RawScopeDeclaration always deserializes into DeclareServiceScopes")
}

pub fn declaration_label(command: &DeclareServiceScopes) -> String {
    let raw = command.raw();
    let scopes = raw
        .scopes
        .iter()
        .map(|s| s.key.as_str())
        .collect::<Vec<_>>()
        .join(",");
    format!("service={:?}, scopes=[{scopes}]", raw.manifest.key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declare_round_trips_a_clean_declaration() {
        let command = declare("notifier", &["notifier:read", "notifier:admin"]);
        let raw = command.raw();
        assert_eq!(raw.manifest.key, "notifier");
        assert_eq!(raw.scopes.len(), 2);
        assert_eq!(raw.scopes[0].key, "notifier:read");
    }

    #[test]
    fn from_raw_carries_a_malformed_key_unvalidated() {
        let command = from_raw(RawScopeDeclaration {
            manifest: raw_manifest("notifier"),
            scopes: vec![raw_spec("notifier:BAD", false)],
        });
        assert_eq!(command.raw().scopes[0].key, "notifier:BAD");
    }
}

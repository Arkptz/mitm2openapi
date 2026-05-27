//! Envelope-based response splitting.
//!
//! Many APIs wrap every response in a `{ "success": bool, ... }` envelope.
//! This module classifies captured response bodies into *success* vs *error*
//! groups based on a discriminator field, infers an `ApiError` schema from the
//! error examples, and builds a `oneOf` schema with a discriminator annotation.

use openapiv3::{Discriminator, ReferenceOr, Schema, SchemaData, SchemaKind};
use serde_json::Value;

/// Configuration for envelope-based response splitting.
#[derive(Clone, Debug)]
pub struct EnvelopeConfig {
    /// JSON field name used as the discriminator (e.g. `"success"`).
    pub discriminator_field: String,
    /// Optional pre-defined error schema; skips inference when set.
    pub error_shape: Option<Schema>,
    /// Suffix appended to component names (e.g. `"Success"`).
    pub success_suffix: String,
}

/// Group response bodies into (success, error) based on a discriminator field.
///
/// Classification: only a JSON boolean `true` at `discriminator` counts as
/// success. Everything else — `false`, `null`, strings, numbers, or a missing
/// field — is classified as error.
pub fn group_bodies(bodies: &[Value], discriminator: &str) -> (Vec<Value>, Vec<Value>) {
    let mut success = Vec::new();
    let mut error = Vec::new();
    for body in bodies {
        if body.get(discriminator) == Some(&Value::Bool(true)) {
            success.push(body.clone());
        } else {
            error.push(body.clone());
        }
    }
    (success, error)
}

/// Infer an `ApiError` schema from error body examples.
///
/// If `config.error_shape` is set, returns that directly.
/// Otherwise uses [`crate::schema::value_to_schema`] on the first error body.
/// Falls back to an empty `Any` schema when no examples exist.
pub fn infer_api_error(error_bodies: &[Value], config: &EnvelopeConfig) -> Schema {
    if let Some(custom) = &config.error_shape {
        return custom.clone();
    }
    if let Some(first) = error_bodies.first() {
        return crate::schema::value_to_schema(first);
    }
    Schema {
        schema_data: SchemaData::default(),
        schema_kind: SchemaKind::Any(openapiv3::AnySchema::default()),
    }
}

/// Build a `oneOf` schema combining a success `$ref` and an error `$ref`,
/// annotated with an OpenAPI discriminator.
pub fn build_one_of_schema(
    success_ref: &str,
    error_ref: &str,
    discriminator_field: &str,
) -> ReferenceOr<Schema> {
    let one_of = vec![ReferenceOr::ref_(success_ref), ReferenceOr::ref_(error_ref)];

    ReferenceOr::Item(Schema {
        schema_data: SchemaData {
            discriminator: Some(Discriminator {
                property_name: discriminator_field.to_string(),
                mapping: indexmap::IndexMap::new(),
                extensions: indexmap::IndexMap::new(),
            }),
            ..SchemaData::default()
        },
        schema_kind: SchemaKind::OneOf { one_of },
    })
}

/// Derive a PascalCase component name for the success schema.
///
/// Prefers `operationId` when available (uppercasing the first letter),
/// otherwise falls back to `Method` + path segments with each segment
/// capitalised.
pub fn success_component_name(
    operation_id: Option<&str>,
    path: &str,
    method: &str,
    suffix: &str,
) -> String {
    if let Some(op_id) = operation_id {
        let mut chars = op_id.chars();
        return match chars.next() {
            Some(c) => {
                let upper: String = c.to_uppercase().collect();
                format!("{upper}{}{suffix}", chars.as_str())
            }
            None => suffix.to_string(),
        };
    }

    let path_part: String = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let s = s.trim_matches(|c: char| c == '{' || c == '}');
            let mut chars = s.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{upper}{}", chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect();

    let method_part = {
        let mut chars = method.chars();
        match chars.next() {
            Some(c) => {
                let upper: String = c.to_uppercase().collect();
                format!("{upper}{}", chars.as_str().to_lowercase())
            }
            None => String::new(),
        }
    };

    format!("{method_part}{path_part}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn group_by_discriminator() {
        let bodies = vec![
            json!({"success": true, "data": {}}),
            json!({"success": true, "data": {"price": 1.0}}),
            json!({"success": true, "data": {"price": 2.0}}),
            json!({"success": false, "code": 1, "message": "err"}),
        ];
        let (success, error) = group_bodies(&bodies, "success");
        assert_eq!(success.len(), 3);
        assert_eq!(error.len(), 1);
    }

    #[test]
    fn only_success_unchanged() {
        let bodies = vec![json!({"success": true, "data": {}})];
        let (success, error) = group_bodies(&bodies, "success");
        assert_eq!(success.len(), 1);
        assert!(error.is_empty());
    }

    #[test]
    fn non_boolean_discriminator_is_error() {
        let bodies = vec![
            json!({"success": 1}),
            json!({"success": "yes"}),
            json!({"success": null}),
        ];
        let (success, error) = group_bodies(&bodies, "success");
        assert!(success.is_empty());
        assert_eq!(error.len(), 3);
    }

    #[test]
    fn missing_discriminator_field_is_error() {
        let bodies = vec![json!({"data": {}})];
        let (success, error) = group_bodies(&bodies, "success");
        assert!(success.is_empty());
        assert_eq!(error.len(), 1);
    }

    #[test]
    fn zero_error_bodies() {
        let bodies = vec![json!({"success": true, "data": {}})];
        let (success, error) = group_bodies(&bodies, "success");
        assert_eq!(success.len(), 1);
        assert!(error.is_empty());
    }

    #[test]
    fn success_component_name_from_operation_id() {
        let name = success_component_name(
            Some("getFairPrice"),
            "/api/v1/contract/fair_price/{symbol}",
            "GET",
            "Success",
        );
        assert_eq!(name, "GetFairPriceSuccess");
    }

    #[test]
    fn success_component_name_fallback() {
        let name = success_component_name(None, "/api/v1/users/{id}", "GET", "Success");
        assert!(name.contains("Success"));
        assert!(!name.is_empty());
    }

    #[test]
    fn build_one_of_schema_structure() {
        let schema = build_one_of_schema(
            "#/components/schemas/GetTickerSuccess",
            "#/components/schemas/ApiError",
            "success",
        );
        if let ReferenceOr::Item(s) = schema {
            match &s.schema_kind {
                SchemaKind::OneOf { one_of } => {
                    assert_eq!(one_of.len(), 2);
                }
                other => panic!("Expected OneOf, got {other:?}"),
            }
            assert!(s.schema_data.discriminator.is_some());
        } else {
            panic!("Expected Item, got Ref");
        }
    }
}

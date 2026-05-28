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
/// Otherwise merges all error bodies into a single schema using majority-vote
/// type selection per field. Falls back to an empty `Any` schema when no
/// examples exist.
pub fn infer_api_error(error_bodies: &[Value], config: &EnvelopeConfig) -> Schema {
    if let Some(custom) = &config.error_shape {
        return custom.clone();
    }
    if error_bodies.is_empty() {
        return Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Any(openapiv3::AnySchema::default()),
        };
    }
    let mut schema = merge_error_body_schemas(error_bodies);
    pin_discriminator_field(&mut schema, &config.discriminator_field);
    schema
}

fn pin_discriminator_field(schema: &mut Schema, field_name: &str) {
    if let SchemaKind::Type(openapiv3::Type::Object(ref mut obj)) = schema.schema_kind {
        let pinned = Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(openapiv3::Type::Boolean(openapiv3::BooleanType {
                enumeration: vec![Some(false)],
            })),
        };
        obj.properties
            .insert(field_name.to_string(), ReferenceOr::Item(Box::new(pinned)));
    }
}

/// Merge multiple error body JSON values into a single schema.
///
/// For each field across all bodies, picks the representative value whose JSON
/// type appears most frequently (majority vote), then converts the merged
/// object to a schema.
fn merge_error_body_schemas(bodies: &[Value]) -> Schema {
    use std::collections::HashMap;

    let mut field_values: indexmap::IndexMap<String, Vec<&Value>> = indexmap::IndexMap::new();
    for body in bodies {
        if let Value::Object(obj) = body {
            for (key, val) in obj {
                field_values.entry(key.clone()).or_default().push(val);
            }
        }
    }

    let mut merged = serde_json::Map::new();
    for (key, values) in &field_values {
        let mut type_counts: HashMap<u8, (usize, &Value)> = HashMap::new();
        for val in values {
            let disc = json_type_discriminant(val);
            let entry = type_counts.entry(disc).or_insert((0, val));
            entry.0 += 1;
        }
        if let Some((_, representative)) = type_counts.into_values().max_by_key(|(count, _)| *count)
        {
            merged.insert(key.clone(), (*representative).clone());
        }
    }

    crate::schema::value_to_schema(&Value::Object(merged))
}

fn json_type_discriminant(val: &Value) -> u8 {
    match val {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
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
    fn infer_api_error_merges_all_bodies_not_just_first() {
        let bodies = vec![
            json!({"success": false, "code": 401, "msg": 0}),
            json!({"success": false, "code": 401, "msg": "Not logged in"}),
            json!({"success": false, "code": 401, "msg": "Please login first"}),
        ];
        let config = EnvelopeConfig {
            discriminator_field: "success".to_string(),
            error_shape: None,
            success_suffix: "Success".to_string(),
        };
        let schema = infer_api_error(&bodies, &config);
        let yaml = serde_yaml_ng::to_string(&schema).unwrap();
        assert!(
            yaml.contains("msg:")
                && (yaml.contains("type: string") || yaml.contains("- type: string")),
            "msg must be string (or oneOf with string) when 2/3 samples are string:\n{yaml}"
        );
    }

    #[test]
    fn inferred_api_error_includes_discriminator_field_pinned_to_false() {
        let bodies = vec![
            json!({"success": false, "code": 401, "msg": "Not logged in"}),
            json!({"success": false, "code": 99999, "msg": "System busy"}),
        ];
        let config = EnvelopeConfig {
            discriminator_field: "success".to_string(),
            error_shape: None,
            success_suffix: "Success".to_string(),
        };
        let schema = infer_api_error(&bodies, &config);
        let yaml = serde_yaml_ng::to_string(&schema).unwrap();
        assert!(
            yaml.contains("success:"),
            "discriminator field must be in ApiError:\n{yaml}"
        );
        assert!(
            yaml.contains("enum:") && yaml.contains("- false"),
            "discriminator field must be pinned with enum: [false]:\n{yaml}"
        );
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

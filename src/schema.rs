//! Schema inference: convert JSON values to OpenAPI schemas.
//!
//! Converts `serde_json::Value` into `openapiv3::Schema`, matching the Python
//! mitmproxy2swagger `value_to_schema` behavior exactly.

use indexmap::IndexMap;
use openapiv3::{
    AdditionalProperties, AnySchema, ArrayType, BooleanType, IntegerType, NumberType, ObjectType,
    ReferenceOr, Schema, SchemaData, SchemaKind, StringType, Type,
};

/// Check if a string looks like a numeric value (all digits, possibly with leading minus).
fn is_numeric_string(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let s = s.strip_prefix('-').unwrap_or(s);
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Check if a string looks like a UUID (8-4-4-4-12 hex pattern).
fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected_lens = [8, 4, 4, 4, 12];
    parts
        .iter()
        .zip(expected_lens.iter())
        .all(|(part, &len)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Convert a `serde_json::Value` into an `openapiv3::Schema`.
///
/// Matches Python mitmproxy2swagger `value_to_schema` behavior exactly.
pub fn value_to_schema(value: &serde_json::Value) -> Schema {
    match value {
        serde_json::Value::Null => Schema {
            schema_data: SchemaData {
                nullable: true,
                ..SchemaData::default()
            },
            schema_kind: SchemaKind::Type(Type::Object(ObjectType::default())),
        },

        serde_json::Value::Bool(_) => Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::Boolean(BooleanType::default())),
        },

        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                Schema {
                    schema_data: SchemaData::default(),
                    schema_kind: SchemaKind::Type(Type::Integer(IntegerType::default())),
                }
            } else {
                Schema {
                    schema_data: SchemaData::default(),
                    schema_kind: SchemaKind::Type(Type::Number(NumberType::default())),
                }
            }
        }

        serde_json::Value::String(_) => Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        },

        serde_json::Value::Array(arr) => {
            let items = if arr.is_empty() {
                Some(ReferenceOr::Item(Box::new(Schema {
                    schema_data: SchemaData::default(),
                    schema_kind: SchemaKind::Any(AnySchema::default()),
                })))
            } else {
                Some(ReferenceOr::Item(Box::new(value_to_schema(&arr[0]))))
            };
            Schema {
                schema_data: SchemaData::default(),
                schema_kind: SchemaKind::Type(Type::Array(ArrayType {
                    items,
                    min_items: None,
                    max_items: None,
                    unique_items: false,
                })),
            }
        }

        serde_json::Value::Object(map) => {
            if map.is_empty() {
                Schema {
                    schema_data: SchemaData::default(),
                    schema_kind: SchemaKind::Type(Type::Object(ObjectType::default())),
                }
            } else if map.keys().all(|k| is_numeric_string(k)) || map.keys().all(|k| is_uuid(k)) {
                let first_value = map.values().next().unwrap();
                Schema {
                    schema_data: SchemaData::default(),
                    schema_kind: SchemaKind::Type(Type::Object(ObjectType {
                        additional_properties: Some(AdditionalProperties::Schema(Box::new(
                            ReferenceOr::Item(value_to_schema(first_value)),
                        ))),
                        ..ObjectType::default()
                    })),
                }
            } else {
                let properties: IndexMap<String, ReferenceOr<Box<Schema>>> = map
                    .iter()
                    .map(|(key, val)| {
                        (
                            key.clone(),
                            ReferenceOr::Item(Box::new(value_to_schema(val))),
                        )
                    })
                    .collect();
                Schema {
                    schema_data: SchemaData::default(),
                    schema_kind: SchemaKind::Type(Type::Object(ObjectType {
                        properties,
                        ..ObjectType::default()
                    })),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assert_type(schema: &Schema, f: impl FnOnce(&Type)) {
        match &schema.schema_kind {
            SchemaKind::Type(t) => f(t),
            other => panic!("expected SchemaKind::Type, got {:?}", other),
        }
    }

    #[test]
    fn null_produces_nullable_object() {
        let schema = value_to_schema(&json!(null));
        assert!(schema.schema_data.nullable);
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::Object(_)));
        });
    }

    #[test]
    fn true_produces_boolean() {
        let schema = value_to_schema(&json!(true));
        assert!(!schema.schema_data.nullable);
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::Boolean(_)));
        });
    }

    #[test]
    fn false_produces_boolean() {
        let schema = value_to_schema(&json!(false));
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::Boolean(_)));
        });
    }

    #[test]
    fn zero_produces_integer() {
        let schema = value_to_schema(&json!(0));
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::Integer(_)));
        });
    }

    #[test]
    fn positive_int_produces_integer() {
        let schema = value_to_schema(&json!(1));
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::Integer(_)));
        });
    }

    #[test]
    fn negative_int_produces_integer() {
        let schema = value_to_schema(&json!(-5));
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::Integer(_)));
        });
    }

    #[test]
    fn float_produces_number() {
        let schema = value_to_schema(&json!(1.5));
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::Number(_)));
        });
    }

    #[test]
    fn pi_produces_number() {
        let schema = value_to_schema(&json!(1.23));
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::Number(_)));
        });
    }

    #[test]
    fn empty_string_produces_string() {
        let schema = value_to_schema(&json!(""));
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::String(_)));
        });
    }

    #[test]
    fn hello_produces_string() {
        let schema = value_to_schema(&json!("hello"));
        assert_type(&schema, |t| {
            assert!(matches!(t, Type::String(_)));
        });
    }

    #[test]
    fn empty_array_produces_array_with_any_items() {
        let schema = value_to_schema(&json!([]));
        assert_type(&schema, |t| match t {
            Type::Array(arr) => {
                let items = arr.items.as_ref().expect("items should be Some");
                match items {
                    ReferenceOr::Item(boxed) => {
                        assert!(matches!(boxed.schema_kind, SchemaKind::Any(_)));
                    }
                    _ => panic!("expected Item, got Reference"),
                }
            }
            _ => panic!("expected Array"),
        });
    }

    #[test]
    fn array_with_int_produces_array_with_integer_items() {
        let schema = value_to_schema(&json!([1]));
        assert_type(&schema, |t| match t {
            Type::Array(arr) => {
                let items = arr.items.as_ref().expect("items should be Some");
                match items {
                    ReferenceOr::Item(boxed) => {
                        assert!(matches!(
                            boxed.schema_kind,
                            SchemaKind::Type(Type::Integer(_))
                        ));
                    }
                    _ => panic!("expected Item, got Reference"),
                }
            }
            _ => panic!("expected Array"),
        });
    }

    #[test]
    fn mixed_array_uses_first_element_only() {
        let schema = value_to_schema(&json!([1, "a"]));
        assert_type(&schema, |t| match t {
            Type::Array(arr) => {
                let items = arr.items.as_ref().unwrap();
                match items {
                    ReferenceOr::Item(boxed) => {
                        assert!(matches!(
                            boxed.schema_kind,
                            SchemaKind::Type(Type::Integer(_))
                        ));
                    }
                    _ => panic!("expected Item"),
                }
            }
            _ => panic!("expected Array"),
        });
    }

    #[test]
    fn empty_object_produces_object_with_empty_properties() {
        let schema = value_to_schema(&json!({}));
        assert_type(&schema, |t| match t {
            Type::Object(obj) => {
                assert!(obj.properties.is_empty());
                assert!(obj.additional_properties.is_none());
            }
            _ => panic!("expected Object"),
        });
    }

    #[test]
    fn object_with_normal_keys_produces_properties() {
        let schema = value_to_schema(&json!({"a": 1}));
        assert_type(&schema, |t| match t {
            Type::Object(obj) => {
                assert_eq!(obj.properties.len(), 1);
                assert!(obj.properties.contains_key("a"));
                assert!(obj.additional_properties.is_none());

                let prop = &obj.properties["a"];
                match prop {
                    ReferenceOr::Item(boxed) => {
                        assert!(matches!(
                            boxed.schema_kind,
                            SchemaKind::Type(Type::Integer(_))
                        ));
                    }
                    _ => panic!("expected Item"),
                }
            }
            _ => panic!("expected Object"),
        });
    }

    #[test]
    fn all_numeric_keys_produces_additional_properties() {
        let schema = value_to_schema(&json!({"1": "a", "2": "b"}));
        assert_type(&schema, |t| match t {
            Type::Object(obj) => {
                assert!(obj.properties.is_empty());
                match &obj.additional_properties {
                    Some(AdditionalProperties::Schema(boxed_ref)) => match boxed_ref.as_ref() {
                        ReferenceOr::Item(s) => {
                            assert!(matches!(s.schema_kind, SchemaKind::Type(Type::String(_))));
                        }
                        _ => panic!("expected Item"),
                    },
                    other => panic!("expected Schema additionalProperties, got {:?}", other),
                }
            }
            _ => panic!("expected Object"),
        });
    }

    #[test]
    fn all_uuid_keys_produces_additional_properties() {
        let schema = value_to_schema(&json!({"550e8400-e29b-41d4-a716-446655440000": "val"}));
        assert_type(&schema, |t| match t {
            Type::Object(obj) => {
                assert!(obj.properties.is_empty());
                assert!(obj.additional_properties.is_some());
            }
            _ => panic!("expected Object"),
        });
    }

    #[test]
    fn nested_object_array_object() {
        let schema = value_to_schema(&json!({
            "users": [{"id": 1, "name": "test"}]
        }));
        assert_type(&schema, |t| match t {
            Type::Object(outer_obj) => {
                assert_eq!(outer_obj.properties.len(), 1);
                let users_prop = &outer_obj.properties["users"];
                match users_prop {
                    ReferenceOr::Item(users_schema) => match &users_schema.schema_kind {
                        SchemaKind::Type(Type::Array(arr)) => {
                            let items = arr.items.as_ref().unwrap();
                            match items {
                                ReferenceOr::Item(item_schema) => match &item_schema.schema_kind {
                                    SchemaKind::Type(Type::Object(inner_obj)) => {
                                        assert_eq!(inner_obj.properties.len(), 2);
                                        assert!(inner_obj.properties.contains_key("id"));
                                        assert!(inner_obj.properties.contains_key("name"));
                                    }
                                    _ => panic!("expected inner Object"),
                                },
                                _ => panic!("expected Item"),
                            }
                        }
                        _ => panic!("expected Array"),
                    },
                    _ => panic!("expected Item"),
                }
            }
            _ => panic!("expected outer Object"),
        });
    }

    #[test]
    fn null_serializes_correctly() {
        let schema = value_to_schema(&json!(null));
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], "object");
        assert_eq!(json["nullable"], true);
    }

    #[test]
    fn integer_serializes_correctly() {
        let schema = value_to_schema(&json!(42));
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], "integer");
    }

    #[test]
    fn float_serializes_correctly() {
        let schema = value_to_schema(&json!(1.5));
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], "number");
    }

    #[test]
    fn string_serializes_correctly() {
        let schema = value_to_schema(&json!("hello"));
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], "string");
    }

    #[test]
    fn empty_array_serializes_correctly() {
        let schema = value_to_schema(&json!([]));
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], "array");
        assert_eq!(json["items"], json!({}));
    }

    #[test]
    fn object_with_properties_serializes_correctly() {
        let schema = value_to_schema(&json!({"name": "test", "age": 30}));
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], "object");
        assert_eq!(json["properties"]["name"]["type"], "string");
        assert_eq!(json["properties"]["age"]["type"], "integer");
    }

    #[test]
    fn numeric_keys_serializes_with_additional_properties() {
        let schema = value_to_schema(&json!({"1": "a", "2": "b"}));
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], "object");
        assert_eq!(json["additionalProperties"]["type"], "string");
    }

    #[test]
    fn is_numeric_string_valid() {
        assert!(is_numeric_string("0"));
        assert!(is_numeric_string("123"));
        assert!(is_numeric_string("-1"));
        assert!(is_numeric_string("-999"));
    }

    #[test]
    fn is_numeric_string_invalid() {
        assert!(!is_numeric_string(""));
        assert!(!is_numeric_string("abc"));
        assert!(!is_numeric_string("12.3"));
        assert!(!is_numeric_string("1a2"));
        assert!(!is_numeric_string("-"));
    }

    #[test]
    fn is_uuid_valid() {
        assert!(is_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_uuid("00000000-0000-0000-0000-000000000000"));
        assert!(is_uuid("ABCDEF01-2345-6789-abcd-ef0123456789"));
    }

    #[test]
    fn is_uuid_invalid() {
        assert!(!is_uuid(""));
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("550e8400-e29b-41d4-a716"));
        assert!(!is_uuid("550e8400-e29b-41d4-a716-44665544000"));
        assert!(!is_uuid("550e8400-e29b-41d4-a716-4466554400000"));
        assert!(!is_uuid("ZZZZZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZZZZZZZZZ"));
    }
}

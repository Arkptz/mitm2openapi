//! Schema inference: convert JSON values to OpenAPI schemas.
//!
//! Converts `serde_json::Value` into `openapiv3::Schema`, matching the Python
//! mitmproxy2swagger `value_to_schema` behavior exactly.

use indexmap::IndexMap;
use openapiv3::{
    AdditionalProperties, AnySchema, ArrayType, BooleanType, IntegerType, NumberType, ObjectType,
    ReferenceOr, Schema, SchemaData, SchemaKind, StringType, Type,
};

use crate::type_hints::{is_numeric_string, is_uuid};

/// Maximum number of array elements sampled for schema union.
const ARRAY_SAMPLE_LIMIT: usize = 3;

/// Minimum number of dynamic keys required before classifying an object as a dictionary
/// with `additionalProperties` (rather than a fixed-key object).
const MIN_DICT_KEYS: usize = 3;

/// Classify the `SchemaKind` discriminant for deduplication.
fn schema_discriminant(schema: &Schema) -> u8 {
    match &schema.schema_kind {
        SchemaKind::Type(Type::Boolean(_)) => 0,
        SchemaKind::Type(Type::Integer(_)) => 1,
        SchemaKind::Type(Type::Number(_)) => 2,
        SchemaKind::Type(Type::String(_)) => 3,
        SchemaKind::Type(Type::Array(_)) => 4,
        SchemaKind::Type(Type::Object(_)) => 5,
        SchemaKind::Any(_) => 6,
        _ => 7,
    }
}

/// Sample up to `ARRAY_SAMPLE_LIMIT` elements and union their schemas.
/// Returns a single schema if all sampled elements produce the same type,
/// otherwise wraps them in `oneOf`.
fn union_array_elements(arr: &[serde_json::Value], depth: usize) -> Schema {
    let sample_count = arr.len().min(ARRAY_SAMPLE_LIMIT);
    let mut schemas: Vec<Schema> = Vec::with_capacity(sample_count);
    let mut seen: Vec<u8> = Vec::with_capacity(sample_count);

    for elem in arr.iter().take(sample_count) {
        let s = value_to_schema_depth(elem, depth);
        let disc = schema_discriminant(&s);
        if !seen.contains(&disc) {
            seen.push(disc);
            schemas.push(s);
        }
    }

    if schemas.len() == 1 {
        schemas.remove(0)
    } else {
        Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::OneOf {
                one_of: schemas.into_iter().map(ReferenceOr::Item).collect(),
            },
        }
    }
}

/// Union all map values into a single schema (for dict-style objects).
fn union_map_values(map: &serde_json::Map<String, serde_json::Value>, depth: usize) -> Schema {
    let sample_count = map.len().min(ARRAY_SAMPLE_LIMIT);
    let mut schemas: Vec<Schema> = Vec::with_capacity(sample_count);
    let mut seen: Vec<u8> = Vec::with_capacity(sample_count);

    for val in map.values().take(sample_count) {
        let s = value_to_schema_depth(val, depth + 1);
        let disc = schema_discriminant(&s);
        if !seen.contains(&disc) {
            seen.push(disc);
            schemas.push(s);
        }
    }

    if schemas.len() == 1 {
        schemas.remove(0)
    } else {
        Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::OneOf {
                one_of: schemas.into_iter().map(ReferenceOr::Item).collect(),
            },
        }
    }
}

/// Determine whether a JSON object should be treated as a dict (dynamic keys +
/// `additionalProperties`) vs a regular object with known property names.
///
/// Requires that ALL keys are numeric or ALL keys are UUIDs, AND either there are
/// at least `MIN_DICT_KEYS` entries or the values have mixed types (suggesting the
/// keys aren't simply enumerated constants like HTTP status codes).
fn looks_like_dict(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    let all_dynamic = map.keys().all(|k| is_numeric_string(k)) || map.keys().all(|k| is_uuid(k));
    if !all_dynamic {
        return false;
    }
    if map.len() >= MIN_DICT_KEYS {
        return true;
    }
    // Fewer than MIN_DICT_KEYS: only classify as dict if values have mixed types
    let mut first_disc: Option<u8> = None;
    for val in map.values() {
        let disc = json_value_discriminant(val);
        match first_disc {
            None => first_disc = Some(disc),
            Some(prev) if prev != disc => return true,
            _ => {}
        }
    }
    false
}

fn json_value_discriminant(val: &serde_json::Value) -> u8 {
    match val {
        serde_json::Value::Null => 0,
        serde_json::Value::Bool(_) => 1,
        serde_json::Value::Number(_) => 2,
        serde_json::Value::String(_) => 3,
        serde_json::Value::Array(_) => 4,
        serde_json::Value::Object(_) => 5,
    }
}

/// Convert a `serde_json::Value` into an `openapiv3::Schema`.
pub fn value_to_schema(value: &serde_json::Value) -> Schema {
    value_to_schema_depth(value, 0)
}

fn value_to_schema_depth(value: &serde_json::Value, depth: usize) -> Schema {
    if depth >= crate::MAX_SCHEMA_DEPTH {
        return Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Any(AnySchema::default()),
        };
    }
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
                Some(ReferenceOr::Item(Box::new(union_array_elements(
                    arr,
                    depth + 1,
                ))))
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
            } else if looks_like_dict(map) {
                let value_schema = union_map_values(map, depth + 1);
                Schema {
                    schema_data: SchemaData::default(),
                    schema_kind: SchemaKind::Type(Type::Object(ObjectType {
                        additional_properties: Some(AdditionalProperties::Schema(Box::new(
                            ReferenceOr::Item(value_schema),
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
                            ReferenceOr::Item(Box::new(value_to_schema_depth(val, depth + 1))),
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
#[allow(clippy::indexing_slicing)]
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
    fn mixed_array_produces_one_of() {
        let schema = value_to_schema(&json!([1, "a"]));
        assert_type(&schema, |t| match t {
            Type::Array(arr) => {
                let items = arr.items.as_ref().unwrap();
                match items {
                    ReferenceOr::Item(boxed) => {
                        assert!(
                            matches!(boxed.schema_kind, SchemaKind::OneOf { .. }),
                            "mixed array should produce oneOf, got {:?}",
                            boxed.schema_kind
                        );
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
    fn few_numeric_keys_same_type_treated_as_object() {
        // Fewer than MIN_DICT_KEYS with same-type values → treated as fixed-key object
        let schema = value_to_schema(&json!({"1": "a", "2": "b"}));
        assert_type(&schema, |t| match t {
            Type::Object(obj) => {
                assert_eq!(obj.properties.len(), 2);
                assert!(obj.properties.contains_key("1"));
                assert!(obj.properties.contains_key("2"));
                assert!(obj.additional_properties.is_none());
            }
            _ => panic!("expected Object"),
        });
    }

    #[test]
    fn many_numeric_keys_produces_additional_properties() {
        let schema = value_to_schema(&json!({"1": "a", "2": "b", "3": "c"}));
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
        let schema = value_to_schema(&json!({
            "550e8400-e29b-41d4-a716-446655440000": "val1",
            "660e8400-e29b-41d4-a716-446655440001": "val2",
            "770e8400-e29b-41d4-a716-446655440002": "val3"
        }));
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
        // Need 3+ keys to trigger dict classification
        let schema = value_to_schema(&json!({"1": "a", "2": "b", "3": "c"}));
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
    fn deeply_nested_json_caps_at_any_schema() {
        let mut val = json!(null);
        for _ in 0..80 {
            val = json!({ "nested": val });
        }
        let schema = value_to_schema(&val);

        fn find_any(s: &Schema, depth: usize) -> Option<usize> {
            if matches!(s.schema_kind, SchemaKind::Any(_)) {
                return Some(depth);
            }
            if let SchemaKind::Type(Type::Object(obj)) = &s.schema_kind {
                for prop in obj.properties.values() {
                    if let ReferenceOr::Item(inner) = prop {
                        if let Some(d) = find_any(inner, depth + 1) {
                            return Some(d);
                        }
                    }
                }
            }
            None
        }
        let any_depth = find_any(&schema, 0).expect("should have AnySchema at depth limit");
        assert_eq!(any_depth, crate::MAX_SCHEMA_DEPTH);
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

    #[test]
    fn array_union_first_n_elements() {
        let schema = value_to_schema(&json!([1, "two", 3]));
        assert_type(&schema, |t| match t {
            Type::Array(arr) => {
                let items = arr.items.as_ref().unwrap();
                match items {
                    ReferenceOr::Item(boxed) => match &boxed.schema_kind {
                        SchemaKind::OneOf { one_of } => {
                            assert_eq!(one_of.len(), 2, "should have integer + string");
                        }
                        other => panic!("expected oneOf, got {:?}", other),
                    },
                    _ => panic!("expected Item"),
                }
            }
            _ => panic!("expected Array"),
        });
    }

    #[test]
    fn status_keyed_object_not_dict() {
        let schema = value_to_schema(&json!({
            "200": {"description": "OK"},
            "404": {"description": "Not Found"}
        }));
        assert_type(&schema, |t| match t {
            Type::Object(obj) => {
                assert_eq!(obj.properties.len(), 2);
                assert!(obj.properties.contains_key("200"));
                assert!(obj.properties.contains_key("404"));
                assert!(
                    obj.additional_properties.is_none(),
                    "status-code keys should not trigger additionalProperties"
                );
            }
            _ => panic!("expected Object"),
        });
    }
}

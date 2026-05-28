//! Overlay-based enrichments for generated OpenAPI specs.
//!
//! Users provide a YAML overlay file that adds summaries, descriptions, tags,
//! `x-` extensions, response descriptions, component schema descriptions, and
//! top-level info overrides. `apply_enrichments` merges the overlay into a
//! generated `OpenAPI` document.

use anyhow::Result;
use indexmap::IndexMap;
use openapiv3::OpenAPI;
use serde::de::{IgnoredAny, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Overlay {
    pub info: Option<InfoOverlay>,
    #[serde(default)]
    pub operations: HashMap<String, OperationOverlay>,
    pub components: Option<ComponentsOverlay>,
}

#[derive(Debug, Deserialize)]
pub struct InfoOverlay {
    pub title: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OperationOverlay {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub deprecated: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub responses: Option<HashMap<String, ResponseOverlay>>,
    #[serde(flatten, deserialize_with = "deserialize_extensions")]
    pub extensions: IndexMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseOverlay {
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ComponentsOverlay {
    pub schemas: Option<HashMap<String, SchemaOverlay>>,
}

#[derive(Debug, Deserialize)]
pub struct SchemaOverlay {
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum ApplyMode {
    Lenient,
    Strict,
}

fn deserialize_extensions<'de, D>(
    deserializer: D,
) -> Result<IndexMap<String, serde_json::Value>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_map(PredicateVisitor(
        |key: &String| key.starts_with("x-"),
        PhantomData,
    ))
}

struct PredicateVisitor<F, K, V>(F, PhantomData<(K, V)>);

impl<'de, F, K, V> Visitor<'de> for PredicateVisitor<F, K, V>
where
    F: Fn(&K) -> bool,
    K: serde::Deserialize<'de> + Eq + Hash,
    V: serde::Deserialize<'de>,
{
    type Value = IndexMap<K, V>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a map whose fields satisfy a predicate")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut ret = Self::Value::default();
        loop {
            match map.next_key::<K>() {
                Err(_) => (),
                Ok(None) => break,
                Ok(Some(key)) if self.0(&key) => {
                    let _ = ret.insert(key, map.next_value()?);
                }
                Ok(Some(_)) => {
                    let _ = map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(ret)
    }
}

const MAX_OVERLAY_SIZE: u64 = 10 * 1024 * 1024;

pub fn load_overlay(path: &Path) -> Result<Overlay> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_OVERLAY_SIZE {
        anyhow::bail!("overlay file exceeds 10 MiB limit ({} bytes)", meta.len());
    }
    let content = std::fs::read_to_string(path)?;
    let overlay: Overlay = serde_yaml_ng::from_str(&content)?;
    Ok(overlay)
}

pub fn apply_enrichments(_spec: &mut OpenAPI, _overlay: &Overlay, _mode: ApplyMode) -> Result<()> {
    // STUB — will be implemented in Task 3
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_spec() -> OpenAPI {
        serde_yaml_ng::from_str(
            r#"
openapi: "3.0.3"
info:
  title: Test
  version: 1.0.0
paths:
  /fair_price/{symbol}:
    get:
      summary: GET /fair_price/{symbol}
      operationId: getFairPrice
      responses:
        '200':
          description: ''
"#,
        )
        .unwrap()
    }

    #[test]
    fn overlay_summary_and_description_win_over_auto() {
        let mut spec = minimal_spec();
        let overlay: Overlay = serde_yaml_ng::from_str(
            r#"
operations:
  getFairPrice:
    summary: Fair price
    description: Mark price for liquidation
"#,
        )
        .unwrap();
        apply_enrichments(&mut spec, &overlay, ApplyMode::Lenient).unwrap();
        let paths = &spec.paths.paths;
        let path_item = match paths.get("/fair_price/{symbol}").unwrap() {
            openapiv3::ReferenceOr::Item(pi) => pi,
            _ => panic!("expected Item"),
        };
        let op = path_item.get.as_ref().unwrap();
        assert_eq!(op.summary.as_deref(), Some("Fair price"));
        assert_eq!(
            op.description.as_deref(),
            Some("Mark price for liquidation")
        );
    }

    #[test]
    fn operation_not_in_overlay_is_untouched() {
        let mut spec = minimal_spec();
        let overlay: Overlay = serde_yaml_ng::from_str("operations: {}").unwrap();
        apply_enrichments(&mut spec, &overlay, ApplyMode::Lenient).unwrap();
        let path_item = match spec.paths.paths.get("/fair_price/{symbol}").unwrap() {
            openapiv3::ReferenceOr::Item(pi) => pi,
            _ => panic!("expected Item"),
        };
        let op = path_item.get.as_ref().unwrap();
        assert_eq!(op.summary.as_deref(), Some("GET /fair_price/{symbol}"));
    }

    #[test]
    fn x_extensions_are_passed_through_verbatim() {
        let mut spec = minimal_spec();
        let overlay: Overlay = serde_yaml_ng::from_str(
            r#"
operations:
  getFairPrice:
    x-requires-auth: false
    x-rate-limit: "10/s"
    x-error-codes:
      - code: 401
        message: Not logged in
"#,
        )
        .unwrap();
        apply_enrichments(&mut spec, &overlay, ApplyMode::Lenient).unwrap();
        let path_item = match spec.paths.paths.get("/fair_price/{symbol}").unwrap() {
            openapiv3::ReferenceOr::Item(pi) => pi,
            _ => panic!("expected Item"),
        };
        let op = path_item.get.as_ref().unwrap();
        assert_eq!(op.extensions.get("x-requires-auth"), Some(&json!(false)));
        assert_eq!(op.extensions.get("x-rate-limit"), Some(&json!("10/s")));
        assert_eq!(
            op.extensions
                .get("x-error-codes")
                .unwrap()
                .get(0)
                .unwrap()
                .get("code"),
            Some(&json!(401))
        );
    }

    #[test]
    fn unknown_operation_id_ok_in_lenient_mode() {
        let mut spec = minimal_spec();
        let overlay: Overlay = serde_yaml_ng::from_str(
            r#"
operations:
  doesNotExist:
    summary: Ghost
"#,
        )
        .unwrap();
        let result = apply_enrichments(&mut spec, &overlay, ApplyMode::Lenient);
        assert!(result.is_ok());
    }

    #[test]
    fn unknown_operation_id_errors_in_strict_mode() {
        let mut spec = minimal_spec();
        let overlay: Overlay = serde_yaml_ng::from_str(
            r#"
operations:
  doesNotExist:
    summary: Ghost
"#,
        )
        .unwrap();
        let result = apply_enrichments(&mut spec, &overlay, ApplyMode::Strict);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("doesNotExist"),
            "error should mention the unknown opId: {err}"
        );
    }

    #[test]
    fn response_description_per_status_is_merged() {
        let mut spec = minimal_spec();
        let overlay: Overlay = serde_yaml_ng::from_str(
            r#"
operations:
  getFairPrice:
    responses:
      "200":
        description: Fair price payload
"#,
        )
        .unwrap();
        apply_enrichments(&mut spec, &overlay, ApplyMode::Lenient).unwrap();
        let path_item = match spec.paths.paths.get("/fair_price/{symbol}").unwrap() {
            openapiv3::ReferenceOr::Item(pi) => pi,
            _ => panic!("expected Item"),
        };
        let op = path_item.get.as_ref().unwrap();
        let resp = match op
            .responses
            .responses
            .get(&openapiv3::StatusCode::Code(200))
        {
            Some(openapiv3::ReferenceOr::Item(r)) => r,
            other => panic!("expected Item response for 200, got: {other:?}"),
        };
        assert_eq!(resp.description, "Fair price payload");
    }

    #[test]
    fn component_schema_description_set_without_touching_properties() {
        let mut spec: OpenAPI = serde_yaml_ng::from_str(
            r#"
openapi: "3.0.3"
info: { title: T, version: "1" }
paths: {}
components:
  schemas:
    ApiError:
      type: object
      properties:
        code: { type: integer }
        success: { type: boolean }
"#,
        )
        .unwrap();
        let overlay: Overlay = serde_yaml_ng::from_str(
            r#"
components:
  schemas:
    ApiError:
      description: MEXC envelope error
"#,
        )
        .unwrap();
        apply_enrichments(&mut spec, &overlay, ApplyMode::Lenient).unwrap();
        let components = spec.components.as_ref().unwrap();
        let schema_ref = components.schemas.get("ApiError").unwrap();
        if let openapiv3::ReferenceOr::Item(schema) = schema_ref {
            assert_eq!(
                schema.schema_data.description.as_deref(),
                Some("MEXC envelope error")
            );
            // properties survived
            if let openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) = &schema.schema_kind {
                assert!(
                    obj.properties.contains_key("code"),
                    "code property must survive"
                );
                assert!(
                    obj.properties.contains_key("success"),
                    "success property must survive"
                );
            } else {
                panic!("expected Object type");
            }
        } else {
            panic!("expected Item schema");
        }
    }

    #[test]
    fn info_overlay_merges_per_key() {
        let mut spec = minimal_spec();
        let overlay: Overlay = serde_yaml_ng::from_str(
            r#"
info:
  description: Reverse-engineered API
"#,
        )
        .unwrap();
        apply_enrichments(&mut spec, &overlay, ApplyMode::Lenient).unwrap();
        assert_eq!(spec.info.title, "Test"); // untouched
        assert_eq!(
            spec.info.description.as_deref(),
            Some("Reverse-engineered API")
        );
    }
}

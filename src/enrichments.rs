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

fn find_operation_mut<'a>(
    spec: &'a mut OpenAPI,
    target_id: &str,
) -> Option<&'a mut openapiv3::Operation> {
    for (_path, path_ref) in &mut spec.paths.paths {
        if let openapiv3::ReferenceOr::Item(path_item) = path_ref {
            for op in [
                &mut path_item.get,
                &mut path_item.put,
                &mut path_item.post,
                &mut path_item.delete,
                &mut path_item.options,
                &mut path_item.head,
                &mut path_item.patch,
                &mut path_item.trace,
            ]
            .into_iter()
            .flatten()
            {
                if op.operation_id.as_deref() == Some(target_id) {
                    return Some(op);
                }
            }
        }
    }
    None
}

fn apply_operation_enrichment(op: &mut openapiv3::Operation, enrichment: &OperationOverlay) {
    if let Some(s) = &enrichment.summary {
        op.summary = Some(s.clone());
    }
    if let Some(d) = &enrichment.description {
        op.description = Some(d.clone());
    }
    if let Some(d) = enrichment.deprecated {
        op.deprecated = d;
    }
    if let Some(tags) = &enrichment.tags {
        op.tags = tags.clone();
    }

    for (key, val) in &enrichment.extensions {
        op.extensions.insert(key.clone(), val.clone());
    }

    if let Some(responses) = &enrichment.responses {
        for (status_str, resp_overlay) in responses {
            let status_code: u16 = match status_str.parse() {
                Ok(n) => n,
                Err(_) => {
                    tracing::warn!(
                        event = "non_numeric_response_status",
                        status = %status_str,
                        "non-numeric response status in enrichment overlay, skipping"
                    );
                    continue;
                }
            };
            let key = openapiv3::StatusCode::Code(status_code);
            if let Some(openapiv3::ReferenceOr::Item(resp)) = op.responses.responses.get_mut(&key) {
                if let Some(d) = &resp_overlay.description {
                    resp.description = d.clone();
                }
            } else {
                tracing::warn!(
                    event = "enrichment_response_status_not_found",
                    status = %status_str,
                    "response status not found in operation, skipping"
                );
            }
        }
    }
}

pub fn apply_enrichments(spec: &mut OpenAPI, overlay: &Overlay, mode: ApplyMode) -> Result<()> {
    // 1. Info merge
    if let Some(info_overlay) = &overlay.info {
        if let Some(title) = &info_overlay.title {
            spec.info.title = title.clone();
        }
        if let Some(desc) = &info_overlay.description {
            spec.info.description = Some(desc.clone());
        }
        if let Some(ver) = &info_overlay.version {
            spec.info.version = ver.clone();
        }
    }

    // 2. Operations merge
    for (op_id, enrichment) in &overlay.operations {
        let found = find_operation_mut(spec, op_id);
        match found {
            None => match mode {
                ApplyMode::Lenient => {
                    tracing::warn!(
                        event = "unknown_enrichment_operation_id",
                        operation_id = %op_id,
                        "overlay references unknown operationId, skipping"
                    );
                }
                ApplyMode::Strict => {
                    anyhow::bail!("unknown operationId '{op_id}' in enrichments overlay");
                }
            },
            Some(op) => {
                apply_operation_enrichment(op, enrichment);
            }
        }
    }

    // 3. Components merge
    if let Some(comp_overlay) = &overlay.components {
        if let Some(schema_overlays) = &comp_overlay.schemas {
            if let Some(ref mut components) = spec.components {
                for (name, schema_overlay) in schema_overlays {
                    if let Some(schema_ref) = components.schemas.get_mut(name) {
                        if let openapiv3::ReferenceOr::Item(schema) = schema_ref {
                            if let Some(desc) = &schema_overlay.description {
                                schema.schema_data.description = Some(desc.clone());
                            }
                        } else {
                            tracing::warn!(
                                event = "enrichment_schema_is_ref",
                                schema = %name,
                                "schema is a $ref, skipping enrichment"
                            );
                        }
                    }
                }
            } else {
                tracing::warn!(
                    event = "no_components_in_spec",
                    "spec has no components section, skipping component enrichments"
                );
            }
        }
    }

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

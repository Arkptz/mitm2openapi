use indexmap::IndexMap;
use openapiv3::{
    Example, Info, MediaType, OpenAPI, Operation, PathItem, Paths, ReferenceOr, RequestBody,
    Response, Responses, Server, StatusCode,
};
use std::collections::HashMap;
use tracing::{debug, warn};

use crate::params;
use crate::path_matching;
use crate::schema;
use crate::types::{CapturedRequest, Config};

const MAX_FORM_FIELDS: usize = 1000;

pub fn glob_match(pattern: &str, path: &str) -> bool {
    let Ok(glob) = globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
    else {
        return false;
    };
    glob.compile_matcher().is_match(path)
}

/// Discover unique API paths from captured requests and generate templates.
/// Each template is prefixed with "ignore:" — the user removes the prefix for paths they want.
/// Parameterized paths (with numeric/UUID segments) get suggestions like "/users/{id}".
///
/// `exclude_patterns`: paths matching any glob are dropped entirely (not even emitted as `ignore:`).
/// `include_patterns`: paths matching any glob are emitted WITHOUT the `ignore:` prefix
/// (i.e. auto-activated for `generate`). Non-matching paths still get `ignore:` for review.
pub fn discover_paths_streaming(
    requests: impl Iterator<Item = crate::error::Result<Box<dyn CapturedRequest>>>,
    prefix: &str,
    custom_regex: Option<&regex::Regex>,
    exclude_patterns: &[String],
    include_patterns: &[String],
) -> Vec<String> {
    let is_excluded = |path: &str| exclude_patterns.iter().any(|pat| glob_match(pat, path));
    let is_included = |path: &str| include_patterns.iter().any(|pat| glob_match(pat, path));
    let format_template = |path: &str| -> String {
        if is_included(path) {
            path.to_string()
        } else {
            format!("ignore:{}", path)
        }
    };

    let mut seen_paths: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for req_result in requests {
        let req = match req_result {
            Ok(r) => r,
            Err(_) => continue,
        };
        let url = req.get_url();
        if !url.starts_with(prefix) {
            continue;
        }
        let raw_path = &url[prefix.len()..];
        let path_no_query = raw_path.split('?').next().unwrap_or(raw_path);
        let path = if path_no_query.starts_with('/') {
            path_no_query.to_string()
        } else {
            format!("/{}", path_no_query)
        };
        if is_excluded(&path) {
            continue;
        }
        seen_paths.insert(path);
    }

    let paths_vec: Vec<String> = seen_paths.iter().cloned().collect();
    let suggested = path_matching::suggest_param_templates(&paths_vec, custom_regex);

    let mut all: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for p in &paths_vec {
        all.insert(format_template(p));
    }
    for s in &suggested {
        if !is_excluded(s) {
            all.insert(format_template(s));
        }
    }

    all.into_iter().collect()
}

pub fn discover_paths(
    requests: &[Box<dyn CapturedRequest>],
    prefix: &str,
    custom_regex: Option<&regex::Regex>,
    exclude_patterns: &[String],
    include_patterns: &[String],
) -> Vec<String> {
    let is_excluded = |path: &str| exclude_patterns.iter().any(|pat| glob_match(pat, path));
    let is_included = |path: &str| include_patterns.iter().any(|pat| glob_match(pat, path));
    let format_template = |path: &str| -> String {
        if is_included(path) {
            path.to_string()
        } else {
            format!("ignore:{}", path)
        }
    };

    let mut seen_paths: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for req in requests {
        let url = req.get_url();
        if !url.starts_with(prefix) {
            continue;
        }
        let raw_path = &url[prefix.len()..];
        let path_no_query = raw_path.split('?').next().unwrap_or(raw_path);
        let path = if path_no_query.starts_with('/') {
            path_no_query.to_string()
        } else {
            format!("/{}", path_no_query)
        };
        if is_excluded(&path) {
            continue;
        }
        seen_paths.insert(path);
    }

    let paths_vec: Vec<String> = seen_paths.iter().cloned().collect();
    let suggested = path_matching::suggest_param_templates(&paths_vec, custom_regex);

    let mut all: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for p in &paths_vec {
        all.insert(format_template(p));
    }
    for s in &suggested {
        if !is_excluded(s) {
            all.insert(format_template(s));
        }
    }

    all.into_iter().collect()
}

pub struct OpenApiBuilder {
    prefix: String,
    config: Config,
    tag_strategy: crate::tag_rules::TagStrategy,
    tags_overrides: Option<serde_json::Map<String, serde_json::Value>>,
    compiled_templates: path_matching::CompiledTemplates,
    spec: OpenAPI,
    examples_store: HashMap<(String, String, u16), Vec<(String, serde_json::Value)>>,
    req_examples_store: HashMap<(String, String, String), Vec<(String, serde_json::Value)>>,
    max_examples: usize,
    redactor: Option<crate::redact::Redactor>,
}

fn extract_tag(
    path: &str,
    overrides: &Option<serde_json::Map<String, serde_json::Value>>,
) -> Option<String> {
    let first_segment = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .filter(|s| !s.is_empty() && !s.starts_with('{'))?;

    if let Some(map) = overrides {
        if let Some(val) = map.get(first_segment) {
            return val.as_str().map(|s| s.to_string());
        }
    }

    Some(first_segment.to_string())
}

fn parse_tags_overrides(
    json_str: &Option<String>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let raw = json_str.as_ref()?;
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => v.as_object().cloned(),
        Err(err) => {
            warn!(
                event = "invalid_tags_overrides",
                input = %raw,
                error = %err,
                "ignoring malformed --tags-overrides JSON"
            );
            None
        }
    }
}

fn is_image_content_type(ct: Option<&str>) -> bool {
    ct.is_some_and(|s| s.to_lowercase().starts_with("image/"))
}

fn is_binary_content_type(ct: Option<&str>) -> bool {
    ct.is_some_and(|s| {
        let lower = s.to_lowercase();
        lower.starts_with("image/") || lower == "application/octet-stream"
    })
}

fn make_example_name(val: &serde_json::Value, existing: &[String]) -> String {
    let base = val
        .as_object()
        .and_then(|obj| {
            obj.iter().filter_map(|(_, v)| v.as_str()).next().map(|s| {
                s.chars()
                    .take(32)
                    .map(|c| if c.is_alphanumeric() { c } else { '_' })
                    .collect::<String>()
            })
        })
        .filter(|s| !s.is_empty());

    let base = match base {
        Some(b) => b,
        None => {
            let n = existing.len() + 1;
            return format!("response_{n}");
        }
    };

    if !existing.contains(&base) {
        return base;
    }
    let mut i = 2;
    loop {
        let candidate = format!("{base}_{i}");
        if !existing.contains(&candidate) {
            return candidate;
        }
        i += 1;
    }
}

fn host_from_prefix(prefix: &str) -> String {
    prefix
        .strip_prefix("https://")
        .or_else(|| prefix.strip_prefix("http://"))
        .unwrap_or(prefix)
        .split('/')
        .next()
        .unwrap_or(prefix)
        .to_string()
}

/// Try to parse a body as a `serde_json::Value` based on content type.
///
/// Cascade: JSON → msgpack → form-urlencoded → None.
fn parse_body(body: &[u8], content_type: Option<&str>) -> Option<(String, serde_json::Value)> {
    let ct = content_type.unwrap_or("");
    let ct_lower = ct.to_lowercase();

    if ct_lower.contains("json") {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) {
            return Some(("application/json".to_string(), val));
        }
    }

    if ct_lower.contains("msgpack") {
        if let Ok(val) = rmp_serde::from_slice::<serde_json::Value>(body) {
            return Some(("application/msgpack".to_string(), val));
        }
    }

    if ct_lower.contains("form-urlencoded") {
        if let Ok(body_str) = std::str::from_utf8(body) {
            let mut map = serde_json::Map::new();
            let mut count = 0usize;
            for pair in body_str.split('&') {
                if count >= MAX_FORM_FIELDS {
                    warn!(
                        event = "form_fields_truncated",
                        max = MAX_FORM_FIELDS,
                        "form-urlencoded body exceeds field limit, truncating"
                    );
                    break;
                }
                if let Some((k, v)) = pair.split_once('=') {
                    map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
                    count += 1;
                }
            }
            if !map.is_empty() {
                return Some((
                    "application/x-www-form-urlencoded".to_string(),
                    serde_json::Value::Object(map),
                ));
            }
        }
    }

    None
}

/// Get the method-specific operation slot from a PathItem (mutable).
/// Returns `None` for HTTP methods not supported by the OpenAPI spec.
fn get_operation_mut<'a>(
    path_item: &'a mut PathItem,
    method: &str,
) -> Option<&'a mut Option<Operation>> {
    match method.to_uppercase().as_str() {
        "GET" => Some(&mut path_item.get),
        "PUT" => Some(&mut path_item.put),
        "POST" => Some(&mut path_item.post),
        "DELETE" => Some(&mut path_item.delete),
        "OPTIONS" => Some(&mut path_item.options),
        "HEAD" => Some(&mut path_item.head),
        "PATCH" => Some(&mut path_item.patch),
        "TRACE" => Some(&mut path_item.trace),
        _ => None,
    }
}

/// Check if a method operation already exists on a PathItem.
fn has_operation(path_item: &PathItem, method: &str) -> bool {
    match method.to_uppercase().as_str() {
        "GET" => path_item.get.is_some(),
        "PUT" => path_item.put.is_some(),
        "POST" => path_item.post.is_some(),
        "DELETE" => path_item.delete.is_some(),
        "OPTIONS" => path_item.options.is_some(),
        "HEAD" => path_item.head.is_some(),
        "PATCH" => path_item.patch.is_some(),
        "TRACE" => path_item.trace.is_some(),
        _ => false,
    }
}

fn merge_response_content(existing: &mut Response, incoming: &Response) {
    for (media_type, incoming_mt) in &incoming.content {
        if let Some(existing_mt) = existing.content.get_mut(media_type) {
            let existing_schema = existing_mt.schema.take();
            let incoming_schema = incoming_mt.schema.clone();
            existing_mt.schema = match (existing_schema, incoming_schema) {
                (Some(a), Some(b)) => Some(merge_schemas_one_of(a, b)),
                (Some(a), None) => Some(a),
                (None, b) => b,
            };
        } else {
            existing
                .content
                .insert(media_type.clone(), incoming_mt.clone());
        }
    }
}

fn merge_request_body_content(existing: &mut RequestBody, incoming: &RequestBody) {
    for (media_type, incoming_mt) in &incoming.content {
        if let Some(existing_mt) = existing.content.get_mut(media_type) {
            let existing_schema = existing_mt.schema.take();
            let incoming_schema = incoming_mt.schema.clone();
            existing_mt.schema = match (existing_schema, incoming_schema) {
                (Some(a), Some(b)) => Some(merge_schemas_one_of(a, b)),
                (Some(a), None) => Some(a),
                (None, b) => b,
            };
        } else {
            existing
                .content
                .insert(media_type.clone(), incoming_mt.clone());
        }
    }
}

fn merge_schemas_one_of(
    a: ReferenceOr<openapiv3::Schema>,
    b: ReferenceOr<openapiv3::Schema>,
) -> ReferenceOr<openapiv3::Schema> {
    if schema_refs_equal(&a, &b) {
        return a;
    }
    let mut variants = Vec::new();
    collect_one_of_variants(a, &mut variants);
    collect_one_of_variants(b, &mut variants);
    dedup_schema_variants(&mut variants);

    if variants.len() == 1 {
        return variants.pop().unwrap();
    }

    ReferenceOr::Item(openapiv3::Schema {
        schema_data: openapiv3::SchemaData::default(),
        schema_kind: openapiv3::SchemaKind::OneOf { one_of: variants },
    })
}

fn collect_one_of_variants(
    schema_ref: ReferenceOr<openapiv3::Schema>,
    out: &mut Vec<ReferenceOr<openapiv3::Schema>>,
) {
    if let ReferenceOr::Item(ref s) = schema_ref {
        if let openapiv3::SchemaKind::OneOf { ref one_of } = s.schema_kind {
            for v in one_of {
                out.push(v.clone());
            }
            return;
        }
    }
    out.push(schema_ref);
}

fn dedup_schema_variants(variants: &mut Vec<ReferenceOr<openapiv3::Schema>>) {
    let mut i = 0;
    while i < variants.len() {
        let Some(anchor) = variants.get(i).cloned() else {
            break;
        };
        let mut j = i + 1;
        while j < variants.len() {
            let same = variants
                .get(j)
                .map(|candidate| schema_refs_equal(&anchor, candidate))
                .unwrap_or(false);
            if same {
                variants.remove(j);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}

fn schema_refs_equal(
    a: &ReferenceOr<openapiv3::Schema>,
    b: &ReferenceOr<openapiv3::Schema>,
) -> bool {
    match (serde_json::to_value(a), serde_json::to_value(b)) {
        (Ok(va), Ok(vb)) => va == vb,
        _ => false,
    }
}

impl OpenApiBuilder {
    pub fn new(prefix: &str, config: &Config, templates: Vec<String>) -> Self {
        let host = host_from_prefix(prefix);
        let title = config
            .openapi_title
            .clone()
            .unwrap_or_else(|| format!("{} API", host));

        let spec = OpenAPI {
            openapi: "3.0.3".to_string(),
            info: Info {
                title,
                version: config.openapi_version.clone(),
                ..Info::default()
            },
            servers: vec![Server {
                url: prefix.to_string(),
                ..Server::default()
            }],
            paths: Paths::default(),
            ..OpenAPI::default()
        };

        let tags_overrides = parse_tags_overrides(&config.tags_overrides);
        let compiled_templates =
            path_matching::CompiledTemplates::new(&templates).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to compile templates, using empty set");
                path_matching::CompiledTemplates::new(&[]).unwrap()
            });

        let redactor = if !config.redact_patterns.is_empty() || !config.redact_fields.is_empty() {
            match crate::redact::Redactor::new(&config.redact_patterns, &config.redact_fields) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to compile redact patterns, skipping redaction");
                    None
                }
            }
        } else {
            None
        };

        let tag_strategy = config.tag_strategy.clone();

        Self {
            prefix: prefix.to_string(),
            config: config.clone(),
            tag_strategy,
            tags_overrides,
            compiled_templates,
            spec,
            examples_store: HashMap::new(),
            req_examples_store: HashMap::new(),
            max_examples: config.max_examples,
            redactor,
        }
    }

    pub fn add_request(&mut self, request: &dyn CapturedRequest) {
        let url = request.get_url();
        let method = request.get_method().to_uppercase();

        if self.config.skip_options && method == "OPTIONS" {
            return;
        }

        if !matches!(
            method.as_str(),
            "GET" | "PUT" | "POST" | "DELETE" | "OPTIONS" | "HEAD" | "PATCH" | "TRACE"
        ) {
            warn!(
                event = "unknown_http_method",
                method = %method,
                url = %url,
                "skipping request with unknown HTTP method"
            );
            return;
        }

        if !url.starts_with(&self.prefix) {
            return;
        }

        if self.config.ignore_images && is_image_content_type(request.get_response_content_type()) {
            debug!(url, "Skipping image request");
            return;
        }

        let raw_path = &url[self.prefix.len()..];
        let path_no_query = raw_path.split('?').next().unwrap_or(raw_path);
        let path = if path_no_query.starts_with('/') {
            path_no_query.to_string()
        } else {
            format!("/{}", path_no_query)
        };

        let template_path = if self.compiled_templates.is_empty() {
            path.clone()
        } else {
            match self.compiled_templates.match_path(&path) {
                Some(t) => t.to_string(),
                None => return,
            }
        };

        // Build the new response for this request
        let status_code = request.get_response_status_code().unwrap_or(200);
        let reason = request.get_response_reason().unwrap_or("OK").to_string();

        let mut new_response = Response {
            description: reason,
            ..Response::default()
        };

        if let Some(resp_body) = request.get_response_body() {
            let resp_ct = request.get_response_content_type();
            if !is_binary_content_type(resp_ct) {
                if let Some((media_type_str, val)) = parse_body(resp_body, resp_ct) {
                    let resp_schema = schema::value_to_schema(&val);
                    let mut content = IndexMap::new();
                    content.insert(
                        media_type_str,
                        MediaType {
                            schema: Some(ReferenceOr::Item(resp_schema)),
                            ..MediaType::default()
                        },
                    );
                    new_response.content = content;

                    let key = (template_path.clone(), method.clone(), status_code);
                    let entries = self.examples_store.entry(key).or_default();
                    if self.max_examples > 0 && entries.len() < self.max_examples {
                        let existing_names: Vec<String> =
                            entries.iter().map(|(n, _)| n.clone()).collect();
                        let name = make_example_name(&val, &existing_names);
                        entries.push((name, val));
                    }
                }
            }
        }

        // If operation already exists for this (path, method), merge response only
        if let Some(ReferenceOr::Item(existing)) = self.spec.paths.paths.get_mut(&template_path) {
            if has_operation(existing, &method) {
                if let Some(Some(op)) = get_operation_mut(existing, &method).map(|s| s.as_mut()) {
                    let sc = StatusCode::Code(status_code);
                    if let Some(existing_resp_ref) = op.responses.responses.get_mut(&sc) {
                        // Same status code exists — merge schemas via oneOf
                        if let ReferenceOr::Item(existing_resp) = existing_resp_ref {
                            merge_response_content(existing_resp, &new_response);
                        }
                    } else {
                        // New status code — add directly
                        op.responses
                            .responses
                            .insert(sc, ReferenceOr::Item(new_response));
                    }

                    if let Some(req_body) = request.get_request_body() {
                        let req_ct = request
                            .get_request_headers()
                            .iter()
                            .find(|(k, _)| k.to_lowercase() == "content-type")
                            .map(|(_, v)| v.as_str());

                        if let Some((media_type_str, val)) = parse_body(req_body, req_ct) {
                            let schema = schema::value_to_schema(&val);
                            let mut incoming_content = IndexMap::new();
                            incoming_content.insert(
                                media_type_str.clone(),
                                MediaType {
                                    schema: Some(ReferenceOr::Item(schema)),
                                    ..MediaType::default()
                                },
                            );
                            let incoming_rb = RequestBody {
                                content: incoming_content,
                                required: true,
                                ..RequestBody::default()
                            };

                            match &mut op.request_body {
                                Some(ReferenceOr::Item(existing_rb)) => {
                                    merge_request_body_content(existing_rb, &incoming_rb);
                                }
                                _ => {
                                    op.request_body = Some(ReferenceOr::Item(incoming_rb));
                                }
                            }

                            let req_key = (template_path.clone(), method.clone(), media_type_str);
                            let req_entries = self.req_examples_store.entry(req_key).or_default();
                            let existing_names: Vec<String> =
                                req_entries.iter().map(|(n, _)| n.clone()).collect();
                            let name = make_example_name(&val, &existing_names);
                            req_entries.push((name, val));
                        }
                    }
                }
                return;
            }
        }

        // New operation — build from scratch
        let mut operation = Operation {
            summary: Some(format!("{} {}", method, template_path)),
            ..Operation::default()
        };

        match &self.tag_strategy {
            crate::tag_rules::TagStrategy::Legacy => {
                if let Some(tag) = extract_tag(&template_path, &self.tags_overrides) {
                    operation.tags = vec![tag];
                }
            }
            crate::tag_rules::TagStrategy::None => {
                // suppress tags — leave operation.tags empty
            }
            crate::tag_rules::TagStrategy::PathSegment { .. }
            | crate::tag_rules::TagStrategy::Rules { .. } => {
                if let Some(tag) = crate::tag_rules::resolve_tag(&self.tag_strategy, &template_path)
                {
                    operation.tags = vec![tag];
                }
            }
        }

        if !self.config.suppress_params {
            let mut parameters: Vec<ReferenceOr<openapiv3::Parameter>> = Vec::new();

            for p in params::extract_path_params(&template_path) {
                parameters.push(ReferenceOr::Item(p));
            }
            for p in params::extract_query_params(url) {
                parameters.push(ReferenceOr::Item(p));
            }

            if self.config.include_headers {
                for p in params::extract_header_params(
                    request.get_request_headers(),
                    &self.config.exclude_headers,
                ) {
                    parameters.push(ReferenceOr::Item(p));
                }
            }

            operation.parameters = parameters;
        }

        if let Some(req_body) = request.get_request_body() {
            let req_ct = request
                .get_request_headers()
                .iter()
                .find(|(k, _)| k.to_lowercase() == "content-type")
                .map(|(_, v)| v.as_str());

            if let Some((media_type_str, val)) = parse_body(req_body, req_ct) {
                let schema = schema::value_to_schema(&val);
                let mut content = IndexMap::new();
                content.insert(
                    media_type_str.clone(),
                    MediaType {
                        schema: Some(ReferenceOr::Item(schema)),
                        ..MediaType::default()
                    },
                );
                operation.request_body = Some(ReferenceOr::Item(RequestBody {
                    content,
                    required: true,
                    ..RequestBody::default()
                }));

                let req_key = (template_path.clone(), method.clone(), media_type_str);
                let req_entries = self.req_examples_store.entry(req_key).or_default();
                let existing_names: Vec<String> =
                    req_entries.iter().map(|(n, _)| n.clone()).collect();
                let name = make_example_name(&val, &existing_names);
                req_entries.push((name, val));
            }
        }

        let mut responses = IndexMap::new();
        responses.insert(
            StatusCode::Code(status_code),
            ReferenceOr::Item(new_response),
        );
        operation.responses = Responses {
            responses,
            ..Responses::default()
        };

        let path_item = self
            .spec
            .paths
            .paths
            .entry(template_path)
            .or_insert_with(|| ReferenceOr::Item(PathItem::default()));

        if let ReferenceOr::Item(ref mut item) = path_item {
            if let Some(slot) = get_operation_mut(item, &method) {
                *slot = Some(operation);
            }
        }
    }

    /// Process multiple requests.
    pub fn add_requests(&mut self, requests: &[Box<dyn CapturedRequest>]) {
        for req in requests {
            self.add_request(req.as_ref());
        }
    }

    /// Get the assembled OpenAPI spec.
    pub fn build(mut self) -> OpenAPI {
        for ((path, method, status), examples) in self.examples_store.drain() {
            let Some(ReferenceOr::Item(path_item)) = self.spec.paths.paths.get_mut(&path) else {
                continue;
            };
            let Some(Some(op)) = get_operation_mut(path_item, &method).map(|s| s.as_mut()) else {
                continue;
            };
            let Some(ReferenceOr::Item(resp)) =
                op.responses.responses.get_mut(&StatusCode::Code(status))
            else {
                continue;
            };
            let Some(media_type) = resp.content.values_mut().next() else {
                continue;
            };
            let mut ex_map: IndexMap<String, ReferenceOr<Example>> = IndexMap::new();
            for (name, mut value) in examples {
                if let Some(r) = &self.redactor {
                    r.redact(&mut value);
                    let existing: Vec<String> = ex_map.keys().cloned().collect();
                    let new_name = make_example_name(&value, &existing);
                    ex_map.insert(
                        new_name,
                        ReferenceOr::Item(Example {
                            value: Some(value),
                            ..Example::default()
                        }),
                    );
                } else {
                    ex_map.insert(
                        name,
                        ReferenceOr::Item(Example {
                            value: Some(value),
                            ..Example::default()
                        }),
                    );
                }
            }
            media_type.examples = ex_map;
        }
        for ((path, method, content_type), examples) in self.req_examples_store.drain() {
            let Some(ReferenceOr::Item(path_item)) = self.spec.paths.paths.get_mut(&path) else {
                continue;
            };
            let Some(Some(op)) = get_operation_mut(path_item, &method).map(|s| s.as_mut()) else {
                continue;
            };
            let Some(ReferenceOr::Item(rb)) = op.request_body.as_mut() else {
                continue;
            };
            let Some(media_type) = rb.content.get_mut(&content_type) else {
                continue;
            };
            let mut ex_map: IndexMap<String, ReferenceOr<Example>> = IndexMap::new();
            for (name, mut value) in examples {
                if let Some(r) = &self.redactor {
                    r.redact(&mut value);
                    let existing: Vec<String> = ex_map.keys().cloned().collect();
                    let new_name = make_example_name(&value, &existing);
                    ex_map.insert(
                        new_name,
                        ReferenceOr::Item(Example {
                            value: Some(value),
                            ..Example::default()
                        }),
                    );
                } else {
                    ex_map.insert(
                        name,
                        ReferenceOr::Item(Example {
                            value: Some(value),
                            ..Example::default()
                        }),
                    );
                }
            }
            media_type.examples = ex_map;
        }
        self.spec
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Test helper: a simple CapturedRequest implementation.
    struct MockRequest {
        url: String,
        method: String,
        request_headers: Vec<(String, String)>,
        request_body: Option<Vec<u8>>,
        response_status: Option<u16>,
        response_reason: Option<String>,
        response_headers: Option<Vec<(String, String)>>,
        response_body: Option<Vec<u8>>,
        response_content_type: Option<String>,
    }

    impl MockRequest {
        fn get(url: &str) -> Self {
            Self {
                url: url.to_string(),
                method: "GET".to_string(),
                request_headers: vec![],
                request_body: None,
                response_status: Some(200),
                response_reason: Some("OK".to_string()),
                response_headers: None,
                response_body: None,
                response_content_type: None,
            }
        }

        fn with_json_response(mut self, body: &serde_json::Value) -> Self {
            self.response_body = Some(serde_json::to_vec(body).unwrap());
            self.response_content_type = Some("application/json".to_string());
            self
        }

        fn with_status(mut self, code: u16, reason: &str) -> Self {
            self.response_status = Some(code);
            self.response_reason = Some(reason.to_string());
            self
        }

        fn post(url: &str) -> Self {
            Self {
                url: url.to_string(),
                method: "POST".to_string(),
                request_headers: vec![("Content-Type".to_string(), "application/json".to_string())],
                request_body: None,
                response_status: Some(201),
                response_reason: Some("Created".to_string()),
                response_headers: None,
                response_body: None,
                response_content_type: None,
            }
        }

        fn with_json_request_body(mut self, body: &serde_json::Value) -> Self {
            self.request_body = Some(serde_json::to_vec(body).unwrap());
            self
        }
    }

    impl CapturedRequest for MockRequest {
        fn get_url(&self) -> &str {
            &self.url
        }
        fn get_method(&self) -> &str {
            &self.method
        }
        fn get_request_headers(&self) -> &[(String, String)] {
            &self.request_headers
        }
        fn get_request_body(&self) -> Option<&[u8]> {
            self.request_body.as_deref()
        }
        fn get_response_status_code(&self) -> Option<u16> {
            self.response_status
        }
        fn get_response_reason(&self) -> Option<&str> {
            self.response_reason.as_deref()
        }
        fn get_response_headers(&self) -> Option<&[(String, String)]> {
            self.response_headers.as_deref()
        }
        fn get_response_body(&self) -> Option<&[u8]> {
            self.response_body.as_deref()
        }
        fn get_response_content_type(&self) -> Option<&str> {
            self.response_content_type.as_deref()
        }
    }

    fn test_config() -> Config {
        Config {
            prefix: "https://api.example.com".to_string(),
            openapi_version: "1.0.0".to_string(),
            max_examples: 5,
            ..Default::default()
        }
    }

    // ── host_from_prefix ───────────────────────────────────────────

    #[test]
    fn host_from_https_prefix() {
        assert_eq!(
            host_from_prefix("https://api.example.com/api"),
            "api.example.com"
        );
    }

    #[test]
    fn host_from_http_prefix() {
        assert_eq!(
            host_from_prefix("http://localhost:8080/v1"),
            "localhost:8080"
        );
    }

    #[test]
    fn host_from_bare_prefix() {
        assert_eq!(host_from_prefix("example.com/api"), "example.com");
    }

    // ── parse_body ─────────────────────────────────────────────────

    #[test]
    fn parse_body_json() {
        let body = br#"{"key": "value"}"#;
        let (ct, val) = parse_body(body, Some("application/json")).unwrap();
        assert_eq!(ct, "application/json");
        assert_eq!(val["key"], "value");
    }

    #[test]
    fn parse_body_form_urlencoded() {
        let body = b"name=test&age=30";
        let (ct, val) = parse_body(body, Some("application/x-www-form-urlencoded")).unwrap();
        assert_eq!(ct, "application/x-www-form-urlencoded");
        assert_eq!(val["name"], "test");
        assert_eq!(val["age"], "30");
    }

    #[test]
    fn parse_body_form_fields_cap() {
        let mut pairs: Vec<String> = Vec::new();
        for i in 0..MAX_FORM_FIELDS + 100 {
            pairs.push(format!("key{i}=val{i}"));
        }
        let body_str = pairs.join("&");
        let (_, val) = parse_body(
            body_str.as_bytes(),
            Some("application/x-www-form-urlencoded"),
        )
        .unwrap();
        let obj = val.as_object().unwrap();
        assert_eq!(
            obj.len(),
            MAX_FORM_FIELDS,
            "form fields should be capped at {MAX_FORM_FIELDS}"
        );
    }

    #[test]
    fn parse_body_unknown_ct_returns_none() {
        let body = b"some binary data";
        assert!(parse_body(body, Some("application/octet-stream")).is_none());
    }

    #[test]
    fn parse_body_msgpack() {
        let val = serde_json::json!({"hello": "world"});
        let body = rmp_serde::to_vec(&val).unwrap();
        let (ct, parsed) = parse_body(&body, Some("application/msgpack")).unwrap();
        assert_eq!(ct, "application/msgpack");
        assert_eq!(parsed["hello"], "world");
    }

    // ── OpenApiBuilder::new ────────────────────────────────────────

    #[test]
    fn builder_new_sets_metadata() {
        let config = test_config();
        let builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);
        let spec = builder.build();

        assert_eq!(spec.openapi, "3.0.3");
        assert_eq!(spec.info.title, "api.example.com API");
        assert_eq!(spec.info.version, "1.0.0");
        assert_eq!(spec.servers.len(), 1);
        assert_eq!(spec.servers[0].url, "https://api.example.com");
    }

    #[test]
    fn builder_new_custom_title() {
        let mut config = test_config();
        config.openapi_title = Some("My Custom API".to_string());
        let builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);
        let spec = builder.build();
        assert_eq!(spec.info.title, "My Custom API");
    }

    // ── Simple GET request ─────────────────────────────────────────

    #[test]
    fn simple_get_request() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::get("https://api.example.com/users")
            .with_json_response(&serde_json::json!([{"id": 1, "name": "Alice"}]));

        builder.add_request(&req);
        let spec = builder.build();

        // Verify path exists
        assert!(spec.paths.paths.contains_key("/users"));
        let path_item = spec.paths.paths["/users"].as_item().unwrap();

        // Verify GET operation
        let get_op = path_item.get.as_ref().unwrap();
        assert_eq!(get_op.summary.as_deref(), Some("GET /users"));

        // Verify response
        let resp = get_op
            .responses
            .responses
            .get(&StatusCode::Code(200))
            .unwrap()
            .as_item()
            .unwrap();
        assert_eq!(resp.description, "OK");
        assert!(resp.content.contains_key("application/json"));

        // Verify response schema is array
        let media = &resp.content["application/json"];
        let schema = media.schema.as_ref().unwrap().as_item().unwrap();
        assert!(matches!(
            schema.schema_kind,
            openapiv3::SchemaKind::Type(openapiv3::Type::Array(_))
        ));
    }

    // ── POST request with JSON body ────────────────────────────────

    #[test]
    fn post_request_with_json_body() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::post("https://api.example.com/users")
            .with_json_request_body(&serde_json::json!({"name": "Bob", "email": "bob@test.com"}))
            .with_json_response(&serde_json::json!({"id": 2, "name": "Bob"}))
            .with_status(201, "Created");

        builder.add_request(&req);
        let spec = builder.build();

        let path_item = spec.paths.paths["/users"].as_item().unwrap();
        let post_op = path_item.post.as_ref().unwrap();

        // Verify request body
        let req_body = post_op.request_body.as_ref().unwrap().as_item().unwrap();
        assert!(req_body.required);
        assert!(req_body.content.contains_key("application/json"));

        let req_schema = req_body.content["application/json"]
            .schema
            .as_ref()
            .unwrap()
            .as_item()
            .unwrap();
        match &req_schema.schema_kind {
            openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) => {
                assert!(obj.properties.contains_key("name"));
                assert!(obj.properties.contains_key("email"));
            }
            other => panic!("expected Object schema, got {:?}", other),
        }

        // Verify response
        let resp = post_op
            .responses
            .responses
            .get(&StatusCode::Code(201))
            .unwrap()
            .as_item()
            .unwrap();
        assert_eq!(resp.description, "Created");
    }

    // ── Same status code merges via oneOf ─────────────────────────

    #[test]
    fn same_status_identical_schema_no_one_of() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req1 = MockRequest::get("https://api.example.com/users")
            .with_json_response(&serde_json::json!({"version": 1}));
        let req2 = MockRequest::get("https://api.example.com/users")
            .with_json_response(&serde_json::json!({"version": 2}));

        builder.add_request(&req1);
        builder.add_request(&req2);
        let spec = builder.build();

        let path_item = spec.paths.paths["/users"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();

        let resp = get_op
            .responses
            .responses
            .get(&StatusCode::Code(200))
            .unwrap()
            .as_item()
            .unwrap();
        let media = &resp.content["application/json"];
        let schema = media.schema.as_ref().unwrap().as_item().unwrap();
        match &schema.schema_kind {
            openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) => {
                assert!(obj.properties.contains_key("version"));
            }
            other => panic!(
                "expected Object (identical schemas should not produce oneOf), got {:?}",
                other
            ),
        }
    }

    #[test]
    fn same_status_divergent_schemas_one_of() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req1 = MockRequest::get("https://api.example.com/users")
            .with_json_response(&serde_json::json!({"name": "Alice"}));
        let req2 = MockRequest::get("https://api.example.com/users")
            .with_json_response(&serde_json::json!({"error": "not found"}));

        builder.add_request(&req1);
        builder.add_request(&req2);
        let spec = builder.build();

        let path_item = spec.paths.paths["/users"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();

        let resp = get_op
            .responses
            .responses
            .get(&StatusCode::Code(200))
            .unwrap()
            .as_item()
            .unwrap();
        let media = &resp.content["application/json"];
        let schema = media.schema.as_ref().unwrap().as_item().unwrap();
        assert!(
            matches!(schema.schema_kind, openapiv3::SchemaKind::OneOf { .. }),
            "divergent schemas should produce oneOf, got {:?}",
            schema.schema_kind
        );
        if let openapiv3::SchemaKind::OneOf { one_of } = &schema.schema_kind {
            assert_eq!(one_of.len(), 2, "should have exactly 2 variants");
        }
    }

    #[test]
    fn multiple_status_codes_merged() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req1 = MockRequest::post("https://api.example.com/users")
            .with_json_request_body(&serde_json::json!({"name": "Bob"}))
            .with_json_response(&serde_json::json!({"id": 1}))
            .with_status(200, "OK");
        let req2 = MockRequest::post("https://api.example.com/users")
            .with_json_request_body(&serde_json::json!({"name": ""}))
            .with_json_response(&serde_json::json!({"error": "invalid"}))
            .with_status(400, "Bad Request");

        builder.add_request(&req1);
        builder.add_request(&req2);
        let spec = builder.build();

        let path_item = spec.paths.paths["/users"].as_item().unwrap();
        let post_op = path_item.post.as_ref().unwrap();

        assert!(
            post_op
                .responses
                .responses
                .contains_key(&StatusCode::Code(200)),
            "should have 200 response"
        );
        assert!(
            post_op
                .responses
                .responses
                .contains_key(&StatusCode::Code(400)),
            "should have 400 response"
        );

        let resp_400 = post_op
            .responses
            .responses
            .get(&StatusCode::Code(400))
            .unwrap()
            .as_item()
            .unwrap();
        assert_eq!(resp_400.description, "Bad Request");
        assert!(resp_400.content.contains_key("application/json"));
    }

    // ── Different methods on same path don't conflict ──────────────

    #[test]
    fn different_methods_same_path() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let get_req = MockRequest::get("https://api.example.com/users")
            .with_json_response(&serde_json::json!([]));
        let post_req = MockRequest::post("https://api.example.com/users")
            .with_json_request_body(&serde_json::json!({"name": "test"}))
            .with_json_response(&serde_json::json!({"id": 1}))
            .with_status(201, "Created");

        builder.add_request(&get_req);
        builder.add_request(&post_req);
        let spec = builder.build();

        let path_item = spec.paths.paths["/users"].as_item().unwrap();
        assert!(path_item.get.is_some());
        assert!(path_item.post.is_some());
    }

    // ── URL prefix filtering ───────────────────────────────────────

    #[test]
    fn prefix_filtering_skips_non_matching() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::get("https://other.example.com/users")
            .with_json_response(&serde_json::json!([]));

        builder.add_request(&req);
        let spec = builder.build();

        assert!(spec.paths.paths.is_empty());
    }

    // ── Template matching ──────────────────────────────────────────

    #[test]
    fn template_matching_parameterizes_paths() {
        let config = test_config();
        let templates = vec!["/users/{id}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        let req = MockRequest::get("https://api.example.com/users/123")
            .with_json_response(&serde_json::json!({"id": 123, "name": "Alice"}));

        builder.add_request(&req);
        let spec = builder.build();

        // Should be stored under the template path, not the raw path
        assert!(spec.paths.paths.contains_key("/users/{id}"));
        assert!(!spec.paths.paths.contains_key("/users/123"));

        // Should have path parameter
        let path_item = spec.paths.paths["/users/{id}"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();
        assert!(!get_op.parameters.is_empty());

        let param = get_op.parameters[0].as_item().unwrap();
        assert_eq!(param.parameter_data_ref().name, "id");
        assert!(param.parameter_data_ref().required);
    }

    #[test]
    fn template_matching_skips_unmatched() {
        let config = test_config();
        let templates = vec!["/users/{id}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        let req = MockRequest::get("https://api.example.com/posts/1")
            .with_json_response(&serde_json::json!([]));

        builder.add_request(&req);
        let spec = builder.build();

        assert!(spec.paths.paths.is_empty());
    }

    // ── Multiple paths ─────────────────────────────────────────────

    #[test]
    fn multiple_paths() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req1 = MockRequest::get("https://api.example.com/users")
            .with_json_response(&serde_json::json!([]));
        let req2 = MockRequest::get("https://api.example.com/posts")
            .with_json_response(&serde_json::json!([]));
        let req3 = MockRequest::get("https://api.example.com/health");

        builder.add_request(&req1);
        builder.add_request(&req2);
        builder.add_request(&req3);
        let spec = builder.build();

        assert_eq!(spec.paths.paths.len(), 3);
        assert!(spec.paths.paths.contains_key("/users"));
        assert!(spec.paths.paths.contains_key("/posts"));
        assert!(spec.paths.paths.contains_key("/health"));
    }

    // ── add_requests (batch) ───────────────────────────────────────

    #[test]
    fn add_requests_batch() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let requests: Vec<Box<dyn CapturedRequest>> = vec![
            Box::new(
                MockRequest::get("https://api.example.com/a")
                    .with_json_response(&serde_json::json!({})),
            ),
            Box::new(
                MockRequest::get("https://api.example.com/b")
                    .with_json_response(&serde_json::json!({})),
            ),
        ];

        builder.add_requests(&requests);
        let spec = builder.build();

        assert_eq!(spec.paths.paths.len(), 2);
    }

    // ── Query parameters ───────────────────────────────────────────

    #[test]
    fn query_params_extracted() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::get("https://api.example.com/search?q=hello&page=1")
            .with_json_response(&serde_json::json!([]));

        builder.add_request(&req);
        let spec = builder.build();

        let path_item = spec.paths.paths["/search"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();

        let param_names: Vec<&str> = get_op
            .parameters
            .iter()
            .map(|p| p.as_item().unwrap().parameter_data_ref().name.as_str())
            .collect();
        assert!(param_names.contains(&"q"));
        assert!(param_names.contains(&"page"));
    }

    // ── No response body ───────────────────────────────────────────

    #[test]
    fn no_response_body_still_creates_response() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::get("https://api.example.com/health").with_status(204, "No Content");

        builder.add_request(&req);
        let spec = builder.build();

        let path_item = spec.paths.paths["/health"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();
        let resp = get_op
            .responses
            .responses
            .get(&StatusCode::Code(204))
            .unwrap()
            .as_item()
            .unwrap();
        assert_eq!(resp.description, "No Content");
        assert!(resp.content.is_empty());
    }

    // ── Prefix with path component ─────────────────────────────────

    #[test]
    fn prefix_with_path_component() {
        let mut config = test_config();
        config.prefix = "https://api.example.com/api/v1".to_string();
        let mut builder = OpenApiBuilder::new("https://api.example.com/api/v1", &config, vec![]);

        let req = MockRequest::get("https://api.example.com/api/v1/users")
            .with_json_response(&serde_json::json!([]));

        builder.add_request(&req);
        let spec = builder.build();

        assert!(spec.paths.paths.contains_key("/users"));
    }

    // ── discover_paths ─────────────────────────────────────────────

    #[test]
    fn discover_empty_requests() {
        let result = discover_paths(&[], "https://api.example.com", None, &[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn discover_single_get() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![Box::new(MockRequest::get(
            "https://api.example.com/api/v1/users",
        ))];
        let result = discover_paths(&requests, "https://api.example.com", None, &[], &[]);
        assert_eq!(result, vec!["ignore:/api/v1/users"]);
    }

    #[test]
    fn discover_parameterized_path() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![Box::new(MockRequest::get(
            "https://api.example.com/api/v1/users/123",
        ))];
        let result = discover_paths(&requests, "https://api.example.com", None, &[], &[]);
        assert!(result.contains(&"ignore:/api/v1/users/123".to_string()));
        assert!(result.contains(&"ignore:/api/v1/users/{id}".to_string()));
    }

    #[test]
    fn discover_multiple_paths_sorted_deduped() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![
            Box::new(MockRequest::get("https://api.example.com/users")),
            Box::new(MockRequest::get("https://api.example.com/posts")),
            Box::new(MockRequest::get("https://api.example.com/users")),
        ];
        let result = discover_paths(&requests, "https://api.example.com", None, &[], &[]);
        assert_eq!(result, vec!["ignore:/posts", "ignore:/users"]);
    }

    #[test]
    fn discover_prefix_stripping() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![
            Box::new(MockRequest::get("https://api.example.com/api/v1/health")),
            Box::new(MockRequest::get("https://other.example.com/ignored")),
        ];
        let result = discover_paths(&requests, "https://api.example.com/api/v1", None, &[], &[]);
        assert_eq!(result, vec!["ignore:/health"]);
    }

    #[test]
    fn discover_strips_query_string() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![Box::new(MockRequest::get(
            "https://api.example.com/search?q=hello&page=1",
        ))];
        let result = discover_paths(&requests, "https://api.example.com", None, &[], &[]);
        assert_eq!(result, vec!["ignore:/search"]);
    }

    #[test]
    fn discover_respects_exclude_patterns() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![
            Box::new(MockRequest::get("https://api.example.com/api/v1/users")),
            Box::new(MockRequest::get(
                "https://api.example.com/static/css/main.abc.css",
            )),
            Box::new(MockRequest::get(
                "https://api.example.com/static/js/app.xyz.js",
            )),
            Box::new(MockRequest::get("https://api.example.com/images/logo.svg")),
        ];
        let patterns: Vec<String> = vec!["/static/**".into(), "/images/**".into()];
        let result = discover_paths(&requests, "https://api.example.com", None, &patterns, &[]);
        assert_eq!(result, vec!["ignore:/api/v1/users"]);
    }

    #[test]
    fn discover_respects_include_patterns() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![
            Box::new(MockRequest::get("https://api.example.com/api/v1/users")),
            Box::new(MockRequest::get("https://api.example.com/login")),
        ];
        let include: Vec<String> = vec!["/api/**".into()];
        let result = discover_paths(&requests, "https://api.example.com", None, &[], &include);
        assert!(result.contains(&"/api/v1/users".to_string()));
        assert!(result.contains(&"ignore:/login".to_string()));
    }

    // ── glob_match ─────────────────────────────────────────────────

    #[test]
    fn glob_matches_double_star_subtree() {
        assert!(glob_match("/static/**", "/static/css/main.css"));
        assert!(glob_match("/static/**", "/static/"));
        assert!(!glob_match("/static/**", "/other/file"));
    }

    #[test]
    fn glob_matches_single_star_within_segment() {
        assert!(glob_match("*.css", "main.css"));
        assert!(glob_match("/api/*/users", "/api/v1/users"));
        assert!(!glob_match("/api/*/users", "/api/v1/v2/users"));
    }

    #[test]
    fn glob_exact_match() {
        assert!(glob_match("/health", "/health"));
        assert!(!glob_match("/health", "/healthz"));
    }

    // ── Tag extraction ─────────────────────────────────────────────

    #[test]
    fn tag_extracted_from_first_segment() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::get("https://api.example.com/users/123")
            .with_json_response(&serde_json::json!({}));
        builder.add_request(&req);
        let spec = builder.build();

        let path_item = spec.paths.paths["/users/123"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();
        assert_eq!(get_op.tags, vec!["users"]);
    }

    #[test]
    fn tag_override_applied() {
        let mut config = test_config();
        config.tags_overrides = Some(r#"{"users": "User Management"}"#.to_string());
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::get("https://api.example.com/users")
            .with_json_response(&serde_json::json!({}));
        builder.add_request(&req);
        let spec = builder.build();

        let path_item = spec.paths.paths["/users"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();
        assert_eq!(get_op.tags, vec!["User Management"]);
    }

    // ── ignore_images ──────────────────────────────────────────────

    #[test]
    fn ignore_images_skips_image_responses() {
        let mut config = test_config();
        config.ignore_images = true;
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let mut req = MockRequest::get("https://api.example.com/avatar.png");
        req.response_content_type = Some("image/png".to_string());
        req.response_body = Some(vec![0x89, 0x50, 0x4E, 0x47]);
        builder.add_request(&req);

        let spec = builder.build();
        assert!(spec.paths.paths.is_empty());
    }

    #[test]
    fn ignore_images_off_keeps_image_responses() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let mut req = MockRequest::get("https://api.example.com/avatar.png");
        req.response_content_type = Some("image/png".to_string());
        req.response_body = Some(vec![0x89, 0x50, 0x4E, 0x47]);
        builder.add_request(&req);

        let spec = builder.build();
        assert!(spec.paths.paths.contains_key("/avatar.png"));
    }

    // ── suppress_params ────────────────────────────────────────────

    #[test]
    fn suppress_params_removes_parameters() {
        let mut config = test_config();
        config.suppress_params = true;
        let templates = vec!["/users/{id}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        let req = MockRequest::get("https://api.example.com/users/123?page=1")
            .with_json_response(&serde_json::json!({}));
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = spec.paths.paths["/users/{id}"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();
        assert!(get_op.parameters.is_empty());
    }

    // ── include_headers ────────────────────────────────────────────

    #[test]
    fn include_headers_adds_header_params() {
        let mut config = test_config();
        config.include_headers = true;
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let mut req = MockRequest::get("https://api.example.com/data");
        req.request_headers = vec![
            ("X-Request-Id".to_string(), "abc".to_string()),
            ("Host".to_string(), "api.example.com".to_string()),
        ];
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = spec.paths.paths["/data"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();
        let param_names: Vec<&str> = get_op
            .parameters
            .iter()
            .map(|p| p.as_item().unwrap().parameter_data_ref().name.as_str())
            .collect();
        assert!(param_names.contains(&"X-Request-Id"));
        assert!(!param_names.contains(&"Host"));
    }

    #[test]
    fn exclude_headers_filters_custom_headers() {
        let mut config = test_config();
        config.include_headers = true;
        config.exclude_headers = vec!["X-Internal".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let mut req = MockRequest::get("https://api.example.com/data");
        req.request_headers = vec![
            ("X-Request-Id".to_string(), "abc".to_string()),
            ("X-Internal".to_string(), "secret".to_string()),
        ];
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = spec.paths.paths["/data"].as_item().unwrap();
        let get_op = path_item.get.as_ref().unwrap();
        let param_names: Vec<&str> = get_op
            .parameters
            .iter()
            .map(|p| p.as_item().unwrap().parameter_data_ref().name.as_str())
            .collect();
        assert!(param_names.contains(&"X-Request-Id"));
        assert!(!param_names.contains(&"X-Internal"));
    }

    // ── extract_tag helper ─────────────────────────────────────────

    #[test]
    fn extract_tag_basic() {
        assert_eq!(extract_tag("/api/v1/users", &None), Some("api".to_string()));
    }

    #[test]
    fn extract_tag_root() {
        assert_eq!(extract_tag("/", &None), None);
    }

    #[test]
    fn extract_tag_param_segment_skipped() {
        assert_eq!(extract_tag("/{id}/details", &None), None);
    }

    #[test]
    fn extract_tag_with_override() {
        let overrides: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(r#"{"api": "Core API"}"#).unwrap();
        assert_eq!(
            extract_tag("/api/v1/users", &Some(overrides)),
            Some("Core API".to_string())
        );
    }

    #[test]
    fn unknown_method_skipped() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest {
            url: "https://api.example.com/pets".to_string(),
            method: "FOOBAR".to_string(),
            request_headers: vec![],
            request_body: None,
            response_status: Some(200),
            response_reason: Some("OK".to_string()),
            response_headers: None,
            response_body: None,
            response_content_type: None,
        };
        builder.add_request(&req);

        let spec = builder.build();
        assert!(
            spec.paths.paths.is_empty(),
            "unknown method FOOBAR should not create any path entry"
        );
    }

    #[test]
    fn patch_method_honored() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest {
            url: "https://api.example.com/pets/1".to_string(),
            method: "PATCH".to_string(),
            request_headers: vec![],
            request_body: None,
            response_status: Some(200),
            response_reason: Some("OK".to_string()),
            response_headers: None,
            response_body: None,
            response_content_type: None,
        };
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = match spec.paths.paths.get("/pets/1") {
            Some(ReferenceOr::Item(item)) => item,
            _ => panic!("expected path /pets/1 to exist"),
        };
        assert!(path_item.patch.is_some(), "PATCH operation should be set");
        assert!(
            path_item.get.is_none(),
            "GET should not be set for a PATCH request"
        );
    }

    #[test]
    fn case_insensitive_method() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest {
            url: "https://api.example.com/pets".to_string(),
            method: "patch".to_string(),
            request_headers: vec![],
            request_body: None,
            response_status: Some(200),
            response_reason: Some("OK".to_string()),
            response_headers: None,
            response_body: None,
            response_content_type: None,
        };
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = match spec.paths.paths.get("/pets") {
            Some(ReferenceOr::Item(item)) => item,
            _ => panic!("expected path /pets to exist"),
        };
        assert!(
            path_item.patch.is_some(),
            "lowercase 'patch' should be normalized to PATCH"
        );
    }

    // ── request body merge ───────────────────────────────────────────

    #[test]
    fn request_body_merge_different_schemas() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req1 = MockRequest::post("https://api.example.com/items")
            .with_json_request_body(&serde_json::json!({"name": "Alice"}));
        let req2 = MockRequest::post("https://api.example.com/items")
            .with_json_request_body(&serde_json::json!({"age": 30}));

        builder.add_request(&req1);
        builder.add_request(&req2);

        let spec = builder.build();
        let path_item = spec.paths.paths["/items"].as_item().unwrap();
        let op = path_item.post.as_ref().unwrap();
        let rb = op.request_body.as_ref().unwrap().as_item().unwrap();
        let mt = rb.content.get("application/json").unwrap();
        let schema = mt.schema.as_ref().unwrap().as_item().unwrap();
        match &schema.schema_kind {
            openapiv3::SchemaKind::OneOf { one_of } => {
                assert_eq!(one_of.len(), 2);
            }
            _ => panic!("expected oneOf schema"),
        }
    }

    #[test]
    fn request_body_merge_identical_schemas() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req1 = MockRequest::post("https://api.example.com/items")
            .with_json_request_body(&serde_json::json!({"name": "Alice"}));
        let req2 = MockRequest::post("https://api.example.com/items")
            .with_json_request_body(&serde_json::json!({"name": "Bob"}));

        builder.add_request(&req1);
        builder.add_request(&req2);

        let spec = builder.build();
        let path_item = spec.paths.paths["/items"].as_item().unwrap();
        let op = path_item.post.as_ref().unwrap();
        let rb = op.request_body.as_ref().unwrap().as_item().unwrap();
        let mt = rb.content.get("application/json").unwrap();
        let schema = mt.schema.as_ref().unwrap().as_item().unwrap();
        if let openapiv3::SchemaKind::OneOf { .. } = &schema.schema_kind {
            panic!("identical schemas should NOT produce oneOf");
        }
    }

    #[test]
    fn request_body_first_no_body_second_has_body() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req1 = MockRequest::post("https://api.example.com/items");
        let req2 = MockRequest::post("https://api.example.com/items")
            .with_json_request_body(&serde_json::json!({"name": "Alice"}));

        builder.add_request(&req1);
        builder.add_request(&req2);

        let spec = builder.build();
        let path_item = spec.paths.paths["/items"].as_item().unwrap();
        let op = path_item.post.as_ref().unwrap();
        let rb = op.request_body.as_ref().unwrap().as_item().unwrap();
        let mt = rb.content.get("application/json").unwrap();
        assert!(mt.schema.is_some());
    }

    #[test]
    fn request_body_different_content_types_separate() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req1 = MockRequest::post("https://api.example.com/items")
            .with_json_request_body(&serde_json::json!({"name": "Alice"}));
        let mut req2 = MockRequest::post("https://api.example.com/items");
        req2.request_headers = vec![(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )];
        req2.request_body = Some(b"name=Bob&age=30".to_vec());

        builder.add_request(&req1);
        builder.add_request(&req2);

        let spec = builder.build();
        let path_item = spec.paths.paths["/items"].as_item().unwrap();
        let op = path_item.post.as_ref().unwrap();
        let rb = op.request_body.as_ref().unwrap().as_item().unwrap();
        assert!(rb.content.contains_key("application/json"));
        assert!(rb.content.contains_key("application/x-www-form-urlencoded"));
        assert_eq!(rb.content.len(), 2);
    }

    // ── examples accumulator ───────────────────────────────────────

    #[test]
    fn examples_accumulator_basic() {
        let config = test_config();
        let templates = vec!["/users/{id}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        let req1 = MockRequest::get("https://api.example.com/users/1")
            .with_json_response(&serde_json::json!({"name": "Alice", "age": 30}));
        let req2 = MockRequest::get("https://api.example.com/users/2")
            .with_json_response(&serde_json::json!({"name": "Bob", "age": 25}));

        builder.add_request(&req1);
        builder.add_request(&req2);

        let spec = builder.build();
        let path_item = match spec.paths.paths.get("/users/{id}") {
            Some(ReferenceOr::Item(item)) => item,
            _ => panic!("expected /users/{{id}}"),
        };
        let op = path_item.get.as_ref().unwrap();
        let resp = match op.responses.responses.get(&StatusCode::Code(200)) {
            Some(ReferenceOr::Item(r)) => r,
            _ => panic!("expected 200 response"),
        };
        let mt = resp
            .content
            .get("application/json")
            .expect("expected json media type");
        assert_eq!(mt.examples.len(), 2, "should have 2 examples");
    }

    #[test]
    fn examples_binary_skipped() {
        let config = test_config();
        let templates = vec!["/files/{id}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        let req = MockRequest {
            url: "https://api.example.com/files/1".to_string(),
            method: "GET".to_string(),
            request_headers: vec![],
            request_body: None,
            response_status: Some(200),
            response_reason: Some("OK".to_string()),
            response_headers: None,
            response_body: Some(vec![0x89, 0x50, 0x4E, 0x47]),
            response_content_type: Some("image/png".to_string()),
        };
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = spec.paths.paths.get("/files/{id}");
        if let Some(ReferenceOr::Item(item)) = path_item {
            if let Some(op) = &item.get {
                for (_, resp_ref) in &op.responses.responses {
                    if let ReferenceOr::Item(resp) = resp_ref {
                        for (_, mt) in &resp.content {
                            assert!(mt.examples.is_empty(), "binary should have no examples");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn response_examples_multiple_captures() {
        let config = test_config();
        let templates = vec!["/users/{id}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        for i in 1..=3 {
            let req = MockRequest::get(&format!("https://api.example.com/users/{i}"))
                .with_json_response(&serde_json::json!({"id": i, "name": format!("User{i}")}));
            builder.add_request(&req);
        }

        let spec = builder.build();
        let path_item = match spec.paths.paths.get("/users/{id}") {
            Some(ReferenceOr::Item(item)) => item,
            _ => panic!("expected /users/{{id}}"),
        };
        let op = path_item.get.as_ref().unwrap();
        let resp = match op.responses.responses.get(&StatusCode::Code(200)) {
            Some(ReferenceOr::Item(r)) => r,
            _ => panic!("expected 200 response"),
        };
        let mt = resp
            .content
            .get("application/json")
            .expect("expected json media type");
        assert_eq!(mt.examples.len(), 3, "should have 3 named examples");
    }

    #[test]
    fn response_examples_non_json_skipped() {
        let config = test_config();
        let templates = vec!["/health".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        let req = MockRequest {
            url: "https://api.example.com/health".to_string(),
            method: "GET".to_string(),
            request_headers: vec![],
            request_body: None,
            response_status: Some(200),
            response_reason: Some("OK".to_string()),
            response_headers: None,
            response_body: Some(b"OK".to_vec()),
            response_content_type: Some("text/plain".to_string()),
        };
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = match spec.paths.paths.get("/health") {
            Some(ReferenceOr::Item(item)) => item,
            _ => return,
        };
        if let Some(op) = &path_item.get {
            for (_, resp_ref) in &op.responses.responses {
                if let ReferenceOr::Item(resp) = resp_ref {
                    for (_, mt) in &resp.content {
                        assert!(mt.examples.is_empty(), "text/plain should have no examples");
                    }
                }
            }
        }
    }

    // ── max_examples cap ───────────────────────────────────────────

    #[test]
    fn max_examples_cap_enforced() {
        let mut config = test_config();
        config.max_examples = 2;
        let templates = vec!["/users/{id}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        for i in 1..=10 {
            let req = MockRequest::get(&format!("https://api.example.com/users/{i}"))
                .with_json_response(&serde_json::json!({"id": i, "name": format!("User{i}")}));
            builder.add_request(&req);
        }

        let spec = builder.build();
        let path_item = spec
            .paths
            .paths
            .get("/users/{id}")
            .unwrap()
            .as_item()
            .unwrap();
        let op = path_item.get.as_ref().unwrap();
        let resp = op
            .responses
            .responses
            .get(&StatusCode::Code(200))
            .unwrap()
            .as_item()
            .unwrap();
        let mt = resp.content.get("application/json").expect("expected json");
        assert_eq!(mt.examples.len(), 2, "cap of 2 should be enforced");
    }

    #[test]
    fn max_examples_zero_disables() {
        let mut config = test_config();
        config.max_examples = 0;
        let templates = vec!["/users/{id}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        for i in 1..=3 {
            let req = MockRequest::get(&format!("https://api.example.com/users/{i}"))
                .with_json_response(&serde_json::json!({"id": i}));
            builder.add_request(&req);
        }

        let spec = builder.build();
        let path_item = spec
            .paths
            .paths
            .get("/users/{id}")
            .unwrap()
            .as_item()
            .unwrap();
        let op = path_item.get.as_ref().unwrap();
        let resp = op
            .responses
            .responses
            .get(&StatusCode::Code(200))
            .unwrap()
            .as_item()
            .unwrap();
        let mt = resp.content.get("application/json").expect("expected json");
        assert_eq!(
            mt.examples.len(),
            0,
            "max_examples=0 should store no examples"
        );
    }

    #[test]
    fn max_examples_default_five() {
        let config = test_config();
        let templates = vec!["/users/{id}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        for i in 1..=8 {
            let req = MockRequest::get(&format!("https://api.example.com/users/{i}"))
                .with_json_response(&serde_json::json!({"id": i, "name": format!("User{i}")}));
            builder.add_request(&req);
        }

        let spec = builder.build();
        let path_item = spec
            .paths
            .paths
            .get("/users/{id}")
            .unwrap()
            .as_item()
            .unwrap();
        let op = path_item.get.as_ref().unwrap();
        let resp = op
            .responses
            .responses
            .get(&StatusCode::Code(200))
            .unwrap()
            .as_item()
            .unwrap();
        let mt = resp.content.get("application/json").expect("expected json");
        assert!(
            mt.examples.len() <= 5,
            "default cap of 5 should be enforced, got {}",
            mt.examples.len()
        );
    }

    #[test]
    fn request_examples_multiple_captures() {
        let config = test_config();
        let templates = vec!["/orders".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        for i in 1..=3u32 {
            let req = MockRequest::post("https://api.example.com/orders")
                .with_json_request_body(&serde_json::json!({"item": i, "qty": i}))
                .with_json_response(&serde_json::json!({"id": i}))
                .with_status(201, "Created");
            builder.add_request(&req);
        }

        let spec = builder.build();
        let path_item = spec.paths.paths.get("/orders").expect("expected /orders");
        let op = path_item.as_item().unwrap().post.as_ref().unwrap();
        let rb = op.request_body.as_ref().unwrap().as_item().unwrap();
        let mt = rb.content.get("application/json").expect("expected json");
        assert_eq!(mt.examples.len(), 3, "should have 3 request body examples");
    }

    #[test]
    fn request_examples_get_no_body() {
        let config = test_config();
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        for i in 1..=3u32 {
            let req = MockRequest::get(&format!("https://api.example.com/users/{i}"))
                .with_json_response(&serde_json::json!({"id": i}));
            builder.add_request(&req);
        }

        let spec = builder.build();
        for (_, path_ref) in &spec.paths.paths {
            if let ReferenceOr::Item(item) = path_ref {
                if let Some(op) = &item.get {
                    assert!(op.request_body.is_none(), "GET should have no requestBody");
                }
            }
        }
    }

    #[test]
    fn response_examples_duplicate_names_get_suffix() {
        let config = test_config();
        let templates = vec!["/items".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, templates);

        let req1 = MockRequest::get("https://api.example.com/items")
            .with_json_response(&serde_json::json!({"status": "active", "count": 1}));
        let req2 = MockRequest::get("https://api.example.com/items")
            .with_json_response(&serde_json::json!({"status": "active", "count": 2}));
        builder.add_request(&req1);
        builder.add_request(&req2);

        let spec = builder.build();
        let path_item = match spec.paths.paths.get("/items") {
            Some(ReferenceOr::Item(item)) => item,
            _ => panic!("expected /items"),
        };
        let op = path_item.get.as_ref().unwrap();
        let resp = match op.responses.responses.get(&StatusCode::Code(200)) {
            Some(ReferenceOr::Item(r)) => r,
            _ => panic!("expected 200 response"),
        };
        let mt = resp
            .content
            .get("application/json")
            .expect("expected json media type");
        assert_eq!(mt.examples.len(), 2, "should have 2 examples");
        let names: Vec<&String> = mt.examples.keys().collect();
        assert_eq!(names[0], "active");
        assert_eq!(names[1], "active_2");
    }

    // ── redaction integration ──────────────────────────────────────

    #[test]
    fn redact_integration_field() {
        let mut config = test_config();
        config.redact_fields = vec!["token".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::get("https://api.example.com/auth")
            .with_json_response(&serde_json::json!({"token": "secret123", "user": "alice"}));
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = spec.paths.paths["/auth"].as_item().unwrap();
        let op = path_item.get.as_ref().unwrap();
        let resp = op
            .responses
            .responses
            .get(&StatusCode::Code(200))
            .unwrap()
            .as_item()
            .unwrap();
        let mt = resp.content.get("application/json").unwrap();
        let ex = mt.examples.values().next().unwrap().as_item().unwrap();
        let val = ex.value.as_ref().unwrap();
        assert_eq!(val["token"], "[REDACTED]");
        assert_eq!(val["user"], "alice");
    }

    #[test]
    fn redact_integration_pattern() {
        let mut config = test_config();
        config.redact_patterns = vec!["[0-9a-f]{32,}".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::post("https://api.example.com/login")
            .with_json_request_body(
                &serde_json::json!({"session": "abcdef1234567890abcdef1234567890"}),
            )
            .with_json_response(&serde_json::json!({"ok": true}))
            .with_status(200, "OK");
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = spec.paths.paths["/login"].as_item().unwrap();
        let op = path_item.post.as_ref().unwrap();
        let rb = op.request_body.as_ref().unwrap().as_item().unwrap();
        let mt = rb.content.get("application/json").unwrap();
        let ex = mt.examples.values().next().unwrap().as_item().unwrap();
        let val = ex.value.as_ref().unwrap();
        assert_eq!(val["session"], "[REDACTED]");
    }

    #[test]
    fn redact_schema_unaffected() {
        let mut config = test_config();
        config.redact_fields = vec!["token".to_string()];
        let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

        let req = MockRequest::get("https://api.example.com/auth")
            .with_json_response(&serde_json::json!({"token": "secret123", "user": "alice"}));
        builder.add_request(&req);

        let spec = builder.build();
        let path_item = spec.paths.paths["/auth"].as_item().unwrap();
        let op = path_item.get.as_ref().unwrap();
        let resp = op
            .responses
            .responses
            .get(&StatusCode::Code(200))
            .unwrap()
            .as_item()
            .unwrap();
        let mt = resp.content.get("application/json").unwrap();
        let schema = mt.schema.as_ref().unwrap().as_item().unwrap();
        match &schema.schema_kind {
            openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) => {
                assert!(obj.properties.contains_key("token"));
                let token_schema = obj.properties["token"].as_item().unwrap();
                match &token_schema.schema_kind {
                    openapiv3::SchemaKind::Type(openapiv3::Type::String(_)) => {}
                    other => panic!("expected string schema for token, got {:?}", other),
                }
            }
            other => panic!("expected Object schema, got {:?}", other),
        }
    }
}

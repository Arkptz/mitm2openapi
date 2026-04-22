use indexmap::IndexMap;
use openapiv3::{
    Info, MediaType, OpenAPI, Operation, PathItem, Paths, ReferenceOr, RequestBody, Response,
    Responses, Server, StatusCode,
};
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
    tags_overrides: Option<serde_json::Map<String, serde_json::Value>>,
    compiled_templates: path_matching::CompiledTemplates,
    spec: OpenAPI,
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
    json_str.as_ref().and_then(|s| {
        serde_json::from_str::<serde_json::Value>(s)
            .ok()
            .and_then(|v| v.as_object().cloned())
    })
}

fn is_image_content_type(ct: Option<&str>) -> bool {
    ct.is_some_and(|s| s.to_lowercase().starts_with("image/"))
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

        Self {
            prefix: prefix.to_string(),
            config: config.clone(),
            tags_overrides,
            compiled_templates,
            spec,
        }
    }

    pub fn add_request(&mut self, request: &dyn CapturedRequest) {
        let url = request.get_url();
        let method = request.get_method().to_uppercase();

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

        if let Some(ReferenceOr::Item(existing)) = self.spec.paths.paths.get(&template_path) {
            if has_operation(existing, &method) {
                return;
            }
        }

        let mut operation = Operation {
            summary: Some(format!("{} {}", method, template_path)),
            ..Operation::default()
        };

        if let Some(tag) = extract_tag(&template_path, &self.tags_overrides) {
            operation.tags = vec![tag];
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
                    media_type_str,
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
            }
        }

        let status_code = request.get_response_status_code().unwrap_or(200);
        let reason = request.get_response_reason().unwrap_or("OK").to_string();

        let mut response = Response {
            description: reason,
            ..Response::default()
        };

        if let Some(resp_body) = request.get_response_body() {
            let resp_ct = request.get_response_content_type();
            if let Some((media_type_str, val)) = parse_body(resp_body, resp_ct) {
                let schema = schema::value_to_schema(&val);
                let mut content = IndexMap::new();
                content.insert(
                    media_type_str,
                    MediaType {
                        schema: Some(ReferenceOr::Item(schema)),
                        ..MediaType::default()
                    },
                );
                response.content = content;
            }
        }

        let mut responses = IndexMap::new();
        responses.insert(StatusCode::Code(status_code), ReferenceOr::Item(response));
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
    pub fn build(self) -> OpenAPI {
        self.spec
    }
}

#[cfg(test)]
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
            param_regex: None,
            openapi_title: None,
            openapi_version: "1.0.0".to_string(),
            exclude_headers: vec![],
            exclude_cookies: vec![],
            include_headers: false,
            ignore_images: false,
            suppress_params: false,
            tags_overrides: None,
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

    // ── First-seen-wins ────────────────────────────────────────────

    #[test]
    fn first_seen_wins() {
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

        // Should have the first request's response (version: 1 → object with "version" key)
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
                // The first request had integer value
                let version_schema = obj.properties["version"].as_item().unwrap();
                assert!(matches!(
                    version_schema.schema_kind,
                    openapiv3::SchemaKind::Type(openapiv3::Type::Integer(_))
                ));
            }
            other => panic!("expected Object, got {:?}", other),
        }
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
}

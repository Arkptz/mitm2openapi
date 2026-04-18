//! OpenAPI spec builder: assembles a complete OpenAPI 3.0 spec from processed CapturedRequest data.

use indexmap::IndexMap;
use openapiv3::{
    Info, MediaType, OpenAPI, Operation, PathItem, Paths, ReferenceOr, RequestBody, Response,
    Responses, Server, StatusCode,
};

use crate::params;
use crate::path_matching;
use crate::schema;
use crate::types::{CapturedRequest, Config};

/// Discover unique API paths from captured requests and generate templates.
/// Each template is prefixed with "ignore:" — the user removes the prefix for paths they want.
/// Parameterized paths (with numeric/UUID segments) get suggestions like "/users/{id}".
pub fn discover_paths(
    requests: &[Box<dyn CapturedRequest>],
    prefix: &str,
    custom_regex: Option<&regex::Regex>,
) -> Vec<String> {
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
        seen_paths.insert(path);
    }

    let paths_vec: Vec<String> = seen_paths.iter().cloned().collect();
    let suggested = path_matching::suggest_param_templates(&paths_vec, custom_regex);

    let mut all: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for p in &paths_vec {
        all.insert(format!("ignore:{}", p));
    }
    for s in &suggested {
        all.insert(format!("ignore:{}", s));
    }

    all.into_iter().collect()
}

/// Assembles an OpenAPI 3.0 spec from captured HTTP requests.
pub struct OpenApiBuilder {
    prefix: String,
    #[allow(dead_code)]
    config: Config,
    templates: Vec<String>,
    spec: OpenAPI,
}

/// Extract the host from a URL prefix (e.g. `https://api.example.com/api` → `api.example.com`).
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
        // Parse key=value&key2=value2 into a JSON object
        if let Ok(body_str) = std::str::from_utf8(body) {
            let mut map = serde_json::Map::new();
            for pair in body_str.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
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
fn get_operation_mut<'a>(path_item: &'a mut PathItem, method: &str) -> &'a mut Option<Operation> {
    match method.to_uppercase().as_str() {
        "GET" => &mut path_item.get,
        "PUT" => &mut path_item.put,
        "POST" => &mut path_item.post,
        "DELETE" => &mut path_item.delete,
        "OPTIONS" => &mut path_item.options,
        "HEAD" => &mut path_item.head,
        "PATCH" => &mut path_item.patch,
        "TRACE" => &mut path_item.trace,
        _ => &mut path_item.get, // fallback
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
    /// Create a new builder with the given prefix URL and config.
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

        Self {
            prefix: prefix.to_string(),
            config: config.clone(),
            templates,
            spec,
        }
    }

    /// Process a single CapturedRequest and add it to the spec.
    /// Uses first-seen-wins: if a path+method already exists, skip.
    pub fn add_request(&mut self, request: &dyn CapturedRequest) {
        let url = request.get_url();
        let method = request.get_method().to_uppercase();

        // 1. URL matching: check if request URL starts with prefix
        if !url.starts_with(&self.prefix) {
            return;
        }

        // 2. Path extraction: strip prefix to get path
        let raw_path = &url[self.prefix.len()..];
        // Strip query string
        let path_no_query = raw_path.split('?').next().unwrap_or(raw_path);
        // Ensure path starts with /
        let path = if path_no_query.starts_with('/') {
            path_no_query.to_string()
        } else {
            format!("/{}", path_no_query)
        };

        // 3. Template matching
        let template_path = if self.templates.is_empty() {
            path.clone()
        } else {
            match path_matching::match_path(&path, &self.templates) {
                Some(t) => t.to_string(),
                None => return, // no template matches, skip
            }
        };

        // 4. First-seen-wins: check if path+method already exists
        if let Some(ReferenceOr::Item(existing)) = self.spec.paths.paths.get(&template_path) {
            if has_operation(existing, &method) {
                return;
            }
        }

        // 5. Build the operation
        let mut operation = Operation {
            summary: Some(format!("{} {}", method, template_path)),
            ..Operation::default()
        };

        // 6. Parameters: path params from template + query params from URL
        let mut parameters: Vec<ReferenceOr<openapiv3::Parameter>> = Vec::new();

        for p in params::extract_path_params(&template_path) {
            parameters.push(ReferenceOr::Item(p));
        }
        for p in params::extract_query_params(url) {
            parameters.push(ReferenceOr::Item(p));
        }

        operation.parameters = parameters;

        // 7. Request body schema
        if let Some(req_body) = request.get_request_body() {
            // Determine request content type from request headers
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

        // 8. Response assembly
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

        // 9. Insert into spec
        let path_item = self
            .spec
            .paths
            .paths
            .entry(template_path)
            .or_insert_with(|| ReferenceOr::Item(PathItem::default()));

        if let ReferenceOr::Item(ref mut item) = path_item {
            let slot = get_operation_mut(item, &method);
            *slot = Some(operation);
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
        let result = discover_paths(&[], "https://api.example.com", None);
        assert!(result.is_empty());
    }

    #[test]
    fn discover_single_get() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![Box::new(MockRequest::get(
            "https://api.example.com/api/v1/users",
        ))];
        let result = discover_paths(&requests, "https://api.example.com", None);
        assert_eq!(result, vec!["ignore:/api/v1/users"]);
    }

    #[test]
    fn discover_parameterized_path() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![Box::new(MockRequest::get(
            "https://api.example.com/api/v1/users/123",
        ))];
        let result = discover_paths(&requests, "https://api.example.com", None);
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
        let result = discover_paths(&requests, "https://api.example.com", None);
        assert_eq!(result, vec!["ignore:/posts", "ignore:/users"]);
    }

    #[test]
    fn discover_prefix_stripping() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![
            Box::new(MockRequest::get("https://api.example.com/api/v1/health")),
            Box::new(MockRequest::get("https://other.example.com/ignored")),
        ];
        let result = discover_paths(&requests, "https://api.example.com/api/v1", None);
        assert_eq!(result, vec!["ignore:/health"]);
    }

    #[test]
    fn discover_strips_query_string() {
        let requests: Vec<Box<dyn CapturedRequest>> = vec![Box::new(MockRequest::get(
            "https://api.example.com/search?q=hello&page=1",
        ))];
        let result = discover_paths(&requests, "https://api.example.com", None);
        assert_eq!(result, vec!["ignore:/search"]);
    }
}

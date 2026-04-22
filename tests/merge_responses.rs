use mitm2openapi::builder::OpenApiBuilder;
use mitm2openapi::types::{CapturedRequest, Config};
use openapiv3::StatusCode;

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
    fn post(url: &str) -> Self {
        Self {
            url: url.to_string(),
            method: "POST".to_string(),
            request_headers: vec![("Content-Type".to_string(), "application/json".to_string())],
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

#[test]
fn test_multiple_status_codes_merged() {
    let config = test_config();
    let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

    let req_ok = MockRequest::post("https://api.example.com/users")
        .with_json_request_body(&serde_json::json!({"name": "Alice"}))
        .with_json_response(&serde_json::json!({"id": 1, "name": "Alice"}))
        .with_status(200, "OK");

    let req_err = MockRequest::post("https://api.example.com/users")
        .with_json_request_body(&serde_json::json!({"name": ""}))
        .with_json_response(
            &serde_json::json!({"error": "validation_failed", "message": "name is required"}),
        )
        .with_status(400, "Bad Request");

    builder.add_request(&req_ok);
    builder.add_request(&req_err);
    let spec = builder.build();

    let path_item = spec.paths.paths.get("/users").unwrap().as_item().unwrap();
    let post_op = path_item.post.as_ref().unwrap();

    assert!(
        post_op
            .responses
            .responses
            .contains_key(&StatusCode::Code(200)),
        "200 response missing"
    );
    assert!(
        post_op
            .responses
            .responses
            .contains_key(&StatusCode::Code(400)),
        "400 response missing"
    );

    let resp_200 = post_op.responses.responses[&StatusCode::Code(200)]
        .as_item()
        .unwrap();
    assert_eq!(resp_200.description, "OK");
    assert!(resp_200.content.contains_key("application/json"));

    let resp_400 = post_op.responses.responses[&StatusCode::Code(400)]
        .as_item()
        .unwrap();
    assert_eq!(resp_400.description, "Bad Request");
    assert!(resp_400.content.contains_key("application/json"));
}

#[test]
fn test_same_status_divergent_schemas_one_of() {
    let config = test_config();
    let mut builder = OpenApiBuilder::new("https://api.example.com", &config, vec![]);

    let req1 = MockRequest::post("https://api.example.com/users")
        .with_json_request_body(&serde_json::json!({"name": "Alice"}))
        .with_json_response(&serde_json::json!({"id": 1, "name": "Alice"}))
        .with_status(200, "OK");

    let req2 = MockRequest::post("https://api.example.com/users")
        .with_json_request_body(&serde_json::json!({"name": "Bob"}))
        .with_json_response(&serde_json::json!({"users": [{"id": 1}, {"id": 2}]}))
        .with_status(200, "OK");

    builder.add_request(&req1);
    builder.add_request(&req2);
    let spec = builder.build();

    let path_item = spec.paths.paths.get("/users").unwrap().as_item().unwrap();
    let post_op = path_item.post.as_ref().unwrap();

    let resp = post_op.responses.responses[&StatusCode::Code(200)]
        .as_item()
        .unwrap();
    let media = &resp.content["application/json"];
    let schema = media.schema.as_ref().unwrap().as_item().unwrap();

    assert!(
        matches!(schema.schema_kind, openapiv3::SchemaKind::OneOf { .. }),
        "expected oneOf for divergent schemas, got {:?}",
        schema.schema_kind
    );
    if let openapiv3::SchemaKind::OneOf { one_of } = &schema.schema_kind {
        assert_eq!(one_of.len(), 2);
    }
}

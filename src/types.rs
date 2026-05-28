use crate::envelope::EnvelopeConfig;
use crate::operation_id::OperationIdStrategy;
use crate::tag_rules::TagStrategy;
use std::collections::HashMap;

/// Unified interface for captured HTTP requests from different sources (HAR, mitmproxy).
pub trait CapturedRequest {
    fn get_url(&self) -> &str;
    fn get_method(&self) -> &str;
    fn get_request_headers(&self) -> &[(String, String)];
    fn get_request_body(&self) -> Option<&[u8]>;
    fn get_response_status_code(&self) -> Option<u16>;
    fn get_response_reason(&self) -> Option<&str>;
    fn get_response_headers(&self) -> Option<&[(String, String)]>;
    fn get_response_body(&self) -> Option<&[u8]>;
    fn get_response_content_type(&self) -> Option<&str>;
}

/// Configuration for OpenAPI generation, derived from CLI arguments.
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub prefix: String,
    pub openapi_title: Option<String>,
    pub openapi_version: String,
    pub exclude_headers: Vec<String>,
    pub exclude_cookies: Vec<String>,
    pub include_headers: bool,
    pub ignore_images: bool,
    pub suppress_params: bool,
    pub tags_overrides: Option<String>,
    pub skip_options: bool,
    pub max_examples: usize,
    pub redact_patterns: Vec<String>,
    pub redact_fields: Vec<String>,
    pub tag_strategy: TagStrategy,
    pub operation_id_strategy: OperationIdStrategy,
    pub operation_id_overrides: HashMap<String, String>,
    pub envelope_config: Option<EnvelopeConfig>,
}

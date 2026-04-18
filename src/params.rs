use indexmap::IndexMap;
use openapiv3::{
    Parameter, ParameterData, ParameterSchemaOrContent, ReferenceOr, Schema, SchemaData,
    SchemaKind, StringType, Type,
};

/// Create a `ParameterData` with a string schema and the given name/required flag.
fn string_param_data(name: &str, required: bool) -> ParameterData {
    ParameterData {
        name: name.to_string(),
        description: None,
        required,
        deprecated: None,
        format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        })),
        example: None,
        examples: IndexMap::new(),
        explode: None,
        extensions: IndexMap::new(),
    }
}

/// Extract query parameters from a URL string, returning `openapiv3::Parameter` objects.
///
/// Parses the query string (`?key=value&key2=value2`) and creates query parameters.
/// Each parameter is optional (`required: false`) with a string schema.
pub fn extract_query_params(url: &str) -> Vec<Parameter> {
    let query_str = match url.split_once('?') {
        Some((_, q)) => q,
        None => return Vec::new(),
    };

    let query_str = query_str.split('#').next().unwrap_or(query_str);

    let mut seen = std::collections::HashSet::new();
    let mut params = Vec::new();

    for pair in query_str.split('&') {
        let key = match pair.split_once('=') {
            Some((k, _)) => k,
            None => pair,
        };
        let key = urlencoding_decode(key);
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        params.push(Parameter::Query {
            parameter_data: string_param_data(&key, false),
            allow_reserved: false,
            style: Default::default(),
            allow_empty_value: None,
        });
    }

    params
}

/// Decode percent-encoded strings (minimal implementation, no extra deps).
fn urlencoding_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'+' {
            result.push(' ');
        } else if b == b'%' {
            let hi = chars.next().and_then(hex_val);
            let lo = chars.next().and_then(hex_val);
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push((h << 4 | l) as char);
            } else {
                result.push('%');
            }
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Extract path parameters from a template string like `/users/{id}/posts/{post_id}`.
///
/// Returns `Parameter` objects with `in: path, required: true`.
pub fn extract_path_params(template: &str) -> Vec<Parameter> {
    let mut params = Vec::new();
    let mut rest = template;

    while let Some(start) = rest.find('{') {
        if let Some(end) = rest[start..].find('}') {
            let name = &rest[start + 1..start + end];
            if !name.is_empty() {
                params.push(Parameter::Path {
                    parameter_data: string_param_data(name, true),
                    style: Default::default(),
                });
            }
            rest = &rest[start + end + 1..];
        } else {
            break;
        }
    }

    params
}

/// Headers to exclude by default (case-insensitive).
const DEFAULT_EXCLUDE_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "content-type",
    "accept",
    "accept-encoding",
    "accept-language",
    "connection",
    "user-agent",
    "cookie",
    "authorization",
    "cache-control",
    "pragma",
    "te",
    "transfer-encoding",
    "upgrade",
];

/// Extract headers from request, optionally filtering by exclude list.
///
/// Returns `Parameter` objects with `in: header`.
/// By default, common non-informative headers (Host, Content-Length, etc.) are excluded.
/// The `exclude` list provides *additional* headers to exclude (case-insensitive).
pub fn extract_header_params(headers: &[(String, String)], exclude: &[String]) -> Vec<Parameter> {
    let exclude_lower: Vec<String> = exclude.iter().map(|h| h.to_lowercase()).collect();
    let mut seen = std::collections::HashSet::new();
    let mut params = Vec::new();

    for (name, _value) in headers {
        let lower = name.to_lowercase();
        if DEFAULT_EXCLUDE_HEADERS.contains(&lower.as_str()) {
            continue;
        }
        if exclude_lower.contains(&lower) {
            continue;
        }
        if !seen.insert(lower) {
            continue;
        }
        params.push(Parameter::Header {
            parameter_data: string_param_data(name, false),
            style: Default::default(),
        });
    }

    params
}

/// Generate an endpoint name from method + path template.
///
/// E.g., `"GET"`, `"/api/v1/users/{id}"` → `"getApiV1UsersId"`.
/// Strips parameter braces and converts to camelCase.
pub fn endpoint_name(method: &str, path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(method.to_lowercase());

    for segment in path.split('/') {
        if segment.is_empty() {
            continue;
        }
        let seg = segment.trim_start_matches('{').trim_end_matches('}');
        if seg.is_empty() {
            continue;
        }
        let mut chars = seg.chars();
        if let Some(first) = chars.next() {
            let capitalized: String = first.to_uppercase().chain(chars).collect();
            parts.push(capitalized);
        }
    }

    parts.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_params_basic() {
        let params = extract_query_params("https://example.com/api?page=1&limit=10");
        assert_eq!(params.len(), 2);

        let names: Vec<&str> = params
            .iter()
            .map(|p| p.parameter_data_ref().name.as_str())
            .collect();
        assert_eq!(names, vec!["page", "limit"]);

        for p in &params {
            assert!(!p.parameter_data_ref().required);
        }
    }

    #[test]
    fn query_params_empty() {
        let params = extract_query_params("https://example.com/api");
        assert!(params.is_empty());
    }

    #[test]
    fn query_params_no_value() {
        let params = extract_query_params("https://example.com/api?debug");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].parameter_data_ref().name, "debug");
    }

    #[test]
    fn query_params_dedup() {
        let params = extract_query_params("https://example.com/api?a=1&a=2&b=3");
        let names: Vec<&str> = params
            .iter()
            .map(|p| p.parameter_data_ref().name.as_str())
            .collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn query_params_with_fragment() {
        let params = extract_query_params("https://example.com/api?x=1#section");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].parameter_data_ref().name, "x");
    }

    #[test]
    fn query_params_encoded() {
        let params = extract_query_params("https://example.com/api?user%20name=foo");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].parameter_data_ref().name, "user name");
    }

    #[test]
    fn path_params_single() {
        let params = extract_path_params("/users/{id}");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].parameter_data_ref().name, "id");
        assert!(params[0].parameter_data_ref().required);
    }

    #[test]
    fn path_params_multiple() {
        let params = extract_path_params("/users/{user_id}/posts/{post_id}");
        assert_eq!(params.len(), 2);
        let names: Vec<&str> = params
            .iter()
            .map(|p| p.parameter_data_ref().name.as_str())
            .collect();
        assert_eq!(names, vec!["user_id", "post_id"]);
        for p in &params {
            assert!(p.parameter_data_ref().required);
        }
    }

    #[test]
    fn path_params_none() {
        let params = extract_path_params("/users");
        assert!(params.is_empty());
    }

    #[test]
    fn path_params_empty_braces() {
        let params = extract_path_params("/users/{}");
        assert!(params.is_empty());
    }

    #[test]
    fn header_params_basic() {
        let headers = vec![
            ("X-Request-Id".to_string(), "abc123".to_string()),
            ("X-Custom".to_string(), "val".to_string()),
        ];
        let params = extract_header_params(&headers, &[]);
        assert_eq!(params.len(), 2);
        let names: Vec<&str> = params
            .iter()
            .map(|p| p.parameter_data_ref().name.as_str())
            .collect();
        assert_eq!(names, vec!["X-Request-Id", "X-Custom"]);
    }

    #[test]
    fn header_params_excludes_default() {
        let headers = vec![
            ("Host".to_string(), "example.com".to_string()),
            ("Content-Length".to_string(), "42".to_string()),
            ("X-Custom".to_string(), "val".to_string()),
        ];
        let params = extract_header_params(&headers, &[]);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].parameter_data_ref().name, "X-Custom");
    }

    #[test]
    fn header_params_custom_exclude() {
        let headers = vec![
            ("X-Request-Id".to_string(), "abc".to_string()),
            ("X-Internal".to_string(), "secret".to_string()),
        ];
        let exclude = vec!["X-Internal".to_string()];
        let params = extract_header_params(&headers, &exclude);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].parameter_data_ref().name, "X-Request-Id");
    }

    #[test]
    fn header_params_case_insensitive_exclude() {
        let headers = vec![("host".to_string(), "example.com".to_string())];
        let params = extract_header_params(&headers, &[]);
        assert!(params.is_empty());
    }

    #[test]
    fn header_params_dedup() {
        let headers = vec![
            ("X-Dup".to_string(), "val1".to_string()),
            ("x-dup".to_string(), "val2".to_string()),
        ];
        let params = extract_header_params(&headers, &[]);
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn endpoint_name_basic() {
        assert_eq!(
            endpoint_name("GET", "/api/v1/users/{id}"),
            "getApiV1UsersId"
        );
    }

    #[test]
    fn endpoint_name_post() {
        assert_eq!(endpoint_name("POST", "/api/users"), "postApiUsers");
    }

    #[test]
    fn endpoint_name_root() {
        assert_eq!(endpoint_name("GET", "/"), "get");
    }

    #[test]
    fn endpoint_name_nested_params() {
        assert_eq!(
            endpoint_name("DELETE", "/orgs/{org}/repos/{repo}"),
            "deleteOrgsOrgReposRepo"
        );
    }
}

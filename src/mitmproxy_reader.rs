use std::path::Path;

use tracing::{debug, warn};

use crate::error::{Error, Result};
use crate::tnetstring;
use crate::tnetstring::TNetValue;
use crate::types::CapturedRequest;
use crate::MAX_BODY_SIZE;

const MAX_HEADER_NAME_SIZE: usize = 8 * 1024;
const MAX_HEADER_VALUE_SIZE: usize = 64 * 1024;

type RequestIter = Box<dyn Iterator<Item = Result<Box<dyn CapturedRequest>>>>;

/// Wrapper around a parsed mitmproxy flow that implements CapturedRequest.
pub struct MitmproxyFlowWrapper {
    url: String,
    method: String,
    request_headers: Vec<(String, String)>,
    request_body: Option<Vec<u8>>,
    response_status_code: Option<u16>,
    response_reason: Option<String>,
    response_headers: Option<Vec<(String, String)>>,
    response_body: Option<Vec<u8>>,
    response_content_type: Option<String>,
}

impl CapturedRequest for MitmproxyFlowWrapper {
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
        self.response_status_code
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

fn value_to_string(val: &TNetValue) -> Option<String> {
    match val {
        TNetValue::String(s) => Some(s.clone()),
        TNetValue::Bytes(b) => Some(String::from_utf8_lossy(b).into_owned()),
        _ => None,
    }
}

fn value_to_string_strict(val: &TNetValue) -> Option<String> {
    match val {
        TNetValue::String(s) => Some(s.clone()),
        TNetValue::Bytes(b) => match std::str::from_utf8(b) {
            Ok(s) => Some(s.to_string()),
            Err(_) => {
                warn!(
                    event = "utf8_rejected",
                    "identity field contains invalid UTF-8"
                );
                None
            }
        },
        _ => None,
    }
}

fn strip_control_chars(s: &str) -> String {
    s.chars()
        .filter(|c| {
            let b = *c as u32;
            !(b <= 0x1F || b == 0x7F)
        })
        .collect()
}

fn cap_body(body: &[u8], url: &str) -> Vec<u8> {
    if body.len() > MAX_BODY_SIZE {
        warn!(
            event = "body_truncated",
            original_size = body.len(),
            truncated_to = MAX_BODY_SIZE,
            url = %url,
            "truncating oversized body"
        );
        body.get(..MAX_BODY_SIZE).unwrap_or(body).to_vec()
    } else {
        body.to_vec()
    }
}

fn parse_headers(val: &TNetValue) -> Vec<(String, String)> {
    let list = match val.as_list() {
        Some(l) => l,
        None => return Vec::new(),
    };
    list.iter()
        .filter_map(|pair| {
            let inner = pair.as_list()?;
            let name_val = inner.first()?;
            let value_val = inner.get(1)?;
            let name = match value_to_string_strict(name_val) {
                Some(n) => n,
                None => {
                    warn!(event = "header_name_invalid_utf8", "dropping header with non-UTF-8 name");
                    return None;
                }
            };
            if name.len() > MAX_HEADER_NAME_SIZE {
                warn!(event = "header_name_too_large", size = name.len(), max = MAX_HEADER_NAME_SIZE, "dropping header with oversized name");
                return None;
            }
            let value = value_to_string(value_val)?;
            let value = if value.len() > MAX_HEADER_VALUE_SIZE {
                warn!(event = "header_value_too_large", size = value.len(), max = MAX_HEADER_VALUE_SIZE, name = %name, "truncating oversized header value");
                value.get(..MAX_HEADER_VALUE_SIZE).unwrap_or(&value).to_string()
            } else {
                value
            };
            Some((name, value))
        })
        .collect()
}

fn find_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Resolve hostname: host field → Host header → authority field.
fn resolve_host(request: &TNetValue, headers: &[(String, String)]) -> Option<String> {
    if let Some(host) = request.get("host").and_then(value_to_string_strict) {
        if !host.is_empty() {
            return Some(host);
        }
    }
    if let Some(host) = find_header(headers, "host") {
        if !host.is_empty() {
            return Some(host.to_string());
        }
    }
    if let Some(auth) = request.get("authority").and_then(value_to_string_strict) {
        if !auth.is_empty() {
            return Some(auth);
        }
    }
    None
}

fn is_valid_host(host: &str) -> bool {
    if host.is_empty() {
        return false;
    }
    !host
        .bytes()
        .any(|b| b == b'/' || b == b'\\' || b == b'@' || b <= 0x1F || b == 0x7F || b == b' ')
}

fn build_url_with_fallback(request: &TNetValue, headers: &[(String, String)]) -> Result<String> {
    let scheme = request
        .get("scheme")
        .and_then(value_to_string_strict)
        .unwrap_or_else(|| "https".to_string());

    if scheme != "http" && scheme != "https" {
        warn!(event = "scheme_rejected", scheme = %scheme, "skipping flow with non-http scheme");
        return Err(Error::FlowState(format!("unsupported scheme: {scheme}")));
    }

    let host = resolve_host(request, headers)
        .ok_or_else(|| Error::FlowState("request missing host in all sources".into()))?;

    if !is_valid_host(&host) {
        warn!(event = "host_rejected", host = %host, "skipping flow with invalid host");
        return Err(Error::FlowState(format!("invalid host: {host}")));
    }

    let raw_port = request.get("port").and_then(|v| v.as_int()).unwrap_or(0);
    let port = match u16::try_from(raw_port).ok().filter(|p| *p > 0) {
        Some(p) => p,
        None => {
            if raw_port != 0 {
                warn!(
                    event = "port_out_of_range",
                    value = raw_port,
                    "dropping request with invalid port"
                );
                return Err(Error::FlowState(format!(
                    "port out of valid range: {raw_port}"
                )));
            }
            // port field absent or 0 — treat as default
            0u16
        }
    };

    let raw_path = request
        .get("path")
        .and_then(value_to_string_strict)
        .unwrap_or_else(|| "/".to_string());
    let path = {
        let cleaned = strip_control_chars(&raw_path);
        if cleaned.len() != raw_path.len() {
            warn!(
                event = "path_cleaned",
                original_len = raw_path.len(),
                cleaned_len = cleaned.len(),
                "stripped control characters from path"
            );
        }
        cleaned
    };

    let is_default_port = (scheme == "https" && port == 443) || (scheme == "http" && port == 80);

    if is_default_port || port == 0 {
        Ok(format!("{scheme}://{host}{path}"))
    } else {
        Ok(format!("{scheme}://{host}:{port}{path}"))
    }
}

/// Parse a single mitmproxy flow dict into a MitmproxyFlowWrapper.
fn parse_flow(flow: &TNetValue) -> Result<MitmproxyFlowWrapper> {
    let flow_type = flow.get("type").and_then(value_to_string);
    if flow_type.as_deref() != Some("http") {
        return Err(Error::FlowState("not an HTTP flow".into()));
    }

    let request = flow
        .get("request")
        .ok_or_else(|| Error::FlowState("flow missing 'request'".into()))?;

    let method = request
        .get("method")
        .and_then(value_to_string_strict)
        .ok_or_else(|| Error::FlowState("request missing 'method' or invalid UTF-8".into()))?;

    let request_headers = request
        .get("headers")
        .map(parse_headers)
        .unwrap_or_default();

    let url = build_url_with_fallback(request, &request_headers)?;

    let request_body = request
        .get("content")
        .and_then(|v| if v.is_null() { None } else { v.as_bytes() })
        .filter(|b| !b.is_empty())
        .map(|b| cap_body(b, &url));

    let response = flow.get("response");

    let (
        response_status_code,
        response_reason,
        response_headers,
        response_body,
        response_content_type,
    ) =
        if let Some(resp) = response.filter(|r| !r.is_null()) {
            let status =
                resp.get("status_code").and_then(|v| v.as_int()).and_then(
                    |n| match u16::try_from(n).ok().filter(|s| (100..=599).contains(s)) {
                        Some(s) => Some(s),
                        None => {
                            warn!(
                                event = "status_out_of_range",
                                value = n,
                                "dropping response with invalid status code"
                            );
                            None
                        }
                    },
                );
            let reason = resp.get("reason").and_then(value_to_string);
            let headers = resp.get("headers").map(parse_headers);
            let body = resp
                .get("content")
                .and_then(|v| if v.is_null() { None } else { v.as_bytes() })
                .filter(|b| !b.is_empty())
                .map(|b| cap_body(b, &url));
            let content_type = headers
                .as_ref()
                .and_then(|h| find_header(h, "content-type").map(|v| v.to_string()));
            (status, reason, headers, body, content_type)
        } else {
            (None, None, None, None, None)
        };

    Ok(MitmproxyFlowWrapper {
        url,
        method,
        request_headers,
        request_body,
        response_status_code,
        response_reason,
        response_headers,
        response_body,
        response_content_type,
    })
}

pub fn stream_mitmproxy_file(
    path: &Path,
) -> Result<impl Iterator<Item = Result<Box<dyn CapturedRequest>>>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::with_capacity(64 * 1024, file);
    let display_path = path.display().to_string();

    Ok(
        tnetstring::TNetStringIter::new(reader).filter_map(
            move |value_result| match value_result {
                Ok(flow) => {
                    let flow_type = flow.get("type").and_then(value_to_string);
                    if flow_type.as_deref() != Some("http") {
                        debug!(path = %display_path, "Skipping non-HTTP flow");
                        return None;
                    }
                    match parse_flow(&flow) {
                        Ok(wrapper) => Some(Ok(Box::new(wrapper) as Box<dyn CapturedRequest>)),
                        Err(e) => {
                            warn!(path = %display_path, error = %e, "Skipping corrupt flow");
                            None
                        }
                    }
                }
                Err(e) => {
                    warn!(path = %display_path, error = %e, "Skipping unparseable flow entry");
                    None
                }
            },
        ),
    )
}

pub fn stream_mitmproxy_dir(path: &Path) -> Result<RequestIter> {
    let mut entries: Vec<_> = std::fs::read_dir(path)?
        .filter_map(|e| match e {
            Ok(entry) => Some(entry),
            Err(err) => {
                warn!(
                    event = "dir_entry_skipped",
                    error = %err,
                    "skipping unreadable directory entry"
                );
                None
            }
        })
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("flow"))
        })
        .collect();
    entries.sort_by_key(|e| e.path());

    let mut iters: Vec<RequestIter> = Vec::new();
    for entry in entries {
        match stream_mitmproxy_file(&entry.path()) {
            Ok(iter) => iters.push(Box::new(iter)),
            Err(e) => {
                warn!(path = %entry.path().display(), error = %e, "Skipping unreadable flow file");
            }
        }
    }

    Ok(Box::new(iters.into_iter().flatten()))
}

pub fn read_mitmproxy_file(path: &Path) -> Result<Vec<Box<dyn CapturedRequest>>> {
    Ok(stream_mitmproxy_file(path)?
        .filter_map(|r| r.ok())
        .collect())
}

pub fn read_mitmproxy_dir(path: &Path) -> Result<Vec<Box<dyn CapturedRequest>>> {
    Ok(stream_mitmproxy_dir(path)?.filter_map(|r| r.ok()).collect())
}

/// Heuristic: does this file look like a mitmproxy flow dump?
/// First byte is ASCII digit → likely tnetstring → mitmproxy flow.
pub fn mitmproxy_heuristic(path: &Path) -> bool {
    if path.is_dir() {
        return false;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut reader = std::io::BufReader::new(file);
    let mut buf = [0u8; 1];
    use std::io::Read;
    matches!(reader.read(&mut buf), Ok(1) if buf[0].is_ascii_digit())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join("flows")
            .join(name)
    }

    fn har_fixture(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join("har")
            .join(name)
    }

    #[test]
    fn parse_simple_get() {
        let requests = read_mitmproxy_file(&fixture("simple_get.flow")).unwrap();
        assert_eq!(requests.len(), 1);

        let r = &requests[0];
        assert_eq!(r.get_method(), "GET");
        assert!(
            r.get_url().contains("api.example.com"),
            "url: {}",
            r.get_url()
        );
        assert_eq!(r.get_response_status_code(), Some(200));
    }

    #[test]
    fn parse_post_json() {
        let requests = read_mitmproxy_file(&fixture("post_json.flow")).unwrap();
        assert_eq!(requests.len(), 1);

        let r = &requests[0];
        assert_eq!(r.get_method(), "POST");
        let body = r.get_request_body().expect("should have request body");
        let body_str = String::from_utf8_lossy(body);
        assert!(!body_str.is_empty(), "POST JSON body should not be empty");
    }

    #[test]
    fn parse_post_form() {
        let requests = read_mitmproxy_file(&fixture("post_form.flow")).unwrap();
        assert_eq!(requests.len(), 1);

        let r = &requests[0];
        assert_eq!(r.get_method(), "POST");
        let body = r.get_request_body().expect("should have request body");
        let body_str = String::from_utf8_lossy(body);
        assert!(!body_str.is_empty(), "POST form body should not be empty");
    }

    #[test]
    fn parse_multi_status() {
        let requests = read_mitmproxy_file(&fixture("multi_status.flow")).unwrap();
        assert_eq!(requests.len(), 4, "multi_status.flow should have 4 flows");

        let statuses: Vec<_> = requests
            .iter()
            .filter_map(|r| r.get_response_status_code())
            .collect();
        assert_eq!(statuses.len(), 4);
        let mut unique = statuses.clone();
        unique.sort();
        unique.dedup();
        assert!(
            unique.len() > 1,
            "multi_status should have different status codes, got: {statuses:?}"
        );
    }

    #[test]
    fn parse_no_response() {
        let requests = read_mitmproxy_file(&fixture("no_response.flow")).unwrap();
        assert_eq!(requests.len(), 1);

        let r = &requests[0];
        assert!(r.get_response_status_code().is_none());
        assert!(r.get_response_reason().is_none());
        assert!(r.get_response_headers().is_none());
        assert!(r.get_response_body().is_none());
        assert!(r.get_response_content_type().is_none());
    }

    #[test]
    fn parse_non_utf8() {
        let requests = read_mitmproxy_file(&fixture("non_utf8.flow")).unwrap();
        assert!(
            !requests.is_empty(),
            "non_utf8.flow should parse without panic"
        );

        let r = &requests[0];
        assert!(r.get_response_body().is_some());
    }

    #[test]
    fn parse_multiple() {
        let requests = read_mitmproxy_file(&fixture("multiple.flow")).unwrap();
        assert_eq!(requests.len(), 6, "multiple.flow should have 6 flows");
    }

    #[test]
    fn parse_corrupt_is_lenient() {
        let result = read_mitmproxy_file(&fixture("corrupt.flow"));
        assert!(
            result.is_ok(),
            "corrupt.flow should be handled leniently, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn heuristic_flow_file() {
        assert!(mitmproxy_heuristic(&fixture("simple_get.flow")));
    }

    #[test]
    fn heuristic_har_file() {
        assert!(!mitmproxy_heuristic(&har_fixture("simple.har")));
    }

    #[test]
    fn heuristic_directory() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata");
        assert!(!mitmproxy_heuristic(&dir));
    }

    #[test]
    fn stream_does_not_materialize_all() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        struct CountingReader<R> {
            inner: R,
            reads: Arc<AtomicUsize>,
        }

        impl<R: std::io::Read> std::io::Read for CountingReader<R> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let n = self.inner.read(buf)?;
                if n > 0 {
                    self.reads.fetch_add(1, Ordering::Relaxed);
                }
                Ok(n)
            }
        }

        fn tnet_str(tag: u8, s: &str) -> String {
            format!("{}:{}{}", s.len(), s, tag as char)
        }
        fn tnet_dict(pairs: &[(&str, &str)]) -> String {
            let inner: String = pairs
                .iter()
                .map(|(k, v)| format!("{}{}", tnet_str(b';', k), v))
                .collect();
            format!("{}:{}}}", inner.len(), inner)
        }

        let flow_template = |i: u8| -> Vec<u8> {
            let path_s = format!("/test{i}");
            let path_val = tnet_str(b';', &path_s);
            let req = tnet_dict(&[
                ("method", &tnet_str(b';', "GET")),
                ("scheme", &tnet_str(b';', "https")),
                ("host", &tnet_str(b';', "api.example.com")),
                ("port", &tnet_str(b'#', "443")),
                ("path", &path_val),
            ]);
            let type_val = tnet_str(b';', "http");
            tnet_dict(&[("type", &type_val), ("request", &req)]).into_bytes()
        };

        let mut data = Vec::new();
        for i in 0..3u8 {
            data.extend_from_slice(&flow_template(i));
        }

        let reads = Arc::new(AtomicUsize::new(0));
        let counting = CountingReader {
            inner: std::io::Cursor::new(data),
            reads: reads.clone(),
        };

        let iter = crate::tnetstring::TNetStringIter::new(counting);
        let mut yielded = 0;
        for item in iter {
            assert!(item.is_ok(), "parse failed: {:?}", item.err());
            yielded += 1;
            if yielded == 1 {
                let r = reads.load(Ordering::Relaxed);
                assert!(r > 0, "should have done at least 1 read");
            }
        }
        assert_eq!(yielded, 3, "should yield exactly 3 flows");
    }

    #[test]
    fn read_flow_directory() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join("flows");
        let result = read_mitmproxy_dir(&dir);
        assert!(result.is_err() || !result.unwrap().is_empty());
    }

    fn make_flow(
        method: &str,
        scheme: &str,
        host: &str,
        port: i64,
        path: &str,
        status: Option<i64>,
    ) -> TNetValue {
        let mut req_fields: Vec<(TNetValue, TNetValue)> = vec![
            (
                TNetValue::String("method".into()),
                TNetValue::String(method.into()),
            ),
            (
                TNetValue::String("scheme".into()),
                TNetValue::String(scheme.into()),
            ),
            (
                TNetValue::String("host".into()),
                TNetValue::String(host.into()),
            ),
            (TNetValue::String("port".into()), TNetValue::Int(port)),
            (
                TNetValue::String("path".into()),
                TNetValue::String(path.into()),
            ),
        ];
        let _ = &mut req_fields;

        let mut flow_fields = vec![
            (
                TNetValue::String("type".into()),
                TNetValue::String("http".into()),
            ),
            (
                TNetValue::String("request".into()),
                TNetValue::Dict(req_fields),
            ),
        ];

        if let Some(code) = status {
            let resp = TNetValue::Dict(vec![(
                TNetValue::String("status_code".into()),
                TNetValue::Int(code),
            )]);
            flow_fields.push((TNetValue::String("response".into()), resp));
        }

        TNetValue::Dict(flow_fields)
    }

    #[test]
    fn port_out_of_range() {
        let flow = make_flow("GET", "https", "api.example.com", 65616, "/test", Some(200));
        let result = parse_flow(&flow);
        assert!(result.is_err(), "port 65616 should be rejected");
    }

    #[test]
    fn port_zero_rejected() {
        let flow = make_flow("GET", "https", "api.example.com", 0, "/test", Some(200));
        let wrapper = parse_flow(&flow).unwrap();
        assert!(
            !wrapper.url.contains(":0"),
            "port 0 should not appear in URL"
        );
    }

    #[test]
    fn status_out_of_range() {
        let flow = make_flow("GET", "https", "api.example.com", 443, "/test", Some(70152));
        let wrapper = parse_flow(&flow).unwrap();
        assert_eq!(
            wrapper.response_status_code, None,
            "status 70152 should be rejected"
        );
    }

    #[test]
    fn valid_port_and_status() {
        let flow = make_flow("GET", "https", "api.example.com", 8080, "/test", Some(200));
        let wrapper = parse_flow(&flow).unwrap();
        assert!(wrapper.url.contains(":8080"));
        assert_eq!(wrapper.response_status_code, Some(200));
    }

    #[test]
    fn scheme_whitelist() {
        let flow = make_flow("GET", "javascript", "evil.com", 443, "/test", Some(200));
        let result = parse_flow(&flow);
        assert!(result.is_err(), "javascript scheme should be rejected");
    }

    #[test]
    fn scheme_data_rejected() {
        let flow = make_flow("GET", "data", "x", 443, "/test", Some(200));
        assert!(parse_flow(&flow).is_err());
    }

    #[test]
    fn host_with_slash_rejected() {
        let flow = make_flow("GET", "https", "evil.com/path", 443, "/test", Some(200));
        assert!(parse_flow(&flow).is_err());
    }

    #[test]
    fn host_with_at_rejected() {
        let flow = make_flow("GET", "https", "user@host", 443, "/test", Some(200));
        assert!(parse_flow(&flow).is_err());
    }

    #[test]
    fn host_empty_rejected() {
        let flow = make_flow("GET", "https", "", 443, "/test", Some(200));
        assert!(parse_flow(&flow).is_err());
    }

    #[test]
    fn host_with_control_char_rejected() {
        let flow = make_flow("GET", "https", "evil\x00.com", 443, "/test", Some(200));
        assert!(parse_flow(&flow).is_err());
    }

    #[test]
    fn strict_utf8_no_collision() {
        let flow1 = {
            let mut req = vec![
                (
                    TNetValue::String("method".into()),
                    TNetValue::String("GET".into()),
                ),
                (
                    TNetValue::String("scheme".into()),
                    TNetValue::String("https".into()),
                ),
                (
                    TNetValue::String("host".into()),
                    TNetValue::Bytes(vec![0xFF, 0xFE]),
                ),
                (TNetValue::String("port".into()), TNetValue::Int(443)),
                (
                    TNetValue::String("path".into()),
                    TNetValue::String("/test".into()),
                ),
            ];
            let _ = &mut req;
            TNetValue::Dict(vec![
                (
                    TNetValue::String("type".into()),
                    TNetValue::String("http".into()),
                ),
                (TNetValue::String("request".into()), TNetValue::Dict(req)),
            ])
        };
        let flow2 = {
            let req = vec![
                (
                    TNetValue::String("method".into()),
                    TNetValue::String("GET".into()),
                ),
                (
                    TNetValue::String("scheme".into()),
                    TNetValue::String("https".into()),
                ),
                (
                    TNetValue::String("host".into()),
                    TNetValue::Bytes(vec![0xFE, 0xFF]),
                ),
                (TNetValue::String("port".into()), TNetValue::Int(443)),
                (
                    TNetValue::String("path".into()),
                    TNetValue::String("/test".into()),
                ),
            ];
            TNetValue::Dict(vec![
                (
                    TNetValue::String("type".into()),
                    TNetValue::String("http".into()),
                ),
                (TNetValue::String("request".into()), TNetValue::Dict(req)),
            ])
        };
        assert!(
            parse_flow(&flow1).is_err(),
            "invalid UTF-8 host should be rejected"
        );
        assert!(
            parse_flow(&flow2).is_err(),
            "invalid UTF-8 host should be rejected"
        );
    }

    #[test]
    fn header_name_cap() {
        let big_name = "X-".to_string() + &"A".repeat(MAX_HEADER_NAME_SIZE + 100);
        let headers = TNetValue::List(vec![TNetValue::List(vec![
            TNetValue::String(big_name),
            TNetValue::String("value".into()),
        ])]);
        let result = parse_headers(&headers);
        assert!(
            result.is_empty(),
            "header with oversized name should be dropped"
        );
    }

    #[test]
    fn header_value_cap() {
        let big_value = "V".repeat(MAX_HEADER_VALUE_SIZE + 100);
        let headers = TNetValue::List(vec![TNetValue::List(vec![
            TNetValue::String("X-Test".into()),
            TNetValue::String(big_value),
        ])]);
        let result = parse_headers(&headers);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].1.len(),
            MAX_HEADER_VALUE_SIZE,
            "header value should be truncated"
        );
    }

    #[test]
    fn body_cap_truncates() {
        let mut req_fields = vec![
            (
                TNetValue::String("method".into()),
                TNetValue::String("GET".into()),
            ),
            (
                TNetValue::String("scheme".into()),
                TNetValue::String("https".into()),
            ),
            (
                TNetValue::String("host".into()),
                TNetValue::String("api.example.com".into()),
            ),
            (TNetValue::String("port".into()), TNetValue::Int(443)),
            (
                TNetValue::String("path".into()),
                TNetValue::String("/test".into()),
            ),
        ];
        let large_body = vec![0x41u8; MAX_BODY_SIZE + 1024];
        req_fields.push((
            TNetValue::String("content".into()),
            TNetValue::Bytes(large_body),
        ));

        let resp = TNetValue::Dict(vec![
            (TNetValue::String("status_code".into()), TNetValue::Int(200)),
            (
                TNetValue::String("content".into()),
                TNetValue::Bytes(vec![0x42u8; MAX_BODY_SIZE + 512]),
            ),
        ]);

        let flow = TNetValue::Dict(vec![
            (
                TNetValue::String("type".into()),
                TNetValue::String("http".into()),
            ),
            (
                TNetValue::String("request".into()),
                TNetValue::Dict(req_fields),
            ),
            (TNetValue::String("response".into()), resp),
        ]);

        let wrapper = parse_flow(&flow).unwrap();
        let req_body = wrapper.request_body.as_ref().unwrap();
        assert_eq!(
            req_body.len(),
            MAX_BODY_SIZE,
            "request body should be truncated to MAX_BODY_SIZE"
        );
        let resp_body = wrapper.response_body.as_ref().unwrap();
        assert_eq!(
            resp_body.len(),
            MAX_BODY_SIZE,
            "response body should be truncated to MAX_BODY_SIZE"
        );
    }

    #[test]
    fn path_control_chars_stripped() {
        let flow = make_flow(
            "GET",
            "https",
            "api.example.com",
            443,
            "/api\x00/users\r\n",
            Some(200),
        );
        let wrapper = parse_flow(&flow).unwrap();
        assert!(
            wrapper.url.contains("/api/users"),
            "control chars should be stripped from path, got: {}",
            wrapper.url
        );
        assert!(
            !wrapper.url.contains('\x00'),
            "null byte should not be in URL"
        );
    }
}

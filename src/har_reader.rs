use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use base64::Engine;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::error::{Error, Result};
use crate::types::CapturedRequest;
use crate::MAX_BODY_SIZE;

const MAX_HEADER_NAME_SIZE: usize = 8 * 1024;
const MAX_HEADER_VALUE_SIZE: usize = 64 * 1024;

#[derive(Deserialize)]
struct StreamingHarEntry {
    request: StreamingHarRequest,
    response: StreamingHarResponse,
}

#[derive(Deserialize)]
struct StreamingHarRequest {
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<StreamingHarHeader>,
    #[serde(rename = "postData", default)]
    post_data: Option<StreamingHarPostData>,
}

#[derive(Deserialize)]
struct StreamingHarResponse {
    status: i64,
    #[serde(rename = "statusText", default)]
    status_text: String,
    #[serde(default)]
    headers: Vec<StreamingHarHeader>,
    #[serde(default)]
    content: StreamingHarContent,
}

#[derive(Deserialize)]
struct StreamingHarHeader {
    name: String,
    value: String,
}

#[derive(Deserialize, Default)]
struct StreamingHarPostData {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize, Default)]
struct StreamingHarContent {
    #[serde(default)]
    text: Option<String>,
    #[serde(rename = "mimeType", default)]
    mime_type: Option<String>,
    #[serde(default)]
    encoding: Option<String>,
}

pub struct HarFlowWrapper {
    url: String,
    method: String,
    request_headers: Vec<(String, String)>,
    request_body: Option<Vec<u8>>,
    response_status: Option<u16>,
    response_reason: String,
    response_headers: Vec<(String, String)>,
    response_body: Option<Vec<u8>>,
    response_content_type: Option<String>,
}

impl HarFlowWrapper {
    fn from_streaming_entry(entry: StreamingHarEntry) -> Option<Self> {
        let scheme = entry
            .request
            .url
            .split("://")
            .next()
            .unwrap_or("")
            .to_lowercase();
        if scheme != "http" && scheme != "https" {
            warn!(event = "scheme_rejected", scheme = %scheme, url = %entry.request.url, "skipping HAR entry with non-http scheme");
            return None;
        }

        let request_body = entry
            .request
            .post_data
            .and_then(|pd| pd.text)
            .map(|t| cap_body(t.into_bytes()));

        let response_content_type = entry.response.content.mime_type.clone();
        let response_body = decode_streaming_body(&entry.response.content);

        let response_status = match u16::try_from(entry.response.status)
            .ok()
            .filter(|s| (100..=599).contains(s))
        {
            Some(s) => Some(s),
            None => {
                warn!(
                    event = "status_out_of_range",
                    value = entry.response.status,
                    "dropping response status with invalid code"
                );
                None
            }
        };

        Some(Self {
            url: entry.request.url,
            method: entry.request.method,
            request_headers: cap_headers(entry.request.headers),
            request_body,
            response_status,
            response_reason: entry.response.status_text,
            response_headers: cap_headers(entry.response.headers),
            response_body,
            response_content_type,
        })
    }
}

fn cap_headers(headers: Vec<StreamingHarHeader>) -> Vec<(String, String)> {
    headers
        .into_iter()
        .filter_map(|h| {
            if h.name.len() > MAX_HEADER_NAME_SIZE {
                warn!(
                    event = "header_name_too_large",
                    size = h.name.len(),
                    max = MAX_HEADER_NAME_SIZE,
                    "dropping HAR header with oversized name"
                );
                return None;
            }
            let value = if h.value.len() > MAX_HEADER_VALUE_SIZE {
                warn!(
                    event = "header_value_too_large",
                    size = h.value.len(),
                    max = MAX_HEADER_VALUE_SIZE,
                    name = %h.name,
                    "truncating oversized HAR header value"
                );
                h.value
                    .get(..MAX_HEADER_VALUE_SIZE)
                    .unwrap_or(&h.value)
                    .to_string()
            } else {
                h.value
            };
            Some((h.name, value))
        })
        .collect()
}

fn cap_body(body: Vec<u8>) -> Vec<u8> {
    if body.len() > MAX_BODY_SIZE {
        warn!(
            event = "body_truncated",
            original_size = body.len(),
            truncated_to = MAX_BODY_SIZE,
            "truncating oversized body"
        );
        body.get(..MAX_BODY_SIZE).unwrap_or(&body).to_vec()
    } else {
        body
    }
}

fn decode_streaming_body(content: &StreamingHarContent) -> Option<Vec<u8>> {
    let text = content.text.as_deref()?;
    if content.encoding.as_deref() == Some("base64") {
        match base64::engine::general_purpose::STANDARD.decode(text) {
            Ok(decoded) => Some(cap_body(decoded)),
            Err(e) => {
                warn!(event = "base64_decode_failed", error = %e, "base64 body decode failed, dropping body");
                None
            }
        }
    } else {
        let body = text.as_bytes().to_vec();
        Some(cap_body(body))
    }
}

impl CapturedRequest for HarFlowWrapper {
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
        Some(&self.response_reason)
    }

    fn get_response_headers(&self) -> Option<&[(String, String)]> {
        Some(&self.response_headers)
    }

    fn get_response_body(&self) -> Option<&[u8]> {
        self.response_body.as_deref()
    }

    fn get_response_content_type(&self) -> Option<&str> {
        self.response_content_type.as_deref()
    }
}

fn read_byte(reader: &mut impl Read) -> Result<Option<u8>> {
    let mut buf = [0u8; 1];
    match reader.read(&mut buf)? {
        0 => Ok(None),
        _ => Ok(Some(buf[0])),
    }
}

fn skip_ws_byte(reader: &mut impl Read) -> Result<Option<u8>> {
    loop {
        match read_byte(reader)? {
            None => return Ok(None),
            Some(b) if b.is_ascii_whitespace() => continue,
            Some(b) => return Ok(Some(b)),
        }
    }
}

fn strip_bom_from_reader(reader: &mut BufReader<File>) -> Result<()> {
    let buf = reader.fill_buf()?;
    if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        reader.consume(3);
    }
    Ok(())
}

/// Scan forward through JSON until positioned just past `"entries": [`.
/// Tracks string boundaries so `"entries"` inside a value is not mistaken for the key.
fn find_entries_array_start(reader: &mut impl Read) -> Result<()> {
    let target = b"\"entries\"";
    let mut in_string = false;
    let mut escape_next = false;
    let mut match_pos: usize = 0;

    loop {
        let byte = read_byte(reader)?
            .ok_or_else(|| Error::HarParse("unexpected EOF: entries array not found".into()))?;

        if escape_next {
            escape_next = false;
            match_pos = 0;
            continue;
        }

        if byte == b'\\' && in_string {
            escape_next = true;
            match_pos = 0;
            continue;
        }

        if byte == b'"' {
            in_string = !in_string;
        }

        #[allow(clippy::indexing_slicing)] // match_pos is bounded by target.len()
        if byte == target[match_pos] {
            match_pos += 1;
            if match_pos == target.len() {
                let colon = skip_ws_byte(reader)?
                    .ok_or_else(|| Error::HarParse("unexpected EOF after entries key".into()))?;
                if colon == b':' {
                    let bracket = skip_ws_byte(reader)?.ok_or_else(|| {
                        Error::HarParse("unexpected EOF expecting entries array".into())
                    })?;
                    if bracket == b'[' {
                        return Ok(());
                    }
                }
                match_pos = 0;
            }
        } else if byte == target.first().copied().unwrap_or(0) {
            match_pos = 1;
        } else {
            match_pos = 0;
        }
    }
}

/// Read a balanced JSON object after the opening `{` has been consumed.
/// Tracks nesting depth across braces/brackets and handles string escapes.
fn read_json_object(reader: &mut impl Read) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(4096);
    buf.push(b'{');
    let mut depth: i32 = 1;
    let mut in_string = false;
    let mut escape_next = false;

    loop {
        let byte = read_byte(reader)?
            .ok_or_else(|| Error::HarParse("unexpected EOF inside entry object".into()))?;
        buf.push(byte);

        if escape_next {
            escape_next = false;
            continue;
        }

        if in_string {
            match byte {
                b'\\' => escape_next = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(buf);
                }
            }
            _ => {}
        }
    }
}

pub struct HarStreamIter {
    reader: BufReader<File>,
    done: bool,
    entry_index: usize,
}

impl HarStreamIter {
    fn new(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);

        strip_bom_from_reader(&mut reader)?;
        find_entries_array_start(&mut reader)?;

        Ok(Self {
            reader,
            done: false,
            entry_index: 0,
        })
    }
}

impl Iterator for HarStreamIter {
    type Item = Result<Box<dyn CapturedRequest>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let byte = match skip_ws_byte(&mut self.reader) {
            Ok(Some(b)) => b,
            Ok(None) => {
                self.done = true;
                return None;
            }
            Err(e) => {
                self.done = true;
                return Some(Err(e));
            }
        };

        if byte == b']' {
            self.done = true;
            return None;
        }

        let byte = if byte == b',' {
            match skip_ws_byte(&mut self.reader) {
                Ok(Some(b)) => b,
                Ok(None) => {
                    self.done = true;
                    return None;
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(e));
                }
            }
        } else {
            byte
        };

        if byte == b']' {
            self.done = true;
            return None;
        }

        if byte != b'{' {
            self.done = true;
            return Some(Err(Error::HarParse(format!(
                "expected '{{' at start of entry {}, got '{}'",
                self.entry_index, byte as char
            ))));
        }

        match read_json_object(&mut self.reader) {
            Ok(buf) => {
                let idx = self.entry_index;
                self.entry_index += 1;
                match serde_json::from_slice::<StreamingHarEntry>(&buf) {
                    Ok(entry) => match HarFlowWrapper::from_streaming_entry(entry) {
                        Some(wrapper) => Some(Ok(Box::new(wrapper) as Box<dyn CapturedRequest>)),
                        None => self.next(),
                    },
                    Err(e) => {
                        warn!(entry = idx, error = %e, "Failed to parse HAR entry");
                        Some(Err(Error::HarParse(format!("entry {idx}: {e}"))))
                    }
                }
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

type RequestIter = Box<dyn Iterator<Item = Result<Box<dyn CapturedRequest>>>>;

pub fn stream_har_file(path: &Path) -> Result<RequestIter> {
    if path.is_dir() {
        return stream_har_dir(path);
    }
    debug!(path = %path.display(), "Streaming HAR file");
    let iter = HarStreamIter::new(path)?;
    Ok(Box::new(iter))
}

fn stream_har_dir(path: &Path) -> Result<RequestIter> {
    stream_har_dir_inner(path, false)
}

pub fn stream_har_dir_no_symlinks(path: &Path) -> Result<RequestIter> {
    stream_har_dir_inner(path, true)
}

fn stream_har_dir_inner(path: &Path, reject_symlinks: bool) -> Result<RequestIter> {
    let mut dir_entries: Vec<_> = std::fs::read_dir(path)?
        .filter_map(|e| match e {
            Ok(entry) => Some(entry),
            Err(err) => {
                warn!(
                    event = "har_entry_skipped",
                    error = %err,
                    "skipping unreadable HAR directory entry"
                );
                None
            }
        })
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("har"))
        })
        .filter(|e| {
            if reject_symlinks {
                match e.path().symlink_metadata() {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        warn!(
                            event = "symlink_rejected",
                            path = %e.path().display(),
                            "skipping symlinked HAR directory entry"
                        );
                        false
                    }
                    _ => true,
                }
            } else {
                true
            }
        })
        .collect();
    dir_entries.sort_by_key(|e| e.path());

    let iter = dir_entries
        .into_iter()
        .flat_map(|entry| match HarStreamIter::new(&entry.path()) {
            Ok(it) => {
                debug!(path = %entry.path().display(), "Streaming HAR file from directory");
                Box::new(it) as Box<dyn Iterator<Item = Result<Box<dyn CapturedRequest>>>>
            }
            Err(e) => {
                warn!(path = %entry.path().display(), error = %e, "Skipping unparseable HAR file");
                Box::new(std::iter::empty())
            }
        });

    Ok(Box::new(iter))
}

pub fn read_har_file(path: &Path) -> Result<Vec<Box<dyn CapturedRequest>>> {
    stream_har_file(path)?.collect()
}

pub fn har_heuristic(path: &Path) -> bool {
    if path.is_dir() {
        return false;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    use std::io::Read as _;
    let mut buf = [0u8; 4096];
    let mut reader = std::io::BufReader::new(file);
    let n = match reader.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return false,
    };
    let data = buf.get(..n).unwrap_or(&buf);
    let clean = if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        data.get(3..).unwrap_or_default()
    } else {
        data
    };
    clean
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|&b| b == b'{')
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join("har")
            .join(name)
    }

    #[test]
    fn parse_simple_har() {
        let requests = read_har_file(&fixture("simple.har")).unwrap();
        assert_eq!(requests.len(), 1);

        let r = &requests[0];
        assert_eq!(r.get_url(), "https://api.example.com/api/v1/users");
        assert_eq!(r.get_method(), "GET");
        assert_eq!(r.get_response_status_code(), Some(200));
        assert_eq!(r.get_response_reason(), Some("OK"));
        assert_eq!(r.get_response_content_type(), Some("application/json"));

        let req_headers = r.get_request_headers();
        assert!(req_headers
            .iter()
            .any(|(k, v)| k == "Host" && v == "api.example.com"));
        assert!(req_headers
            .iter()
            .any(|(k, v)| k == "Accept" && v == "application/json"));

        let resp_headers = r.get_response_headers().unwrap();
        assert!(resp_headers
            .iter()
            .any(|(k, v)| k == "Content-Type" && v == "application/json"));

        let body = r.get_response_body().unwrap();
        let body_str = std::str::from_utf8(body).unwrap();
        assert!(body_str.contains("Alice"));

        assert!(r.get_request_body().is_none());
    }

    #[test]
    fn parse_multi_har() {
        let requests = read_har_file(&fixture("multi.har")).unwrap();
        assert_eq!(requests.len(), 3);

        assert_eq!(requests[0].get_method(), "GET");
        assert_eq!(
            requests[0].get_url(),
            "https://api.example.com/api/v1/users"
        );

        assert_eq!(requests[1].get_method(), "POST");
        assert_eq!(requests[1].get_response_status_code(), Some(201));
        let post_body = requests[1].get_request_body().unwrap();
        assert!(std::str::from_utf8(post_body).unwrap().contains("Bob"));

        assert_eq!(requests[2].get_method(), "GET");
        assert_eq!(
            requests[2].get_url(),
            "https://api.example.com/api/v1/products"
        );
    }

    #[test]
    fn parse_base64_body_har() {
        let requests = read_har_file(&fixture("base64_body.har")).unwrap();
        assert_eq!(requests.len(), 1);

        let r = &requests[0];
        assert_eq!(r.get_url(), "https://api.example.com/api/v1/avatar/1.png");
        assert_eq!(r.get_response_content_type(), Some("image/png"));

        let body = r.get_response_body().unwrap();
        assert_eq!(&body[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn har_heuristic_positive() {
        assert!(har_heuristic(&fixture("simple.har")));
    }

    #[test]
    fn har_heuristic_non_har_file() {
        let flow_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join("flows");
        if flow_path.exists() {
            for entry in std::fs::read_dir(&flow_path).unwrap() {
                let entry = entry.unwrap();
                if entry.path().extension().is_some_and(|e| e != "har") {
                    let _ = har_heuristic(&entry.path());
                }
            }
        }
    }

    #[test]
    fn har_heuristic_directory() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata");
        assert!(!har_heuristic(&dir));
    }

    #[test]
    fn read_har_directory() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join("har");
        let requests = read_har_file(&dir).unwrap();
        assert_eq!(requests.len(), 5);
    }

    #[test]
    fn bom_stripping() {
        let path = fixture("simple.har");
        let original = std::fs::read(&path).unwrap();
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(&original);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &with_bom).unwrap();

        let requests = read_har_file(tmp.path()).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].get_url(),
            "https://api.example.com/api/v1/users"
        );
    }

    #[test]
    fn stream_har_does_not_materialize_all() {
        use std::io::Write;

        let mut har = String::from(
            r#"{"log":{"version":"1.2","creator":{"name":"test","version":"1.0"},"entries":["#,
        );
        let entry_count = 20;
        for i in 0..entry_count {
            if i > 0 {
                har.push(',');
            }
            har.push_str(&format!(
                r#"{{"request":{{"method":"GET","url":"https://example.com/api/item/{i}","headers":[]}},"response":{{"status":200,"statusText":"OK","headers":[],"content":{{"text":"{{\"id\":{i}}}"}}}}}}
"#,
            ));
        }
        har.push_str("]}}\n");

        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().write_all(har.as_bytes()).unwrap();
        tmp.as_file().sync_all().unwrap();

        let mut iter = stream_har_file(tmp.path()).unwrap();

        let first = iter.next().unwrap().unwrap();
        assert_eq!(first.get_url(), "https://example.com/api/item/0");
        assert_eq!(first.get_method(), "GET");

        let rest: Vec<_> = iter.collect::<Result<Vec<_>>>().unwrap();
        assert_eq!(rest.len(), entry_count - 1);
    }

    #[test]
    fn stream_matches_read_for_fixtures() {
        for name in &["simple.har", "multi.har", "base64_body.har"] {
            let path = fixture(name);
            let collected: Vec<_> = stream_har_file(&path)
                .unwrap()
                .collect::<Result<Vec<_>>>()
                .unwrap();

            let direct = read_har_file(&path).unwrap();

            assert_eq!(
                collected.len(),
                direct.len(),
                "entry count mismatch for {name}"
            );
            for (i, (a, b)) in collected.iter().zip(direct.iter()).enumerate() {
                assert_eq!(a.get_url(), b.get_url(), "{name} entry {i} url");
                assert_eq!(a.get_method(), b.get_method(), "{name} entry {i} method");
                assert_eq!(
                    a.get_response_status_code(),
                    b.get_response_status_code(),
                    "{name} entry {i} status"
                );
                assert_eq!(
                    a.get_response_body(),
                    b.get_response_body(),
                    "{name} entry {i} body"
                );
                assert_eq!(
                    a.get_request_body(),
                    b.get_request_body(),
                    "{name} entry {i} req body"
                );
                assert_eq!(
                    a.get_response_content_type(),
                    b.get_response_content_type(),
                    "{name} entry {i} content-type"
                );
            }
        }
    }

    #[test]
    fn stream_malformed_entry_returns_error() {
        use std::io::Write;

        let har = r#"{"log":{"version":"1.2","entries":[
            {"request":{"method":"GET","url":"https://ok.example.com","headers":[]},"response":{"status":200,"statusText":"OK","headers":[],"content":{}}},
            {"INVALID JSON STRUCTURE": true},
            {"request":{"method":"POST","url":"https://also-ok.example.com","headers":[]},"response":{"status":201,"statusText":"Created","headers":[],"content":{}}}
        ]}}"#;

        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().write_all(har.as_bytes()).unwrap();
        tmp.as_file().sync_all().unwrap();

        let results: Vec<_> = stream_har_file(tmp.path()).unwrap().collect();

        assert!(results[0].is_ok());
        assert_eq!(
            results[0].as_ref().unwrap().get_url(),
            "https://ok.example.com"
        );

        assert!(results[1].is_err());

        assert!(results[2].is_ok());
        assert_eq!(
            results[2].as_ref().unwrap().get_url(),
            "https://also-ok.example.com"
        );
    }

    #[test]
    fn stream_empty_entries_array() {
        use std::io::Write;

        let har = r#"{"log":{"version":"1.2","entries":[]}}"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().write_all(har.as_bytes()).unwrap();
        tmp.as_file().sync_all().unwrap();

        let results: Vec<_> = stream_har_file(tmp.path()).unwrap().collect();
        assert!(results.is_empty());
    }

    #[test]
    fn base64_decode_error_reported() {
        use std::io::Write;

        let har = r#"{"log":{"version":"1.2","entries":[
            {"request":{"method":"GET","url":"https://example.com/api","headers":[]},
             "response":{"status":200,"statusText":"OK","headers":[],"content":{"text":"Zm9vYg","encoding":"base64","mimeType":"application/octet-stream"}}}
        ]}}"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().write_all(har.as_bytes()).unwrap();
        tmp.as_file().sync_all().unwrap();

        let results: Vec<_> = stream_har_file(tmp.path())
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].get_response_body().is_none(),
            "invalid base64 body should be None"
        );
    }

    #[test]
    fn har_scheme_whitelist() {
        use std::io::Write;

        let har = r#"{"log":{"version":"1.2","entries":[
            {"request":{"method":"GET","url":"javascript:alert(1)","headers":[]},
             "response":{"status":200,"statusText":"OK","headers":[],"content":{}}},
            {"request":{"method":"GET","url":"https://example.com/api","headers":[]},
             "response":{"status":200,"statusText":"OK","headers":[],"content":{}}}
        ]}}"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().write_all(har.as_bytes()).unwrap();
        tmp.as_file().sync_all().unwrap();

        let results: Vec<_> = stream_har_file(tmp.path())
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(results.len(), 1, "javascript: scheme should be skipped");
        assert_eq!(results[0].get_url(), "https://example.com/api");
    }

    #[test]
    fn har_status_out_of_range() {
        use std::io::Write;

        let har = r#"{"log":{"version":"1.2","entries":[
            {"request":{"method":"GET","url":"https://example.com/api","headers":[]},
             "response":{"status":70152,"statusText":"Bogus","headers":[],"content":{}}}
        ]}}"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().write_all(har.as_bytes()).unwrap();
        tmp.as_file().sync_all().unwrap();

        let results: Vec<_> = stream_har_file(tmp.path())
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].get_response_status_code(),
            None,
            "status 70152 should be rejected"
        );
    }
}

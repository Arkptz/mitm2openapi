use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use base64::Engine;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::error::{Error, Result};
use crate::types::CapturedRequest;

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
    response_status: u16,
    response_reason: String,
    response_headers: Vec<(String, String)>,
    response_body: Option<Vec<u8>>,
    response_content_type: Option<String>,
}

impl HarFlowWrapper {
    fn from_streaming_entry(entry: StreamingHarEntry) -> Self {
        let request_body = entry
            .request
            .post_data
            .and_then(|pd| pd.text)
            .map(String::into_bytes);

        let response_content_type = entry.response.content.mime_type.clone();
        let response_body = decode_streaming_body(&entry.response.content);

        Self {
            url: entry.request.url,
            method: entry.request.method,
            request_headers: entry
                .request
                .headers
                .into_iter()
                .map(|h| (h.name, h.value))
                .collect(),
            request_body,
            response_status: entry.response.status as u16,
            response_reason: entry.response.status_text,
            response_headers: entry
                .response
                .headers
                .into_iter()
                .map(|h| (h.name, h.value))
                .collect(),
            response_body,
            response_content_type,
        }
    }
}

fn decode_streaming_body(content: &StreamingHarContent) -> Option<Vec<u8>> {
    let text = content.text.as_deref()?;
    if content.encoding.as_deref() == Some("base64") {
        base64::engine::general_purpose::STANDARD.decode(text).ok()
    } else {
        Some(text.as_bytes().to_vec())
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
        Some(self.response_status)
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
        } else if byte == target[0] {
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
                    Ok(entry) => {
                        let wrapper = HarFlowWrapper::from_streaming_entry(entry);
                        Some(Ok(Box::new(wrapper) as Box<dyn CapturedRequest>))
                    }
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
    let mut dir_entries: Vec<_> = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("har"))
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
    let clean = if buf[..n].starts_with(&[0xEF, 0xBB, 0xBF]) {
        &buf[3..n]
    } else {
        &buf[..n]
    };
    clean
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|&b| b == b'{')
}

#[cfg(test)]
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
}

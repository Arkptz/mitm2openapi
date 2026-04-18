use crate::error::{Error, Result};
use crate::types::CapturedRequest;
use base64::Engine;
use std::path::Path;

fn strip_bom(input: &[u8]) -> &[u8] {
    if input.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &input[3..]
    } else {
        input
    }
}

fn convert_headers(headers: &[har::v1_2::Headers]) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|h| (h.name.clone(), h.value.clone()))
        .collect()
}

fn decode_body(content: &har::v1_2::Content) -> Option<Vec<u8>> {
    let text = content.text.as_deref()?;
    if content.encoding.as_deref() == Some("base64") {
        base64::engine::general_purpose::STANDARD.decode(text).ok()
    } else {
        Some(text.as_bytes().to_vec())
    }
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
    fn from_entry(entry: &har::v1_2::Entries) -> Self {
        let req = &entry.request;
        let resp = &entry.response;

        let request_body = req
            .post_data
            .as_ref()
            .and_then(|pd| pd.text.as_deref())
            .map(|t| t.as_bytes().to_vec());

        let response_content_type = resp.content.mime_type.clone();

        Self {
            url: req.url.clone(),
            method: req.method.clone(),
            request_headers: convert_headers(&req.headers),
            request_body,
            response_status: resp.status as u16,
            response_reason: resp.status_text.clone(),
            response_headers: convert_headers(&resp.headers),
            response_body: decode_body(&resp.content),
            response_content_type,
        }
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

fn parse_har_bytes(bytes: &[u8]) -> Result<Vec<Box<dyn CapturedRequest>>> {
    let clean = strip_bom(bytes);
    let har_doc = har::from_slice(clean).map_err(|e| Error::HarParse(e.to_string()))?;

    let entries = match &har_doc.log {
        har::Spec::V1_2(log) => &log.entries,
        har::Spec::V1_3(_) => {
            return Err(Error::HarParse("HAR v1.3 not yet supported".into()));
        }
    };

    let requests: Vec<Box<dyn CapturedRequest>> = entries
        .iter()
        .map(|e| Box::new(HarFlowWrapper::from_entry(e)) as Box<dyn CapturedRequest>)
        .collect();

    Ok(requests)
}

pub fn read_har_file(path: &Path) -> Result<Vec<Box<dyn CapturedRequest>>> {
    if path.is_dir() {
        let mut all = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("har"))
            })
            .collect();
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let bytes = std::fs::read(entry.path())?;
            all.extend(parse_har_bytes(&bytes)?);
        }
        Ok(all)
    } else {
        let bytes = std::fs::read(path)?;
        parse_har_bytes(&bytes)
    }
}

pub fn har_heuristic(path: &Path) -> bool {
    if path.is_dir() {
        return false;
    }
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let clean = strip_bom(&bytes);
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

        let requests = parse_har_bytes(&with_bom).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].get_url(),
            "https://api.example.com/api/v1/users"
        );
    }
}

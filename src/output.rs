use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::Error;

/// Serialize an OpenAPI spec to a YAML string.
///
/// Uses `serde_yaml_ng` for serialization. Post-processes the output to ensure
/// the `openapi` version field is quoted as a string (e.g., `'3.0.3'`) rather
/// than being interpreted as a YAML float.
pub fn spec_to_yaml(spec: &openapiv3::OpenAPI) -> Result<String, Error> {
    let yaml = serde_yaml_ng::to_string(spec).map_err(|e| Error::Yaml(e.to_string()))?;

    // Post-process: quote the openapi version so YAML parsers don't treat it as a float.
    // Match `openapi: 3.0.3` (possibly with trailing whitespace) and replace with quoted form.
    let yaml = regex::Regex::new(r"(?m)^openapi: (\d+\.\d+\.\d+)\s*$")
        .expect("valid regex")
        .replace(&yaml, "openapi: '$1'")
        .into_owned();

    Ok(yaml)
}

/// Serialize discovered path templates to YAML for the discover output.
///
/// Produces output like:
/// ```yaml
/// x-path-templates:
/// - ignore:/api/v1/users  # <-- remove 'ignore:' prefix to include this path
/// - /api/v1/products
/// ```
///
/// Lines with the `ignore:` prefix get a comment appended explaining how to include them.
pub fn templates_to_yaml(templates: &[String]) -> Result<String, Error> {
    #[derive(serde::Serialize)]
    struct Wrapper {
        #[serde(rename = "x-path-templates")]
        x_path_templates: Vec<String>,
    }

    let wrapper = Wrapper {
        x_path_templates: templates.to_vec(),
    };

    let yaml = serde_yaml_ng::to_string(&wrapper).map_err(|e| Error::Yaml(e.to_string()))?;

    // Post-process: inject comments after lines containing `ignore:`.
    let mut result = String::with_capacity(yaml.len());
    for line in yaml.lines() {
        result.push_str(line);
        if line.contains("ignore:") {
            result.push_str("  # <-- remove 'ignore:' prefix to include this path");
        }
        result.push('\n');
    }

    Ok(result)
}

/// Write a YAML string to a file, creating parent directories if needed.
///
/// Uses an atomic write strategy: content is first written to a temporary file
/// in the same directory, then renamed into place. If the write fails (e.g.
/// disk full, permission denied), the original target file is left unchanged.
pub fn write_yaml(content: &str, path: &Path) -> Result<(), Error> {
    let parent = path.parent().ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output path has no parent directory",
        ))
    })?;
    // Handle empty parent (relative path like "spec.yaml" → parent is "")
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    fs::create_dir_all(parent)?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(content.as_bytes())?;
    tmp.persist(path).map_err(|e| Error::Io(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openapiv3::OpenAPI;

    #[test]
    fn spec_to_yaml_produces_valid_yaml() {
        let spec = OpenAPI {
            openapi: "3.0.3".to_string(),
            info: openapiv3::Info {
                title: "Test API".to_string(),
                version: "1.0.0".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let yaml = spec_to_yaml(&spec).unwrap();
        let parsed: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&yaml).expect("produced YAML should be valid");
        assert_eq!(parsed["info"]["title"].as_str().unwrap(), "Test API");
    }

    #[test]
    fn spec_to_yaml_quotes_openapi_version() {
        let spec = OpenAPI {
            openapi: "3.0.3".to_string(),
            info: openapiv3::Info {
                title: "Test".to_string(),
                version: "1.0.0".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let yaml = spec_to_yaml(&spec).unwrap();
        assert!(
            yaml.contains("openapi: '3.0.3'"),
            "openapi version should be quoted, got:\n{yaml}"
        );
    }

    #[test]
    fn templates_to_yaml_format() {
        let templates = vec![
            "ignore:/api/v1/users".to_string(),
            "ignore:/api/v1/users/{id}".to_string(),
            "/api/v1/products".to_string(),
        ];

        let yaml = templates_to_yaml(&templates).unwrap();

        assert!(yaml.contains("x-path-templates:"));
        assert!(yaml.contains("ignore:/api/v1/users"));
        assert!(yaml.contains("ignore:/api/v1/users/{id}"));
        assert!(yaml.contains("/api/v1/products"));
    }

    #[test]
    fn templates_to_yaml_comment_injection_on_ignore_lines() {
        let templates = vec![
            "ignore:/api/v1/users".to_string(),
            "/api/v1/products".to_string(),
        ];

        let yaml = templates_to_yaml(&templates).unwrap();

        for line in yaml.lines() {
            if line.contains("ignore:") {
                assert!(
                    line.contains("# <-- remove 'ignore:' prefix to include this path"),
                    "ignore lines should have comment, got: {line}"
                );
            }
        }
    }

    #[test]
    fn templates_to_yaml_no_comment_on_non_ignore_lines() {
        let templates = vec!["/api/v1/products".to_string()];

        let yaml = templates_to_yaml(&templates).unwrap();

        for line in yaml.lines() {
            if line.contains("/api/v1/products") && !line.contains("ignore:") {
                assert!(
                    !line.contains("# <--"),
                    "non-ignore lines should not have comment, got: {line}"
                );
            }
        }
    }

    #[test]
    fn templates_to_yaml_roundtrip() {
        let templates = vec![
            "ignore:/api/v1/users".to_string(),
            "/api/v1/products".to_string(),
        ];

        let yaml = templates_to_yaml(&templates).unwrap();

        let parsed: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&yaml).expect("YAML with comments should still parse");

        let list = parsed["x-path-templates"].as_sequence().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].as_str().unwrap(), "ignore:/api/v1/users");
        assert_eq!(list[1].as_str().unwrap(), "/api/v1/products");
    }

    #[test]
    fn write_yaml_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.yaml");

        write_yaml("openapi: '3.0.3'\n", &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "openapi: '3.0.3'\n");
    }

    #[test]
    fn write_yaml_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("output.yaml");

        write_yaml("test content\n", &path).unwrap();

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "test content\n");
    }
}

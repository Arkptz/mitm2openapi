//! Path template matching: convert path templates to regexes, match URLs against templates,
//! detect parameter segments, and suggest parameterized templates from observed paths.

use regex::Regex;
use std::collections::HashSet;

/// Convert a path template like "/api/v1/users/{id}" to a regex pattern.
///
/// Escapes special regex chars in literal parts, then inserts named capture groups
/// for `{param}` placeholders. The resulting regex is anchored with `^` and `$`.
fn is_valid_param_ident(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit())
}

pub fn path_to_regex(template: &str) -> Result<Regex, crate::error::Error> {
    let mut pattern = String::from("^");
    let mut remaining = template;

    while let Some(open) = remaining.find('{') {
        pattern.push_str(&regex::escape(&remaining[..open]));
        remaining = &remaining[open + 1..];

        if let Some(close) = remaining.find('}') {
            let param_name = &remaining[..close];
            if !is_valid_param_ident(param_name) {
                return Err(crate::error::Error::InvalidParamIdent {
                    name: param_name.to_string(),
                });
            }
            pattern.push_str(&format!("(?P<{}>[^/]+)", param_name));
            remaining = &remaining[close + 1..];
        } else {
            pattern.push_str(&regex::escape("{"));
        }
    }

    pattern.push_str(&regex::escape(remaining));
    pattern.push('$');

    Regex::new(&pattern)
        .map_err(|e| crate::error::Error::Schema(format!("invalid path regex: {}", e)))
}

/// Match a URL path against a list of templates. Returns the first matching template.
///
/// First match wins — template order matters.
pub fn match_path<'a>(path: &str, templates: &'a [String]) -> Option<&'a str> {
    for template in templates {
        if let Ok(re) = path_to_regex(template) {
            if re.is_match(path) {
                return Some(template);
            }
        }
    }
    None
}

/// Check if a string looks like a numeric value (all digits, possibly with leading minus).
fn is_numeric(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let s = s.strip_prefix('-').unwrap_or(s);
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Check if a string looks like a UUID (8-4-4-4-12 hex pattern).
fn is_uuid_str(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected_lens = [8, 4, 4, 4, 12];
    parts
        .iter()
        .zip(expected_lens.iter())
        .all(|(part, &len)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Check if a path segment looks like a parameter value (numeric or UUID).
///
/// Only returns true for actually parameterizable values — not arbitrary strings.
/// Optionally accepts a custom regex for additional matching patterns (e.g. `--param-regex`).
pub fn is_param_segment(segment: &str, custom_regex: Option<&Regex>) -> bool {
    if segment.is_empty() {
        return false;
    }
    if is_numeric(segment) || is_uuid_str(segment) {
        return true;
    }
    if let Some(re) = custom_regex {
        return re.is_match(segment);
    }
    false
}

/// Given a list of observed paths, suggest parameterized path templates.
///
/// Replaces segments that look like parameter values (numeric, UUID) with `{param}`
/// placeholders. Deduplicates resulting templates.
///
/// # Examples
/// ```
/// use mitm2openapi::path_matching::suggest_param_templates;
/// let paths = vec!["/users/1".to_string(), "/users/2".to_string()];
/// let templates = suggest_param_templates(&paths, None);
/// assert_eq!(templates, vec!["/users/{id}"]);
/// ```
pub fn suggest_param_templates(paths: &[String], custom_regex: Option<&Regex>) -> Vec<String> {
    let mut templates: HashSet<String> = HashSet::new();

    for path in paths {
        let segments: Vec<&str> = path.split('/').collect();
        let mut param_count = 0u32;
        let mut template_segments: Vec<String> = Vec::new();

        for segment in &segments {
            if segment.is_empty() {
                template_segments.push(String::new());
                continue;
            }
            if is_param_segment(segment, custom_regex) {
                param_count += 1;
                template_segments.push(format!("{{__P{}}}", param_count));
            } else {
                template_segments.push((*segment).to_string());
            }
        }

        let mut template = template_segments.join("/");

        if param_count == 1 {
            template = template.replace("{__P1}", "{id}");
        } else {
            for i in 1..=param_count {
                template = template.replace(&format!("{{__P{}}}", i), &format!("{{id{}}}", i));
            }
        }

        templates.insert(template);
    }

    let mut result: Vec<String> = templates.into_iter().collect();
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── path_to_regex ──────────────────────────────────────────────

    #[test]
    fn simple_path_matches_itself() {
        let re = path_to_regex("/api/v1/users").unwrap();
        assert!(re.is_match("/api/v1/users"));
    }

    #[test]
    fn simple_path_rejects_different_path() {
        let re = path_to_regex("/api/v1/users").unwrap();
        assert!(!re.is_match("/api/v1/posts"));
    }

    #[test]
    fn template_with_param_matches_numeric() {
        let re = path_to_regex("/api/v1/users/{id}").unwrap();
        assert!(re.is_match("/api/v1/users/123"));
    }

    #[test]
    fn template_with_param_matches_uuid() {
        let re = path_to_regex("/api/v1/users/{id}").unwrap();
        assert!(re.is_match("/api/v1/users/550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn template_does_not_match_extra_segments() {
        let re = path_to_regex("/api/v1/users/{id}").unwrap();
        assert!(!re.is_match("/api/v1/users/123/posts"));
    }

    #[test]
    fn multiple_params() {
        let re = path_to_regex("/api/v1/users/{user_id}/posts/{post_id}").unwrap();
        assert!(re.is_match("/api/v1/users/1/posts/42"));
    }

    #[test]
    fn special_chars_dot_in_path() {
        let re = path_to_regex("/api/v1/files/{id}.json").unwrap();
        assert!(re.is_match("/api/v1/files/123.json"));
        assert!(!re.is_match("/api/v1/files/123xjson"));
    }

    #[test]
    fn special_chars_plus_in_path() {
        let re = path_to_regex("/api/v1/search+results").unwrap();
        assert!(re.is_match("/api/v1/search+results"));
        assert!(!re.is_match("/api/v1/searchhresults"));
    }

    #[test]
    fn special_chars_question_mark() {
        let re = path_to_regex("/api/v1/maybe?").unwrap();
        assert!(re.is_match("/api/v1/maybe?"));
        assert!(!re.is_match("/api/v1/mayb"));
    }

    // ── match_path (first match wins) ──────────────────────────────

    #[test]
    fn first_match_wins_param_before_literal() {
        let templates = vec![
            "/api/v1/users/{id}".to_string(),
            "/api/v1/users/me".to_string(),
        ];
        assert_eq!(
            match_path("/api/v1/users/me", &templates),
            Some("/api/v1/users/{id}")
        );
    }

    #[test]
    fn first_match_wins_literal_before_param() {
        let templates = vec![
            "/api/v1/users/me".to_string(),
            "/api/v1/users/{id}".to_string(),
        ];
        assert_eq!(
            match_path("/api/v1/users/me", &templates),
            Some("/api/v1/users/me")
        );
    }

    #[test]
    fn match_path_returns_none_when_no_match() {
        let templates = vec!["/api/v1/users/{id}".to_string()];
        assert_eq!(match_path("/api/v2/posts", &templates), None);
    }

    #[test]
    fn match_path_empty_templates() {
        assert_eq!(match_path("/anything", &[]), None);
    }

    // ── is_param_segment ───────────────────────────────────────────

    #[test]
    fn numeric_is_param() {
        assert!(is_param_segment("123", None));
        assert!(is_param_segment("0", None));
        assert!(is_param_segment("-42", None));
    }

    #[test]
    fn alpha_is_not_param() {
        assert!(!is_param_segment("abc", None));
        assert!(!is_param_segment("v1", None));
        assert!(!is_param_segment("users", None));
    }

    #[test]
    fn uuid_is_param() {
        assert!(is_param_segment(
            "550e8400-e29b-41d4-a716-446655440000",
            None
        ));
    }

    #[test]
    fn empty_is_not_param() {
        assert!(!is_param_segment("", None));
    }

    #[test]
    fn custom_regex_extends_matching() {
        let re = Regex::new(r"^[a-f0-9]{8}$").unwrap();
        assert!(is_param_segment("abcd1234", Some(&re)));
        assert!(!is_param_segment("xyz", Some(&re)));
        assert!(is_param_segment("123", Some(&re)));
    }

    // ── suggest_param_templates ────────────────────────────────────

    #[test]
    fn suggest_replaces_numeric_segments() {
        let paths = vec!["/users/1".to_string(), "/users/2".to_string()];
        let templates = suggest_param_templates(&paths, None);
        assert_eq!(templates, vec!["/users/{id}"]);
    }

    #[test]
    fn suggest_replaces_uuid_segments() {
        let paths = vec![
            "/users/550e8400-e29b-41d4-a716-446655440000".to_string(),
            "/users/660e8400-e29b-41d4-a716-446655440001".to_string(),
        ];
        let templates = suggest_param_templates(&paths, None);
        assert_eq!(templates, vec!["/users/{id}"]);
    }

    #[test]
    fn suggest_multiple_params() {
        let paths = vec![
            "/users/1/posts/10".to_string(),
            "/users/2/posts/20".to_string(),
        ];
        let templates = suggest_param_templates(&paths, None);
        assert_eq!(templates, vec!["/users/{id1}/posts/{id2}"]);
    }

    #[test]
    fn suggest_preserves_non_param_paths() {
        let paths = vec!["/health".to_string(), "/api/status".to_string()];
        let templates = suggest_param_templates(&paths, None);
        assert!(templates.contains(&"/health".to_string()));
        assert!(templates.contains(&"/api/status".to_string()));
    }

    #[test]
    fn suggest_deduplicates() {
        let paths = vec![
            "/users/1".to_string(),
            "/users/2".to_string(),
            "/users/3".to_string(),
        ];
        let templates = suggest_param_templates(&paths, None);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0], "/users/{id}");
    }

    #[test]
    fn suggest_mixed_param_and_literal() {
        let paths = vec!["/users/1".to_string(), "/health".to_string()];
        let templates = suggest_param_templates(&paths, None);
        assert_eq!(templates.len(), 2);
        assert!(templates.contains(&"/health".to_string()));
        assert!(templates.contains(&"/users/{id}".to_string()));
    }

    // ── path_to_regex edge cases ───────────────────────────────────

    #[test]
    fn invalid_param_ident_rejected() {
        let err = path_to_regex("/users/{foo bar}");
        assert!(
            matches!(err, Err(crate::error::Error::InvalidParamIdent { .. })),
            "expected InvalidParamIdent, got {err:?}"
        );
    }

    #[test]
    fn digit_leading_param_rejected() {
        let err = path_to_regex("/users/{1abc}");
        assert!(matches!(
            err,
            Err(crate::error::Error::InvalidParamIdent { .. })
        ));
    }

    #[test]
    fn root_path() {
        let re = path_to_regex("/").unwrap();
        assert!(re.is_match("/"));
        assert!(!re.is_match("/anything"));
    }

    #[test]
    fn param_does_not_match_slash() {
        let re = path_to_regex("/users/{id}").unwrap();
        assert!(!re.is_match("/users/1/2"));
    }

    #[test]
    fn regex_special_chars_in_literal_parts() {
        let re = path_to_regex("/api/(v1)/data").unwrap();
        assert!(re.is_match("/api/(v1)/data"));
        assert!(!re.is_match("/api/v1/data"));
    }
}

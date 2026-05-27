use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TagRule {
    pub pattern: Regex,
    pub tag: String,
}

/// Strategy for assigning tags to API operations.
#[derive(Debug, Clone, Default)]
pub enum TagStrategy {
    /// Default: the builder calls its own `extract_tag()` logic.
    #[default]
    Legacy,
    /// Suppress all tags (empty `tags: []` on every operation).
    None,
    /// Extract the path segment at the given 0-based index.
    /// Segments are split by `/` with empty segments removed.
    /// Index 0 = first segment after the leading `/`.
    PathSegment { index: usize },
    /// First-match-wins regex rules with an optional default tag.
    Rules {
        rules: Vec<TagRule>,
        default: Option<String>,
    },
}

#[derive(Deserialize)]
struct RawTagRule {
    #[serde(rename = "match")]
    match_pattern: String,
    tag: String,
}

#[derive(Deserialize)]
struct RawTagRules {
    rules: Vec<RawTagRule>,
    default: Option<String>,
}

/// Load tag rules from a YAML file.
/// Returns `Err` if the file can't be read or any regex is invalid.
pub fn load_tag_rules(path: &Path) -> Result<TagStrategy> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read tag rules from {}", path.display()))?;
    let raw: RawTagRules = serde_yaml_ng::from_str(&content)
        .with_context(|| format!("failed to parse tag rules YAML from {}", path.display()))?;
    let rules = raw
        .rules
        .into_iter()
        .map(|r| {
            let pattern = Regex::new(&r.match_pattern)
                .with_context(|| format!("invalid regex in tag rule: {}", r.match_pattern))?;
            Ok(TagRule {
                pattern,
                tag: r.tag,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(TagStrategy::Rules {
        rules,
        default: raw.default,
    })
}

/// Apply the strategy to a URL path, returning the resolved tag or `None`.
/// For `Legacy` and `None` strategies, always returns `None` — the builder
/// handles these cases directly.
pub fn resolve_tag(strategy: &TagStrategy, path: &str) -> Option<String> {
    match strategy {
        TagStrategy::Legacy | TagStrategy::None => Option::None,
        TagStrategy::PathSegment { index } => {
            let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            segments.get(*index).map(|s| (*s).to_string())
        }
        TagStrategy::Rules { rules, default } => {
            for rule in rules {
                if rule.pattern.is_match(path) {
                    return Some(rule.tag.clone());
                }
            }
            default.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rules(patterns: &[(&str, &str)], default: Option<&str>) -> TagStrategy {
        TagStrategy::Rules {
            rules: patterns
                .iter()
                .map(|(pat, tag)| TagRule {
                    pattern: Regex::new(pat).unwrap(),
                    tag: (*tag).to_string(),
                })
                .collect(),
            default: default.map(String::from),
        }
    }

    #[test]
    fn match_first_wins() {
        let strategy = make_rules(
            &[
                ("^/api/v1/contract/", "Contract"),
                ("^/api/v1/private/order", "Order"),
            ],
            Option::None,
        );
        assert_eq!(
            resolve_tag(&strategy, "/api/v1/contract/ticker"),
            Some("Contract".to_string()),
        );
    }

    #[test]
    fn no_match_with_default() {
        let strategy = make_rules(&[("^/api/v1/contract/", "Contract")], Some("Default"));
        assert_eq!(
            resolve_tag(&strategy, "/api/v1/other"),
            Some("Default".to_string()),
        );
    }

    #[test]
    fn no_match_no_default() {
        let strategy = make_rules(&[("^/api/v1/contract/", "Contract")], Option::None);
        assert_eq!(resolve_tag(&strategy, "/api/v1/other"), Option::None);
    }

    #[test]
    fn regex_capture_groups() {
        let strategy = make_rules(&[("^/api/v1/(private/)?account", "Account")], Option::None);
        assert_eq!(
            resolve_tag(&strategy, "/api/v1/account"),
            Some("Account".to_string()),
        );
        assert_eq!(
            resolve_tag(&strategy, "/api/v1/private/account"),
            Some("Account".to_string()),
        );
    }

    #[test]
    fn empty_rules() {
        let with_default = make_rules(&[], Some("Fallback"));
        assert_eq!(
            resolve_tag(&with_default, "/anything"),
            Some("Fallback".to_string()),
        );

        let without_default = make_rules(&[], Option::None);
        assert_eq!(resolve_tag(&without_default, "/anything"), Option::None);
    }

    #[test]
    fn path_segment_strategy() {
        let idx0 = TagStrategy::PathSegment { index: 0 };
        assert_eq!(
            resolve_tag(&idx0, "/api/v1/contract/ticker"),
            Some("api".to_string()),
        );

        let idx2 = TagStrategy::PathSegment { index: 2 };
        assert_eq!(
            resolve_tag(&idx2, "/api/v1/contract/ticker"),
            Some("contract".to_string()),
        );
    }

    #[test]
    fn path_segment_out_of_bounds() {
        let strategy = TagStrategy::PathSegment { index: 10 };
        assert_eq!(
            resolve_tag(&strategy, "/api/v1/contract/ticker"),
            Option::None,
        );
    }

    #[test]
    fn load_tag_rules_from_yaml() {
        let yaml = "\
rules:
  - match: \"^/api/v1/contract/\"
    tag: Contract
  - match: \"^/api/v1/private/order\"
    tag: Order
default: Default
";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tags.yaml");
        std::fs::write(&path, yaml).unwrap();

        let strategy = load_tag_rules(&path).unwrap();
        assert_eq!(
            resolve_tag(&strategy, "/api/v1/contract/ticker"),
            Some("Contract".to_string()),
        );
        assert_eq!(
            resolve_tag(&strategy, "/api/v1/private/order/123"),
            Some("Order".to_string()),
        );
        assert_eq!(
            resolve_tag(&strategy, "/api/v1/other"),
            Some("Default".to_string()),
        );
    }

    #[test]
    fn load_tag_rules_invalid_regex() {
        let yaml = "\
rules:
  - match: \"[invalid\"
    tag: Bad
";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tags.yaml");
        std::fs::write(&path, yaml).unwrap();

        assert!(load_tag_rules(&path).is_err());
    }
}

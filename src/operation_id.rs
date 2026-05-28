use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// Strategy for generating operationId values.
#[derive(Clone, Debug, Default)]
pub enum OperationIdStrategy {
    /// Do not generate operationId.
    #[default]
    None,
    /// Derive operationId from HTTP method + path template.
    Path,
    /// Use a custom template string with `{method}` and `{path}` placeholders.
    Template(String),
}

/// Derive an operationId from method + template path.
///
/// `template_path` is the OpenAPI path template (e.g. "/api/v1/users/{id}").
pub fn derive_operation_id(
    method: &str,
    template_path: &str,
    strategy: &OperationIdStrategy,
) -> Option<String> {
    match strategy {
        OperationIdStrategy::None => None,
        OperationIdStrategy::Template(tmpl) => Some(
            tmpl.replace("{method}", method)
                .replace("{path}", template_path),
        ),
        OperationIdStrategy::Path => derive_from_path(method, template_path),
    }
}

/// Resolve collisions in a set of operations.
///
/// Input: mutable vec of `(path, method, Option<operationId>)`.
/// Collision rule: sort by `(path, method)`, first keeps name, rest get `_2`, `_3`, etc.
pub fn resolve_collisions(operations: &mut [(String, String, Option<String>)]) {
    operations.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));

    let mut seen: HashMap<String, usize> = HashMap::new();
    for op in operations.iter_mut() {
        if let Some(ref mut id) = op.2 {
            let count = seen.entry(id.clone()).or_insert(0);
            *count += 1;
            if *count > 1 {
                *id = format!("{}_{}", id, count);
            }
        }
    }
}

/// Load operationId overrides from a YAML file.
///
/// YAML format: `"METHOD /path": operationId`
pub fn load_overrides(path: &Path) -> Result<HashMap<String, String>> {
    let content = std::fs::read_to_string(path)?;
    let map: HashMap<String, String> = serde_yaml_ng::from_str(&content)?;
    Ok(map)
}

fn is_param(segment: &str) -> bool {
    segment.starts_with('{') && segment.ends_with('}')
}

fn derive_from_path(method: &str, template_path: &str) -> Option<String> {
    let segments: Vec<&str> = template_path.split('/').filter(|s| !s.is_empty()).collect();
    let is_item = segments.last().is_some_and(|s| is_param(s));
    let is_collection = !is_item;

    let non_params: Vec<&str> = segments.iter().filter(|s| !is_param(s)).copied().collect();

    if method.eq_ignore_ascii_case("POST") && is_collection && non_params.len() >= 2 {
        if let Some(&last) = non_params.last() {
            if !last.ends_with('s') {
                let verb = last;
                let noun_idx = non_params.len().checked_sub(2)?;
                let noun = non_params.get(noun_idx)?;
                return Some(format!("{}{}", verb, to_pascal_case(noun)));
            }
        }
    }

    let verb = method_to_verb(method, is_collection);
    let mut name_segs = path_to_name_segments(&segments);

    if method.eq_ignore_ascii_case("POST") && is_collection {
        if let Some(last) = name_segs.last_mut() {
            *last = singularize_pascal(last);
        }
    }

    Some(to_camel_case(verb, &name_segs))
}

fn method_to_verb(method: &str, is_collection: bool) -> &'static str {
    match method.to_ascii_uppercase().as_str() {
        "GET" if is_collection => "list",
        "GET" => "get",
        "POST" => "create",
        "PUT" => "update",
        "DELETE" => "delete",
        "PATCH" => "patch",
        _ => "handle",
    }
}

fn path_to_name_segments(segments: &[&str]) -> Vec<String> {
    let last_non_param_pos = segments.iter().rposition(|s| !is_param(s));
    let last_param_pos = segments.iter().rposition(|s| is_param(s));

    let mut result = Vec::new();

    if let Some(param_pos) = last_param_pos {
        if let Some(before_pos) = param_pos.checked_sub(1) {
            if let Some(&before) = segments.get(before_pos) {
                if !is_param(before) {
                    let singular = singularize(before);
                    result.push(to_pascal_case(&singular));

                    if let Some(lnp_pos) = last_non_param_pos {
                        if lnp_pos != before_pos {
                            if let Some(&last) = segments.get(lnp_pos) {
                                result.push(to_pascal_case(last));
                            }
                        }
                    }
                    return result;
                }
            }
        }
    }

    if let Some(pos) = last_non_param_pos {
        if let Some(&seg) = segments.get(pos) {
            result.push(to_pascal_case(seg));
        }
    }

    result
}

fn to_pascal_case(s: &str) -> String {
    s.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{}{}", upper, chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect()
}

fn to_camel_case(verb: &str, segments: &[String]) -> String {
    let mut result = verb.to_string();
    for seg in segments {
        result.push_str(seg);
    }
    result
}

fn singularize(word: &str) -> String {
    if word.len() > 1 && word.ends_with('s') && !word.ends_with("ss") {
        word[..word.len() - 1].to_string()
    } else {
        word.to_string()
    }
}

fn singularize_pascal(word: &str) -> String {
    if word.len() > 1 && word.ends_with('s') && !word.ends_with("ss") {
        word[..word.len() - 1].to_string()
    } else {
        word.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn get_collection() {
        let result = derive_operation_id("GET", "/api/v1/users", &OperationIdStrategy::Path);
        assert_eq!(result, Some("listUsers".to_string()));
    }

    #[test]
    fn get_item() {
        let result = derive_operation_id("GET", "/api/v1/users/{id}", &OperationIdStrategy::Path);
        assert_eq!(result, Some("getUser".to_string()));
    }

    #[test]
    fn post() {
        let result = derive_operation_id("POST", "/api/v1/users", &OperationIdStrategy::Path);
        assert_eq!(result, Some("createUser".to_string()));
    }

    #[test]
    fn put_item() {
        let result = derive_operation_id("PUT", "/api/v1/users/{id}", &OperationIdStrategy::Path);
        assert_eq!(result, Some("updateUser".to_string()));
    }

    #[test]
    fn delete_item() {
        let result =
            derive_operation_id("DELETE", "/api/v1/users/{id}", &OperationIdStrategy::Path);
        assert_eq!(result, Some("deleteUser".to_string()));
    }

    #[test]
    fn patch_item() {
        let result = derive_operation_id("PATCH", "/api/v1/users/{id}", &OperationIdStrategy::Path);
        assert_eq!(result, Some("patchUser".to_string()));
    }

    #[test]
    fn nested_resource() {
        let result = derive_operation_id(
            "GET",
            "/api/v1/users/{id}/orders",
            &OperationIdStrategy::Path,
        );
        assert_eq!(result, Some("listUserOrders".to_string()));
    }

    #[test]
    fn deep_path() {
        let result = derive_operation_id(
            "GET",
            "/api/v1/contract/fair_price/{symbol}",
            &OperationIdStrategy::Path,
        );
        assert_eq!(result, Some("getFairPrice".to_string()));
    }

    #[test]
    fn deep_post() {
        let result = derive_operation_id(
            "POST",
            "/api/v1/private/order/place",
            &OperationIdStrategy::Path,
        );
        assert_eq!(result, Some("placeOrder".to_string()));
    }

    #[test]
    fn strategy_none() {
        let result = derive_operation_id("GET", "/api/v1/users", &OperationIdStrategy::None);
        assert_eq!(result, None);
    }

    #[test]
    fn collision_resolution() {
        let mut ops = vec![
            (
                "/api/v1/users".to_string(),
                "GET".to_string(),
                Some("listUsers".to_string()),
            ),
            (
                "/api/v2/users".to_string(),
                "GET".to_string(),
                Some("listUsers".to_string()),
            ),
        ];
        resolve_collisions(&mut ops);

        assert_eq!(ops.first().unwrap().2, Some("listUsers".to_string()));
        assert_eq!(ops.get(1).unwrap().2, Some("listUsers_2".to_string()));

        let mut ops2 = vec![
            (
                "/api/v2/users".to_string(),
                "GET".to_string(),
                Some("listUsers".to_string()),
            ),
            (
                "/api/v1/users".to_string(),
                "GET".to_string(),
                Some("listUsers".to_string()),
            ),
        ];
        resolve_collisions(&mut ops2);
        assert_eq!(ops2.first().unwrap().2, Some("listUsers".to_string()));
        assert_eq!(ops2.get(1).unwrap().2, Some("listUsers_2".to_string()));
    }

    #[test]
    fn override_wins() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("overrides.yaml");
        let mut f = std::fs::File::create(&file_path).unwrap();
        writeln!(f, "\"GET /api/v1/users\": getAllUsers").unwrap();
        drop(f);

        let overrides = load_overrides(&file_path).unwrap();
        let key = "GET /api/v1/users";
        assert_eq!(overrides.get(key), Some(&"getAllUsers".to_string()));
    }

    #[test]
    fn template_strategy() {
        let result = derive_operation_id(
            "GET",
            "/api/v1/users",
            &OperationIdStrategy::Template("{method}_{path}".to_string()),
        );
        assert_eq!(result, Some("GET_/api/v1/users".to_string()));
    }

    #[test]
    fn pascal_case_snake() {
        assert_eq!(to_pascal_case("fair_price"), "FairPrice");
    }

    #[test]
    fn pascal_case_simple() {
        assert_eq!(to_pascal_case("users"), "Users");
    }

    #[test]
    fn singularize_plural() {
        assert_eq!(singularize("users"), "user");
        assert_eq!(singularize("orders"), "order");
    }

    #[test]
    fn singularize_already_singular() {
        assert_eq!(singularize("place"), "place");
        assert_eq!(singularize("fair_price"), "fair_price");
    }
}

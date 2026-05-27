//! Shared type-hint helpers for detecting numeric strings and UUIDs.
//!
//! Used by both schema inference (`schema.rs`) and path matching (`path_matching.rs`)
//! to classify dynamic segments.

/// Check if a string looks like a numeric value (all digits, possibly with leading minus).
pub(crate) fn is_numeric_string(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let s = s.strip_prefix('-').unwrap_or(s);
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

pub(crate) fn is_upper_case_slug(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let all_upper_digit_underscore = s
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !all_upper_digit_underscore {
        return false;
    }
    let has_underscore = s.contains('_');
    let all_alpha = s.chars().all(|c| c.is_ascii_uppercase());
    if has_underscore {
        s.len() >= 3
    } else {
        all_alpha && s.len() >= 4
    }
}

pub(crate) fn is_hex_string(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix("0x") {
        rest.len() >= 8 && rest.chars().all(|c| c.is_ascii_hexdigit())
    } else {
        s.len() >= 16 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    }
}

pub(crate) fn is_base58(s: &str) -> bool {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    s.len() >= 20 && s.bytes().all(|b| ALPHABET.contains(&b))
}

/// Check if a string looks like a UUID (8-4-4-4-12 hex pattern).
pub(crate) fn is_uuid(s: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_string_valid() {
        assert!(is_numeric_string("0"));
        assert!(is_numeric_string("123"));
        assert!(is_numeric_string("-1"));
        assert!(is_numeric_string("-999"));
    }

    #[test]
    fn numeric_string_invalid() {
        assert!(!is_numeric_string(""));
        assert!(!is_numeric_string("abc"));
        assert!(!is_numeric_string("12.3"));
        assert!(!is_numeric_string("1a2"));
        assert!(!is_numeric_string("-"));
    }

    #[test]
    fn uuid_valid() {
        assert!(is_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_uuid("00000000-0000-0000-0000-000000000000"));
        assert!(is_uuid("ABCDEF01-2345-6789-abcd-ef0123456789"));
    }

    #[test]
    fn uuid_invalid() {
        assert!(!is_uuid(""));
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("550e8400-e29b-41d4-a716"));
        assert!(!is_uuid("550e8400-e29b-41d4-a716-44665544000"));
        assert!(!is_uuid("550e8400-e29b-41d4-a716-4466554400000"));
        assert!(!is_uuid("ZZZZZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZZZZZZZZZ"));
    }
}

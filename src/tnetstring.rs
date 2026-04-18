//! TNetString (Tagged Netstring) parser for mitmproxy flow files.
//!
//! Format: `<length>:<data><tag>` where:
//! - Length is ASCII decimal digits (max 12 digits)
//! - Data is raw bytes of the specified length
//! - Tag is a single ASCII character identifying the type
//!
//! Supports the mitmproxy-specific `;` tag for Unicode strings.

use std::io::Read;

use crate::error::Error;

/// Maximum number of digits in the length prefix (mitmproxy enforces 12).
const MAX_LENGTH_DIGITS: usize = 12;

/// A parsed TNetString value.
#[derive(Debug, Clone, PartialEq)]
pub enum TNetValue {
    /// Raw bytes (tag `,`)
    Bytes(Vec<u8>),
    /// Unicode string (tag `;` — mitmproxy extension)
    String(String),
    /// Integer (tag `#`)
    Int(i64),
    /// Float (tag `^`)
    Float(f64),
    /// Boolean (tag `!`)
    Bool(bool),
    /// Null (tag `~`)
    Null,
    /// Ordered list (tag `]`)
    List(Vec<TNetValue>),
    /// Ordered dict preserving insertion order (tag `}`)
    Dict(Vec<(TNetValue, TNetValue)>),
}

impl TNetValue {
    pub fn as_dict(&self) -> Option<&[(TNetValue, TNetValue)]> {
        match self {
            TNetValue::Dict(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[TNetValue]> {
        match self {
            TNetValue::List(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            TNetValue::Bytes(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            TNetValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            TNetValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            TNetValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            TNetValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, TNetValue::Null)
    }

    /// Look up a value in a dict by string key.
    /// Matches against both `TNetValue::String` and `TNetValue::Bytes` keys.
    pub fn get(&self, key: &str) -> Option<&TNetValue> {
        let dict = self.as_dict()?;
        for (k, v) in dict {
            match k {
                TNetValue::String(s) if s == key => return Some(v),
                TNetValue::Bytes(b) if b == key.as_bytes() => return Some(v),
                _ => {}
            }
        }
        None
    }
}

struct TrackingReader<R> {
    inner: R,
    offset: usize,
}

impl<R: Read> TrackingReader<R> {
    fn new(inner: R) -> Self {
        Self { inner, offset: 0 }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        self.inner
            .read_exact(buf)
            .map_err(|e| self.make_error(format!("unexpected end of input: {e}")))?;
        self.offset += buf.len();
        Ok(())
    }

    fn read_byte(&mut self) -> Result<Option<u8>, Error> {
        let mut buf = [0u8; 1];
        match self.inner.read(&mut buf) {
            Ok(0) => Ok(None),
            Ok(_) => {
                self.offset += 1;
                Ok(Some(buf[0]))
            }
            Err(e) => Err(self.make_error(format!("read error: {e}"))),
        }
    }

    fn make_error(&self, message: String) -> Error {
        Error::TNetParse {
            offset: self.offset,
            message,
        }
    }
}

fn parse_length<R: Read>(reader: &mut TrackingReader<R>) -> Result<Option<usize>, Error> {
    let mut digits = Vec::with_capacity(12);
    let start_offset = reader.offset;

    let first = match reader.read_byte()? {
        Some(b) => b,
        None => return Ok(None),
    };

    if first == b':' {
        return Err(reader.make_error("empty length prefix".into()));
    }

    if !first.is_ascii_digit() {
        return Err(reader.make_error(format!(
            "expected digit in length prefix, got {:?}",
            first as char
        )));
    }
    digits.push(first);

    loop {
        let b = reader
            .read_byte()?
            .ok_or_else(|| reader.make_error("unexpected EOF in length prefix".into()))?;

        if b == b':' {
            break;
        }

        if !b.is_ascii_digit() {
            return Err(reader.make_error(format!(
                "expected digit or ':' in length prefix, got {:?}",
                b as char
            )));
        }

        digits.push(b);

        if digits.len() > MAX_LENGTH_DIGITS {
            return Err(Error::TNetParse {
                offset: start_offset,
                message: format!("length prefix exceeds {} digits", MAX_LENGTH_DIGITS),
            });
        }
    }

    let s = std::str::from_utf8(&digits).expect("digits are ASCII");
    let len: usize = s.parse().map_err(|e| Error::TNetParse {
        offset: start_offset,
        message: format!("invalid length value: {e}"),
    })?;

    Ok(Some(len))
}

fn parse_value<R: Read>(reader: &mut TrackingReader<R>) -> Result<Option<TNetValue>, Error> {
    let len = match parse_length(reader)? {
        Some(l) => l,
        None => return Ok(None),
    };

    let mut data = vec![0u8; len];
    if len > 0 {
        reader.read_exact(&mut data)?;
    }

    let tag = reader
        .read_byte()?
        .ok_or_else(|| reader.make_error("unexpected EOF: expected type tag".into()))?;

    let value = match tag {
        b',' => TNetValue::Bytes(data),
        b';' => {
            let s = String::from_utf8(data)
                .map_err(|e| reader.make_error(format!("invalid UTF-8 in unicode string: {e}")))?;
            TNetValue::String(s)
        }
        b'#' => {
            let s = std::str::from_utf8(&data)
                .map_err(|e| reader.make_error(format!("invalid integer bytes: {e}")))?;
            let n: i64 = s
                .parse()
                .map_err(|e| reader.make_error(format!("invalid integer: {e}")))?;
            TNetValue::Int(n)
        }
        b'^' => {
            let s = std::str::from_utf8(&data)
                .map_err(|e| reader.make_error(format!("invalid float bytes: {e}")))?;
            let f: f64 = s
                .parse()
                .map_err(|e| reader.make_error(format!("invalid float: {e}")))?;
            TNetValue::Float(f)
        }
        b'!' => {
            let s = std::str::from_utf8(&data)
                .map_err(|e| reader.make_error(format!("invalid boolean bytes: {e}")))?;
            match s {
                "true" => TNetValue::Bool(true),
                "false" => TNetValue::Bool(false),
                _ => {
                    return Err(reader.make_error(format!(
                        "invalid boolean content: expected \"true\" or \"false\", got {s:?}"
                    )));
                }
            }
        }
        b'~' => {
            if len != 0 {
                return Err(reader.make_error(format!("null value must have length 0, got {len}")));
            }
            TNetValue::Null
        }
        b']' => {
            let items = parse_nested_list(&data, reader.offset - len - 1)?;
            TNetValue::List(items)
        }
        b'}' => {
            let pairs = parse_nested_dict(&data, reader.offset - len - 1)?;
            TNetValue::Dict(pairs)
        }
        _ => {
            return Err(reader.make_error(format!("unknown type tag: {:?}", tag as char)));
        }
    };

    Ok(Some(value))
}

fn parse_nested_list(data: &[u8], base_offset: usize) -> Result<Vec<TNetValue>, Error> {
    let mut items = Vec::new();
    let mut cursor = std::io::Cursor::new(data);
    let mut reader = TrackingReader {
        inner: &mut cursor,
        offset: base_offset,
    };
    while let Some(val) = parse_value(&mut reader)? {
        items.push(val);
    }
    Ok(items)
}

fn parse_nested_dict(
    data: &[u8],
    base_offset: usize,
) -> Result<Vec<(TNetValue, TNetValue)>, Error> {
    let mut pairs = Vec::new();
    let mut cursor = std::io::Cursor::new(data);
    let mut reader = TrackingReader {
        inner: &mut cursor,
        offset: base_offset,
    };
    while let Some(key) = parse_value(&mut reader)? {
        let value = parse_value(&mut reader)?
            .ok_or_else(|| reader.make_error("unexpected EOF: dict key without value".into()))?;
        pairs.push((key, value));
    }
    Ok(pairs)
}

/// Parse a single TNetValue from a reader.
///
/// Returns `Ok(None)` at clean EOF, `Ok(Some(value))` on success,
/// `Err` on parse error.
pub fn parse_one(reader: &mut impl Read) -> Result<Option<TNetValue>, Error> {
    let mut tracking = TrackingReader::new(reader);
    parse_value(&mut tracking)
}

/// Parse all TNetValues from a reader (for multi-flow files).
/// Fails on first error.
pub fn parse_all(reader: &mut impl Read) -> Result<Vec<TNetValue>, Error> {
    let mut tracking = TrackingReader::new(reader);
    let mut values = Vec::new();
    while let Some(val) = parse_value(&mut tracking)? {
        values.push(val);
    }
    Ok(values)
}

/// Parse all TNetValues leniently — returns an iterator of Result<TNetValue>.
/// On error, stops iteration (remaining data after a corrupt entry is unrecoverable
/// in tnetstring format since we lose framing).
pub fn parse_all_lenient(reader: &mut impl Read) -> Vec<Result<TNetValue, Error>> {
    let mut tracking = TrackingReader::new(reader);
    let mut results = Vec::new();
    loop {
        match parse_value(&mut tracking) {
            Ok(Some(val)) => results.push(Ok(val)),
            Ok(None) => break,
            Err(e) => {
                results.push(Err(e));
                break;
            }
        }
    }
    results
}

/// Convenience: parse a single value from a byte slice.
pub fn parse(data: &[u8]) -> Result<TNetValue, Error> {
    let mut cursor = std::io::Cursor::new(data);
    parse_one(&mut cursor)?.ok_or(Error::TNetParse {
        offset: 0,
        message: "empty input".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Individual type tags ──────────────────────────────────────────

    #[test]
    fn parse_bytes() {
        let val = parse(b"5:hello,").unwrap();
        assert_eq!(val, TNetValue::Bytes(b"hello".to_vec()));
        assert_eq!(val.as_bytes(), Some(b"hello".as_slice()));
    }

    #[test]
    fn parse_unicode_string() {
        let val = parse(b"5:hello;").unwrap();
        assert_eq!(val, TNetValue::String("hello".into()));
        assert_eq!(val.as_str(), Some("hello"));
    }

    #[test]
    fn parse_unicode_string_utf8() {
        // "héllo" in UTF-8 is 6 bytes
        let data = "héllo";
        let encoded = format!("{}:{};", data.len(), data);
        let val = parse(encoded.as_bytes()).unwrap();
        assert_eq!(val, TNetValue::String("héllo".into()));
    }

    #[test]
    fn parse_integer() {
        let val = parse(b"3:123#").unwrap();
        assert_eq!(val, TNetValue::Int(123));
        assert_eq!(val.as_int(), Some(123));
    }

    #[test]
    fn parse_negative_integer() {
        let val = parse(b"2:-5#").unwrap();
        assert_eq!(val, TNetValue::Int(-5));
    }

    #[test]
    fn parse_float() {
        let val = parse(b"4:3.14^").unwrap();
        assert_eq!(val, TNetValue::Float(3.14));
        assert_eq!(val.as_float(), Some(3.14));
    }

    #[test]
    fn parse_bool_true() {
        let val = parse(b"4:true!").unwrap();
        assert_eq!(val, TNetValue::Bool(true));
        assert_eq!(val.as_bool(), Some(true));
    }

    #[test]
    fn parse_bool_false() {
        let val = parse(b"5:false!").unwrap();
        assert_eq!(val, TNetValue::Bool(false));
        assert_eq!(val.as_bool(), Some(false));
    }

    #[test]
    fn parse_null() {
        let val = parse(b"0:~").unwrap();
        assert_eq!(val, TNetValue::Null);
        assert!(val.is_null());
    }

    #[test]
    fn parse_simple_list() {
        // [1, 2]
        let val = parse(b"8:1:1#1:2#]").unwrap();
        assert_eq!(
            val,
            TNetValue::List(vec![TNetValue::Int(1), TNetValue::Int(2)])
        );
        assert_eq!(val.as_list().unwrap().len(), 2);
    }

    #[test]
    fn parse_simple_dict() {
        // {"a": 1}
        let val = parse(b"8:1:a;1:1#}").unwrap();
        assert_eq!(
            val,
            TNetValue::Dict(vec![(TNetValue::String("a".into()), TNetValue::Int(1))])
        );
    }

    // ── Empty values ─────────────────────────────────────────────────

    #[test]
    fn parse_empty_bytes() {
        let val = parse(b"0:,").unwrap();
        assert_eq!(val, TNetValue::Bytes(vec![]));
    }

    #[test]
    fn parse_empty_string() {
        let val = parse(b"0:;").unwrap();
        assert_eq!(val, TNetValue::String(String::new()));
    }

    #[test]
    fn parse_empty_list() {
        let val = parse(b"0:]").unwrap();
        assert_eq!(val, TNetValue::List(vec![]));
    }

    #[test]
    fn parse_empty_dict() {
        let val = parse(b"0:}").unwrap();
        assert_eq!(val, TNetValue::Dict(vec![]));
    }

    #[test]
    fn parse_empty_null() {
        let val = parse(b"0:~").unwrap();
        assert_eq!(val, TNetValue::Null);
    }

    // ── Nested structures ────────────────────────────────────────────

    #[test]
    fn parse_dict_with_list_value() {
        // {"items": [1, 2]}
        // inner list: "1:1#1:2#" = 8 bytes  → "8:1:1#1:2#]" = 11 bytes
        // key: "5:items;" = 8 bytes
        // total inner: 8 + 11 = 19
        let val = parse(b"19:5:items;8:1:1#1:2#]}").unwrap();
        let list = val.get("items").unwrap().as_list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].as_int(), Some(1));
        assert_eq!(list[1].as_int(), Some(2));
    }

    #[test]
    fn parse_list_of_dicts() {
        // [{"a": 1}, {"b": 2}]
        // dict1: "1:a;1:1#" = 8 bytes → "8:1:a;1:1#}" = 11 bytes
        // dict2: "1:b;1:2#" = 8 bytes → "8:1:b;1:2#}" = 11 bytes
        // total inner: 22 bytes
        let val = parse(b"22:8:1:a;1:1#}8:1:b;1:2#}]").unwrap();
        let list = val.as_list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].get("a").unwrap().as_int(), Some(1));
        assert_eq!(list[1].get("b").unwrap().as_int(), Some(2));
    }

    #[test]
    fn parse_deeply_nested() {
        // [[1]]
        // innermost: "1:1#" = 4 bytes → "4:1:1#]" = 7 bytes
        // outer: "7:4:1:1#]]" → 10 bytes
        let val = parse(b"7:4:1:1#]]").unwrap();
        let outer = val.as_list().unwrap();
        assert_eq!(outer.len(), 1);
        let inner = outer[0].as_list().unwrap();
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].as_int(), Some(1));
    }

    // ── Dict key types ───────────────────────────────────────────────

    #[test]
    fn parse_dict_with_bytes_key() {
        // {b"key": 1}
        let val = parse(b"10:3:key,1:1#}").unwrap();
        // get() should match bytes keys too
        assert_eq!(val.get("key").unwrap().as_int(), Some(1));
    }

    // ── Error tests ──────────────────────────────────────────────────

    #[test]
    fn error_truncated_input() {
        let result = parse(b"10:hel");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unexpected end of input"), "got: {err}");
    }

    #[test]
    fn error_invalid_tag() {
        let result = parse(b"3:abcX");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown type tag"), "got: {err}");
    }

    #[test]
    fn error_invalid_length_non_digit() {
        let result = parse(b"abc:xyz,");
        assert!(result.is_err());
    }

    #[test]
    fn error_length_overflow_too_many_digits() {
        // 13 digits — exceeds MAX_LENGTH_DIGITS
        let result = parse(b"1234567890123:x,");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exceeds 12 digits"), "got: {err}");
    }

    #[test]
    fn error_invalid_boolean() {
        let result = parse(b"3:yes!");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid boolean content"), "got: {err}");
    }

    #[test]
    fn error_invalid_utf8_in_unicode() {
        let bad_utf8 = b"2:\xff\xfe;";
        let result = parse(bad_utf8);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid UTF-8"), "got: {err}");
    }

    #[test]
    fn error_null_with_nonzero_length() {
        let result = parse(b"3:abc~");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("null value must have length 0"), "got: {err}");
    }

    #[test]
    fn error_empty_input() {
        let result = parse(b"");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty input"), "got: {err}");
    }

    #[test]
    fn error_dict_key_without_value() {
        // Dict with a key but no value
        let result = parse(b"4:1:a;}");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("dict key without value"), "got: {err}");
    }

    // ── Streaming / multi-value ──────────────────────────────────────

    #[test]
    fn parse_multiple_values_from_reader() {
        let data = b"1:1#1:2#1:3#";
        let mut cursor = std::io::Cursor::new(data.as_slice());
        let values = parse_all(&mut cursor).unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].as_int(), Some(1));
        assert_eq!(values[1].as_int(), Some(2));
        assert_eq!(values[2].as_int(), Some(3));
    }

    #[test]
    fn parse_one_returns_none_at_eof() {
        let data = b"";
        let mut cursor = std::io::Cursor::new(data.as_slice());
        let result = parse_one(&mut cursor).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_one_reads_single_value() {
        let data = b"1:1#1:2#";
        let mut cursor = std::io::Cursor::new(data.as_slice());
        let v1 = parse_one(&mut cursor).unwrap().unwrap();
        assert_eq!(v1.as_int(), Some(1));
        let v2 = parse_one(&mut cursor).unwrap().unwrap();
        assert_eq!(v2.as_int(), Some(2));
        let v3 = parse_one(&mut cursor).unwrap();
        assert!(v3.is_none());
    }

    // ── Helper method tests ──────────────────────────────────────────

    #[test]
    fn helper_methods_return_none_on_type_mismatch() {
        let val = TNetValue::Int(42);
        assert!(val.as_dict().is_none());
        assert!(val.as_list().is_none());
        assert!(val.as_bytes().is_none());
        assert!(val.as_str().is_none());
        assert!(val.as_float().is_none());
        assert!(val.as_bool().is_none());
        assert!(!val.is_null());
    }

    #[test]
    fn get_on_non_dict_returns_none() {
        let val = TNetValue::Int(42);
        assert!(val.get("key").is_none());
    }

    #[test]
    fn get_missing_key_returns_none() {
        let val = TNetValue::Dict(vec![(TNetValue::String("a".into()), TNetValue::Int(1))]);
        assert!(val.get("missing").is_none());
    }

    // ── Fixture test ─────────────────────────────────────────────────

    #[test]
    fn parse_simple_get_flow() {
        let data = std::fs::read("testdata/flows/simple_get.flow").unwrap();
        let mut cursor = std::io::Cursor::new(data);
        let values = parse_all(&mut cursor).unwrap();
        assert!(
            !values.is_empty(),
            "flow file should contain at least one value"
        );

        // Each flow should be a dict
        let flow = &values[0];
        assert!(flow.as_dict().is_some(), "flow should be a dict");

        // mitmproxy flows have a "request" key
        let request = flow.get("request");
        assert!(
            request.is_some(),
            "flow dict should have a 'request' key, keys: {:?}",
            flow.as_dict()
                .unwrap()
                .iter()
                .map(|(k, _)| format!("{k:?}"))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn parse_multiple_flows_file() {
        let data = std::fs::read("testdata/flows/multiple.flow").unwrap();
        let mut cursor = std::io::Cursor::new(data);
        let values = parse_all(&mut cursor).unwrap();
        assert!(
            values.len() > 1,
            "multiple.flow should contain multiple flows, got {}",
            values.len()
        );
    }

    // ── Proptest ─────────────────────────────────────────────────────

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        fn arb_tnet_bytes() -> impl Strategy<Value = Vec<u8>> {
            prop::collection::vec(any::<u8>(), 0..256).prop_map(|data| {
                let mut encoded = format!("{}:", data.len()).into_bytes();
                encoded.extend_from_slice(&data);
                encoded.push(b',');
                encoded
            })
        }

        fn arb_tnet_string() -> impl Strategy<Value = Vec<u8>> {
            "[ -~]{0,100}".prop_map(|s| {
                let mut encoded = format!("{}:", s.len()).into_bytes();
                encoded.extend_from_slice(s.as_bytes());
                encoded.push(b';');
                encoded
            })
        }

        fn arb_tnet_int() -> impl Strategy<Value = Vec<u8>> {
            any::<i64>().prop_map(|n| {
                let s = n.to_string();
                format!("{}:{}#", s.len(), s).into_bytes()
            })
        }

        fn arb_tnet_bool() -> impl Strategy<Value = Vec<u8>> {
            any::<bool>().prop_map(|b| {
                let s = if b { "true" } else { "false" };
                format!("{}:{}!", s.len(), s).into_bytes()
            })
        }

        fn arb_tnet_simple() -> impl Strategy<Value = Vec<u8>> {
            prop_oneof![
                arb_tnet_bytes(),
                arb_tnet_string(),
                arb_tnet_int(),
                arb_tnet_bool(),
                Just(b"0:~".to_vec()),
            ]
        }

        proptest! {
            #[test]
            fn roundtrip_simple_values(encoded in arb_tnet_simple()) {
                let result = parse(&encoded);
                prop_assert!(result.is_ok(), "failed to parse: {:?}", result.err());
            }

            #[test]
            fn roundtrip_multiple_values(
                values in prop::collection::vec(arb_tnet_simple(), 1..10)
            ) {
                let mut combined = Vec::new();
                for v in &values {
                    combined.extend_from_slice(v);
                }
                let mut cursor = std::io::Cursor::new(combined);
                let parsed = parse_all(&mut cursor).unwrap();
                prop_assert_eq!(parsed.len(), values.len());
            }

            #[test]
            fn bytes_roundtrip_content(data in prop::collection::vec(any::<u8>(), 0..256)) {
                let mut encoded = format!("{}:", data.len()).into_bytes();
                encoded.extend_from_slice(&data);
                encoded.push(b',');
                let parsed = parse(&encoded).unwrap();
                prop_assert_eq!(parsed.as_bytes().unwrap(), data.as_slice());
            }

            #[test]
            fn int_roundtrip_content(n in any::<i64>()) {
                let s = n.to_string();
                let encoded = format!("{}:{}#", s.len(), s);
                let parsed = parse(encoded.as_bytes()).unwrap();
                prop_assert_eq!(parsed.as_int().unwrap(), n);
            }
        }
    }
}

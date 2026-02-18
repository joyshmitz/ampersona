use serde_json::Value;

/// Canonicalize a JSON value per JCS (RFC 8785).
///
/// Rules:
/// - Object keys sorted lexicographically
/// - No whitespace
/// - Numbers in shortest form
/// - Strings with minimal escaping
pub fn canonicalize(value: &Value) -> Vec<u8> {
    let canonical = canonicalize_value(value);
    canonical.into_bytes()
}

fn canonicalize_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Number(n) => {
            // JCS: shortest representation
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(f) = n.as_f64() {
                // Use JavaScript-style number serialization
                format_jcs_number(f)
            } else {
                n.to_string()
            }
        }
        Value::String(s) => format!("\"{}\"", escape_jcs_string(s)),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonicalize_value).collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(obj) => {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            let items: Vec<String> = keys
                .iter()
                .map(|k| {
                    format!(
                        "\"{}\":{}",
                        escape_jcs_string(k),
                        canonicalize_value(&obj[*k])
                    )
                })
                .collect();
            format!("{{{}}}", items.join(","))
        }
    }
}

fn escape_jcs_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\x08' => result.push_str("\\b"),
            '\x0C' => result.push_str("\\f"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c < '\x20' => result.push_str(&format!("\\u{:04x}", c as u32)),
            c => result.push(c),
        }
    }
    result
}

fn format_jcs_number(f: f64) -> String {
    if f == 0.0 {
        return "0".to_string();
    }
    // Try integer representation first
    if f.fract() == 0.0 && f.abs() < (1i64 << 53) as f64 {
        return (f as i64).to_string();
    }
    format!("{}", f)
}

/// Canonicalize only the specified signed_fields from a persona value.
pub fn canonicalize_fields(value: &Value, signed_fields: &[String]) -> Vec<u8> {
    let mut obj = serde_json::Map::new();
    if let Some(source) = value.as_object() {
        for field in signed_fields {
            if let Some(v) = source.get(field) {
                obj.insert(field.clone(), v.clone());
            }
        }
    }
    canonicalize(&Value::Object(obj))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_sorted() {
        let v = json!({"b": 1, "a": 2});
        let canonical = String::from_utf8(canonicalize(&v)).unwrap();
        assert_eq!(canonical, r#"{"a":2,"b":1}"#);
    }

    #[test]
    fn no_whitespace() {
        let v = json!({"key": [1, 2, 3]});
        let canonical = String::from_utf8(canonicalize(&v)).unwrap();
        assert_eq!(canonical, r#"{"key":[1,2,3]}"#);
    }

    #[test]
    fn string_escaping() {
        let v = json!({"text": "hello\nworld"});
        let canonical = String::from_utf8(canonicalize(&v)).unwrap();
        assert_eq!(canonical, r#"{"text":"hello\nworld"}"#);
    }
}

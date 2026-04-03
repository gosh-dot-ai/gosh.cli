// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use serde_json::Value;

/// Parse a `key=value` pair into `(key, JSON scalar)`.
///
/// RHS parsing rules (flat only):
///   - integers/floats -> `Value::Number`
///   - `true` / `false` -> `Value::Bool`
///   - `null` -> `Value::Null`
///   - arrays/objects -> kept as string (flat only)
///   - everything else -> `Value::String`
pub fn parse_meta_pair(pair: &str) -> anyhow::Result<(String, Value)> {
    let (key, raw) = pair
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("invalid --meta format, expected key=value: {pair}"))?;

    let key = key.trim().to_string();
    if key.is_empty() {
        anyhow::bail!("empty key in --meta: {pair}");
    }

    let val = parse_scalar(raw.trim());
    Ok((key, val))
}

/// Parse a single scalar value from a string.
/// Arrays and objects are NOT parsed — they stay as strings.
fn parse_scalar(s: &str) -> Value {
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => {
            // Try integer first, then float.
            if let Ok(n) = s.parse::<i64>() {
                return Value::Number(n.into());
            }
            if let Ok(f) = s.parse::<f64>() {
                if let Some(n) = serde_json::Number::from_f64(f) {
                    return Value::Number(n);
                }
            }
            // Reject arrays / objects — flat only.
            if (s.starts_with('[') && s.ends_with(']')) || (s.starts_with('{') && s.ends_with('}'))
            {
                return Value::String(s.to_string());
            }
            Value::String(s.to_string())
        }
    }
}

/// Build a flat `serde_json::Map` from parsed pairs.
pub fn pairs_to_map(pairs: &[(String, Value)]) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    for (k, v) in pairs {
        map.insert(k.clone(), v.clone());
    }
    map
}

/// Convenience: parse a `Vec<String>` of `key=value` into pairs.
pub fn parse_meta_vec(raw: &[String]) -> anyhow::Result<Vec<(String, Value)>> {
    raw.iter().map(|s| parse_meta_pair(s)).collect()
}

/// Build a `serde_json::Value::Object` from `--meta` flag values.
pub fn build_metadata(raw: &[String]) -> anyhow::Result<Value> {
    let pairs = parse_meta_vec(raw)?;
    Ok(Value::Object(pairs_to_map(&pairs)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_string() {
        let (k, v) = parse_meta_pair("name=hello").unwrap();
        assert_eq!(k, "name");
        assert_eq!(v, Value::String("hello".into()));
    }

    #[test]
    fn parse_integer() {
        let (_, v) = parse_meta_pair("count=42").unwrap();
        assert_eq!(v, Value::Number(42.into()));
    }

    #[test]
    fn parse_negative_integer() {
        let (_, v) = parse_meta_pair("offset=-5").unwrap();
        assert_eq!(v, Value::Number((-5).into()));
    }

    #[test]
    fn parse_float() {
        let (_, v) = parse_meta_pair("weight=3.5").unwrap();
        assert_eq!(v, Value::Number(serde_json::Number::from_f64(3.5).unwrap()));
    }

    #[test]
    fn parse_bool_true() {
        let (_, v) = parse_meta_pair("enabled=true").unwrap();
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn parse_bool_false() {
        let (_, v) = parse_meta_pair("enabled=false").unwrap();
        assert_eq!(v, Value::Bool(false));
    }

    #[test]
    fn parse_null() {
        let (_, v) = parse_meta_pair("gone=null").unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn array_stays_string() {
        let (_, v) = parse_meta_pair("items=[1,2,3]").unwrap();
        assert_eq!(v, Value::String("[1,2,3]".into()));
    }

    #[test]
    fn object_stays_string() {
        let (_, v) = parse_meta_pair(r#"obj={"a":1}"#).unwrap();
        assert_eq!(v, Value::String(r#"{"a":1}"#.into()));
    }

    #[test]
    fn missing_eq_is_error() {
        assert!(parse_meta_pair("noequals").is_err());
    }

    #[test]
    fn empty_key_is_error() {
        assert!(parse_meta_pair("=value").is_err());
    }

    #[test]
    fn value_with_equals_inside() {
        let (k, v) = parse_meta_pair("expr=a=b").unwrap();
        assert_eq!(k, "expr");
        assert_eq!(v, Value::String("a=b".into()));
    }

    #[test]
    fn build_metadata_works() {
        let raw =
            vec!["task_id=t-001".to_string(), "priority=1".to_string(), "active=true".to_string()];
        let meta = build_metadata(&raw).unwrap();
        let obj = meta.as_object().unwrap();
        assert_eq!(obj.get("task_id").unwrap(), &Value::String("t-001".into()));
        assert_eq!(obj.get("priority").unwrap(), &Value::Number(1.into()));
        assert_eq!(obj.get("active").unwrap(), &Value::Bool(true));
    }
}

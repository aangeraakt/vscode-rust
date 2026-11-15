//! Coercions that validate untyped setting values the way the option validators do.
//!
//! Ported from `src/vs/editor/common/config/editorOptions.ts`: `boolean`, `clampedInt`,
//! `EditorFloatOption.float`, `stringSet` and `EditorStringOption.string`. The JavaScript
//! coercions they rely on (`Boolean(x)`, `parseInt(s, 10)`, `parseFloat(s)`) are reproduced for
//! JSON values. A missing key is `None`, which is JavaScript's `undefined`.
//!
//! Tests follow the validator cases of `src/vs/editor/test/browser/config/editorConfiguration.test.ts`.

use serde_json::Value;

/// `boolean(value, defaultValue)`: a missing value gives the default, the string `"false"` gives
/// `false`, and anything else is coerced with JavaScript truthiness.
pub fn boolean(value: Option<&Value>, default: bool) -> bool {
    match value {
        None => default,
        Some(Value::String(s)) if s == "false" => false,
        Some(value) => js_truthy(value),
    }
}

fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `clampedInt(value, defaultValue, minimum, maximum)`. Strings are read with `parseInt(_, 10)`;
/// anything that is not a number gives `default`. Numbers are clamped to `minimum..=maximum`
/// and truncated. The default is returned as given, without clamping.
pub fn clamped_int(value: Option<&Value>, default: i64, minimum: i64, maximum: i64) -> i64 {
    let number = match value {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => parse_int(s),
        _ => None,
    };
    match number {
        Some(n) => n.max(minimum as f64).min(maximum as f64).trunc() as i64,
        None => default,
    }
}

/// `EditorFloatOption.float(value, defaultValue)`: strings are read with `parseFloat`, and
/// anything that is not a number gives `default`. The caller clamps the result.
pub fn float_or(value: Option<&Value>, default: f64) -> f64 {
    match value {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
        Some(Value::String(s)) => parse_float(s).unwrap_or(default),
        _ => default,
    }
}

/// `stringSet(value, defaultValue, allowedValues)`: the value is mapped through `allowed` when it
/// is one of its names; any other input gives `default`.
pub fn string_set<T: Copy>(value: Option<&str>, default: T, allowed: &[(&str, T)]) -> T {
    match value {
        Some(value) => allowed
            .iter()
            .find(|(name, _)| *name == value)
            .map_or(default, |(_, mapped)| *mapped),
        None => default,
    }
}

/// `EditorStringOption.string(value, defaultValue)`: a non-string gives `default`.
pub fn string_or(value: Option<&Value>, default: &str) -> String {
    match value.and_then(Value::as_str) {
        Some(value) => value.to_owned(),
        None => default.to_owned(),
    }
}

/// `parseInt(s, 10)`: leading whitespace, an optional sign, then decimal digits. `None` is `NaN`.
fn parse_int(s: &str) -> Option<f64> {
    let s = s.trim_start();
    let (sign, digits) = match s.as_bytes().first() {
        Some(b'-') => (-1.0, &s[1..]),
        Some(b'+') => (1.0, &s[1..]),
        _ => (1.0, s),
    };
    let count = count_digits(digits.as_bytes(), 0);
    if count == 0 {
        return None;
    }
    digits[..count].parse::<f64>().ok().map(|n| sign * n)
}

/// `parseFloat(s)`: the longest prefix that is a decimal literal, or `Infinity`. `None` is `NaN`.
fn parse_float(s: &str) -> Option<f64> {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let negative = bytes.first() == Some(&b'-');
    let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    if s[end..].starts_with("Infinity") {
        return Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }

    let int_digits = count_digits(bytes, end);
    end += int_digits;
    let mut frac_digits = 0;
    if bytes.get(end) == Some(&b'.') {
        frac_digits = count_digits(bytes, end + 1);
        if int_digits + frac_digits > 0 {
            end += 1 + frac_digits;
        }
    }
    if int_digits + frac_digits == 0 {
        return None;
    }

    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut exp = end + 1;
        if matches!(bytes.get(exp), Some(b'+' | b'-')) {
            exp += 1;
        }
        let exp_digits = count_digits(bytes, exp);
        if exp_digits > 0 {
            end = exp + exp_digits;
        }
    }
    s[..end].parse::<f64>().ok()
}

/// The number of ASCII digits in `bytes` starting at `from`.
fn count_digits(bytes: &[u8], from: usize) -> usize {
    bytes.get(from..).map_or(0, |rest| {
        rest.iter().take_while(|b| b.is_ascii_digit()).count()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn boolean_coercions() {
        assert!(boolean(None, true));
        assert!(!boolean(None, false));
        assert!(!boolean(Some(&json!("false")), true));
        assert!(boolean(Some(&json!("yes")), false));
        assert!(!boolean(Some(&json!(0)), true));
        assert!(!boolean(Some(&Value::Null), true));
        assert!(boolean(Some(&json!({})), false));
    }

    #[test]
    fn clamped_int_reads_strings_and_clamps() {
        assert_eq!(clamped_int(Some(&json!("12px")), 80, 1, 100), 12);
        assert_eq!(clamped_int(Some(&json!("abc")), 80, 1, 100), 80);
        assert_eq!(clamped_int(Some(&json!(-1)), 80, 1, 100), 1);
        assert_eq!(clamped_int(Some(&json!(1000)), 80, 1, 100), 100);
        assert_eq!(clamped_int(Some(&json!(7.9)), 80, 1, 100), 7);
        assert_eq!(clamped_int(None, 80, 1, 100), 80);
        assert_eq!(clamped_int(Some(&json!(true)), 80, 1, 100), 80);
    }

    #[test]
    fn float_or_reads_prefix_of_strings() {
        assert_eq!(float_or(Some(&json!("1.5em")), 0.0), 1.5);
        assert_eq!(float_or(Some(&json!(".5")), 0.0), 0.5);
        assert_eq!(float_or(Some(&json!("1e2x")), 0.0), 100.0);
        assert_eq!(float_or(Some(&json!("1e")), 0.0), 1.0);
        assert_eq!(float_or(Some(&json!("-Infinity")), 0.0), f64::NEG_INFINITY);
        assert_eq!(float_or(Some(&json!("x")), 3.0), 3.0);
        assert_eq!(float_or(Some(&json!("-")), 3.0), 3.0);
    }

    #[test]
    fn string_set_maps_allowed_names_only() {
        let allowed = [("on", 1), ("off", 0)];
        assert_eq!(string_set(Some("on"), 0, &allowed), 1);
        assert_eq!(string_set(Some("maybe"), 7, &allowed), 7);
        assert_eq!(string_set(None, 7, &allowed), 7);
    }

    #[test]
    fn string_or_rejects_non_strings() {
        assert_eq!(string_or(Some(&json!("a")), "d"), "a");
        assert_eq!(string_or(Some(&json!(1)), "d"), "d");
    }
}

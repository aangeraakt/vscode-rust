//! String comparison and character-code helpers.
//!
//! Ported from `src/vs/base/common/strings.ts`: only the parts that the comparers, filters,
//! fuzzy scorer and glob modules need, not the whole file.
//!
//! JavaScript strings are sequences of UTF-16 code units, and upstream compares and indexes them
//! unit by unit. These helpers keep that: [`compare`] orders by UTF-16 code units (not by Unicode
//! scalar value, which differs for characters above U+FFFF against U+E000..U+FFFF), and lengths
//! and offsets are UTF-16 code units (ADR 0003). The `*_substring*` variants take `&[u16]` slices
//! in place of upstream's `(start, end)` index pairs.
//!
//! Lower-casing uses Rust's full Unicode `to_lowercase`, which matches `String.prototype
//! .toLowerCase` for the cases the upstream tests cover (including `İ` becoming `i̇`). Lone
//! surrogates are replaced by U+FFFD before lower-casing; upstream keeps them.

use std::cmp::Ordering;

/// Encodes `s` as UTF-16 code units, the unit upstream offsets count in.
pub fn to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// The length of `s` in UTF-16 code units (JavaScript's `length`).
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// Lower-cases UTF-16 text as `String.prototype.toLowerCase` does. The result may be longer than
/// the input (`İ` lower-cases to two code units).
pub fn to_lower_utf16(s: &[u16]) -> Vec<u16> {
    if s.iter().all(|&c| c < 128) {
        return s
            .iter()
            .map(|&c| (c as u8).to_ascii_lowercase() as u16)
            .collect();
    }
    String::from_utf16_lossy(s)
        .to_lowercase()
        .encode_utf16()
        .collect()
}

/// `compare`: orders two strings by UTF-16 code units, as JavaScript's `<` does.
pub fn compare(a: &str, b: &str) -> Ordering {
    if a.is_ascii() && b.is_ascii() {
        return a.cmp(b);
    }
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `compareSubstring`: orders two UTF-16 slices by code unit, shorter first on a common prefix.
pub fn compare_substring(a: &[u16], b: &[u16]) -> Ordering {
    a.cmp(b)
}

/// `compareIgnoreCase`.
pub fn compare_ignore_case(a: &str, b: &str) -> Ordering {
    compare_substring_ignore_case(&to_utf16(a), &to_utf16(b))
}

/// `compareSubstringIgnoreCase`: ASCII letters compare case-insensitively code unit by code unit;
/// at the first differing pair where either unit is not ASCII, the rest of both slices is
/// lower-cased and compared.
pub fn compare_substring_ignore_case(a: &[u16], b: &[u16]) -> Ordering {
    for (i, (&code_a, &code_b)) in a.iter().zip(b).enumerate() {
        if code_a == code_b {
            continue;
        }
        if code_a >= 128 || code_b >= 128 {
            return to_lower_utf16(&a[i..]).cmp(&to_lower_utf16(&b[i..]));
        }
        // Map lower-case ASCII letters onto their upper-case variants.
        let fold = |c: u16| if is_lower_ascii_letter(c) { c - 32 } else { c };
        match fold(code_a).cmp(&fold(code_b)) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    a.len().cmp(&b.len())
}

/// `isAsciiDigit`.
pub fn is_ascii_digit(code: u16) -> bool {
    (u16::from(b'0')..=u16::from(b'9')).contains(&code)
}

/// `isLowerAsciiLetter`.
pub fn is_lower_ascii_letter(code: u16) -> bool {
    (u16::from(b'a')..=u16::from(b'z')).contains(&code)
}

/// `isUpperAsciiLetter`.
pub fn is_upper_ascii_letter(code: u16) -> bool {
    (u16::from(b'A')..=u16::from(b'Z')).contains(&code)
}

/// `equalsIgnoreCase`.
pub fn equals_ignore_case(a: &str, b: &str) -> bool {
    let (a, b) = (to_utf16(a), to_utf16(b));
    a.len() == b.len() && compare_substring_ignore_case(&a, &b) == Ordering::Equal
}

/// `equals`: `None` stands for `undefined`.
pub fn equals(a: Option<&str>, b: Option<&str>, ignore_case: bool) -> bool {
    a == b
        || match (a, b) {
            (Some(a), Some(b)) => ignore_case && equals_ignore_case(a, b),
            _ => false,
        }
}

/// `startsWithIgnoreCase`.
pub fn starts_with_ignore_case(s: &str, candidate: &str) -> bool {
    let (s, candidate) = (to_utf16(s), to_utf16(candidate));
    let len = candidate.len();
    len <= s.len() && compare_substring_ignore_case(&s[..len], &candidate) == Ordering::Equal
}

/// `endsWithIgnoreCase`.
pub fn ends_with_ignore_case(s: &str, candidate: &str) -> bool {
    let (s, candidate) = (to_utf16(s), to_utf16(candidate));
    s.len() >= candidate.len()
        && compare_substring_ignore_case(&s[s.len() - candidate.len()..], &candidate)
            == Ordering::Equal
}

/// `isHighSurrogate`.
pub fn is_high_surrogate(code: u16) -> bool {
    (0xD800..=0xDBFF).contains(&code)
}

/// `isLowSurrogate`.
pub fn is_low_surrogate(code: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&code)
}

/// `computeCodePoint`.
pub fn compute_code_point(high: u16, low: u16) -> u32 {
    ((u32::from(high) - 0xD800) << 10) + (u32::from(low) - 0xDC00) + 0x10000
}

/// `String.prototype.codePointAt`: the code point starting at UTF-16 offset `index`, combining
/// a surrogate pair; `None` past the end.
pub fn code_point_at(s: &[u16], index: usize) -> Option<u32> {
    let code = *s.get(index)?;
    if is_high_surrogate(code) {
        if let Some(&low) = s.get(index + 1) {
            if is_low_surrogate(low) {
                return Some(compute_code_point(code, low));
            }
        }
    }
    Some(u32::from(code))
}

/// `isEmojiImprecise`: a fast, imprecise emoji check on a code point.
pub fn is_emoji_imprecise(x: u32) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&x)
        || x == 8986
        || x == 8987
        || x == 9200
        || x == 9203
        || (9728..=10175).contains(&x)
        || x == 11088
        || x == 11093
        || (127744..=128591).contains(&x)
        || (128640..=128764).contains(&x)
        || (128992..=129008).contains(&x)
        || (129280..=129535).contains(&x)
        || (129648..=129782).contains(&x)
}

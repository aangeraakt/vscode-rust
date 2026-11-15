//! Port of the `strings.test.ts` cases for what `strings.rs` ports (`src/vs/base/test/common/`).

use std::cmp::Ordering;

use lathe_workspace::strings as s;

#[test]
fn equals_ignore_case() {
    assert!(s::equals_ignore_case("", ""));
    assert!(!s::equals_ignore_case("", "1"));
    assert!(!s::equals_ignore_case("1", ""));
    assert!(s::equals_ignore_case("a", "a"));
    assert!(s::equals_ignore_case("abc", "Abc"));
    assert!(s::equals_ignore_case("abc", "ABC"));
    assert!(s::equals_ignore_case("Höhenmeter", "HÖhenmeter"));
    assert!(s::equals_ignore_case("ÖL", "Öl"));
}

#[test]
fn equals() {
    assert!(!s::equals(None, Some("abc"), false));
    assert!(!s::equals(Some("abc"), None, false));
    assert!(s::equals(None, None, false));
    assert!(s::equals(Some(""), Some(""), false));
    assert!(s::equals(Some("a"), Some("a"), false));
    assert!(!s::equals(Some("abc"), Some("Abc"), false));
    assert!(s::equals(Some("abc"), Some("ABC"), true));
    assert!(!s::equals(Some("Höhenmeter"), Some("HÖhenmeter"), false));
    assert!(!s::equals(Some("ÖL"), Some("Öl"), false));
    assert!(s::equals(Some("ÖL"), Some("Öl"), true));
}

#[test]
fn starts_with_ignore_case() {
    assert!(s::starts_with_ignore_case("", ""));
    assert!(!s::starts_with_ignore_case("", "1"));
    assert!(s::starts_with_ignore_case("1", ""));
    assert!(s::starts_with_ignore_case("a", "a"));
    assert!(s::starts_with_ignore_case("abc", "Abc"));
    assert!(s::starts_with_ignore_case("abc", "ABC"));
    assert!(s::starts_with_ignore_case("Höhenmeter", "HÖhenmeter"));
    assert!(s::starts_with_ignore_case("ÖL", "Öl"));
    assert!(s::starts_with_ignore_case("alles klar", "a"));
    assert!(s::starts_with_ignore_case("alles klar", "A"));
    assert!(s::starts_with_ignore_case("alles klar", "alles k"));
    assert!(s::starts_with_ignore_case("alles klar", "alles K"));
    assert!(s::starts_with_ignore_case("alles klar", "ALLES K"));
    assert!(s::starts_with_ignore_case("alles klar", "alles klar"));
    assert!(s::starts_with_ignore_case("alles klar", "ALLES KLAR"));
    assert!(!s::starts_with_ignore_case("alles klar", " ALLES K"));
    assert!(!s::starts_with_ignore_case("alles klar", "ALLES K "));
    assert!(!s::starts_with_ignore_case("alles klar", "öALLES K "));
    assert!(!s::starts_with_ignore_case("alles klar", " "));
    assert!(!s::starts_with_ignore_case("alles klar", "ö"));
}

#[test]
fn ends_with_ignore_case() {
    assert!(s::ends_with_ignore_case("", ""));
    assert!(!s::ends_with_ignore_case("", "1"));
    assert!(s::ends_with_ignore_case("1", ""));
    assert!(!s::ends_with_ignore_case("abcd", "abcde"));
    assert!(s::ends_with_ignore_case("a", "a"));
    assert!(s::ends_with_ignore_case("abc", "Abc"));
    assert!(s::ends_with_ignore_case("abc", "ABC"));
    assert!(s::ends_with_ignore_case("Höhenmeter", "HÖhenmeter"));
    assert!(s::ends_with_ignore_case("ÖL", "Öl"));
    assert!(s::ends_with_ignore_case("alles klar", "r"));
    assert!(s::ends_with_ignore_case("alles klar", "R"));
    assert!(s::ends_with_ignore_case("alles klar", "s klar"));
    assert!(s::ends_with_ignore_case("alles klar", "S klar"));
    assert!(s::ends_with_ignore_case("alles klar", "S KLAR"));
    assert!(s::ends_with_ignore_case("alles klar", "alles klar"));
    assert!(s::ends_with_ignore_case("alles klar", "ALLES KLAR"));
    assert!(!s::ends_with_ignore_case("alles klar", "S KLAR "));
    assert!(!s::ends_with_ignore_case("alles klar", " S KLAR"));
    assert!(!s::ends_with_ignore_case("alles klar", "S KLARö"));
    assert!(!s::ends_with_ignore_case("alles klar", " "));
    assert!(!s::ends_with_ignore_case("alles klar", "ö"));
}

fn sign(o: Ordering) -> i32 {
    o as i32
}

/// `strings.compare(a.toLowerCase(), b.toLowerCase())`, the reference upstream checks against.
fn lower_compare(a: &[u16], b: &[u16]) -> i32 {
    sign(s::to_lower_utf16(a).cmp(&s::to_lower_utf16(b)))
}

#[test]
fn compare_ignore_case() {
    fn assert_compare_ignore_case(a: &str, b: &str, recurse: bool) {
        let actual = sign(s::compare_ignore_case(a, b));
        let expected = lower_compare(&s::to_utf16(a), &s::to_utf16(b));
        assert_eq!(actual, expected, "{a} <> {b}");
        if recurse {
            assert_compare_ignore_case(b, a, false);
        }
    }

    for (a, b) in [
        ("", ""),
        ("abc", "ABC"),
        ("abc", "ABc"),
        ("abc", "ABcd"),
        ("abc", "abcd"),
        ("foo", "föo"),
        ("Code", "code"),
        ("Code", "cöde"),
        ("B", "a"),
        ("a", "B"),
        ("b", "a"),
        ("a", "b"),
        ("aa", "ab"),
        ("aa", "aB"),
        ("aa", "aA"),
        ("a", "aa"),
        ("ab", "aA"),
        ("O", "/"),
    ] {
        assert_compare_ignore_case(a, b, true);
    }
}

#[test]
fn compare_ignore_case_substring() {
    fn assert_compare_ignore_case(a: &str, b: &str, ar: (usize, usize), br: (usize, usize)) {
        let (a, b) = (s::to_utf16(a), s::to_utf16(b));
        let (a, b) = (&a[ar.0..ar.1], &b[br.0..br.1]);
        assert_eq!(
            sign(s::compare_substring_ignore_case(a, b)),
            lower_compare(a, b)
        );
        assert_eq!(
            sign(s::compare_substring_ignore_case(b, a)),
            lower_compare(b, a)
        );
    }

    assert_compare_ignore_case("", "", (0, 0), (0, 0));
    assert_compare_ignore_case("abc", "ABC", (0, 1), (0, 1));
    assert_compare_ignore_case("abc", "Aabc", (0, 3), (1, 4));
    assert_compare_ignore_case("abcABc", "ABcd", (3, 6), (0, 4));
    assert_compare_ignore_case("fluentui", "fluentu\u{130}", (0, 8), (0, 8));
}

#[test]
fn compare_orders_by_utf16_code_units() {
    // Not an upstream test: JavaScript's `<` puts U+1F600 (surrogates D83D DE00) before U+FFFD.
    assert!(s::compare("\u{1F600}", "\u{FFFD}").is_lt());
    assert!("\u{1F600}" > "\u{FFFD}");
    assert_eq!(s::utf16_len("a\u{1F600}"), 3);
    assert_eq!(
        s::code_point_at(&s::to_utf16("a\u{1F600}"), 1),
        Some(0x1F600)
    );
}

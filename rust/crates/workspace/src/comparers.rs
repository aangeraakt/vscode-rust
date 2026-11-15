//! File name comparers for the Explorer sort orders and quick open tie-breaking.
//!
//! Ported from `src/vs/base/common/comparers.ts`. Upstream's `null` arguments are the empty string
//! here (upstream treats them the same), and every comparer returns an [`Ordering`] so it can be
//! passed to `sort_by` directly. Upstream's unused `caseSensitive` parameter of
//! `compareFileNames` is dropped; `comparePaths` is not ported yet.
//!
//! # Collation
//!
//! Upstream sorts with `Intl.Collator(undefined, { numeric: true, ... })`, which is ICU's root
//! collation in the user's locale. This module carries a small collator that reproduces the parts
//! of it the upstream tests and the Explorer rely on:
//!
//! - three levels, compared in order over the whole string: primary (base letters), secondary
//!   (accents), tertiary (case, lower before upper); `sensitivity: "base"` stops after primary,
//!   `"accent"` after secondary, the default compares all three;
//! - primary groups in ICU root order: whitespace, then punctuation and symbols in the order
//!   `_-,;:!?.'"()[]{}@*/\&#%`^+<=>|~$`, then other symbols by code point, then digits, then
//!   letters (by lower-cased code point, so `a < B < c`);
//! - numeric runs of ASCII digits compare by value, ignoring leading zeros, so `foo1` and `foo01`
//!   are equal, as in ICU;
//! - text is NFD-decomposed, so a combining mark is a secondary difference on its base letter;
//!   `æ`, `œ`, `ß`, `ø`, `đ` and `ł` expand as in ICU (`æ` sorts as `ae`), and control characters
//!   are ignorable.
//!
//! It differs from a full ICU collator in that letters of different scripts sort by code point,
//! locale tailorings (Swedish `ä` after `z`, Turkish dotless `ı`) are not applied, non-ASCII
//! digits are not numeric, and contractions and other expansions are not modelled.

use std::cmp::Ordering;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::strings::compare as compare_utf16;

/// How many collation levels a comparison looks at (`Intl.Collator`'s `sensitivity`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Sensitivity {
    /// `base`: letters only.
    Base,
    /// `accent`: letters and accents.
    Accent,
    /// The default (`variant`): letters, accents and case.
    Variant,
}

/// ICU root order of the ASCII punctuation and symbols.
const PUNCTUATION: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

const WHITESPACE_BASE: u32 = 0x0010_0000;
const PUNCTUATION_BASE: u32 = 0x0030_0000;
const SYMBOL_BASE: u32 = 0x0040_0000;
const NUMBER_LENGTH_BASE: u32 = 0x0060_0000;
const DIGIT_BASE: u32 = 0x0070_0000;
const LETTER_BASE: u32 = 0x0100_0000;

/// The secondary weight of a character without accents.
const COMMON_SECONDARY: u32 = 0;
/// The secondary weight of a letter that ICU sorts after its base letter (`ø` after `o`).
const VARIANT_SECONDARY: u32 = 0x10;

#[derive(Default)]
struct SortKey {
    primary: Vec<u32>,
    secondary: Vec<u32>,
    tertiary: Vec<u8>,
}

impl SortKey {
    fn push(&mut self, primary: u32, secondary: u32, upper: bool) {
        self.primary.push(primary);
        self.secondary.push(secondary);
        self.tertiary.push(u8::from(upper));
    }

    fn push_letter(&mut self, c: char, secondary: u32) {
        let lower = c.to_lowercase().next().unwrap_or(c);
        self.push(LETTER_BASE + lower as u32, secondary, lower != c);
    }
}

fn sort_key(s: &str, numeric: bool) -> SortKey {
    let mut key = SortKey::default();
    let mut chars = s.nfd().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut digits = vec![c as u32 - '0' as u32];
            while numeric && chars.peek().is_some_and(char::is_ascii_digit) {
                digits.push(chars.next().map_or(0, |d| d as u32 - '0' as u32));
            }
            let significant = digits
                .iter()
                .position(|&d| d != 0)
                .unwrap_or(digits.len() - 1);
            let digits = &digits[significant..];
            key.push(
                NUMBER_LENGTH_BASE + digits.len() as u32,
                COMMON_SECONDARY,
                false,
            );
            for &d in digits {
                key.push(DIGIT_BASE + d, COMMON_SECONDARY, false);
            }
        } else if is_combining_mark(c) {
            // A secondary weight on the previous element, after its own.
            key.secondary.push(1 + c as u32);
        } else if c.is_control() {
            // Ignorable.
        } else if c.is_whitespace() {
            key.push(WHITESPACE_BASE + c as u32, COMMON_SECONDARY, false);
        } else if let Some(index) = PUNCTUATION.find(c) {
            key.push(PUNCTUATION_BASE + index as u32, COMMON_SECONDARY, false);
        } else if let Some(expansion) = expansion(c) {
            for e in expansion.chars() {
                key.push_letter(e, COMMON_SECONDARY);
            }
            // Distinguish `æ` from `ae` after the accents of the expansion.
            key.secondary.push(VARIANT_SECONDARY);
        } else if let Some(base) = variant_of(c) {
            key.push_letter(base, VARIANT_SECONDARY);
            if c.is_uppercase() {
                if let Some(t) = key.tertiary.last_mut() {
                    *t = 1;
                }
            }
        } else if c.is_alphanumeric() {
            key.push_letter(c, COMMON_SECONDARY);
        } else {
            key.push(SYMBOL_BASE + c as u32, COMMON_SECONDARY, false);
        }
    }
    key
}

/// Letters that ICU expands to two base letters.
fn expansion(c: char) -> Option<&'static str> {
    Some(match c {
        'æ' => "ae",
        'Æ' => "AE",
        'œ' => "oe",
        'Œ' => "OE",
        'ß' => "ss",
        _ => return None,
    })
}

/// Letters without a decomposition that ICU sorts as a secondary variant of a base letter.
fn variant_of(c: char) -> Option<char> {
    Some(match c {
        'ø' | 'Ø' => 'o',
        'đ' | 'Đ' => 'd',
        'ł' | 'Ł' => 'l',
        _ => return None,
    })
}

/// The `collator.compare(one, other)` of an `Intl.Collator` with the given options.
fn collate(one: &str, other: &str, numeric: bool, sensitivity: Sensitivity) -> Ordering {
    if one == other {
        return Ordering::Equal;
    }
    let (a, b) = (sort_key(one, numeric), sort_key(other, numeric));
    let mut result = a.primary.cmp(&b.primary);
    if result.is_eq() && sensitivity >= Sensitivity::Accent {
        result = a.secondary.cmp(&b.secondary);
    }
    if result.is_eq() && sensitivity >= Sensitivity::Variant {
        result = a.tertiary.cmp(&b.tertiary);
    }
    result
}

/// `String.prototype.localeCompare(other)` without options: the default collator, not numeric.
/// Upstream tests use it as a reference; exposed for tests and callers that sort labels.
pub fn compare_locale(one: &str, other: &str) -> Ordering {
    collate(one, other, false, Sensitivity::Variant)
}

/// `localeCompare(other, undefined, { numeric: true })`.
pub fn compare_locale_numeric(one: &str, other: &str) -> Ordering {
    collate(one, other, true, Sensitivity::Variant)
}

fn collate_base_numeric(one: &str, other: &str) -> Ordering {
    collate(one, other, true, Sensitivity::Base)
}

fn collate_numeric_case_insensitive(one: &str, other: &str) -> Ordering {
    collate(one, other, true, Sensitivity::Accent)
}

/// Equal under the base collator but different strings: disambiguate by UTF-16 code units, as
/// upstream does for `compare("foo1", "foo01") === 0`.
fn base_numeric_then_unicode(one: &str, other: &str) -> Ordering {
    collate_base_numeric(one, other).then_with(|| compare_utf16(one, other))
}

/// `compareFileNames`: compares file names without distinguishing the name from the extension;
/// disambiguates by unicode comparison.
pub fn compare_file_names(one: &str, other: &str) -> Ordering {
    base_numeric_then_unicode(one, other)
}

/// `compareFileNamesDefault`: compares full file names without grouping by case.
pub fn compare_file_names_default(one: &str, other: &str) -> Ordering {
    compare_and_disambiguate_by_length(compare_locale_numeric, one, other)
}

/// `compareFileNamesUpper`: compares full file names, upper-case names first.
pub fn compare_file_names_upper(one: &str, other: &str) -> Ordering {
    compare_case_upper_first(one, other)
        .then_with(|| compare_and_disambiguate_by_length(compare_locale_numeric, one, other))
}

/// `compareFileNamesLower`: compares full file names, lower-case names first.
pub fn compare_file_names_lower(one: &str, other: &str) -> Ordering {
    compare_case_lower_first(one, other)
        .then_with(|| compare_and_disambiguate_by_length(compare_locale_numeric, one, other))
}

/// `compareFileNamesUnicode`: compares full file names by UTF-16 code units.
pub fn compare_file_names_unicode(one: &str, other: &str) -> Ordering {
    compare_utf16(one, other)
}

/// `compareFileExtensions`: compares by extension, then by name; disambiguates by unicode
/// comparison.
pub fn compare_file_extensions(one: &str, other: &str) -> Ordering {
    let (one_name, one_extension) = extract_name_and_extension(one);
    let (other_name, other_extension) = extract_name_and_extension(other);
    base_numeric_then_unicode(one_extension, other_extension)
        .then_with(|| base_numeric_then_unicode(one_name, other_name))
}

/// `compareFileExtensionsDefault`: compares by extension, then by full file name, mixing upper-
/// and lower-case names.
pub fn compare_file_extensions_default(one: &str, other: &str) -> Ordering {
    let (one_extension, other_extension) = (extract_extension(one), extract_extension(other));
    compare_and_disambiguate_by_length(
        collate_numeric_case_insensitive,
        one_extension,
        other_extension,
    )
    .then_with(|| compare_and_disambiguate_by_length(compare_locale_numeric, one, other))
}

/// `compareFileExtensionsUpper`: compares by extension, then case (upper first), then full name.
pub fn compare_file_extensions_upper(one: &str, other: &str) -> Ordering {
    let (one_extension, other_extension) = (extract_extension(one), extract_extension(other));
    compare_and_disambiguate_by_length(
        collate_numeric_case_insensitive,
        one_extension,
        other_extension,
    )
    .then_with(|| compare_case_upper_first(one, other))
    .then_with(|| compare_and_disambiguate_by_length(compare_locale_numeric, one, other))
}

/// `compareFileExtensionsLower`: compares by extension, then case (lower first), then full name.
pub fn compare_file_extensions_lower(one: &str, other: &str) -> Ordering {
    let (one_extension, other_extension) = (extract_extension(one), extract_extension(other));
    compare_and_disambiguate_by_length(
        collate_numeric_case_insensitive,
        one_extension,
        other_extension,
    )
    .then_with(|| compare_case_lower_first(one, other))
    .then_with(|| compare_and_disambiguate_by_length(compare_locale_numeric, one, other))
}

/// `compareFileExtensionsUnicode`: compares by lower-cased extension, then by full file name,
/// both by UTF-16 code units.
pub fn compare_file_extensions_unicode(one: &str, other: &str) -> Ordering {
    let one_extension = extract_extension(one).to_lowercase();
    let other_extension = extract_extension(other).to_lowercase();
    compare_utf16(&one_extension, &other_extension).then_with(|| compare_utf16(one, other))
}

/// `extractNameAndExtension` (without `dotfilesAsNames`, which no caller sets): splits at the
/// last dot, so `.env` is an empty name with extension `env`.
fn extract_name_and_extension(s: &str) -> (&str, &str) {
    match s.rfind('.') {
        Some(dot) => (&s[..dot], &s[dot + 1..]),
        None => (s, ""),
    }
}

/// `extractExtension`: the extension, treating dotfiles as names.
fn extract_extension(s: &str) -> &str {
    let (name, extension) = extract_name_and_extension(s);
    if name.is_empty() || name.starts_with('.') {
        ""
    } else {
        extension
    }
}

/// `compareAndDisambiguateByLength`: numerically equal strings (`foo1`, `foo01`) sort shorter
/// first.
fn compare_and_disambiguate_by_length(
    collator: fn(&str, &str) -> Ordering,
    one: &str,
    other: &str,
) -> Ordering {
    collator(one, other)
        .then_with(|| crate::strings::utf16_len(one).cmp(&crate::strings::utf16_len(other)))
}

/// The first UTF-16 code unit as a character; `None` for an empty string or when it is half of a
/// surrogate pair (upstream's `charAt(0)` is then a lone surrogate, which has no case).
fn first_bmp_char(s: &str) -> Option<char> {
    s.chars().next().filter(|c| c.len_utf16() == 1)
}

/// `startsWithLower`.
fn starts_with_lower(s: &str) -> bool {
    first_bmp_char(s).is_some_and(|c| c.to_uppercase().ne(std::iter::once(c)))
}

/// `startsWithUpper`.
fn starts_with_upper(s: &str) -> bool {
    first_bmp_char(s).is_some_and(|c| c.to_lowercase().ne(std::iter::once(c)))
}

/// `compareCaseLowerFirst`.
fn compare_case_lower_first(one: &str, other: &str) -> Ordering {
    if starts_with_lower(one) && starts_with_upper(other) {
        Ordering::Less
    } else if starts_with_upper(one) && starts_with_lower(other) {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// `compareCaseUpperFirst`.
fn compare_case_upper_first(one: &str, other: &str) -> Ordering {
    compare_case_lower_first(other, one)
}

/// `compareAnything`: prefix matches of `look_for` first, then suffix matches, then by file name.
pub fn compare_anything(one: &str, other: &str, look_for: &str) -> Ordering {
    let element_a_name = one.to_lowercase();
    let element_b_name = other.to_lowercase();

    // Sort prefix matches over non prefix matches.
    let prefix_compare = compare_by_prefix(one, other, look_for);
    if prefix_compare.is_ne() {
        return prefix_compare;
    }

    // Sort suffix matches over non suffix matches.
    let a_suffix = element_a_name.ends_with(look_for);
    let b_suffix = element_b_name.ends_with(look_for);
    if a_suffix != b_suffix {
        return if a_suffix {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }

    // Understand file names, then compare by name.
    compare_file_names(&element_a_name, &element_b_name)
        .then_with(|| compare_locale(&element_a_name, &element_b_name))
}

/// `compareByPrefix`: names starting with `look_for` (case-insensitively) first, shorter first.
pub fn compare_by_prefix(one: &str, other: &str, look_for: &str) -> Ordering {
    let element_a_name = one.to_lowercase();
    let element_b_name = other.to_lowercase();

    let a_prefix = element_a_name.starts_with(look_for);
    let b_prefix = element_b_name.starts_with(look_for);
    if a_prefix != b_prefix {
        return if a_prefix {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    if a_prefix {
        // Same prefix: shorter matches first, they match more precisely.
        return crate::strings::utf16_len(&element_a_name)
            .cmp(&crate::strings::utf16_len(&element_b_name));
    }
    Ordering::Equal
}

//! Filters that match a typed word against a label and return the highlight ranges.
//!
//! Ported from `src/vs/base/common/filters.ts`, with `tryNormalizeToBase` from
//! `src/vs/base/common/normalization.ts`. Every [`Match`] is a half-open range of **UTF-16 code
//! units** into the matched string, as upstream (ADR 0003), so highlight ranges can be handed to
//! the renderer unchanged. Filters take `&str`; the algorithms run on UTF-16 code units so that
//! indexing and lower-casing behave as JavaScript's.
//!
//! Differences from upstream:
//! - The Korean alternate-character table (`naturalLanguage/korean.ts`, used by `matchesWords`
//!   to match Hangul typed on a QWERTY layout) is not ported yet; Hangul only matches itself.
//! - `emptyMatches` is returned as a fresh `vec![Match { start: 0, end: 0 }]`; upstream shares one
//!   frozen array, which ownership makes unnecessary.
//! - `matchesFuzzy` matches the `*` wildcard pattern with a small matcher instead of building a
//!   `RegExp`, and has no regex cache.
//! - The LRU caches of `tryNormalizeToBase` and `getAlternateCodes` are not ported.

use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

use crate::strings::{self, to_lower_utf16, to_utf16};

/// `IMatch`: a highlighted range `[start, end)` in UTF-16 code units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Match {
    /// First highlighted UTF-16 code unit.
    pub start: usize,
    /// One past the last highlighted UTF-16 code unit.
    pub end: usize,
}

impl Match {
    /// A match of `[start, end)`.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// `IFilter`: `filter(word, word_to_match_against)` is `None` when the word does not match.
pub type Filter<'a> = dyn Fn(&str, &str) -> Option<Vec<Match>> + 'a;

/// `emptyMatches`: the matches of an empty query.
pub fn empty_matches() -> Vec<Match> {
    vec![Match::new(0, 0)]
}

/// `or`: a filter returning the result of the first of `filters` that matches.
pub fn or<'a>(filters: Vec<&'a Filter<'a>>) -> impl Fn(&str, &str) -> Option<Vec<Match>> + 'a {
    move |word, target| filters.iter().find_map(|filter| filter(word, target))
}

/// `matchesStrictPrefix`: `word` is a case-sensitive prefix of `target`.
pub fn matches_strict_prefix(word: &str, target: &str) -> Option<Vec<Match>> {
    matches_prefix_impl(false, word, target)
}

/// `matchesPrefix`: `word` is a case-insensitive prefix of `target`.
pub fn matches_prefix(word: &str, target: &str) -> Option<Vec<Match>> {
    matches_prefix_impl(true, word, target)
}

fn matches_prefix_impl(ignore_case: bool, word: &str, target: &str) -> Option<Vec<Match>> {
    let len = strings::utf16_len(word);
    if target.is_empty() || strings::utf16_len(target) < len {
        return None;
    }
    let matches = if ignore_case {
        strings::starts_with_ignore_case(target, word)
    } else {
        target.starts_with(word)
    };
    matches.then(|| {
        if len > 0 {
            vec![Match::new(0, len)]
        } else {
            Vec::new()
        }
    })
}

fn index_of(haystack: &[u16], needle: &[u16]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// `matchesContiguousSubString`: `word` occurs in `target`, ignoring case.
pub fn matches_contiguous_sub_string(word: &str, target: &str) -> Option<Vec<Match>> {
    let (word, target) = (to_utf16(word), to_utf16(target));
    if word.is_empty() {
        return Some(empty_matches());
    }
    if word.len() > target.len() {
        return None;
    }
    let index = index_of(&to_lower_utf16(&target), &to_lower_utf16(&word))?;
    Some(vec![Match::new(index, index + word.len())])
}

/// `matchesBaseContiguousSubString`: like [`matches_contiguous_sub_string`], also ignoring
/// accents (see [`try_normalize_to_base`]).
pub fn matches_base_contiguous_sub_string(word: &str, target: &str) -> Option<Vec<Match>> {
    let (word, target) = (to_utf16(word), to_utf16(target));
    if word.is_empty() {
        return Some(empty_matches());
    }
    if word.len() > target.len() {
        return None;
    }
    let (word, target) = (try_normalize_to_base(&word), try_normalize_to_base(&target));
    let index = index_of(&target, &word)?;
    Some(vec![Match::new(index, index + word.len())])
}

/// `tryNormalizeToBase`: NFD, remove the combining diacritics U+0300..U+036F, lower-case. When
/// removing accents changes the UTF-16 length (the input had decomposed accents, or NFD expanded
/// it), only lower-casing is applied, so that indices stay valid.
pub fn try_normalize_to_base(s: &[u16]) -> Vec<u16> {
    let text = String::from_utf16_lossy(s);
    let no_accents: Vec<u16> = if text.is_ascii() {
        s.to_vec()
    } else {
        let stripped: String = text
            .nfd()
            .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
            .collect();
        to_utf16(&stripped)
    };
    if no_accents.len() == s.len() {
        to_lower_utf16(&no_accents)
    } else {
        to_lower_utf16(s)
    }
}

/// `matchesSubString`: the characters of `word` occur in order in `target`, ignoring case.
pub fn matches_sub_string(word: &str, target: &str) -> Option<Vec<Match>> {
    let (word, target) = (to_utf16(word), to_utf16(target));
    if word.len() > target.len() {
        return None;
    }
    let (word, target) = (to_lower_utf16(&word), to_lower_utf16(&target));
    // Upstream recurses; a greedy scan gives the same result, as a failed match never backtracks.
    let mut result: Vec<Match> = Vec::new();
    let mut i = 0;
    for (j, &c) in target.iter().enumerate() {
        if i == word.len() {
            break;
        }
        if word[i] == c {
            i += 1;
            match result.last_mut() {
                Some(last) if last.end == j => last.end = j + 1,
                _ => result.push(Match::new(j, j + 1)),
            }
        }
    }
    (i == word.len()).then_some(result)
}

fn is_lower(code: u16) -> bool {
    strings::is_lower_ascii_letter(code)
}

/// `isUpper`: an upper-case ASCII letter.
pub fn is_upper(code: u16) -> bool {
    strings::is_upper_ascii_letter(code)
}

fn is_number(code: u16) -> bool {
    strings::is_ascii_digit(code)
}

fn is_whitespace(code: u16) -> bool {
    matches!(code, 0x20 | 0x09 | 0x0A | 0x0D)
}

fn is_word_separator(code: u16) -> bool {
    is_whitespace(code) || (code < 128 && "()[]{}<>`'\"-/;:,.?!".contains(code as u8 as char))
}

fn characters_match(a: u16, b: u16) -> bool {
    a == b || (is_word_separator(a) && is_word_separator(b))
}

/// `getAlternateCodes`: upstream returns the QWERTY keys of a Hangul character; not ported yet.
fn get_alternate_codes(_code: u16) -> Option<&'static [u16]> {
    None
}

fn is_alphanumeric(code: u16) -> bool {
    is_lower(code) || is_upper(code) || is_number(code)
}

/// `join`: prepends `head` to `tail`, merging adjacent ranges.
fn join(head: Match, mut tail: Vec<Match>) -> Vec<Match> {
    match tail.first_mut() {
        None => tail.push(head),
        Some(first) if first.start == head.end => first.start = head.start,
        Some(_) => tail.insert(0, head),
    }
    tail
}

fn next_anchor(camel_case_word: &[u16], start: usize) -> usize {
    (start..camel_case_word.len())
        .find(|&i| {
            let c = camel_case_word[i];
            is_upper(c) || is_number(c) || (i > 0 && !is_alphanumeric(camel_case_word[i - 1]))
        })
        .unwrap_or(camel_case_word.len())
}

/// `a === String.fromCharCode(b).toLowerCase()` for single code units.
fn equals_lowered(a: u16, b: u16) -> bool {
    let lowered = to_lower_utf16(&[b]);
    lowered.len() == 1 && lowered[0] == a
}

fn matches_camel_case_impl(word: &[u16], camel: &[u16], i: usize, j: usize) -> Option<Vec<Match>> {
    if i == word.len() {
        return Some(Vec::new());
    }
    if j == camel.len() || !equals_lowered(word[i], camel[j]) {
        return None;
    }
    let mut result = matches_camel_case_impl(word, camel, i + 1, j + 1);
    let mut next_upper_index = j + 1;
    while result.is_none() {
        next_upper_index = next_anchor(camel, next_upper_index);
        if next_upper_index >= camel.len() {
            break;
        }
        result = matches_camel_case_impl(word, camel, i + 1, next_upper_index);
        next_upper_index += 1;
    }
    result.map(|result| join(Match::new(j, j + 1), result))
}

struct CamelCaseAnalysis {
    upper_percent: f64,
    lower_percent: f64,
    alpha_percent: f64,
    numeric_percent: f64,
}

fn analyze_camel_case_word(word: &[u16]) -> CamelCaseAnalysis {
    let count = |f: fn(u16) -> bool| word.iter().filter(|&&c| f(c)).count() as f64;
    let len = word.len() as f64;
    CamelCaseAnalysis {
        upper_percent: count(is_upper) / len,
        lower_percent: count(is_lower) / len,
        alpha_percent: count(is_alphanumeric) / len,
        numeric_percent: count(is_number) / len,
    }
}

fn is_upper_case_word(a: &CamelCaseAnalysis) -> bool {
    a.lower_percent == 0.0 && a.upper_percent > 0.6
}

fn is_camel_case_word(a: &CamelCaseAnalysis) -> bool {
    a.lower_percent > 0.2
        && a.upper_percent < 0.8
        && a.alpha_percent > 0.6
        && a.numeric_percent < 0.2
}

fn is_camel_case_pattern(word: &[u16]) -> bool {
    let upper = word.iter().filter(|&&c| is_upper(c)).count();
    let lower = word.iter().filter(|&&c| is_lower(c)).count();
    let whitespace = word.iter().filter(|&&c| is_whitespace(c)).count();
    if (upper == 0 || lower == 0) && whitespace == 0 {
        word.len() <= 30
    } else {
        upper <= 5
    }
}

/// `matchesCamelCase`: `word` matches the starts of the camel-case humps of `camel_case_word`.
pub fn matches_camel_case(word: &str, camel_case_word: &str) -> Option<Vec<Match>> {
    let mut camel = to_utf16(camel_case_word.trim());
    if camel.is_empty() {
        return None;
    }
    let word = to_utf16(word);
    if !is_camel_case_pattern(&word) {
        return None;
    }
    // Upstream: "TODO: Consider removing this check".
    camel.truncate(60);

    let analysis = analyze_camel_case_word(&camel);
    if !is_camel_case_word(&analysis) {
        if !is_upper_case_word(&analysis) {
            return None;
        }
        camel = to_lower_utf16(&camel);
    }

    let word = to_lower_utf16(&word);
    let mut i = 0;
    while i < camel.len() {
        if let Some(result) = matches_camel_case_impl(&word, &camel, 0, i) {
            return Some(result);
        }
        i = next_anchor(&camel, i + 1);
    }
    None
}

/// `matchesWords`: matches the beginnings of the words in `target`, for non-ASCII languages too.
/// With `contiguous`, `pul` matches `Git: Pull`; without, `gp` and `g p` match it as well.
pub fn matches_words(word: &str, target: &str, contiguous: bool) -> Option<Vec<Match>> {
    if target.is_empty() {
        return None;
    }
    let word = try_normalize_to_base(&to_utf16(word));
    let target = try_normalize_to_base(&to_utf16(target));
    // Memoize within one call: separators match each other, so the recursion can otherwise
    // explode for inputs like `editor.action`.
    let mut words = WordsMatcher {
        word: &word,
        target: &target,
        contiguous,
        memo: HashMap::new(),
    };
    let mut target_index = 0;
    while target_index < target.len() {
        if let Some(result) = words.matches(0, target_index) {
            return Some(result);
        }
        target_index = next_word(&target, target_index + 1);
    }
    None
}

struct WordsMatcher<'a> {
    word: &'a [u16],
    target: &'a [u16],
    contiguous: bool,
    memo: HashMap<(usize, usize), Option<Vec<Match>>>,
}

impl WordsMatcher<'_> {
    fn matches(&mut self, word_index: usize, target_index: usize) -> Option<Vec<Match>> {
        if word_index == self.word.len() {
            return Some(Vec::new());
        }
        if target_index == self.target.len() {
            return None;
        }
        if let Some(cached) = self.memo.get(&(word_index, target_index)) {
            return cached.clone();
        }
        let computed = self.compute(word_index, target_index);
        self.memo
            .insert((word_index, target_index), computed.clone());
        computed
    }

    fn target_at(&self, i: usize) -> u16 {
        // Upstream's `charCodeAt` past the end is NaN, which matches nothing.
        self.target.get(i).copied().unwrap_or(u16::MAX)
    }

    fn compute(&mut self, word_index: usize, target_index: usize) -> Option<Vec<Match>> {
        let w = self.word[word_index];
        let mut target_index_offset = 0;
        if !characters_match(w, self.target[target_index]) {
            // Verify alternate characters before exiting.
            let alt = get_alternate_codes(w)?;
            for (k, &a) in alt.iter().enumerate() {
                if !characters_match(a, self.target_at(target_index + k)) {
                    return None;
                }
            }
            target_index_offset += alt.len() - 1;
        }

        let mut next_word_index = target_index + target_index_offset + 1;
        let mut result = self.matches(word_index + 1, next_word_index);
        if !self.contiguous {
            while result.is_none() {
                next_word_index = next_word(self.target, next_word_index);
                if next_word_index >= self.target.len() {
                    break;
                }
                result = self.matches(word_index + 1, next_word_index);
                next_word_index += 1;
            }
        }
        let result = result?;

        // Characters that differ are word separators: keep the match but do not highlight them.
        if w != self.target[target_index] {
            match get_alternate_codes(w) {
                None => return Some(result),
                Some(alt) => {
                    if alt
                        .iter()
                        .enumerate()
                        .any(|(k, &a)| a != self.target_at(target_index + k))
                    {
                        return Some(result);
                    }
                }
            }
        }
        Some(join(
            Match::new(target_index, target_index + target_index_offset + 1),
            result,
        ))
    }
}

fn next_word(word: &[u16], start: usize) -> usize {
    (start..word.len())
        .find(|&i| is_word_separator(word[i]) || (i > 0 && is_word_separator(word[i - 1])))
        .unwrap_or(word.len())
}

/// `matchesFuzzy`: a `*` wildcard match first, then prefix, camel case and contiguous substring
/// (or, with `enable_separate_substring_matching`, non-contiguous substring) matching.
pub fn matches_fuzzy(
    word: &str,
    target: &str,
    enable_separate_substring_matching: bool,
) -> Option<Vec<Match>> {
    if word.is_empty() {
        return Some(empty_matches());
    }
    if let Some(m) = match_wildcard(&to_utf16(word), &to_utf16(target)) {
        return Some(vec![m]);
    }
    if enable_separate_substring_matching {
        or(vec![
            &matches_prefix,
            &matches_camel_case,
            &matches_sub_string,
        ])(word, target)
    } else {
        or(vec![
            &matches_prefix,
            &matches_camel_case,
            &matches_contiguous_sub_string,
        ])(word, target)
    }
}

/// The JavaScript `RegExp` of `convertSimple2RegExpPattern(pattern)` with the `i` flag: the
/// pattern is literal except that `*` is a greedy `.*` (which does not cross line terminators).
/// Returns the leftmost match.
fn match_wildcard(pattern: &[u16], text: &[u16]) -> Option<Match> {
    let segments: Vec<&[u16]> = pattern.split(|&c| c == u16::from(b'*')).collect();
    (0..=text.len()).find_map(|start| {
        match_segments(&segments, text, start, true).map(|end| Match::new(start, end))
    })
}

fn match_segments(segments: &[&[u16]], text: &[u16], pos: usize, first: bool) -> Option<usize> {
    let Some((segment, rest)) = segments.split_first() else {
        return Some(pos);
    };
    if first {
        let end = match_literal(segment, text, pos)?;
        return match_segments(rest, text, end, false);
    }
    // A `.*` before `segment`: try the longest run first.
    let line_end = (pos..text.len())
        .find(|&i| matches!(text[i], 0x0A | 0x0D | 0x2028 | 0x2029))
        .unwrap_or(text.len());
    (pos..=line_end).rev().find_map(|at| {
        let end = match_literal(segment, text, at)?;
        match_segments(rest, text, end, false)
    })
}

fn match_literal(literal: &[u16], text: &[u16], pos: usize) -> Option<usize> {
    let end = pos + literal.len();
    let slice = text.get(pos..end)?;
    slice
        .iter()
        .zip(literal)
        .all(|(&a, &b)| canonicalize(a) == canonicalize(b))
        .then_some(end)
}

/// The case folding of a non-unicode case-insensitive `RegExp` (ECMAScript `Canonicalize`).
fn canonicalize(c: u16) -> u16 {
    let Some(ch) = char::from_u32(u32::from(c)) else {
        return c;
    };
    let mut upper = ch.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(u), None) if u.len_utf16() == 1 && !(c >= 128 && (u as u32) < 128) => u as u16,
        _ => c,
    }
}

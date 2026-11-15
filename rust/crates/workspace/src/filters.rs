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

/// `matchesFuzzy2`: [`fuzzy_score`] with a weak first match allowed, as highlight ranges.
pub fn matches_fuzzy2(pattern: &str, word: &str) -> Option<Vec<Match>> {
    let options = FuzzyScoreOptions {
        first_match_can_be_weak: true,
        boost_full_match: true,
    };
    let score = fuzzy_score_str(pattern, 0, word, 0, options)?;
    Some(create_matches(Some(&score)))
}

// --- fuzzyScore ---

/// `FuzzyScore`: the score of a fuzzy match and where it matched.
///
/// Upstream packs this into an array `[score, wordStart, ...matches]`; `matches` keeps upstream's
/// order (last matched position first), relative to `word_start`, in UTF-16 code units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzyScore {
    /// The score; higher is better.
    pub score: i32,
    /// The UTF-16 offset in the word at which matching started.
    pub word_start: usize,
    /// The matched positions relative to `word_start`, last first.
    pub matches: Vec<usize>,
}

impl FuzzyScore {
    /// `FuzzyScore.Default`: no matches and score -100.
    pub fn default_score() -> Self {
        Self {
            score: -100,
            word_start: 0,
            matches: Vec::new(),
        }
    }

    /// `FuzzyScore.isDefault`.
    pub fn is_default(score: Option<&FuzzyScore>) -> bool {
        score.is_none_or(|s| s.matches.is_empty() && s.score == -100 && s.word_start == 0)
    }
}

/// `FuzzyScoreOptions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FuzzyScoreOptions {
    /// Accept a match whose first character does not hit a word start or upper-case letter.
    pub first_match_can_be_weak: bool,
    /// Add 2 to the score when the pattern matches every character of the word.
    pub boost_full_match: bool,
}

impl Default for FuzzyScoreOptions {
    /// `FuzzyScoreOptions.default`.
    fn default() -> Self {
        Self {
            first_match_can_be_weak: false,
            boost_full_match: true,
        }
    }
}

/// `FuzzyScorer`: the signature shared by [`fuzzy_score`] and its graceful variants. Text is
/// UTF-16; `pattern_low` and `word_low` are the lower-cased pattern and word.
pub type FuzzyScorer = fn(
    pattern: &[u16],
    pattern_low: &[u16],
    pattern_pos: usize,
    word: &[u16],
    word_low: &[u16],
    word_pos: usize,
    options: FuzzyScoreOptions,
) -> Option<FuzzyScore>;

/// [`fuzzy_score`] on `&str`, lower-casing pattern and word; positions are UTF-16 offsets.
pub fn fuzzy_score_str(
    pattern: &str,
    pattern_pos: usize,
    word: &str,
    word_pos: usize,
    options: FuzzyScoreOptions,
) -> Option<FuzzyScore> {
    let (pattern, word) = (to_utf16(pattern), to_utf16(word));
    let (pattern_low, word_low) = (to_lower_utf16(&pattern), to_lower_utf16(&word));
    fuzzy_score(
        &pattern,
        &pattern_low,
        pattern_pos,
        &word,
        &word_low,
        word_pos,
        options,
    )
}

/// `anyScore`: the first [`fuzzy_score`] (weak first match allowed) from any of the first 13
/// pattern positions, else score 0 with no matches.
pub fn any_score(
    pattern: &[u16],
    pattern_low: &[u16],
    pattern_pos: usize,
    word: &[u16],
    word_low: &[u16],
    word_pos: usize,
    _options: FuzzyScoreOptions,
) -> Option<FuzzyScore> {
    let options = FuzzyScoreOptions {
        first_match_can_be_weak: true,
        boost_full_match: true,
    };
    let max = pattern.len().min(13);
    (pattern_pos..max)
        .find_map(|p| fuzzy_score(pattern, pattern_low, p, word, word_low, word_pos, options))
        .or(Some(FuzzyScore {
            score: 0,
            word_start: word_pos,
            matches: Vec::new(),
        }))
}

/// `createMatches`: the highlight ranges of a score.
pub fn create_matches(score: Option<&FuzzyScore>) -> Vec<Match> {
    let Some(score) = score else {
        return Vec::new();
    };
    let mut res: Vec<Match> = Vec::new();
    for &m in score.matches.iter().rev() {
        let pos = m + score.word_start;
        match res.last_mut() {
            Some(last) if last.end == pos => last.end = pos + 1,
            _ => res.push(Match::new(pos, pos + 1)),
        }
    }
    res
}

const MAX_LEN: usize = 128;
const MIN_SCORE: i32 = i32::MIN;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Arrow {
    Diag = 1,
    Left = 2,
    LeftLeft = 3,
}

/// Upstream's module-level tables. They are reused without clearing between calls, as upstream
/// does, so they live in a thread local.
struct Tables {
    min_word_match_pos: [usize; 2 * MAX_LEN + 1],
    max_word_match_pos: [usize; 2 * MAX_LEN + 1],
    diag: [[i32; MAX_LEN + 1]; MAX_LEN + 1],
    table: [[i32; MAX_LEN + 1]; MAX_LEN + 1],
    arrows: [[u8; MAX_LEN + 1]; MAX_LEN + 1],
}

thread_local! {
    static TABLES: std::cell::RefCell<Box<Tables>> = std::cell::RefCell::new(Box::new(Tables {
        min_word_match_pos: [0; 2 * MAX_LEN + 1],
        max_word_match_pos: [0; 2 * MAX_LEN + 1],
        diag: [[0; MAX_LEN + 1]; MAX_LEN + 1],
        table: [[0; MAX_LEN + 1]; MAX_LEN + 1],
        arrows: [[0; MAX_LEN + 1]; MAX_LEN + 1],
    }));
}

/// `s[i]` in JavaScript: `None` (undefined) when out of range, including negative.
fn at(s: &[u16], i: isize) -> Option<u16> {
    usize::try_from(i).ok().and_then(|i| s.get(i).copied())
}

fn is_separator_at_pos(value: &[u16], index: isize) -> bool {
    let Ok(index) = usize::try_from(index) else {
        return false;
    };
    let Some(code) = strings::code_point_at(value, index) else {
        return false;
    };
    matches!(
        char::from_u32(code),
        Some('_' | '-' | '.' | ' ' | '/' | '\\' | '\'' | '"' | ':' | '$' | '<' | '>')
            | Some('(' | ')' | '[' | ']' | '{' | '}')
    ) || strings::is_emoji_imprecise(code)
}

fn is_whitespace_at_pos(value: &[u16], index: isize) -> bool {
    matches!(at(value, index), Some(0x20 | 0x09))
}

fn is_upper_case_at_pos(pos: isize, word: &[u16], word_low: &[u16]) -> bool {
    at(word, pos) != at(word_low, pos)
}

/// `isPatternInWord`: the pattern's characters occur in order in the word. With `fill`, records
/// the minimum word position of each pattern position.
fn is_pattern_in_word(
    t: &mut Tables,
    pattern_low: &[u16],
    mut pattern_pos: usize,
    pattern_len: usize,
    word_low: &[u16],
    mut word_pos: usize,
    word_len: usize,
) -> bool {
    while pattern_pos < pattern_len && word_pos < word_len {
        if pattern_low.get(pattern_pos) == word_low.get(word_pos) {
            t.min_word_match_pos[pattern_pos] = word_pos;
            pattern_pos += 1;
        }
        word_pos += 1;
    }
    pattern_pos == pattern_len
}

/// `fuzzyScore`: matches a pattern against a word the way IntelliSense does, preferring word
/// starts, upper-case letters and contiguous runs. Text is UTF-16; at most 128 code units of
/// pattern and word are considered.
pub fn fuzzy_score(
    pattern: &[u16],
    pattern_low: &[u16],
    pattern_start: usize,
    word: &[u16],
    word_low: &[u16],
    word_start: usize,
    options: FuzzyScoreOptions,
) -> Option<FuzzyScore> {
    TABLES.with(|t| {
        let t = &mut **t.borrow_mut();
        fuzzy_score_impl(
            t,
            pattern,
            pattern_low,
            pattern_start,
            word,
            word_low,
            word_start,
            options,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn fuzzy_score_impl(
    t: &mut Tables,
    pattern: &[u16],
    pattern_low: &[u16],
    pattern_start: usize,
    word: &[u16],
    word_low: &[u16],
    word_start: usize,
    options: FuzzyScoreOptions,
) -> Option<FuzzyScore> {
    let pattern_len = pattern.len().min(MAX_LEN);
    let word_len = word.len().min(MAX_LEN);

    if pattern_start >= pattern_len
        || word_start >= word_len
        || (pattern_len - pattern_start) > (word_len - word_start)
    {
        return None;
    }

    // Stop early when the pattern's characters do not occur in order in the word.
    if !is_pattern_in_word(
        t,
        pattern_low,
        pattern_start,
        pattern_len,
        word_low,
        word_start,
        word_len,
    ) {
        return None;
    }

    // Find the max matching word position for each pattern position.
    fill_in_max_word_match_pos(
        t,
        pattern_len,
        word_len,
        pattern_start,
        word_start,
        pattern_low,
        word_low,
    );

    let mut has_strong_first_match = false;
    let mut row = 1;
    let mut column = 1;
    let mut pattern_pos = pattern_start;
    while pattern_pos < pattern_len {
        // Reduce the search space to possible matching word positions.
        let min_word_match_pos = t.min_word_match_pos[pattern_pos];
        let max_word_match_pos = t.max_word_match_pos[pattern_pos];
        let next_max_word_match_pos = if pattern_pos + 1 < pattern_len {
            t.max_word_match_pos[pattern_pos + 1]
        } else {
            word_len
        };

        column = min_word_match_pos - word_start + 1;
        let mut word_pos = min_word_match_pos;
        while word_pos < next_max_word_match_pos {
            let mut score = MIN_SCORE;
            let mut can_come_diag = false;

            if word_pos <= max_word_match_pos {
                score = do_score(
                    pattern,
                    pattern_low,
                    pattern_pos,
                    pattern_start,
                    word,
                    word_low,
                    word_pos,
                    word_len,
                    word_start,
                    t.diag[row - 1][column - 1] == 0,
                    &mut has_strong_first_match,
                );
            }

            let mut diag_score = 0;
            if score != MIN_SCORE {
                can_come_diag = true;
                diag_score = score + t.table[row - 1][column - 1];
            }

            let can_come_left = word_pos > min_word_match_pos;
            let left_score = if can_come_left {
                // Penalty for a gap start.
                t.table[row][column - 1] + if t.diag[row][column - 1] > 0 { -5 } else { 0 }
            } else {
                0
            };

            let can_come_left_left =
                word_pos > min_word_match_pos + 1 && t.diag[row][column - 1] > 0;
            let left_left_score = if can_come_left_left {
                t.table[row][column - 2] + if t.diag[row][column - 2] > 0 { -5 } else { 0 }
            } else {
                0
            };

            if can_come_left_left
                && (!can_come_left || left_left_score >= left_score)
                && (!can_come_diag || left_left_score >= diag_score)
            {
                // Prefer jumping over a diagonal: the match is earlier in the word.
                t.table[row][column] = left_left_score;
                t.arrows[row][column] = Arrow::LeftLeft as u8;
                t.diag[row][column] = 0;
            } else if can_come_left && (!can_come_diag || left_score >= diag_score) {
                // Prefer left: the match is earlier in the word.
                t.table[row][column] = left_score;
                t.arrows[row][column] = Arrow::Left as u8;
                t.diag[row][column] = 0;
            } else if can_come_diag {
                t.table[row][column] = diag_score;
                t.arrows[row][column] = Arrow::Diag as u8;
                t.diag[row][column] = t.diag[row - 1][column - 1] + 1;
            } else {
                unreachable!("not possible");
            }
            column += 1;
            word_pos += 1;
        }
        row += 1;
        pattern_pos += 1;
    }

    if !has_strong_first_match && !options.first_match_can_be_weak {
        return None;
    }

    row -= 1;
    column -= 1;

    let mut result = FuzzyScore {
        score: t.table[row][column],
        word_start,
        matches: Vec::new(),
    };
    let mut backwards_diag_length = 0;
    let mut max_match_column = 0;

    while row >= 1 {
        // Find the column where we go diagonally up.
        let mut diag_column = column as isize;
        loop {
            let arrow = t.arrows[row][diag_column as usize];
            if arrow == Arrow::LeftLeft as u8 {
                diag_column -= 2;
            } else if arrow == Arrow::Left as u8 {
                diag_column -= 1;
            } else {
                break;
            }
            if diag_column < 1 {
                break;
            }
        }

        // Overturn the forwards decision if keeping the backwards diagonal gives a better match.
        if backwards_diag_length > 1
            && pattern_low.get(pattern_start + row - 1) == word_low.get(word_start + column - 1)
            && !is_upper_case_at_pos(diag_column + word_start as isize - 1, word, word_low)
            && backwards_diag_length + 1 > t.diag[row][diag_column as usize]
        {
            diag_column = column as isize;
        }

        if diag_column == column as isize {
            backwards_diag_length += 1;
        } else {
            backwards_diag_length = 1;
        }

        if max_match_column == 0 {
            max_match_column = diag_column;
        }

        row -= 1;
        column = (diag_column - 1).max(0) as usize;
        result.matches.push(column);
    }

    if word_len - word_start == pattern_len && options.boost_full_match {
        // The word matches the pattern with all characters.
        result.score += 2;
    }

    // One penalty point per skipped character in the word.
    result.score -= (max_match_column - pattern_len as isize) as i32;
    Some(result)
}

fn fill_in_max_word_match_pos(
    t: &mut Tables,
    pattern_len: usize,
    word_len: usize,
    pattern_start: usize,
    word_start: usize,
    pattern_low: &[u16],
    word_low: &[u16],
) {
    let mut pattern_pos = pattern_len as isize - 1;
    let mut word_pos = word_len as isize - 1;
    while pattern_pos >= pattern_start as isize && word_pos >= word_start as isize {
        if at(pattern_low, pattern_pos) == at(word_low, word_pos) {
            t.max_word_match_pos[pattern_pos as usize] = word_pos as usize;
            pattern_pos -= 1;
        }
        word_pos -= 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn do_score(
    pattern: &[u16],
    pattern_low: &[u16],
    pattern_pos: usize,
    pattern_start: usize,
    word: &[u16],
    word_low: &[u16],
    word_pos: usize,
    word_len: usize,
    word_start: usize,
    new_match_start: bool,
    out_first_match_strong: &mut bool,
) -> i32 {
    if pattern_low.get(pattern_pos) != word_low.get(word_pos) {
        return MIN_SCORE;
    }
    let wp = word_pos as isize;
    let exact = || pattern.get(pattern_pos) == word.get(word_pos);

    let mut score = 1;
    let mut is_gap_location = false;
    if word_pos == pattern_pos - pattern_start {
        // Common prefix: `foobar <-> foobaz`.
        score = if exact() { 7 } else { 5 };
    } else if is_upper_case_at_pos(wp, word, word_low)
        && (word_pos == 0 || !is_upper_case_at_pos(wp - 1, word, word_low))
    {
        // Hitting upper-case: `foo <-> forOthers`.
        score = if exact() { 7 } else { 5 };
        is_gap_location = true;
    } else if is_separator_at_pos(word_low, wp)
        && (word_pos == 0 || !is_separator_at_pos(word_low, wp - 1))
    {
        // Hitting a separator: `. <-> foo.bar`.
        score = 5;
    } else if is_separator_at_pos(word_low, wp - 1) || is_whitespace_at_pos(word_low, wp - 1) {
        // Post separator: `foo <-> bar_foo`.
        score = 5;
        is_gap_location = true;
    }

    if score > 1 && pattern_pos == pattern_start {
        *out_first_match_strong = true;
    }

    if !is_gap_location {
        is_gap_location = is_upper_case_at_pos(wp, word, word_low)
            || is_separator_at_pos(word_low, wp - 1)
            || is_whitespace_at_pos(word_low, wp - 1);
    }

    if pattern_pos == pattern_start {
        // The first pattern character matching after the word start: penalize the gap.
        if word_pos > word_start {
            score -= if is_gap_location { 3 } else { 5 };
        }
    } else if new_match_start {
        // The beginning of a new match, after a gap.
        score += if is_gap_location { 2 } else { 0 };
    } else {
        // Part of a contiguous match: a small bonus unless it is a preferred gap location.
        score += if is_gap_location { 0 } else { 1 };
    }

    if word_pos + 1 == word_len {
        // Pretend there is a gap after the last character so that matching it is not favored.
        score -= if is_gap_location { 3 } else { 5 };
    }
    score
}

// --- graceful ---

/// `fuzzyScoreGracefulAggressive`: like [`fuzzy_score_graceful`], but always tries the typo
/// permutations to find a better alignment.
pub fn fuzzy_score_graceful_aggressive(
    pattern: &[u16],
    pattern_low: &[u16],
    pattern_pos: usize,
    word: &[u16],
    word_low: &[u16],
    word_pos: usize,
    options: FuzzyScoreOptions,
) -> Option<FuzzyScore> {
    fuzzy_score_with_permutations(
        pattern,
        pattern_low,
        pattern_pos,
        word,
        word_low,
        word_pos,
        true,
        options,
    )
}

/// `fuzzyScoreGraceful`: [`fuzzy_score`], falling back to patterns with two neighbouring
/// characters swapped (a 3 point penalty).
pub fn fuzzy_score_graceful(
    pattern: &[u16],
    pattern_low: &[u16],
    pattern_pos: usize,
    word: &[u16],
    word_low: &[u16],
    word_pos: usize,
    options: FuzzyScoreOptions,
) -> Option<FuzzyScore> {
    fuzzy_score_with_permutations(
        pattern,
        pattern_low,
        pattern_pos,
        word,
        word_low,
        word_pos,
        false,
        options,
    )
}

#[allow(clippy::too_many_arguments)]
fn fuzzy_score_with_permutations(
    pattern: &[u16],
    pattern_low: &[u16],
    pattern_pos: usize,
    word: &[u16],
    word_low: &[u16],
    word_pos: usize,
    aggressive: bool,
    options: FuzzyScoreOptions,
) -> Option<FuzzyScore> {
    let mut top = fuzzy_score(
        pattern,
        pattern_low,
        pattern_pos,
        word,
        word_low,
        word_pos,
        options,
    );
    if top.is_some() && !aggressive {
        return top;
    }
    if pattern.len() >= 3 {
        // Try a few (max 7) permutations that swap neighbouring characters.
        let tries = (pattern.len() - 1).min(7);
        for moving_pattern_pos in pattern_pos + 1..tries {
            if let Some(new_pattern) = next_typo_permutation(pattern, moving_pattern_pos) {
                let low = to_lower_utf16(&new_pattern);
                let candidate = fuzzy_score(
                    &new_pattern,
                    &low,
                    pattern_pos,
                    word,
                    word_low,
                    word_pos,
                    options,
                );
                if let Some(mut candidate) = candidate {
                    candidate.score -= 3; // Permutation penalty.
                    if top.as_ref().is_none_or(|top| candidate.score > top.score) {
                        top = Some(candidate);
                    }
                }
            }
        }
    }
    top
}

fn next_typo_permutation(pattern: &[u16], pattern_pos: usize) -> Option<Vec<u16>> {
    if pattern_pos + 1 >= pattern.len() || pattern[pattern_pos] == pattern[pattern_pos + 1] {
        return None;
    }
    let mut result = pattern.to_vec();
    result.swap(pattern_pos, pattern_pos + 1);
    Some(result)
}

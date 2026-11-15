//! Tokens of one line: contiguous spans of the line text, each with a packed metadata word.
//!
//! Ported from `src/vs/editor/common/tokens/lineTokens.ts` (`LineTokens`, `SliceLineTokens`,
//! `IViewLineTokens`, `findIndexInTokensArray`, `withInserted`, `createEmpty`). Parity tests
//! port `src/vs/editor/test/common/core/lineTokens.test.ts` (names are the snake-case forms).
//!
//! Representation: a list of `(end_offset, metadata)` pairs. The end offsets are strictly
//! increasing, and the last one equals the length of the line. Offsets count UTF-16 code units,
//! as in the text model (ADR 0003); the text itself is a UTF-8 `String`, so offsets are converted
//! to byte positions at each access.
//!
//! Divergences from the TypeScript:
//! - Language ids are decoded with an explicit [`LanguageIdCodec`] argument instead of being
//!   stored in the tokens. The codec is shared state owned by the caller.
//! - Invariant violations (token ends not matching the text length, unsorted or out-of-range
//!   insertions) are `debug_assert`s. TypeScript reports the first through `onUnexpectedError`.
//! - Only the view methods used by the tokenizer and the renderer are ported. Equality, the
//!   presentation class-name and inline-style helpers, and `TokenArray` are not here yet.
//! - Slice deltas are unsigned. The TypeScript delta may be negative, which no caller here needs.

use crate::encoded_token_attributes::{
    ColorId, FontStyle, StandardTokenType, TokenMetadata, TokenPresentation,
};
use crate::language_id_codec::LanguageIdCodec;
use vscode_api::LanguageId;

/// A token inserted by [`LineTokens::with_inserted`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenInsertion {
    /// UTF-16 offset in the original line where the text is inserted.
    pub offset: u32,
    /// The inserted text.
    pub text: String,
    /// Metadata of the inserted token.
    pub metadata: u32,
}

/// Read access to a sequence of line tokens (`IViewLineTokens`). Implemented by [`LineTokens`]
/// and by the view returned from [`LineTokens::slice_and_inflate`].
///
/// Required methods are the storage primitives; the rest are derived from the metadata word.
pub trait ViewLineTokens {
    /// Number of tokens.
    fn count(&self) -> usize;
    /// UTF-16 offset just past token `token`.
    fn end_offset(&self, token: usize) -> u32;
    /// Packed metadata of token `token`.
    fn metadata(&self, token: usize) -> u32;
    /// The text covered by these tokens.
    fn line_content(&self) -> &str;
    /// The text of token `token`.
    fn token_text(&self, token: usize) -> &str;
    /// Index of the token containing UTF-16 offset `offset` (`findTokenIndexAtOffset`). An offset
    /// on a boundary belongs to the token that starts there. Past the end, the last token.
    fn find_token_index_at_offset(&self, offset: u32) -> usize;

    /// Standard token type of token `token`.
    fn standard_token_type(&self, token: usize) -> StandardTokenType {
        TokenMetadata::token_type(self.metadata(token))
    }

    /// Foreground color slot of token `token`.
    fn foreground(&self, token: usize) -> ColorId {
        TokenMetadata::foreground(self.metadata(token))
    }

    /// Font style of token `token`.
    fn font_style(&self, token: usize) -> FontStyle {
        TokenMetadata::font_style(self.metadata(token))
    }

    /// Presentation fields of token `token`.
    fn presentation(&self, token: usize) -> TokenPresentation {
        TokenMetadata::presentation(self.metadata(token))
    }

    /// Language id of token `token`, decoded through `codec` (`getLanguageId`).
    fn language_id(&self, token: usize, codec: &LanguageIdCodec) -> LanguageId {
        codec.decode(TokenMetadata::language_id(self.metadata(token)))
    }

    /// Offset where token `token` starts.
    fn start_offset(&self, token: usize) -> u32 {
        if token == 0 {
            0
        } else {
            self.end_offset(token - 1)
        }
    }
}

/// The tokens of one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineTokens {
    tokens: Vec<Token>,
    text: String,
    text_len: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Token {
    end_offset: u32,
    metadata: u32,
}

impl LineTokens {
    /// One token spanning the whole line, with the default metadata (`createEmpty`).
    pub fn create_empty(line: impl Into<String>) -> Self {
        let text: String = line.into();
        let len = utf16_len(&text);
        Self::from_parts(
            text,
            vec![Token {
                end_offset: len,
                metadata: crate::encoded_token_attributes::DEFAULT_TOKEN_METADATA,
            }],
        )
    }

    /// Builds tokens from `(end_offset, metadata)` pairs over `text`.
    ///
    /// The last end offset must equal the UTF-16 length of `text`, and the ends must not decrease.
    pub fn new(text: impl Into<String>, tokens: &[(u32, u32)]) -> Self {
        let tokens = tokens
            .iter()
            .map(|&(end_offset, metadata)| Token {
                end_offset,
                metadata,
            })
            .collect();
        Self::from_parts(text.into(), tokens)
    }

    fn from_parts(text: String, tokens: Vec<Token>) -> Self {
        let text_len = utf16_len(&text);
        let last_end = tokens.last().map_or(0, |t| t.end_offset);
        debug_assert_eq!(
            last_end, text_len,
            "token length and text length do not match"
        );
        debug_assert!(
            tokens
                .windows(2)
                .all(|w| w[0].end_offset <= w[1].end_offset),
            "token end offsets must not decrease"
        );
        Self {
            tokens,
            text,
            text_len,
        }
    }

    /// Length of the line in UTF-16 code units.
    pub fn text_len(&self) -> u32 {
        self.text_len
    }

    /// Returns the tokens with `insertions` spliced in (`withInserted`). The original tokens keep
    /// their metadata; an insertion inside a token splits that token. Insertions must be sorted by
    /// offset and lie within the line.
    pub fn with_inserted(&self, insertions: &[TokenInsertion]) -> LineTokens {
        if insertions.is_empty() {
            return self.clone();
        }

        let mut out = Builder::default();
        let mut next_original = 0;
        let mut next_insertion = 0;
        let mut original_end: u32 = 0;

        loop {
            let next_original_end = self.tokens.get(next_original).map(|t| t.end_offset);
            let next_insert = insertions.get(next_insertion);
            let take_original = match (next_original_end, next_insert) {
                (Some(end), Some(insert)) => end <= insert.offset,
                (Some(_), None) => true,
                (None, _) => false,
            };

            if take_original {
                // The original token ends before the next insertion (or there is none).
                let end = next_original_end.unwrap_or_default();
                out.append(self.substring(original_end, end));
                out.close_token(self.tokens[next_original].metadata);
                next_original += 1;
                original_end = end;
            } else if let Some(insert) = next_insert {
                // The insertion comes first: cut the current original token at its offset.
                if insert.offset > original_end {
                    debug_assert!(next_original < self.tokens.len());
                    out.append(self.substring(original_end, insert.offset));
                    out.close_token(self.tokens[next_original].metadata);
                    original_end = insert.offset;
                }
                out.append(&insert.text);
                out.close_token(insert.metadata);
                next_insertion += 1;
            } else {
                break;
            }
        }

        Self::from_parts(out.text, out.tokens)
    }

    /// A read-only view of tokens `start_offset..end_offset` of this line, with every end offset
    /// shifted by `delta_offset` (`sliceAndInflate`). Tokens that only partly overlap the range
    /// are clipped to it. Requires `start_offset <= end_offset <= text_len`.
    pub fn slice_and_inflate(
        &self,
        start_offset: u32,
        end_offset: u32,
        delta_offset: u32,
    ) -> SlicedLineTokens<'_> {
        SlicedLineTokens::new(self, start_offset, end_offset, delta_offset)
    }

    /// Index of the token that contains `desired` (`findIndexInTokensArray`).
    fn find_index(tokens: &[Token], desired: u32) -> usize {
        if tokens.len() <= 1 {
            return 0;
        }
        let mut low = 0;
        let mut high = tokens.len() - 1;
        while low < high {
            let mid = low + (high - low) / 2;
            let end = tokens[mid].end_offset;
            if end == desired {
                return mid + 1;
            } else if end < desired {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        low
    }

    fn substring(&self, start: u32, end: u32) -> &str {
        substring_utf16(&self.text, start, end)
    }
}

impl ViewLineTokens for LineTokens {
    fn count(&self) -> usize {
        self.tokens.len()
    }

    fn end_offset(&self, token: usize) -> u32 {
        self.tokens[token].end_offset
    }

    fn metadata(&self, token: usize) -> u32 {
        self.tokens[token].metadata
    }

    fn line_content(&self) -> &str {
        &self.text
    }

    fn token_text(&self, token: usize) -> &str {
        let start = self.start_offset(token);
        self.substring(start, self.tokens[token].end_offset)
    }

    fn find_token_index_at_offset(&self, offset: u32) -> usize {
        Self::find_index(&self.tokens, offset)
    }
}

/// A view of a range of a [`LineTokens`], created by [`LineTokens::slice_and_inflate`]
/// (`SliceLineTokens`).
#[derive(Clone, Debug)]
pub struct SlicedLineTokens<'a> {
    source: &'a LineTokens,
    start: u32,
    end: u32,
    delta: u32,
    first_token: usize,
    count: usize,
}

impl<'a> SlicedLineTokens<'a> {
    fn new(source: &'a LineTokens, start: u32, end: u32, delta: u32) -> Self {
        debug_assert!(start <= end && end <= source.text_len);
        let first_token = source.find_token_index_at_offset(start);
        let count = (first_token..source.count())
            .take_while(|&i| source.start_offset(i) < end)
            .count();
        Self {
            source,
            start,
            end,
            delta,
            first_token,
            count,
        }
    }
}

impl ViewLineTokens for SlicedLineTokens<'_> {
    fn count(&self) -> usize {
        self.count
    }

    fn end_offset(&self, token: usize) -> u32 {
        let source_end = self.source.end_offset(self.first_token + token);
        source_end.min(self.end) - self.start + self.delta
    }

    fn metadata(&self, token: usize) -> u32 {
        self.source.metadata(self.first_token + token)
    }

    fn line_content(&self) -> &str {
        self.source.substring(self.start, self.end)
    }

    fn token_text(&self, token: usize) -> &str {
        let index = self.first_token + token;
        let token_start = self.source.start_offset(index);
        let token_end = self.source.end_offset(index);
        self.source
            .substring(token_start.max(self.start), token_end.min(self.end))
    }

    fn find_token_index_at_offset(&self, offset: u32) -> usize {
        // Map the view offset back to the source line. Offsets before the view map to its first
        // token rather than underflowing.
        let source_offset = (offset + self.start).saturating_sub(self.delta);
        self.source
            .find_token_index_at_offset(source_offset)
            .saturating_sub(self.first_token)
    }
}

/// Accumulates text and token ends for [`LineTokens::with_inserted`].
#[derive(Default)]
struct Builder {
    text: String,
    text_len: u32,
    tokens: Vec<Token>,
}

impl Builder {
    fn append(&mut self, piece: &str) {
        self.text.push_str(piece);
        self.text_len += utf16_len(piece);
    }

    fn close_token(&mut self, metadata: u32) {
        self.tokens.push(Token {
            end_offset: self.text_len,
            metadata,
        });
    }
}

/// Length of `s` in UTF-16 code units.
fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// The part of `s` between two UTF-16 offsets. A boundary inside a surrogate pair is rounded to
/// the start of that character, since a `&str` cannot hold half of one.
fn substring_utf16(s: &str, start: u32, end: u32) -> &str {
    &s[byte_index(s, start)..byte_index(s, end)]
}

/// Byte index of the character that contains UTF-16 offset `offset`. Past the end, `s.len()`.
fn byte_index(s: &str, offset: u32) -> usize {
    let mut units = 0;
    for (index, c) in s.char_indices() {
        let next = units + c.len_utf16() as u32;
        if next > offset {
            return index;
        }
        units = next;
    }
    s.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoded_token_attributes::metadata::FOREGROUND_OFFSET;

    /// Builds a line whose token `i` has foreground `i + 1`, starting at `starts[i]`.
    fn create_line_tokens(text: &str, starts: &[(u32, u32)]) -> LineTokens {
        // `starts` holds (start index, foreground) pairs.
        let text_len = utf16_len(text);
        let pairs: Vec<(u32, u32)> = starts
            .iter()
            .enumerate()
            .map(|(i, &(_, foreground))| {
                let end = starts.get(i + 1).map_or(text_len, |next| next.0);
                (end, foreground << FOREGROUND_OFFSET)
            })
            .collect();
        LineTokens::new(text, &pairs)
    }

    fn create_test_line_tokens() -> LineTokens {
        create_line_tokens(
            "Hello world, this is a lovely day",
            &[(0, 1), (6, 2), (13, 3), (18, 4), (21, 5), (23, 6), (30, 7)],
        )
    }

    /// Same as the upstream `renderLineTokens` helper, which slices the line by end offsets.
    fn render_line_tokens(tokens: &LineTokens) -> String {
        let text = tokens.line_content();
        let mut result = String::new();
        let mut last = 0;
        for i in 0..tokens.count() {
            result.push_str(substring_utf16(text, last, tokens.end_offset(i)));
            result.push_str(&format!("({})", tokens.metadata(i)));
            last = tokens.end_offset(i);
        }
        result
    }

    fn assert_view(actual: &impl ViewLineTokens, expected: &[(u32, u32)]) {
        let actual: Vec<(u32, u32)> = (0..actual.count())
            .map(|i| (actual.end_offset(i), actual.foreground(i).0))
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn with_inserted_1() {
        let line = create_test_line_tokens();
        assert_eq!(
            render_line_tokens(&line),
            "Hello (32768)world, (65536)this (98304)is (131072)a (163840)lovely (196608)day(229376)"
        );

        let line2 = line.with_inserted(&[
            TokenInsertion {
                offset: 0,
                text: "1".into(),
                metadata: 0,
            },
            TokenInsertion {
                offset: 6,
                text: "2".into(),
                metadata: 0,
            },
            TokenInsertion {
                offset: 9,
                text: "3".into(),
                metadata: 0,
            },
        ]);

        assert_eq!(
            render_line_tokens(&line2),
            "1(0)Hello (32768)2(0)wor(65536)3(0)ld, (65536)this (98304)is (131072)a (163840)lovely (196608)day(229376)"
        );
    }

    #[test]
    fn with_inserted_tokens_at_the_same_position() {
        let line = create_test_line_tokens();
        let line2 = line.with_inserted(&[
            TokenInsertion {
                offset: 0,
                text: "1".into(),
                metadata: 0,
            },
            TokenInsertion {
                offset: 0,
                text: "2".into(),
                metadata: 0,
            },
            TokenInsertion {
                offset: 0,
                text: "3".into(),
                metadata: 0,
            },
        ]);

        assert_eq!(
            render_line_tokens(&line2),
            "1(0)2(0)3(0)Hello (32768)world, (65536)this (98304)is (131072)a (163840)lovely (196608)day(229376)"
        );
    }

    #[test]
    fn with_inserted_tokens_at_the_end() {
        let line = create_test_line_tokens();
        let line2 = line.with_inserted(&[
            TokenInsertion {
                offset: 32,
                text: "1".into(),
                metadata: 0,
            },
            TokenInsertion {
                offset: 33,
                text: "2".into(),
                metadata: 0,
            },
        ]);

        assert_eq!(
            render_line_tokens(&line2),
            "Hello (32768)world, (65536)this (98304)is (131072)a (163840)lovely (196608)da(229376)1(0)y(229376)2(0)"
        );
    }

    #[test]
    fn with_inserted_without_insertions_is_identity() {
        let line = create_test_line_tokens();
        assert_eq!(line.with_inserted(&[]), line);
    }

    #[test]
    fn basics() {
        let line = create_test_line_tokens();

        assert_eq!(line.line_content(), "Hello world, this is a lovely day");
        assert_eq!(line.line_content().len(), 33);
        assert_eq!(line.count(), 7);

        let expected = [
            (0, 6),
            (6, 13),
            (13, 18),
            (18, 21),
            (21, 23),
            (23, 30),
            (30, 33),
        ];
        for (i, (start, end)) in expected.into_iter().enumerate() {
            assert_eq!(line.start_offset(i), start);
            assert_eq!(line.end_offset(i), end);
        }
    }

    #[test]
    fn find_token() {
        let line = create_test_line_tokens();
        // (offset, token index) pairs, one per offset from 0 to 34.
        let expected = [
            0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 5, 5, 5, 5, 5, 5,
            5, 6, 6, 6, 6, 6,
        ];
        for (offset, index) in expected.into_iter().enumerate() {
            assert_eq!(
                line.find_token_index_at_offset(offset as u32),
                index,
                "offset {offset}"
            );
        }
    }

    #[test]
    fn inflate() {
        let line = create_test_line_tokens();
        assert_view(
            &line,
            &[(6, 1), (13, 2), (18, 3), (21, 4), (23, 5), (30, 6), (33, 7)],
        );
    }

    #[test]
    fn slice_and_inflate() {
        let line = create_test_line_tokens();

        assert_view(
            &line.slice_and_inflate(0, 33, 0),
            &[(6, 1), (13, 2), (18, 3), (21, 4), (23, 5), (30, 6), (33, 7)],
        );
        assert_view(
            &line.slice_and_inflate(0, 32, 0),
            &[(6, 1), (13, 2), (18, 3), (21, 4), (23, 5), (30, 6), (32, 7)],
        );
        assert_view(
            &line.slice_and_inflate(0, 30, 0),
            &[(6, 1), (13, 2), (18, 3), (21, 4), (23, 5), (30, 6)],
        );
        assert_view(
            &line.slice_and_inflate(0, 30, 1),
            &[(7, 1), (14, 2), (19, 3), (22, 4), (24, 5), (31, 6)],
        );
        assert_view(&line.slice_and_inflate(6, 18, 0), &[(7, 2), (12, 3)]);
        assert_view(&line.slice_and_inflate(7, 18, 0), &[(6, 2), (11, 3)]);
        assert_view(&line.slice_and_inflate(6, 17, 0), &[(7, 2), (11, 3)]);
        assert_view(
            &line.slice_and_inflate(6, 19, 0),
            &[(7, 2), (12, 3), (13, 4)],
        );
    }

    #[test]
    fn slice_exposes_clipped_text() {
        let line = create_test_line_tokens();
        let slice = line.slice_and_inflate(7, 18, 0);
        assert_eq!(slice.line_content(), "orld, this ");
        assert_eq!(slice.token_text(0), "orld, ");
        assert_eq!(slice.token_text(1), "this ");
        assert_eq!(slice.find_token_index_at_offset(0), 0);
        assert_eq!(slice.find_token_index_at_offset(6), 1);
    }

    #[test]
    fn language_id_is_decoded_through_the_codec() {
        let mut codec = LanguageIdCodec::new();
        codec.register("rust");
        let encoded = codec.encode("rust");
        let metadata = TokenMetadata::pack(
            encoded,
            StandardTokenType::Other,
            false,
            FontStyle::NONE,
            ColorId::NONE,
            ColorId::NONE,
        );
        let line = LineTokens::new("fn", &[(2, metadata)]);
        assert_eq!(line.language_id(0, &codec).as_str(), "rust");
        assert_eq!(line.standard_token_type(0), StandardTokenType::Other);
    }

    #[test]
    fn create_empty_spans_the_line_with_default_metadata() {
        let line = LineTokens::create_empty("abc");
        assert_eq!(line.count(), 1);
        assert_eq!(line.end_offset(0), 3);
        assert_eq!(
            line.metadata(0),
            crate::encoded_token_attributes::DEFAULT_TOKEN_METADATA
        );
        assert_eq!(line.token_text(0), "abc");
    }

    #[test]
    fn offsets_count_utf16_units() {
        // "😀" is two UTF-16 units and four UTF-8 bytes.
        let line = LineTokens::new("a😀b", &[(1, 0), (3, 1), (4, 2)]);
        assert_eq!(line.text_len(), 4);
        assert_eq!(line.token_text(1), "😀");
        assert_eq!(line.find_token_index_at_offset(2), 1);
        assert_eq!(line.find_token_index_at_offset(3), 2);

        let inserted = line.with_inserted(&[TokenInsertion {
            offset: 3,
            text: "x".into(),
            metadata: 9,
        }]);
        assert_eq!(inserted.line_content(), "a😀xb");
        assert_eq!(inserted.end_offset(2), 4);
    }
}

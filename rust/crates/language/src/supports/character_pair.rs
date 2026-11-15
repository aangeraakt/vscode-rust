//! Auto-closing and surrounding pairs derived from a language configuration.
//!
//! Ported from `src/vs/editor/common/languages/supports/characterPair.ts`
//! (`CharacterPairSupport`). Tests port `src/vs/editor/test/common/modes/supports/characterPair.test.ts`
//! (snake-case names). Upstream builds the token fixtures with `createFakeScopedLineTokens`
//! from `modesTestUtils.ts`; the test module here builds the same tokens with [`LineTokens`].

use crate::language_configuration::{
    AutoClosingPair, AutoClosingPairConditional, LanguageConfiguration,
    StandardAutoClosingPairConditional,
};

/// Characters that may follow the cursor for quote pairs when `autoCloseBefore` is not set.
pub const DEFAULT_AUTOCLOSE_BEFORE_LANGUAGE_DEFINED_QUOTES: &str = ";:.,=}])> \n\t";
/// Characters that may follow the cursor for bracket pairs when `autoCloseBefore` is not set.
pub const DEFAULT_AUTOCLOSE_BEFORE_LANGUAGE_DEFINED_BRACKETS: &str = "'\"`;:.,=}])> \n\t";
/// Characters that count as whitespace for auto-closing.
pub const DEFAULT_AUTOCLOSE_BEFORE_WHITESPACE: &str = " \n\t";

/// The auto-closing and surrounding pairs of one language.
#[derive(Clone, Debug)]
pub struct CharacterPairSupport {
    auto_closing_pairs: Vec<StandardAutoClosingPairConditional>,
    surrounding_pairs: Vec<AutoClosingPair>,
    auto_close_before_for_quotes: String,
    auto_close_before_for_brackets: String,
}

impl CharacterPairSupport {
    /// Derives the pairs from `config`. Auto-closing pairs come from `auto_closing_pairs`, else
    /// from `brackets`, else none. The legacy doc comment pair is appended. Surrounding pairs
    /// default to the auto-closing pairs.
    pub fn new(config: &LanguageConfiguration) -> Self {
        let mut auto_closing_pairs: Vec<StandardAutoClosingPairConditional> =
            if let Some(pairs) = &config.auto_closing_pairs {
                pairs
                    .iter()
                    .cloned()
                    .map(StandardAutoClosingPairConditional::new)
                    .collect()
            } else if let Some(brackets) = &config.brackets {
                brackets
                    .iter()
                    .map(|(open, close)| {
                        StandardAutoClosingPairConditional::new(AutoClosingPairConditional::new(
                            open.as_str(),
                            close.as_str(),
                        ))
                    })
                    .collect()
            } else {
                Vec::new()
            };

        if let Some(doc) = &config.doc_comment {
            auto_closing_pairs.push(StandardAutoClosingPairConditional::new(
                AutoClosingPairConditional::new(
                    doc.open.as_str(),
                    doc.close.as_deref().unwrap_or_default(),
                ),
            ));
        }

        let surrounding_pairs = match &config.surrounding_pairs {
            Some(pairs) => pairs.clone(),
            None => auto_closing_pairs
                .iter()
                .map(|pair| AutoClosingPair {
                    open: pair.open.clone(),
                    close: pair.close.clone(),
                })
                .collect(),
        };

        let (auto_close_before_for_quotes, auto_close_before_for_brackets) =
            match &config.auto_close_before {
                Some(set) => (set.clone(), set.clone()),
                None => (
                    DEFAULT_AUTOCLOSE_BEFORE_LANGUAGE_DEFINED_QUOTES.to_owned(),
                    DEFAULT_AUTOCLOSE_BEFORE_LANGUAGE_DEFINED_BRACKETS.to_owned(),
                ),
            };

        Self {
            auto_closing_pairs,
            surrounding_pairs,
            auto_close_before_for_quotes,
            auto_close_before_for_brackets,
        }
    }

    /// The auto-closing pairs (`getAutoClosingPairs`).
    pub fn auto_closing_pairs(&self) -> &[StandardAutoClosingPairConditional] {
        &self.auto_closing_pairs
    }

    /// The pairs used to surround a selection (`getSurroundingPairs`).
    pub fn surrounding_pairs(&self) -> &[AutoClosingPair] {
        &self.surrounding_pairs
    }

    /// The characters that may follow the cursor for auto-closing a quote (`for_quotes`) or a
    /// bracket (`getAutoCloseBeforeSet`).
    pub fn auto_close_before_set(&self, for_quotes: bool) -> &str {
        if for_quotes {
            &self.auto_close_before_for_quotes
        } else {
            &self.auto_close_before_for_brackets
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoded_token_attributes::{ColorId, FontStyle, StandardTokenType, TokenMetadata};
    use crate::language_configuration::{DocComment, NotIn};
    use crate::line_tokens::LineTokens;

    /// Upstream's `createFakeScopedLineTokens`: one token per `(text, type)`.
    fn fake_line(tokens: &[(&str, StandardTokenType)]) -> LineTokens {
        let mut text = String::new();
        let mut ends = Vec::new();
        for (piece, token_type) in tokens {
            text.push_str(piece);
            let metadata = TokenMetadata::pack(
                0,
                *token_type,
                false,
                FontStyle::NONE,
                ColorId::NONE,
                ColorId::NONE,
            );
            ends.push((text.encode_utf16().count() as u32, metadata));
        }
        LineTokens::new(text, &ends)
    }

    fn pair(open: &str, close: &str) -> AutoClosingPair {
        AutoClosingPair {
            open: open.into(),
            close: close.into(),
        }
    }

    fn auto_closing(support: &CharacterPairSupport) -> Vec<AutoClosingPair> {
        support
            .auto_closing_pairs()
            .iter()
            .map(|p| pair(&p.open, &p.close))
            .collect()
    }

    fn with_pairs(pairs: Vec<AutoClosingPairConditional>) -> LanguageConfiguration {
        LanguageConfiguration {
            auto_closing_pairs: Some(pairs),
            ..Default::default()
        }
    }

    #[test]
    fn only_auto_closing_pairs() {
        let support =
            CharacterPairSupport::new(&with_pairs(vec![AutoClosingPairConditional::new("a", "b")]));
        assert_eq!(auto_closing(&support), [pair("a", "b")]);
        assert_eq!(support.surrounding_pairs(), [pair("a", "b")]);
    }

    #[test]
    fn only_empty_auto_closing_pairs() {
        let support = CharacterPairSupport::new(&with_pairs(Vec::new()));
        assert!(support.auto_closing_pairs().is_empty());
        assert!(support.surrounding_pairs().is_empty());
    }

    #[test]
    fn only_brackets() {
        let config = LanguageConfiguration {
            brackets: Some(vec![("a".into(), "b".into())]),
            ..Default::default()
        };
        let support = CharacterPairSupport::new(&config);
        assert_eq!(auto_closing(&support), [pair("a", "b")]);
        assert_eq!(support.surrounding_pairs(), [pair("a", "b")]);
    }

    #[test]
    fn only_empty_brackets() {
        let config = LanguageConfiguration {
            brackets: Some(Vec::new()),
            ..Default::default()
        };
        let support = CharacterPairSupport::new(&config);
        assert!(support.auto_closing_pairs().is_empty());
        assert!(support.surrounding_pairs().is_empty());
    }

    #[test]
    fn only_surrounding_pairs() {
        let config = LanguageConfiguration {
            surrounding_pairs: Some(vec![pair("a", "b")]),
            ..Default::default()
        };
        let support = CharacterPairSupport::new(&config);
        assert!(support.auto_closing_pairs().is_empty());
        assert_eq!(support.surrounding_pairs(), [pair("a", "b")]);
    }

    #[test]
    fn only_empty_surrounding_pairs() {
        let config = LanguageConfiguration {
            surrounding_pairs: Some(Vec::new()),
            ..Default::default()
        };
        let support = CharacterPairSupport::new(&config);
        assert!(support.auto_closing_pairs().is_empty());
        assert!(support.surrounding_pairs().is_empty());
    }

    #[test]
    fn brackets_is_ignored_when_having_auto_closing_pairs() {
        let config = LanguageConfiguration {
            auto_closing_pairs: Some(Vec::new()),
            brackets: Some(vec![("a".into(), "b".into())]),
            ..Default::default()
        };
        let support = CharacterPairSupport::new(&config);
        assert!(support.auto_closing_pairs().is_empty());
        assert!(support.surrounding_pairs().is_empty());
    }

    /// Whether the first auto-closing pair may close at `column` of `line`.
    fn should_auto_close(support: &CharacterPairSupport, line: &LineTokens, column: u32) -> bool {
        support.auto_closing_pairs()[0].should_auto_close(line, column)
    }

    fn braces_not_in_string_or_comment() -> CharacterPairSupport {
        let mut source = AutoClosingPairConditional::new("{", "}");
        source.not_in = vec![NotIn::String, NotIn::Comment];
        CharacterPairSupport::new(&with_pairs(vec![source]))
    }

    #[test]
    fn should_auto_close_pair_in_empty_line() {
        let support = braces_not_in_string_or_comment();
        assert!(should_auto_close(&support, &fake_line(&[]), 1));
    }

    #[test]
    fn should_auto_close_pair_in_not_interesting_line_1() {
        let support = braces_not_in_string_or_comment();
        let line = fake_line(&[("do", StandardTokenType::Other)]);
        assert!(should_auto_close(&support, &line, 3));
    }

    #[test]
    fn should_auto_close_pair_in_not_interesting_line_2() {
        // The pair does not exclude strings, so a string token does not block it.
        let support =
            CharacterPairSupport::new(&with_pairs(vec![AutoClosingPairConditional::new("{", "}")]));
        let line = fake_line(&[("do", StandardTokenType::String)]);
        assert!(should_auto_close(&support, &line, 3));
    }

    #[test]
    fn should_auto_close_pair_in_interesting_line_1() {
        let support = braces_not_in_string_or_comment();
        let line = fake_line(&[("\"a\"", StandardTokenType::String)]);
        for column in 1..=4 {
            assert!(
                !should_auto_close(&support, &line, column),
                "column {column}"
            );
        }
    }

    #[test]
    fn should_auto_close_pair_in_interesting_line_2() {
        let support = braces_not_in_string_or_comment();
        let line = fake_line(&[
            ("x=", StandardTokenType::Other),
            ("\"a\"", StandardTokenType::String),
            (";", StandardTokenType::Other),
        ]);
        let expected = [true, true, true, false, false, false, true];
        for (index, want) in expected.into_iter().enumerate() {
            let column = index as u32 + 1;
            assert_eq!(
                should_auto_close(&support, &line, column),
                want,
                "column {column}"
            );
        }
    }

    #[test]
    fn should_auto_close_pair_in_interesting_line_3() {
        let support = braces_not_in_string_or_comment();
        let line = fake_line(&[
            (" ", StandardTokenType::Other),
            ("//a", StandardTokenType::Comment),
        ]);
        let expected = [true, true, false, false, false];
        for (index, want) in expected.into_iter().enumerate() {
            let column = index as u32 + 1;
            assert_eq!(
                should_auto_close(&support, &line, column),
                want,
                "column {column}"
            );
        }
    }

    #[test]
    fn doc_comment_is_appended_to_the_auto_closing_pairs() {
        let config = LanguageConfiguration {
            brackets: Some(vec![("(".into(), ")".into())]),
            doc_comment: Some(DocComment {
                open: "/**".into(),
                close: Some(" */".into()),
            }),
            ..Default::default()
        };
        let support = CharacterPairSupport::new(&config);
        assert_eq!(auto_closing(&support), [pair("(", ")"), pair("/**", " */")]);
        // Surrounding pairs default to the auto-closing pairs, so they include the doc comment.
        assert_eq!(support.surrounding_pairs().len(), 2);
    }

    #[test]
    fn auto_close_before_defaults_and_overrides() {
        let default = CharacterPairSupport::new(&LanguageConfiguration::default());
        assert_eq!(
            default.auto_close_before_set(true),
            DEFAULT_AUTOCLOSE_BEFORE_LANGUAGE_DEFINED_QUOTES
        );
        assert_eq!(
            default.auto_close_before_set(false),
            DEFAULT_AUTOCLOSE_BEFORE_LANGUAGE_DEFINED_BRACKETS
        );

        let custom = CharacterPairSupport::new(&LanguageConfiguration {
            auto_close_before: Some(" )".into()),
            ..Default::default()
        });
        assert_eq!(custom.auto_close_before_set(true), " )");
        assert_eq!(custom.auto_close_before_set(false), " )");
    }
}

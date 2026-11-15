//! Language configuration: the per-language data that drives brackets, auto-closing, and indent
//! behavior.
//!
//! Ported from `src/vs/editor/common/languages/languageConfiguration.ts`. This PR covers the
//! auto-closing and bracket types; on-enter and indentation types come with their supports.
//!
//! Regular expressions use [`fancy_regex::Regex`] rather than the `regex` crate. Language
//! configurations are written against JavaScript regex syntax, and the shipped rules use
//! lookahead (`(?!...)`), which `regex` does not support. A runtime failure of `fancy-regex`
//! (its backtracking limit) is treated as "no match". JavaScript would never fail there, so this
//! is the one place where behavior can differ.
//!
//! Divergences from the TypeScript:
//! - `notIn` strings are parsed into [`NotIn`] by the caller. Unknown strings are ignored in
//!   TypeScript; the caller should do the same when it builds the list.
//! - `StandardAutoClosingPairConditional.findNeutralCharacter` and `AutoClosingPairs`, the
//!   per-character lookup index, are not ported yet.

use crate::encoded_token_attributes::StandardTokenType;
use crate::line_tokens::ViewLineTokens;
use fancy_regex::Regex;

/// An opening and closing string, such as `("(", ")")`.
pub type CharacterPair = (String, String);

/// An open/close pair (`IAutoClosingPair`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoClosingPair {
    /// The opening text.
    pub open: String,
    /// The closing text.
    pub close: String,
}

/// A token category an auto-closing pair may be disabled in (`notIn`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotIn {
    /// String literals.
    String,
    /// Comments.
    Comment,
    /// Regular expression literals.
    RegEx,
}

/// An auto-closing pair with its conditions (`IAutoClosingPairConditional`).
#[derive(Clone, Debug)]
pub struct AutoClosingPairConditional {
    /// The opening text.
    pub open: String,
    /// The closing text.
    pub close: String,
    /// Token categories where the pair must not auto-close.
    pub not_in: Vec<NotIn>,
    /// If set, the pair auto-closes only when the text before the cursor matches.
    pub before_text: Option<Regex>,
}

impl AutoClosingPairConditional {
    /// A pair with no conditions.
    pub fn new(open: impl Into<String>, close: impl Into<String>) -> Self {
        Self {
            open: open.into(),
            close: close.into(),
            not_in: Vec::new(),
            before_text: None,
        }
    }
}

/// An auto-closing pair with its conditions resolved (`StandardAutoClosingPairConditional`).
#[derive(Clone, Debug)]
pub struct StandardAutoClosingPairConditional {
    /// The opening text.
    pub open: String,
    /// The closing text.
    pub close: String,
    /// The `beforeText` condition, if any.
    pub before_text: Option<Regex>,
    in_string: bool,
    in_comment: bool,
    in_regex: bool,
}

impl StandardAutoClosingPairConditional {
    /// Resolves the `notIn` list into per-category flags.
    pub fn new(source: AutoClosingPairConditional) -> Self {
        let excluded = |category: NotIn| source.not_in.contains(&category);
        Self {
            in_string: !excluded(NotIn::String),
            in_comment: !excluded(NotIn::Comment),
            in_regex: !excluded(NotIn::RegEx),
            open: source.open,
            close: source.close,
            before_text: source.before_text,
        }
    }

    /// Whether the pair may auto-close when the text before the cursor is `text`
    /// (`shouldAutoCloseBefore`). A pair with no `beforeText` always may.
    pub fn should_auto_close_before(&self, text: &str) -> bool {
        match &self.before_text {
            None => true,
            // A runtime regex failure counts as no match; see the module docs.
            Some(regex) => regex.is_match(text).unwrap_or(false),
        }
    }

    /// Whether the pair is allowed in a token of type `token` (`isOK`).
    pub fn is_ok(&self, token: StandardTokenType) -> bool {
        match token {
            StandardTokenType::Other => true,
            StandardTokenType::Comment => self.in_comment,
            StandardTokenType::String => self.in_string,
            StandardTokenType::RegEx => self.in_regex,
        }
    }

    /// Whether the pair may auto-close at `column` (1-based) of a line with tokens `context`
    /// (`shouldAutoClose`). The token checked is the one just before the cursor. An empty line
    /// always allows it.
    pub fn should_auto_close(&self, context: &impl ViewLineTokens, column: u32) -> bool {
        if context.count() == 0 {
            return true;
        }
        // Column 1 looks at offset -1 upstream, which the token search maps to the first token.
        let token = context.find_token_index_at_offset(column.saturating_sub(2));
        self.is_ok(context.standard_token_type(token))
    }
}

/// The `__electricCharacterSupport.docComment` legacy option.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocComment {
    /// The opening text, such as `/**`.
    pub open: String,
    /// The closing text, such as `*/`. Empty when absent.
    pub close: Option<String>,
}

/// The parts of a language configuration this PR covers (`LanguageConfiguration`). A field left
/// as `None` means "not configured", which is not the same as an empty list.
#[derive(Clone, Debug, Default)]
pub struct LanguageConfiguration {
    /// Bracket pairs. Used as auto-closing pairs when `auto_closing_pairs` is `None`.
    pub brackets: Option<Vec<CharacterPair>>,
    /// Auto-closing pairs with conditions.
    pub auto_closing_pairs: Option<Vec<AutoClosingPairConditional>>,
    /// Pairs used for surrounding a selection. Defaults to the auto-closing pairs.
    pub surrounding_pairs: Option<Vec<AutoClosingPair>>,
    /// Characters that may follow the cursor for auto-closing. Defaults depend on the pair kind.
    pub auto_close_before: Option<String>,
    /// The legacy doc comment pair.
    pub doc_comment: Option<DocComment>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoded_token_attributes::{ColorId, FontStyle, TokenMetadata};
    use crate::line_tokens::LineTokens;

    #[test]
    fn not_in_flags_disable_only_the_named_categories() {
        let mut source = AutoClosingPairConditional::new("{", "}");
        source.not_in = vec![NotIn::String, NotIn::Comment];
        let pair = StandardAutoClosingPairConditional::new(source);

        assert!(!pair.is_ok(StandardTokenType::String));
        assert!(!pair.is_ok(StandardTokenType::Comment));
        assert!(pair.is_ok(StandardTokenType::RegEx));
        assert!(pair.is_ok(StandardTokenType::Other));
    }

    #[test]
    fn before_text_gates_auto_close() {
        let mut source = AutoClosingPairConditional::new("(", ")");
        source.before_text = Some(Regex::new(r"\s$").unwrap());
        let pair = StandardAutoClosingPairConditional::new(source);

        assert!(pair.should_auto_close_before("if "));
        assert!(!pair.should_auto_close_before("if"));
        assert!(
            StandardAutoClosingPairConditional::new(AutoClosingPairConditional::new("(", ")"))
                .should_auto_close_before("anything")
        );
    }

    #[test]
    fn empty_line_always_allows_auto_close() {
        let pair =
            StandardAutoClosingPairConditional::new(AutoClosingPairConditional::new("[", "]"));
        let line = LineTokens::create_empty("");
        assert!(pair.should_auto_close(&line, 1));
    }

    #[test]
    fn column_one_checks_the_first_token() {
        let mut source = AutoClosingPairConditional::new("'", "'");
        source.not_in = vec![NotIn::String];
        let pair = StandardAutoClosingPairConditional::new(source);
        let metadata = TokenMetadata::pack(
            0,
            StandardTokenType::String,
            false,
            FontStyle::NONE,
            ColorId::NONE,
            ColorId::NONE,
        );
        let line = LineTokens::new("\"x\"", &[(3, metadata)]);
        assert!(!pair.should_auto_close(&line, 1));
    }
}

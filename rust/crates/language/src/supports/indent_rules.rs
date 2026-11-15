//! Classifies lines for indentation with a language's [`IndentationRule`].
//!
//! Ported from `src/vs/editor/common/languages/supports/indentRules.ts` (`IndentRulesSupport`,
//! `IndentConsts`). Upstream has no dedicated test file for this class; its consumers are tested
//! in the indentation contrib. The tests here cover the class itself, so they make no parity
//! claim.
//!
//! Upstream resets `lastIndex` on global regexes before each test. Rust regexes have no such
//! state, so that step is not needed.

use crate::language_configuration::{IndentationRule, regex_test};

/// Bit set when the line increases indentation for the next line.
pub const INCREASE_MASK: u32 = 0b0001;
/// Bit set when the line decreases indentation.
pub const DECREASE_MASK: u32 = 0b0010;
/// Bit set when the next line is indented once.
pub const INDENT_NEXTLINE_MASK: u32 = 0b0100;
/// Bit set when the line's indentation is ignored.
pub const UNINDENT_MASK: u32 = 0b1000;

/// Answers indentation questions about lines for one language.
#[derive(Clone, Debug)]
pub struct IndentRulesSupport {
    rules: Option<IndentationRule>,
}

impl IndentRulesSupport {
    /// Creates the support. With no rules, every question answers "no".
    pub fn new(rules: Option<IndentationRule>) -> Self {
        Self { rules }
    }

    /// Whether `text` increases indentation (`shouldIncrease`).
    pub fn should_increase(&self, text: &str) -> bool {
        self.rules
            .as_ref()
            .is_some_and(|rules| regex_test(&rules.increase_indent_pattern, text))
    }

    /// Whether `text` decreases indentation (`shouldDecrease`).
    pub fn should_decrease(&self, text: &str) -> bool {
        self.rules
            .as_ref()
            .is_some_and(|rules| regex_test(&rules.decrease_indent_pattern, text))
    }

    /// Whether the line after `text` is indented once (`shouldIndentNextLine`).
    pub fn should_indent_next_line(&self, text: &str) -> bool {
        self.rules.as_ref().is_some_and(|rules| {
            rules
                .indent_next_line_pattern
                .as_ref()
                .is_some_and(|pattern| regex_test(pattern, text))
        })
    }

    /// Whether the indentation of `text` is ignored (`shouldIgnore`).
    pub fn should_ignore(&self, text: &str) -> bool {
        self.rules.as_ref().is_some_and(|rules| {
            rules
                .un_indented_line_pattern
                .as_ref()
                .is_some_and(|pattern| regex_test(pattern, text))
        })
    }

    /// The indentation classification of `text` as a bit set of the `*_MASK` constants
    /// (`getIndentMetadata`).
    pub fn indent_metadata(&self, text: &str) -> u32 {
        let mut bits = 0;
        if self.should_increase(text) {
            bits |= INCREASE_MASK;
        }
        if self.should_decrease(text) {
            bits |= DECREASE_MASK;
        }
        if self.should_indent_next_line(text) {
            bits |= INDENT_NEXTLINE_MASK;
        }
        if self.should_ignore(text) {
            bits |= UNINDENT_MASK;
        }
        bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fancy_regex::Regex;

    fn re(pattern: &str) -> Regex {
        Regex::new(pattern).expect("test pattern compiles")
    }

    /// The JavaScript rules from upstream's `indentationRules.ts` fixture.
    fn javascript_rules() -> IndentationRule {
        IndentationRule {
            decrease_indent_pattern: re(r#"^((?!.*?/\*).*\*/)?\s*[\}\]\)].*$"#),
            increase_indent_pattern: re(
                r#"^((?!//).)*(\{([^}"'`]*|(\t|[ ])*//.*)|\([^)"'`]*|\[[^\]"'`]*)$"#,
            ),
            un_indented_line_pattern: Some(re(
                r#"^(\t|[ ])*[ ]\*[^/]*\*/\s*$|^(\t|[ ])*[ ]\*/\s*$|^(\t|[ ])*[ ]\*([ ]([^\*]|\*(?!/))*)?$"#,
            )),
            indent_next_line_pattern: Some(re(
                r#"^((.*=>\s*)|((.*[^\w]+|\s*)(if|while|for)\s*\(.*\)\s*))$"#,
            )),
        }
    }

    #[test]
    fn classifies_javascript_lines() {
        let support = IndentRulesSupport::new(Some(javascript_rules()));
        assert!(support.should_increase("function f() {"));
        assert!(!support.should_increase("}"));
        assert!(support.should_decrease("}"));
        assert!(support.should_decrease("  );"));
        assert!(!support.should_decrease("function f() {"));
        assert!(support.should_ignore(" * a comment line"));
        assert!(support.should_indent_next_line("const f = () =>"));
    }

    #[test]
    fn metadata_is_a_bit_set_of_the_answers() {
        let support = IndentRulesSupport::new(Some(javascript_rules()));
        assert_eq!(support.indent_metadata("function f() {"), INCREASE_MASK);
        assert_eq!(support.indent_metadata("}"), DECREASE_MASK);
        assert_eq!(support.indent_metadata(" * a comment line"), UNINDENT_MASK);
        assert_eq!(
            support.indent_metadata("const f = () =>"),
            INDENT_NEXTLINE_MASK
        );
    }

    #[test]
    fn no_rules_answers_no_to_everything() {
        let support = IndentRulesSupport::new(None);
        assert!(!support.should_increase("{"));
        assert!(!support.should_decrease("}"));
        assert!(!support.should_indent_next_line("if (x)"));
        assert!(!support.should_ignore(" *"));
        assert_eq!(support.indent_metadata("{"), 0);
    }

    #[test]
    fn optional_patterns_absent_answer_no() {
        let rules = IndentationRule {
            decrease_indent_pattern: re(r"^\}"),
            increase_indent_pattern: re(r"\{$"),
            indent_next_line_pattern: None,
            un_indented_line_pattern: None,
        };
        let support = IndentRulesSupport::new(Some(rules));
        assert!(!support.should_indent_next_line("if (x)"));
        assert!(!support.should_ignore(" *"));
        assert_eq!(support.indent_metadata("{"), INCREASE_MASK);
    }
}

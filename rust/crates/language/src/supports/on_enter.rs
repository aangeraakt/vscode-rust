//! What happens to indentation when Enter is pressed: on-enter rules and bracket pairs.
//!
//! Ported from `src/vs/editor/common/languages/supports/onEnter.ts` (`OnEnterSupport`). Tests
//! port `src/vs/editor/test/common/modes/supports/onEnter.test.ts` (snake-case names). The
//! `javascriptOnEnterRules` fixture is copied from `onEnterRules.ts` into the test module.
//!
//! Divergences from the TypeScript:
//! - [`AutoIndentStrategy`] lives here because this module is its only user so far. It mirrors
//!   `EditorAutoIndentStrategy` in `config/editorOptions.ts` and belongs with the editor options
//!   once they are ported.
//! - Bracket regexes are built with [`fancy_regex::escape`], which escapes a superset of what
//!   `escapeRegExpCharacters` escapes. The escaped text matches the same strings.
//! - A bracket whose escaped pattern does not compile is a bug, not bad input, so it panics; the
//!   TypeScript reports and skips it.

use crate::language_configuration::{
    CharacterPair, EnterAction, IndentAction, OnEnterRule, regex_test,
};
use fancy_regex::Regex;

/// How much the editor changes indentation while typing (`EditorAutoIndentStrategy`). Each level
/// includes the ones before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AutoIndentStrategy {
    /// No automatic indentation.
    None,
    /// Keep the indentation of the previous line.
    Keep,
    /// Also handle brackets.
    Brackets,
    /// Also apply the language's on-enter rules.
    Advanced,
    /// Everything, including indentation rules.
    Full,
}

#[derive(Clone, Debug)]
struct BracketRegexes {
    open: Regex,
    close: Regex,
}

/// The on-enter behavior of one language.
#[derive(Clone, Debug)]
pub struct OnEnterSupport {
    brackets: Vec<BracketRegexes>,
    rules: Vec<OnEnterRule>,
}

impl OnEnterSupport {
    /// Builds the support. With no `brackets`, `(`/`)`, `{`/`}`, and `[`/`]` are used.
    pub fn new(brackets: Option<Vec<CharacterPair>>, on_enter_rules: Vec<OnEnterRule>) -> Self {
        let brackets = brackets.unwrap_or_else(|| {
            vec![
                ("(".to_owned(), ")".to_owned()),
                ("{".to_owned(), "}".to_owned()),
                ("[".to_owned(), "]".to_owned()),
            ]
        });
        Self {
            brackets: brackets
                .iter()
                .map(|(open, close)| BracketRegexes {
                    open: open_bracket_regex(open),
                    close: close_bracket_regex(close),
                })
                .collect(),
            rules: on_enter_rules,
        }
    }

    /// The action for pressing Enter between `before_enter_text` and `after_enter_text`, on a line
    /// that follows `previous_line_text`. `None` keeps the default behavior (`onEnter`).
    pub fn on_enter(
        &self,
        auto_indent: AutoIndentStrategy,
        previous_line_text: &str,
        before_enter_text: &str,
        after_enter_text: &str,
    ) -> Option<EnterAction> {
        // (1) Rules, the first match wins.
        if auto_indent >= AutoIndentStrategy::Advanced {
            for rule in &self.rules {
                let matches = regex_test(&rule.before_text, before_enter_text)
                    && optional_test(&rule.after_text, after_enter_text)
                    && optional_test(&rule.previous_line_text, previous_line_text);
                if matches {
                    return Some(rule.action.clone());
                }
            }
        }

        // (2) and (4): bracket pairs. The numbering skips (3) as upstream does.
        if auto_indent >= AutoIndentStrategy::Brackets && !before_enter_text.is_empty() {
            if !after_enter_text.is_empty()
                && self.brackets.iter().any(|bracket| {
                    regex_test(&bracket.open, before_enter_text)
                        && regex_test(&bracket.close, after_enter_text)
                })
            {
                return Some(EnterAction::new(IndentAction::IndentOutdent));
            }
            if self
                .brackets
                .iter()
                .any(|bracket| regex_test(&bracket.open, before_enter_text))
            {
                return Some(EnterAction::new(IndentAction::Indent));
            }
        }

        None
    }
}

/// Whether `regex` matches `text`, or `true` when there is no regex.
fn optional_test(regex: &Option<Regex>, text: &str) -> bool {
    regex.as_ref().is_none_or(|regex| regex_test(regex, text))
}

/// Whether `c` is a word character for `\b` in JavaScript (ASCII letters, digits, `_`).
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Matches `bracket` at the end of the text, followed only by whitespace. A word-character
/// bracket also needs a word boundary before it.
fn open_bracket_regex(bracket: &str) -> Regex {
    let mut pattern = String::new();
    if bracket.chars().next().is_some_and(is_word_char) {
        pattern.push_str(r"\b");
    }
    pattern.push_str(&fancy_regex::escape(bracket));
    pattern.push_str(r"\s*$");
    Regex::new(&pattern).expect("an escaped bracket is a valid pattern")
}

/// Matches `bracket` at the start of the text, after optional whitespace. A word-character
/// bracket also needs a word boundary after it.
fn close_bracket_regex(bracket: &str) -> Regex {
    let mut pattern = String::from(r"^\s*");
    pattern.push_str(&fancy_regex::escape(bracket));
    if bracket.chars().last().is_some_and(is_word_char) {
        pattern.push_str(r"\b");
    }
    Regex::new(&pattern).expect("an escaped bracket is a valid pattern")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn re(pattern: &str) -> Regex {
        Regex::new(pattern).expect("test pattern compiles")
    }

    fn rule(
        before: &str,
        after: Option<&str>,
        previous: Option<&str>,
        action: EnterAction,
    ) -> OnEnterRule {
        OnEnterRule {
            before_text: re(before),
            after_text: after.map(re),
            previous_line_text: previous.map(re),
            action,
        }
    }

    fn action(indent: IndentAction, append: Option<&str>, remove: Option<u32>) -> EnterAction {
        EnterAction {
            indent_action: indent,
            append_text: append.map(str::to_owned),
            remove_text: remove,
        }
    }

    /// Port of upstream's `javascriptOnEnterRules` (`onEnterRules.ts`).
    fn javascript_on_enter_rules() -> Vec<OnEnterRule> {
        vec![
            // e.g. /** | */
            rule(
                r"^\s*/\*\*(?!/)([^\*]|\*(?!/))*$",
                Some(r"^\s*\*/$"),
                None,
                action(IndentAction::IndentOutdent, Some(" * "), None),
            ),
            // e.g. /** ...|
            rule(
                r"^\s*/\*\*(?!/)([^\*]|\*(?!/))*$",
                None,
                None,
                action(IndentAction::None, Some(" * "), None),
            ),
            // e.g.  * ...|
            rule(
                r"^(\t|[ ])*[ ]\*([ ]([^\*]|\*(?!/))*)?$",
                None,
                Some(r"(?=^(\s*(/\*\*|\*)).*)(?=(?!(\s*\*/)))"),
                action(IndentAction::None, Some("* "), None),
            ),
            // e.g.  */|
            rule(
                r"^(\t|[ ])*[ ]\*/\s*$",
                None,
                None,
                action(IndentAction::None, None, Some(1)),
            ),
            // e.g.  *-----*/|
            rule(
                r"^(\t|[ ])*[ ]\*[^/]*\*/\s*$",
                None,
                None,
                action(IndentAction::None, None, Some(1)),
            ),
            rule(
                r"^\s*(\bcase\s.+:|\bdefault:)$",
                Some(r"^(?!\s*(\bcase\b|\bdefault\b))"),
                None,
                action(IndentAction::Indent, None, None),
            ),
            rule(
                r"^\s+([^{i\s]|i(?!f\b))",
                None,
                Some(r"^\s*(((else ?)?if|for|while)\s*\(.*\)\s*|else\s*)$"),
                action(IndentAction::Outdent, None, None),
            ),
            // Indent when pressing enter from inside ()
            rule(
                r"^.*\([^\)]*$",
                Some(r"^\s*\).*$"),
                None,
                action(IndentAction::IndentOutdent, Some("\t"), None),
            ),
            // Indent when pressing enter from inside {}
            rule(
                r"^.*\{[^\}]*$",
                Some(r"^\s*\}.*$"),
                None,
                action(IndentAction::IndentOutdent, Some("\t"), None),
            ),
            // Indent when pressing enter from inside []
            rule(
                r"^.*\[[^\]]*$",
                Some(r"^\s*\].*$"),
                None,
                action(IndentAction::IndentOutdent, Some("\t"), None),
            ),
        ]
    }

    /// Asserts the result of `on_enter` with the `Advanced` strategy. `None` expects no action.
    /// `append` and `remove` are checked only when given (upstream: non-null expectations).
    fn expect_enter(
        support: &OnEnterSupport,
        previous: &str,
        before: &str,
        after: &str,
        expected: Option<IndentAction>,
        append: Option<&str>,
        remove: u32,
    ) {
        let actual = support.on_enter(AutoIndentStrategy::Advanced, previous, before, after);
        let Some(indent) = expected else {
            assert!(actual.is_none(), "isNull: {before}");
            return;
        };
        let actual = actual.unwrap_or_else(|| panic!("isNotNull: {before}"));
        assert_eq!(actual.indent_action, indent, "indentAction: {before}");
        if let Some(append) = append {
            assert_eq!(
                actual.append_text.as_deref(),
                Some(append),
                "appendText: {before}"
            );
        }
        if remove != 0 {
            assert_eq!(actual.remove_text, Some(remove), "removeText: {before}");
        }
    }

    #[test]
    fn uses_brackets() {
        let support = OnEnterSupport::new(
            Some(vec![
                ("(".into(), ")".into()),
                ("begin".into(), "end".into()),
            ]),
            Vec::new(),
        );
        // `None` here means "no action", which is what upstream's IndentAction.None expects.
        let check = |before: &str, after: &str, expected: Option<IndentAction>| {
            expect_enter(&support, "", before, after, expected, None, 0);
        };

        check("a", "", None);
        check("", "b", None);
        check("(", "b", Some(IndentAction::Indent));
        check("a", ")", None);
        check("begin", "ending", Some(IndentAction::Indent));
        check("abegin", "end", None);
        check("begin", ")", Some(IndentAction::Indent));
        check("begin", "end", Some(IndentAction::IndentOutdent));
        check("begin ", " end", Some(IndentAction::IndentOutdent));
        check(" begin", "end//as", Some(IndentAction::IndentOutdent));
        check("(", ")", Some(IndentAction::IndentOutdent));
        check("( ", ")", Some(IndentAction::IndentOutdent));
        check("a(", ")b", Some(IndentAction::IndentOutdent));
        check("(", "", Some(IndentAction::Indent));
        check("(", "foo", Some(IndentAction::Indent));
        check("begin", "foo", Some(IndentAction::Indent));
        check("begin", "", Some(IndentAction::Indent));
    }

    #[test]
    fn issue_121125_on_enter_rules_with_global_modifier() {
        // Upstream uses `/^\s*\/{3}.*$/gm`; the `g` flag has no effect in Rust, `m` becomes `(?m)`.
        let support = OnEnterSupport::new(
            None,
            vec![rule(
                r"(?m)^\s*/{3}.*$",
                None,
                None,
                action(IndentAction::Outdent, Some("/// "), None),
            )],
        );
        expect_enter(
            &support,
            "/// line",
            "/// line",
            "",
            Some(IndentAction::Outdent),
            Some("/// "),
            0,
        );
        expect_enter(
            &support,
            "/// line",
            "/// line",
            "",
            Some(IndentAction::Outdent),
            Some("/// "),
            0,
        );
    }

    #[test]
    fn uses_regex_rules() {
        let support = OnEnterSupport::new(None, javascript_on_enter_rules());
        let check = |previous: &str,
                     before: &str,
                     after: &str,
                     expected: Option<IndentAction>,
                     append: Option<&str>,
                     remove: u32| {
            expect_enter(&support, previous, before, after, expected, append, remove);
        };

        check(
            "",
            "\t/**",
            " */",
            Some(IndentAction::IndentOutdent),
            Some(" * "),
            0,
        );
        check("", "\t/**", "", Some(IndentAction::None), Some(" * "), 0);
        check(
            "",
            "\t/** * / * / * /",
            "",
            Some(IndentAction::None),
            Some(" * "),
            0,
        );
        check("", "\t/** /*", "", Some(IndentAction::None), Some(" * "), 0);
        check("", "/**", "", Some(IndentAction::None), Some(" * "), 0);
        check("", "\t/**/", "", None, None, 0);
        check("", "\t/***/", "", None, None, 0);
        check("", "\t/*******/", "", None, None, 0);
        check("", "\t/** * * * * */", "", None, None, 0);
        check("", "\t/** */", "", None, None, 0);
        check("", "\t/** asdfg */", "", None, None, 0);
        check("", "\t/* asdfg */", "", None, None, 0);
        check("", "\t/* asdfg */", "", None, None, 0);
        check("", "\t/** asdfg */", "", None, None, 0);
        check("", "*/", "", None, None, 0);
        check("", "\t/*", "", None, None, 0);
        check("", "\t*", "", None, None, 0);
        check("\t/**", "\t *", "", Some(IndentAction::None), Some("* "), 0);
        check(
            "\t * something",
            "\t *",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check("\t *", "\t *", "", Some(IndentAction::None), Some("* "), 0);
        check("", "\t */", "", Some(IndentAction::None), None, 1);
        check("", "\t * */", "", Some(IndentAction::None), None, 1);
        check("", "\t * * / * / * / */", "", None, None, 0);
        check(
            "\t/**",
            "\t * ",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            "\t * something",
            "\t * ",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check("\t *", "\t * ", "", Some(IndentAction::None), Some("* "), 0);
        check("/**", " * ", "", Some(IndentAction::None), Some("* "), 0);
        check(
            " * something",
            " * ",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            " *",
            " * asdfsfagadfg",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            "/**",
            " * asdfsfagadfg * * * ",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            " * something",
            " * asdfsfagadfg * * * ",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            " *",
            " * asdfsfagadfg * * * ",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check("/**", " * /*", "", Some(IndentAction::None), Some("* "), 0);
        check(
            " * something",
            " * /*",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(" *", " * /*", "", Some(IndentAction::None), Some("* "), 0);
        check(
            "/**",
            " * asdfsfagadfg * / * / * /",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            " * something",
            " * asdfsfagadfg * / * / * /",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            " *",
            " * asdfsfagadfg * / * / * /",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            "/**",
            " * asdfsfagadfg * / * / * /*",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            " * something",
            " * asdfsfagadfg * / * / * /*",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check(
            " *",
            " * asdfsfagadfg * / * / * /*",
            "",
            Some(IndentAction::None),
            Some("* "),
            0,
        );
        check("", " */", "", Some(IndentAction::None), None, 1);
        check(
            " */",
            " * test() {",
            "",
            Some(IndentAction::Indent),
            None,
            0,
        );
        check("", "\t */", "", Some(IndentAction::None), None, 1);
        check("", "\t\t */", "", Some(IndentAction::None), None, 1);
        check("", "   */", "", Some(IndentAction::None), None, 1);
        check("", "     */", "", Some(IndentAction::None), None, 1);
        check("", "\t     */", "", Some(IndentAction::None), None, 1);
        check(
            "",
            " *--------------------------------------------------------------------------------------------*/",
            "",
            Some(IndentAction::None),
            None,
            1,
        );
        // issue #43469
        check(
            "class A {",
            "    * test() {",
            "",
            Some(IndentAction::Indent),
            None,
            0,
        );
        check(
            "",
            "    * test() {",
            "",
            Some(IndentAction::Indent),
            None,
            0,
        );
        check(
            "    ",
            "    * test() {",
            "",
            Some(IndentAction::Indent),
            None,
            0,
        );
        check(
            "class A {",
            "  * test() {",
            "",
            Some(IndentAction::Indent),
            None,
            0,
        );
        check("", "  * test() {", "", Some(IndentAction::Indent), None, 0);
        check(
            "  ",
            "  * test() {",
            "",
            Some(IndentAction::Indent),
            None,
            0,
        );
    }

    #[test]
    fn issue_141816_braces_inside_regex_literals_do_not_indent() {
        let support = OnEnterSupport::new(None, javascript_on_enter_rules());
        // The brace belongs to a regex literal on the line; there is nothing after the cursor.
        for before in [
            "const r = /{/;",
            "const r = /{[0-9]/;",
            "const r = /[a-zA-Z]{/;",
        ] {
            expect_enter(&support, "", before, "", None, None, 0);
        }
    }

    #[test]
    fn auto_indent_strategy_gates_each_step() {
        let support = OnEnterSupport::new(None, javascript_on_enter_rules());
        // `/** | */` is handled by rule 1, which only runs at `Advanced` and above.
        let indent = |strategy| {
            support
                .on_enter(strategy, "", "\t/**", " */")
                .map(|a| a.indent_action)
        };
        assert_eq!(
            indent(AutoIndentStrategy::Advanced),
            Some(IndentAction::IndentOutdent)
        );
        assert_eq!(indent(AutoIndentStrategy::Brackets), None);
        // Below `Brackets` nothing runs, even for a bracket pair.
        assert!(
            support
                .on_enter(AutoIndentStrategy::Keep, "", "(", ")")
                .is_none()
        );
    }
}

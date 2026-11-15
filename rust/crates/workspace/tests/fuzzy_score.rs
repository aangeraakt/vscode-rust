//! Port of the `fuzzyScore` suites of `src/vs/base/test/common/filters.test.ts` (the
//! `assertMatches` and `assertTopScore` calls are generated from the upstream assertions).

use lathe_workspace::filters::{self as f, FuzzyScoreOptions, FuzzyScorer};
use lathe_workspace::strings::{to_lower_utf16, to_utf16};

/// `assertMatches`; `opts` is `(patternPos, wordPos, firstMatchCanBeWeak)`.
fn assert_matches(
    pattern: &str,
    word: &str,
    decorated_word: Option<&str>,
    filter: FuzzyScorer,
    opts: (usize, usize, bool),
) {
    let (p, w) = (to_utf16(pattern), to_utf16(word));
    let options = FuzzyScoreOptions {
        first_match_can_be_weak: opts.2,
        boost_full_match: true,
    };
    let r = filter(
        &p,
        &to_lower_utf16(&p),
        opts.0,
        &w,
        &to_lower_utf16(&w),
        opts.1,
        options,
    );
    assert_eq!(
        decorated_word.is_none(),
        r.is_none(),
        "{pattern} against {word}: {r:?}"
    );
    if let Some(r) = r {
        let mut actual: Vec<u16> = Vec::new();
        let mut pos = 0;
        for m in f::create_matches(Some(&r)) {
            actual.extend_from_slice(&w[pos..m.start]);
            for &c in &w[m.start..m.end] {
                actual.extend_from_slice(&[u16::from(b'^'), c]);
            }
            pos = m.end;
        }
        actual.extend_from_slice(&w[pos..]);
        assert_eq!(
            String::from_utf16_lossy(&actual),
            decorated_word.unwrap_or_default()
        );
    }
}

/// `assertTopScore`.
fn assert_top_score(filter: FuzzyScorer, pattern: &str, expected: usize, words: &[&str]) {
    let p = to_utf16(pattern);
    let mut top_score = -(100 * 10);
    let mut top_idx = 0;
    for (i, word) in words.iter().enumerate() {
        let w = to_utf16(word);
        let options = FuzzyScoreOptions::default();
        if let Some(m) = filter(
            &p,
            &to_lower_utf16(&p),
            0,
            &w,
            &to_lower_utf16(&w),
            0,
            options,
        ) {
            if m.score > top_score {
                top_score = m.score;
                top_idx = i;
            }
        }
    }
    assert_eq!(
        top_idx, expected,
        "{pattern} -> actual={} <> expected={}",
        words[top_idx], words[expected]
    );
}

#[test]
fn cannot_set_property_1_of_undefined_26511() {
    let word = "a".repeat(122);
    let pattern = "a".repeat(119);
    f::fuzzy_score_str(&pattern, 0, &word, 0, FuzzyScoreOptions::default()); // Must not explode.
}

#[test]
fn go_to_symbol_with_the_exact_method_name_doesn_t_work_as_expected_84787() {
    let options = FuzzyScoreOptions {
        first_match_can_be_weak: true,
        boost_full_match: true,
    };
    let (p, w) = (to_utf16(":get"), to_utf16("get"));
    assert!(f::fuzzy_score(&p, &p, 1, &w, &w, 0, options).is_some());
}

#[test]
fn configurable_full_match_boost() {
    let (prefix, a, b) = ("create", "createModelServices", "create");
    let boost = FuzzyScoreOptions {
        boost_full_match: true,
        first_match_can_be_weak: true,
    };
    let score = |word: &str, start: usize, options| {
        f::fuzzy_score_str(prefix, 0, word, start, options)
            .expect("matches")
            .score
    };
    assert!(score(a, 0, boost) < score(b, 0, boost));
    // Also with wordStart > 0 (upstream issue #187921).
    let word_prefix = "$(symbol-function) ";
    let len = word_prefix.len();
    assert!(
        score(&format!("{word_prefix}{a}"), len, boost)
            < score(&format!("{word_prefix}{b}"), len, boost)
    );
    let no_boost = FuzzyScoreOptions {
        boost_full_match: false,
        first_match_can_be_weak: true,
    };
    assert_eq!(score(a, 0, no_boost), score(b, 0, no_boost));
}

#[test]
fn matches_fuzzy2_and_default_score() {
    // Not upstream tests.
    assert_eq!(
        f::matches_fuzzy2("ccm", "cacmelCase"),
        Some(vec![f::Match::new(0, 1), f::Match::new(2, 4)])
    );
    assert!(f::FuzzyScore::is_default(None));
    assert!(f::FuzzyScore::is_default(Some(
        &f::FuzzyScore::default_score()
    )));
}

#[rustfmt::skip]
#[test]
fn fuzzy_score_23215() {
    assert_matches("tit", "win.tit", Some("win.^t^i^t"), f::fuzzy_score, (0, 0, false));
    assert_matches("title", "win.title", Some("win.^t^i^t^l^e"), f::fuzzy_score, (0, 0, false));
    assert_matches("WordCla", "WordCharacterClassifier", Some("^W^o^r^dCharacter^C^l^assifier"), f::fuzzy_score, (0, 0, false));
    assert_matches("WordCCla", "WordCharacterClassifier", Some("^W^o^r^d^Character^C^l^assifier"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn fuzzy_score_23332() {
    assert_matches("dete", "\"editor.quickSuggestionsDelay\"", None, f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn fuzzy_score_23190() {
    assert_matches("c:\\do", "& 'C:\\Documents and Settings'", Some("& '^C^:^\\^D^ocuments and Settings'"), f::fuzzy_score, (0, 0, false));
    assert_matches("c:\\do", "& 'c:\\Documents and Settings'", Some("& '^c^:^\\^D^ocuments and Settings'"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn fuzzy_score_23581() {
    assert_matches("close", "css.lint.importStatement", Some("^css.^lint.imp^ort^Stat^ement"), f::fuzzy_score, (0, 0, false));
    assert_matches("close", "css.colorDecorators.enable", Some("^css.co^l^orDecorator^s.^enable"), f::fuzzy_score, (0, 0, false));
    assert_matches("close", "workbench.quickOpen.closeOnFocusOut", Some("workbench.quickOpen.^c^l^o^s^eOnFocusOut"), f::fuzzy_score, (0, 0, false));
    assert_top_score(f::fuzzy_score, "close", 2, &["css.lint.importStatement", "css.colorDecorators.enable", "workbench.quickOpen.closeOnFocusOut"]);
}

#[rustfmt::skip]
#[test]
fn fuzzy_score_23458() {
    assert_matches("highlight", "editorHoverHighlight", Some("editorHover^H^i^g^h^l^i^g^h^t"), f::fuzzy_score, (0, 0, false));
    assert_matches("hhighlight", "editorHoverHighlight", Some("editor^Hover^H^i^g^h^l^i^g^h^t"), f::fuzzy_score, (0, 0, false));
    assert_matches("dhhighlight", "editorHoverHighlight", None, f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn fuzzy_score_23746() {
    assert_matches("-moz", "-moz-foo", Some("^-^m^o^z-foo"), f::fuzzy_score, (0, 0, false));
    assert_matches("moz", "-moz-foo", Some("-^m^o^z-foo"), f::fuzzy_score, (0, 0, false));
    assert_matches("moz", "-moz-animation", Some("-^m^o^z-animation"), f::fuzzy_score, (0, 0, false));
    assert_matches("moza", "-moz-animation", Some("-^m^o^z-^animation"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn fuzzy_score() {
    assert_matches("ab", "abA", Some("^a^bA"), f::fuzzy_score, (0, 0, false));
    assert_matches("ccm", "cacmelCase", Some("^ca^c^melCase"), f::fuzzy_score, (0, 0, false));
    assert_matches("bti", "the_black_knight", None, f::fuzzy_score, (0, 0, false));
    assert_matches("ccm", "camelCase", None, f::fuzzy_score, (0, 0, false));
    assert_matches("cmcm", "camelCase", None, f::fuzzy_score, (0, 0, false));
    assert_matches("BK", "the_black_knight", Some("the_^black_^knight"), f::fuzzy_score, (0, 0, false));
    assert_matches("KeyboardLayout=", "KeyboardLayout", None, f::fuzzy_score, (0, 0, false));
    assert_matches("LLL", "SVisualLoggerLogsList", Some("SVisual^Logger^Logs^List"), f::fuzzy_score, (0, 0, false));
    assert_matches("LLLL", "SVilLoLosLi", None, f::fuzzy_score, (0, 0, false));
    assert_matches("LLLL", "SVisualLoggerLogsList", None, f::fuzzy_score, (0, 0, false));
    assert_matches("TEdit", "TextEdit", Some("^Text^E^d^i^t"), f::fuzzy_score, (0, 0, false));
    assert_matches("TEdit", "TextEditor", Some("^Text^E^d^i^tor"), f::fuzzy_score, (0, 0, false));
    assert_matches("TEdit", "Textedit", Some("^Text^e^d^i^t"), f::fuzzy_score, (0, 0, false));
    assert_matches("TEdit", "text_edit", Some("^text_^e^d^i^t"), f::fuzzy_score, (0, 0, false));
    assert_matches("TEditDit", "TextEditorDecorationType", Some("^Text^E^d^i^tor^Decorat^ion^Type"), f::fuzzy_score, (0, 0, false));
    assert_matches("TEdit", "TextEditorDecorationType", Some("^Text^E^d^i^torDecorationType"), f::fuzzy_score, (0, 0, false));
    assert_matches("Tedit", "TextEdit", Some("^Text^E^d^i^t"), f::fuzzy_score, (0, 0, false));
    assert_matches("ba", "?AB?", None, f::fuzzy_score, (0, 0, false));
    assert_matches("bkn", "the_black_knight", Some("the_^black_^k^night"), f::fuzzy_score, (0, 0, false));
    assert_matches("bt", "the_black_knight", Some("the_^black_knigh^t"), f::fuzzy_score, (0, 0, false));
    assert_matches("ccm", "camelCasecm", Some("^camel^Casec^m"), f::fuzzy_score, (0, 0, false));
    assert_matches("fdm", "findModel", Some("^fin^d^Model"), f::fuzzy_score, (0, 0, false));
    assert_matches("fob", "foobar", Some("^f^oo^bar"), f::fuzzy_score, (0, 0, false));
    assert_matches("fobz", "foobar", None, f::fuzzy_score, (0, 0, false));
    assert_matches("foobar", "foobar", Some("^f^o^o^b^a^r"), f::fuzzy_score, (0, 0, false));
    assert_matches("form", "editor.formatOnSave", Some("editor.^f^o^r^matOnSave"), f::fuzzy_score, (0, 0, false));
    assert_matches("g p", "Git: Pull", Some("^Git:^ ^Pull"), f::fuzzy_score, (0, 0, false));
    assert_matches("g p", "Git: Pull", Some("^Git:^ ^Pull"), f::fuzzy_score, (0, 0, false));
    assert_matches("gip", "Git: Pull", Some("^G^it: ^Pull"), f::fuzzy_score, (0, 0, false));
    assert_matches("gip", "Git: Pull", Some("^G^it: ^Pull"), f::fuzzy_score, (0, 0, false));
    assert_matches("gp", "Git: Pull", Some("^Git: ^Pull"), f::fuzzy_score, (0, 0, false));
    assert_matches("gp", "Git_Git_Pull", Some("^Git_Git_^Pull"), f::fuzzy_score, (0, 0, false));
    assert_matches("is", "ImportStatement", Some("^Import^Statement"), f::fuzzy_score, (0, 0, false));
    assert_matches("is", "isValid", Some("^i^sValid"), f::fuzzy_score, (0, 0, false));
    assert_matches("lowrd", "lowWord", Some("^l^o^wWo^r^d"), f::fuzzy_score, (0, 0, false));
    assert_matches("myvable", "myvariable", Some("^m^y^v^aria^b^l^e"), f::fuzzy_score, (0, 0, false));
    assert_matches("no", "", None, f::fuzzy_score, (0, 0, false));
    assert_matches("no", "match", None, f::fuzzy_score, (0, 0, false));
    assert_matches("ob", "foobar", None, f::fuzzy_score, (0, 0, false));
    assert_matches("sl", "SVisualLoggerLogsList", Some("^SVisual^LoggerLogsList"), f::fuzzy_score, (0, 0, false));
    assert_matches("sllll", "SVisualLoggerLogsList", Some("^SVisua^l^Logger^Logs^List"), f::fuzzy_score, (0, 0, false));
    assert_matches("Three", "HTMLHRElement", None, f::fuzzy_score, (0, 0, false));
    assert_matches("Three", "Three", Some("^T^h^r^e^e"), f::fuzzy_score, (0, 0, false));
    assert_matches("fo", "barfoo", None, f::fuzzy_score, (0, 0, false));
    assert_matches("fo", "bar_foo", Some("bar_^f^oo"), f::fuzzy_score, (0, 0, false));
    assert_matches("fo", "bar_Foo", Some("bar_^F^oo"), f::fuzzy_score, (0, 0, false));
    assert_matches("fo", "bar foo", Some("bar ^f^oo"), f::fuzzy_score, (0, 0, false));
    assert_matches("fo", "bar.foo", Some("bar.^f^oo"), f::fuzzy_score, (0, 0, false));
    assert_matches("fo", "bar/foo", Some("bar/^f^oo"), f::fuzzy_score, (0, 0, false));
    assert_matches("fo", "bar\\foo", Some("bar\\^f^oo"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn fuzzy_score_first_match_can_be_weak() {
    assert_matches("Three", "HTMLHRElement", Some("H^TML^H^R^El^ement"), f::fuzzy_score, (0, 0, true));
    assert_matches("tor", "constructor", Some("construc^t^o^r"), f::fuzzy_score, (0, 0, true));
    assert_matches("ur", "constructor", Some("constr^ucto^r"), f::fuzzy_score, (0, 0, true));
    assert_top_score(f::fuzzy_score, "tor", 2, &["constructor", "Thor", "cTor"]);
}

#[rustfmt::skip]
#[test]
fn fuzzy_score_many_matches() {
    assert_matches("aaaaaa", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", Some("^a^a^a^a^a^aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn freeze_when_fjfj_jfjf_https_github_com_microsoft_vscode_issues_91807() {
    assert_matches("jfjfj", "fjfjfjfjfjfjfjfjfjfjfj", None, f::fuzzy_score, (0, 0, false));
    assert_matches("jfjfjfjfjfjfjfjfjfj", "fjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfj", None, f::fuzzy_score, (0, 0, false));
    assert_matches("jfjfjfjfjfjfjfjfjfjjfjfjfjfjfjfjfjfjfjjfjfjfjfjfjfjfjfjfjjfjfjfjfjfjfjfjfjfjjfjfjfjfjfjfjfjfjfjjfjfjfjfjfjfjfjfjfj", "fjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfj", None, f::fuzzy_score, (0, 0, false));
    assert_matches("jfjfjfjfjfjfjfjfjfj", "fJfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfj", Some("f^J^f^j^f^j^f^j^f^j^f^j^f^j^f^j^f^j^f^jfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfj"), f::fuzzy_score, (0, 0, false));
    assert_matches("jfjfjfjfjfjfjfjfjfj", "fjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfj", Some("f^j^f^j^f^j^f^j^f^j^f^j^f^j^f^j^f^j^f^jfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfjfj"), f::fuzzy_score, (0, 0, true));
}

#[rustfmt::skip]
#[test]
fn fuzzy_score_issue_26423() {
    assert_matches("baba", "abababab", None, f::fuzzy_score, (0, 0, false));
    assert_matches("fsfsfs", "dsafdsafdsafdsafdsafdsafdsafasdfdsa", None, f::fuzzy_score, (0, 0, false));
    assert_matches("fsfsfsfsfsfsfsf", "dsafdsafdsafdsafdsafdsafdsafasdfdsafdsafdsafdsafdsfdsafdsfdfdfasdnfdsajfndsjnafjndsajlknfdsa", None, f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn fuzzy_intelli_sense_matching_vs_haxe_metadata_completion_26995() {
    assert_matches("f", ":Foo", Some(":^Foo"), f::fuzzy_score, (0, 0, false));
    assert_matches("f", ":foo", Some(":^foo"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn separator_only_match_should_not_be_weak_79558() {
    assert_matches(".", "foo.bar", Some("foo^.bar"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn vscode_1_12_no_longer_obeys_sort_text_in_completion_items_from_language_server_26096() {
    assert_matches("  ", "  group", None, f::fuzzy_score, (2, 0, false));
    assert_matches("  g", "  group", Some("  ^group"), f::fuzzy_score, (2, 0, false));
    assert_matches("g", "  group", Some("  ^group"), f::fuzzy_score, (0, 0, false));
    assert_matches("g g", "  groupGroup", None, f::fuzzy_score, (0, 0, false));
    assert_matches("g g", "  group Group", Some("  ^group^ ^Group"), f::fuzzy_score, (0, 0, false));
    assert_matches(" g g", "  group Group", Some("  ^group^ ^Group"), f::fuzzy_score, (1, 0, false));
    assert_matches("zz", "zzGroup", Some("^z^zGroup"), f::fuzzy_score, (0, 0, false));
    assert_matches("zzg", "zzGroup", Some("^z^z^Group"), f::fuzzy_score, (0, 0, false));
    assert_matches("g", "zzGroup", Some("zz^Group"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn pattern_pos_isn_t_working_correctly_79815() {
    assert_matches("p", "prop", Some("^prop"), f::fuzzy_score, (0, 0, false));
    assert_matches(":p", "prop", Some("^prop"), f::fuzzy_score, (1, 0, false));
    assert_matches(":p", "prop", None, f::fuzzy_score, (2, 0, false));
    assert_matches(":p", "proP", Some("pro^P"), f::fuzzy_score, (1, 1, false));
    assert_matches(":p", "aprop", Some("a^prop"), f::fuzzy_score, (1, 0, true));
    assert_matches(":p", "aprop", None, f::fuzzy_score, (1, 0, false));
}

#[rustfmt::skip]
#[test]
fn top_score_fuzzy_score() {
    assert_top_score(f::fuzzy_score, "cons", 2, &["ArrayBufferConstructor", "Console", "console"]);
    assert_top_score(f::fuzzy_score, "Foo", 1, &["foo", "Foo", "foo"]);
    assert_top_score(f::fuzzy_score, "onMess", 1, &["onmessage", "onMessage", "onThisMegaEscape"]);
    assert_top_score(f::fuzzy_score, "CC", 1, &["camelCase", "CamelCase"]);
    assert_top_score(f::fuzzy_score, "cC", 0, &["camelCase", "CamelCase"]);
    assert_top_score(f::fuzzy_score, "p", 4, &["parse", "posix", "pafdsa", "path", "p"]);
    assert_top_score(f::fuzzy_score, "pa", 0, &["parse", "pafdsa", "path"]);
    assert_top_score(f::fuzzy_score, "log", 3, &["HTMLOptGroupElement", "ScrollLogicalPosition", "SVGFEMorphologyElement", "log", "logger"]);
    assert_top_score(f::fuzzy_score, "e", 2, &["AbstractWorker", "ActiveXObject", "else"]);
    assert_top_score(f::fuzzy_score, "workbench.sideb", 1, &["workbench.editor.defaultSideBySideLayout", "workbench.sideBar.location"]);
    assert_top_score(f::fuzzy_score, "editor.r", 2, &["diffEditor.renderSideBySide", "editor.overviewRulerlanes", "editor.renderControlCharacter", "editor.renderWhitespace"]);
    assert_top_score(f::fuzzy_score, "-mo", 1, &["-ms-ime-mode", "-moz-columns"]);
    assert_top_score(f::fuzzy_score, "convertModelPosition", 0, &["convertModelPositionToViewPosition", "convertViewToModelPosition"]);
    assert_top_score(f::fuzzy_score, "is", 0, &["isValidViewletId", "import statement"]);
    assert_top_score(f::fuzzy_score, "title", 1, &["files.trimTrailingWhitespace", "window.title"]);
    assert_top_score(f::fuzzy_score, "const", 1, &["constructor", "const", "cuOnstrul"]);
}

#[rustfmt::skip]
#[test]
fn unexpected_suggestion_scoring_28791() {
    assert_top_score(f::fuzzy_score, "_lines", 1, &["_lineStarts", "_lines"]);
    assert_top_score(f::fuzzy_score, "_lines", 1, &["_lineS", "_lines"]);
    assert_top_score(f::fuzzy_score, "_lineS", 0, &["_lineS", "_lines"]);
}

#[rustfmt::skip]
#[test]
fn html_closing_tag_proposal_filtered_out_38880() {
    assert_matches("\t\t<", "\t\t</body>", Some("^\t^\t^</body>"), f::fuzzy_score, (0, 0, false));
    assert_matches("\t\t<", "\t\t</body>", Some("\t\t^</body>"), f::fuzzy_score, (2, 0, false));
    assert_matches("\t<", "\t</body>", Some("\t^</body>"), f::fuzzy_score, (1, 0, false));
}

#[rustfmt::skip]
#[test]
fn list_highlight_filter_not_all_characters_from_match_are_highlighterd_66923() {
    assert_matches("foo", "barbarbarbarbarbarbarbarbarbarbarbarbarbarbarbar_foo", Some("barbarbarbarbarbarbarbarbarbarbarbarbarbarbarbar_^f^o^o"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn autocompletion_is_matched_against_truncated_filter_text_to_54_characters_74133() {
    assert_matches("foo", "ffffffffffffffffffffffffffffbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbar_foo", Some("ffffffffffffffffffffffffffffbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbar_^f^o^o"), f::fuzzy_score, (0, 0, false));
    assert_matches("Aoo", "Affffffffffffffffffffffffffffbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbar_foo", Some("^Affffffffffffffffffffffffffffbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbar_f^o^o"), f::fuzzy_score, (0, 0, false));
    assert_matches("foo", "Gffffffffffffffffffffffffffffbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbarbar_foo", None, f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn wrong_highlight_after_emoji_113404() {
    assert_matches("di", "✨div classname=\"\"></div>", Some("✨^d^iv classname=\"\"></div>"), f::fuzzy_score, (0, 0, false));
    assert_matches("di", "adiv classname=\"\"></div>", Some("adiv classname=\"\"></^d^iv>"), f::fuzzy_score, (0, 0, false));
}

#[rustfmt::skip]
#[test]
fn unexpected_suggest_highlighting_ignores_whole_word_match_in_favor_of_matching_first_letter_147423() {
    assert_matches("i", "machine/{id}", Some("machine/{^id}"), f::fuzzy_score, (0, 0, false));
    assert_matches("ok", "obobobf{ok}/user", Some("^obobobf{o^k}/user"), f::fuzzy_score, (0, 0, false));
}

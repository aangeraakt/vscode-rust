//! Port of the matcher suites of `src/vs/base/test/common/filters.test.ts` (the `filterOk` and
//! `filterNotOk` suites are generated from the upstream assertions). Highlights are UTF-16 ranges.

use std::cell::Cell;

use lathe_workspace::filters::{self as f, Filter, Match};

fn ok(filter: &Filter<'_>, word: &str, target: &str, highlights: Option<&[(usize, usize)]>) {
    let r = filter(word, target);
    assert!(r.is_some(), "{word} didn't match {target}");
    if let Some(highlights) = highlights {
        let expected: Vec<Match> = highlights.iter().map(|&(s, e)| Match::new(s, e)).collect();
        assert_eq!(r.unwrap(), expected, "{word} against {target}");
    }
}

fn not_ok(filter: &Filter<'_>, word: &str, target: &str) {
    assert!(filter(word, target).is_none(), "{word} matched {target}");
}

fn words(word: &str, target: &str) -> Option<Vec<Match>> {
    f::matches_words(word, target, false)
}

#[test]
fn empty_queries_share_immutable_matches() {
    // Upstream also checks that the shared array is frozen; Rust returns owned vectors.
    let fuzzy = |w: &str, t: &str| f::matches_fuzzy(w, t, false);
    let filters: [&Filter<'_>; 3] = [
        &fuzzy,
        &f::matches_contiguous_sub_string,
        &f::matches_base_contiguous_sub_string,
    ];
    for filter in filters {
        assert_eq!(filter("", "first"), Some(f::empty_matches()));
        assert_eq!(filter("", ""), Some(vec![Match::new(0, 0)]));
    }
}

#[test]
fn or() {
    let counters = [Cell::new(0), Cell::new(0)];
    let new_filter = |i: usize, r: bool| {
        let counters = &counters;
        move |_: &str, _: &str| {
            counters[i].set(counters[i].get() + 1);
            r.then(Vec::new)
        }
    };
    for (a, b, matches, expected) in [
        (false, false, false, [1, 1]),
        (true, false, true, [1, 0]),
        (true, true, true, [1, 0]),
    ]
    .into_iter()
    .chain([(false, true, true, [1, 1])])
    {
        counters.iter().for_each(|c| c.set(0));
        let (fa, fb) = (new_filter(0, a), new_filter(1, b));
        let filter = f::or(vec![&fa, &fb]);
        assert_eq!(filter("anything", "anything").is_some(), matches);
        assert_eq!([counters[0].get(), counters[1].get()], expected);
    }
}

#[test]
fn matches_words_performance_309582() {
    let targets = [
        "workbench.action.terminal.focusNextLine",
        "editor.action.clipboardCopyAction",
        "workbench.action.editor.changeLanguageMode",
        "editor.action.smartSelect.expand",
        "workbench.action.files.saveAll",
    ];
    for _ in 0..1000 {
        for t in targets {
            f::matches_words("editor.action", t, false);
        }
    }
}

#[test]
fn matches_fuzzy_wildcards() {
    // Not an upstream test: the `*` wildcard path of `matchesFuzzy`.
    assert_eq!(
        f::matches_fuzzy("a*c", "xAbbcd", false),
        Some(vec![Match::new(1, 5)])
    );
    assert_eq!(
        f::matches_fuzzy("a*", "xab\ncd", false),
        Some(vec![Match::new(1, 3)])
    );
    assert_eq!(
        f::matches_fuzzy("b*b", "abcbdb", false),
        Some(vec![Match::new(1, 6)])
    );
    assert_eq!(
        f::matches_fuzzy("cela", "cancelAnimation", false),
        Some(vec![Match::new(3, 7)])
    );
    assert_eq!(
        f::matches_fuzzy("caf", "CamelAndFoo", false),
        Some(vec![Match::new(0, 2), Match::new(8, 9)])
    );
}

#[rustfmt::skip]
#[test]
fn prefix_filter_case_sensitive() {
    not_ok(&f::matches_strict_prefix, "", "");
    ok(&f::matches_strict_prefix, "", "anything", Some(&[]));
    ok(&f::matches_strict_prefix, "alpha", "alpha", Some(&[(0, 5)]));
    ok(&f::matches_strict_prefix, "alpha", "alphasomething", Some(&[(0, 5)]));
    not_ok(&f::matches_strict_prefix, "alpha", "alp");
    ok(&f::matches_strict_prefix, "a", "alpha", Some(&[(0, 1)]));
    not_ok(&f::matches_strict_prefix, "x", "alpha");
    not_ok(&f::matches_strict_prefix, "A", "alpha");
    not_ok(&f::matches_strict_prefix, "AlPh", "alPHA");
}

#[rustfmt::skip]
#[test]
fn prefix_filter_ignore_case() {
    ok(&f::matches_prefix, "alpha", "alpha", Some(&[(0, 5)]));
    ok(&f::matches_prefix, "alpha", "alphasomething", Some(&[(0, 5)]));
    not_ok(&f::matches_prefix, "alpha", "alp");
    ok(&f::matches_prefix, "a", "alpha", Some(&[(0, 1)]));
    ok(&f::matches_prefix, "ä", "Älpha", Some(&[(0, 1)]));
    not_ok(&f::matches_prefix, "x", "alpha");
    ok(&f::matches_prefix, "A", "alpha", Some(&[(0, 1)]));
    ok(&f::matches_prefix, "AlPh", "alPHA", Some(&[(0, 4)]));
    not_ok(&f::matches_prefix, "T", "4");
}

#[rustfmt::skip]
#[test]
fn camel_case_filter() {
    not_ok(&f::matches_camel_case, "", "");
    ok(&f::matches_camel_case, "", "anything", Some(&[]));
    ok(&f::matches_camel_case, "alpha", "alpha", Some(&[(0, 5)]));
    ok(&f::matches_camel_case, "AlPhA", "alpha", Some(&[(0, 5)]));
    ok(&f::matches_camel_case, "alpha", "alphasomething", Some(&[(0, 5)]));
    not_ok(&f::matches_camel_case, "alpha", "alp");
    ok(&f::matches_camel_case, "c", "CamelCaseRocks", Some(&[(0, 1)]));
    ok(&f::matches_camel_case, "cc", "CamelCaseRocks", Some(&[(0, 1), (5, 6)]));
    ok(&f::matches_camel_case, "ccr", "CamelCaseRocks", Some(&[(0, 1), (5, 6), (9, 10)]));
    ok(&f::matches_camel_case, "cacr", "CamelCaseRocks", Some(&[(0, 2), (5, 6), (9, 10)]));
    ok(&f::matches_camel_case, "cacar", "CamelCaseRocks", Some(&[(0, 2), (5, 7), (9, 10)]));
    ok(&f::matches_camel_case, "ccarocks", "CamelCaseRocks", Some(&[(0, 1), (5, 7), (9, 14)]));
    ok(&f::matches_camel_case, "cr", "CamelCaseRocks", Some(&[(0, 1), (9, 10)]));
    ok(&f::matches_camel_case, "fba", "FooBarAbe", Some(&[(0, 1), (3, 5)]));
    ok(&f::matches_camel_case, "fbar", "FooBarAbe", Some(&[(0, 1), (3, 6)]));
    ok(&f::matches_camel_case, "fbara", "FooBarAbe", Some(&[(0, 1), (3, 7)]));
    ok(&f::matches_camel_case, "fbaa", "FooBarAbe", Some(&[(0, 1), (3, 5), (6, 7)]));
    ok(&f::matches_camel_case, "fbaab", "FooBarAbe", Some(&[(0, 1), (3, 5), (6, 8)]));
    ok(&f::matches_camel_case, "c2d", "canvasCreation2D", Some(&[(0, 1), (14, 16)]));
    ok(&f::matches_camel_case, "cce", "_canvasCreationEvent", Some(&[(1, 2), (7, 8), (15, 16)]));
}

#[rustfmt::skip]
#[test]
fn camel_case_filter_19256() {
    ok(&f::matches_camel_case, "Debug Console", "Open: Debug Console", None);
    ok(&f::matches_camel_case, "Debug console", "Open: Debug Console", None);
    ok(&f::matches_camel_case, "debug console", "Open: Debug Console", None);
}

#[rustfmt::skip]
#[test]
fn matches_contiguous_sub_string() {
    ok(&f::matches_contiguous_sub_string, "cela", "cancelAnimationFrame()", Some(&[(3, 7)]));
}

#[rustfmt::skip]
#[test]
fn matches_base_contiguous_sub_string() {
    ok(&f::matches_base_contiguous_sub_string, "cela", "cancelAnimationFrame()", Some(&[(3, 7)]));
    ok(&f::matches_base_contiguous_sub_string, "cafe", "café", Some(&[(0, 4)]));
    ok(&f::matches_base_contiguous_sub_string, "cafe", "caféBar", Some(&[(0, 4)]));
    ok(&f::matches_base_contiguous_sub_string, "resume", "résumé", Some(&[(0, 6)]));
    ok(&f::matches_base_contiguous_sub_string, "naïve", "naïve", Some(&[(0, 5)]));
    ok(&f::matches_base_contiguous_sub_string, "naive", "naïve", Some(&[(0, 5)]));
    ok(&f::matches_base_contiguous_sub_string, "aeou", "àéöü", Some(&[(0, 4)]));
}

#[rustfmt::skip]
#[test]
fn matches_sub_string() {
    ok(&f::matches_sub_string, "cmm", "cancelAnimationFrame()", Some(&[(0, 1), (9, 10), (18, 19)]));
    ok(&f::matches_sub_string, "abc", "abcabc", Some(&[(0, 3)]));
    ok(&f::matches_sub_string, "abc", "aaabbbccc", Some(&[(0, 1), (3, 4), (6, 7)]));
}

#[rustfmt::skip]
#[test]
fn matches_sub_string_performance_35346() {
    not_ok(&f::matches_sub_string, "aaaaaaaaaaaaaaaaaaaax", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
}

#[rustfmt::skip]
#[test]
fn word_filter() {
    ok(&words, "alpha", "alpha", Some(&[(0, 5)]));
    ok(&words, "alpha", "alphasomething", Some(&[(0, 5)]));
    not_ok(&words, "alpha", "alp");
    ok(&words, "a", "alpha", Some(&[(0, 1)]));
    not_ok(&words, "x", "alpha");
    ok(&words, "A", "alpha", Some(&[(0, 1)]));
    ok(&words, "AlPh", "alPHA", Some(&[(0, 4)]));
    ok(&words, "Debug Console", "Open: Debug Console", None);
    ok(&words, "gp", "Git: Pull", Some(&[(0, 1), (5, 6)]));
    ok(&words, "g p", "Git: Pull", Some(&[(0, 1), (5, 6)]));
    ok(&words, "gipu", "Git: Pull", Some(&[(0, 2), (5, 7)]));
    ok(&words, "gp", "Category: Git: Pull", Some(&[(10, 11), (15, 16)]));
    ok(&words, "g p", "Category: Git: Pull", Some(&[(10, 11), (15, 16)]));
    ok(&words, "gipu", "Category: Git: Pull", Some(&[(10, 12), (15, 17)]));
    not_ok(&words, "it", "Git: Pull");
    not_ok(&words, "ll", "Git: Pull");
    ok(&words, "git: プル", "git: プル", Some(&[(0, 7)]));
    ok(&words, "git プル", "git: プル", Some(&[(0, 3), (5, 7)]));
    ok(&words, "öäk", "Öhm: Älles Klar", Some(&[(0, 1), (5, 6), (11, 12)]));
    ok(&words, "C++", "C/C++: command", Some(&[(2, 5)]));
    ok(&words, ".", ":", Some(&[]));
    ok(&words, ".", ".", Some(&[(0, 1)]));
    ok(&words, "bar", "foo-bar", None);
    ok(&words, "bar test", "foo-bar test", None);
    ok(&words, "fbt", "foo-bar test", None);
    ok(&words, "bar test", "foo-bar (test)", None);
    ok(&words, "foo bar", "foo (bar)", None);
    not_ok(&words, "bar est", "foo-bar test");
    not_ok(&words, "fo ar", "foo-bar test");
    not_ok(&words, "for", "foo-bar test");
    ok(&words, "foo bar", "foo-bar", None);
    ok(&words, "foo bar", "123 foo-bar 456", None);
    ok(&words, "foo-bar", "foo bar", None);
    ok(&words, "foo:bar", "foo:bar", None);
}

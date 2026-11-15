//! Ported from `src/vs/base/test/common/resources.test.ts`. Test names match the upstream
//! `test(...)` names. Branches that TypeScript guards with `isWindows` are kept as `cfg!` checks,
//! so both are compiled; only the host's branch runs.

use super::{ExtUri, HOST_SEPARATOR};
use crate::uri::Uri;

fn uri(s: &str) -> Uri {
    Uri::parse(s).unwrap()
}

fn file(path: &str) -> Uri {
    Uri::file(path)
}

fn from(scheme: &str, authority: &str, path: &str, query: &str, fragment: &str) -> Uri {
    Uri::from_parts(scheme, authority, path, query, fragment).unwrap()
}

/// `extUri`, or `extUriIgnorePathCase` when `ignore_case` is set.
fn util_for(ignore_case: bool) -> ExtUri {
    if ignore_case {
        ExtUri::ignore_path_case()
    } else {
        ExtUri::case_sensitive()
    }
}

fn e() -> ExtUri {
    ExtUri::case_sensitive()
}

fn cwd() -> &'static str {
    if cfg!(windows) { "C:\\" } else { "/" }
}

fn assert_equal_uri(actual: &Uri, expected: &Uri, ignore_case: bool) {
    if !util_for(ignore_case).is_equal(expected, actual, false) {
        assert_eq!(actual.to_string(), expected.to_string());
    }
}

fn remove_trailing(u: &Uri) -> Uri {
    e().remove_trailing_path_separator(u, HOST_SEPARATOR)
        .unwrap()
}

#[test]
fn distinct_parents() {
    let resources = [
        file("/some/folderA/file.txt"),
        file("/some/folderB/file.txt"),
        file("/some/folderC/file.txt"),
    ];
    let distinct = super::distinct_parents(&resources, |r| r);
    assert_eq!(distinct, [&resources[0], &resources[1], &resources[2]]);

    // Parent and child.
    let resources = [
        file("/some/folderA"),
        file("/some/folderA/file.txt"),
        file("/some/folderA/child/file.txt"),
        file("/some/folderA2/file.txt"),
        file("/some/file.txt"),
    ];
    let distinct = super::distinct_parents(&resources, |r| r);
    assert_eq!(distinct, [&resources[0], &resources[3], &resources[4]]);
}

#[test]
fn dirname() {
    let x = e();
    if cfg!(windows) {
        assert_eq!(
            x.dirname(&file("c:\\some\\file\\test.txt"))
                .unwrap()
                .to_string(),
            "file:///c%3A/some/file"
        );
        assert_eq!(
            x.dirname(&file("c:\\some\\file")).unwrap().to_string(),
            "file:///c%3A/some"
        );
        assert_eq!(
            x.dirname(&file("c:\\some\\file\\")).unwrap().to_string(),
            "file:///c%3A/some"
        );
        assert_eq!(
            x.dirname(&file("c:\\some")).unwrap().to_string(),
            "file:///c%3A/"
        );
        assert_eq!(
            x.dirname(&file("C:\\some")).unwrap().to_string(),
            "file:///c%3A/"
        );
        assert_eq!(
            x.dirname(&file("c:\\")).unwrap().to_string(),
            "file:///c%3A/"
        );
    } else {
        assert_eq!(
            x.dirname(&file("/some/file/test.txt")).unwrap().to_string(),
            "file:///some/file"
        );
        assert_eq!(
            x.dirname(&file("/some/file/")).unwrap().to_string(),
            "file:///some"
        );
        assert_eq!(
            x.dirname(&file("/some/file")).unwrap().to_string(),
            "file:///some"
        );
    }
    let d = |s: &str| x.dirname(&uri(s)).unwrap().to_string();
    assert_eq!(d("foo://a/some/file/test.txt"), "foo://a/some/file");
    assert_eq!(d("foo://a/some/file/"), "foo://a/some");
    assert_eq!(d("foo://a/some/file"), "foo://a/some");
    assert_eq!(d("foo://a/some"), "foo://a/");
    assert_eq!(d("foo://a/"), "foo://a/");
    assert_eq!(d("foo://a"), "foo://a");

    // Does not fail on an authority with slashes and no path (issue 41987).
    x.dirname(&from("file", "/users/someone/portal.h", "", "", ""))
        .unwrap();

    assert_eq!(d("foo://a/b/c?q"), "foo://a/b?q");
}

#[test]
fn basename() {
    let x = e();
    if cfg!(windows) {
        assert_eq!(x.basename(&file("c:\\some\\file\\test.txt")), "test.txt");
        assert_eq!(x.basename(&file("c:\\some\\file")), "file");
        assert_eq!(x.basename(&file("c:\\some\\file\\")), "file");
        assert_eq!(x.basename(&file("C:\\some\\file\\")), "file");
    } else {
        assert_eq!(x.basename(&file("/some/file/test.txt")), "test.txt");
        assert_eq!(x.basename(&file("/some/file/")), "file");
        assert_eq!(x.basename(&file("/some/file")), "file");
        assert_eq!(x.basename(&file("/some")), "some");
    }
    assert_eq!(x.basename(&uri("foo://a/some/file/test.txt")), "test.txt");
    assert_eq!(x.basename(&uri("foo://a/some/file/")), "file");
    assert_eq!(x.basename(&uri("foo://a/some/file")), "file");
    assert_eq!(x.basename(&uri("foo://a/some")), "some");
    assert_eq!(x.basename(&uri("foo://a/")), "");
    assert_eq!(x.basename(&uri("foo://a")), "");
}

#[test]
fn join_path() {
    let x = e();
    let j = |u: &Uri, fragments: &[&str]| x.join_path(u, fragments).unwrap().to_string();
    if cfg!(windows) {
        assert_eq!(
            j(&file("c:\\foo\\bar"), &["/file.js"]),
            "file:///c%3A/foo/bar/file.js"
        );
        assert_eq!(
            j(&file("c:\\foo\\bar\\"), &["file.js"]),
            "file:///c%3A/foo/bar/file.js"
        );
        assert_eq!(
            j(&file("c:\\foo\\bar\\"), &["/file.js"]),
            "file:///c%3A/foo/bar/file.js"
        );
        assert_eq!(j(&file("c:\\"), &["/file.js"]), "file:///c%3A/file.js");
        assert_eq!(
            j(&file("c:\\"), &["bar/file.js"]),
            "file:///c%3A/bar/file.js"
        );
        assert_eq!(
            j(&file("c:\\foo"), &["./file.js"]),
            "file:///c%3A/foo/file.js"
        );
        assert_eq!(
            j(&file("c:\\foo"), &["/./file.js"]),
            "file:///c%3A/foo/file.js"
        );
        assert_eq!(j(&file("C:\\foo"), &["../file.js"]), "file:///c%3A/file.js");
        assert_eq!(
            j(&file("C:\\foo\\."), &["../file.js"]),
            "file:///c%3A/file.js"
        );
    } else {
        assert_eq!(
            j(&file("/foo/bar"), &["/file.js"]),
            "file:///foo/bar/file.js"
        );
        assert_eq!(
            j(&file("/foo/bar"), &["file.js"]),
            "file:///foo/bar/file.js"
        );
        assert_eq!(
            j(&file("/foo/bar/"), &["/file.js"]),
            "file:///foo/bar/file.js"
        );
        assert_eq!(j(&file("/"), &["/file.js"]), "file:///file.js");
        assert_eq!(
            j(&file("/foo/bar"), &["./file.js"]),
            "file:///foo/bar/file.js"
        );
        assert_eq!(
            j(&file("/foo/bar"), &["/./file.js"]),
            "file:///foo/bar/file.js"
        );
        assert_eq!(j(&file("/foo/bar"), &["../file.js"]), "file:///foo/file.js");
    }
    assert_eq!(j(&uri("foo://a/foo/bar"), &[]), "foo://a/foo/bar");
    assert_eq!(
        j(&uri("foo://a/foo/bar"), &["/file.js"]),
        "foo://a/foo/bar/file.js"
    );
    assert_eq!(
        j(&uri("foo://a/foo/bar"), &["file.js"]),
        "foo://a/foo/bar/file.js"
    );
    assert_eq!(
        j(&uri("foo://a/foo/bar/"), &["/file.js"]),
        "foo://a/foo/bar/file.js"
    );
    assert_eq!(j(&uri("foo://a/"), &["/file.js"]), "foo://a/file.js");
    assert_eq!(
        j(&uri("foo://a/foo/bar/"), &["./file.js"]),
        "foo://a/foo/bar/file.js"
    );
    assert_eq!(
        j(&uri("foo://a/foo/bar/"), &["/./file.js"]),
        "foo://a/foo/bar/file.js"
    );
    assert_eq!(
        j(&uri("foo://a/foo/bar/"), &["../file.js"]),
        "foo://a/foo/file.js"
    );

    let custom = from("myScheme", "authority", "/path", "query", "fragment");
    assert_eq!(
        j(&custom, &["/file.js"]),
        "myScheme://authority/path/file.js?query#fragment"
    );
}

#[test]
fn normalize_path() {
    let x = e();
    let n = |u: &Uri| x.normalize_path(u).unwrap().to_string();
    if cfg!(windows) {
        assert_eq!(n(&file("c:\\foo\\.\\bar")), "file:///c%3A/foo/bar");
        assert_eq!(n(&file("c:\\foo\\.")), "file:///c%3A/foo");
        assert_eq!(n(&file("c:\\foo\\.\\")), "file:///c%3A/foo/");
        assert_eq!(n(&file("c:\\foo\\..")), "file:///c%3A/");
        assert_eq!(n(&file("c:\\foo\\..\\bar")), "file:///c%3A/bar");
        assert_eq!(n(&file("c:\\foo\\..\\..\\bar")), "file:///c%3A/bar");
        assert_eq!(n(&file("c:\\foo\\foo\\..\\..\\bar")), "file:///c%3A/bar");
        assert_eq!(n(&file("C:\\foo\\foo\\.\\..\\..\\bar")), "file:///c%3A/bar");
        assert_eq!(
            n(&file("C:\\foo\\foo\\.\\..\\some\\..\\bar")),
            "file:///c%3A/foo/bar"
        );
    } else {
        assert_eq!(n(&file("/foo/./bar")), "file:///foo/bar");
        assert_eq!(n(&file("/foo/.")), "file:///foo");
        assert_eq!(n(&file("/foo/./")), "file:///foo/");
        assert_eq!(n(&file("/foo/..")), "file:///");
        assert_eq!(n(&file("/foo/../bar")), "file:///bar");
        assert_eq!(n(&file("/foo/../../bar")), "file:///bar");
        assert_eq!(n(&file("/foo/foo/../../bar")), "file:///bar");
        assert_eq!(n(&file("/foo/foo/./../../bar")), "file:///bar");
        assert_eq!(n(&file("/foo/foo/./../some/../bar")), "file:///foo/bar");
        assert_eq!(n(&file("/f")), "file:///f");
    }
    assert_eq!(n(&uri("foo://a/foo/./bar")), "foo://a/foo/bar");
    assert_eq!(n(&uri("foo://a/foo/.")), "foo://a/foo");
    assert_eq!(n(&uri("foo://a/foo/./")), "foo://a/foo/");
    assert_eq!(n(&uri("foo://a/foo/..")), "foo://a/");
    assert_eq!(n(&uri("foo://a/foo/../bar")), "foo://a/bar");
    assert_eq!(n(&uri("foo://a/foo/../../bar")), "foo://a/bar");
    assert_eq!(n(&uri("foo://a/foo/foo/../../bar")), "foo://a/bar");
    assert_eq!(n(&uri("foo://a/foo/foo/./../../bar")), "foo://a/bar");
    assert_eq!(
        n(&uri("foo://a/foo/foo/./../some/../bar")),
        "foo://a/foo/bar"
    );
    assert_eq!(n(&uri("foo://a")), "foo://a");
    assert_eq!(n(&uri("foo://a/")), "foo://a/");
    assert_eq!(
        n(&uri("foo://a/foo/./bar?q=1")),
        uri("foo://a/foo/bar?q%3D1").to_string()
    );
}

#[test]
fn is_absolute() {
    if cfg!(windows) {
        assert!(e().is_absolute_path(&file("c:\\foo\\")));
        assert!(e().is_absolute_path(&file("C:\\foo\\")));
        assert!(e().is_absolute_path(&file("bar"))); // URI normalizes all file URIs to be absolute
    } else {
        assert!(e().is_absolute_path(&file("/foo/bar")));
        assert!(e().is_absolute_path(&file("bar"))); // URI normalizes all file URIs to be absolute
    }
    assert!(!e().is_absolute_path(&uri("foo:foo")));
    assert!(e().is_absolute_path(&uri("foo://a/foo/.")));
}

fn assert_trailing_separator(u: &Uri, expected: bool) {
    assert_eq!(
        e().has_trailing_path_separator(u, HOST_SEPARATOR),
        expected,
        "{u}"
    );
}

fn assert_remove_trailing_separator(u: &Uri, expected: &Uri) {
    assert_equal_uri(&remove_trailing(u), expected, false);
}

fn assert_add_trailing_separator(u: &Uri, expected: &Uri) {
    let actual = e().add_trailing_path_separator(u, HOST_SEPARATOR).unwrap();
    assert_equal_uri(&actual, expected, false);
}

#[test]
fn trailing_path_separator() {
    assert_trailing_separator(&uri("foo://a/foo"), false);
    assert_trailing_separator(&uri("foo://a/foo/"), true);
    assert_trailing_separator(&uri("foo://a/"), false);
    assert_trailing_separator(&uri("foo://a"), false);

    assert_remove_trailing_separator(&uri("foo://a/foo"), &uri("foo://a/foo"));
    assert_remove_trailing_separator(&uri("foo://a/foo/"), &uri("foo://a/foo"));
    assert_remove_trailing_separator(&uri("foo://a/"), &uri("foo://a/"));
    assert_remove_trailing_separator(&uri("foo://a"), &uri("foo://a"));

    assert_add_trailing_separator(&uri("foo://a/foo"), &uri("foo://a/foo/"));
    assert_add_trailing_separator(&uri("foo://a/foo/"), &uri("foo://a/foo/"));
    assert_add_trailing_separator(&uri("foo://a/"), &uri("foo://a/"));
    assert_add_trailing_separator(&uri("foo://a"), &uri("foo://a/"));

    if cfg!(windows) {
        assert_trailing_separator(&file("c:\\a\\foo"), false);
        assert_trailing_separator(&file("c:\\a\\foo\\"), true);
        assert_trailing_separator(&file("c:\\"), false);
        assert_trailing_separator(&file("\\\\server\\share\\some\\"), true);
        assert_trailing_separator(&file("\\\\server\\share\\"), false);

        assert_remove_trailing_separator(&file("c:\\a\\foo"), &file("c:\\a\\foo"));
        assert_remove_trailing_separator(&file("c:\\a\\foo\\"), &file("c:\\a\\foo"));
        assert_remove_trailing_separator(&file("c:\\"), &file("c:\\"));
        assert_remove_trailing_separator(
            &file("\\\\server\\share\\some\\"),
            &file("\\\\server\\share\\some"),
        );
        assert_remove_trailing_separator(
            &file("\\\\server\\share\\"),
            &file("\\\\server\\share\\"),
        );

        assert_add_trailing_separator(&file("c:\\a\\foo"), &file("c:\\a\\foo\\"));
        assert_add_trailing_separator(&file("c:\\a\\foo\\"), &file("c:\\a\\foo\\"));
        assert_add_trailing_separator(&file("c:\\"), &file("c:\\"));
        assert_add_trailing_separator(
            &file("\\\\server\\share\\some"),
            &file("\\\\server\\share\\some\\"),
        );
        assert_add_trailing_separator(
            &file("\\\\server\\share\\some\\"),
            &file("\\\\server\\share\\some\\"),
        );
    } else {
        assert_trailing_separator(&file("/foo/bar"), false);
        assert_trailing_separator(&file("/foo/bar/"), true);
        assert_trailing_separator(&file("/"), false);

        assert_remove_trailing_separator(&file("/foo/bar"), &file("/foo/bar"));
        assert_remove_trailing_separator(&file("/foo/bar/"), &file("/foo/bar"));
        assert_remove_trailing_separator(&file("/"), &file("/"));

        assert_add_trailing_separator(&file("/foo/bar"), &file("/foo/bar/"));
        assert_add_trailing_separator(&file("/foo/bar/"), &file("/foo/bar/"));
        assert_add_trailing_separator(&file("/"), &file("/"));
    }
}

fn assert_relative_path(
    u1: &Uri,
    u2: &Uri,
    expected: Option<&str>,
    ignore_join: bool,
    ignore_case: bool,
) {
    let util = util_for(ignore_case);
    assert_eq!(
        util.relative_path(u1, u2, cwd()).as_deref(),
        expected,
        "from {u1} to {u2}"
    );
    if let (Some(expected), false) = (expected, ignore_join) {
        let joined = util.join_path(u1, &[expected]).unwrap();
        assert_equal_uri(&remove_trailing(&joined), &remove_trailing(u2), ignore_case);
    }
}

#[test]
fn relative_path() {
    assert_relative_path(
        &uri("foo://a/foo"),
        &uri("foo://a/foo/bar"),
        Some("bar"),
        false,
        false,
    );
    assert_relative_path(
        &uri("foo://a/foo"),
        &uri("foo://a/foo/bar/"),
        Some("bar"),
        false,
        false,
    );
    assert_relative_path(
        &uri("foo://a/foo"),
        &uri("foo://a/foo/bar/goo"),
        Some("bar/goo"),
        false,
        false,
    );
    assert_relative_path(
        &uri("foo://a/"),
        &uri("foo://a/foo/bar/goo"),
        Some("foo/bar/goo"),
        false,
        false,
    );
    assert_relative_path(
        &uri("foo://a/foo/xoo"),
        &uri("foo://a/foo/bar"),
        Some("../bar"),
        false,
        false,
    );
    assert_relative_path(
        &uri("foo://a/foo/xoo/yoo"),
        &uri("foo://a"),
        Some("../../.."),
        true,
        false,
    );
    assert_relative_path(
        &uri("foo://a/foo"),
        &uri("foo://a/foo/"),
        Some(""),
        false,
        false,
    );
    assert_relative_path(
        &uri("foo://a/foo/"),
        &uri("foo://a/foo"),
        Some(""),
        false,
        false,
    );
    assert_relative_path(
        &uri("foo://a/foo/"),
        &uri("foo://a/foo/"),
        Some(""),
        false,
        false,
    );
    assert_relative_path(
        &uri("foo://a/foo"),
        &uri("foo://a/foo"),
        Some(""),
        false,
        false,
    );
    assert_relative_path(&uri("foo://a"), &uri("foo://a"), Some(""), true, false);
    assert_relative_path(&uri("foo://a/"), &uri("foo://a/"), Some(""), false, false);
    assert_relative_path(&uri("foo://a/"), &uri("foo://a"), Some(""), true, false);
    assert_relative_path(
        &uri("foo://a/foo?q"),
        &uri("foo://a/foo/bar#h"),
        Some("bar"),
        true,
        false,
    );
    assert_relative_path(&uri("foo://"), &uri("foo://a/b"), None, false, false);
    assert_relative_path(&uri("foo://a2/b"), &uri("foo://a/b"), None, false, false);
    assert_relative_path(&uri("goo://a/b"), &uri("foo://a/b"), None, false, false);

    assert_relative_path(
        &uri("foo://a/foo"),
        &uri("foo://A/FOO/bar/goo"),
        Some("bar/goo"),
        false,
        true,
    );
    assert_relative_path(
        &uri("foo://a/foo"),
        &uri("foo://A/FOO/BAR/GOO"),
        Some("BAR/GOO"),
        false,
        true,
    );
    assert_relative_path(
        &uri("foo://a/foo/xoo"),
        &uri("foo://A/FOO/BAR/GOO"),
        Some("../BAR/GOO"),
        false,
        true,
    );
    assert_relative_path(
        &uri("foo:///c:/a/foo"),
        &uri("foo:///C:/a/foo/xoo/"),
        Some("xoo"),
        false,
        true,
    );

    if cfg!(windows) {
        assert_relative_path(
            &file("c:\\foo\\bar"),
            &file("c:\\foo\\bar"),
            Some(""),
            false,
            false,
        );
        assert_relative_path(
            &file("c:\\foo\\bar\\huu"),
            &file("c:\\foo\\bar"),
            Some(".."),
            false,
            false,
        );
        assert_relative_path(
            &file("c:\\foo\\bar\\a1\\a2"),
            &file("c:\\foo\\bar"),
            Some("../.."),
            false,
            false,
        );
        assert_relative_path(
            &file("c:\\foo\\bar\\"),
            &file("c:\\foo\\bar\\a1\\a2"),
            Some("a1/a2"),
            false,
            false,
        );
        assert_relative_path(
            &file("c:\\foo\\bar\\"),
            &file("c:\\foo\\bar\\a1\\a2\\"),
            Some("a1/a2"),
            false,
            false,
        );
        assert_relative_path(
            &file("c:\\"),
            &file("c:\\foo\\bar"),
            Some("foo/bar"),
            false,
            false,
        );
        assert_relative_path(
            &file("\\\\server\\share\\some\\"),
            &file("\\\\server\\share\\some\\path"),
            Some("path"),
            false,
            false,
        );
        // path.join is not root aware, so skip the join check.
        assert_relative_path(
            &file("\\\\server\\share\\some\\"),
            &file("\\\\server\\share2\\some\\path"),
            Some("../../share2/some/path"),
            true,
            false,
        );
    } else {
        assert_relative_path(
            &file("/a/foo"),
            &file("/a/foo/bar"),
            Some("bar"),
            false,
            false,
        );
        assert_relative_path(
            &file("/a/foo"),
            &file("/a/foo/bar/"),
            Some("bar"),
            false,
            false,
        );
        assert_relative_path(
            &file("/a/foo"),
            &file("/a/foo/bar/goo"),
            Some("bar/goo"),
            false,
            false,
        );
        assert_relative_path(
            &file("/a/"),
            &file("/a/foo/bar/goo"),
            Some("foo/bar/goo"),
            false,
            false,
        );
        assert_relative_path(
            &file("/"),
            &file("/a/foo/bar/goo"),
            Some("a/foo/bar/goo"),
            false,
            false,
        );
        assert_relative_path(
            &file("/a/foo/xoo"),
            &file("/a/foo/bar"),
            Some("../bar"),
            false,
            false,
        );
        assert_relative_path(
            &file("/a/foo/xoo/yoo"),
            &file("/a"),
            Some("../../.."),
            false,
            false,
        );
        assert_relative_path(&file("/a/foo"), &file("/a/foo/"), Some(""), false, false);
        assert_relative_path(
            &file("/a/foo"),
            &file("/b/foo/"),
            Some("../../b/foo"),
            false,
            false,
        );
    }
}

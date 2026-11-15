//! Ported from `src/vs/base/test/common/resources.test.ts`. Test names match the upstream
//! `test(...)` names. Branches that TypeScript guards with `isWindows` are kept as `cfg!` checks,
//! so both are compiled; only the host's branch runs.

use super::{ExtUri, HOST_SEPARATOR};
use crate::path::{posix, win32};
use crate::uri::Uri;

const USER_DATA: &str = "vscode-userdata";
const REMOTE: &str = "vscode-remote";

fn uri(s: &str) -> Uri {
    Uri::parse(s).unwrap()
}

fn file(path: &str) -> Uri {
    Uri::file(path)
}

fn from(scheme: &str, authority: &str, path: &str, query: &str, fragment: &str) -> Uri {
    Uri::from_parts(scheme, authority, path, query, fragment).unwrap()
}

fn with_path(u: &Uri, path: &str) -> Uri {
    from(u.scheme(), u.authority(), path, u.query(), u.fragment())
}

fn with_scheme(u: &Uri, scheme: &str) -> Uri {
    from(scheme, u.authority(), u.path(), u.query(), u.fragment())
}

fn with_authority(u: &Uri, authority: &str) -> Uri {
    from(u.scheme(), authority, u.path(), u.query(), u.fragment())
}

fn with_query(u: &Uri, query: &str) -> Uri {
    from(u.scheme(), u.authority(), u.path(), query, u.fragment())
}

fn with_fragment(u: &Uri, fragment: &str) -> Uri {
    from(u.scheme(), u.authority(), u.path(), u.query(), fragment)
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

fn assert_resolve(u1: &Uri, path: &str, expected: &Uri) {
    let actual = e().resolve_path(u1, path, cwd()).unwrap();
    assert_equal_uri(&actual, expected, false);

    let is_absolute = if path.contains('/') {
        posix::is_absolute(path)
    } else {
        win32::is_absolute(path)
    };
    if !is_absolute {
        let mut expected_path = if cfg!(windows) {
            path.replace('\\', "/")
        } else {
            path.to_owned()
        };
        if let Some(stripped) = expected_path.strip_prefix("./") {
            expected_path = stripped.to_owned();
        }
        assert_eq!(
            e().relative_path(u1, &actual, cwd()).as_deref(),
            Some(expected_path.as_str()),
            "relativePath ({u1}) on actual ({actual}) should be to path ({expected_path})"
        );
    }
}

#[test]
fn resolve() {
    if cfg!(windows) {
        assert_resolve(
            &file("c:\\foo\\bar"),
            "file.js",
            &file("c:\\foo\\bar\\file.js"),
        );
        assert_resolve(
            &file("c:\\foo\\bar"),
            "t\\file.js",
            &file("c:\\foo\\bar\\t\\file.js"),
        );
        assert_resolve(
            &file("c:\\foo\\bar"),
            ".\\t\\file.js",
            &file("c:\\foo\\bar\\t\\file.js"),
        );
        assert_resolve(
            &file("c:\\foo\\bar"),
            "a1/file.js",
            &file("c:\\foo\\bar\\a1\\file.js"),
        );
        assert_resolve(
            &file("c:\\foo\\bar"),
            "./a1/file.js",
            &file("c:\\foo\\bar\\a1\\file.js"),
        );
        assert_resolve(
            &file("c:\\foo\\bar"),
            "\\b1\\file.js",
            &file("c:\\b1\\file.js"),
        );
        assert_resolve(
            &file("c:\\foo\\bar"),
            "/b1/file.js",
            &file("c:\\b1\\file.js"),
        );
        assert_resolve(
            &file("c:\\foo\\bar\\"),
            "file.js",
            &file("c:\\foo\\bar\\file.js"),
        );

        assert_resolve(&file("c:\\"), "file.js", &file("c:\\file.js"));
        assert_resolve(&file("c:\\"), "\\b1\\file.js", &file("c:\\b1\\file.js"));
        assert_resolve(&file("c:\\"), "/b1/file.js", &file("c:\\b1\\file.js"));
        assert_resolve(&file("c:\\"), "d:\\foo\\bar.txt", &file("d:\\foo\\bar.txt"));

        assert_resolve(
            &file("\\\\server\\share\\some\\"),
            "b1\\file.js",
            &file("\\\\server\\share\\some\\b1\\file.js"),
        );
        assert_resolve(
            &file("\\\\server\\share\\some\\"),
            "\\file.js",
            &file("\\\\server\\share\\file.js"),
        );

        assert_resolve(
            &file("c:\\"),
            "\\\\server\\share\\some\\",
            &file("\\\\server\\share\\some"),
        );
        assert_resolve(&file("\\\\server\\share\\some\\"), "c:\\", &file("c:\\"));
    } else {
        assert_resolve(&file("/foo/bar"), "file.js", &file("/foo/bar/file.js"));
        assert_resolve(&file("/foo/bar"), "./file.js", &file("/foo/bar/file.js"));
        assert_resolve(&file("/foo/bar"), "/file.js", &file("/file.js"));
        assert_resolve(&file("/foo/bar/"), "file.js", &file("/foo/bar/file.js"));
        assert_resolve(&file("/"), "file.js", &file("/file.js"));
        assert_resolve(&file(""), "./file.js", &file("/file.js"));
        assert_resolve(&file(""), "/file.js", &file("/file.js"));
    }

    assert_resolve(
        &uri("foo://server/foo/bar"),
        "file.js",
        &uri("foo://server/foo/bar/file.js"),
    );
    assert_resolve(
        &uri("foo://server/foo/bar"),
        "./file.js",
        &uri("foo://server/foo/bar/file.js"),
    );
    assert_resolve(
        &uri("foo://server/foo/bar"),
        "c:\\a1\\b1",
        &uri("foo://server/c:/a1/b1"),
    );
    assert_resolve(
        &uri("foo://server/foo/bar"),
        "c:\\",
        &uri("foo://server/c:"),
    );
}

fn assert_is_equal(u1: &Uri, u2: &Uri, ignore_case: bool, expected: bool) {
    let util = util_for(ignore_case);
    assert_eq!(util.is_equal(u1, u2, false), expected, "{u1} vs {u2}");
    assert_eq!(
        util.compare(u1, u2, false) == std::cmp::Ordering::Equal,
        expected
    );
    assert_eq!(
        util.get_comparison_key(u1, false) == util.get_comparison_key(u2, false),
        expected,
        "comparison keys {u1}, {u2}"
    );
    assert_eq!(
        util.is_equal_or_parent(u1, u2, false, false),
        expected,
        "isEqualOrParent {u1}, {u2}"
    );
    if !ignore_case {
        assert_eq!(u1.to_string() == u2.to_string(), expected);
    }
}

#[test]
fn is_equal() {
    let file_uri = if cfg!(windows) {
        file("c:\\foo\\bar")
    } else {
        file("/foo/bar")
    };
    let file_uri2 = if cfg!(windows) {
        file("C:\\foo\\Bar")
    } else {
        file("/foo/Bar")
    };
    assert_is_equal(&file_uri, &file_uri, true, true);
    assert_is_equal(&file_uri, &file_uri, false, true);
    assert_is_equal(&file_uri, &file_uri2, true, true);
    assert_is_equal(&file_uri, &file_uri2, false, false);

    let uri3 = uri("foo://server:453/foo/bar");
    let uri4 = uri("foo://server:453/foo/Bar");
    assert_is_equal(&uri3, &uri3, true, true);
    assert_is_equal(&uri3, &uri3, false, true);
    assert_is_equal(&uri3, &uri4, true, true);
    assert_is_equal(&uri3, &uri4, false, false);

    assert_is_equal(&file_uri, &uri3, true, false);

    assert_is_equal(&uri("file://server"), &uri("file://server/"), true, true);
    assert_is_equal(&uri("http://server"), &uri("http://server/"), true, true);
    assert_is_equal(&uri("foo://server"), &uri("foo://server/"), true, false); // only some schemes default their path to /
    assert_is_equal(
        &uri("foo://server/foo"),
        &uri("foo://server/foo/"),
        true,
        false,
    );
    assert_is_equal(
        &uri("foo://server/foo"),
        &uri("foo://server/foo?"),
        true,
        true,
    );

    let uri5 = uri("foo://server:453/foo/bar?q=1");
    let uri6 = uri("foo://server:453/foo/bar#xy");

    assert_is_equal(&uri5, &uri5, true, true);
    assert_is_equal(&uri5, &uri3, true, false);
    assert_is_equal(&uri6, &uri6, true, true);
    assert_is_equal(&uri6, &uri5, true, false);
    assert_is_equal(&uri6, &uri3, true, false);
}

#[test]
fn is_equal_or_parent() {
    let x = ExtUri::ignore_path_case();
    let y = e();
    let file_uri = if cfg!(windows) {
        file("c:\\foo\\bar")
    } else {
        file("/foo/bar")
    };
    let file_uri2 = if cfg!(windows) {
        file("c:\\foo")
    } else {
        file("/foo")
    };
    let file_uri2b = if cfg!(windows) {
        file("C:\\Foo\\")
    } else {
        file("/Foo/")
    };
    assert!(
        x.is_equal_or_parent(&file_uri, &file_uri, false, false),
        "1"
    );
    assert!(
        y.is_equal_or_parent(&file_uri, &file_uri, false, false),
        "2"
    );
    assert!(
        x.is_equal_or_parent(&file_uri, &file_uri2, false, false),
        "3"
    );
    assert!(
        y.is_equal_or_parent(&file_uri, &file_uri2, false, false),
        "4"
    );
    assert!(
        x.is_equal_or_parent(&file_uri, &file_uri2b, false, false),
        "5"
    );
    assert!(
        !y.is_equal_or_parent(&file_uri, &file_uri2b, false, false),
        "6"
    );

    assert!(
        !y.is_equal_or_parent(&file_uri2, &file_uri, false, false),
        "7"
    );
    assert!(
        x.is_equal_or_parent(&file_uri2b, &file_uri2, false, false),
        "8"
    );

    let uri3 = uri("foo://server:453/foo/bar/goo");
    let uri4 = uri("foo://server:453/foo/");
    let uri5 = uri("foo://server:453/foo");
    assert!(x.is_equal_or_parent(&uri3, &uri3, true, false), "11");
    assert!(y.is_equal_or_parent(&uri3, &uri3, false, false), "12");
    assert!(x.is_equal_or_parent(&uri3, &uri4, true, false), "13");
    assert!(y.is_equal_or_parent(&uri3, &uri4, false, false), "14");
    assert!(!x.is_equal_or_parent(&uri3, &file_uri, true, false), "15");
    assert!(x.is_equal_or_parent(&uri5, &uri5, true, false), "16");

    let uri6 = uri("foo://server:453/foo?q=1");
    let uri7 = uri("foo://server:453/foo/bar?q=1");
    assert!(!x.is_equal_or_parent(&uri6, &uri5, false, false), "17");
    assert!(x.is_equal_or_parent(&uri6, &uri6, false, false), "18");
    assert!(x.is_equal_or_parent(&uri7, &uri6, false, false), "19");
    assert!(!x.is_equal_or_parent(&uri7, &uri5, false, false), "20");
}

#[test]
fn is_equal_or_parent_handles_dotdot() {
    let file_uri = file("/home/user/projects");
    let file_uri2 = file("/home/user/projects/../../../../tmp/folder");
    assert!(!e().is_equal_or_parent(&file_uri2, &file_uri, false, false));
}

#[test]
fn is_equal_or_parent_handles_backslash_traversal() {
    for scheme in [USER_DATA, REMOTE] {
        for ignore_case in [false, true] {
            let util = util_for(ignore_case);
            let root = from(
                scheme,
                if scheme == REMOTE { "host" } else { "" },
                "/c:/lab/ws",
                "",
                "",
            );
            let cases: &[(&str, bool)] = &[
                ("readme.md", true),
                ("nested/file.txt", true),
                ("nested\\file.txt", true),
                ("nested/..\\readme.md", true),
                ("nested\\..\\readme.md", true),
                ("nested\\..", true),
                ("../outside.txt", false),
                ("..%2Foutside.txt", false),
                ("..%5Coutside.txt", false),
                ("..%5C..%5Coutside.txt", false),
                ("..%5C..%5C..%5Coutside.txt", false),
                ("..\\..\\..\\..\\outside.txt", false),
                ("..\\..\\..\\..\\..\\outside.txt", false),
                ("..%5C..%5C..%5C..%5C..%5C..%5Coutside.txt", false),
                ("..\\\\..\\\\outside.txt", false),
                ("nested/..\\..\\outside.txt", false),
                (
                    "..\\..\\..\\..\\..\\..\\..\\..\\..\\..\\..\\..\\outside.txt",
                    false,
                ),
                ("..%255Coutside.txt", true),
                ("nested\\file.txt?query=1", false),
                ("nested\\file.txt#fragment", false),
            ];
            for (path, expected) in cases {
                let child = uri(&format!("{root}/{path}"));
                assert_eq!(
                    util.is_equal_or_parent(&child, &root, false, false),
                    *expected,
                    "{scheme} ignoreCase={ignore_case} {path}"
                );
            }
        }
    }
}

#[test]
fn is_equal_or_parent_handles_backslash_traversal_with_backslashes_in_parent_path() {
    let root = from(REMOTE, "host", "/c:/lab/ws\\nested", "", "");
    let paths = ["readme.md", "..\\outside.txt", "child\\..\\readme.md"];
    let mut results: Vec<bool> = paths
        .iter()
        .map(|p| e().is_equal_or_parent(&e_join(&root, p), &root, false, false))
        .collect();
    let outside = with_path(&root, "/c:/lab/outside.txt");
    let parent = with_path(&root, &format!("{}/..", root.path()));
    results.push(e().is_equal_or_parent(&outside, &parent, false, false));
    assert_eq!(results, [true, false, true, false]);
}

fn e_join(u: &Uri, fragment: &str) -> Uri {
    e().join_path(u, &[fragment]).unwrap()
}

#[test]
fn is_equal_or_parent_handles_backslash_traversal_through_unc_roots() {
    for ignore_case in [false, true] {
        let util = util_for(ignore_case);
        let root = from(REMOTE, "host", r"/\server/share/ws", "", "");
        let cases: Vec<(&str, bool)> = vec![
            (root.path(), true),
            (r"/\server/share/ws/readme.md", true),
            (r"/\server/share/ws/nested\..\readme.md", true),
            (r"/\SERVER/SHARE/WS/nested\..\readme.md", ignore_case),
            (
                r"/\server/share/ws/..\..\..\server\share\ws\marker.txt",
                false,
            ),
        ];
        for (path, expected) in cases {
            assert_eq!(
                util.is_equal_or_parent(&with_path(&root, path), &root, false, false),
                expected,
                "ignoreCase={ignore_case} {path}"
            );
        }
    }
}

#[test]
fn is_equal_or_parent_uses_explicit_posix_semantics_for_non_file_uris() {
    for ignore_case in [false, true] {
        let util = util_for(ignore_case);
        let root = from(REMOTE, "host", "/workspace/project", "", "");
        let child = e().join_path(&root, &["..\\file.txt"]).unwrap();
        let backslash_root = with_path(&root, "/workspace/parent\\..\\project");
        let joined_backslash = e().join_path(&backslash_root, &["..\\file.txt"]).unwrap();
        let nested = e().join_path(&root, &["nested/..\\..\\file.txt"]).unwrap();
        let sibling = e().join_path(&root, &["../outside.txt"]).unwrap();
        let cases: Vec<(Uri, Uri, bool, bool)> = vec![
            (child.clone(), root.clone(), false, true),
            (nested, root.clone(), false, true),
            (joined_backslash, backslash_root, false, true),
            (
                with_path(&root, "/workspace/project/a\\b/../../outside.txt"),
                root.clone(),
                false,
                false,
            ),
            (sibling, root.clone(), false, false),
            (
                with_path(&root, "/workspace/project-other/..\\file.txt"),
                root.clone(),
                false,
                false,
            ),
            (
                with_path(&child, &child.path().to_uppercase()),
                root.clone(),
                false,
                ignore_case,
            ),
            (with_scheme(&child, USER_DATA), root.clone(), false, false),
            (with_authority(&child, "other"), root.clone(), false, false),
            (with_query(&child, "version=1"), root.clone(), false, false),
            (with_fragment(&child, "section"), root.clone(), false, false),
            (with_fragment(&child, "section"), root.clone(), true, true),
        ];
        for (base, parent, ignore_fragment, expected) in cases {
            assert_eq!(
                util.is_equal_or_parent(&base, &parent, ignore_fragment, true),
                expected,
                "ignoreCase={ignore_case} base={base}"
            );
        }
    }
}

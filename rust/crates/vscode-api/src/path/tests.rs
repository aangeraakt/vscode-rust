//! Ported from `src/vs/base/test/common/path.test.ts`. Test names match the upstream `test(...)`
//! names. Cases that the TypeScript suite runs only on one host are ported as-is: the functions
//! here do not depend on the host, so both flavors are checked everywhere.

use super::{ParsedPath, posix, win32};

/// Replaces forward slashes with backslashes (`win` in path.test.ts).
fn win(s: &str) -> String {
    s.replace('/', "\\")
}

type JoinCase = (&'static [&'static str], &'static str);

/// Cases run with both `posix.join` and `win32.join`.
const JOIN_CASES: &[JoinCase] = &[
    (&[".", "x/b", "..", "/b/c.js"], "x/b/c.js"),
    (&[], "."),
    (&["/.", "x/b", "..", "/b/c.js"], "/x/b/c.js"),
    (&["/foo", "../../../bar"], "/bar"),
    (&["foo", "../../../bar"], "../../bar"),
    (&["foo/", "../../../bar"], "../../bar"),
    (&["foo/x", "../../../bar"], "../bar"),
    (&["foo/x", "./bar"], "foo/x/bar"),
    (&["foo/x/", "./bar"], "foo/x/bar"),
    (&["foo/x/", ".", "bar"], "foo/x/bar"),
    (&["./"], "./"),
    (&[".", "./"], "./"),
    (&[".", ".", "."], "."),
    (&[".", "./", "."], "."),
    (&[".", "/./", "."], "."),
    (&[".", "/////./", "."], "."),
    (&["."], "."),
    (&["", "."], "."),
    (&["", "foo"], "foo"),
    (&["foo", "/bar"], "foo/bar"),
    (&["", "/foo"], "/foo"),
    (&["", "", "/foo"], "/foo"),
    (&["", "", "foo"], "foo"),
    (&["foo", ""], "foo"),
    (&["foo/", ""], "foo/"),
    (&["foo", "", "/bar"], "foo/bar"),
    (&["./", "..", "/foo"], "../foo"),
    (&["./", "..", "..", "/foo"], "../../foo"),
    (&[".", "..", "..", "/foo"], "../../foo"),
    (&["", "..", "..", "/foo"], "../../foo"),
    (&["/"], "/"),
    (&["/", "."], "/"),
    (&["/", ".."], "/"),
    (&["/", "..", ".."], "/"),
    (&[""], "."),
    (&["", ""], "."),
    (&[" /foo"], " /foo"),
    (&[" ", "foo"], " /foo"),
    (&[" ", "."], " "),
    (&[" ", "/"], " /"),
    (&[" ", ""], " "),
    (&["/", "foo"], "/foo"),
    (&["/", "/foo"], "/foo"),
    (&["/", "//foo"], "/foo"),
    (&["/", "", "/foo"], "/foo"),
    (&["", "/", "foo"], "/foo"),
    (&["", "/", "/foo"], "/foo"),
];

/// Windows-only join cases (UNC paths and drive-relative paths).
const JOIN_WIN32_CASES: &[JoinCase] = &[
    // UNC path expected.
    (&["//foo/bar"], "\\\\foo\\bar\\"),
    (&["\\/foo/bar"], "\\\\foo\\bar\\"),
    (&["\\\\foo/bar"], "\\\\foo\\bar\\"),
    // UNC path expected: server and share separate.
    (&["//foo", "bar"], "\\\\foo\\bar\\"),
    (&["//foo/", "bar"], "\\\\foo\\bar\\"),
    (&["//foo", "/bar"], "\\\\foo\\bar\\"),
    // UNC path expected: questionable.
    (&["//foo", "", "bar"], "\\\\foo\\bar\\"),
    (&["//foo/", "", "bar"], "\\\\foo\\bar\\"),
    (&["//foo/", "", "/bar"], "\\\\foo\\bar\\"),
    // UNC path expected: even more questionable.
    (&["", "//foo", "bar"], "\\\\foo\\bar\\"),
    (&["", "//foo/", "bar"], "\\\\foo\\bar\\"),
    (&["", "//foo/", "/bar"], "\\\\foo\\bar\\"),
    // No UNC path expected (no double slash in first component).
    (&["\\", "foo/bar"], "\\foo\\bar"),
    (&["\\", "/foo/bar"], "\\foo\\bar"),
    (&["", "/", "/foo/bar"], "\\foo\\bar"),
    // No UNC path expected (no non-slashes in first component: questionable).
    (&["//", "foo/bar"], "\\foo\\bar"),
    (&["//", "/foo/bar"], "\\foo\\bar"),
    (&["\\\\", "/", "/foo/bar"], "\\foo\\bar"),
    (&["//"], "\\"),
    // No UNC path expected (share name missing: questionable).
    (&["//foo"], "\\foo"),
    (&["//foo/"], "\\foo\\"),
    (&["//foo", "/"], "\\foo\\"),
    (&["//foo", "", "/"], "\\foo\\"),
    // No UNC path expected (too many leading slashes: questionable).
    (&["///foo/bar"], "\\foo\\bar"),
    (&["////foo", "bar"], "\\foo\\bar"),
    (&["\\\\\\/foo/bar"], "\\foo\\bar"),
    // Drive-relative vs drive-absolute paths. This describes the status quo.
    (&["c:"], "c:."),
    (&["c:."], "c:."),
    (&["c:", ""], "c:."),
    (&["", "c:"], "c:."),
    (&["c:.", "/"], "c:.\\"),
    (&["c:.", "file"], "c:file"),
    (&["c:", "/"], "c:\\"),
    (&["c:", "file"], "c:\\file"),
];

/// `join`: both flavors, with the win32 comparison ignoring slash direction.
#[test]
fn join() {
    for (args, expected) in JOIN_CASES {
        assert_eq!(posix::join(args), *expected, "posix.join({args:?})");
        let actual = win32::join(args);
        assert!(
            actual == *expected || actual.replace('\\', "/") == *expected,
            "win32.join({args:?}): expected {expected:?}, actual {actual:?}"
        );
    }
    for (args, expected) in JOIN_WIN32_CASES {
        let actual = win32::join(args);
        assert!(
            actual == *expected || actual.replace('\\', "/") == *expected,
            "win32.join({args:?}): expected {expected:?}, actual {actual:?}"
        );
    }
}

#[test]
fn dirname() {
    assert_eq!(posix::dirname("/a/b/"), "/a");
    assert_eq!(posix::dirname("/a/b"), "/a");
    assert_eq!(posix::dirname("/a"), "/");
    assert_eq!(posix::dirname(""), ".");
    assert_eq!(posix::dirname("/"), "/");
    assert_eq!(posix::dirname("////"), "/");
    assert_eq!(posix::dirname("//a"), "//");
    assert_eq!(posix::dirname("foo"), ".");

    let w = win32::dirname;
    assert_eq!(w("c:\\"), "c:\\");
    assert_eq!(w("c:\\foo"), "c:\\");
    assert_eq!(w("c:\\foo\\"), "c:\\");
    assert_eq!(w("c:\\foo\\bar"), "c:\\foo");
    assert_eq!(w("c:\\foo\\bar\\"), "c:\\foo");
    assert_eq!(w("c:\\foo\\bar\\baz"), "c:\\foo\\bar");
    assert_eq!(w("\\"), "\\");
    assert_eq!(w("\\foo"), "\\");
    assert_eq!(w("\\foo\\"), "\\");
    assert_eq!(w("\\foo\\bar"), "\\foo");
    assert_eq!(w("\\foo\\bar\\"), "\\foo");
    assert_eq!(w("\\foo\\bar\\baz"), "\\foo\\bar");
    assert_eq!(w("c:"), "c:");
    assert_eq!(w("c:foo"), "c:");
    assert_eq!(w("c:foo\\"), "c:");
    assert_eq!(w("c:foo\\bar"), "c:foo");
    assert_eq!(w("c:foo\\bar\\"), "c:foo");
    assert_eq!(w("c:foo\\bar\\baz"), "c:foo\\bar");
    assert_eq!(w("file:stream"), ".");
    assert_eq!(w("dir\\file:stream"), "dir");
    assert_eq!(w("\\\\unc\\share"), "\\\\unc\\share");
    assert_eq!(w("\\\\unc\\share\\foo"), "\\\\unc\\share\\");
    assert_eq!(w("\\\\unc\\share\\foo\\"), "\\\\unc\\share\\");
    assert_eq!(w("\\\\unc\\share\\foo\\bar"), "\\\\unc\\share\\foo");
    assert_eq!(w("\\\\unc\\share\\foo\\bar\\"), "\\\\unc\\share\\foo");
    assert_eq!(
        w("\\\\unc\\share\\foo\\bar\\baz"),
        "\\\\unc\\share\\foo\\bar"
    );
    assert_eq!(w("/a/b/"), "/a");
    assert_eq!(w("/a/b"), "/a");
    assert_eq!(w("/a"), "/");
    assert_eq!(w(""), ".");
    assert_eq!(w("/"), "/");
    assert_eq!(w("////"), "/");
    assert_eq!(w("foo"), ".");

    // Tests from VS Code: `assertDirname(p, expected, win)`.
    let cases: &[(&str, &str, bool)] = &[
        ("foo/bar", "foo", false),
        ("foo\\bar", "foo", true),
        ("/foo/bar", "/foo", false),
        ("\\foo\\bar", "\\foo", true),
        ("/foo", "/", false),
        ("\\foo", "\\", true),
        ("/", "/", false),
        ("\\", "\\", true),
        ("foo", ".", false),
        ("f", ".", false),
        ("f/", ".", false),
        ("/folder/", "/", false),
        ("c:\\some\\file.txt", "c:\\some", true),
        ("c:\\some", "c:\\", true),
        ("c:\\", "c:\\", true),
        ("c:", "c:", true),
        (
            "\\\\server\\share\\some\\path",
            "\\\\server\\share\\some",
            true,
        ),
        ("\\\\server\\share\\some", "\\\\server\\share\\", true),
        ("\\\\server\\share\\", "\\\\server\\share\\", true),
    ];
    for (p, expected, win_flavor) in cases {
        let actual = if *win_flavor {
            win32::dirname(p)
        } else {
            posix::dirname(p)
        };
        assert_eq!(actual, *expected, "dirname({p:?})");
    }
}

type ExtCase = (&'static str, &'static str);

/// `extname` cases. The win32 run replaces `/` with `\`, and a third run prefixes `C:`.
const EXTNAME_CASES: &[ExtCase] = &[
    ("path.test.js", ".js"),
    ("", ""),
    ("/path/to/file", ""),
    ("/path/to/file.ext", ".ext"),
    ("/path.to/file.ext", ".ext"),
    ("/path.to/file", ""),
    ("/path.to/.file", ""),
    ("/path.to/.file.ext", ".ext"),
    ("/path/to/f.ext", ".ext"),
    ("/path/to/..ext", ".ext"),
    ("/path/to/..", ""),
    ("file", ""),
    ("file.ext", ".ext"),
    (".file", ""),
    (".file.ext", ".ext"),
    ("/file", ""),
    ("/file.ext", ".ext"),
    ("/.file", ""),
    ("/.file.ext", ".ext"),
    (".path/file.ext", ".ext"),
    ("file.ext.ext", ".ext"),
    ("file.", "."),
    (".", ""),
    ("./", ""),
    (".file.ext", ".ext"),
    (".file", ""),
    (".file.", "."),
    (".file..", "."),
    ("..", ""),
    ("../", ""),
    ("..file.ext", ".ext"),
    ("..file", ".file"),
    ("..file.", "."),
    ("..file..", "."),
    ("...", "."),
    ("...ext", ".ext"),
    ("....", "."),
    ("file.ext/", ".ext"),
    ("file.ext//", ".ext"),
    ("file/", ""),
    ("file//", ""),
    ("file./", "."),
    ("file.//", "."),
];

#[test]
fn extname() {
    for (input, expected) in EXTNAME_CASES {
        assert_eq!(posix::extname(input), *expected, "posix.extname({input:?})");

        let slashed = win(input);
        assert_eq!(
            win32::extname(&slashed),
            *expected,
            "win32.extname({slashed:?})"
        );

        let drive = format!("C:{slashed}");
        assert_eq!(
            win32::extname(&drive),
            *expected,
            "win32.extname({drive:?})"
        );
    }

    // On Windows, backslash is a path separator.
    assert_eq!(win32::extname(".\\"), "");
    assert_eq!(win32::extname("..\\"), "");
    assert_eq!(win32::extname("file.ext\\"), ".ext");
    assert_eq!(win32::extname("file.ext\\\\"), ".ext");
    assert_eq!(win32::extname("file\\"), "");
    assert_eq!(win32::extname("file\\\\"), "");
    assert_eq!(win32::extname("file.\\"), ".");
    assert_eq!(win32::extname("file.\\\\"), ".");

    // On POSIX, backslash is an ordinary name character.
    assert_eq!(posix::extname(".\\"), "");
    assert_eq!(posix::extname("..\\"), ".\\");
    assert_eq!(posix::extname("file.ext\\"), ".ext\\");
    assert_eq!(posix::extname("file.ext\\\\"), ".ext\\\\");
    assert_eq!(posix::extname("file\\"), "");
    assert_eq!(posix::extname("file\\\\"), "");
    assert_eq!(posix::extname("file.\\"), ".\\");
    assert_eq!(posix::extname("file.\\\\"), ".\\\\");

    // Tests from VS Code.
    assert_eq!(posix::extname("far.boo"), ".boo");
    assert_eq!(posix::extname("far.b"), ".b");
    assert_eq!(posix::extname("far."), ".");
    assert_eq!(posix::extname("far.boo/boo.far"), ".far");
    assert_eq!(posix::extname("far.boo/boo"), "");
}

#[test]
fn resolve() {
    let win_cases: &[(&[&str], &str)] = &[
        (&["c:/blah\\blah", "d:/games", "c:../a"], "c:\\blah\\a"),
        (&["c:/ignore", "d:\\a/b\\c/d", "\\e.exe"], "d:\\e.exe"),
        (&["c:/ignore", "c:/some/file"], "c:\\some\\file"),
        (&["d:/ignore", "d:some/dir//"], "d:\\ignore\\some\\dir"),
        (
            &["//server/share", "..", "relative\\"],
            "\\\\server\\share\\relative",
        ),
        (&["c:/", "//"], "c:\\"),
        (&["c:/", "//dir"], "c:\\dir"),
        (&["c:/", "//server/share"], "\\\\server\\share\\"),
        (&["c:/", "//server//share"], "\\\\server\\share\\"),
        (&["c:/", "///some//dir"], "c:\\some\\dir"),
        (
            &["C:\\foo\\tmp.3\\", "..\\tmp.3\\cycles\\root.js"],
            "C:\\foo\\tmp.3\\cycles\\root.js",
        ),
    ];
    for (args, expected) in win_cases {
        assert_eq!(
            win32::resolve("C:\\some\\cwd", args),
            *expected,
            "win32.resolve({args:?})"
        );
    }

    let posix_cases: &[(&[&str], &str)] = &[
        (&["/var/lib", "../", "file/"], "/var/file"),
        (&["/var/lib", "/../", "file/"], "/file"),
        (&["/some/dir", ".", "/absolute/"], "/absolute"),
        (
            &["/foo/tmp.3/", "../tmp.3/cycles/root.js"],
            "/foo/tmp.3/cycles/root.js",
        ),
    ];
    for (args, expected) in posix_cases {
        assert_eq!(
            posix::resolve("/unused", args),
            *expected,
            "posix.resolve({args:?})"
        );
    }

    // Relative segments resolve against the working directory (the TypeScript test used
    // `process.cwd()`).
    assert_eq!(posix::resolve("/some/cwd", &["."]), "/some/cwd");
    assert_eq!(
        posix::resolve("/some/cwd", &["a/b/c", "../../.."]),
        "/some/cwd"
    );
    assert_eq!(win32::resolve("C:\\some\\cwd", &["."]), "C:\\some\\cwd");
    assert_eq!(
        win32::resolve("C:\\some\\cwd", &["a\\b\\c", "..\\..\\.."]),
        "C:\\some\\cwd"
    );
}

#[test]
fn basename() {
    let file = "path.test.js";
    assert_eq!(posix::basename(file, None), "path.test.js");
    assert_eq!(posix::basename(file, Some(".js")), "path.test");
    assert_eq!(posix::basename(".js", Some(".js")), "");
    assert_eq!(posix::basename("", None), "");
    assert_eq!(posix::basename("/dir/basename.ext", None), "basename.ext");
    assert_eq!(posix::basename("/basename.ext", None), "basename.ext");
    assert_eq!(posix::basename("basename.ext", None), "basename.ext");
    assert_eq!(posix::basename("basename.ext/", None), "basename.ext");
    assert_eq!(posix::basename("basename.ext//", None), "basename.ext");
    assert_eq!(posix::basename("aaa/bbb", Some("/bbb")), "bbb");
    assert_eq!(posix::basename("aaa/bbb", Some("a/bbb")), "bbb");
    assert_eq!(posix::basename("aaa/bbb", Some("bbb")), "bbb");
    assert_eq!(posix::basename("aaa/bbb//", Some("bbb")), "bbb");
    assert_eq!(posix::basename("aaa/bbb", Some("bb")), "b");
    assert_eq!(posix::basename("aaa/bbb", Some("b")), "bb");
    assert_eq!(posix::basename("/aaa/bbb", Some("/bbb")), "bbb");
    assert_eq!(posix::basename("/aaa/bbb", Some("a/bbb")), "bbb");
    assert_eq!(posix::basename("/aaa/bbb", Some("bbb")), "bbb");
    assert_eq!(posix::basename("/aaa/bbb//", Some("bbb")), "bbb");
    assert_eq!(posix::basename("/aaa/bbb", Some("bb")), "b");
    assert_eq!(posix::basename("/aaa/bbb", Some("b")), "bb");
    assert_eq!(posix::basename("/aaa/bbb", None), "bbb");
    assert_eq!(posix::basename("/aaa/", None), "aaa");
    assert_eq!(posix::basename("/aaa/b", None), "b");
    assert_eq!(posix::basename("/a/b", None), "b");
    assert_eq!(posix::basename("//a", None), "a");
    assert_eq!(posix::basename("a", Some("a")), "");

    // On Windows a backslash acts as a path separator.
    let w = win32::basename;
    assert_eq!(w("\\dir\\basename.ext", None), "basename.ext");
    assert_eq!(w("\\basename.ext", None), "basename.ext");
    assert_eq!(w("basename.ext", None), "basename.ext");
    assert_eq!(w("basename.ext\\", None), "basename.ext");
    assert_eq!(w("basename.ext\\\\", None), "basename.ext");
    assert_eq!(w("foo", None), "foo");
    assert_eq!(w("aaa\\bbb", Some("\\bbb")), "bbb");
    assert_eq!(w("aaa\\bbb", Some("a\\bbb")), "bbb");
    assert_eq!(w("aaa\\bbb", Some("bbb")), "bbb");
    assert_eq!(w("aaa\\bbb\\\\\\\\", Some("bbb")), "bbb");
    assert_eq!(w("aaa\\bbb", Some("bb")), "b");
    assert_eq!(w("aaa\\bbb", Some("b")), "bb");
    assert_eq!(w("C:", None), "");
    assert_eq!(w("C:.", None), ".");
    assert_eq!(w("C:\\", None), "");
    assert_eq!(w("C:\\dir\\base.ext", None), "base.ext");
    assert_eq!(w("C:\\basename.ext", None), "basename.ext");
    assert_eq!(w("C:basename.ext", None), "basename.ext");
    assert_eq!(w("C:basename.ext\\", None), "basename.ext");
    assert_eq!(w("C:basename.ext\\\\", None), "basename.ext");
    assert_eq!(w("C:foo", None), "foo");
    assert_eq!(w("file:stream", None), "file:stream");
    assert_eq!(w("a", Some("a")), "");

    // On POSIX a backslash is an ordinary character.
    assert_eq!(
        posix::basename("\\dir\\basename.ext", None),
        "\\dir\\basename.ext"
    );
    assert_eq!(posix::basename("\\basename.ext", None), "\\basename.ext");
    assert_eq!(posix::basename("basename.ext", None), "basename.ext");
    assert_eq!(posix::basename("basename.ext\\", None), "basename.ext\\");
    assert_eq!(
        posix::basename("basename.ext\\\\", None),
        "basename.ext\\\\"
    );
    assert_eq!(posix::basename("foo", None), "foo");

    // POSIX filenames may include control characters.
    let control = format!("Icon{}", '\r');
    assert_eq!(posix::basename(&format!("/a/b/{control}"), None), control);

    // Tests from VS Code.
    assert_eq!(posix::basename("foo/bar", None), "bar");
    assert_eq!(posix::basename("foo\\bar", None), "foo\\bar");
    assert_eq!(win32::basename("foo\\bar", None), "bar");
    assert_eq!(posix::basename("/foo/bar", None), "bar");
    assert_eq!(posix::basename("\\foo\\bar", None), "\\foo\\bar");
    assert_eq!(win32::basename("\\foo\\bar", None), "bar");
    assert_eq!(posix::basename("./bar", None), "bar");
    assert_eq!(posix::basename(".\\bar", None), ".\\bar");
    assert_eq!(win32::basename(".\\bar", None), "bar");
    assert_eq!(posix::basename("/bar", None), "bar");
    assert_eq!(posix::basename("\\bar", None), "\\bar");
    assert_eq!(win32::basename("\\bar", None), "bar");
    assert_eq!(posix::basename("bar/", None), "bar");
    assert_eq!(posix::basename("bar\\", None), "bar\\");
    assert_eq!(win32::basename("bar\\", None), "bar");
    assert_eq!(posix::basename("bar", None), "bar");
    assert_eq!(posix::basename("////////", None), "");
    assert_eq!(posix::basename("\\\\\\\\", None), "\\\\\\\\");
    assert_eq!(win32::basename("\\\\\\\\", None), "");
}

#[test]
fn relative() {
    let win_cases: &[(&str, &str, &str)] = &[
        ("c:/blah\\blah", "d:/games", "d:\\games"),
        ("c:/aaaa/bbbb", "c:/aaaa", ".."),
        ("c:/aaaa/bbbb", "c:/cccc", "..\\..\\cccc"),
        ("c:/aaaa/bbbb", "c:/aaaa/bbbb", ""),
        ("c:/aaaa/bbbb", "c:/aaaa/cccc", "..\\cccc"),
        ("c:/aaaa/", "c:/aaaa/cccc", "cccc"),
        ("c:/", "c:\\aaaa\\bbbb", "aaaa\\bbbb"),
        ("c:/aaaa/bbbb", "d:\\", "d:\\"),
        ("c:/AaAa/bbbb", "c:/aaaa/bbbb", ""),
        ("c:/aaaaa/", "c:/aaaa/cccc", "..\\aaaa\\cccc"),
        ("C:\\foo\\bar\\baz\\quux", "C:\\", "..\\..\\..\\.."),
        (
            "C:\\foo\\test",
            "C:\\foo\\test\\bar\\package.json",
            "bar\\package.json",
        ),
        ("C:\\foo\\bar\\baz-quux", "C:\\foo\\bar\\baz", "..\\baz"),
        (
            "C:\\foo\\bar\\baz",
            "C:\\foo\\bar\\baz-quux",
            "..\\baz-quux",
        ),
        ("\\\\foo\\bar", "\\\\foo\\bar\\baz", "baz"),
        ("\\\\foo\\bar\\baz", "\\\\foo\\bar", ".."),
        ("\\\\foo\\bar\\baz-quux", "\\\\foo\\bar\\baz", "..\\baz"),
        (
            "\\\\foo\\bar\\baz",
            "\\\\foo\\bar\\baz-quux",
            "..\\baz-quux",
        ),
        ("C:\\baz-quux", "C:\\baz", "..\\baz"),
        ("C:\\baz", "C:\\baz-quux", "..\\baz-quux"),
        ("\\\\foo\\baz-quux", "\\\\foo\\baz", "..\\baz"),
        ("\\\\foo\\baz", "\\\\foo\\baz-quux", "..\\baz-quux"),
        ("C:\\baz", "\\\\foo\\bar\\baz", "\\\\foo\\bar\\baz"),
        ("\\\\foo\\bar\\baz", "C:\\baz", "C:\\baz"),
    ];
    for (from, to, expected) in win_cases {
        assert_eq!(
            win32::relative("C:\\cwd", from, to),
            *expected,
            "win32.relative({from:?}, {to:?})"
        );
    }

    let posix_cases: &[(&str, &str, &str)] = &[
        ("/var/lib", "/var", ".."),
        ("/var/lib", "/bin", "../../bin"),
        ("/var/lib", "/var/lib", ""),
        ("/var/lib", "/var/apache", "../apache"),
        ("/var/", "/var/lib", "lib"),
        ("/", "/var/lib", "var/lib"),
        (
            "/foo/test",
            "/foo/test/bar/package.json",
            "bar/package.json",
        ),
        ("/Users/a/web/b/test/mails", "/Users/a/web/b", "../.."),
        ("/foo/bar/baz-quux", "/foo/bar/baz", "../baz"),
        ("/foo/bar/baz", "/foo/bar/baz-quux", "../baz-quux"),
        ("/baz-quux", "/baz", "../baz"),
        ("/baz", "/baz-quux", "../baz-quux"),
    ];
    for (from, to, expected) in posix_cases {
        assert_eq!(
            posix::relative("/unused", from, to),
            *expected,
            "posix.relative({from:?}, {to:?})"
        );
    }
}

#[test]
fn normalize() {
    let w = win32::normalize;
    assert_eq!(w("./fixtures///b/../b/c.js"), "fixtures\\b\\c.js");
    assert_eq!(w("/foo/../../../bar"), "\\bar");
    assert_eq!(w("a//b//../b"), "a\\b");
    assert_eq!(w("a//b//./c"), "a\\b\\c");
    assert_eq!(w("a//b//."), "a\\b");
    assert_eq!(
        w("//server/share/dir/file.ext"),
        "\\\\server\\share\\dir\\file.ext"
    );
    assert_eq!(w("/a/b/c/../../../x/y/z"), "\\x\\y\\z");
    assert_eq!(w("C:"), "C:.");
    assert_eq!(w("C:..\\abc"), "C:..\\abc");
    assert_eq!(w("C:..\\..\\abc\\..\\def"), "C:..\\..\\def");
    assert_eq!(w("C:\\."), "C:\\");
    assert_eq!(w("file:stream"), "file:stream");
    assert_eq!(w("bar\\foo..\\..\\"), "bar\\");
    assert_eq!(w("bar\\foo..\\.."), "bar");
    assert_eq!(w("bar\\foo..\\..\\baz"), "bar\\baz");
    assert_eq!(w("bar\\foo..\\"), "bar\\foo..\\");
    assert_eq!(w("bar\\foo.."), "bar\\foo..");
    assert_eq!(w("..\\foo..\\..\\..\\bar"), "..\\..\\bar");
    assert_eq!(w("..\\...\\..\\.\\...\\..\\..\\bar"), "..\\..\\bar");
    assert_eq!(w("../../../foo/../../../bar"), "..\\..\\..\\..\\..\\bar");
    assert_eq!(
        w("../../../foo/../../../bar/../../"),
        "..\\..\\..\\..\\..\\..\\"
    );
    assert_eq!(w("../foobar/barfoo/foo/../../../bar/../../"), "..\\..\\");
    assert_eq!(
        w("../.../../foobar/../../../bar/../../baz"),
        "..\\..\\..\\..\\baz"
    );
    assert_eq!(w("foo/bar\\baz"), "foo\\bar\\baz");

    let p = posix::normalize;
    assert_eq!(p("./fixtures///b/../b/c.js"), "fixtures/b/c.js");
    assert_eq!(p("/foo/../../../bar"), "/bar");
    assert_eq!(p("a//b//../b"), "a/b");
    assert_eq!(p("a//b//./c"), "a/b/c");
    assert_eq!(p("a//b//."), "a/b");
    assert_eq!(p("/a/b/c/../../../x/y/z"), "/x/y/z");
    assert_eq!(p("///..//./foo/.//bar"), "/foo/bar");
    assert_eq!(p("bar/foo../../"), "bar/");
    assert_eq!(p("bar/foo../.."), "bar");
    assert_eq!(p("bar/foo../../baz"), "bar/baz");
    assert_eq!(p("bar/foo../"), "bar/foo../");
    assert_eq!(p("bar/foo.."), "bar/foo..");
    assert_eq!(p("../foo../../../bar"), "../../bar");
    assert_eq!(p("../.../.././.../../../bar"), "../../bar");
    assert_eq!(p("../../../foo/../../../bar"), "../../../../../bar");
    assert_eq!(p("../../../foo/../../../bar/../../"), "../../../../../../");
    assert_eq!(p("../foobar/barfoo/foo/../../../bar/../../"), "../../");
    assert_eq!(
        p("../.../../foobar/../../../bar/../../baz"),
        "../../../../baz"
    );
    assert_eq!(p("foo/bar\\baz"), "foo/bar\\baz");
}

#[test]
fn is_absolute() {
    assert!(win32::is_absolute("/"));
    assert!(win32::is_absolute("//"));
    assert!(win32::is_absolute("//server"));
    assert!(win32::is_absolute("//server/file"));
    assert!(win32::is_absolute("\\\\server\\file"));
    assert!(win32::is_absolute("\\\\server"));
    assert!(win32::is_absolute("\\\\"));
    assert!(!win32::is_absolute("c"));
    assert!(!win32::is_absolute("c:"));
    assert!(win32::is_absolute("c:\\"));
    assert!(win32::is_absolute("c:/"));
    assert!(win32::is_absolute("c://"));
    assert!(win32::is_absolute("C:/Users/"));
    assert!(win32::is_absolute("C:\\Users\\"));
    assert!(!win32::is_absolute("C:cwd/another"));
    assert!(!win32::is_absolute("C:cwd\\another"));
    assert!(!win32::is_absolute("directory/directory"));
    assert!(!win32::is_absolute("directory\\directory"));

    assert!(posix::is_absolute("/home/foo"));
    assert!(posix::is_absolute("/home/foo/.."));
    assert!(!posix::is_absolute("bar/"));
    assert!(!posix::is_absolute("./baz"));

    // Tests from VS Code.
    for absolute in [
        "C:/",
        "C:\\",
        "C:/foo",
        "C:\\foo",
        "z:/foo/bar.txt",
        "z:\\foo\\bar.txt",
        "\\\\localhost\\c$\\foo",
        "/",
        "/foo",
    ] {
        assert!(win32::is_absolute(absolute), "{absolute}");
    }
    for absolute in ["/", "/foo", "/foo/bar.txt"] {
        assert!(posix::is_absolute(absolute), "{absolute}");
    }
    for relative in ["", "foo", "foo/bar", "./foo", "http://foo.com/bar"] {
        assert!(!win32::is_absolute(relative), "{relative}");
    }
    for relative in [
        "",
        "foo",
        "foo/bar",
        "./foo",
        "http://foo.com/bar",
        "z:/foo/bar.txt",
    ] {
        assert!(!posix::is_absolute(relative), "{relative}");
    }
}

/// `path`: the separator and delimiter constants.
#[test]
fn sep_and_delimiter() {
    assert_eq!(win32::SEP, '\\');
    assert_eq!(posix::SEP, '/');
    assert_eq!(win32::DELIMITER, ';');
    assert_eq!(posix::DELIMITER, ':');
}

/// Node's documented `path.parse` and `path.format` examples.
#[test]
fn parse_and_format() {
    let parsed = posix::parse("/home/user/dir/file.txt");
    assert_eq!(
        parsed,
        ParsedPath {
            root: "/".into(),
            dir: "/home/user/dir".into(),
            base: "file.txt".into(),
            ext: ".txt".into(),
            name: "file".into(),
        }
    );
    assert_eq!(posix::format(&parsed), "/home/user/dir/file.txt");

    let parsed = win32::parse("C:\\path\\dir\\file.txt");
    assert_eq!(
        parsed,
        ParsedPath {
            root: "C:\\".into(),
            dir: "C:\\path\\dir".into(),
            base: "file.txt".into(),
            ext: ".txt".into(),
            name: "file".into(),
        }
    );
    assert_eq!(win32::format(&parsed), "C:\\path\\dir\\file.txt");

    // A base name without an extension, and a root-only path.
    assert_eq!(
        posix::parse("/"),
        ParsedPath {
            root: "/".into(),
            dir: "/".into(),
            ..Default::default()
        }
    );
    assert_eq!(
        posix::parse("relative/name"),
        ParsedPath {
            dir: "relative".into(),
            base: "name".into(),
            name: "name".into(),
            ..Default::default()
        }
    );
    assert_eq!(
        win32::parse("\\\\server\\share\\dir\\f.md"),
        ParsedPath {
            root: "\\\\server\\share\\".into(),
            dir: "\\\\server\\share\\dir".into(),
            base: "f.md".into(),
            ext: ".md".into(),
            name: "f".into(),
        }
    );

    // format() adds the dot when the extension lacks one, and uses name + ext when base is empty.
    let parts = ParsedPath {
        dir: "/tmp".into(),
        name: "a".into(),
        ext: "txt".into(),
        ..Default::default()
    };
    assert_eq!(posix::format(&parts), "/tmp/a.txt");
}

/// Node's documented `path.toNamespacedPath` examples.
#[test]
fn to_namespaced_path() {
    assert_eq!(
        win32::to_namespaced_path("C:\\cwd", "C:\\foo"),
        "\\\\?\\C:\\foo"
    );
    assert_eq!(
        win32::to_namespaced_path("C:\\cwd", "\\\\server\\share\\x"),
        "\\\\?\\UNC\\server\\share\\x"
    );
    assert_eq!(
        win32::to_namespaced_path("C:\\cwd", "\\\\?\\C:\\foo"),
        "\\\\?\\C:\\foo"
    );
    assert_eq!(posix::to_namespaced_path("/a/b"), "/a/b");
}

//! Ported from `src/vs/base/test/common/path.test.ts`. Test names match the upstream `test(...)`
//! names. This file holds the POSIX halves; the win32 halves follow in a stacked PR.

use super::{ParsedPath, posix};

type JoinCase = (&'static [&'static str], &'static str);

/// Cases run with `posix::join`.
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

#[test]
fn join() {
    for (args, expected) in JOIN_CASES {
        assert_eq!(posix::join(args), *expected, "posix.join({args:?})");
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

    // Tests from VS Code.
    let cases: &[(&str, &str)] = &[
        ("foo/bar", "foo"),
        ("/foo/bar", "/foo"),
        ("/foo", "/"),
        ("/", "/"),
        ("foo", "."),
        ("f", "."),
        ("f/", "."),
        ("/folder/", "/"),
    ];
    for (p, expected) in cases {
        assert_eq!(posix::dirname(p), *expected, "dirname({p:?})");
    }
}

type ExtCase = (&'static str, &'static str);

/// `extname` cases for the POSIX flavor.
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
    }

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
    let cases: &[(&[&str], &str)] = &[
        (&["/var/lib", "../", "file/"], "/var/file"),
        (&["/var/lib", "/../", "file/"], "/file"),
        (&["/some/dir", ".", "/absolute/"], "/absolute"),
        (
            &["/foo/tmp.3/", "../tmp.3/cycles/root.js"],
            "/foo/tmp.3/cycles/root.js",
        ),
    ];
    for (args, expected) in cases {
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
    assert_eq!(posix::basename("/foo/bar", None), "bar");
    assert_eq!(posix::basename("\\foo\\bar", None), "\\foo\\bar");
    assert_eq!(posix::basename("./bar", None), "bar");
    assert_eq!(posix::basename(".\\bar", None), ".\\bar");
    assert_eq!(posix::basename("/bar", None), "bar");
    assert_eq!(posix::basename("\\bar", None), "\\bar");
    assert_eq!(posix::basename("bar/", None), "bar");
    assert_eq!(posix::basename("bar\\", None), "bar\\");
    assert_eq!(posix::basename("bar", None), "bar");
    assert_eq!(posix::basename("////////", None), "");
    assert_eq!(posix::basename("\\\\\\\\", None), "\\\\\\\\");
}

#[test]
fn relative() {
    let cases: &[(&str, &str, &str)] = &[
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
    for (from, to, expected) in cases {
        assert_eq!(
            posix::relative("/unused", from, to),
            *expected,
            "posix.relative({from:?}, {to:?})"
        );
    }
}

#[test]
fn normalize() {
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
    assert!(posix::is_absolute("/home/foo"));
    assert!(posix::is_absolute("/home/foo/.."));
    assert!(!posix::is_absolute("bar/"));
    assert!(!posix::is_absolute("./baz"));

    // Tests from VS Code.
    for absolute in ["/", "/foo", "/foo/bar.txt"] {
        assert!(posix::is_absolute(absolute), "{absolute}");
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

/// `path`: the POSIX separator and delimiter.
#[test]
fn sep_and_delimiter() {
    assert_eq!(posix::SEP, '/');
    assert_eq!(posix::DELIMITER, ':');
}

/// Node's documented `path.parse` and `path.format` examples, for POSIX.
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

    // format() adds the dot when the extension lacks one, and uses name + ext when base is empty.
    let parts = ParsedPath {
        dir: "/tmp".into(),
        name: "a".into(),
        ext: "txt".into(),
        ..Default::default()
    };
    assert_eq!(posix::format(&parts), "/tmp/a.txt");
}

#[test]
fn to_namespaced_path() {
    assert_eq!(posix::to_namespaced_path("/a/b"), "/a/b");
}

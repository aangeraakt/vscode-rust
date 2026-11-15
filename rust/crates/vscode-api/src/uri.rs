//! Uniform resource identifiers with VS Code's parsing, formatting and file-path semantics.
//!
//! Ported from `src/vs/base/common/uri.ts` (`URI.parse`, `URI.file`, `URI.from`, `fsPath`,
//! `toString`, `_validateUri`, `percentDecode`, `encodeURIComponentFast`). Test cases come from
//! `src/vs/base/test/common/uri.test.ts`.
//!
//! Components are stored decoded, as in TypeScript. `Display` produces the encoded form
//! (`toString()`); [`Uri::to_string_skip_encoding`] matches `toString(true)`.
//! Windows-specific behavior (backslash handling in `file`, separators in `fs_path`) follows
//! the build target, as TypeScript follows `isWindows`.

use std::fmt;

/// A parsed URI: `scheme://authority/path?query#fragment`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Uri {
    scheme: String,
    authority: String,
    path: String,
    query: String,
    fragment: String,
}

/// A URI that violates RFC 3986 the way `_validateUri` checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriError {
    /// The scheme contains characters outside `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`.
    IllegalScheme(String),
    /// An authority is present but the path does not start with `/`.
    PathMustStartWithSlash,
    /// No authority is present but the path starts with `//`.
    PathMustNotStartWithDoubleSlash,
}

impl fmt::Display for UriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UriError::IllegalScheme(s) => write!(f, "[UriError]: Scheme contains illegal characters: {s:?}"),
            UriError::PathMustStartWithSlash => f.write_str("[UriError]: If a URI contains an authority component, then the path component must either be empty or begin with a slash (\"/\") character"),
            UriError::PathMustNotStartWithDoubleSlash => f.write_str("[UriError]: If a URI does not contain an authority component, then the path cannot begin with two slash characters (\"//\")"),
        }
    }
}

impl std::error::Error for UriError {}

impl Uri {
    /// Builds a URI from decoded components (`URI.from`). An empty scheme becomes `file`, and the
    /// path of `http`, `https` and `file` URIs gets a leading `/`.
    pub fn from_parts(
        scheme: &str,
        authority: &str,
        path: &str,
        query: &str,
        fragment: &str,
    ) -> Result<Self, UriError> {
        let scheme = if scheme.is_empty() { "file" } else { scheme };
        let path = match scheme {
            "http" | "https" | "file" if !path.starts_with('/') => format!("/{path}"),
            _ => path.to_owned(),
        };
        let uri = Uri {
            scheme: scheme.to_owned(),
            authority: authority.to_owned(),
            path,
            query: query.to_owned(),
            fragment: fragment.to_owned(),
        };
        uri.validate()?;
        Ok(uri)
    }

    /// Parses an encoded URI string (`URI.parse` in non-strict mode).
    pub fn parse(value: &str) -> Result<Self, UriError> {
        // Equivalent to /^(([^:/?#]+?):)?(\/\/([^/?#]*))?([^?#]*)(\?([^#]*))?(#(.*))?/
        let mut rest = value;
        let mut scheme = "";
        if let Some(i) = rest.find([':', '/', '?', '#'])
            && i > 0
            && rest.as_bytes()[i] == b':'
        {
            scheme = &rest[..i];
            rest = &rest[i + 1..];
        }
        let mut authority = "";
        if let Some(after) = rest.strip_prefix("//") {
            let end = after.find(['/', '?', '#']).unwrap_or(after.len());
            authority = &after[..end];
            rest = &after[end..];
        }
        let path_end = rest.find(['?', '#']).unwrap_or(rest.len());
        let path = &rest[..path_end];
        rest = &rest[path_end..];
        let mut query = "";
        if let Some(after) = rest.strip_prefix('?') {
            let end = after.find('#').unwrap_or(after.len());
            query = &after[..end];
            rest = &after[end..];
        }
        let fragment = rest.strip_prefix('#').unwrap_or("");
        Self::from_parts(
            scheme,
            &percent_decode(authority),
            &percent_decode(path),
            &percent_decode(query),
            &percent_decode(fragment),
        )
    }

    /// Creates a `file` URI from a file-system path (`URI.file`). UNC paths (`//server/share`)
    /// put the server in the authority. On Windows, backslashes become slashes.
    pub fn file(path: &str) -> Self {
        let mut path = if cfg!(windows) {
            path.replace('\\', "/")
        } else {
            path.to_owned()
        };
        let mut authority = String::new();
        if path.starts_with("//") {
            match path[2..].find('/') {
                None => {
                    authority = path[2..].to_owned();
                    path = "/".to_owned();
                }
                Some(i) => {
                    authority = path[2..2 + i].to_owned();
                    path = path[2 + i..].to_owned();
                }
            }
        }
        if !path.starts_with('/') {
            path.insert(0, '/');
        }
        Uri {
            scheme: "file".to_owned(),
            authority,
            path,
            query: String::new(),
            fragment: String::new(),
        }
    }

    /// The scheme, e.g. `file`.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }
    /// The decoded authority, e.g. `www.example.com`.
    pub fn authority(&self) -> &str {
        &self.authority
    }
    /// The decoded path, e.g. `/some/path`.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// The decoded query.
    pub fn query(&self) -> &str {
        &self.query
    }
    /// The decoded fragment.
    pub fn fragment(&self) -> &str {
        &self.fragment
    }

    /// The platform file-system path (`fsPath`): handles UNC authorities, lower-cases Windows drive
    /// letters and uses `\` on Windows. Does not look at the scheme beyond the UNC check.
    pub fn fs_path(&self) -> String {
        let b = self.path.as_bytes();
        let value = if !self.authority.is_empty() && self.path.len() > 1 && self.scheme == "file" {
            format!("//{}{}", self.authority, self.path)
        } else if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
            format!("{}{}", (b[1] as char).to_ascii_lowercase(), &self.path[2..])
        } else {
            self.path.clone()
        };
        if cfg!(windows) {
            value.replace('/', "\\")
        } else {
            value
        }
    }

    /// The external form with only `?` and `#` in the path escaped (`toString(true)`).
    pub fn to_string_skip_encoding(&self) -> String {
        self.format(false)
    }

    fn validate(&self) -> Result<(), UriError> {
        let s = self.scheme.as_bytes();
        let scheme_ok = s[0].is_ascii_alphanumeric() || s[0] == b'_';
        let rest_ok = s[1..]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'+' | b'.' | b'-'));
        if !scheme_ok || !rest_ok {
            return Err(UriError::IllegalScheme(self.scheme.clone()));
        }
        if !self.path.is_empty() {
            if !self.authority.is_empty() {
                if !self.path.starts_with('/') {
                    return Err(UriError::PathMustStartWithSlash);
                }
            } else if self.path.starts_with("//") {
                return Err(UriError::PathMustNotStartWithDoubleSlash);
            }
        }
        Ok(())
    }

    fn format(&self, encode: bool) -> String {
        let enc = |s: &str, is_path: bool, is_authority: bool| {
            if encode {
                encode_component(s, is_path, is_authority)
            } else {
                encode_minimal(s)
            }
        };
        let mut res = String::new();
        if !self.scheme.is_empty() {
            res.push_str(&self.scheme);
            res.push(':');
        }
        if !self.authority.is_empty() || self.scheme == "file" {
            res.push_str("//");
        }
        if !self.authority.is_empty() {
            let mut authority = self.authority.as_str();
            if let Some(at) = authority.find('@') {
                let userinfo = &authority[..at];
                authority = &authority[at + 1..];
                match userinfo.find(':') {
                    None => res.push_str(&enc(userinfo, false, false)),
                    Some(colon) => {
                        res.push_str(&enc(&userinfo[..colon], false, false));
                        res.push(':');
                        res.push_str(&enc(&userinfo[colon + 1..], false, false));
                    }
                }
                res.push('@');
            }
            let authority = authority.to_lowercase();
            match authority.rfind(':') {
                None => res.push_str(&enc(&authority, false, true)),
                Some(colon) => {
                    res.push_str(&enc(&authority[..colon], false, true));
                    res.push_str(&authority[colon..]);
                }
            }
        }
        if !self.path.is_empty() {
            let mut path = self.path.clone();
            let lower_scheme = self.scheme.to_ascii_lowercase();
            let http = lower_scheme == "http" || lower_scheme == "https";
            let b = path.as_bytes();
            if b.len() >= 3 && b[0] == b'/' && b[2] == b':' {
                if b[1].is_ascii_uppercase() && !http {
                    path.replace_range(1..2, &(b[1] as char).to_ascii_lowercase().to_string());
                }
            } else if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_uppercase() && !http {
                path.replace_range(0..1, &(b[0] as char).to_ascii_lowercase().to_string());
            }
            res.push_str(&enc(&path, true, false));
        }
        if !self.query.is_empty() {
            res.push('?');
            res.push_str(&enc(&self.query, false, false));
        }
        if !self.fragment.is_empty() {
            res.push('#');
            if encode {
                res.push_str(&encode_component(&self.fragment, false, false));
            } else {
                res.push_str(&self.fragment);
            }
        }
        res
    }
}

impl fmt::Display for Uri {
    /// The encoded external form (`toString()`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format(true))
    }
}

/// `encodeTable` from uri.ts: reserved characters that are always escaped with these spellings.
fn table_escape(c: char) -> Option<&'static str> {
    Some(match c {
        ':' => "%3A",
        '/' => "%2F",
        '?' => "%3F",
        '#' => "%23",
        '[' => "%5B",
        ']' => "%5D",
        '@' => "%40",
        '!' => "%21",
        '$' => "%24",
        '&' => "%26",
        '\'' => "%27",
        '(' => "%28",
        ')' => "%29",
        '*' => "%2A",
        '+' => "%2B",
        ',' => "%2C",
        ';' => "%3B",
        '=' => "%3D",
        ' ' => "%20",
        _ => return None,
    })
}

/// `encodeURIComponentFast`: unreserved characters stay, table characters use the table, and
/// everything else is UTF-8 percent-encoded as `encodeURIComponent` would.
fn encode_component(s: &str, is_path: bool, is_authority: bool) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        let keep = c.is_ascii_alphanumeric()
            || matches!(c, '-' | '.' | '_' | '~')
            || (is_path && c == '/')
            || (is_authority && matches!(c, '[' | ']' | ':'));
        if keep {
            res.push(c);
        } else if let Some(escaped) = table_escape(c) {
            res.push_str(escaped);
        } else {
            let mut buf = [0u8; 4];
            for byte in c.encode_utf8(&mut buf).bytes() {
                res.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    res
}

/// `encodeURIComponentMinimal`: escapes only `?` and `#`.
fn encode_minimal(s: &str) -> String {
    s.replace('?', "%3F").replace('#', "%23")
}

/// `percentDecode`: decodes each maximal run of `%XX` triplets as UTF-8, keeping triplets that do
/// not form valid UTF-8 (`decodeURIComponentGraceful`).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let is_triplet = |i: usize| {
        i + 2 < bytes.len()
            && bytes[i] == b'%'
            && bytes[i + 1].is_ascii_alphanumeric()
            && bytes[i + 2].is_ascii_alphanumeric()
    };
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if !is_triplet(i) {
            let ch = s[i..].chars().next().expect("index is on a char boundary");
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        let start = i;
        while is_triplet(i) {
            i += 3;
        }
        decode_run_graceful(&s[start..i], &mut out);
    }
    out
}

fn decode_run_graceful(run: &str, out: &mut String) {
    let decoded: Option<Vec<u8>> = run
        .as_bytes()
        .chunks(3)
        .map(|t| u8::from_str_radix(std::str::from_utf8(&t[1..]).ok()?, 16).ok())
        .collect();
    match decoded.and_then(|bytes| String::from_utf8(bytes).ok()) {
        Some(text) => out.push_str(&text),
        None if run.len() > 3 => {
            out.push_str(&run[..3]);
            decode_run_graceful(&run[3..], out);
        }
        None => out.push_str(run),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Uri {
        Uri::parse(s).unwrap()
    }

    #[test]
    fn file_to_string() {
        assert_eq!(
            Uri::file("c:/win/path").to_string(),
            "file:///c%3A/win/path"
        );
        assert_eq!(
            Uri::file("C:/win/path").to_string(),
            "file:///c%3A/win/path"
        );
        assert_eq!(Uri::file("/usr/home").to_string(), "file:///usr/home");
        assert_eq!(Uri::file("/foo/bar").path(), "/foo/bar");
        assert_eq!(Uri::file("/./foo/bar").path(), "/./foo/bar");
    }

    #[test]
    fn file_backslashes_follow_platform() {
        let uri = Uri::file("c:\\win\\path");
        if cfg!(windows) {
            assert_eq!(uri.to_string(), "file:///c%3A/win/path");
            assert_eq!(uri.fs_path(), "c:\\win\\path");
        } else {
            assert_eq!(uri.to_string(), "file:///c%3A%5Cwin%5Cpath");
        }
    }

    #[test]
    fn unc_paths() {
        let uri = Uri::file("//shares/files/c#/p.cs");
        assert_eq!(uri.authority(), "shares");
        assert_eq!(uri.path(), "/files/c#/p.cs");
        assert_eq!(uri.to_string(), "file://shares/files/c%23/p.cs");
        let expected = if cfg!(windows) {
            "\\\\shares\\files\\c#\\p.cs"
        } else {
            "//shares/files/c#/p.cs"
        };
        assert_eq!(uri.fs_path(), expected);
    }

    #[test]
    fn from_parts_to_string() {
        let u = |s, a, p, q| Uri::from_parts(s, a, p, q, "").unwrap();
        assert_eq!(
            u("http", "www.example.com", "/my/path", "").to_string(),
            "http://www.example.com/my/path"
        );
        assert_eq!(
            u("http", "example.com", "/", "test=true").to_string(),
            "http://example.com/?test%3Dtrue"
        );
        assert_eq!(
            u("http", "", "/api/files/test.me", "t=1234").to_string_skip_encoding(),
            "http:/api/files/test.me?t=1234"
        );
        assert_eq!(u("http", "a", "no-slash", "").path(), "/no-slash");
        assert_eq!(u("", "", "x", "").scheme(), "file");
    }

    #[test]
    fn parse_components() {
        let uri = parse("http://api/files/test.me?t=1234#frag");
        assert_eq!(
            (
                uri.scheme(),
                uri.authority(),
                uri.path(),
                uri.query(),
                uri.fragment()
            ),
            ("http", "api", "/files/test.me", "t=1234", "frag")
        );
        let uri = parse("file:///c:/test/me");
        assert_eq!((uri.authority(), uri.path()), ("", "/c:/test/me"));
        assert_eq!(
            uri.fs_path(),
            if cfg!(windows) {
                "c:\\test\\me"
            } else {
                "c:/test/me"
            }
        );
        assert_eq!(
            parse("untitled:c:/Users/jrieken/Code/abc.txt").to_string(),
            "untitled:c%3A/Users/jrieken/Code/abc.txt"
        );
    }

    #[test]
    fn percent_decoding_is_graceful() {
        assert_eq!(
            parse("file://some/%.txt").to_string(),
            "file://some/%25.txt"
        );
        assert_eq!(
            parse("file://some/%A0.txt").to_string(),
            "file://some/%25A0.txt"
        );
        assert_eq!(parse("file:///a%20b/%C3%A4").path(), "/a b/ä");
        assert_eq!(
            parse("file:///a%20b/%C3%A4").to_string(),
            "file:///a%20b/%C3%A4"
        );
    }

    #[test]
    fn authority_with_userinfo_and_ipv6() {
        assert_eq!(
            parse("http://user@[FEDC:BA98:7654:3210:FEDC:BA98:7654:3210]:80/index.html")
                .to_string(),
            "http://user@[fedc:ba98:7654:3210:fedc:ba98:7654:3210]:80/index.html"
        );
        assert_eq!(
            parse("http://us[er@[FEDC:BA98:7654:3210:FEDC:BA98:7654:3210]:80/index.html")
                .to_string(),
            "http://us%5Ber@[fedc:ba98:7654:3210:fedc:ba98:7654:3210]:80/index.html"
        );
    }

    #[test]
    fn validation() {
        assert!(matches!(
            Uri::parse("ht^tp://x/y"),
            Err(UriError::IllegalScheme(_))
        ));
        assert_eq!(
            Uri::from_parts("foo", "auth", "no-slash", "", ""),
            Err(UriError::PathMustStartWithSlash)
        );
        assert_eq!(
            Uri::from_parts("foo", "", "//double", "", ""),
            Err(UriError::PathMustNotStartWithDoubleSlash)
        );
    }

    #[test]
    fn round_trips_through_display() {
        for s in [
            "file:///c%3A/a%20b.txt",
            "https://example.com/p?q#f",
            "untitled:Untitled-1",
        ] {
            assert_eq!(parse(s).to_string(), s);
            assert_eq!(parse(&parse(s).to_string()), parse(s));
        }
    }
}

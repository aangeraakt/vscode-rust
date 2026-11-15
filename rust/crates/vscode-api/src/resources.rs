//! URI path helpers with a path-casing policy (`ExtUri`).
//!
//! Ported from `src/vs/base/common/resources.ts`, with the helpers it needs from `extpath.ts`
//! (`getRoot`, `isEqualOrParent`, `toSlashes`, `toPosixPath`) and `strings.ts`
//! (`equalsIgnoreCase`, `startsWithIgnoreCase`). Test cases come from
//! `src/vs/base/test/common/resources.test.ts` (see `resources/tests.rs`).
//!
//! Differences from the TypeScript:
//! - There is no `undefined`: an absent `Uri` is simply not passed.
//! - Default arguments are explicit: `ignore_fragment`, `force_posix_semantics`, and the path
//!   separator (`HOST_SEPARATOR` where TypeScript defaults to `paths.sep`).
//! - Methods that resolve relative paths take the working directory as `cwd`, as `path` does.
//! - Operations that TypeScript reports with a thrown error return [`ResourceError`]. In one
//!   `dirname` case TypeScript logs and falls back to `/`; this port falls back silently.
//! - `DataUri` and `toLocalResource` are not ported yet. They have no upstream tests.
//! - Lowercasing uses Rust's Unicode case mapping, which may differ from JavaScript for some
//!   unusual characters.

use std::cmp::Ordering;
use std::fmt;

use crate::path::{posix, win32};
use crate::uri::{Uri, UriError};

#[cfg(test)]
mod tests;

/// The path separator of the host platform (`paths.sep`).
pub const HOST_SEPARATOR: char = if cfg!(windows) { '\\' } else { '/' };

/// An error from a path operation on a URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceError {
    /// The URI has no path to join onto (`URI.joinPath` throws in that case).
    EmptyPath,
    /// The new components do not form a valid URI.
    InvalidUri(UriError),
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceError::EmptyPath => {
                f.write_str("[UriError]: cannot join a path onto a URI without a path")
            }
            ResourceError::InvalidUri(err) => fmt::Display::fmt(err, f),
        }
    }
}

impl std::error::Error for ResourceError {}

impl From<UriError> for ResourceError {
    fn from(err: UriError) -> Self {
        ResourceError::InvalidUri(err)
    }
}

type ResourceResult = Result<Uri, ResourceError>;

/// Path operations on URIs, with a policy for when path casing is ignored (`ExtUri`).
pub struct ExtUri {
    ignore_path_casing: Box<dyn Fn(&Uri) -> bool + Send + Sync>,
}

impl ExtUri {
    /// Creates an `ExtUri` with a path-casing policy (`new ExtUri(ignorePathCasing)`).
    pub fn new(ignore_path_casing: impl Fn(&Uri) -> bool + Send + Sync + 'static) -> Self {
        Self {
            ignore_path_casing: Box::new(ignore_path_casing),
        }
    }

    /// Paths are compared as they are (`extUri`).
    pub fn case_sensitive() -> Self {
        Self::new(|_| false)
    }

    /// Path casing is always ignored (`extUriIgnorePathCase`).
    pub fn ignore_path_case() -> Self {
        Self::new(|_| true)
    }

    /// Whether the path casing of `uri` is ignored.
    pub fn ignore_path_casing(&self, uri: &Uri) -> bool {
        (self.ignore_path_casing)(uri)
    }

    /// Orders two URIs by their comparison keys.
    pub fn compare(&self, uri1: &Uri, uri2: &Uri, ignore_fragment: bool) -> Ordering {
        units(&self.get_comparison_key(uri1, ignore_fragment))
            .cmp(&units(&self.get_comparison_key(uri2, ignore_fragment)))
    }

    /// Whether two URIs have the same comparison key.
    pub fn is_equal(&self, uri1: &Uri, uri2: &Uri, ignore_fragment: bool) -> bool {
        self.get_comparison_key(uri1, ignore_fragment)
            == self.get_comparison_key(uri2, ignore_fragment)
    }

    /// The string form of `uri` with the path lowercased when casing is ignored. Two URIs are
    /// equal exactly when their keys are equal.
    pub fn get_comparison_key(&self, uri: &Uri, ignore_fragment: bool) -> String {
        let path = if self.ignore_path_casing(uri) {
            uri.path().to_lowercase()
        } else {
            uri.path().to_owned()
        };
        let fragment = if ignore_fragment { "" } else { uri.fragment() };
        // Lowercasing keeps the components valid: the scheme, the authority and the leading slash
        // do not change.
        Uri::from_parts(uri.scheme(), uri.authority(), &path, uri.query(), fragment)
            .expect("lowercasing a valid path keeps it valid")
            .to_string()
    }

    /// Whether `parent_candidate` is `base` or one of its ancestors (`isEqualOrParent`). For a
    /// non-file URI, a Windows-style path is also checked with Windows semantics unless
    /// `force_posix_semantics` is set.
    pub fn is_equal_or_parent(
        &self,
        base: &Uri,
        parent_candidate: &Uri,
        ignore_fragment: bool,
        force_posix_semantics: bool,
    ) -> bool {
        if base.scheme() != parent_candidate.scheme() {
            return false;
        }
        if base.scheme() == "file" {
            return is_equal_or_parent_path(
                &original_fs_path(base),
                &original_fs_path(parent_candidate),
                self.ignore_path_casing(base),
                false,
            ) && base.query() == parent_candidate.query()
                && (ignore_fragment || base.fragment() == parent_candidate.fragment());
        }
        if !self.is_equal_authority(base.authority(), parent_candidate.authority()) {
            return false;
        }

        let ignore_case = self.ignore_path_casing(base);
        if !is_equal_or_parent_path(base.path(), parent_candidate.path(), ignore_case, true) {
            return false;
        }
        // Non-file URIs can still be backed by a Windows file system.
        if !force_posix_semantics
            && (base.path().contains('\\') || parent_candidate.path().contains('\\'))
        {
            let base_path = to_slashes(&win32::normalize(&original_fs_path(base)));
            let parent_path = to_slashes(&win32::normalize(&original_fs_path(parent_candidate)));
            if !is_equal_or_parent_path(&base_path, &parent_path, ignore_case, true) {
                return false;
            }
        }
        base.query() == parent_candidate.query()
            && (ignore_fragment || base.fragment() == parent_candidate.fragment())
    }

    /// The basename, or the authority when the path has no basename.
    pub fn basename_or_authority(&self, resource: &Uri) -> String {
        let basename = self.basename(resource);
        if basename.is_empty() {
            resource.authority().to_owned()
        } else {
            basename
        }
    }

    /// The last segment of the path (`path.posix.basename`).
    pub fn basename(&self, resource: &Uri) -> String {
        posix::basename(resource.path(), None)
    }

    /// The extension of the last segment of the path, including the dot.
    pub fn extname(&self, resource: &Uri) -> String {
        posix::extname(resource.path())
    }

    /// The URI of the directory containing `resource`. A `file` URI uses the host's path rules.
    pub fn dirname(&self, resource: &Uri) -> ResourceResult {
        if resource.path().is_empty() {
            return Ok(resource.clone());
        }
        let dirname = if resource.scheme() == "file" {
            Uri::file(&host_dirname(&original_fs_path(resource)))
                .path()
                .to_owned()
        } else {
            let dirname = posix::dirname(resource.path());
            // A URI with an authority needs an absolute path.
            if !resource.authority().is_empty() && !dirname.is_empty() && !dirname.starts_with('/')
            {
                "/".to_owned()
            } else {
                dirname
            }
        };
        with_path(resource, &dirname)
    }

    /// Appends `fragments` to the path and normalizes the result (`joinPath`). A `file` URI on
    /// Windows uses Windows path rules.
    pub fn join_path(&self, resource: &Uri, fragments: &[&str]) -> ResourceResult {
        if resource.path().is_empty() {
            return Err(ResourceError::EmptyPath);
        }
        let path = if cfg!(windows) && resource.scheme() == "file" {
            let fs_path = original_fs_path(resource);
            let mut parts: Vec<&str> = vec![fs_path.as_str()];
            parts.extend_from_slice(fragments);
            Uri::file(&win32::join(&parts)).path().to_owned()
        } else {
            let mut parts: Vec<&str> = vec![resource.path()];
            parts.extend_from_slice(fragments);
            posix::join(&parts)
        };
        with_path(resource, &path)
    }

    /// Resolves `.` and `..` in the path (`normalizePath`).
    pub fn normalize_path(&self, resource: &Uri) -> ResourceResult {
        if resource.path().is_empty() {
            return Ok(resource.clone());
        }
        let normalized = if resource.scheme() == "file" {
            Uri::file(&host_normalize(&original_fs_path(resource)))
                .path()
                .to_owned()
        } else {
            posix::normalize(resource.path())
        };
        with_path(resource, &normalized)
    }

    /// The path from `from` to `to`, or `None` when they differ in scheme or authority
    /// (`relativePath`). Relative paths resolve against `cwd`. When path casing is ignored for
    /// `from`, its casing is first made to match `to`.
    pub fn relative_path(&self, from: &Uri, to: &Uri, cwd: &str) -> Option<String> {
        if from.scheme() != to.scheme()
            || !self.is_equal_authority(from.authority(), to.authority())
        {
            return None;
        }
        if from.scheme() == "file" {
            let relative = host_relative(cwd, &original_fs_path(from), &original_fs_path(to));
            return Some(if cfg!(windows) {
                to_slashes(&relative)
            } else {
                relative
            });
        }

        let to_path = if to.path().is_empty() { "/" } else { to.path() };
        let mut from_path = if from.path().is_empty() {
            "/".to_owned()
        } else {
            from.path().to_owned()
        };
        if self.ignore_path_casing(from) {
            // Make the casing of `from_path` match `to_path`, up to the first real difference.
            let from_u = units(&from_path);
            let to_u = units(to_path);
            let len = from_u.len().min(to_u.len());
            let mut i = 0;
            while i < len {
                if from_u[i] != to_u[i] && !same_unit_ignoring_case(from_u[i], to_u[i]) {
                    break;
                }
                i += 1;
            }
            let mut joined = to_u[..i].to_vec();
            joined.extend_from_slice(&from_u[i..]);
            from_path = text(&joined);
        }
        Some(posix::relative(cwd, &from_path, to_path))
    }

    /// Resolves `path` against `base`. The path may be a POSIX or Windows path (`resolvePath`).
    /// Relative paths resolve against `cwd`.
    pub fn resolve_path(&self, base: &Uri, path: &str, cwd: &str) -> ResourceResult {
        if base.scheme() == "file" {
            let base_fs_path = original_fs_path(base);
            let resolved = Uri::file(&host_resolve(cwd, &[base_fs_path.as_str(), path]));
            return Ok(Uri::from_parts(
                base.scheme(),
                resolved.authority(),
                resolved.path(),
                base.query(),
                base.fragment(),
            )?);
        }
        let path = to_posix_path(path);
        with_path(base, &posix::resolve(cwd, &[base.path(), &path]))
    }

    /// Whether the path starts with `/` (`isAbsolutePath`).
    pub fn is_absolute_path(&self, resource: &Uri) -> bool {
        resource.path().starts_with('/')
    }

    /// Whether two authorities are equal, ignoring ASCII-insensitive case
    /// (`isEqualAuthority`).
    pub fn is_equal_authority(&self, a1: &str, a2: &str) -> bool {
        a1 == a2 || equals_ignore_case(a1, a2)
    }

    /// Whether the path ends with `sep`, ignoring a drive root (`hasTrailingPathSeparator`).
    pub fn has_trailing_path_separator(&self, resource: &Uri, sep: char) -> bool {
        if resource.scheme() == "file" {
            let fs_path = original_fs_path(resource);
            fs_path.len() > get_root(&fs_path, '/').len() && fs_path.ends_with(sep)
        } else {
            let path = resource.path();
            path.len() > 1 && path.ends_with('/') && !is_drive_root_fs_path(&resource.fs_path())
        }
    }

    /// Removes one trailing separator from the path, if there is one. The leading slash is kept
    /// (`removeTrailingPathSeparator`).
    pub fn remove_trailing_path_separator(&self, resource: &Uri, sep: char) -> ResourceResult {
        if self.has_trailing_path_separator(resource, sep) {
            let path = resource.path();
            return with_path(resource, &path[..path.len() - 1]);
        }
        Ok(resource.clone())
    }

    /// Adds a trailing `/` to the path unless it is already there or is a root
    /// (`addTrailingPathSeparator`). `c:\` stays as it is; `c:\users` becomes `c:\users\`.
    pub fn add_trailing_path_separator(&self, resource: &Uri, sep: char) -> ResourceResult {
        let (is_root_sep, sep) = if resource.scheme() == "file" {
            let fs_path = original_fs_path(resource);
            let is_root = fs_path.len() == get_root(&fs_path, '/').len() && fs_path.ends_with(sep);
            (is_root, sep)
        } else {
            (resource.path() == "/", '/')
        };
        if !is_root_sep && !self.has_trailing_path_separator(resource, sep) {
            return with_path(resource, &format!("{}/", resource.path()));
        }
        Ok(resource.clone())
    }
}

/// Items whose resource is not equal to, or inside, the resource of another item
/// (`distinctParents`). Uses case-sensitive comparison.
pub fn distinct_parents<T>(items: &[T], resource: impl Fn(&T) -> &Uri) -> Vec<&T> {
    let ext = ExtUri::case_sensitive();
    let mut distinct = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let candidate = resource(item);
        let has_parent = items.iter().enumerate().any(|(j, other)| {
            j != i && ext.is_equal_or_parent(candidate, resource(other), false, false)
        });
        if !has_parent {
            distinct.push(item);
        }
    }
    distinct
}

/// The file-system path of `uri` with the drive letter's case kept (`originalFSPath`, which is
/// `uriToFsPath(uri, true)`).
pub fn original_fs_path(uri: &Uri) -> String {
    let path = uri.path();
    let b = path.as_bytes();
    let mut value =
        if !uri.authority().is_empty() && units(path).len() > 1 && uri.scheme() == "file" {
            // UNC path: file://server/share/file
            format!("//{}{}", uri.authority(), path)
        } else if b.first() == Some(&b'/')
            && b.get(1).is_some_and(u8::is_ascii_alphabetic)
            && b.get(2) == Some(&b':')
        {
            // Drive letter: /c:/file. The leading slash goes; the letter keeps its case.
            path[1..].to_owned()
        } else {
            path.to_owned()
        };
    if cfg!(windows) {
        value = value.replace('/', "\\");
    }
    value
}

fn with_path(uri: &Uri, path: &str) -> ResourceResult {
    Ok(Uri::from_parts(
        uri.scheme(),
        uri.authority(),
        path,
        uri.query(),
        uri.fragment(),
    )?)
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn text(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

fn host_normalize(path: &str) -> String {
    if cfg!(windows) {
        win32::normalize(path)
    } else {
        posix::normalize(path)
    }
}

fn host_resolve(cwd: &str, segments: &[&str]) -> String {
    if cfg!(windows) {
        win32::resolve(cwd, segments)
    } else {
        posix::resolve(cwd, segments)
    }
}

fn host_dirname(path: &str) -> String {
    if cfg!(windows) {
        win32::dirname(path)
    } else {
        posix::dirname(path)
    }
}

fn host_relative(cwd: &str, from: &str, to: &str) -> String {
    if cfg!(windows) {
        win32::relative(cwd, from, to)
    } else {
        posix::relative(cwd, from, to)
    }
}

fn normalize_for(force_posix: bool, path: &str) -> String {
    if force_posix {
        posix::normalize(path)
    } else {
        host_normalize(path)
    }
}

fn is_path_sep(c: u8) -> bool {
    c == b'/' || c == b'\\'
}

fn byte_at(b: &[u8], i: usize) -> u8 {
    b.get(i).copied().unwrap_or(0)
}

/// `getRoot` (extpath.ts): the root of a path, with separators replaced by `sep`. Returns an
/// empty string when there is no root. Only ASCII positions are examined, so byte offsets are
/// valid string boundaries.
fn get_root(path: &str, sep: char) -> String {
    if path.is_empty() {
        return String::new();
    }
    let b = path.as_bytes();
    let len = b.len();
    let first = b[0];

    if is_path_sep(first) {
        if is_path_sep(byte_at(b, 1)) && !is_path_sep(byte_at(b, 2)) {
            // UNC candidate: \\localhost\shares\ddd
            let mut pos = 3;
            let start = pos;
            while pos < len && !is_path_sep(b[pos]) {
                pos += 1;
            }
            if start != pos && !is_path_sep(byte_at(b, pos + 1)) {
                pos += 1;
                while pos < len {
                    if is_path_sep(b[pos]) {
                        return path[..=pos]
                            .chars()
                            .map(|c| if c == '\\' || c == '/' { sep } else { c })
                            .collect();
                    }
                    pos += 1;
                }
            }
        }
        // /user/far
        return sep.to_string();
    }

    if first.is_ascii_alphabetic() && byte_at(b, 1) == b':' {
        // C:\fff or C:
        return if is_path_sep(byte_at(b, 2)) {
            format!("{}{sep}", &path[..2])
        } else {
            path[..2].to_owned()
        };
    }

    // scheme://authority/path
    if let Some(found) = path.find("://") {
        let mut pos = found + 3;
        while pos < len {
            if is_path_sep(b[pos]) {
                return path[..=pos].to_owned();
            }
            pos += 1;
        }
    }
    String::new()
}

/// `isEqualOrParent` (extpath.ts) on file-system or URI paths. `ignore_case` uses
/// case-insensitive prefix matching. `force_posix` uses `/` and POSIX normalization.
fn is_equal_or_parent_path(base: &str, parent: &str, ignore_case: bool, force_posix: bool) -> bool {
    let separator = if force_posix { '/' } else { HOST_SEPARATOR };
    if base == parent {
        return true;
    }
    if base.is_empty() || parent.is_empty() {
        return false;
    }

    let (mut base, mut parent) = (base.to_owned(), parent.to_owned());
    if base.contains("..") || parent.contains("..") {
        base = normalize_for(force_posix, &base);
        parent = normalize_for(force_posix, &parent);
    }

    let base_u = units(&base);
    let parent_u = units(&parent);
    if parent_u.len() > base_u.len() {
        return false;
    }

    let sep = separator as u16;
    if ignore_case {
        if !starts_with_ignore_case(&base_u, &parent_u) {
            return false;
        }
        if parent_u.len() == base_u.len() {
            // Same path, different casing.
            return true;
        }
        let mut sep_offset = parent_u.len();
        if parent_u.last() == Some(&sep) {
            sep_offset -= 1;
        }
        return base_u.get(sep_offset) == Some(&sep);
    }

    if !parent.ends_with(separator) {
        parent.push(separator);
    }
    base.starts_with(&parent)
}

/// A drive root as `fsPath` writes it: `C:/` or `C:\` (the regex in `hasTrailingPathSeparator`).
fn is_drive_root_fs_path(fs_path: &str) -> bool {
    let b = fs_path.as_bytes();
    b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\')
}

/// `toPosixPath` (extpath.ts): converts to `/` separators, and prefixes a drive path with `/`.
fn to_posix_path(os_path: &str) -> String {
    let path = if os_path.contains('/') {
        os_path.to_owned()
    } else {
        to_slashes(os_path)
    };
    let b = path.as_bytes();
    let drive = b.len() >= 2
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b.len() == 2 || b[2] == b'/');
    if drive { format!("/{path}") } else { path }
}

/// `toSlashes` (extpath.ts): replaces `\` with `/`.
fn to_slashes(os_path: &str) -> String {
    os_path.replace(['\\', '/'], "/")
}

/// Compares two UTF-16 units ignoring case (`strings.ts` `equalsIgnoreCase` per unit).
fn same_unit_ignoring_case(a: u16, b: u16) -> bool {
    if a == b {
        return true;
    }
    if a < 128 && b < 128 {
        return (a as u8).eq_ignore_ascii_case(&(b as u8));
    }
    text(&[a]).to_lowercase() == text(&[b]).to_lowercase()
}

/// `startsWithIgnoreCase`: whether `prefix` matches the start of `s`, ignoring case.
fn starts_with_ignore_case(s: &[u16], prefix: &[u16]) -> bool {
    prefix.len() <= s.len()
        && prefix
            .iter()
            .zip(s)
            .all(|(&p, &c)| same_unit_ignoring_case(c, p))
}

/// `equalsIgnoreCase` (strings.ts): same length, equal ignoring case.
fn equals_ignore_case(a: &str, b: &str) -> bool {
    let (ua, ub) = (units(a), units(b));
    ua.len() == ub.len() && starts_with_ignore_case(&ua, &ub)
}

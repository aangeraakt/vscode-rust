//! Node.js-compatible path manipulation with `posix` and `win32` flavors.
//!
//! Ported from `src/vs/base/common/path.ts`, which copies Node.js `lib/path.js` (v22.15.0).
//! Test cases come from `src/vs/base/test/common/path.test.ts` (see `path/tests.rs`).
//!
//! Differences from the TypeScript module:
//! - The TypeScript exports pick a flavor for the host platform. Here callers pick [`posix`] or
//!   [`win32`] explicitly.
//! - `resolve`, `relative` and `to_namespaced_path` take the working directory as an argument
//!   (`cwd`) instead of reading `process.cwd()`. The `posix` flavor expects a POSIX-style `cwd`.
//! - `win32::resolve` does not read the per-drive working directory (`process.env["=C:"]`).
//!   It uses `cwd`, falling back to the drive root when `cwd` is on another drive. That is what
//!   Node does when the variable is unset.
//! - The `validateString` checks are not needed: the arguments are `&str`.
//!
//! Strings are processed as UTF-16 code units, as JavaScript does, so lengths and indexes match
//! the reference. Every cut point is an ASCII separator or a string boundary, so the conversion
//! back to `String` never splits a surrogate pair.

#[cfg(test)]
mod tests;

const SLASH: u16 = b'/' as u16;
const BACKSLASH: u16 = b'\\' as u16;
const DOT: u16 = b'.' as u16;
const COLON: u16 = b':' as u16;
const QUESTION: u16 = b'?' as u16;

/// The components of a path (`ParsedPath`), as returned by [`posix::parse`] and [`win32::parse`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedPath {
    /// The root, such as `/`, `C:\` or `\\server\share\`. Empty for a relative path.
    pub root: String,
    /// The directory, without a trailing separator (unless it is the root).
    pub dir: String,
    /// The last portion of the path, including the extension.
    pub base: String,
    /// The extension, including the leading dot.
    pub ext: String,
    /// The last portion of the path without the extension.
    pub name: String,
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn text(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

/// `charCodeAt`. Out-of-range reads give 0, which matches no code this module tests for (JS gives
/// `NaN`, which also matches nothing).
fn at(u: &[u16], i: usize) -> u16 {
    u.get(i).copied().unwrap_or(0)
}

/// `slice(start, end)` for non-negative bounds: clamps to the string, and is empty when
/// `start >= end`.
fn sub(u: &[u16], start: usize, end: usize) -> &[u16] {
    let end = end.min(u.len());
    let start = start.min(end);
    &u[start..end]
}

fn is_sep(c: u16) -> bool {
    c == SLASH || c == BACKSLASH
}

fn is_posix_sep(c: u16) -> bool {
    c == SLASH
}

/// `isWindowsDeviceRoot`: an ASCII letter, which may start a drive such as `C:`.
fn is_device_root(c: u16) -> bool {
    matches!(c, 65..=90 | 97..=122)
}

/// Resolves `.` and `..` elements in a path with directory names (`normalizeString`).
fn normalize_string(
    path: &[u16],
    allow_above_root: bool,
    sep: u16,
    is_sep: fn(u16) -> bool,
) -> Vec<u16> {
    let mut res: Vec<u16> = Vec::new();
    let mut last_segment_length: isize = 0;
    let mut last_slash: isize = -1;
    let mut dots: isize = 0;
    let mut code: u16 = 0;
    for i in 0..=path.len() {
        if i < path.len() {
            code = path[i];
        } else if is_sep(code) {
            break;
        } else {
            code = SLASH;
        }

        if is_sep(code) {
            if last_slash == i as isize - 1 || dots == 1 {
                // Empty segment or `.`: nothing to do.
            } else if dots == 2 {
                if res.len() < 2
                    || last_segment_length != 2
                    || res[res.len() - 1] != DOT
                    || res[res.len() - 2] != DOT
                {
                    if res.len() > 2 {
                        match res.iter().rposition(|&c| c == sep) {
                            None => {
                                res.clear();
                                last_segment_length = 0;
                            }
                            Some(idx) => {
                                res.truncate(idx);
                                let last = res
                                    .iter()
                                    .rposition(|&c| c == sep)
                                    .map_or(-1, |p| p as isize);
                                last_segment_length = res.len() as isize - 1 - last;
                            }
                        }
                        last_slash = i as isize;
                        dots = 0;
                        continue;
                    } else if !res.is_empty() {
                        res.clear();
                        last_segment_length = 0;
                        last_slash = i as isize;
                        dots = 0;
                        continue;
                    }
                }
                if allow_above_root {
                    if !res.is_empty() {
                        res.push(sep);
                    }
                    res.push(DOT);
                    res.push(DOT);
                    last_segment_length = 2;
                }
            } else {
                let segment = sub(path, (last_slash + 1) as usize, i);
                if !res.is_empty() {
                    res.push(sep);
                }
                res.extend_from_slice(segment);
                last_segment_length = i as isize - last_slash - 1;
            }
            last_slash = i as isize;
            dots = 0;
        } else if code == DOT && dots != -1 {
            dots += 1;
        } else {
            dots = -1;
        }
    }
    res
}

fn format_ext(ext: &str) -> String {
    if ext.is_empty() {
        String::new()
    } else if ext.starts_with('.') {
        ext.to_owned()
    } else {
        format!(".{ext}")
    }
}

/// `_format`: builds a path from its parts. The root is used as the directory when `dir` is
/// empty, and the base is rebuilt from `name` and `ext` when it is empty.
fn format_with(sep: char, p: &ParsedPath) -> String {
    let dir = if p.dir.is_empty() { &p.root } else { &p.dir };
    let base = if p.base.is_empty() {
        format!("{}{}", p.name, format_ext(&p.ext))
    } else {
        p.base.clone()
    };
    if dir.is_empty() {
        base
    } else if dir == &p.root {
        format!("{dir}{base}")
    } else {
        format!("{dir}{sep}{base}")
    }
}

/// Result of the reverse scan over the last path segment, shared by `extname` and `parse`.
struct LastSegment {
    start_dot: isize,
    start_part: isize,
    end: isize,
    pre_dot_state: i8,
}

impl LastSegment {
    /// True when the segment has an extension: the `startDot`/`preDotState` tests in path.ts.
    /// A leading dot (`.file`) and a lone `..` do not count.
    fn has_extension(&self) -> bool {
        self.end != -1
            && self.start_dot != -1
            && self.pre_dot_state != 0
            && !(self.pre_dot_state == 1
                && self.start_dot == self.end - 1
                && self.start_dot == self.start_part + 1)
    }
}

/// Scans backwards for the last segment. `floor` is where the scan stops. `start_part` is the
/// initial start of the segment, which differs from `floor` in the posix `parse`.
fn scan_last_segment(
    p: &[u16],
    floor: usize,
    start_part_init: usize,
    is_sep: fn(u16) -> bool,
) -> LastSegment {
    let mut start_dot: isize = -1;
    let mut start_part = start_part_init as isize;
    let mut end: isize = -1;
    let mut matched_slash = true;
    let mut pre_dot_state: i8 = 0;
    for i in (floor..p.len()).rev() {
        let code = p[i];
        if is_sep(code) {
            if !matched_slash {
                start_part = i as isize + 1;
                break;
            }
            continue;
        }
        if end == -1 {
            // First non-separator from the end: this is the end of the extension.
            matched_slash = false;
            end = i as isize + 1;
        }
        if code == DOT {
            if start_dot == -1 {
                start_dot = i as isize;
            } else if pre_dot_state != 1 {
                pre_dot_state = 1;
            }
        } else if start_dot != -1 {
            // A non-dot before the dot: the extension may be non-empty.
            pre_dot_state = -1;
        }
    }
    LastSegment {
        start_dot,
        start_part,
        end,
        pre_dot_state,
    }
}

/// `basename` shared by both flavors. `start` skips a drive prefix in win32 (`C:`), and `suffix`
/// is the optional extension to strip.
fn basename_impl(
    p: &[u16],
    suffix: Option<&[u16]>,
    start: usize,
    is_sep: fn(u16) -> bool,
) -> Vec<u16> {
    let mut start = start as isize;
    let floor = start as usize;
    let mut end: isize = -1;
    let mut matched_slash = true;

    if let Some(suffix) = suffix
        && !suffix.is_empty()
        && suffix.len() <= p.len()
    {
        if suffix == p {
            return Vec::new();
        }
        let mut ext_idx: isize = suffix.len() as isize - 1;
        let mut first_non_slash_end: isize = -1;
        for i in (floor..p.len()).rev() {
            let code = p[i];
            if is_sep(code) {
                // A separator that is not part of a run at the end stops the scan.
                if !matched_slash {
                    start = i as isize + 1;
                    break;
                }
            } else {
                if first_non_slash_end == -1 {
                    matched_slash = false;
                    first_non_slash_end = i as isize + 1;
                }
                if ext_idx >= 0 {
                    if code == suffix[ext_idx as usize] {
                        ext_idx -= 1;
                        if ext_idx == -1 {
                            end = i as isize;
                        }
                    } else {
                        // The extension does not match: the whole component is the result.
                        ext_idx = -1;
                        end = first_non_slash_end;
                    }
                }
            }
        }

        if start == end {
            end = first_non_slash_end;
        } else if end == -1 {
            end = p.len() as isize;
        }
        return sub(p, start as usize, end as usize).to_vec();
    }

    for i in (floor..p.len()).rev() {
        if is_sep(p[i]) {
            if !matched_slash {
                start = i as isize + 1;
                break;
            }
        } else if end == -1 {
            matched_slash = false;
            end = i as isize + 1;
        }
    }

    if end == -1 {
        return Vec::new();
    }
    sub(p, start as usize, end as usize).to_vec()
}

/// The POSIX flavor (`path.posix`).
pub mod posix {
    use super::*;

    /// The path separator (`path.posix.sep`).
    pub const SEP: char = '/';
    /// The `PATH` list delimiter (`path.posix.delimiter`).
    pub const DELIMITER: char = ':';

    /// Resolves segments to an absolute path, right to left, against `cwd` (`path.posix.resolve`).
    /// The result is normalized and has no trailing separator. It is `.` when the result is empty
    /// and relative.
    pub fn resolve(cwd: &str, segments: &[&str]) -> String {
        let mut resolved: Vec<u16> = Vec::new();
        let mut resolved_absolute = false;

        for segment in segments.iter().rev() {
            if resolved_absolute {
                break;
            }
            let path = units(segment);
            if path.is_empty() {
                continue;
            }
            let mut next = path.clone();
            next.push(SLASH);
            next.extend_from_slice(&resolved);
            resolved = next;
            resolved_absolute = path[0] == SLASH;
        }

        if !resolved_absolute {
            let cwd = units(cwd);
            let mut next = cwd.clone();
            next.push(SLASH);
            next.extend_from_slice(&resolved);
            resolved = next;
            resolved_absolute = at(&cwd, 0) == SLASH;
        }

        let resolved = normalize_string(&resolved, !resolved_absolute, SLASH, is_posix_sep);
        if resolved_absolute {
            format!("/{}", text(&resolved))
        } else if resolved.is_empty() {
            ".".to_owned()
        } else {
            text(&resolved)
        }
    }

    /// Normalizes a path, resolving `.` and `..` (`path.posix.normalize`).
    pub fn normalize(path: &str) -> String {
        if path.is_empty() {
            return ".".to_owned();
        }
        let p = units(path);
        let is_absolute = p[0] == SLASH;
        let trailing_separator = p[p.len() - 1] == SLASH;

        let mut out = normalize_string(&p, !is_absolute, SLASH, is_posix_sep);
        if out.is_empty() {
            if is_absolute {
                return "/".to_owned();
            }
            return if trailing_separator { "./" } else { "." }.to_owned();
        }
        if trailing_separator {
            out.push(SLASH);
        }
        if is_absolute {
            format!("/{}", text(&out))
        } else {
            text(&out)
        }
    }

    /// True when the path starts with `/` (`path.posix.isAbsolute`).
    pub fn is_absolute(path: &str) -> bool {
        path.starts_with('/')
    }

    /// Joins the non-empty segments with `/` and normalizes the result (`path.posix.join`).
    pub fn join(paths: &[&str]) -> String {
        let parts: Vec<&str> = paths.iter().copied().filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            return ".".to_owned();
        }
        normalize(&parts.join("/"))
    }

    /// The relative path from `from` to `to`, with both resolved against `cwd`
    /// (`path.posix.relative`).
    pub fn relative(cwd: &str, from: &str, to: &str) -> String {
        if from == to {
            return String::new();
        }
        let from = resolve(cwd, &[from]);
        let to = resolve(cwd, &[to]);
        if from == to {
            return String::new();
        }

        let f = units(&from);
        let t = units(&to);
        // Both paths start with `/`, which is skipped (as in path.ts).
        let from_start = 1;
        let from_end = f.len();
        let from_len = from_end - from_start;
        let to_start = 1;
        let to_len = t.len() - to_start;

        // Find the longest common prefix, remembering the last common separator.
        let length = from_len.min(to_len);
        let mut last_common_sep: isize = -1;
        let mut i = 0;
        while i < length {
            let from_code = f[from_start + i];
            if from_code != t[to_start + i] {
                break;
            } else if from_code == SLASH {
                last_common_sep = i as isize;
            }
            i += 1;
        }
        if i == length {
            if to_len > length {
                if t[to_start + i] == SLASH {
                    // `from` is the exact base path of `to`: from='/foo/bar', to='/foo/bar/baz'.
                    return text(sub(&t, to_start + i + 1, t.len()));
                }
                if i == 0 {
                    // `from` is the root: from='/', to='/foo'.
                    return text(sub(&t, to_start + i, t.len()));
                }
            } else if from_len > length {
                if f[from_start + i] == SLASH {
                    // `to` is the exact base path of `from`: from='/foo/bar/baz', to='/foo/bar'.
                    last_common_sep = i as isize;
                } else if i == 0 {
                    // `to` is the root: from='/foo/bar', to='/'.
                    last_common_sep = 0;
                }
            }
        }

        // One `..` for each separator after the common part, plus one for the last segment.
        let first = (from_start as isize + last_common_sep + 1) as usize;
        let parents = if first > from_end {
            0
        } else {
            f[first..from_end].iter().filter(|&&c| c == SLASH).count() + 1
        };
        let mut out = vec![".."; parents].join("/");

        let rest_start = (to_start as isize + last_common_sep) as usize;
        out.push_str(&text(sub(&t, rest_start, t.len())));
        out
    }

    /// Returns the path unchanged; POSIX has no namespaced form (`path.posix.toNamespacedPath`).
    pub fn to_namespaced_path(path: &str) -> String {
        path.to_owned()
    }

    /// The directory part of a path (`path.posix.dirname`).
    pub fn dirname(path: &str) -> String {
        if path.is_empty() {
            return ".".to_owned();
        }
        let p = units(path);
        let has_root = p[0] == SLASH;
        let mut end: isize = -1;
        let mut matched_slash = true;
        for i in (1..p.len()).rev() {
            if p[i] == SLASH {
                if !matched_slash {
                    end = i as isize;
                    break;
                }
            } else {
                matched_slash = false;
            }
        }

        if end == -1 {
            return if has_root { "/" } else { "." }.to_owned();
        }
        if has_root && end == 1 {
            return "//".to_owned();
        }
        text(&p[..end as usize])
    }

    /// The last portion of a path, with `suffix` removed when it matches (`path.posix.basename`).
    pub fn basename(path: &str, suffix: Option<&str>) -> String {
        let suffix = suffix.map(units);
        text(&basename_impl(
            &units(path),
            suffix.as_deref(),
            0,
            is_posix_sep,
        ))
    }

    /// The extension of the last portion, including the dot (`path.posix.extname`). Empty when
    /// there is none.
    pub fn extname(path: &str) -> String {
        let p = units(path);
        let seg = scan_last_segment(&p, 0, 0, is_posix_sep);
        if seg.has_extension() {
            text(sub(&p, seg.start_dot as usize, seg.end as usize))
        } else {
            String::new()
        }
    }

    /// Builds a path from its parts (`path.posix.format`).
    pub fn format(path_object: &ParsedPath) -> String {
        format_with(SEP, path_object)
    }

    /// Splits a path into its parts (`path.posix.parse`).
    pub fn parse(path: &str) -> ParsedPath {
        let mut ret = ParsedPath::default();
        let p = units(path);
        if p.is_empty() {
            return ret;
        }
        let is_absolute = p[0] == SLASH;
        let floor = if is_absolute {
            ret.root = "/".to_owned();
            1
        } else {
            0
        };

        let seg = scan_last_segment(&p, floor, 0, is_posix_sep);
        if seg.end != -1 {
            let start = if seg.start_part == 0 && is_absolute {
                1
            } else {
                seg.start_part as usize
            };
            let end = seg.end as usize;
            if seg.has_extension() {
                ret.name = text(sub(&p, start, seg.start_dot as usize));
                ret.base = text(sub(&p, start, end));
                ret.ext = text(sub(&p, seg.start_dot as usize, end));
            } else {
                ret.base = text(sub(&p, start, end));
                ret.name = ret.base.clone();
            }
        }

        if seg.start_part > 0 {
            ret.dir = text(sub(&p, 0, seg.start_part as usize - 1));
        } else if is_absolute {
            ret.dir = "/".to_owned();
        }
        ret
    }
}

/// The win32 flavor (`path.win32`). Accepts both `\` and `/` as separators.
pub mod win32 {
    use super::*;

    /// The path separator (`path.win32.sep`).
    pub const SEP: char = '\\';
    /// The `PATH` list delimiter (`path.win32.delimiter`).
    pub const DELIMITER: char = ';';

    /// Resolves segments to an absolute path, right to left, against `cwd` (`path.win32.resolve`).
    /// The result is normalized and has no trailing separator. It is `.` when the result is empty
    /// and relative.
    pub fn resolve(cwd: &str, segments: &[&str]) -> String {
        let mut resolved_device = String::new();
        let mut resolved_tail = String::new();
        let mut resolved_absolute = false;

        for index in (-1..segments.len() as isize).rev() {
            let path: Vec<u16> = if index >= 0 {
                let path = units(segments[index as usize]);
                if path.is_empty() {
                    continue;
                }
                path
            } else if resolved_device.is_empty() {
                units(cwd)
            } else {
                // Node reads `process.env["=C:"]` here. Without it, it uses the process cwd, or the
                // drive root when the cwd is on another drive.
                let mut path = units(cwd);
                let cwd_drive = text(sub(&path, 0, 2)).to_lowercase();
                if cwd_drive != resolved_device.to_lowercase() && at(&path, 2) == BACKSLASH {
                    path = units(&format!("{resolved_device}\\"));
                }
                path
            };

            let len = path.len();
            let mut root_end = 0usize;
            let mut device = String::new();
            let mut is_absolute = false;
            let code = at(&path, 0);

            if len == 1 {
                if is_sep(code) {
                    root_end = 1;
                    is_absolute = true;
                }
            } else if is_sep(code) {
                // Possible UNC root.
                is_absolute = true;
                if is_sep(at(&path, 1)) {
                    if let Some((unc_device, unc_end)) = match_unc_root(&path) {
                        device = unc_device;
                        root_end = unc_end;
                    }
                } else {
                    root_end = 1;
                }
            } else if is_device_root(code) && at(&path, 1) == COLON {
                // Possible drive root.
                device = text(sub(&path, 0, 2));
                root_end = 2;
                if len > 2 && is_sep(path[2]) {
                    is_absolute = true;
                    root_end = 3;
                }
            }

            if !device.is_empty() {
                if !resolved_device.is_empty() {
                    if device.to_lowercase() != resolved_device.to_lowercase() {
                        // Another drive: not applicable.
                        continue;
                    }
                } else {
                    resolved_device = device;
                }
            }

            if resolved_absolute {
                if !resolved_device.is_empty() {
                    break;
                }
            } else {
                resolved_tail = format!("{}\\{}", text(sub(&path, root_end, len)), resolved_tail);
                resolved_absolute = is_absolute;
                if is_absolute && !resolved_device.is_empty() {
                    break;
                }
            }
        }

        let tail = text(&normalize_string(
            &units(&resolved_tail),
            !resolved_absolute,
            BACKSLASH,
            is_sep,
        ));
        if resolved_absolute {
            format!("{resolved_device}\\{tail}")
        } else {
            let result = format!("{resolved_device}{tail}");
            if result.is_empty() {
                ".".to_owned()
            } else {
                result
            }
        }
    }

    /// Matches a UNC root (`\\server\share`) at the start of `path`, which starts with two
    /// separators. Returns the device (`\\server\share`) and the end of the root.
    fn match_unc_root(path: &[u16]) -> Option<(String, usize)> {
        let len = path.len();
        let mut j = 2;
        let mut last = j;
        while j < len && !is_sep(path[j]) {
            j += 1;
        }
        if j < len && j != last {
            let first_part = text(&path[last..j]);
            last = j;
            while j < len && is_sep(path[j]) {
                j += 1;
            }
            if j < len && j != last {
                last = j;
                while j < len && !is_sep(path[j]) {
                    j += 1;
                }
                if j == len || j != last {
                    return Some((format!("\\\\{first_part}\\{}", text(&path[last..j])), j));
                }
            }
        }
        None
    }

    /// Normalizes a path, resolving `.` and `..` (`path.win32.normalize`).
    pub fn normalize(path: &str) -> String {
        if path.is_empty() {
            return ".".to_owned();
        }
        let p = units(path);
        let len = p.len();
        let code = p[0];

        if len == 1 {
            return if is_posix_sep(code) {
                "\\".to_owned()
            } else {
                path.to_owned()
            };
        }

        let mut root_end = 0usize;
        let mut device: Option<String> = None;
        let mut is_absolute = false;
        if is_sep(code) {
            // Possible UNC root.
            is_absolute = true;
            if is_sep(at(&p, 1)) {
                if let Some((unc, end)) = match_unc_root(&p) {
                    if end == len {
                        // A UNC root only: return it normalized.
                        return format!("{unc}\\");
                    }
                    // A UNC root with more path after it.
                    device = Some(unc);
                    root_end = end;
                }
            } else {
                root_end = 1;
            }
        } else if is_device_root(code) && at(&p, 1) == COLON {
            // Drive root.
            device = Some(text(&p[0..2]));
            root_end = 2;
            if len > 2 && is_sep(p[2]) {
                is_absolute = true;
                root_end = 3;
            }
        }

        let mut tail = if root_end < len {
            text(&normalize_string(
                sub(&p, root_end, len),
                !is_absolute,
                BACKSLASH,
                is_sep,
            ))
        } else {
            String::new()
        };
        if tail.is_empty() && !is_absolute {
            tail = ".".to_owned();
        }
        if !tail.is_empty() && is_sep(p[len - 1]) {
            tail.push('\\');
        }

        if !is_absolute && device.is_none() && path.contains(':') {
            // Keep a relative path from becoming one Windows reads as absolute (CVE-2024-36139).
            let tail_units = units(&tail);
            if tail_units.len() >= 2 && is_device_root(tail_units[0]) && tail_units[1] == COLON {
                return format!(".\\{tail}");
            }
            for (index, &code) in p.iter().enumerate() {
                if code == COLON && (index == len - 1 || is_sep(p[index + 1])) {
                    return format!(".\\{tail}");
                }
            }
        }

        match device {
            None => {
                if is_absolute {
                    format!("\\{tail}")
                } else {
                    tail
                }
            }
            Some(device) => {
                if is_absolute {
                    format!("{device}\\{tail}")
                } else {
                    format!("{device}{tail}")
                }
            }
        }
    }

    /// True when the path is absolute: starts with a separator, or with a drive and a separator
    /// (`path.win32.isAbsolute`).
    pub fn is_absolute(path: &str) -> bool {
        let p = units(path);
        if p.is_empty() {
            return false;
        }
        let code = p[0];
        is_sep(code) || (p.len() > 2 && is_device_root(code) && p[1] == COLON && is_sep(p[2]))
    }

    /// Joins the non-empty segments with `\` and normalizes the result (`path.win32.join`).
    ///
    /// A joined path that starts with exactly two separators is a UNC path (`//server/share`
    /// becomes `\\server\share\`). Extra leading separators are collapsed to one, except that
    /// a first segment which already names a UNC root is left as it is.
    pub fn join(paths: &[&str]) -> String {
        let parts: Vec<&str> = paths.iter().copied().filter(|p| !p.is_empty()).collect();
        let Some(&first_part) = parts.first() else {
            return ".".to_owned();
        };
        let mut joined = parts.join("\\");

        let first = units(first_part);
        let mut needs_replace = true;
        let mut slash_count = 0usize;
        if is_sep(at(&first, 0)) {
            slash_count += 1;
            let first_len = first.len();
            if first_len > 1 && is_sep(first[1]) {
                slash_count += 1;
                if first_len > 2 {
                    if is_sep(first[2]) {
                        slash_count += 1;
                    } else {
                        // A UNC path in the first part.
                        needs_replace = false;
                    }
                }
            }
        }

        if needs_replace {
            let joined_units = units(&joined);
            while slash_count < joined_units.len() && is_sep(joined_units[slash_count]) {
                slash_count += 1;
            }
            if slash_count >= 2 {
                joined = format!("\\{}", text(&joined_units[slash_count..]));
            }
        }

        normalize(&joined)
    }

    /// The relative path from `from` to `to`, with both resolved against `cwd`
    /// (`path.win32.relative`). Comparison ignores case.
    pub fn relative(cwd: &str, from: &str, to: &str) -> String {
        if from == to {
            return String::new();
        }
        let from_orig = resolve(cwd, &[from]);
        let to_orig = resolve(cwd, &[to]);
        if from_orig == to_orig {
            return String::new();
        }
        let from_lower = from_orig.to_lowercase();
        let to_lower = to_orig.to_lowercase();
        if from_lower == to_lower {
            return String::new();
        }

        if units(&from_orig).len() != units(&from_lower).len()
            || units(&to_orig).len() != units(&to_lower).len()
        {
            return relative_by_segments(&from_orig, &to_orig);
        }

        let from_u = units(&from_lower);
        let to_u = units(&to_lower);
        let to_orig_u = units(&to_orig);

        // Trim leading and trailing backslashes (trailing ones only matter for UNC paths).
        let (from_start, from_end) = trim_backslashes(&from_u);
        let from_len = from_end - from_start;
        let (mut to_start, to_end) = trim_backslashes(&to_u);
        let to_len = to_end - to_start;

        let length = from_len.min(to_len);
        let mut last_common_sep: isize = -1;
        let mut i = 0;
        while i < length {
            let from_code = from_u[from_start + i];
            if from_code != to_u[to_start + i] {
                break;
            } else if from_code == BACKSLASH {
                last_common_sep = i as isize;
            }
            i += 1;
        }

        if i != length {
            if last_common_sep == -1 {
                // A mismatch before the first common separator: the result is `to`.
                return to_orig;
            }
        } else {
            if to_len > length {
                if to_u[to_start + i] == BACKSLASH {
                    // `from` is the exact base path of `to`.
                    return text(sub(&to_orig_u, to_start + i + 1, to_end));
                }
                if i == 2 {
                    // `from` is the device root: from='C:\', to='C:\foo'.
                    return text(sub(&to_orig_u, to_start + i, to_end));
                }
            }
            if from_len > length {
                if from_u[from_start + i] == BACKSLASH {
                    // `to` is the exact base path of `from`.
                    last_common_sep = i as isize;
                } else if i == 2 {
                    // `to` is the device root: from='C:\foo\bar', to='C:\'.
                    last_common_sep = 3;
                }
            }
            if last_common_sep == -1 {
                last_common_sep = 0;
            }
        }

        // One `..` for each separator after the common part, plus one for the last segment.
        let first = (from_start as isize + last_common_sep + 1) as usize;
        let parents = if first > from_end {
            0
        } else {
            from_u[first..from_end]
                .iter()
                .filter(|&&c| c == BACKSLASH)
                .count()
                + 1
        };
        let out = vec![".."; parents].join("\\");

        to_start = (to_start as isize + last_common_sep) as usize;
        if !out.is_empty() {
            return format!("{out}{}", text(sub(&to_orig_u, to_start, to_end)));
        }
        if at(&to_orig_u, to_start) == BACKSLASH {
            to_start += 1;
        }
        text(sub(&to_orig_u, to_start, to_end))
    }

    /// Returns `(start, end)` of a UTF-16 slice with leading and trailing backslashes trimmed.
    /// The last character is kept, as path.ts does.
    fn trim_backslashes(u: &[u16]) -> (usize, usize) {
        let mut start = 0;
        while start < u.len() && u[start] == BACKSLASH {
            start += 1;
        }
        let mut end = u.len();
        while end > start + 1 && u[end - 1] == BACKSLASH {
            end -= 1;
        }
        (start, end)
    }

    /// The branch of `relative` for paths whose case-folding changes their length: it compares
    /// whole segments instead of code units.
    fn relative_by_segments(from_orig: &str, to_orig: &str) -> String {
        let mut from_split: Vec<&str> = from_orig.split('\\').collect();
        let mut to_split: Vec<&str> = to_orig.split('\\').collect();
        if from_split.last() == Some(&"") {
            from_split.pop();
        }
        if to_split.last() == Some(&"") {
            to_split.pop();
        }

        let from_len = from_split.len();
        let to_len = to_split.len();
        let length = from_len.min(to_len);

        let mut i = 0;
        while i < length && from_split[i].to_lowercase() == to_split[i].to_lowercase() {
            i += 1;
        }

        if i == 0 {
            return to_orig.to_owned();
        }
        if i == length {
            if to_len > length {
                return to_split[i..].join("\\");
            }
            if from_len > length {
                return format!("{}..", "..\\".repeat(from_len - 1 - i));
            }
            return String::new();
        }
        format!(
            "{}{}",
            "..\\".repeat(from_len - i),
            to_split[i..].join("\\")
        )
    }

    /// The namespaced form of a path, such as `\\?\C:\...` (`path.win32.toNamespacedPath`). The
    /// path is resolved against `cwd` first.
    pub fn to_namespaced_path(cwd: &str, path: &str) -> String {
        if path.is_empty() {
            return path.to_owned();
        }
        let resolved = resolve(cwd, &[path]);
        let r = units(&resolved);
        if r.len() <= 2 {
            return path.to_owned();
        }

        if r[0] == BACKSLASH {
            // Possible UNC root.
            if r[1] == BACKSLASH {
                let code = r[2];
                if code != QUESTION && code != DOT {
                    // A non-long UNC root: convert to the long UNC form.
                    return format!("\\\\?\\UNC\\{}", text(&r[2..]));
                }
            }
        } else if is_device_root(r[0]) && r[1] == COLON && r[2] == BACKSLASH {
            // A drive root: convert to the long form.
            return format!("\\\\?\\{resolved}");
        }

        resolved
    }

    /// The directory part of a path (`path.win32.dirname`). For a UNC root the server and share
    /// are the root, as in `\\server\share\`.
    pub fn dirname(path: &str) -> String {
        if path.is_empty() {
            return ".".to_owned();
        }
        let p = units(path);
        let len = p.len();
        if len == 1 {
            return if is_sep(p[0]) { path } else { "." }.to_owned();
        }

        let mut root_end: isize = -1;
        let mut offset = 0usize;
        let code = p[0];
        if is_sep(code) {
            // Possible UNC root.
            root_end = 1;
            offset = 1;
            if is_sep(at(&p, 1)) {
                if let Some((_, end)) = match_unc_root(&p) {
                    if end == len {
                        // A UNC root only.
                        return path.to_owned();
                    }
                    // A UNC root with more path: the separator after the root counts as a
                    // normal root on top of it.
                    root_end = end as isize + 1;
                    offset = end + 1;
                }
            }
        } else if is_device_root(code) && p[1] == COLON {
            root_end = if len > 2 && is_sep(p[2]) { 3 } else { 2 };
            offset = root_end as usize;
        }

        let mut end: isize = -1;
        let mut matched_slash = true;
        for i in (offset..len).rev() {
            if is_sep(p[i]) {
                if !matched_slash {
                    end = i as isize;
                    break;
                }
            } else {
                matched_slash = false;
            }
        }

        if end == -1 {
            if root_end == -1 {
                return ".".to_owned();
            }
            end = root_end;
        }
        text(&p[..end as usize])
    }

    /// The last portion of a path, with `suffix` removed when it matches
    /// (`path.win32.basename`). A drive prefix (`C:`) is not part of the basename.
    pub fn basename(path: &str, suffix: Option<&str>) -> String {
        let p = units(path);
        let start = if p.len() >= 2 && is_device_root(p[0]) && p[1] == COLON {
            2
        } else {
            0
        };
        let suffix = suffix.map(units);
        text(&basename_impl(&p, suffix.as_deref(), start, is_sep))
    }

    /// The extension of the last portion, including the dot (`path.win32.extname`). Empty when
    /// there is none.
    pub fn extname(path: &str) -> String {
        let p = units(path);
        let start = if p.len() >= 2 && p[1] == COLON && is_device_root(p[0]) {
            2
        } else {
            0
        };
        let seg = scan_last_segment(&p, start, start, is_sep);
        if seg.has_extension() {
            text(sub(&p, seg.start_dot as usize, seg.end as usize))
        } else {
            String::new()
        }
    }

    /// Builds a path from its parts (`path.win32.format`).
    pub fn format(path_object: &ParsedPath) -> String {
        format_with(SEP, path_object)
    }

    /// Splits a path into its parts (`path.win32.parse`).
    pub fn parse(path: &str) -> ParsedPath {
        let mut ret = ParsedPath::default();
        let p = units(path);
        if p.is_empty() {
            return ret;
        }
        let len = p.len();
        let code = p[0];

        if len == 1 {
            if is_sep(code) {
                ret.root = path.to_owned();
                ret.dir = ret.root.clone();
            } else {
                ret.base = path.to_owned();
                ret.name = ret.base.clone();
            }
            return ret;
        }

        let mut root_end = 0usize;
        if is_sep(code) {
            // Possible UNC root.
            root_end = 1;
            if is_sep(at(&p, 1)) {
                if let Some((_, end)) = match_unc_root(&p) {
                    root_end = if end == len { end } else { end + 1 };
                }
            }
        } else if is_device_root(code) && p[1] == COLON {
            // Drive root.
            if len <= 2 {
                ret.root = path.to_owned();
                ret.dir = ret.root.clone();
                return ret;
            }
            root_end = 2;
            if is_sep(p[2]) {
                if len == 3 {
                    ret.root = path.to_owned();
                    ret.dir = ret.root.clone();
                    return ret;
                }
                root_end = 3;
            }
        }
        if root_end > 0 {
            ret.root = text(&p[..root_end]);
        }

        let seg = scan_last_segment(&p, root_end, root_end, is_sep);
        if seg.end != -1 {
            let start = seg.start_part as usize;
            let end = seg.end as usize;
            if seg.has_extension() {
                ret.name = text(sub(&p, start, seg.start_dot as usize));
                ret.base = text(sub(&p, start, end));
                ret.ext = text(sub(&p, seg.start_dot as usize, end));
            } else {
                ret.base = text(sub(&p, start, end));
                ret.name = ret.base.clone();
            }
        }

        // The directory is the root when the base sits directly under it. Otherwise it is the
        // path up to the separator before the base, without that separator.
        if seg.start_part > 0 && seg.start_part != root_end as isize {
            ret.dir = text(sub(&p, 0, seg.start_part as usize - 1));
        } else {
            ret.dir = ret.root.clone();
        }
        ret
    }
}

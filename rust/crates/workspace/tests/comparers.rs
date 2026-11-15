//! Port of `src/vs/base/test/browser/comparers.test.ts` (generated from the upstream
//! assertions; `null` arguments are the empty string).

use std::cmp::Ordering;

use lathe_workspace::comparers as c;

fn sorted(items: &[&str], compare: fn(&str, &str) -> Ordering) -> Vec<String> {
    let mut items: Vec<&str> = items.to_vec();
    items.sort_by(|a, b| compare(a, b));
    items.into_iter().map(String::from).collect()
}

#[rustfmt::skip]
#[test]
fn compare_file_names() {
    assert!(c::compare_file_names("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_names("", "abc").is_lt(), "null should be come before real values");
    assert!(c::compare_file_names("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_names("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_names("z", "A").is_gt(), "z comes after A");
    assert!(c::compare_file_names("Z", "a").is_gt(), "Z comes after a");
    assert!(c::compare_file_names("bbb.aaa", "aaa.bbb").is_gt(), "compares the whole name all at once by locale");
    assert!(c::compare_file_names("aggregate.go", "aggregate_repo.go").is_gt(), "compares the whole name all at once by locale");
    assert!(c::compare_file_names(".abc", ".abc").is_eq(), "equal dotfile names should be equal");
    assert!(c::compare_file_names(".env.", ".gitattributes").is_lt(), "filenames starting with dots and with extensions should still sort properly");
    assert!(c::compare_file_names(".env", ".aaa.env").is_gt(), "dotfiles sort alphabetically when they contain multiple dots");
    assert!(c::compare_file_names(".env", ".env.aaa").is_lt(), "dotfiles with the same root sort shortest first");
    assert!(c::compare_file_names(".aaa_env", ".aaa.env").is_lt(), "an underscore in a dotfile name will sort before a dot");
    assert!(c::compare_file_names("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_names(".env", "aaa").is_lt(), "dotfiles come before filenames without extensions");
    assert!(c::compare_file_names(".env", "aaa.env").is_lt(), "dotfiles come before filenames with extensions");
    assert!(c::compare_file_names(".md", "A.MD").is_lt(), "dotfiles sort before uppercase files");
    assert!(c::compare_file_names(".MD", "a.md").is_lt(), "dotfiles sort before lowercase files");
    assert!(c::compare_file_names("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_names("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_names("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_names("abc2.txt", "abc10.txt").is_lt(), "filenames with numbers should be in numerical order even when they are multiple digits long");
    assert!(c::compare_file_names("abc02.txt", "abc010.txt").is_lt(), "filenames with numbers that have leading zeros sort numerically");
    assert!(c::compare_file_names("abc1.10.txt", "abc1.2.txt").is_gt(), "numbers with dots between them are treated as two separate numbers, not one decimal number");
    assert!(c::compare_file_names("a.ext1", "b.Ext1").is_lt(), "if names are different and extensions with numbers are equal except for case, filenames are sorted in name order");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_names), vec!["A2.txt", "a10.txt", "a20.txt", "A100.txt"], "filenames with number and case differences compare numerically");
    assert_ne!(c::compare_file_names("a", "A"), c::compare_locale("a", "A"), "the same letter sorts in unicode order, not by locale");
    assert_ne!(c::compare_file_names("â", "Â"), c::compare_locale("â", "Â"), "the same accented letter sorts in unicode order, not by locale");
    assert_ne!(sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_file_names), sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_locale), "words with the same root and different cases do not sort in locale order");
    assert_ne!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_names), sorted(&["email", "Email", "émail", "Émail"], c::compare_locale), "the same base characters with different case or accents do not sort in locale order");
    assert!(c::compare_file_names("abc02.txt", "abc002.txt").is_gt(), "filenames with equivalent numbers and leading zeros sort in unicode order");
    assert!(c::compare_file_names("abc.txt1", "abc.txt01").is_gt(), "same name plus extensions with equal numbers sort in unicode order");
    assert_ne!(c::compare_file_names("art01", "Art01"), c::compare_locale_numeric("art01", "Art01"), "a numerically equivalent word of a different case does not compare numerically based on locale");
    assert!(c::compare_file_names("a.ext1", "a.Ext1").is_gt(), "if names are equal and extensions with numbers are equal except for case, filenames are sorted in full filename unicode order");
}

#[rustfmt::skip]
#[test]
fn compare_file_extensions() {
    assert!(c::compare_file_extensions("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_extensions("", "abc").is_lt(), "null should come before real files without extension");
    assert!(c::compare_file_extensions("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_extensions("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_extensions("z", "A").is_gt(), "z comes after A");
    assert!(c::compare_file_extensions("Z", "a").is_gt(), "Z comes after a");
    assert!(c::compare_file_extensions("file.ext", "file.ext").is_eq(), "equal full names should be equal");
    assert!(c::compare_file_extensions("a.ext", "b.ext").is_lt(), "if equal extensions, filenames should be compared");
    assert!(c::compare_file_extensions("file.aaa", "file.bbb").is_lt(), "files with equal names should be compared by extensions");
    assert!(c::compare_file_extensions("bbb.aaa", "aaa.bbb").is_lt(), "files should be compared by extensions even if filenames compare differently");
    assert!(c::compare_file_extensions(".abc", ".abc").is_eq(), "equal dotfiles should be equal");
    assert!(c::compare_file_extensions(".md", ".Gitattributes").is_gt(), "dotfiles sort alphabetically regardless of case");
    assert!(c::compare_file_extensions("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_extensions(".env", "aaa.env").is_lt(), "if equal extensions, filenames should be compared, empty filename should come before others");
    assert!(c::compare_file_extensions(".MD", "a.md").is_lt(), "if extensions differ in case, files sort by extension in unicode order");
    assert!(c::compare_file_extensions("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_extensions("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_extensions("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions("abc2.txt", "abc10.txt").is_lt(), "filenames with numbers should be in numerical order even when they are multiple digits long");
    assert!(c::compare_file_extensions("abc02.txt", "abc010.txt").is_lt(), "filenames with numbers that have leading zeros sort numerically");
    assert!(c::compare_file_extensions("abc1.10.txt", "abc1.2.txt").is_gt(), "numbers with dots between them are treated as two separate numbers, not one decimal number");
    assert!(c::compare_file_extensions("abc2.txt2", "abc1.txt10").is_lt(), "extensions with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions("txt.abc1", "txt.abc1").is_eq(), "equal extensions with numbers should be equal");
    assert!(c::compare_file_extensions("txt.abc1", "txt.abc2").is_lt(), "extensions with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions("txt.abc2", "txt.abc10").is_lt(), "extensions with numbers should be in numerical order even when they are multiple digits long");
    assert!(c::compare_file_extensions("a.ext1", "b.ext1").is_lt(), "if equal extensions with numbers, names should be compared");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_extensions), vec!["A2.txt", "a10.txt", "a20.txt", "A100.txt"], "filenames with number and case differences compare numerically");
    assert_ne!(c::compare_file_extensions("a", "A"), c::compare_locale("a", "A"), "the same letter of different case does not sort by locale");
    assert_ne!(c::compare_file_extensions("â", "Â"), c::compare_locale("â", "Â"), "the same accented letter of different case does not sort by locale");
    assert_ne!(sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_file_extensions), sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_locale), "words with the same root and different cases do not sort in locale order");
    assert_ne!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_extensions), sorted(&["email", "Email", "émail", "Émail"], c::compare_locale), "the same base characters with different case or accents do not sort in locale order");
    assert!(c::compare_file_extensions("a.MD", "a.md").is_lt(), "case differences in extensions sort in unicode order");
    assert!(c::compare_file_extensions("a.md", "A.md").is_gt(), "case differences in names sort in unicode order");
    assert!(c::compare_file_extensions("a.md", "b.MD").is_gt(), "when extensions are the same except for case, the files sort by extension");
    assert!(c::compare_file_extensions("aggregate.go", "aggregate_repo.go").is_lt(), "when extensions are equal, names sort in dictionary order");
    assert!(c::compare_file_extensions(".env", ".aaa.env").is_lt(), "a dotfile with an extension is treated as a name plus an extension - equal extensions");
    assert!(c::compare_file_extensions(".env", ".env.aaa").is_gt(), "a dotfile with an extension is treated as a name plus an extension - unequal extensions");
    assert!(c::compare_file_extensions(".env", "aaa").is_gt(), "filenames without extensions come before dotfiles");
    assert!(c::compare_file_extensions(".md", "A.MD").is_gt(), "a file with an uppercase extension sorts before a dotfile of the same lowercase extension");
    assert!(c::compare_file_extensions("abc.txt01", "abc.txt1").is_lt(), "extensions with equal numbers sort in unicode order");
    assert_ne!(c::compare_file_extensions("art01", "Art01"), c::compare_locale_numeric("art01", "Art01"), "a numerically equivalent word of a different case does not compare by locale");
    assert!(c::compare_file_extensions("abc02.txt", "abc002.txt").is_gt(), "filenames with equivalent numbers and leading zeros sort in unicode order");
    assert!(c::compare_file_extensions("txt.abc01", "txt.abc1").is_lt(), "extensions with equivalent numbers sort in unicode order");
    assert!(c::compare_file_extensions("a.ext1", "b.Ext1").is_gt(), "if names are different and extensions with numbers are equal except for case, filenames are sorted in extension unicode order");
    assert!(c::compare_file_extensions("a.ext1", "a.Ext1").is_gt(), "if names are equal and extensions with numbers are equal except for case, filenames are sorted in extension unicode order");
}

#[rustfmt::skip]
#[test]
fn compare_file_names_default() {
    assert!(c::compare_file_names_default("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_names_default("", "abc").is_lt(), "null should be come before real values");
    assert!(c::compare_file_names_default("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_names_default("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_names_default("z", "A").is_gt(), "z comes after A");
    assert!(c::compare_file_names_default("Z", "a").is_gt(), "Z comes after a");
    assert!(c::compare_file_names_default("file.ext", "file.ext").is_eq(), "equal full names should be equal");
    assert!(c::compare_file_names_default("a.ext", "b.ext").is_lt(), "if equal extensions, filenames should be compared");
    assert!(c::compare_file_names_default("file.aaa", "file.bbb").is_lt(), "files with equal names should be compared by extensions");
    assert!(c::compare_file_names_default("bbb.aaa", "aaa.bbb").is_gt(), "files should be compared by names even if extensions compare differently");
    assert!(c::compare_file_names_default("aggregate.go", "aggregate_repo.go").is_gt(), "compares the whole filename in locale order");
    assert!(c::compare_file_names_default(".abc", ".abc").is_eq(), "equal dotfile names should be equal");
    assert!(c::compare_file_names_default(".env.", ".gitattributes").is_lt(), "filenames starting with dots and with extensions should still sort properly");
    assert!(c::compare_file_names_default(".env", ".aaa.env").is_gt(), "dotfiles sort alphabetically when they contain multiple dots");
    assert!(c::compare_file_names_default(".env", ".env.aaa").is_lt(), "dotfiles with the same root sort shortest first");
    assert!(c::compare_file_names_default(".aaa_env", ".aaa.env").is_lt(), "an underscore in a dotfile name will sort before a dot");
    assert!(c::compare_file_names_default("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_names_default(".env", "aaa").is_lt(), "dotfiles come before filenames without extensions");
    assert!(c::compare_file_names_default(".env", "aaa.env").is_lt(), "dotfiles come before filenames with extensions");
    assert!(c::compare_file_names_default(".md", "A.MD").is_lt(), "dotfiles sort before uppercase files");
    assert!(c::compare_file_names_default(".MD", "a.md").is_lt(), "dotfiles sort before lowercase files");
    assert!(c::compare_file_names_default("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_names_default("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_names_default("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_names_default("abc2.txt", "abc10.txt").is_lt(), "filenames with numbers should be in numerical order even when they are multiple digits long");
    assert!(c::compare_file_names_default("abc02.txt", "abc010.txt").is_lt(), "filenames with numbers that have leading zeros sort numerically");
    assert!(c::compare_file_names_default("abc1.10.txt", "abc1.2.txt").is_gt(), "numbers with dots between them are treated as two separate numbers, not one decimal number");
    assert!(c::compare_file_names_default("a.ext1", "b.Ext1").is_lt(), "if names are different and extensions with numbers are equal except for case, filenames are compared by full filename");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_names_default), vec!["A2.txt", "a10.txt", "a20.txt", "A100.txt"], "filenames with number and case differences compare numerically");
    assert_eq!(c::compare_file_names_default("a", "A"), c::compare_locale("a", "A"), "the same letter sorts by locale");
    assert_eq!(c::compare_file_names_default("â", "Â"), c::compare_locale("â", "Â"), "the same accented letter sorts by locale");
    assert_eq!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_names_default), sorted(&["email", "Email", "émail", "Émail"], c::compare_locale), "the same base characters with different case or accents sort in locale order");
    assert!(c::compare_file_names_default("abc02.txt", "abc002.txt").is_lt(), "filenames with equivalent numbers and leading zeros sort shortest number first");
    assert!(c::compare_file_names_default("abc.txt1", "abc.txt01").is_lt(), "same name plus extensions with equal numbers sort shortest number first");
    assert_eq!(c::compare_file_names_default("art01", "Art01"), c::compare_locale_numeric("art01", "Art01"), "a numerically equivalent word of a different case compares numerically based on locale");
    assert_eq!(c::compare_file_names_default("a.ext1", "a.Ext1"), c::compare_locale("ext1", "Ext1"), "if names are equal and extensions with numbers are equal except for case, filenames are sorted in extension locale order");
}

#[rustfmt::skip]
#[test]
fn compare_file_extensions_default() {
    assert!(c::compare_file_extensions_default("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_extensions_default("", "abc").is_lt(), "null should come before real files without extensions");
    assert!(c::compare_file_extensions_default("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_extensions_default("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_extensions_default("z", "A").is_gt(), "z comes after A");
    assert!(c::compare_file_extensions_default("Z", "a").is_gt(), "Z comes after a");
    assert!(c::compare_file_extensions_default("file.ext", "file.ext").is_eq(), "equal full filenames should be equal");
    assert!(c::compare_file_extensions_default("a.ext", "b.ext").is_lt(), "if equal extensions, filenames should be compared");
    assert!(c::compare_file_extensions_default("file.aaa", "file.bbb").is_lt(), "files with equal names should be compared by extensions");
    assert!(c::compare_file_extensions_default("bbb.aaa", "aaa.bbb").is_lt(), "files should be compared by extension first");
    assert!(c::compare_file_extensions_default(".abc", ".abc").is_eq(), "equal dotfiles should be equal");
    assert!(c::compare_file_extensions_default(".md", ".Gitattributes").is_gt(), "dotfiles sort alphabetically regardless of case");
    assert!(c::compare_file_extensions_default("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_extensions_default(".env", "aaa.env").is_lt(), "dotfiles come before filenames with extensions");
    assert!(c::compare_file_extensions_default(".MD", "a.md").is_lt(), "dotfiles sort before lowercase files");
    assert!(c::compare_file_extensions_default("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_extensions_default("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_extensions_default("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_default("abc2.txt", "abc10.txt").is_lt(), "filenames with numbers should be in numerical order");
    assert!(c::compare_file_extensions_default("abc02.txt", "abc010.txt").is_lt(), "filenames with numbers that have leading zeros sort numerically");
    assert!(c::compare_file_extensions_default("abc1.10.txt", "abc1.2.txt").is_gt(), "numbers with dots between them are treated as two separate numbers, not one decimal number");
    assert!(c::compare_file_extensions_default("abc2.txt2", "abc1.txt10").is_lt(), "extensions with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_default("txt.abc1", "txt.abc1").is_eq(), "equal extensions with numbers should be equal");
    assert!(c::compare_file_extensions_default("txt.abc1", "txt.abc2").is_lt(), "extensions with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_default("txt.abc2", "txt.abc10").is_lt(), "extensions with numbers should be in numerical order even when they are multiple digits long");
    assert!(c::compare_file_extensions_default("a.ext1", "b.ext1").is_lt(), "if equal extensions with numbers, full filenames should be compared");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_extensions_default), vec!["A2.txt", "a10.txt", "a20.txt", "A100.txt"], "filenames with number and case differences compare numerically");
    assert_eq!(c::compare_file_extensions_default("a", "A"), c::compare_locale("a", "A"), "the same letter of different case sorts by locale");
    assert_eq!(c::compare_file_extensions_default("â", "Â"), c::compare_locale("â", "Â"), "the same accented letter of different case sorts by locale");
    assert_eq!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_extensions_default), sorted(&["email", "Email", "émail", "Émail"], c::compare_locale), "the same base characters with different case or accents sort in locale order");
    assert_eq!(c::compare_file_extensions_default("a.MD", "a.md"), c::compare_locale("MD", "md"), "case differences in extensions sort by locale");
    assert_eq!(c::compare_file_extensions_default("a.md", "A.md"), c::compare_locale("a", "A"), "case differences in names sort by locale");
    assert!(c::compare_file_extensions_default("a.md", "b.MD").is_lt(), "when extensions are the same except for case, the files sort by name");
    assert!(c::compare_file_extensions_default("aggregate.go", "aggregate_repo.go").is_gt(), "names with the same extension sort in full filename locale order");
    assert!(c::compare_file_extensions_default(".env", ".aaa.env").is_gt(), "dotfiles sort alphabetically when they contain multiple dots");
    assert!(c::compare_file_extensions_default(".env", ".env.aaa").is_lt(), "dotfiles with the same root sort shortest first");
    assert!(c::compare_file_extensions_default(".env", "aaa").is_lt(), "dotfiles come before filenames without extensions");
    assert!(c::compare_file_extensions_default(".md", "A.MD").is_lt(), "dotfiles sort before uppercase files");
    assert!(c::compare_file_extensions_default("abc.txt01", "abc.txt1").is_gt(), "extensions with equal numbers should be in shortest-first order");
    assert_eq!(c::compare_file_extensions_default("art01", "Art01"), c::compare_locale_numeric("art01", "Art01"), "a numerically equivalent word of a different case compares numerically based on locale");
    assert!(c::compare_file_extensions_default("abc02.txt", "abc002.txt").is_lt(), "filenames with equivalent numbers and leading zeros sort shortest string first");
    assert!(c::compare_file_extensions_default("txt.abc01", "txt.abc1").is_gt(), "extensions with equivalent numbers sort shortest extension first");
    assert!(c::compare_file_extensions_default("a.ext1", "b.Ext1").is_lt(), "if extensions with numbers are equal except for case, full filenames should be compared");
    assert_eq!(c::compare_file_extensions_default("a.ext1", "a.Ext1"), c::compare_locale("a.ext1", "a.Ext1"), "if extensions with numbers are equal except for case, full filenames are compared in locale order");
}

#[rustfmt::skip]
#[test]
fn compare_file_names_upper() {
    assert!(c::compare_file_names_upper("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_names_upper("", "abc").is_lt(), "null should be come before real values");
    assert!(c::compare_file_names_upper("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_names_upper("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_names_upper("z", "A").is_gt(), "z comes after A");
    assert!(c::compare_file_names_upper("file.ext", "file.ext").is_eq(), "equal full names should be equal");
    assert!(c::compare_file_names_upper("a.ext", "b.ext").is_lt(), "if equal extensions, filenames should be compared");
    assert!(c::compare_file_names_upper("file.aaa", "file.bbb").is_lt(), "files with equal names should be compared by extensions");
    assert!(c::compare_file_names_upper("bbb.aaa", "aaa.bbb").is_gt(), "files should be compared by names even if extensions compare differently");
    assert!(c::compare_file_names_upper("aggregate.go", "aggregate_repo.go").is_gt(), "compares the full filename in locale order");
    assert!(c::compare_file_names_upper(".abc", ".abc").is_eq(), "equal dotfile names should be equal");
    assert!(c::compare_file_names_upper(".env.", ".gitattributes").is_lt(), "filenames starting with dots and with extensions should still sort properly");
    assert!(c::compare_file_names_upper(".env", ".aaa.env").is_gt(), "dotfiles sort alphabetically when they contain multiple dots");
    assert!(c::compare_file_names_upper(".env", ".env.aaa").is_lt(), "dotfiles with the same root sort shortest first");
    assert!(c::compare_file_names_upper(".aaa_env", ".aaa.env").is_lt(), "an underscore in a dotfile name will sort before a dot");
    assert!(c::compare_file_names_upper("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_names_upper(".env", "aaa").is_lt(), "dotfiles come before filenames without extensions");
    assert!(c::compare_file_names_upper(".env", "aaa.env").is_lt(), "dotfiles come before filenames with extensions");
    assert!(c::compare_file_names_upper(".md", "A.MD").is_lt(), "dotfiles sort before uppercase files");
    assert!(c::compare_file_names_upper(".MD", "a.md").is_lt(), "dotfiles sort before lowercase files");
    assert!(c::compare_file_names_upper("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_names_upper("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_names_upper("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_names_upper("abc2.txt", "abc10.txt").is_lt(), "filenames with numbers should be in numerical order even when they are multiple digits long");
    assert!(c::compare_file_names_upper("abc02.txt", "abc010.txt").is_lt(), "filenames with numbers that have leading zeros sort numerically");
    assert!(c::compare_file_names_upper("abc1.10.txt", "abc1.2.txt").is_gt(), "numbers with dots between them are treated as two separate numbers, not one decimal number");
    assert!(c::compare_file_names_upper("abc02.txt", "abc002.txt").is_lt(), "filenames with equivalent numbers and leading zeros sort shortest number first");
    assert!(c::compare_file_names_upper("abc.txt1", "abc.txt01").is_lt(), "same name plus extensions with equal numbers sort shortest number first");
    assert!(c::compare_file_names_upper("a.ext1", "b.Ext1").is_lt(), "different names with the equal extensions except for case are sorted by full filename");
    assert_eq!(c::compare_file_names_upper("a.ext1", "a.Ext1"), c::compare_locale("a.ext1", "a.Ext1"), "same names with equal and extensions except for case are sorted in full filename locale order");
    assert!(c::compare_file_names_upper("Z", "a").is_lt(), "Z comes before a");
    assert!(c::compare_file_names_upper("a", "A").is_gt(), "the same letter sorts uppercase first");
    assert!(c::compare_file_names_upper("â", "Â").is_gt(), "the same accented letter sorts uppercase first");
    assert_eq!(sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_file_names_upper), vec!["Art", "Artichoke", "art", "artichoke"], "names with the same root and different cases sort uppercase first");
    assert_eq!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_names_upper), vec!["Email", "Émail", "email", "émail"], "the same base characters with different case or accents sort uppercase first");
    assert!(c::compare_file_names_upper("art01", "Art01").is_gt(), "a numerically equivalent name of a different case compares uppercase first");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_names_upper), vec!["A2.txt", "A100.txt", "a10.txt", "a20.txt"], "filenames with number and case differences group by case then compare by number");
}

#[rustfmt::skip]
#[test]
fn compare_file_extensions_upper() {
    assert!(c::compare_file_extensions_upper("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_extensions_upper("", "abc").is_lt(), "null should come before real files without extensions");
    assert!(c::compare_file_extensions_upper("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_extensions_upper("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_extensions_upper("z", "A").is_gt(), "z comes after A");
    assert!(c::compare_file_extensions_upper("file.ext", "file.ext").is_eq(), "equal full filenames should be equal");
    assert!(c::compare_file_extensions_upper("a.ext", "b.ext").is_lt(), "if equal extensions, filenames should be compared");
    assert!(c::compare_file_extensions_upper("file.aaa", "file.bbb").is_lt(), "files with equal names should be compared by extensions");
    assert!(c::compare_file_extensions_upper("bbb.aaa", "aaa.bbb").is_lt(), "files should be compared by extension first");
    assert!(c::compare_file_extensions_upper("a.md", "b.MD").is_lt(), "when extensions are the same except for case, the files sort by name");
    assert_eq!(c::compare_file_extensions_upper("a.MD", "a.md"), c::compare_locale("MD", "md"), "case differences in extensions sort by locale");
    assert!(c::compare_file_extensions_upper("aggregate.go", "aggregate_repo.go").is_gt(), "when extensions are equal, compares the full filename");
    assert!(c::compare_file_extensions_upper(".abc", ".abc").is_eq(), "equal dotfiles should be equal");
    assert!(c::compare_file_extensions_upper(".md", ".Gitattributes").is_gt(), "dotfiles sort alphabetically regardless of case");
    assert!(c::compare_file_extensions_upper(".env", ".aaa.env").is_gt(), "dotfiles sort alphabetically when they contain multiple dots");
    assert!(c::compare_file_extensions_upper(".env", ".env.aaa").is_lt(), "dotfiles with the same root sort shortest first");
    assert!(c::compare_file_extensions_upper("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_extensions_upper(".env", "aaa.env").is_lt(), "dotfiles come before filenames with extensions");
    assert!(c::compare_file_extensions_upper(".MD", "a.md").is_lt(), "dotfiles sort before lowercase files");
    assert!(c::compare_file_extensions_upper(".env", "aaa").is_lt(), "dotfiles come before filenames without extensions");
    assert!(c::compare_file_extensions_upper(".md", "A.MD").is_lt(), "dotfiles sort before uppercase files");
    assert!(c::compare_file_extensions_upper("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_extensions_upper("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_extensions_upper("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_upper("abc2.txt", "abc10.txt").is_lt(), "filenames with numbers should be in numerical order");
    assert!(c::compare_file_extensions_upper("abc02.txt", "abc010.txt").is_lt(), "filenames with numbers that have leading zeros sort numerically");
    assert!(c::compare_file_extensions_upper("abc1.10.txt", "abc1.2.txt").is_gt(), "numbers with dots between them are treated as two separate numbers, not one decimal number");
    assert!(c::compare_file_extensions_upper("abc2.txt2", "abc1.txt10").is_lt(), "extensions with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_upper("txt.abc1", "txt.abc1").is_eq(), "equal extensions with numbers should be equal");
    assert!(c::compare_file_extensions_upper("txt.abc1", "txt.abc2").is_lt(), "extensions with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_upper("txt.abc2", "txt.abc10").is_lt(), "extensions with numbers should be in numerical order even when they are multiple digits long");
    assert!(c::compare_file_extensions_upper("a.ext1", "b.ext1").is_lt(), "if equal extensions with numbers, full filenames should be compared");
    assert!(c::compare_file_extensions_upper("abc.txt01", "abc.txt1").is_gt(), "extensions with equal numbers should be in shortest-first order");
    assert!(c::compare_file_extensions_upper("abc02.txt", "abc002.txt").is_lt(), "filenames with equivalent numbers and leading zeros sort shortest string first");
    assert!(c::compare_file_extensions_upper("txt.abc01", "txt.abc1").is_gt(), "extensions with equivalent numbers sort shortest extension first");
    assert!(c::compare_file_extensions_upper("a.ext1", "b.Ext1").is_lt(), "different names and extensions that are equal except for case are sorted in full filename order");
    assert_eq!(c::compare_file_extensions_upper("a.ext1", "a.Ext1"), c::compare_locale("a.ext1", "b.Ext1"), "same names and extensions that are equal except for case are sorted in full filename locale order");
    assert!(c::compare_file_extensions_upper("Z", "a").is_lt(), "Z comes before a");
    assert!(c::compare_file_extensions_upper("a", "A").is_gt(), "the same letter sorts uppercase first");
    assert!(c::compare_file_extensions_upper("â", "Â").is_gt(), "the same accented letter sorts uppercase first");
    assert_eq!(sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_file_extensions_upper), vec!["Art", "Artichoke", "art", "artichoke"], "names with the same root and different cases sort uppercase names first");
    assert_eq!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_extensions_upper), vec!["Email", "Émail", "email", "émail"], "the same base characters with different case or accents sort uppercase names first");
    assert!(c::compare_file_extensions_upper("a.md", "A.md").is_gt(), "case differences in names sort uppercase first");
    assert!(c::compare_file_extensions_upper("art01", "Art01").is_gt(), "a numerically equivalent word of a different case sorts uppercase first");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_extensions_upper), vec!["A2.txt", "A100.txt", "a10.txt", "a20.txt"], "filenames with number and case differences group by case then sort by number");
}

#[rustfmt::skip]
#[test]
fn compare_file_names_lower() {
    assert!(c::compare_file_names_lower("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_names_lower("", "abc").is_lt(), "null should be come before real values");
    assert!(c::compare_file_names_lower("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_names_lower("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_names_lower("Z", "a").is_gt(), "Z comes after a");
    assert!(c::compare_file_names_lower("file.ext", "file.ext").is_eq(), "equal full names should be equal");
    assert!(c::compare_file_names_lower("a.ext", "b.ext").is_lt(), "if equal extensions, filenames should be compared");
    assert!(c::compare_file_names_lower("file.aaa", "file.bbb").is_lt(), "files with equal names should be compared by extensions");
    assert!(c::compare_file_names_lower("bbb.aaa", "aaa.bbb").is_gt(), "files should be compared by names even if extensions compare differently");
    assert!(c::compare_file_names_lower("aggregate.go", "aggregate_repo.go").is_gt(), "compares full filenames");
    assert!(c::compare_file_names_lower(".abc", ".abc").is_eq(), "equal dotfile names should be equal");
    assert!(c::compare_file_names_lower(".env.", ".gitattributes").is_lt(), "filenames starting with dots and with extensions should still sort properly");
    assert!(c::compare_file_names_lower(".env", ".aaa.env").is_gt(), "dotfiles sort alphabetically when they contain multiple dots");
    assert!(c::compare_file_names_lower(".env", ".env.aaa").is_lt(), "dotfiles with the same root sort shortest first");
    assert!(c::compare_file_names_lower(".aaa_env", ".aaa.env").is_lt(), "an underscore in a dotfile name will sort before a dot");
    assert!(c::compare_file_names_lower("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_names_lower(".env", "aaa").is_lt(), "dotfiles come before filenames without extensions");
    assert!(c::compare_file_names_lower(".env", "aaa.env").is_lt(), "dotfiles come before filenames with extensions");
    assert!(c::compare_file_names_lower(".md", "A.MD").is_lt(), "dotfiles sort before uppercase files");
    assert!(c::compare_file_names_lower(".MD", "a.md").is_lt(), "dotfiles sort before lowercase files");
    assert!(c::compare_file_names_lower("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_names_lower("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_names_lower("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_names_lower("abc2.txt", "abc10.txt").is_lt(), "filenames with numbers should be in numerical order even when they are multiple digits long");
    assert!(c::compare_file_names_lower("abc02.txt", "abc010.txt").is_lt(), "filenames with numbers that have leading zeros sort numerically");
    assert!(c::compare_file_names_lower("abc1.10.txt", "abc1.2.txt").is_gt(), "numbers with dots between them are treated as two separate numbers, not one decimal number");
    assert!(c::compare_file_names_lower("abc02.txt", "abc002.txt").is_lt(), "filenames with equivalent numbers and leading zeros sort shortest number first");
    assert!(c::compare_file_names_lower("abc.txt1", "abc.txt01").is_lt(), "same name plus extensions with equal numbers sort shortest number first");
    assert!(c::compare_file_names_lower("a.ext1", "b.Ext1").is_lt(), "different names and extensions that are equal except for case are sorted in full filename order");
    assert_eq!(c::compare_file_names_lower("a.ext1", "a.Ext1"), c::compare_locale("a.ext1", "b.Ext1"), "same names and extensions that are equal except for case are sorted in full filename locale order");
    assert!(c::compare_file_names_lower("z", "A").is_lt(), "z comes before A");
    assert!(c::compare_file_names_lower("a", "A").is_lt(), "the same letter sorts lowercase first");
    assert!(c::compare_file_names_lower("â", "Â").is_lt(), "the same accented letter sorts lowercase first");
    assert_eq!(sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_file_names_lower), vec!["art", "artichoke", "Art", "Artichoke"], "names with the same root and different cases sort lowercase first");
    assert_eq!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_names_lower), vec!["email", "émail", "Email", "Émail"], "the same base characters with different case or accents sort lowercase first");
    assert!(c::compare_file_names_lower("art01", "Art01").is_lt(), "a numerically equivalent name of a different case compares lowercase first");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_names_lower), vec!["a10.txt", "a20.txt", "A2.txt", "A100.txt"], "filenames with number and case differences group by case then compare by number");
}

#[rustfmt::skip]
#[test]
fn compare_file_extensions_lower() {
    assert!(c::compare_file_extensions_lower("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_extensions_lower("", "abc").is_lt(), "null should come before real files without extensions");
    assert!(c::compare_file_extensions_lower("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_extensions_lower("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_extensions_lower("Z", "a").is_gt(), "Z comes after a");
    assert!(c::compare_file_extensions_lower("file.ext", "file.ext").is_eq(), "equal full filenames should be equal");
    assert!(c::compare_file_extensions_lower("a.ext", "b.ext").is_lt(), "if equal extensions, filenames should be compared");
    assert!(c::compare_file_extensions_lower("file.aaa", "file.bbb").is_lt(), "files with equal names should be compared by extensions");
    assert!(c::compare_file_extensions_lower("bbb.aaa", "aaa.bbb").is_lt(), "files should be compared by extension first");
    assert!(c::compare_file_extensions_lower("a.md", "b.MD").is_lt(), "when extensions are the same except for case, the files sort by name");
    assert_eq!(c::compare_file_extensions_lower("a.MD", "a.md"), c::compare_locale("MD", "md"), "case differences in extensions sort by locale");
    assert!(c::compare_file_extensions_lower(".abc", ".abc").is_eq(), "equal dotfiles should be equal");
    assert!(c::compare_file_extensions_lower(".md", ".Gitattributes").is_gt(), "dotfiles sort alphabetically regardless of case");
    assert!(c::compare_file_extensions_lower(".env", ".aaa.env").is_gt(), "dotfiles sort alphabetically when they contain multiple dots");
    assert!(c::compare_file_extensions_lower(".env", ".env.aaa").is_lt(), "dotfiles with the same root sort shortest first");
    assert!(c::compare_file_extensions_lower("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_extensions_lower(".env", "aaa.env").is_lt(), "dotfiles come before filenames with extensions");
    assert!(c::compare_file_extensions_lower(".MD", "a.md").is_lt(), "dotfiles sort before lowercase files");
    assert!(c::compare_file_extensions_lower(".env", "aaa").is_lt(), "dotfiles come before filenames without extensions");
    assert!(c::compare_file_extensions_lower(".md", "A.MD").is_lt(), "dotfiles sort before uppercase files");
    assert!(c::compare_file_extensions_lower("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_extensions_lower("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_extensions_lower("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_lower("abc2.txt", "abc10.txt").is_lt(), "filenames with numbers should be in numerical order");
    assert!(c::compare_file_extensions_lower("abc02.txt", "abc010.txt").is_lt(), "filenames with numbers that have leading zeros sort numerically");
    assert!(c::compare_file_extensions_lower("abc1.10.txt", "abc1.2.txt").is_gt(), "numbers with dots between them are treated as two separate numbers, not one decimal number");
    assert!(c::compare_file_extensions_lower("abc2.txt2", "abc1.txt10").is_lt(), "extensions with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_lower("txt.abc1", "txt.abc1").is_eq(), "equal extensions with numbers should be equal");
    assert!(c::compare_file_extensions_lower("txt.abc1", "txt.abc2").is_lt(), "extensions with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_lower("txt.abc2", "txt.abc10").is_lt(), "extensions with numbers should be in numerical order even when they are multiple digits long");
    assert!(c::compare_file_extensions_lower("a.ext1", "b.ext1").is_lt(), "if equal extensions with numbers, full filenames should be compared");
    assert!(c::compare_file_extensions_lower("abc.txt01", "abc.txt1").is_gt(), "extensions with equal numbers should be in shortest-first order");
    assert!(c::compare_file_extensions_lower("abc02.txt", "abc002.txt").is_lt(), "filenames with equivalent numbers and leading zeros sort shortest string first");
    assert!(c::compare_file_extensions_lower("txt.abc01", "txt.abc1").is_gt(), "extensions with equivalent numbers sort shortest extension first");
    assert!(c::compare_file_extensions_lower("a.ext1", "b.Ext1").is_lt(), "if extensions with numbers are equal except for case, full filenames should be compared");
    assert_eq!(c::compare_file_extensions_lower("a.ext1", "a.Ext1"), c::compare_locale("a.ext1", "a.Ext1"), "if extensions with numbers are equal except for case, filenames are sorted in locale order");
    assert!(c::compare_file_extensions_lower("z", "A").is_lt(), "z comes before A");
    assert!(c::compare_file_extensions_lower("a", "A").is_lt(), "the same letter sorts lowercase first");
    assert!(c::compare_file_extensions_lower("â", "Â").is_lt(), "the same accented letter sorts lowercase first");
    assert_eq!(sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_file_extensions_lower), vec!["art", "artichoke", "Art", "Artichoke"], "names with the same root and different cases sort lowercase names first");
    assert_eq!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_extensions_lower), vec!["email", "émail", "Email", "Émail"], "the same base characters with different case or accents sort lowercase names first");
    assert!(c::compare_file_extensions_lower("a.md", "A.md").is_lt(), "case differences in names sort lowercase first");
    assert!(c::compare_file_extensions_lower("art01", "Art01").is_lt(), "a numerically equivalent word of a different case sorts lowercase first");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_extensions_lower), vec!["a10.txt", "a20.txt", "A2.txt", "A100.txt"], "filenames with number and case differences group by case then sort by number");
    assert!(c::compare_file_extensions_lower("aggregate.go", "aggregate_repo.go").is_gt(), "when extensions are equal, compares full filenames");
}

#[rustfmt::skip]
#[test]
fn compare_file_names_unicode() {
    assert!(c::compare_file_names_unicode("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_names_unicode("", "abc").is_lt(), "null should be come before real values");
    assert!(c::compare_file_names_unicode("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_names_unicode("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_names_unicode("z", "A").is_gt(), "z comes after A");
    assert!(c::compare_file_names_unicode("file.ext", "file.ext").is_eq(), "equal full names should be equal");
    assert!(c::compare_file_names_unicode("a.ext", "b.ext").is_lt(), "if equal extensions, filenames should be compared");
    assert!(c::compare_file_names_unicode("file.aaa", "file.bbb").is_lt(), "files with equal names should be compared by extensions");
    assert!(c::compare_file_names_unicode("bbb.aaa", "aaa.bbb").is_gt(), "files should be compared by names even if extensions compare differently");
    assert!(c::compare_file_names_unicode(".abc", ".abc").is_eq(), "equal dotfile names should be equal");
    assert!(c::compare_file_names_unicode(".env.", ".gitattributes").is_lt(), "filenames starting with dots and with extensions should still sort properly");
    assert!(c::compare_file_names_unicode(".env", ".aaa.env").is_gt(), "dotfiles sort alphabetically when they contain multiple dots");
    assert!(c::compare_file_names_unicode(".env", ".env.aaa").is_lt(), "dotfiles with the same root sort shortest first");
    assert!(c::compare_file_names_unicode("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_names_unicode(".env", "aaa").is_lt(), "dotfiles come before filenames without extensions");
    assert!(c::compare_file_names_unicode(".env", "aaa.env").is_lt(), "dotfiles come before filenames with extensions");
    assert!(c::compare_file_names_unicode(".md", "A.MD").is_lt(), "dotfiles sort before uppercase files");
    assert!(c::compare_file_names_unicode(".MD", "a.md").is_lt(), "dotfiles sort before lowercase files");
    assert!(c::compare_file_names_unicode("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_names_unicode("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_names_unicode("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_names_unicode("a.ext1", "b.Ext1").is_lt(), "if names are different and extensions with numbers are equal except for case, filenames are sorted by unicode full filename");
    assert!(c::compare_file_names_unicode("a.ext1", "a.Ext1").is_gt(), "if names are equal and extensions with numbers are equal except for case, filenames are sorted by unicode full filename");
    assert!(c::compare_file_names_unicode("Z", "a").is_lt(), "Z comes before a");
    assert!(c::compare_file_names_unicode("a", "A").is_gt(), "the same letter sorts uppercase first");
    assert!(c::compare_file_names_unicode("â", "Â").is_gt(), "the same accented letter sorts uppercase first");
    assert_eq!(sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_file_names_unicode), vec!["Art", "Artichoke", "art", "artichoke"], "names with the same root and different cases sort uppercase first");
    assert_eq!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_names_unicode), vec!["Email", "email", "Émail", "émail"], "the same base characters with different case or accents sort in unicode order");
    assert!(c::compare_file_names_unicode("aggregate.go", "aggregate_repo.go").is_lt(), "compares the whole name in unicode order, but dot comes before underscore");
    assert!(c::compare_file_names_unicode(".aaa_env", ".aaa.env").is_gt(), "an underscore in a dotfile name will sort after a dot");
    assert!(c::compare_file_names_unicode("abc2.txt", "abc10.txt").is_gt(), "filenames with numbers should be in unicode order even when they are multiple digits long");
    assert!(c::compare_file_names_unicode("abc02.txt", "abc010.txt").is_gt(), "filenames with numbers that have leading zeros sort in unicode order");
    assert!(c::compare_file_names_unicode("abc1.10.txt", "abc1.2.txt").is_lt(), "numbers with dots between them are sorted in unicode order");
    assert!(c::compare_file_names_unicode("abc02.txt", "abc002.txt").is_gt(), "filenames with equivalent numbers and leading zeros sort in unicode order");
    assert!(c::compare_file_names_unicode("abc.txt1", "abc.txt01").is_gt(), "same name plus extensions with equal numbers sort in unicode order");
    assert!(c::compare_file_names_unicode("art01", "Art01").is_gt(), "a numerically equivalent name of a different case compares uppercase first");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_names_unicode), vec!["A100.txt", "A2.txt", "a10.txt", "a20.txt"], "filenames with number and case differences sort in unicode order");
}

#[rustfmt::skip]
#[test]
fn compare_file_extensions_unicode() {
    assert!(c::compare_file_extensions_unicode("", "").is_eq(), "null should be equal");
    assert!(c::compare_file_extensions_unicode("", "abc").is_lt(), "null should come before real files without extensions");
    assert!(c::compare_file_extensions_unicode("", "").is_eq(), "empty should be equal");
    assert!(c::compare_file_extensions_unicode("abc", "abc").is_eq(), "equal names should be equal");
    assert!(c::compare_file_extensions_unicode("z", "A").is_gt(), "z comes after A");
    assert!(c::compare_file_extensions_unicode("file.ext", "file.ext").is_eq(), "equal full filenames should be equal");
    assert!(c::compare_file_extensions_unicode("a.ext", "b.ext").is_lt(), "if equal extensions, filenames should be compared");
    assert!(c::compare_file_extensions_unicode("file.aaa", "file.bbb").is_lt(), "files with equal names should be compared by extensions");
    assert!(c::compare_file_extensions_unicode("bbb.aaa", "aaa.bbb").is_lt(), "files should be compared by extension first");
    assert!(c::compare_file_extensions_unicode("a.md", "b.MD").is_lt(), "when extensions are the same except for case, the files sort by name");
    assert!(c::compare_file_extensions_unicode("a.MD", "a.md").is_lt(), "case differences in extensions sort in unicode order");
    assert!(c::compare_file_extensions_unicode(".abc", ".abc").is_eq(), "equal dotfiles should be equal");
    assert!(c::compare_file_extensions_unicode(".md", ".Gitattributes").is_gt(), "dotfiles sort alphabetically regardless of case");
    assert!(c::compare_file_extensions_unicode(".env", ".aaa.env").is_gt(), "dotfiles sort alphabetically when they contain multiple dots");
    assert!(c::compare_file_extensions_unicode(".env", ".env.aaa").is_lt(), "dotfiles with the same root sort shortest first");
    assert!(c::compare_file_extensions_unicode("", ".abc").is_lt(), "null should come before dotfiles");
    assert!(c::compare_file_extensions_unicode(".env", "aaa.env").is_lt(), "dotfiles come before filenames with extensions");
    assert!(c::compare_file_extensions_unicode(".MD", "a.md").is_lt(), "dotfiles sort before lowercase files");
    assert!(c::compare_file_extensions_unicode(".env", "aaa").is_lt(), "dotfiles come before filenames without extensions");
    assert!(c::compare_file_extensions_unicode(".md", "A.MD").is_lt(), "dotfiles sort before uppercase files");
    assert!(c::compare_file_extensions_unicode("1", "1").is_eq(), "numerically equal full names should be equal");
    assert!(c::compare_file_extensions_unicode("abc1.txt", "abc1.txt").is_eq(), "equal filenames with numbers should be equal");
    assert!(c::compare_file_extensions_unicode("abc1.txt", "abc2.txt").is_lt(), "filenames with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_unicode("txt.abc1", "txt.abc1").is_eq(), "equal extensions with numbers should be equal");
    assert!(c::compare_file_extensions_unicode("txt.abc1", "txt.abc2").is_lt(), "extensions with numbers should be in numerical order, not alphabetical order");
    assert!(c::compare_file_extensions_unicode("a.ext1", "b.ext1").is_lt(), "if equal extensions with numbers, full filenames should be compared");
    assert!(c::compare_file_extensions_unicode("Z", "a").is_lt(), "Z comes before a");
    assert!(c::compare_file_extensions_unicode("a", "A").is_gt(), "the same letter sorts uppercase first");
    assert!(c::compare_file_extensions_unicode("â", "Â").is_gt(), "the same accented letter sorts uppercase first");
    assert_eq!(sorted(&["artichoke", "Artichoke", "art", "Art"], c::compare_file_extensions_unicode), vec!["Art", "Artichoke", "art", "artichoke"], "names with the same root and different cases sort uppercase names first");
    assert_eq!(sorted(&["email", "Email", "émail", "Émail"], c::compare_file_extensions_unicode), vec!["Email", "email", "Émail", "émail"], "the same base characters with different case or accents sort in unicode order");
    assert!(c::compare_file_extensions_unicode("a.MD", "a.md").is_lt(), "case differences in extensions sort by uppercase extension first");
    assert!(c::compare_file_extensions_unicode("a.md", "A.md").is_gt(), "case differences in names sort uppercase first");
    assert!(c::compare_file_extensions_unicode("art01", "Art01").is_gt(), "a numerically equivalent name of a different case sorts uppercase first");
    assert_eq!(sorted(&["a10.txt", "A2.txt", "A100.txt", "a20.txt"], c::compare_file_extensions_unicode), vec!["A100.txt", "A2.txt", "a10.txt", "a20.txt"], "filenames with number and case differences sort in unicode order");
    assert!(c::compare_file_extensions_unicode("aggregate.go", "aggregate_repo.go").is_lt(), "when extensions are equal, compares full filenames in unicode order");
    assert!(c::compare_file_extensions_unicode("abc2.txt", "abc10.txt").is_gt(), "filenames with numbers should be in unicode order");
    assert!(c::compare_file_extensions_unicode("abc02.txt", "abc010.txt").is_gt(), "filenames with numbers that have leading zeros sort in unicode order");
    assert!(c::compare_file_extensions_unicode("abc1.10.txt", "abc1.2.txt").is_lt(), "numbers with dots between them sort in unicode order");
    assert!(c::compare_file_extensions_unicode("abc2.txt2", "abc1.txt10").is_gt(), "extensions with numbers should be in unicode order");
    assert!(c::compare_file_extensions_unicode("txt.abc2", "txt.abc10").is_gt(), "extensions with numbers should be in unicode order even when they are multiple digits long");
    assert!(c::compare_file_extensions_unicode("abc.txt01", "abc.txt1").is_lt(), "extensions with equal numbers should be in unicode order");
    assert!(c::compare_file_extensions_unicode("abc02.txt", "abc002.txt").is_gt(), "filenames with equivalent numbers and leading zeros sort in unicode order");
    assert!(c::compare_file_extensions_unicode("txt.abc01", "txt.abc1").is_lt(), "extensions with equivalent numbers sort in unicode order");
    assert!(c::compare_file_extensions_unicode("a.ext1", "b.Ext1").is_lt(), "if extensions with numbers are equal except for case, unicode full filenames should be compared");
    assert!(c::compare_file_extensions_unicode("a.ext1", "a.Ext1").is_gt(), "if extensions with numbers are equal except for case, unicode full filenames should be compared");
}

#[test]
fn compare_anything_and_compare_by_prefix() {
    // Not an upstream test (upstream covers these through fuzzyScorer.test.ts).
    assert!(c::compare_by_prefix("Foo.ts", "barFoo.ts", "foo").is_lt());
    assert!(c::compare_by_prefix("foobar.ts", "Foo.ts", "foo").is_gt());
    assert!(c::compare_by_prefix("a", "b", "foo").is_eq());
    assert!(c::compare_anything("xfoo", "xbar", "foo").is_lt());
    assert!(c::compare_anything("file10", "file9", "x").is_gt());
}

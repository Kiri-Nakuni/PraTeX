//! `kpsewhich --show-path` が返す探索 path の保守的な部分集合。
//!
//! 展開規則そのものを再実装せず、`kpsewhich` が展開した結果だけを読む。意味を一意に
//! 判定できない要素は `Unsupported` のまま残し、呼び出し側が one-shot 探索へ戻れる
//! ようにする。

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SearchPath {
    elements: Vec<SearchPathElement>,
}

impl SearchPath {
    pub(super) fn parse(value: &OsStr) -> Option<Self> {
        if value.is_empty() {
            return None;
        }
        let elements = std::env::split_paths(value)
            .map(parse_element)
            .collect::<Vec<_>>();
        if elements.is_empty() {
            None
        } else {
            Some(Self { elements })
        }
    }

    pub(super) fn elements(&self) -> &[SearchPathElement] {
        &self.elements
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SearchPathElement {
    /// 直接 path の存在確認ですでに扱った要素。
    CurrentDirectory,
    Supported {
        database_only: bool,
        pattern: SearchPattern,
    },
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SearchPattern {
    Exact(PathBuf),
    Recursive { prefix: PathBuf, suffix: PathBuf },
}

impl SearchPattern {
    pub(super) fn matches_directory(&self, directory: &Path) -> bool {
        match self {
            Self::Exact(expected) => directory == expected,
            Self::Recursive { prefix, suffix } => {
                let Ok(remainder) = directory.strip_prefix(prefix) else {
                    return false;
                };
                suffix.as_os_str().is_empty() || path_ends_with(remainder, suffix)
            }
        }
    }

    /// Kpathsea の仕様上、この root の database が当該要素へ適用されるかを調べる。
    pub(super) fn is_covered_by(&self, database_root: &Path) -> bool {
        match self {
            Self::Exact(directory) => directory.starts_with(database_root),
            Self::Recursive { prefix, .. } => prefix.starts_with(database_root),
        }
    }

    pub(super) fn disk_root(&self) -> &Path {
        match self {
            Self::Exact(directory) => directory,
            Self::Recursive { prefix, .. } => prefix,
        }
    }

    pub(super) fn exact_directory(&self) -> Option<&Path> {
        match self {
            Self::Exact(directory) => Some(directory),
            Self::Recursive { .. } => None,
        }
    }
}

fn parse_element(path: PathBuf) -> SearchPathElement {
    let Some(raw) = os_bytes(path.as_os_str()) else {
        return SearchPathElement::Unsupported;
    };
    let (database_only, raw) = match raw.strip_prefix(b"!!") {
        Some(raw) => (true, raw),
        None => (false, raw.as_ref()),
    };
    if raw.is_empty()
        || raw.contains(&b'$')
        || raw.contains(&b'{')
        || raw.contains(&b'}')
        || raw.starts_with(b"~")
    {
        return SearchPathElement::Unsupported;
    }

    let Some(without_flags) = os_string_from_bytes(raw) else {
        return SearchPathElement::Unsupported;
    };
    let without_flags = PathBuf::from(without_flags);
    if without_flags == Path::new(".") {
        return SearchPathElement::CurrentDirectory;
    }

    let recursive_run = match one_recursive_run(raw) {
        Ok(Some(recursive_run)) => recursive_run,
        Ok(None) => {
            return if without_flags.is_absolute() {
                SearchPathElement::Supported {
                    database_only,
                    pattern: SearchPattern::Exact(without_flags),
                }
            } else {
                SearchPathElement::Unsupported
            };
        }
        Err(()) => return SearchPathElement::Unsupported,
    };
    let (start, end) = recursive_run;
    // `//server/share` のような UNC 表記を再帰記号と誤認しない。
    if start == 0 {
        return SearchPathElement::Unsupported;
    }
    let Some(prefix) = os_string_from_bytes(&raw[..start]).map(PathBuf::from) else {
        return SearchPathElement::Unsupported;
    };
    let Some(suffix) = os_string_from_bytes(&raw[end..]).map(PathBuf::from) else {
        return SearchPathElement::Unsupported;
    };
    if !prefix.is_absolute() || !is_safe_suffix(&suffix) {
        return SearchPathElement::Unsupported;
    }
    SearchPathElement::Supported {
        database_only,
        pattern: SearchPattern::Recursive { prefix, suffix },
    }
}

fn one_recursive_run(bytes: &[u8]) -> Result<Option<(usize, usize)>, ()> {
    let mut found = None;
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b'/' && bytes[index + 1] == b'/' {
            let start = index;
            index += 2;
            while index < bytes.len() && bytes[index] == b'/' {
                index += 1;
            }
            if found.is_some() {
                return Err(());
            }
            found = Some((start, index));
        } else {
            index += 1;
        }
    }
    Ok(found)
}

fn is_safe_suffix(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn path_ends_with(path: &Path, suffix: &Path) -> bool {
    let mut path_components = path.components().rev();
    suffix
        .components()
        .rev()
        .all(|expected| path_components.next() == Some(expected))
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> Option<Cow<'_, [u8]>> {
    use std::os::unix::ffi::OsStrExt;
    Some(Cow::Borrowed(value.as_bytes()))
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> Option<Cow<'_, [u8]>> {
    value
        .to_str()
        .map(|value| Cow::Owned(value.as_bytes().to_vec()))
}

#[cfg(unix)]
fn os_string_from_bytes(value: &[u8]) -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt;
    Some(OsString::from_vec(value.to_vec()))
}

#[cfg(not(unix))]
fn os_string_from_bytes(value: &[u8]) -> Option<OsString> {
    String::from_utf8(value.to_vec()).ok().map(OsString::from)
}

#[cfg(test)]
mod tests {
    use super::{SearchPath, SearchPathElement, SearchPattern};
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    fn joined(elements: impl IntoIterator<Item = PathBuf>) -> OsString {
        std::env::join_paths(elements).unwrap()
    }

    fn recursive(root: &Path, suffix: &str) -> PathBuf {
        let mut value = root.as_os_str().to_os_string();
        value.push("//");
        value.push(suffix);
        PathBuf::from(value)
    }

    #[test]
    fn exactとdatabase限定と現在directoryを順序どおり読む() {
        let root = std::env::temp_dir().join("pratex-search-path-exact");
        let mut database_only = OsString::from("!!");
        database_only.push(&root);
        let path = joined([PathBuf::from("."), PathBuf::from(database_only)]);
        let parsed = SearchPath::parse(&path).unwrap();

        assert_eq!(parsed.elements()[0], SearchPathElement::CurrentDirectory);
        assert_eq!(
            parsed.elements()[1],
            SearchPathElement::Supported {
                database_only: true,
                pattern: SearchPattern::Exact(root),
            }
        );
    }

    #[test]
    fn 一つの再帰記号だけをprefixとsuffixへ分ける() {
        let root = std::env::temp_dir().join("pratex-search-path-recursive");
        let path = joined([recursive(&root, "latex")]);
        let parsed = SearchPath::parse(&path).unwrap();
        let SearchPathElement::Supported { pattern, .. } = &parsed.elements()[0] else {
            panic!("再帰要素にならなかった");
        };

        assert!(pattern.matches_directory(&root.join("a/b/latex")));
        assert!(!pattern.matches_directory(&root.join("a/b/plain")));
    }

    #[test]
    fn 末尾の三重slashも一つの再帰記号として読む() {
        let root = std::env::temp_dir().join("pratex-search-path-triple");
        let mut value = root.as_os_str().to_os_string();
        value.push("///");
        let parsed = SearchPath::parse(&joined([PathBuf::from(value)])).unwrap();
        let SearchPathElement::Supported { pattern, .. } = &parsed.elements()[0] else {
            panic!("再帰要素にならなかった");
        };

        assert!(pattern.matches_directory(&root));
        assert!(pattern.matches_directory(&root.join("deep/tree")));
    }

    #[test]
    fn 複数の再帰記号と未展開記号と相対pathを保守的に拒む() {
        let root = std::env::temp_dir().join("pratex-search-path-unsupported");
        let mut multiple = root.as_os_str().to_os_string();
        multiple.push("//a//b");
        let parsed = SearchPath::parse(&joined([
            PathBuf::from(multiple),
            PathBuf::from("$TEXMF/tex"),
            PathBuf::from("relative/tree"),
        ]))
        .unwrap();

        assert!(parsed
            .elements()
            .iter()
            .all(|element| *element == SearchPathElement::Unsupported));
    }
}

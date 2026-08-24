//! Kpathsea が公開している `ls-R` 形式の、読み取り専用索引。
//!
//! このモジュールは Kpathsea の実装コードには依存せず、公開マニュアルに記載された
//! GNU `ls -R` 形式だけを扱う。探索 path の優先順位はここでは決めず、候補を呼び出し側へ
//! 返す。これにより、曖昧な場合は公開 CLI の `kpsewhich` へ戻せる。

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

const DEFAULT_MAX_DATABASE_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_MAX_ALIASES_BYTES: u64 = 16 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 1024 * 1024;
const MAX_ENTRIES: usize = 8_000_000;
const MAX_ALIASES: usize = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
struct DatabaseSnapshot {
    length: u64,
    modified: Option<SystemTime>,
}

impl DatabaseSnapshot {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
}

/// 一件の basename に対応する索引候補。
///
/// directory と path を分けるのは、後段で format ごとの search path と照合するためである。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LsRCandidate {
    directory: PathBuf,
    path: PathBuf,
}

impl LsRCandidate {
    pub(super) fn directory(&self) -> &Path {
        &self.directory
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

/// 一つの `ls-R` を、directory を共有した形で保持する索引。
#[derive(Debug)]
pub(super) struct LsRDatabase {
    database_path: PathBuf,
    root: PathBuf,
    snapshot: DatabaseSnapshot,
    directories: Vec<PathBuf>,
    by_basename: HashMap<OsString, CandidateDirectories>,
    aliases: AliasIndex,
}

#[derive(Debug)]
struct AliasIndex {
    snapshot: Option<DatabaseSnapshot>,
    names: HashSet<OsString>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AliasMatch {
    No,
    Yes,
    UnsupportedName,
}

/// 大半の basename は一つの directory にしか現れないため、その場合は heap 上の
/// `Vec` を作らない。同名が現れたときだけ複数形へ昇格させる。
#[derive(Debug)]
enum CandidateDirectories {
    One(usize),
    Many(Vec<usize>),
}

impl CandidateDirectories {
    fn insert(&mut self, directory_id: usize) {
        match self {
            Self::One(existing) if *existing == directory_id => {}
            Self::One(existing) => {
                let first = *existing;
                *self = Self::Many(vec![first, directory_id]);
            }
            Self::Many(directory_ids) if directory_ids.contains(&directory_id) => {}
            Self::Many(directory_ids) => directory_ids.push(directory_id),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(directory_ids) => directory_ids.len(),
        }
    }

    fn visit(&self, mut visitor: impl FnMut(usize)) {
        match self {
            Self::One(directory_id) => visitor(*directory_id),
            Self::Many(directory_ids) => {
                for &directory_id in directory_ids {
                    visitor(directory_id);
                }
            }
        }
    }
}

impl LsRDatabase {
    pub(super) fn load(database_path: impl Into<PathBuf>) -> Result<Self, LsRError> {
        Self::load_with_limit(database_path.into(), DEFAULT_MAX_DATABASE_BYTES)
    }

    fn load_with_limit(database_path: PathBuf, max_bytes: u64) -> Result<Self, LsRError> {
        let root = database_path
            .parent()
            .ok_or_else(|| LsRError::Malformed("ls-R has no parent directory"))?
            .to_path_buf();
        let file = File::open(&database_path).map_err(|source| LsRError::Open {
            path: database_path.clone(),
            source,
        })?;
        let before = file.metadata().map_err(|source| LsRError::Inspect {
            path: database_path.clone(),
            source,
        })?;
        if before.len() > max_bytes {
            return Err(LsRError::TooLarge { limit: max_bytes });
        }

        // metadata の長さだけを信用せず、伸長した場合も limit + 1 byte で止める。
        let capacity = before.len().min(max_bytes).min(usize::MAX as u64) as usize;
        let mut bytes = Vec::with_capacity(capacity);
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| LsRError::Read {
                path: database_path.clone(),
                source,
            })?;
        if bytes.len() as u64 > max_bytes {
            return Err(LsRError::TooLarge { limit: max_bytes });
        }

        let after = fs::metadata(&database_path).map_err(|source| LsRError::Inspect {
            path: database_path.clone(),
            source,
        })?;
        let before = DatabaseSnapshot::from_metadata(&before);
        let after = DatabaseSnapshot::from_metadata(&after);
        if before != after {
            return Err(LsRError::ChangedDuringRead);
        }

        let (directories, by_basename) = parse_database(&root, &bytes)?;
        let aliases = load_aliases(&root)?;
        Ok(Self {
            database_path,
            root,
            snapshot: after,
            directories,
            by_basename,
            aliases,
        })
    }

    pub(super) fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn alias_match(&self, logical_name: &OsStr) -> AliasMatch {
        if self.aliases.names.is_empty() {
            return AliasMatch::No;
        }
        if !is_plain_basename(logical_name) {
            return AliasMatch::UnsupportedName;
        }
        if self.aliases.names.contains(logical_name) {
            AliasMatch::Yes
        } else {
            AliasMatch::No
        }
    }

    /// run-local snapshot が書き換わっていない場合だけ true を返す。
    pub(super) fn is_unchanged(&self) -> bool {
        let database_unchanged = fs::metadata(&self.database_path)
            .map(|metadata| DatabaseSnapshot::from_metadata(&metadata) == self.snapshot)
            .unwrap_or(false);
        database_unchanged && aliases_are_unchanged(&self.root, self.aliases.snapshot.as_ref())
    }

    /// 論理名の basename と、指定されていれば directory suffix を満たす候補を返す。
    ///
    /// 絶対 path、`.`、`..` を含む名前は直接 path 探索の領域なので、索引では扱わない。
    /// `None` は索引対象外、`Some([])` は索引上の候補なしを表す。
    pub(super) fn candidates(&self, logical_name: &OsStr) -> Option<Vec<LsRCandidate>> {
        let logical_path = Path::new(logical_name);
        if logical_path.is_absolute() {
            return None;
        }

        let mut normal_components = Vec::new();
        for component in logical_path.components() {
            match component {
                Component::Normal(component) => normal_components.push(component),
                Component::Prefix(_)
                | Component::RootDir
                | Component::CurDir
                | Component::ParentDir => return None,
            }
        }
        let (basename, requested_directories) = normal_components.split_last()?;
        let directory_ids = match self.by_basename.get(*basename) {
            Some(directory_ids) => directory_ids,
            None => return Some(Vec::new()),
        };

        let mut candidates = Vec::with_capacity(directory_ids.len());
        directory_ids.visit(|directory_id| {
            let relative_directory = &self.directories[directory_id];
            if !requested_directories.is_empty()
                && !ends_with_components(relative_directory, requested_directories)
            {
                return;
            }
            let directory = self.root.join(relative_directory);
            candidates.push(LsRCandidate {
                path: directory.join(*basename),
                directory,
            });
        });
        Some(candidates)
    }
}

fn load_aliases(root: &Path) -> Result<AliasIndex, LsRError> {
    load_aliases_with_limit(root, DEFAULT_MAX_ALIASES_BYTES)
}

fn load_aliases_with_limit(root: &Path, max_bytes: u64) -> Result<AliasIndex, LsRError> {
    let path = root.join("aliases");
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(AliasIndex {
                snapshot: None,
                names: HashSet::new(),
            });
        }
        Err(source) => return Err(LsRError::Open { path, source }),
    };
    let before = file.metadata().map_err(|source| LsRError::Inspect {
        path: path.clone(),
        source,
    })?;
    if !before.is_file() {
        return Err(LsRError::MalformedAliases("aliases is not a regular file"));
    }
    if before.len() > max_bytes {
        return Err(LsRError::AliasesTooLarge { limit: max_bytes });
    }

    let capacity = before.len().min(max_bytes).min(usize::MAX as u64) as usize;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| LsRError::Read {
            path: path.clone(),
            source,
        })?;
    if bytes.len() as u64 > max_bytes {
        return Err(LsRError::AliasesTooLarge { limit: max_bytes });
    }

    let after = fs::metadata(&path).map_err(|source| LsRError::Inspect {
        path: path.clone(),
        source,
    })?;
    let before = DatabaseSnapshot::from_metadata(&before);
    let after = DatabaseSnapshot::from_metadata(&after);
    if before != after {
        return Err(LsRError::AliasesChangedDuringRead);
    }

    Ok(AliasIndex {
        snapshot: Some(after),
        names: parse_aliases(&bytes)?,
    })
}

fn parse_aliases(bytes: &[u8]) -> Result<HashSet<OsString>, LsRError> {
    let mut names = HashSet::new();
    let mut entry_count = 0usize;
    for raw_line in bytes.split(|&byte| byte == b'\n') {
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.len() > MAX_LINE_BYTES {
            return Err(LsRError::AliasLineTooLong {
                limit: MAX_LINE_BYTES,
            });
        }
        if line.contains(&0) {
            return Err(LsRError::MalformedAliases("NUL byte in aliases"));
        }
        if line.iter().all(|byte| byte.is_ascii_whitespace())
            || matches!(line.first(), Some(b'%' | b'#'))
        {
            continue;
        }
        let mut words = line
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|word| !word.is_empty());
        let real = words
            .next()
            .ok_or(LsRError::MalformedAliases("aliases entry has no real name"))?;
        let alias = words.next().ok_or(LsRError::MalformedAliases(
            "aliases entry has no alias name",
        ))?;
        if words.next().is_some() {
            return Err(LsRError::MalformedAliases(
                "aliases entry has more than two words",
            ));
        }
        let real = os_string_from_aliases(real.to_vec())?;
        let alias = os_string_from_aliases(alias.to_vec())?;
        if !is_plain_basename(&real) || !is_plain_basename(&alias) {
            return Err(LsRError::MalformedAliases(
                "aliases names are not basenames",
            ));
        }
        entry_count = entry_count
            .checked_add(1)
            .ok_or(LsRError::TooManyAliases { limit: MAX_ALIASES })?;
        if entry_count > MAX_ALIASES {
            return Err(LsRError::TooManyAliases { limit: MAX_ALIASES });
        }
        names.insert(alias);
    }
    Ok(names)
}

fn aliases_are_unchanged(root: &Path, snapshot: Option<&DatabaseSnapshot>) -> bool {
    match (snapshot, fs::metadata(root.join("aliases"))) {
        (None, Err(error)) if error.kind() == io::ErrorKind::NotFound => true,
        (Some(expected), Ok(metadata)) if metadata.is_file() => {
            DatabaseSnapshot::from_metadata(&metadata) == *expected
        }
        _ => false,
    }
}

fn parse_database(
    root: &Path,
    bytes: &[u8],
) -> Result<(Vec<PathBuf>, HashMap<OsString, CandidateDirectories>), LsRError> {
    let mut directories = Vec::new();
    let mut directory_ids = HashMap::<PathBuf, usize>::new();
    let mut by_basename = HashMap::<OsString, CandidateDirectories>::new();
    let mut current_directory = None;
    let mut entry_count = 0usize;

    for raw_line in bytes.split(|&byte| byte == b'\n') {
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.len() > MAX_LINE_BYTES {
            return Err(LsRError::LineTooLong {
                limit: MAX_LINE_BYTES,
            });
        }
        if line.contains(&0) {
            return Err(LsRError::Malformed("NUL byte in ls-R"));
        }
        if line.is_empty() {
            continue;
        }

        match parse_header(root, line)? {
            Header::Directory(relative_directory) => {
                let next_id = directories.len();
                let directory_id = *directory_ids
                    .entry(relative_directory.clone())
                    .or_insert_with(|| {
                        directories.push(relative_directory);
                        next_id
                    });
                current_directory = Some(directory_id);
            }
            Header::RejectedDirectory => current_directory = None,
            Header::NotHeader => {
                let Some(directory_id) = current_directory else {
                    // 公開仕様どおり、最初の directory header より前は無視する。
                    continue;
                };
                let basename = os_string_from_database(line.to_vec())?;
                if !is_plain_basename(&basename) {
                    // separator 入り entry の意味は公開仕様で確定しないため利用しない。
                    continue;
                }
                entry_count = entry_count
                    .checked_add(1)
                    .ok_or(LsRError::TooManyEntries { limit: MAX_ENTRIES })?;
                if entry_count > MAX_ENTRIES {
                    return Err(LsRError::TooManyEntries { limit: MAX_ENTRIES });
                }
                by_basename
                    .entry(basename)
                    .and_modify(|ids| ids.insert(directory_id))
                    .or_insert(CandidateDirectories::One(directory_id));
            }
        }
    }

    Ok((directories, by_basename))
}

enum Header {
    Directory(PathBuf),
    /// hidden directory など、以後の entry を意図的に索引しない header。
    RejectedDirectory,
    NotHeader,
}

fn parse_header(root: &Path, line: &[u8]) -> Result<Header, LsRError> {
    let Some(raw_path) = line.strip_suffix(b":") else {
        return Ok(Header::NotHeader);
    };
    let path = PathBuf::from(os_string_from_database(raw_path.to_vec())?);
    let begins_like_documented_header = raw_path.starts_with(b"/")
        || raw_path.starts_with(b"./")
        || raw_path.starts_with(b"../")
        || path.is_absolute();
    if !begins_like_documented_header {
        return Ok(Header::NotHeader);
    }
    if raw_path.starts_with(b"../") {
        // DB root 外を指す header は fast path 全体を諦める。
        return Err(LsRError::Malformed("parent directory header in ls-R"));
    }

    let relative = if raw_path.starts_with(b"./") {
        PathBuf::from(os_string_from_database(raw_path[2..].to_vec())?)
    } else {
        path.strip_prefix(root)
            .map_err(|_| LsRError::Malformed("absolute ls-R header is outside its root"))?
            .to_path_buf()
    };
    if !is_safe_relative_directory(&relative) {
        return Err(LsRError::Malformed("invalid directory header in ls-R"));
    }
    if contains_hidden_component(&relative) {
        return Ok(Header::RejectedDirectory);
    }
    Ok(Header::Directory(relative))
}

fn is_safe_relative_directory(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn contains_hidden_component(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => name.to_string_lossy().starts_with('.'),
        _ => false,
    })
}

fn is_plain_basename(name: &OsStr) -> bool {
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(Component::Normal(component)) if component == name)
        && components.next().is_none()
}

fn ends_with_components(path: &Path, suffix: &[&OsStr]) -> bool {
    let mut components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(component) => Some(component),
            _ => None,
        })
        .rev();
    suffix
        .iter()
        .rev()
        .all(|expected| components.next() == Some(*expected))
}

#[cfg(unix)]
fn os_string_from_database(bytes: Vec<u8>) -> Result<OsString, LsRError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn os_string_from_database(bytes: Vec<u8>) -> Result<OsString, LsRError> {
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(|_| LsRError::Malformed("ls-R is not valid UTF-8"))
}

#[cfg(unix)]
fn os_string_from_aliases(bytes: Vec<u8>) -> Result<OsString, LsRError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn os_string_from_aliases(bytes: Vec<u8>) -> Result<OsString, LsRError> {
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(|_| LsRError::MalformedAliases("aliases is not valid UTF-8"))
}

#[derive(Debug)]
pub(super) enum LsRError {
    Open { path: PathBuf, source: io::Error },
    Inspect { path: PathBuf, source: io::Error },
    Read { path: PathBuf, source: io::Error },
    TooLarge { limit: u64 },
    LineTooLong { limit: usize },
    TooManyEntries { limit: usize },
    AliasesTooLarge { limit: u64 },
    AliasLineTooLong { limit: usize },
    TooManyAliases { limit: usize },
    ChangedDuringRead,
    AliasesChangedDuringRead,
    Malformed(&'static str),
    MalformedAliases(&'static str),
}

impl fmt::Display for LsRError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, source } => {
                write!(formatter, "cannot open `{}`: {source}", path.display())
            }
            Self::Inspect { path, source } => {
                write!(formatter, "cannot inspect `{}`: {source}", path.display())
            }
            Self::Read { path, source } => {
                write!(formatter, "cannot read `{}`: {source}", path.display())
            }
            Self::TooLarge { limit } => write!(formatter, "ls-R exceeds {limit} bytes"),
            Self::LineTooLong { limit } => write!(formatter, "ls-R line exceeds {limit} bytes"),
            Self::TooManyEntries { limit } => {
                write!(formatter, "ls-R contains more than {limit} entries")
            }
            Self::AliasesTooLarge { limit } => write!(formatter, "aliases exceeds {limit} bytes"),
            Self::AliasLineTooLong { limit } => {
                write!(formatter, "aliases line exceeds {limit} bytes")
            }
            Self::TooManyAliases { limit } => {
                write!(formatter, "aliases contains more than {limit} entries")
            }
            Self::ChangedDuringRead => write!(formatter, "ls-R changed while it was read"),
            Self::AliasesChangedDuringRead => {
                write!(formatter, "aliases changed while it was read")
            }
            Self::Malformed(reason) => write!(formatter, "malformed ls-R: {reason}"),
            Self::MalformedAliases(reason) => write!(formatter, "malformed aliases: {reason}"),
        }
    }
}

impl std::error::Error for LsRError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open { source, .. }
            | Self::Inspect { source, .. }
            | Self::Read { source, .. } => Some(source),
            Self::TooLarge { .. }
            | Self::LineTooLong { .. }
            | Self::TooManyEntries { .. }
            | Self::AliasesTooLarge { .. }
            | Self::AliasLineTooLong { .. }
            | Self::TooManyAliases { .. }
            | Self::ChangedDuringRead
            | Self::AliasesChangedDuringRead
            | Self::Malformed(_)
            | Self::MalformedAliases(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{load_aliases_with_limit, AliasMatch, LsRDatabase, LsRError};
    #[cfg(unix)]
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestTree {
        root: PathBuf,
    }

    impl TestTree {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "pratex-lsr-{label}-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn write_database(&self, bytes: &[u8]) -> PathBuf {
            let path = self.root.join("ls-R");
            fs::write(&path, bytes).unwrap();
            path
        }

        fn write_aliases(&self, bytes: &[u8]) -> PathBuf {
            let path = self.root.join("aliases");
            fs::write(&path, bytes).unwrap();
            path
        }
    }

    impl Drop for TestTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn 見出し前を無視してcrlfと空白入り名と最終改行なしを読む() {
        let tree = TestTree::new("crlf");
        let database = tree.write_database(
            b"orphan.tex\r\n\r\n./tex/latex/a package:\r\nspace file.tex\r\n./:\r\nroot.tex",
        );
        let index = LsRDatabase::load(database).unwrap();

        assert_eq!(index.candidates("orphan.tex".as_ref()).unwrap().len(), 0);
        let spaced = index.candidates("space file.tex".as_ref()).unwrap();
        assert_eq!(spaced.len(), 1);
        assert_eq!(
            spaced[0].path(),
            tree.root.join("tex/latex/a package/space file.tex")
        );
        let root = index.candidates("root.tex".as_ref()).unwrap();
        assert_eq!(root[0].path(), tree.root.join("root.tex"));
    }

    #[test]
    fn aliasesは実名とaliasの二語を読みcommentと空行を無視する() {
        let tree = TestTree::new("aliases");
        let database = tree.write_database(b"./tex:\nreal.tex\nother.tex\n");
        tree.write_aliases(
            b"% generated aliases\r\n# second comment\n\n \t\r\nreal.tex alias.tex\r\nother.tex\tsecond.tex\n",
        );
        let index = LsRDatabase::load(database).unwrap();

        assert_eq!(index.alias_match("alias.tex".as_ref()), AliasMatch::Yes);
        assert_eq!(index.alias_match("second.tex".as_ref()), AliasMatch::Yes);
        assert_eq!(index.alias_match("real.tex".as_ref()), AliasMatch::No);
        assert_eq!(
            index.alias_match("subdir/alias.tex".as_ref()),
            AliasMatch::UnsupportedName
        );
    }

    #[test]
    fn 壊れたaliasesを部分的に使わない() {
        for aliases in [
            b"one-word\n".as_slice(),
            b"real.tex alias.tex extra\n".as_slice(),
            b"dir/real.tex alias.tex\n".as_slice(),
            b"real.tex dir/alias.tex\n".as_slice(),
            b"real.tex ali\0as.tex\n".as_slice(),
        ] {
            let tree = TestTree::new("malformed-aliases");
            let database = tree.write_database(b"./tex:\nreal.tex\n");
            tree.write_aliases(aliases);
            assert!(matches!(
                LsRDatabase::load(database),
                Err(LsRError::MalformedAliases(_))
            ));
        }
    }

    #[test]
    fn aliasesの指定上限を越えて読まない() {
        let tree = TestTree::new("bounded-aliases");
        tree.write_aliases(b"real.tex alias.tex\n");
        assert!(matches!(
            load_aliases_with_limit(&tree.root, 4),
            Err(LsRError::AliasesTooLarge { limit: 4 })
        ));
    }

    #[test]
    fn aliasesの作成と変更をsnapshotで検出する() {
        let absent = TestTree::new("aliases-created");
        let absent_database = absent.write_database(b"./tex:\nreal.tex\n");
        let absent_index = LsRDatabase::load(absent_database).unwrap();
        assert!(absent_index.is_unchanged());
        absent.write_aliases(b"real.tex alias.tex\n");
        assert!(!absent_index.is_unchanged());

        let changed = TestTree::new("aliases-changed");
        let changed_database = changed.write_database(b"./tex:\nreal.tex\n");
        let aliases = changed.write_aliases(b"real.tex old-name.tex\n");
        let changed_index = LsRDatabase::load(changed_database).unwrap();
        assert!(changed_index.is_unchanged());
        fs::write(aliases, b"real.tex new-long-name.tex\n").unwrap();
        assert!(!changed_index.is_unchanged());
    }

    #[test]
    fn 同名候補を上書きせずdirectoryを共有する() {
        let tree = TestTree::new("duplicate");
        let database =
            tree.write_database(b"./tex/a:\nsame.tex\nsame.tex\n./tex/b:\nsame.tex\nother.tex\n");
        let index = LsRDatabase::load(database).unwrap();

        let candidates = index.candidates("same.tex".as_ref()).unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].directory(), tree.root.join("tex/a"));
        assert_eq!(candidates[1].directory(), tree.root.join("tex/b"));
    }

    #[test]
    fn 論理名のdirectory部分で同名候補を絞る() {
        let tree = TestTree::new("suffix");
        let database = tree.write_database(b"./tex/latex/a:\nsame.tex\n./tex/plain/a:\nsame.tex\n");
        let index = LsRDatabase::load(database).unwrap();

        let candidates = index.candidates("latex/a/same.tex".as_ref()).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].directory(), tree.root.join("tex/latex/a"));
        assert!(index.candidates("./same.tex".as_ref()).is_none());
        assert!(index.candidates("../same.tex".as_ref()).is_none());
    }

    #[test]
    fn 隠しdirectoryを索引しない() {
        let tree = TestTree::new("hidden");
        let database = tree.write_database(b"./.cache:\nhidden.tex\n./visible:\nvisible.tex\n");
        let index = LsRDatabase::load(database).unwrap();

        assert!(index.candidates("hidden.tex".as_ref()).unwrap().is_empty());
        assert_eq!(index.candidates("visible.tex".as_ref()).unwrap().len(), 1);
    }

    #[test]
    fn 記号をcomment扱いせず見出しに似たfile行もdirectoryにしない() {
        let tree = TestTree::new("ordinary-lines");
        let database = tree.write_database(b"./:\n#hash.tex\n%percent.tex\ntex/base:\nafter.tex\n");
        let index = LsRDatabase::load(database).unwrap();

        assert_eq!(index.candidates("#hash.tex".as_ref()).unwrap().len(), 1);
        assert_eq!(index.candidates("%percent.tex".as_ref()).unwrap().len(), 1);
        assert_eq!(index.candidates("after.tex".as_ref()).unwrap().len(), 1);
        assert!(index.candidates("tex/base:".as_ref()).unwrap().is_empty());
    }

    #[test]
    fn root内の絶対見出しを受け入れる() {
        let tree = TestTree::new("absolute");
        let directory = tree.root.join("tex/base");
        let contents = format!("{}:\nplain.tex\n", directory.display());
        let database = tree.write_database(contents.as_bytes());
        let index = LsRDatabase::load(database).unwrap();

        let candidates = index.candidates("plain.tex".as_ref()).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].path(), directory.join("plain.tex"));
    }

    #[test]
    fn 親directory見出しとnulで部分索引を作らない() {
        for contents in [
            b"./ok:\nok.tex\n../outside:\nbad.tex\n".as_slice(),
            b"./ok:\nok.tex\nna\0me.tex\n".as_slice(),
        ] {
            let tree = TestTree::new("malformed");
            let database = tree.write_database(contents);
            assert!(matches!(
                LsRDatabase::load(database),
                Err(LsRError::Malformed(_))
            ));
        }
    }

    #[test]
    fn 指定上限を越えるdatabaseを読まない() {
        let tree = TestTree::new("bounded");
        let database = tree.write_database(b"./:\nfile.tex\n");
        assert!(matches!(
            LsRDatabase::load_with_limit(database, 4),
            Err(LsRError::TooLarge { limit: 4 })
        ));
    }

    #[test]
    fn databaseのsnapshot変更を検出する() {
        let tree = TestTree::new("snapshot");
        let database = tree.write_database(b"./:\nold.tex\n");
        let index = LsRDatabase::load(&database).unwrap();
        assert!(index.is_unchanged());

        fs::write(&database, b"./:\nnew-file.tex\n").unwrap();
        assert!(!index.is_unchanged());
        assert_eq!(index.database_path(), database);
        assert_eq!(index.root(), tree.root);
    }

    #[test]
    #[ignore = "PRATEX_REAL_LSR で明示した TeX Live database の手動照合"]
    fn 指定した実物databaseで既知のfileを索引できる() {
        let database = std::env::var_os("PRATEX_REAL_LSR")
            .map(PathBuf::from)
            .expect("PRATEX_REAL_LSR が必要");
        let index = LsRDatabase::load(database).unwrap();
        let candidates = index.candidates("latex.ltx".as_ref()).unwrap();

        assert!(!candidates.is_empty());
        assert!(candidates
            .iter()
            .all(|candidate| fs::metadata(candidate.path()).unwrap().is_file()));
    }

    #[cfg(unix)]
    #[test]
    fn unixでは非utf8名をbyte列のまま索引する() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let tree = TestTree::new("non-utf8");
        let database = tree.write_database(b"./:\nna\xffme.tex\n");
        let index = LsRDatabase::load(database).unwrap();
        let logical = OsString::from_vec(b"na\xffme.tex".to_vec());
        let candidates = index.candidates(&logical).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].path().file_name().unwrap().as_bytes(),
            b"na\xffme.tex"
        );
    }

    #[cfg(not(unix))]
    #[test]
    fn windowsでは不正utf8を置換せずdatabaseを無効にする() {
        let tree = TestTree::new("non-utf8");
        let database = tree.write_database(b"./:\nna\xffme.tex\n");
        assert!(matches!(
            LsRDatabase::load(database),
            Err(LsRError::Malformed("ls-R is not valid UTF-8"))
        ));
    }
}

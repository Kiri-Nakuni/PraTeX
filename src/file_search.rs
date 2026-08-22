//! TeX の論理ファイル名を、実際に開く物理パスへ解決する。
//!
//! 外部探索は `kpsewhich` の公開 CLI だけを利用する。シェルや kpathsea の C API は
//! 介さないため、この層は safe Rust のままであり、問い合わせに見せた名前も
//! `OsString` のまま保たれる。

mod lsr;

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::hash::Hash;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// kpathsea の探索種別と一対一に対応させる、rtex 側の用途。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum FileKind {
    Tex,
    Format,
    Tfm,
    FontMap,
    Encoding,
    Type1,
    Afm,
    Vaak,
    PdfData,
}

impl FileKind {
    fn kpsewhich_format(self) -> &'static str {
        match self {
            Self::Tex => "tex",
            Self::Format => "fmt",
            Self::Tfm => "tfm",
            Self::FontMap => "map",
            Self::Encoding => "enc files",
            Self::Type1 => "type1 fonts",
            Self::Afm => "afm",
            Self::Vaak | Self::PdfData => "other text files",
        }
    }
}

/// TeX 入力や map が指す論理名。
///
/// 解決後もこの値を捨てず、DVI/PDF のフォント識別名などへ物理パスが漏れないように
/// `PathBuf` とは別の型にする。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct LogicalFileName(OsString);

impl LogicalFileName {
    pub(crate) fn new(name: impl Into<OsString>) -> Self {
        Self(name.into())
    }

    pub(crate) fn as_os_str(&self) -> &OsStr {
        &self.0
    }
}

impl From<OsString> for LogicalFileName {
    fn from(name: OsString) -> Self {
        Self(name)
    }
}

impl From<&OsStr> for LogicalFileName {
    fn from(name: &OsStr) -> Self {
        Self(name.to_os_string())
    }
}

impl AsRef<OsStr> for LogicalFileName {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolutionSource {
    DirectPath,
    Kpsewhich,
}

/// 開くパスと、照会に使った論理名を分離して保持する。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedFile {
    logical_name: LogicalFileName,
    physical_path: PathBuf,
    source: ResolutionSource,
}

impl ResolvedFile {
    pub(crate) fn logical_name(&self) -> &LogicalFileName {
        &self.logical_name
    }

    pub(crate) fn physical_path(&self) -> &Path {
        &self.physical_path
    }

    pub(crate) fn source(&self) -> ResolutionSource {
        self.source
    }

    pub(crate) fn into_physical_path(self) -> PathBuf {
        self.physical_path
    }
}

/// rtex の fmt は既存エンジンのバイナリ fmt と互換ではない。
///
/// そのため既定値は必ずローカル限定とし、外部 fmt を探す場合だけ rtex 用 engine 名を
/// 明示した問い合わせに切り替える。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ExternalFormatSearch {
    #[default]
    LocalOnly,
    KpsewhichRtexEngine,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolverOptions {
    kpsewhich_program: OsString,
    program_name: OsString,
    external_format_search: ExternalFormatSearch,
}

impl Default for ResolverOptions {
    fn default() -> Self {
        Self {
            kpsewhich_program: OsString::from("kpsewhich"),
            program_name: OsString::from("euptex"),
            external_format_search: ExternalFormatSearch::LocalOnly,
        }
    }
}

impl ResolverOptions {
    pub(crate) fn with_kpsewhich_program(mut self, program: impl Into<OsString>) -> Self {
        self.kpsewhich_program = program.into();
        self
    }

    pub(crate) fn with_program_name(mut self, program_name: impl Into<OsString>) -> Self {
        self.program_name = program_name.into();
        self
    }

    pub(crate) fn with_external_format_search(mut self, policy: ExternalFormatSearch) -> Self {
        self.external_format_search = policy;
        self
    }
}

/// ファイル探索を利用する側が依存する最小の境界。
pub(crate) trait FileResolver {
    fn resolve(
        &mut self,
        kind: FileKind,
        logical_name: &LogicalFileName,
    ) -> Result<Option<ResolvedFile>, ResolveError>;
}

/// `Command::output` から探索層が必要とする情報だけを切り出した値。
///
/// `ExitStatus` を直接使わないので、合成試験は OS 固有 API に依存しない。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandOutput {
    pub(crate) code: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

/// 外部プロセスの起動を注入する境界。
pub(crate) trait CommandExecutor {
    fn execute(&mut self, program: &OsStr, arguments: &[OsString]) -> io::Result<CommandOutput>;
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ProcessCommandExecutor;

impl CommandExecutor for ProcessCommandExecutor {
    fn execute(&mut self, program: &OsStr, arguments: &[OsString]) -> io::Result<CommandOutput> {
        // `Command` へ直接渡し、引用や metacharacter の解釈をシェルへ委ねない。
        let output = Command::new(program).args(arguments).output()?;
        Ok(CommandOutput {
            code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Query {
    kind: FileKind,
    logical_name: LogicalFileName,
}

/// 直接パスと `kpsewhich` を順に試す resolver。
///
/// Windows でも既定の実行ファイルは `kpsewhich` だけであり、WSL への暗黙の fallback は
/// 行わない。必要なら呼び出し側が `ResolverOptions` へ実行ファイルを明示する。
pub(crate) struct KpsewhichResolver<E = ProcessCommandExecutor> {
    options: ResolverOptions,
    executor: E,
    external_cache: HashMap<Query, Option<PathBuf>>,
}

impl Default for KpsewhichResolver<ProcessCommandExecutor> {
    fn default() -> Self {
        Self::new(ResolverOptions::default(), ProcessCommandExecutor)
    }
}

impl<E> KpsewhichResolver<E> {
    pub(crate) fn new(options: ResolverOptions, executor: E) -> Self {
        Self {
            options,
            executor,
            external_cache: HashMap::new(),
        }
    }

    /// TeX の探索環境が変わった場合に、成功と不在の両方を再照会できるようにする。
    pub(crate) fn clear_external_cache(&mut self) {
        self.external_cache.clear();
    }
}

impl<E: CommandExecutor> FileResolver for KpsewhichResolver<E> {
    fn resolve(
        &mut self,
        kind: FileKind,
        logical_name: &LogicalFileName,
    ) -> Result<Option<ResolvedFile>, ResolveError> {
        let direct_path = PathBuf::from(logical_name.as_os_str());
        match fs::metadata(&direct_path) {
            Ok(metadata) if metadata.is_file() => {
                return Ok(Some(ResolvedFile {
                    logical_name: logical_name.clone(),
                    physical_path: direct_path,
                    source: ResolutionSource::DirectPath,
                }));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ResolveError::InspectDirectPath {
                    path: direct_path,
                    source,
                });
            }
        }

        if kind == FileKind::Format
            && self.options.external_format_search == ExternalFormatSearch::LocalOnly
        {
            return Ok(None);
        }

        let query = Query {
            kind,
            logical_name: logical_name.clone(),
        };
        if let Some(cached_path) = self.external_cache.get(&query) {
            return Ok(cached_path.clone().map(|physical_path| ResolvedFile {
                logical_name: logical_name.clone(),
                physical_path,
                source: ResolutionSource::Kpsewhich,
            }));
        }

        let arguments = self.kpsewhich_arguments(&query);
        let output = self
            .executor
            .execute(&self.options.kpsewhich_program, &arguments)
            .map_err(|source| ResolveError::LaunchKpsewhich {
                program: self.options.kpsewhich_program.clone(),
                source,
            })?;

        let physical_path = classify_kpsewhich_output(output)?;
        self.external_cache.insert(query, physical_path.clone());
        Ok(physical_path.map(|physical_path| ResolvedFile {
            logical_name: logical_name.clone(),
            physical_path,
            source: ResolutionSource::Kpsewhich,
        }))
    }
}

impl<E> KpsewhichResolver<E> {
    fn kpsewhich_arguments(&self, query: &Query) -> Vec<OsString> {
        let mut arguments = vec![
            OsString::from("--must-exist"),
            option_with_value("--progname=", &self.options.program_name),
            OsString::from(format!("--format={}", query.kind.kpsewhich_format())),
        ];
        if query.kind == FileKind::Format
            && self.options.external_format_search == ExternalFormatSearch::KpsewhichRtexEngine
        {
            arguments.push(OsString::from("--engine=rtex"));
        }
        // 論理名が `--version` などでも option として解釈させない。
        arguments.push(OsString::from("--"));
        arguments.push(query.logical_name.as_os_str().to_os_string());
        arguments
    }
}

fn option_with_value(prefix: &str, value: &OsStr) -> OsString {
    let mut option = OsString::from(prefix);
    option.push(value);
    option
}

fn classify_kpsewhich_output(output: CommandOutput) -> Result<Option<PathBuf>, ResolveError> {
    if output.code == Some(0) {
        return parse_output_path(output.stdout);
    }

    // 公開 CLI の「該当なし」は status 1・出力なしである。診断が付いた status 1 や
    // signal 終了を不在へ潰すと、設定不良を永久に negative-cache してしまう。
    if output.code == Some(1)
        && trim_line_endings(&output.stdout).is_empty()
        && trim_ascii_whitespace(&output.stderr).is_empty()
    {
        return Ok(None);
    }

    Err(ResolveError::KpsewhichFailed {
        code: output.code,
        stderr: output.stderr,
    })
}

fn parse_output_path(mut bytes: Vec<u8>) -> Result<Option<PathBuf>, ResolveError> {
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
    if bytes.is_empty() {
        return Ok(None);
    }
    if bytes.contains(&0) {
        return Err(ResolveError::MalformedKpsewhichOutput(
            "NUL byte in pathname",
        ));
    }
    if bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        return Err(ResolveError::MalformedKpsewhichOutput(
            "more than one output line",
        ));
    }

    Ok(Some(PathBuf::from(os_string_from_output(bytes)?)))
}

fn trim_line_endings(mut bytes: &[u8]) -> &[u8] {
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn trim_ascii_whitespace(mut bytes: &[u8]) -> &[u8] {
    while matches!(bytes.first(), Some(byte) if byte.is_ascii_whitespace()) {
        bytes = &bytes[1..];
    }
    while matches!(bytes.last(), Some(byte) if byte.is_ascii_whitespace()) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

#[cfg(unix)]
fn os_string_from_output(bytes: Vec<u8>) -> Result<OsString, ResolveError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn os_string_from_output(bytes: Vec<u8>) -> Result<OsString, ResolveError> {
    // Windows のファイル名はUnicodeだが、プロセス出力はbyte列である。不正UTF-8を
    // 置換すると存在しない別pathへ変わるため、境界の失敗として明示する。
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(|_| ResolveError::MalformedKpsewhichOutput("pathname is not valid UTF-8"))
}

#[derive(Debug)]
pub(crate) enum ResolveError {
    InspectDirectPath {
        path: PathBuf,
        source: io::Error,
    },
    LaunchKpsewhich {
        program: OsString,
        source: io::Error,
    },
    KpsewhichFailed {
        code: Option<i32>,
        stderr: Vec<u8>,
    },
    MalformedKpsewhichOutput(&'static str),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InspectDirectPath { path, source } => {
                write!(formatter, "cannot inspect `{}`: {source}", path.display())
            }
            Self::LaunchKpsewhich { program, source } => write!(
                formatter,
                "cannot launch `{}`: {source}",
                program.to_string_lossy()
            ),
            Self::KpsewhichFailed { code, stderr } => write!(
                formatter,
                "kpsewhich failed with status {code:?}: {}",
                String::from_utf8_lossy(stderr)
            ),
            Self::MalformedKpsewhichOutput(reason) => {
                write!(formatter, "kpsewhich returned malformed output: {reason}")
            }
        }
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InspectDirectPath { source, .. } | Self::LaunchKpsewhich { source, .. } => {
                Some(source)
            }
            Self::KpsewhichFailed { .. } | Self::MalformedKpsewhichOutput(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandExecutor, CommandOutput, ExternalFormatSearch, FileKind, FileResolver,
        KpsewhichResolver, LogicalFileName, ResolutionSource, ResolveError, ResolverOptions,
    };
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Invocation {
        program: OsString,
        arguments: Vec<OsString>,
    }

    #[derive(Clone, Default)]
    struct FakeExecutor {
        invocations: Rc<RefCell<Vec<Invocation>>>,
        responses: Rc<RefCell<VecDeque<io::Result<CommandOutput>>>>,
    }

    impl FakeExecutor {
        fn with_responses(responses: impl IntoIterator<Item = io::Result<CommandOutput>>) -> Self {
            Self {
                invocations: Rc::new(RefCell::new(Vec::new())),
                responses: Rc::new(RefCell::new(responses.into_iter().collect())),
            }
        }

        fn invocation_count(&self) -> usize {
            self.invocations.borrow().len()
        }
    }

    impl CommandExecutor for FakeExecutor {
        fn execute(
            &mut self,
            program: &OsStr,
            arguments: &[OsString],
        ) -> io::Result<CommandOutput> {
            self.invocations.borrow_mut().push(Invocation {
                program: program.to_os_string(),
                arguments: arguments.to_vec(),
            });
            self.responses
                .borrow_mut()
                .pop_front()
                .expect("合成応答が足りない")
        }
    }

    fn success(path: &[u8]) -> io::Result<CommandOutput> {
        let mut stdout = path.to_vec();
        stdout.extend_from_slice(b"\r\n");
        Ok(CommandOutput {
            code: Some(0),
            stdout,
            stderr: Vec::new(),
        })
    }

    fn missing() -> io::Result<CommandOutput> {
        Ok(CommandOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn unique_absent_name(suffix: &str) -> LogicalFileName {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        LogicalFileName::new(format!(
            "rtex-resolver-{}-{}-{suffix}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn 存在する直接pathを外部探索より優先する() {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "rtex-resolver-direct-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("直接.tex");
        fs::write(&path, b"test").unwrap();

        let fake = FakeExecutor::with_responses([success(b"external.tex")]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let logical = LogicalFileName::new(path.as_os_str());
        let resolved = resolver.resolve(FileKind::Tex, &logical).unwrap().unwrap();

        assert_eq!(resolved.logical_name(), &logical);
        assert_eq!(resolved.physical_path(), path);
        assert_eq!(resolved.source(), ResolutionSource::DirectPath);
        assert_eq!(fake.invocation_count(), 0);

        fs::remove_file(path).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn 成功と不在を問い合わせごとにcacheする() {
        let fake = FakeExecutor::with_responses([success(b"/tree/a.tex"), missing()]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let found = unique_absent_name("found.tex");
        let absent = unique_absent_name("absent.tex");

        for _ in 0..2 {
            let resolved = resolver.resolve(FileKind::Tex, &found).unwrap().unwrap();
            assert_eq!(resolved.physical_path(), PathBuf::from("/tree/a.tex"));
            assert_eq!(resolved.source(), ResolutionSource::Kpsewhich);
        }
        for _ in 0..2 {
            assert!(resolver.resolve(FileKind::Tex, &absent).unwrap().is_none());
        }

        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn 不在をcacheした後でも新しい直接pathを優先する() {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rtex-resolver-late-direct-{}-{}.tex",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        let fake = FakeExecutor::with_responses([missing()]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let logical = LogicalFileName::new(path.as_os_str());

        assert!(resolver.resolve(FileKind::Tex, &logical).unwrap().is_none());
        fs::write(&path, b"later").unwrap();
        let resolved = resolver.resolve(FileKind::Tex, &logical).unwrap().unwrap();
        assert_eq!(resolved.source(), ResolutionSource::DirectPath);
        assert_eq!(resolved.physical_path(), path);
        assert_eq!(fake.invocation_count(), 1);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn 同じ綴りでも用途ごとに別の問い合わせになる() {
        let fake = FakeExecutor::with_responses([success(b"/tree/a.tex"), success(b"/tree/a.afm")]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let name = unique_absent_name("a");

        resolver.resolve(FileKind::Tex, &name).unwrap();
        resolver.resolve(FileKind::Afm, &name).unwrap();

        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn 起動失敗を不在にせず次回は再試行する() {
        let fake = FakeExecutor::with_responses([
            Err(io::Error::new(io::ErrorKind::NotFound, "kpsewhichなし")),
            success(b"/tree/recovered.tex"),
        ]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let name = unique_absent_name("retry.tex");

        assert!(matches!(
            resolver.resolve(FileKind::Tex, &name),
            Err(ResolveError::LaunchKpsewhich { .. })
        ));
        assert!(resolver.resolve(FileKind::Tex, &name).unwrap().is_some());
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn 既定の起動先からwslへ黙ってfallbackしない() {
        let fake = FakeExecutor::with_responses([Err(io::Error::new(
            io::ErrorKind::NotFound,
            "kpsewhichなし",
        ))]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let name = unique_absent_name("no-fallback.tex");

        assert!(matches!(
            resolver.resolve(FileKind::Tex, &name),
            Err(ResolveError::LaunchKpsewhich { .. })
        ));
        let invocations = fake.invocations.borrow();
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].program, OsString::from("kpsewhich"));
    }

    #[test]
    fn 異常終了を不在にせず次回は再試行する() {
        let fake = FakeExecutor::with_responses([
            Ok(CommandOutput {
                code: Some(2),
                stdout: Vec::new(),
                stderr: b"bad configuration".to_vec(),
            }),
            success(b"/tree/recovered.tfm"),
        ]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let name = unique_absent_name("retry.tfm");

        assert!(matches!(
            resolver.resolve(FileKind::Tfm, &name),
            Err(ResolveError::KpsewhichFailed { code: Some(2), .. })
        ));
        assert!(resolver.resolve(FileKind::Tfm, &name).unwrap().is_some());
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn status一でも診断があれば失敗として扱う() {
        let fake = FakeExecutor::with_responses([Ok(CommandOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: b"cannot read texmf.cnf".to_vec(),
        })]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake);
        let name = unique_absent_name("broken.tex");

        assert!(matches!(
            resolver.resolve(FileKind::Tex, &name),
            Err(ResolveError::KpsewhichFailed { code: Some(1), .. })
        ));
    }

    #[test]
    fn 既定ではfmtを外へ探さない() {
        let fake = FakeExecutor::with_responses([success(b"/tree/plain.fmt")]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let name = unique_absent_name("plain.fmt");

        assert!(resolver.resolve(FileKind::Format, &name).unwrap().is_none());
        assert_eq!(fake.invocation_count(), 0);
    }

    #[test]
    fn fmtの外部探索はrtex用engineを明示する() {
        let fake = FakeExecutor::with_responses([success(b"/tree/plain.fmt")]);
        let options = ResolverOptions::default()
            .with_external_format_search(ExternalFormatSearch::KpsewhichRtexEngine);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());
        let name = unique_absent_name("plain.fmt");

        resolver.resolve(FileKind::Format, &name).unwrap();
        let invocations = fake.invocations.borrow();
        assert_eq!(invocations.len(), 1);
        assert!(invocations[0]
            .arguments
            .contains(&OsString::from("--engine=rtex")));
    }

    #[test]
    fn 全用途を公開cliのformatへ対応させる() {
        let kinds_and_formats = [
            (FileKind::Tex, "tex"),
            (FileKind::Format, "fmt"),
            (FileKind::Tfm, "tfm"),
            (FileKind::FontMap, "map"),
            (FileKind::Encoding, "enc files"),
            (FileKind::Type1, "type1 fonts"),
            (FileKind::Afm, "afm"),
            (FileKind::Vaak, "other text files"),
            (FileKind::PdfData, "other text files"),
        ];
        let responses = kinds_and_formats
            .iter()
            .map(|_| success(b"/tree/found"))
            .collect::<Vec<_>>();
        let fake = FakeExecutor::with_responses(responses);
        let options = ResolverOptions::default()
            .with_kpsewhich_program("custom-kpsewhich")
            .with_program_name("euptex")
            .with_external_format_search(ExternalFormatSearch::KpsewhichRtexEngine);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        for (index, (kind, _)) in kinds_and_formats.iter().enumerate() {
            resolver
                .resolve(*kind, &unique_absent_name(&format!("kind-{index}")))
                .unwrap();
        }

        let invocations = fake.invocations.borrow();
        assert_eq!(invocations.len(), kinds_and_formats.len());
        for (invocation, (_, format)) in invocations.iter().zip(kinds_and_formats) {
            assert_eq!(invocation.program, OsString::from("custom-kpsewhich"));
            assert!(invocation
                .arguments
                .contains(&OsString::from("--progname=euptex")));
            assert!(invocation
                .arguments
                .contains(&OsString::from(format!("--format={format}"))));
        }
    }

    #[test]
    fn optionに見える論理名も一つの引数として渡す() {
        let fake = FakeExecutor::with_responses([success(b"/tree/version.tex")]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let name = LogicalFileName::new("--version");

        resolver.resolve(FileKind::Tex, &name).unwrap();
        let invocations = fake.invocations.borrow();
        let tail = &invocations[0].arguments[invocations[0].arguments.len() - 2..];
        assert_eq!(tail, [OsString::from("--"), OsString::from("--version")]);
    }

    #[test]
    fn 論理名と返された物理pathを混ぜない() {
        let fake = FakeExecutor::with_responses([success(b"/texmf/fonts/cmr10.pfb")]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake);
        let logical = unique_absent_name("cmr10.pfb");

        let resolved = resolver
            .resolve(FileKind::Type1, &logical)
            .unwrap()
            .unwrap();
        assert_eq!(resolved.logical_name(), &logical);
        assert_eq!(
            resolved.into_physical_path(),
            PathBuf::from("/texmf/fonts/cmr10.pfb")
        );
    }

    #[test]
    fn 空の成功出力も不在としてcacheする() {
        let fake = FakeExecutor::with_responses([Ok(CommandOutput {
            code: Some(0),
            stdout: b"\r\n".to_vec(),
            stderr: Vec::new(),
        })]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let logical = unique_absent_name("empty.tex");

        assert!(resolver.resolve(FileKind::Tex, &logical).unwrap().is_none());
        assert!(resolver.resolve(FileKind::Tex, &logical).unwrap().is_none());
        assert_eq!(fake.invocation_count(), 1);
    }

    #[test]
    fn 複数行やnulを物理pathとして受け入れない() {
        for stdout in [b"/one\n/two\n".to_vec(), b"/one\0two\n".to_vec()] {
            let fake = FakeExecutor::with_responses([Ok(CommandOutput {
                code: Some(0),
                stdout,
                stderr: Vec::new(),
            })]);
            let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake);
            let logical = unique_absent_name("bad-output.tex");
            assert!(matches!(
                resolver.resolve(FileKind::Tex, &logical),
                Err(ResolveError::MalformedKpsewhichOutput(_))
            ));
        }
    }

    #[cfg(not(unix))]
    #[test]
    fn windowsで不正utf8の出力を別pathへ置換しない() {
        let malformed = || {
            Ok(CommandOutput {
                code: Some(0),
                stdout: vec![b'C', b':', b'\\', 0xff, b'.', b't', b'e', b'x', b'\n'],
                stderr: Vec::new(),
            })
        };
        let fake = FakeExecutor::with_responses([malformed(), malformed()]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let logical = unique_absent_name("invalid-output.tex");

        for _ in 0..2 {
            assert!(matches!(
                resolver.resolve(FileKind::Tex, &logical),
                Err(ResolveError::MalformedKpsewhichOutput(
                    "pathname is not valid UTF-8"
                ))
            ));
        }
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn cacheを消すと同じ問い合わせをやり直す() {
        let fake = FakeExecutor::with_responses([missing(), success(b"/tree/appeared.tex")]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let logical = unique_absent_name("appeared.tex");

        assert!(resolver.resolve(FileKind::Tex, &logical).unwrap().is_none());
        resolver.clear_external_cache();
        assert!(resolver.resolve(FileKind::Tex, &logical).unwrap().is_some());
        assert_eq!(fake.invocation_count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn 非utf8の論理名と物理pathをbyteのまま保つ() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let logical_bytes = vec![b'r', b't', b'e', b'x', b'-', 0xff, b'.', b'p', b'f', b'b'];
        let physical_bytes = vec![b'/', b't', b'm', b'p', b'/', 0xfe, b'.', b'p', b'f', b'b'];
        let fake = FakeExecutor::with_responses([success(&physical_bytes)]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let logical = LogicalFileName::new(OsString::from_vec(logical_bytes.clone()));

        let resolved = resolver
            .resolve(FileKind::Type1, &logical)
            .unwrap()
            .unwrap();
        assert_eq!(
            resolved.logical_name().as_os_str().as_bytes(),
            logical_bytes
        );
        assert_eq!(
            resolved.physical_path().as_os_str().as_bytes(),
            physical_bytes
        );

        let invocations = fake.invocations.borrow();
        assert_eq!(
            invocations[0].arguments.last().unwrap().as_bytes(),
            logical_bytes
        );
    }
}

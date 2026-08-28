//! TeX の論理ファイル名を、実際に開く物理パスへ解決する。
//!
//! Linuxでは監査済みRust wrapperを介したin-process Kpathseaを先に試し、
//! library不在またはpath encoding非対応の時だけ既存のsafe resolverへ戻る。
//! PraTeX自身のこの層はsafe Rustのままで、問い合わせに見せた名前も`OsString`のまま保つ。

mod in_process;
mod lsr;
mod search_path;
mod wsl;

use self::in_process::{FastPathFailure, NativeFirstResolver};
use self::lsr::{AliasMatch, LsRDatabase};
use self::search_path::{SearchPath, SearchPathElement};
use self::wsl::{
    linux_absolute_path_to_unc, parse_wsl_root_output, translate_linux_search_path, WslContext,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::hash::Hash;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

/// kpathsea の探索種別と一対一に対応させる、rtex 側の用途。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum FileKind {
    Tex,
    Format,
    Tfm,
    Vf,
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
            Self::Vf => "vf",
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
    InProcessKpathsea,
    FilenameDatabase,
    Kpsewhich,
    WslKpsewhich,
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

/// `ls-R` fast path の発見方法。
///
/// `ResolverOptions::default()` は合成 executor を使う既存の明示 constructor を壊さない
/// よう無効である。実運用の `KpsewhichResolver::default()` だけが `Auto` を選ぶ。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum FilenameDatabaseSearch {
    #[default]
    Disabled,
    Auto,
    Explicit(Vec<PathBuf>),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum WslFallbackPolicy {
    #[default]
    Disabled,
    AutoDefault,
    Distribution(OsString),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolverOptions {
    kpsewhich_program: OsString,
    program_name: OsString,
    external_format_search: ExternalFormatSearch,
    filename_database_search: FilenameDatabaseSearch,
    wsl_fallback: WslFallbackPolicy,
    wsl_program: OsString,
    wsl_kpsewhich_program: OsString,
}

impl Default for ResolverOptions {
    fn default() -> Self {
        Self {
            kpsewhich_program: OsString::from("kpsewhich"),
            program_name: OsString::from("pratex"),
            external_format_search: ExternalFormatSearch::LocalOnly,
            filename_database_search: FilenameDatabaseSearch::Disabled,
            wsl_fallback: WslFallbackPolicy::Disabled,
            wsl_program: OsString::from("wsl.exe"),
            wsl_kpsewhich_program: OsString::from("kpsewhich"),
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

    pub(crate) fn with_filename_database_search(mut self, policy: FilenameDatabaseSearch) -> Self {
        self.filename_database_search = policy;
        self
    }

    pub(crate) fn with_wsl_fallback(mut self, policy: WslFallbackPolicy) -> Self {
        self.wsl_fallback = policy;
        self
    }

    pub(crate) fn with_wsl_program(mut self, program: impl Into<OsString>) -> Self {
        self.wsl_program = program.into();
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

/// 一つのTeX runに属するresolverを、Scannerと遅延生成される出力backendで共有するhandle。
///
/// PraTeX coreは単threadであり、各`resolve`は同期的に完了する。resolver本体をprocess globalや
/// thread localへ置かず、runの所有者がこのhandleを一度だけ作って必要なconsumerへcloneする。
/// cloneしてもpositive/negative cache、`ls-R` catalog、探索path、native/WSL backend選択は
/// 同じresolver instanceに残る。
#[derive(Clone)]
pub(crate) struct RunFileResolver {
    inner: Rc<RefCell<Box<dyn FileResolver>>>,
}

impl RunFileResolver {
    pub(crate) fn new<R>(resolver: R) -> Self
    where
        R: FileResolver + 'static,
    {
        Self {
            inner: Rc::new(RefCell::new(Box::new(resolver))),
        }
    }

    pub(crate) fn from_boxed(resolver: Box<dyn FileResolver>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(resolver)),
        }
    }
}

impl Default for RunFileResolver {
    fn default() -> Self {
        Self::new(NativeFirstResolver::default())
    }
}

impl FileResolver for RunFileResolver {
    fn resolve(
        &mut self,
        kind: FileKind,
        logical_name: &LogicalFileName,
    ) -> Result<Option<ResolvedFile>, ResolveError> {
        self.inner.borrow_mut().resolve(kind, logical_name)
    }
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

enum LocalBoundary {
    Resolved(Option<ResolvedFile>),
    External(Query),
}

fn resolve_local_boundary(
    kind: FileKind,
    logical_name: &LogicalFileName,
    external_format_search: ExternalFormatSearch,
) -> Result<LocalBoundary, ResolveError> {
    let direct_path = PathBuf::from(logical_name.as_os_str());
    match fs::metadata(&direct_path) {
        Ok(metadata) if metadata.is_file() => {
            return Ok(LocalBoundary::Resolved(Some(ResolvedFile {
                logical_name: logical_name.clone(),
                physical_path: direct_path,
                source: ResolutionSource::DirectPath,
            })));
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

    if kind == FileKind::Format && external_format_search == ExternalFormatSearch::LocalOnly {
        return Ok(LocalBoundary::Resolved(None));
    }

    Ok(LocalBoundary::External(Query {
        kind,
        logical_name: logical_name.clone(),
    }))
}

#[derive(Clone, Debug)]
struct CachedExternalResolution {
    physical_path: PathBuf,
    source: ResolutionSource,
}

enum DatabaseCatalog {
    Uninitialized,
    Unavailable,
    Ready(Vec<LsRDatabase>),
}

enum BackendState {
    Undecided,
    Native,
    Wsl(WslContext),
    WslUnavailable(WslDiscoveryFailure),
}

#[derive(Clone, Debug)]
enum WslDiscoveryFailure {
    Launch {
        program: OsString,
        source: ReplayableIoError,
    },
    Failed {
        code: Option<i32>,
        stderr: Vec<u8>,
    },
    Malformed(&'static str),
}

impl WslDiscoveryFailure {
    fn to_resolve_error(&self) -> ResolveError {
        match self {
            Self::Launch { program, source } => ResolveError::LaunchWsl {
                program: program.clone(),
                source: source.to_io_error(),
            },
            Self::Failed { code, stderr } => ResolveError::WslDiscoveryFailed {
                code: *code,
                stderr: stderr.clone(),
            },
            Self::Malformed(reason) => ResolveError::MalformedWslOutput(reason),
        }
    }
}

#[derive(Clone, Debug)]
struct ReplayableIoError {
    kind: io::ErrorKind,
    raw_os_error: Option<i32>,
    message: String,
}

impl ReplayableIoError {
    fn capture(source: io::Error) -> Self {
        Self {
            kind: source.kind(),
            raw_os_error: source.raw_os_error(),
            message: source.to_string(),
        }
    }

    fn to_io_error(&self) -> io::Error {
        match self.raw_os_error {
            Some(code) => io::Error::from_raw_os_error(code),
            None => io::Error::new(self.kind, self.message.clone()),
        }
    }
}

#[derive(Clone, Copy)]
enum ExecutedBackend {
    Native,
    Wsl,
}

struct ExecutedKpsewhich {
    backend: ExecutedBackend,
    output: CommandOutput,
}

/// 直接パスと `kpsewhich` を順に試す resolver。
///
/// Windows の実運用既定値は、native `kpsewhich` が存在しない場合だけ既定 WSL へ移る。
/// 明示 constructor の既定値は WSL 無効のため、埋め込み側が異なる TeX Live を意図せず
/// 混ぜることはない。一度選んだ backend は run 中固定する。
pub(crate) struct KpsewhichResolver<E = ProcessCommandExecutor> {
    options: ResolverOptions,
    executor: E,
    external_cache: HashMap<Query, Option<CachedExternalResolution>>,
    database_catalog: DatabaseCatalog,
    search_paths: HashMap<FileKind, Option<SearchPath>>,
    backend_state: BackendState,
}

impl Default for KpsewhichResolver<ProcessCommandExecutor> {
    fn default() -> Self {
        let options =
            ResolverOptions::default().with_filename_database_search(FilenameDatabaseSearch::Auto);
        #[cfg(windows)]
        {
            options = options.with_wsl_fallback(WslFallbackPolicy::AutoDefault);
        }
        Self::new(options, ProcessCommandExecutor)
    }
}

impl<E> KpsewhichResolver<E> {
    pub(crate) fn new(options: ResolverOptions, executor: E) -> Self {
        Self {
            options,
            executor,
            external_cache: HashMap::new(),
            database_catalog: DatabaseCatalog::Uninitialized,
            search_paths: HashMap::new(),
            backend_state: BackendState::Undecided,
        }
    }

    /// TeX の探索環境が変わった場合に、成功と不在の両方を再照会できるようにする。
    pub(crate) fn clear_external_cache(&mut self) {
        self.external_cache.clear();
        self.database_catalog = DatabaseCatalog::Uninitialized;
        self.search_paths.clear();
        self.backend_state = BackendState::Undecided;
    }
}

impl<E: CommandExecutor> FileResolver for KpsewhichResolver<E> {
    fn resolve(
        &mut self,
        kind: FileKind,
        logical_name: &LogicalFileName,
    ) -> Result<Option<ResolvedFile>, ResolveError> {
        let query = match resolve_local_boundary(
            kind,
            logical_name,
            self.options.external_format_search,
        )? {
            LocalBoundary::Resolved(resolved) => return Ok(resolved),
            LocalBoundary::External(query) => query,
        };
        self.resolve_external(query)
    }
}

impl<E: CommandExecutor> KpsewhichResolver<E> {
    fn resolve_external(&mut self, query: Query) -> Result<Option<ResolvedFile>, ResolveError> {
        if let Some(cached_resolution) = self.external_cache.get(&query) {
            return Ok(cached_resolution.clone().map(|cached| ResolvedFile {
                logical_name: query.logical_name.clone(),
                physical_path: cached.physical_path,
                source: cached.source,
            }));
        }

        if let Some(physical_path) = self.probe_filename_database(&query) {
            let cached = CachedExternalResolution {
                physical_path: physical_path.clone(),
                source: ResolutionSource::FilenameDatabase,
            };
            self.external_cache.insert(query.clone(), Some(cached));
            return Ok(Some(ResolvedFile {
                logical_name: query.logical_name.clone(),
                physical_path,
                source: ResolutionSource::FilenameDatabase,
            }));
        }

        let arguments = self.kpsewhich_arguments(&query);
        let executed = self.execute_kpsewhich(&arguments)?;
        let (physical_path, source) = match executed.backend {
            ExecutedBackend::Native => (
                classify_kpsewhich_output(executed.output)?,
                ResolutionSource::Kpsewhich,
            ),
            ExecutedBackend::Wsl => {
                let context = self.wsl_context().expect("WSL backend without a context");
                (
                    classify_wsl_kpsewhich_output(executed.output, context)?,
                    ResolutionSource::WslKpsewhich,
                )
            }
        };
        self.external_cache.insert(
            query.clone(),
            physical_path
                .clone()
                .map(|physical_path| CachedExternalResolution {
                    physical_path,
                    source,
                }),
        );
        Ok(physical_path.map(|physical_path| ResolvedFile {
            logical_name: query.logical_name,
            physical_path,
            source,
        }))
    }
}

impl<E: CommandExecutor> KpsewhichResolver<E> {
    fn execute_kpsewhich(
        &mut self,
        arguments: &[OsString],
    ) -> Result<ExecutedKpsewhich, ResolveError> {
        match &self.backend_state {
            BackendState::Native => {
                return self.execute_native_kpsewhich(arguments);
            }
            BackendState::Wsl(context) => {
                let context = context.clone();
                return self.execute_wsl_kpsewhich(&context, arguments);
            }
            BackendState::WslUnavailable(failure) => {
                return Err(failure.to_resolve_error());
            }
            BackendState::Undecided => {}
        }

        match self
            .executor
            .execute(&self.options.kpsewhich_program, arguments)
        {
            Ok(output) => {
                self.backend_state = BackendState::Native;
                Ok(ExecutedKpsewhich {
                    backend: ExecutedBackend::Native,
                    output,
                })
            }
            Err(source)
                if source.kind() == io::ErrorKind::NotFound
                    && self.options.wsl_fallback != WslFallbackPolicy::Disabled =>
            {
                let context = match self.discover_wsl_context() {
                    Ok(context) => context,
                    Err(failure) => {
                        let error = failure.to_resolve_error();
                        self.backend_state = BackendState::WslUnavailable(failure);
                        return Err(error);
                    }
                };
                self.backend_state = BackendState::Wsl(context.clone());
                self.execute_wsl_kpsewhich(&context, arguments)
            }
            Err(source) => Err(ResolveError::LaunchKpsewhich {
                program: self.options.kpsewhich_program.clone(),
                source,
            }),
        }
    }

    fn execute_native_kpsewhich(
        &mut self,
        arguments: &[OsString],
    ) -> Result<ExecutedKpsewhich, ResolveError> {
        let output = self
            .executor
            .execute(&self.options.kpsewhich_program, arguments)
            .map_err(|source| ResolveError::LaunchKpsewhich {
                program: self.options.kpsewhich_program.clone(),
                source,
            })?;
        Ok(ExecutedKpsewhich {
            backend: ExecutedBackend::Native,
            output,
        })
    }

    fn discover_wsl_context(&mut self) -> Result<WslContext, WslDiscoveryFailure> {
        let mut arguments = Vec::new();
        if let WslFallbackPolicy::Distribution(distribution) = &self.options.wsl_fallback {
            arguments.push(OsString::from("--distribution"));
            arguments.push(distribution.clone());
        }
        arguments.extend([
            OsString::from("--cd"),
            OsString::from("/"),
            OsString::from("--exec"),
            OsString::from("wslpath"),
            OsString::from("-w"),
            OsString::from("/"),
        ]);
        let output = self
            .executor
            .execute(&self.options.wsl_program, &arguments)
            .map_err(|source| WslDiscoveryFailure::Launch {
                program: self.options.wsl_program.clone(),
                source: ReplayableIoError::capture(source),
            })?;
        if output.code != Some(0) {
            return Err(WslDiscoveryFailure::Failed {
                code: output.code,
                stderr: output.stderr,
            });
        }
        parse_wsl_root_output(&output.stdout).map_err(WslDiscoveryFailure::Malformed)
    }

    fn execute_wsl_kpsewhich(
        &mut self,
        context: &WslContext,
        kpsewhich_arguments: &[OsString],
    ) -> Result<ExecutedKpsewhich, ResolveError> {
        if kpsewhich_arguments
            .iter()
            .any(|argument| argument.to_str().is_none())
        {
            return Err(ResolveError::UnrepresentableWslArgument);
        }
        let mut arguments = vec![
            OsString::from("--distribution"),
            context.distribution_name().to_os_string(),
            OsString::from("--cd"),
            OsString::from("/"),
            OsString::from("--exec"),
            self.options.wsl_kpsewhich_program.clone(),
        ];
        arguments.extend_from_slice(kpsewhich_arguments);
        let output = self
            .executor
            .execute(&self.options.wsl_program, &arguments)
            .map_err(|source| ResolveError::LaunchWsl {
                program: self.options.wsl_program.clone(),
                source,
            })?;
        Ok(ExecutedKpsewhich {
            backend: ExecutedBackend::Wsl,
            output,
        })
    }

    fn wsl_context(&self) -> Option<&WslContext> {
        match &self.backend_state {
            BackendState::Wsl(context) => Some(context),
            BackendState::Undecided | BackendState::Native | BackendState::WslUnavailable(_) => {
                None
            }
        }
    }

    fn probe_filename_database(&mut self, query: &Query) -> Option<PathBuf> {
        self.ensure_database_catalog();
        if matches!(self.database_catalog, DatabaseCatalog::Ready(_)) {
            self.ensure_search_path(query.kind);
        }
        if !matches!(self.database_catalog, DatabaseCatalog::Ready(_)) {
            return None;
        }
        let search_path = self.search_paths.get(&query.kind)?.as_ref()?;
        let DatabaseCatalog::Ready(databases) = &self.database_catalog else {
            return None;
        };
        choose_filename_database_hit(databases, search_path, query.logical_name.as_os_str())
    }

    fn ensure_database_catalog(&mut self) {
        if !matches!(self.database_catalog, DatabaseCatalog::Uninitialized) {
            return;
        }
        let policy = self.options.filename_database_search.clone();
        let paths = match policy {
            FilenameDatabaseSearch::Disabled => {
                self.database_catalog = DatabaseCatalog::Unavailable;
                return;
            }
            FilenameDatabaseSearch::Explicit(paths) => Some(paths),
            FilenameDatabaseSearch::Auto => self.discover_database_paths(),
        };
        let Some(paths) = paths else {
            self.database_catalog = DatabaseCatalog::Unavailable;
            return;
        };

        let mut unique_paths = Vec::new();
        for path in paths {
            if !path.is_absolute()
                || path.file_name() != Some(OsStr::new("ls-R"))
                || unique_paths.contains(&path)
            {
                self.database_catalog = DatabaseCatalog::Unavailable;
                return;
            }
            unique_paths.push(path);
        }
        if unique_paths.is_empty() {
            self.database_catalog = DatabaseCatalog::Unavailable;
            return;
        }

        let mut databases = Vec::with_capacity(unique_paths.len());
        for path in unique_paths {
            match LsRDatabase::load(path) {
                Ok(database) => databases.push(database),
                Err(_) => {
                    // 一部だけ使うと、壊れた DB にある先行候補を飛ばしかねない。
                    self.database_catalog = DatabaseCatalog::Unavailable;
                    return;
                }
            }
        }
        self.database_catalog = DatabaseCatalog::Ready(databases);
    }

    fn discover_database_paths(&mut self) -> Option<Vec<PathBuf>> {
        let arguments = vec![
            OsString::from("--all"),
            OsString::from("--must-exist"),
            option_with_value("--progname=", &self.options.program_name),
            OsString::from("--format=ls-R"),
            OsString::from("--"),
            OsString::from("ls-R"),
        ];
        let executed = self.execute_kpsewhich(&arguments).ok()?;
        match executed.backend {
            ExecutedBackend::Native => parse_database_discovery_output(executed.output),
            ExecutedBackend::Wsl => {
                parse_wsl_database_discovery_output(executed.output, self.wsl_context()?)
            }
        }
    }

    fn ensure_search_path(&mut self, kind: FileKind) {
        if self.search_paths.contains_key(&kind) {
            return;
        }
        let search_path = self.query_search_path(kind);
        self.search_paths.insert(kind, search_path);
    }

    fn query_search_path(&mut self, kind: FileKind) -> Option<SearchPath> {
        let mut arguments = vec![
            option_with_value("--progname=", &self.options.program_name),
            OsString::from(format!("--show-path={}", kind.kpsewhich_format())),
        ];
        if kind == FileKind::Format
            && self.options.external_format_search == ExternalFormatSearch::KpsewhichRtexEngine
        {
            arguments.push(OsString::from("--engine=rtex"));
        }
        self.execute_kpsewhich(&arguments)
            .ok()
            .and_then(|executed| match executed.backend {
                ExecutedBackend::Native => parse_search_path_output(executed.output),
                ExecutedBackend::Wsl => parse_wsl_search_path_output(
                    executed.output,
                    self.wsl_context().expect("WSL backend without a context"),
                ),
            })
    }
}

struct IndexedCandidate {
    directory: PathBuf,
    path: PathBuf,
}

fn choose_filename_database_hit(
    databases: &[LsRDatabase],
    search_path: &SearchPath,
    logical_name: &OsStr,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for database in databases {
        if !database.is_unchanged() {
            return None;
        }
        let indexed = database.candidates(logical_name)?;
        candidates.extend(indexed.into_iter().map(|candidate| IndexedCandidate {
            directory: candidate.directory().to_path_buf(),
            path: candidate.path().to_path_buf(),
        }));
    }

    for element in search_path.elements() {
        let (database_only, pattern) = match element {
            SearchPathElement::CurrentDirectory => continue,
            SearchPathElement::Unsupported => return None,
            SearchPathElement::Supported {
                database_only,
                pattern,
            } => (*database_only, pattern),
        };

        let mut live_paths = Vec::new();
        for candidate in candidates
            .iter()
            .filter(|candidate| pattern.matches_directory(&candidate.directory))
        {
            match fs::metadata(&candidate.path) {
                Ok(metadata) if metadata.is_file() => {
                    if !live_paths.contains(&candidate.path) {
                        live_paths.push(candidate.path.clone());
                    }
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => return None,
            }
        }
        match live_paths.len() {
            1 => return live_paths.pop(),
            2.. => return None,
            0 => {}
        }

        let covered_databases = databases
            .iter()
            .enumerate()
            .filter(|(_, database)| pattern.is_covered_by(database.root()))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if covered_databases
            .iter()
            .any(|&index| !matches!(databases[index].alias_match(logical_name), AliasMatch::No))
        {
            return None;
        }

        if database_only {
            // `!!` は disk fallback を禁止する。発見済み DB のどれにも属さない要素も
            // database 上は不在なので、次の要素へ進んでよい。
            continue;
        }

        if let Some(directory) = pattern.exact_directory() {
            match fs::metadata(directory.join(logical_name)) {
                Ok(metadata) if metadata.is_file() => return None,
                Ok(_) => continue,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(_) => return None,
            }
        }
        match fs::metadata(pattern.disk_root()) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            // 実在する再帰 tree を列挙せず飛ばすと、利用者側の override を失う。
            Ok(_) | Err(_) => return None,
        }
    }
    None
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

const MAX_DATABASE_DISCOVERY_OUTPUT: usize = 16 * 1024 * 1024;
const MAX_SEARCH_PATH_OUTPUT: usize = 4 * 1024 * 1024;

fn parse_database_discovery_output(output: CommandOutput) -> Option<Vec<PathBuf>> {
    if output.code != Some(0) || output.stdout.len() > MAX_DATABASE_DISCOVERY_OUTPUT {
        return None;
    }
    let mut paths = Vec::new();
    for raw_line in output.stdout.split(|&byte| byte == b'\n') {
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.is_empty() {
            continue;
        }
        if line.contains(&0) || line.contains(&b'\r') {
            return None;
        }
        let path = PathBuf::from(os_string_from_output(line.to_vec()).ok()?);
        paths.push(path);
    }
    (!paths.is_empty()).then_some(paths)
}

fn parse_wsl_database_discovery_output(
    output: CommandOutput,
    context: &WslContext,
) -> Option<Vec<PathBuf>> {
    if output.code != Some(0) || output.stdout.len() > MAX_DATABASE_DISCOVERY_OUTPUT {
        return None;
    }
    let mut paths = Vec::new();
    for raw_line in output.stdout.split(|&byte| byte == b'\n') {
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.is_empty() {
            continue;
        }
        if line.contains(&0) || line.contains(&b'\r') {
            return None;
        }
        let linux_path = std::str::from_utf8(line).ok()?;
        paths.push(linux_absolute_path_to_unc(context, linux_path).ok()?);
    }
    (!paths.is_empty()).then_some(paths)
}

fn parse_search_path_output(output: CommandOutput) -> Option<SearchPath> {
    if output.code != Some(0) || output.stdout.len() > MAX_SEARCH_PATH_OUTPUT {
        return None;
    }
    let bytes = trim_line_endings(&output.stdout);
    if bytes.is_empty() || bytes.contains(&0) || bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        return None;
    }
    let value = os_string_from_output(bytes.to_vec()).ok()?;
    SearchPath::parse(&value)
}

fn parse_wsl_search_path_output(output: CommandOutput, context: &WslContext) -> Option<SearchPath> {
    if output.code != Some(0) || output.stdout.len() > MAX_SEARCH_PATH_OUTPUT {
        return None;
    }
    let translated = translate_linux_search_path(context, &output.stdout).ok()?;
    SearchPath::parse(&translated)
}

fn classify_wsl_kpsewhich_output(
    output: CommandOutput,
    context: &WslContext,
) -> Result<Option<PathBuf>, ResolveError> {
    if output.code == Some(0) {
        let bytes = trim_line_endings(&output.stdout);
        if bytes.is_empty() {
            return Ok(None);
        }
        if bytes.contains(&0) || bytes.contains(&b'\n') || bytes.contains(&b'\r') {
            return Err(ResolveError::MalformedWslOutput(
                "kpsewhich returned more than one Linux pathname",
            ));
        }
        let linux_path = std::str::from_utf8(bytes)
            .map_err(|_| ResolveError::MalformedWslOutput("Linux pathname is not valid UTF-8"))?;
        let physical_path = linux_absolute_path_to_unc(context, linux_path)
            .map_err(ResolveError::MalformedWslOutput)?;
        match fs::metadata(&physical_path) {
            Ok(metadata) if metadata.is_file() => return Ok(Some(physical_path)),
            Ok(_) => {
                return Err(ResolveError::MalformedWslOutput(
                    "kpsewhich pathname is not a regular file",
                ));
            }
            Err(source) => {
                return Err(ResolveError::InspectWslPath {
                    path: physical_path,
                    source,
                });
            }
        }
    }

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
    InProcessKpathsea(FastPathFailure),
    InspectInProcessPath {
        path: PathBuf,
        source: io::Error,
    },
    InProcessPathNotFile {
        path: PathBuf,
    },
    LaunchKpsewhich {
        program: OsString,
        source: io::Error,
    },
    LaunchWsl {
        program: OsString,
        source: io::Error,
    },
    WslDiscoveryFailed {
        code: Option<i32>,
        stderr: Vec<u8>,
    },
    MalformedWslOutput(&'static str),
    UnrepresentableWslArgument,
    InspectWslPath {
        path: PathBuf,
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
            Self::InProcessKpathsea(reason) => {
                write!(formatter, "in-process kpathsea lookup failed: {reason}")
            }
            Self::InspectInProcessPath { path, source } => write!(
                formatter,
                "in-process kpathsea found `{}`, but it cannot be inspected: {source}",
                path.display()
            ),
            Self::InProcessPathNotFile { path } => write!(
                formatter,
                "in-process kpathsea found `{}`, but it is not a regular file",
                path.display()
            ),
            Self::LaunchKpsewhich { program, source } => write!(
                formatter,
                "cannot launch `{}`: {source}",
                program.to_string_lossy()
            ),
            Self::LaunchWsl { program, source } => write!(
                formatter,
                "cannot launch WSL through `{}`: {source}",
                program.to_string_lossy()
            ),
            Self::WslDiscoveryFailed { code, stderr } => write!(
                formatter,
                "WSL distribution discovery failed with status {code:?}: {}",
                String::from_utf8_lossy(stderr)
            ),
            Self::MalformedWslOutput(reason) => {
                write!(formatter, "WSL returned malformed output: {reason}")
            }
            Self::UnrepresentableWslArgument => {
                write!(
                    formatter,
                    "the filename cannot be represented in a WSL argument"
                )
            }
            Self::InspectWslPath { path, source } => write!(
                formatter,
                "WSL found `{}`, but Windows cannot inspect it: {source}",
                path.display()
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
            Self::InspectDirectPath { source, .. }
            | Self::InspectInProcessPath { source, .. }
            | Self::LaunchKpsewhich { source, .. }
            | Self::LaunchWsl { source, .. }
            | Self::InspectWslPath { source, .. } => Some(source),
            Self::InProcessKpathsea(_)
            | Self::InProcessPathNotFile { .. }
            | Self::WslDiscoveryFailed { .. }
            | Self::MalformedWslOutput(_)
            | Self::UnrepresentableWslArgument
            | Self::KpsewhichFailed { .. }
            | Self::MalformedKpsewhichOutput(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandExecutor, CommandOutput, ExternalFormatSearch, FileKind, FileResolver,
        FilenameDatabaseSearch, KpsewhichResolver, LogicalFileName, ResolutionSource, ResolveError,
        ResolverOptions, WslFallbackPolicy,
    };
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
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

    fn success_os(value: &OsStr) -> io::Result<CommandOutput> {
        let mut stdout = os_bytes(value);
        stdout.extend_from_slice(b"\r\n");
        Ok(CommandOutput {
            code: Some(0),
            stdout,
            stderr: Vec::new(),
        })
    }

    #[cfg(unix)]
    fn os_bytes(value: &OsStr) -> Vec<u8> {
        use std::os::unix::ffi::OsStrExt;
        value.as_bytes().to_vec()
    }

    #[cfg(not(unix))]
    fn os_bytes(value: &OsStr) -> Vec<u8> {
        value.to_str().expect("試験pathはUTF-8").as_bytes().to_vec()
    }

    fn search_path_success(
        elements: impl IntoIterator<Item = PathBuf>,
    ) -> io::Result<CommandOutput> {
        let joined = std::env::join_paths(elements).unwrap();
        success_os(&joined)
    }

    fn database_only_recursive(root: &Path) -> PathBuf {
        let mut value = OsString::from("!!");
        value.push(root);
        value.push("//");
        PathBuf::from(value)
    }

    fn recursive(root: &Path) -> PathBuf {
        let mut value = root.as_os_str().to_os_string();
        value.push("//");
        PathBuf::from(value)
    }

    struct DatabaseFixture {
        root: PathBuf,
        database_path: PathBuf,
    }

    impl DatabaseFixture {
        fn new(label: &str, database: &[u8], files: &[&str]) -> Self {
            static NEXT_FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "pratex-resolver-lsr-{label}-{}-{}",
                std::process::id(),
                NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            for relative in files {
                let path = root.join(relative);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, b"fixture").unwrap();
            }
            let database_path = root.join("ls-R");
            fs::write(&database_path, database).unwrap();
            Self {
                root,
                database_path,
            }
        }

        fn options(&self) -> ResolverOptions {
            ResolverOptions::default().with_filename_database_search(
                FilenameDatabaseSearch::Explicit(vec![self.database_path.clone()]),
            )
        }

        fn write_aliases(&self, bytes: &[u8]) {
            fs::write(self.root.join("aliases"), bytes).unwrap();
        }
    }

    impl Drop for DatabaseFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
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
    fn tfmとvfは同じrun_cacheで用途を混ぜずに解決する() {
        let fake = FakeExecutor::with_responses([
            success(b"/tree/fonts/tfm/public/example.tfm"),
            success(b"/tree/fonts/vf/public/example.vf"),
        ]);
        let mut resolver = KpsewhichResolver::new(ResolverOptions::default(), fake.clone());
        let tfm_name = unique_absent_name("example.tfm");
        let vf_name = unique_absent_name("example.vf");

        for _ in 0..2 {
            let tfm = resolver.resolve(FileKind::Tfm, &tfm_name).unwrap().unwrap();
            let vf = resolver.resolve(FileKind::Vf, &vf_name).unwrap().unwrap();
            assert_eq!(tfm.logical_name(), &tfm_name);
            assert_eq!(vf.logical_name(), &vf_name);
            assert_ne!(
                tfm.logical_name().as_os_str(),
                tfm.physical_path().as_os_str()
            );
            assert_ne!(
                vf.logical_name().as_os_str(),
                vf.physical_path().as_os_str()
            );
        }

        let invocations = fake.invocations.borrow();
        assert_eq!(invocations.len(), 2);
        assert!(invocations[0]
            .arguments
            .contains(&OsString::from("--format=tfm")));
        assert_eq!(
            invocations[0].arguments.last().map(OsString::as_os_str),
            Some(tfm_name.as_os_str())
        );
        assert!(invocations[1]
            .arguments
            .contains(&OsString::from("--format=vf")));
        assert_eq!(
            invocations[1].arguments.last().map(OsString::as_os_str),
            Some(vf_name.as_os_str())
        );
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
    fn nativeが起動できればwslを混ぜない() {
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let fake = FakeExecutor::with_responses([success(b"/native/found.tex"), missing()]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        let found = resolver
            .resolve(FileKind::Tex, &unique_absent_name("native-found.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(found.source(), ResolutionSource::Kpsewhich);
        assert!(resolver
            .resolve(FileKind::Tex, &unique_absent_name("native-missing.tex"))
            .unwrap()
            .is_none());

        let invocations = fake.invocations.borrow();
        assert_eq!(invocations.len(), 2);
        assert!(invocations
            .iter()
            .all(|invocation| invocation.program == OsString::from("kpsewhich")));
    }

    #[test]
    fn nativeの不在回答をwslで覆わない() {
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let fake = FakeExecutor::with_responses([missing()]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        assert!(resolver
            .resolve(FileKind::Tex, &unique_absent_name("native-none.tex"))
            .unwrap()
            .is_none());
        assert_eq!(fake.invocation_count(), 1);
        assert_eq!(
            fake.invocations.borrow()[0].program,
            OsString::from("kpsewhich")
        );
    }

    #[test]
    fn native実行fileが無いときだけ既定wslへ移る() {
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let fake = FakeExecutor::with_responses([
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "native kpsewhichなし",
            )),
            success(br"\\wsl.localhost\Ubuntu-24.04\"),
            missing(),
            missing(),
        ]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        for suffix in ["first.tex", "second.tex"] {
            assert!(resolver
                .resolve(FileKind::Tex, &unique_absent_name(suffix))
                .unwrap()
                .is_none());
        }

        let invocations = fake.invocations.borrow();
        assert_eq!(invocations.len(), 4);
        assert_eq!(invocations[0].program, OsString::from("kpsewhich"));
        assert!(invocations[1..]
            .iter()
            .all(|invocation| invocation.program == OsString::from("fake-wsl")));
        assert_eq!(
            invocations[1].arguments,
            ["--cd", "/", "--exec", "wslpath", "-w", "/"].map(OsString::from)
        );
        assert!(invocations[2]
            .arguments
            .windows(2)
            .any(|pair| pair == ["--distribution", "Ubuntu-24.04"]));
    }

    #[test]
    fn wsl発見の異常終了はrun内で一度だけ実行して同じ診断を返す() {
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let fake = FakeExecutor::with_responses([
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "native kpsewhichなし",
            )),
            Ok(CommandOutput {
                code: Some(1),
                stdout: Vec::new(),
                stderr: b"distribution unavailable".to_vec(),
            }),
        ]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        let first = resolver
            .resolve(FileKind::Tex, &unique_absent_name("first-optional.cfg"))
            .expect_err("WSL発見失敗を返す");
        let second = resolver
            .resolve(FileKind::Tex, &unique_absent_name("second-optional.cfg"))
            .expect_err("同じWSL発見失敗を返す");

        for error in [&first, &second] {
            assert!(matches!(
                error,
                ResolveError::WslDiscoveryFailed {
                    code: Some(1),
                    stderr,
                } if stderr == b"distribution unavailable"
            ));
        }
        assert_eq!(first.to_string(), second.to_string());
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn wsl発見programの起動失敗もrun内で再実行しない() {
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let fake = FakeExecutor::with_responses([
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "native kpsewhichなし",
            )),
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "wsl blocked",
            )),
        ]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        let first = resolver
            .resolve(FileKind::Tex, &unique_absent_name("first-launch.tex"))
            .expect_err("WSL起動失敗を返す");
        let second = resolver
            .resolve(FileKind::Tex, &unique_absent_name("second-launch.tex"))
            .expect_err("同じWSL起動失敗を返す");

        for error in [&first, &second] {
            match error {
                ResolveError::LaunchWsl { program, source } => {
                    assert_eq!(program, &OsString::from("fake-wsl"));
                    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
                    assert_eq!(source.to_string(), "wsl blocked");
                }
                _ => panic!("WSL起動失敗ではない: {error}"),
            }
        }
        assert_eq!(first.to_string(), second.to_string());
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn wsl発見のraw_os_errorをrun内の再生で保つ() {
        let original = io::Error::from_raw_os_error(5);
        let expected_kind = original.kind();
        let expected_message = original.to_string();
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let fake = FakeExecutor::with_responses([
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "native kpsewhichなし",
            )),
            Err(original),
        ]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        for suffix in ["first-os-error.tex", "second-os-error.tex"] {
            match resolver.resolve(FileKind::Tex, &unique_absent_name(suffix)) {
                Err(ResolveError::LaunchWsl { program, source }) => {
                    assert_eq!(program, OsString::from("fake-wsl"));
                    assert_eq!(source.raw_os_error(), Some(5));
                    assert_eq!(source.kind(), expected_kind);
                    assert_eq!(source.to_string(), expected_message);
                }
                Err(error) => panic!("WSL起動失敗ではない: {error}"),
                Ok(_) => panic!("WSL発見失敗を成功にしている"),
            }
        }
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn 不正なwsl_rootもrun内で再解釈しない() {
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let fake = FakeExecutor::with_responses([
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "native kpsewhichなし",
            )),
            success(b"not-a-wsl-root"),
        ]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        for suffix in ["first-malformed.tex", "second-malformed.tex"] {
            assert!(matches!(
                resolver.resolve(FileKind::Tex, &unique_absent_name(suffix)),
                Err(ResolveError::MalformedWslOutput(_))
            ));
        }
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn clear後はcacheしたwsl発見失敗も再試行する() {
        let discovery_failure = |status, stderr: &'static [u8]| {
            [
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "native kpsewhichなし",
                )),
                Ok(CommandOutput {
                    code: Some(status),
                    stdout: Vec::new(),
                    stderr: stderr.to_vec(),
                }),
            ]
        };
        let fake = FakeExecutor::with_responses(
            discovery_failure(1, b"first failure")
                .into_iter()
                .chain(discovery_failure(2, b"second failure")),
        );
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        for suffix in ["first-before-clear.tex", "second-before-clear.tex"] {
            assert!(matches!(
                resolver.resolve(FileKind::Tex, &unique_absent_name(suffix)),
                Err(ResolveError::WslDiscoveryFailed {
                    code: Some(1),
                    stderr,
                }) if stderr == b"first failure"
            ));
        }
        assert_eq!(fake.invocation_count(), 2);

        resolver.clear_external_cache();
        assert!(matches!(
            resolver.resolve(
                FileKind::Tex,
                &unique_absent_name("after-clear.tex"),
            ),
            Err(ResolveError::WslDiscoveryFailed {
                code: Some(2),
                stderr,
            }) if stderr == b"second failure"
        ));
        assert_eq!(fake.invocation_count(), 4);
    }

    #[test]
    fn clear後はwsl発見失敗から成功へ回復する() {
        let fake = FakeExecutor::with_responses([
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "native kpsewhichなし",
            )),
            Ok(CommandOutput {
                code: Some(1),
                stdout: Vec::new(),
                stderr: b"temporary discovery failure".to_vec(),
            }),
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "native kpsewhichなし",
            )),
            success(br"\\wsl.localhost\Ubuntu-24.04\"),
            missing(),
        ]);
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        for suffix in ["first-before-recovery.tex", "second-before-recovery.tex"] {
            assert!(matches!(
                resolver.resolve(FileKind::Tex, &unique_absent_name(suffix)),
                Err(ResolveError::WslDiscoveryFailed {
                    code: Some(1),
                    stderr,
                }) if stderr == b"temporary discovery failure"
            ));
        }
        assert_eq!(fake.invocation_count(), 2);

        resolver.clear_external_cache();
        assert!(resolver
            .resolve(FileKind::Tex, &unique_absent_name("after-recovery.tex"),)
            .unwrap()
            .is_none());
        assert_eq!(fake.invocation_count(), 5);
        let invocations = fake.invocations.borrow();
        assert_eq!(invocations[3].program, OsString::from("fake-wsl"));
        assert_eq!(
            invocations[3].arguments,
            ["--cd", "/", "--exec", "wslpath", "-w", "/"].map(OsString::from)
        );
        assert!(invocations[4]
            .arguments
            .windows(2)
            .any(|pair| pair == ["--distribution", "Ubuntu-24.04"]));
    }

    #[test]
    fn nativeの異常をwslで隠さない() {
        for error_response in [
            Ok(CommandOutput {
                code: Some(2),
                stdout: Vec::new(),
                stderr: b"native broken".to_vec(),
            }),
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "native denied",
            )),
        ] {
            let options = ResolverOptions::default()
                .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
                .with_wsl_program("fake-wsl");
            let fake = FakeExecutor::with_responses([error_response]);
            let mut resolver = KpsewhichResolver::new(options, fake.clone());

            assert!(resolver
                .resolve(FileKind::Tex, &unique_absent_name("native-error.tex"))
                .is_err());
            assert_eq!(fake.invocation_count(), 1);
        }
    }

    #[test]
    fn 明示distributionを一argumentでwslpathへ渡す() {
        let distribution = "TeX Live 日本語";
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::Distribution(OsString::from(
                distribution,
            )))
            .with_wsl_program("fake-wsl");
        let root = format!("\\\\wsl.localhost\\{distribution}\\");
        let fake = FakeExecutor::with_responses([
            Err(io::Error::new(io::ErrorKind::NotFound, "nativeなし")),
            success(root.as_bytes()),
            missing(),
        ]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        assert!(resolver
            .resolve(FileKind::Tex, &unique_absent_name("explicit-wsl.tex"))
            .unwrap()
            .is_none());
        let invocation = &fake.invocations.borrow()[1];
        assert_eq!(invocation.arguments[0], OsString::from("--distribution"));
        assert_eq!(invocation.arguments[1], OsString::from(distribution));
    }

    #[test]
    fn clear後はwsl_backendも再発見する() {
        let cycle = || {
            [
                Err(io::Error::new(io::ErrorKind::NotFound, "nativeなし")),
                success(br"\\wsl.localhost\Ubuntu-24.04\"),
                missing(),
            ]
        };
        let fake = FakeExecutor::with_responses(cycle().into_iter().chain(cycle()));
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let mut resolver = KpsewhichResolver::new(options, fake.clone());
        let logical = unique_absent_name("clear-wsl.tex");

        assert!(resolver.resolve(FileKind::Tex, &logical).unwrap().is_none());
        resolver.clear_external_cache();
        assert!(resolver.resolve(FileKind::Tex, &logical).unwrap().is_none());
        assert_eq!(fake.invocation_count(), 6);
    }

    #[test]
    fn local限定fmtではwslも起動しない() {
        let options = ResolverOptions::default()
            .with_wsl_fallback(WslFallbackPolicy::AutoDefault)
            .with_wsl_program("fake-wsl");
        let fake = FakeExecutor::with_responses([]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        assert!(resolver
            .resolve(FileKind::Format, &LogicalFileName::new("external.fmt"))
            .unwrap()
            .is_none());
        assert_eq!(fake.invocation_count(), 0);
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
    fn 全用途をpratex名で公開cliのformatへ対応させる() {
        let kinds_and_formats = [
            (FileKind::Tex, "tex"),
            (FileKind::Format, "fmt"),
            (FileKind::Tfm, "tfm"),
            (FileKind::Vf, "vf"),
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
                .contains(&OsString::from("--progname=pratex")));
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

    #[test]
    fn 一意なlsr候補ならfileごとの外部探索を起動しない() {
        let fixture = DatabaseFixture::new(
            "unique",
            b"./tex/latex/base:\nfast.tex\n",
            &["tex/latex/base/fast.tex"],
        );
        let fake = FakeExecutor::with_responses([search_path_success([database_only_recursive(
            &fixture.root.join("tex"),
        )])]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());
        let logical = LogicalFileName::new("fast.tex");

        for _ in 0..2 {
            let resolved = resolver.resolve(FileKind::Tex, &logical).unwrap().unwrap();
            assert_eq!(
                resolved.physical_path(),
                fixture.root.join("tex/latex/base/fast.tex")
            );
            assert_eq!(resolved.source(), ResolutionSource::FilenameDatabase);
        }
        assert_eq!(fake.invocation_count(), 1);
        assert!(fake.invocations.borrow()[0]
            .arguments
            .contains(&OsString::from("--show-path=tex")));
    }

    #[test]
    fn tfmとvfは用途別pathから同じlsr索引を引く() {
        let fixture = DatabaseFixture::new(
            "font-kinds",
            b"./fonts/tfm/public/example:\nmetric.tfm\n\
./fonts/vf/public/example:\nmetric.vf\n",
            &[
                "fonts/tfm/public/example/metric.tfm",
                "fonts/vf/public/example/metric.vf",
            ],
        );
        let fake = FakeExecutor::with_responses([
            search_path_success([database_only_recursive(&fixture.root.join("fonts/tfm"))]),
            search_path_success([database_only_recursive(&fixture.root.join("fonts/vf"))]),
        ]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let tfm = resolver
            .resolve(FileKind::Tfm, &LogicalFileName::new("metric.tfm"))
            .unwrap()
            .unwrap();
        let vf = resolver
            .resolve(FileKind::Vf, &LogicalFileName::new("metric.vf"))
            .unwrap()
            .unwrap();

        assert_eq!(tfm.source(), ResolutionSource::FilenameDatabase);
        assert_eq!(
            tfm.physical_path(),
            fixture.root.join("fonts/tfm/public/example/metric.tfm")
        );
        assert_eq!(vf.source(), ResolutionSource::FilenameDatabase);
        assert_eq!(
            vf.physical_path(),
            fixture.root.join("fonts/vf/public/example/metric.vf")
        );
        let invocations = fake.invocations.borrow();
        assert_eq!(invocations.len(), 2);
        assert!(invocations[0]
            .arguments
            .contains(&OsString::from("--show-path=tfm")));
        assert!(invocations[1]
            .arguments
            .contains(&OsString::from("--show-path=vf")));
    }

    #[test]
    fn 同じ最初の探索要素に複数候補ならkpsewhichへ戻す() {
        let fixture = DatabaseFixture::new(
            "ambiguous",
            b"./tex/a:\nsame.tex\n./tex/b:\nsame.tex\n",
            &["tex/a/same.tex", "tex/b/same.tex"],
        );
        let fallback = fixture.root.join("chosen-by-kpsewhich.tex");
        let fake = FakeExecutor::with_responses([
            search_path_success([database_only_recursive(&fixture.root.join("tex"))]),
            success_os(fallback.as_os_str()),
        ]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("same.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.physical_path(), fallback);
        assert_eq!(resolved.source(), ResolutionSource::Kpsewhich);
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn format別pathで文書用の同名候補を除外する() {
        let fixture = DatabaseFixture::new(
            "kind-filter",
            b"./tex/latex:\nshared.tex\n./doc/latex:\nshared.tex\n",
            &["tex/latex/shared.tex", "doc/latex/shared.tex"],
        );
        let fake = FakeExecutor::with_responses([search_path_success([database_only_recursive(
            &fixture.root.join("tex"),
        )])]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("shared.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(
            resolved.physical_path(),
            fixture.root.join("tex/latex/shared.tex")
        );
        assert_eq!(fake.invocation_count(), 1);
    }

    #[test]
    fn 先行する実在の利用者treeを索引候補で飛ばさない() {
        let fixture = DatabaseFixture::new(
            "user-tree",
            b"./tex/latex:\nsystem.tex\n",
            &["tex/latex/system.tex"],
        );
        let user_tree = fixture.root.join("user-tree");
        fs::create_dir_all(&user_tree).unwrap();
        let fallback = fixture.root.join("kpse-user-choice.tex");
        let fake = FakeExecutor::with_responses([
            search_path_success([
                recursive(&user_tree),
                database_only_recursive(&fixture.root.join("tex")),
            ]),
            success_os(fallback.as_os_str()),
        ]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("system.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.physical_path(), fallback);
        assert_eq!(resolved.source(), ResolutionSource::Kpsewhich);
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn 先行する利用者treeが無ければ後続索引候補を使う() {
        let fixture = DatabaseFixture::new(
            "missing-user-tree",
            b"./tex/latex:\nsystem.tex\n",
            &["tex/latex/system.tex"],
        );
        let missing_user_tree = fixture.root.join("not-created-user-tree");
        let fake = FakeExecutor::with_responses([search_path_success([
            recursive(&missing_user_tree),
            database_only_recursive(&fixture.root.join("tex")),
        ])]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("system.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.source(), ResolutionSource::FilenameDatabase);
        assert_eq!(fake.invocation_count(), 1);
    }

    #[test]
    fn 無関係なaliasは後続の一意なdatabase候補を妨げない() {
        let fixture = DatabaseFixture::new(
            "unrelated-alias",
            b"./tex/first:\nreal.tex\n./tex/second:\ntarget.tex\n",
            &["tex/first/real.tex", "tex/second/target.tex"],
        );
        fixture.write_aliases(b"real.tex unrelated.tex\n");
        let fake = FakeExecutor::with_responses([search_path_success([
            database_only_recursive(&fixture.root.join("tex/first")),
            database_only_recursive(&fixture.root.join("tex/second")),
        ])]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("target.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(
            resolved.physical_path(),
            fixture.root.join("tex/second/target.tex")
        );
        assert_eq!(resolved.source(), ResolutionSource::FilenameDatabase);
        assert_eq!(fake.invocation_count(), 1);
    }

    #[test]
    fn 一致するaliasは直接名のshadowingをnativeに推測せずkpsewhichへ戻す() {
        let fixture = DatabaseFixture::new(
            "matching-alias",
            b"./tex/first:\nreal.tex\n./tex/second:\nalias.tex\n",
            &["tex/first/real.tex", "tex/second/alias.tex"],
        );
        fixture.write_aliases(b"real.tex alias.tex\n");
        let direct_alias = fixture.root.join("tex/second/alias.tex");
        let fake = FakeExecutor::with_responses([
            search_path_success([
                database_only_recursive(&fixture.root.join("tex/first")),
                database_only_recursive(&fixture.root.join("tex/second")),
            ]),
            success_os(direct_alias.as_os_str()),
        ]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("alias.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.physical_path(), direct_alias);
        assert_eq!(resolved.source(), ResolutionSource::Kpsewhich);
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn 壊れたaliasesがあればdatabase全体を使わずkpsewhichへ戻す() {
        let fixture = DatabaseFixture::new(
            "malformed-alias",
            b"./tex:\ntarget.tex\n",
            &["tex/target.tex"],
        );
        fixture.write_aliases(b"one-word\n");
        let fallback = fixture.root.join("chosen-by-kpsewhich.tex");
        let fake = FakeExecutor::with_responses([success_os(fallback.as_os_str())]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("target.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.physical_path(), fallback);
        assert_eq!(resolved.source(), ResolutionSource::Kpsewhich);
        assert_eq!(fake.invocation_count(), 1);
    }

    #[test]
    fn staleな索引候補は不在と断定せずkpsewhichへ戻す() {
        let fixture = DatabaseFixture::new("stale", b"./tex/latex:\nstale.tex\n", &[]);
        let fallback = fixture.root.join("new-location.tex");
        let fake = FakeExecutor::with_responses([
            search_path_success([database_only_recursive(&fixture.root.join("tex"))]),
            success_os(fallback.as_os_str()),
        ]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("stale.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.physical_path(), fallback);
        assert_eq!(resolved.source(), ResolutionSource::Kpsewhich);
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn 壊れたdatabaseの部分索引を使わない() {
        let fixture = DatabaseFixture::new(
            "malformed",
            b"./tex:\nunsafe.tex\nname\0.tex\n",
            &["tex/unsafe.tex"],
        );
        let fallback = fixture.root.join("safe-fallback.tex");
        let fake = FakeExecutor::with_responses([success_os(fallback.as_os_str())]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("unsafe.tex"))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.physical_path(), fallback);
        assert_eq!(fake.invocation_count(), 1);
    }

    #[test]
    fn clear後はdatabaseと探索pathも読み直す() {
        let fixture = DatabaseFixture::new("clear", b"./tex:\nold.tex\n", &["tex/old.tex"]);
        let search_path =
            || search_path_success([database_only_recursive(&fixture.root.join("tex"))]);
        let fake = FakeExecutor::with_responses([search_path(), search_path()]);
        let mut resolver = KpsewhichResolver::new(fixture.options(), fake.clone());

        assert!(resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("old.tex"))
            .unwrap()
            .is_some());
        fs::write(&fixture.database_path, b"./tex:\nnew-long-name.tex\n").unwrap();
        fs::write(fixture.root.join("tex/new-long-name.tex"), b"new").unwrap();
        resolver.clear_external_cache();
        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("new-long-name.tex"))
            .unwrap()
            .unwrap();

        assert_eq!(resolved.source(), ResolutionSource::FilenameDatabase);
        assert_eq!(fake.invocation_count(), 2);
    }

    #[test]
    fn 自動発見は一度で探索pathは用途ごとに一度だけ取る() {
        let fixture = DatabaseFixture::new(
            "auto",
            b"./tex:\none.tex\ntwo.tex\n",
            &["tex/one.tex", "tex/two.tex"],
        );
        let auto_options =
            ResolverOptions::default().with_filename_database_search(FilenameDatabaseSearch::Auto);
        let fake = FakeExecutor::with_responses([
            success_os(fixture.database_path.as_os_str()),
            search_path_success([database_only_recursive(&fixture.root.join("tex"))]),
            search_path_success([database_only_recursive(&fixture.root.join("tex"))]),
        ]);
        let mut resolver = KpsewhichResolver::new(auto_options, fake.clone());

        for name in ["one.tex", "two.tex"] {
            assert!(resolver
                .resolve(FileKind::Tex, &LogicalFileName::new(name))
                .unwrap()
                .is_some());
        }
        assert!(resolver
            .resolve(FileKind::Afm, &LogicalFileName::new("one.tex"))
            .unwrap()
            .is_some());

        let invocations = fake.invocations.borrow();
        assert_eq!(invocations.len(), 3);
        assert!(invocations[0]
            .arguments
            .contains(&OsString::from("--format=ls-R")));
        assert!(invocations[1]
            .arguments
            .contains(&OsString::from("--show-path=tex")));
        assert!(invocations[2]
            .arguments
            .contains(&OsString::from("--show-path=afm")));
    }

    #[test]
    fn autoでも直接pathとlocal限定fmtでは発見を起動しない() {
        let fixture = DatabaseFixture::new("direct-auto", b"./:\n", &["direct.tex"]);
        let options =
            ResolverOptions::default().with_filename_database_search(FilenameDatabaseSearch::Auto);
        let fake = FakeExecutor::with_responses([]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        let direct = LogicalFileName::new(fixture.root.join("direct.tex").as_os_str());
        assert!(resolver.resolve(FileKind::Tex, &direct).unwrap().is_some());
        assert!(resolver
            .resolve(FileKind::Format, &LogicalFileName::new("external.fmt"))
            .unwrap()
            .is_none());
        assert_eq!(fake.invocation_count(), 0);
    }

    #[test]
    #[ignore = "PRATEX_REAL_LSR で明示した TeX Live database の手動照合"]
    fn 指定した実物databaseをresolverの探索順で引ける() {
        let database_path = std::env::var_os("PRATEX_REAL_LSR")
            .map(PathBuf::from)
            .expect("PRATEX_REAL_LSR が必要");
        let root = database_path.parent().unwrap().to_path_buf();
        let options = ResolverOptions::default()
            .with_filename_database_search(FilenameDatabaseSearch::Explicit(vec![database_path]));
        let fake = FakeExecutor::with_responses([search_path_success([database_only_recursive(
            &root.join("tex"),
        )])]);
        let mut resolver = KpsewhichResolver::new(options, fake.clone());

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new("latex/base/latex.ltx"))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.source(), ResolutionSource::FilenameDatabase);
        assert!(fs::metadata(resolved.physical_path()).unwrap().is_file());
        assert_eq!(fake.invocation_count(), 1);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "PRATEX_TEST_WSL=1 の実環境でだけ WSL TeX Live を照合"]
    fn 既定resolverがwsl_tex_liveのtfmをwindowsから開ける() {
        assert_eq!(std::env::var("PRATEX_TEST_WSL").as_deref(), Ok("1"));
        let mut resolver = KpsewhichResolver::default();
        let resolved = resolver
            .resolve(FileKind::Tfm, &LogicalFileName::new("cmr10.tfm"))
            .unwrap()
            .unwrap();

        assert!(matches!(
            resolved.source(),
            ResolutionSource::FilenameDatabase | ResolutionSource::WslKpsewhich
        ));
        assert!(fs::File::open(resolved.physical_path()).is_ok());
    }

    #[test]
    #[ignore = "PRATEX_TEST_TEXLIVE=1 の実環境でだけ配布JFM/VFを照合"]
    fn tex_liveのjfmとvfを用途別に解決できる() {
        assert_eq!(std::env::var("PRATEX_TEST_TEXLIVE").as_deref(), Ok("1"));
        let mut resolver = KpsewhichResolver::default();

        for (kind, name) in [
            (FileKind::Tfm, "upjisr-h.tfm"),
            (FileKind::Vf, "upjisr-h.vf"),
            (FileKind::Tfm, "upjisg-h.tfm"),
            (FileKind::Vf, "upjisg-h.vf"),
        ] {
            let logical = LogicalFileName::new(name);
            let first = resolver.resolve(kind, &logical).unwrap().unwrap();
            let second = resolver.resolve(kind, &logical).unwrap().unwrap();
            assert_eq!(first.logical_name(), &logical);
            assert_eq!(first.physical_path(), second.physical_path());
            assert!(matches!(
                first.source(),
                ResolutionSource::FilenameDatabase
                    | ResolutionSource::Kpsewhich
                    | ResolutionSource::WslKpsewhich
            ));
            assert!(fs::File::open(first.physical_path()).is_ok());
        }
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

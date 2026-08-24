//! Run-localなin-process Kpathseaとsafe resolverを一つの決定点で重ねる。

use super::{
    resolve_local_boundary, CommandExecutor, FileKind, FileResolver, KpsewhichResolver,
    LocalBoundary, LogicalFileName, ProcessCommandExecutor, ResolutionSource, ResolveError,
    ResolvedFile,
};
use std::fmt;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FastPathFallback {
    Unavailable,
    UnsupportedPathEncoding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FastPathFailure {
    InteriorNul,
    InitializationFailed,
    Unexpected,
}

impl fmt::Display for FastPathFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InteriorNul => "a logical name contains an interior NUL byte",
            Self::InitializationFailed => "the linked library could not initialize",
            Self::Unexpected => "the linked adapter returned an unknown error",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum FastPathLookup {
    Found(PathBuf),
    Missing,
    UseSafeResolver(FastPathFallback),
    Failed(FastPathFailure),
}

pub(super) trait FastFileResolver {
    fn resolve(&mut self, kind: FileKind, logical_name: &LogicalFileName) -> FastPathLookup;
}

/// Kpathseaの公開format値をPraTeXの用途domainへ一箇所で対応づける。
///
/// 値は`kpse_file_format_type`の公開ABIであり、vendored Rust wrapperも同じ定数を
/// linked testで照合する。consumerへ生の整数を散らさないため、この型からだけ渡す。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
enum KpathseaFormat {
    Tfm = 3,
    Afm = 4,
    Fmt = 10,
    FontMap = 11,
    Tex = 26,
    Type1 = 32,
    Vf = 33,
    ProgramText = 39,
    Enc = 44,
}

impl KpathseaFormat {
    fn value(self) -> u32 {
        self as u32
    }
}

fn format_for_kind(kind: FileKind) -> KpathseaFormat {
    match kind {
        FileKind::Tex => KpathseaFormat::Tex,
        FileKind::Format => KpathseaFormat::Fmt,
        FileKind::Tfm => KpathseaFormat::Tfm,
        FileKind::Vf => KpathseaFormat::Vf,
        FileKind::FontMap => KpathseaFormat::FontMap,
        FileKind::Encoding => KpathseaFormat::Enc,
        FileKind::Type1 => KpathseaFormat::Type1,
        FileKind::Afm => KpathseaFormat::Afm,
        FileKind::Vaak | FileKind::PdfData => KpathseaFormat::ProgramText,
    }
}

/// 一TeX runにつき一個だけ所有するlinked Kpathsea handle。
///
/// 監査対象はLinuxのsystem library境界だけである。このcheckpointでは依存自体を
/// Linuxへ閉じ、Windows、WASM、その他Unixは明示的にsafe resolverへ戻す。
pub(super) struct KpathseaFastPath {
    #[cfg(all(feature = "system-kpathsea", target_os = "linux"))]
    state: NativeState,
}

#[cfg(all(feature = "system-kpathsea", target_os = "linux"))]
enum NativeState {
    Ready(kpathsea::Kpaths),
    UseSafeResolver(FastPathFallback),
    Failed(FastPathFailure),
}

impl KpathseaFastPath {
    fn new() -> Self {
        #[cfg(all(feature = "system-kpathsea", target_os = "linux"))]
        {
            let state = match kpathsea::Kpaths::new_in_process_with_program_name("pratex") {
                Ok(kpaths) => NativeState::Ready(kpaths),
                Err(error) => match classify_path_error(error) {
                    FastPathLookup::UseSafeResolver(reason) => NativeState::UseSafeResolver(reason),
                    FastPathLookup::Failed(reason) => NativeState::Failed(reason),
                    FastPathLookup::Found(_) | FastPathLookup::Missing => unreachable!(),
                },
            };
            return Self { state };
        }

        #[cfg(not(all(feature = "system-kpathsea", target_os = "linux")))]
        Self {}
    }
}

impl Default for KpathseaFastPath {
    fn default() -> Self {
        Self::new()
    }
}

impl FastFileResolver for KpathseaFastPath {
    fn resolve(&mut self, kind: FileKind, logical_name: &LogicalFileName) -> FastPathLookup {
        #[cfg(all(feature = "system-kpathsea", target_os = "linux"))]
        {
            return match &self.state {
                NativeState::Ready(kpaths) => match kpaths.find_file_path_with_format(
                    logical_name.as_os_str(),
                    format_for_kind(kind).value(),
                    true,
                ) {
                    Ok(Some(path)) => FastPathLookup::Found(path),
                    Ok(None) => FastPathLookup::Missing,
                    Err(error) => classify_path_error(error),
                },
                NativeState::UseSafeResolver(reason) => FastPathLookup::UseSafeResolver(*reason),
                NativeState::Failed(reason) => FastPathLookup::Failed(*reason),
            };
        }

        #[cfg(not(all(feature = "system-kpathsea", target_os = "linux")))]
        {
            let _ = (kind, logical_name);
            FastPathLookup::UseSafeResolver(FastPathFallback::Unavailable)
        }
    }
}

#[cfg(all(feature = "system-kpathsea", target_os = "linux"))]
fn classify_path_error(error: kpathsea::PathError) -> FastPathLookup {
    match error {
        kpathsea::PathError::InProcessUnavailable => {
            FastPathLookup::UseSafeResolver(FastPathFallback::Unavailable)
        }
        kpathsea::PathError::UnsupportedPathEncoding => {
            FastPathLookup::UseSafeResolver(FastPathFallback::UnsupportedPathEncoding)
        }
        kpathsea::PathError::InteriorNul => FastPathLookup::Failed(FastPathFailure::InteriorNul),
        kpathsea::PathError::InitializationFailed => {
            FastPathLookup::Failed(FastPathFailure::InitializationFailed)
        }
        _ => FastPathLookup::Failed(FastPathFailure::Unexpected),
    }
}

/// direct pathとLocalOnlyを先に確定し、外部探索だけをfast/safeへ振り分ける。
pub(super) struct NativeFirstResolver<F = KpathseaFastPath, E = ProcessCommandExecutor> {
    fast: F,
    safe: KpsewhichResolver<E>,
}

impl Default for NativeFirstResolver<KpathseaFastPath, ProcessCommandExecutor> {
    fn default() -> Self {
        Self::new(KpathseaFastPath::default(), KpsewhichResolver::default())
    }
}

impl<F, E> NativeFirstResolver<F, E> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn new(fast: F, safe: KpsewhichResolver<E>) -> Self {
        Self { fast, safe }
    }
}

impl<F: FastFileResolver, E: CommandExecutor> FileResolver for NativeFirstResolver<F, E> {
    fn resolve(
        &mut self,
        kind: FileKind,
        logical_name: &LogicalFileName,
    ) -> Result<Option<ResolvedFile>, ResolveError> {
        let query = match resolve_local_boundary(
            kind,
            logical_name,
            self.safe.options.external_format_search,
        )? {
            LocalBoundary::Resolved(resolved) => return Ok(resolved),
            LocalBoundary::External(query) => query,
        };

        // `--engine=rtex`は旧fmt形式を意図的に隔離するsafe CLI固有の契約であり、
        // program名`pratex`だけのKpathsへ読み替えない。
        if query.kind == FileKind::Format {
            return self.safe.resolve_external(query);
        }

        match self.fast.resolve(query.kind, &query.logical_name) {
            FastPathLookup::Found(physical_path) => match fs::metadata(&physical_path) {
                Ok(metadata) if metadata.is_file() => Ok(Some(ResolvedFile {
                    logical_name: query.logical_name,
                    physical_path,
                    source: ResolutionSource::InProcessKpathsea,
                })),
                Ok(_) => Err(ResolveError::InProcessPathNotFile {
                    path: physical_path,
                }),
                Err(source) => Err(ResolveError::InspectInProcessPath {
                    path: physical_path,
                    source,
                }),
            },
            FastPathLookup::Missing => Ok(None),
            FastPathLookup::UseSafeResolver(_reason) => self.safe.resolve_external(query),
            FastPathLookup::Failed(reason) => Err(ResolveError::InProcessKpathsea(reason)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_search::{CommandOutput, ExternalFormatSearch, ResolverOptions};
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::io;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Clone)]
    struct FakeFastPath {
        replies: Rc<RefCell<VecDeque<FastPathLookup>>>,
        calls: Rc<Cell<usize>>,
    }

    impl FakeFastPath {
        fn new(replies: impl IntoIterator<Item = FastPathLookup>) -> Self {
            Self {
                replies: Rc::new(RefCell::new(replies.into_iter().collect())),
                calls: Rc::new(Cell::new(0)),
            }
        }
    }

    impl FastFileResolver for FakeFastPath {
        fn resolve(&mut self, _kind: FileKind, _logical_name: &LogicalFileName) -> FastPathLookup {
            self.calls.set(self.calls.get() + 1);
            self.replies.borrow_mut().pop_front().unwrap()
        }
    }

    #[derive(Clone)]
    struct FakeSafeExecutor {
        replies: Rc<RefCell<VecDeque<io::Result<CommandOutput>>>>,
        calls: Rc<Cell<usize>>,
    }

    impl FakeSafeExecutor {
        fn new(replies: impl IntoIterator<Item = io::Result<CommandOutput>>) -> Self {
            Self {
                replies: Rc::new(RefCell::new(replies.into_iter().collect())),
                calls: Rc::new(Cell::new(0)),
            }
        }
    }

    impl CommandExecutor for FakeSafeExecutor {
        fn execute(
            &mut self,
            _program: &OsStr,
            _arguments: &[OsString],
        ) -> io::Result<CommandOutput> {
            self.calls.set(self.calls.get() + 1);
            self.replies.borrow_mut().pop_front().unwrap()
        }
    }

    fn safe_success(path: &str) -> io::Result<CommandOutput> {
        Ok(CommandOutput {
            code: Some(0),
            stdout: format!("{path}\n").into_bytes(),
            stderr: Vec::new(),
        })
    }

    fn resolver(
        fast: FakeFastPath,
        safe: FakeSafeExecutor,
    ) -> NativeFirstResolver<FakeFastPath, FakeSafeExecutor> {
        NativeFirstResolver::new(
            fast,
            KpsewhichResolver::new(ResolverOptions::default(), safe),
        )
    }

    fn unique_name(suffix: &str) -> String {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        format!(
            "pratex-kpathsea-{}-{}-{suffix}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    }

    struct TempFile {
        directory: PathBuf,
        path: PathBuf,
    }

    impl TempFile {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir().join(unique_name("fast-hit"));
            fs::create_dir_all(&directory).unwrap();
            let path = directory.join(name);
            fs::write(&path, b"fast").unwrap();
            Self { directory, path }
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.directory).unwrap();
        }
    }

    #[test]
    fn linked_hitではsafe_resolverを呼ばない() {
        let hit = TempFile::new("hit.tex");
        let fast = FakeFastPath::new([FastPathLookup::Found(hit.path.clone())]);
        let safe = FakeSafeExecutor::new([]);
        let fast_calls = fast.calls.clone();
        let safe_calls = safe.calls.clone();
        let mut resolver = resolver(fast, safe);

        let resolved = resolver
            .resolve(FileKind::Tex, &LogicalFileName::new(unique_name("hit.tex")))
            .unwrap()
            .unwrap();

        assert_eq!(resolved.physical_path(), hit.path);
        assert_eq!(resolved.source(), ResolutionSource::InProcessKpathsea);
        assert_eq!(fast_calls.get(), 1);
        assert_eq!(safe_calls.get(), 0);
    }

    #[test]
    fn linked_missはauthoritativeでsafe_resolverへ戻さない() {
        let fast = FakeFastPath::new([FastPathLookup::Missing]);
        let safe = FakeSafeExecutor::new([safe_success("/must/not/be/used.tex")]);
        let safe_calls = safe.calls.clone();
        let mut resolver = resolver(fast, safe);

        assert!(resolver
            .resolve(
                FileKind::Tex,
                &LogicalFileName::new(unique_name("missing.tex"))
            )
            .unwrap()
            .is_none());
        assert_eq!(safe_calls.get(), 0);
    }

    #[test]
    fn unavailableとencodingだけsafe_resolverへ戻す() {
        for (index, reason) in [
            FastPathFallback::Unavailable,
            FastPathFallback::UnsupportedPathEncoding,
        ]
        .into_iter()
        .enumerate()
        {
            let fast = FakeFastPath::new([FastPathLookup::UseSafeResolver(reason)]);
            let safe = FakeSafeExecutor::new([safe_success(&format!("/safe/{index}.tex"))]);
            let safe_calls = safe.calls.clone();
            let mut resolver = resolver(fast, safe);

            let resolved = resolver
                .resolve(
                    FileKind::Tex,
                    &LogicalFileName::new(unique_name("fallback.tex")),
                )
                .unwrap()
                .unwrap();
            assert_eq!(resolved.source(), ResolutionSource::Kpsewhich);
            assert_eq!(safe_calls.get(), 1);
        }
    }

    #[test]
    fn fast_pathの意味エラーをfallbackで隠さない() {
        let fast = FakeFastPath::new([FastPathLookup::Failed(FastPathFailure::InteriorNul)]);
        let safe = FakeSafeExecutor::new([safe_success("/must/not/be/used.tex")]);
        let safe_calls = safe.calls.clone();
        let mut resolver = resolver(fast, safe);

        let error = resolver
            .resolve(
                FileKind::Tex,
                &LogicalFileName::new(unique_name("fatal.tex")),
            )
            .unwrap_err();
        assert!(matches!(error, ResolveError::InProcessKpathsea(_)));
        assert_eq!(safe_calls.get(), 0);
    }

    #[test]
    fn linked_hitが通常fileでなければ受理しない() {
        let directory = std::env::temp_dir().join(unique_name("fast-directory"));
        fs::create_dir_all(&directory).unwrap();
        let fast = FakeFastPath::new([FastPathLookup::Found(directory.clone())]);
        let safe = FakeSafeExecutor::new([]);
        let mut resolver = resolver(fast, safe);

        let error = resolver
            .resolve(
                FileKind::Tex,
                &LogicalFileName::new(unique_name("directory.tex")),
            )
            .unwrap_err();
        assert!(matches!(error, ResolveError::InProcessPathNotFile { .. }));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn direct_pathとlocal_only_fmtはfast_pathより先に確定する() {
        let directory = std::env::temp_dir().join(unique_name("direct"));
        fs::create_dir_all(&directory).unwrap();
        let direct_path = directory.join("direct.tex");
        fs::write(&direct_path, b"direct").unwrap();

        let fast = FakeFastPath::new([]);
        let safe = FakeSafeExecutor::new([]);
        let fast_calls = fast.calls.clone();
        let safe_calls = safe.calls.clone();
        let mut resolver = resolver(fast, safe);

        let direct = resolver
            .resolve(
                FileKind::Tex,
                &LogicalFileName::new(direct_path.as_os_str()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(direct.source(), ResolutionSource::DirectPath);
        assert!(resolver
            .resolve(
                FileKind::Format,
                &LogicalFileName::new(unique_name("external.fmt")),
            )
            .unwrap()
            .is_none());
        assert_eq!(fast_calls.get(), 0);
        assert_eq!(safe_calls.get(), 0);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn 全file_kindを公開kpathsea_formatへ対応づける() {
        assert_eq!(format_for_kind(FileKind::Tex).value(), 26);
        assert_eq!(format_for_kind(FileKind::Format).value(), 10);
        assert_eq!(format_for_kind(FileKind::Tfm).value(), 3);
        assert_eq!(format_for_kind(FileKind::Vf).value(), 33);
        assert_eq!(format_for_kind(FileKind::FontMap).value(), 11);
        assert_eq!(format_for_kind(FileKind::Encoding).value(), 44);
        assert_eq!(format_for_kind(FileKind::Type1).value(), 32);
        assert_eq!(format_for_kind(FileKind::Afm).value(), 4);
        assert_eq!(format_for_kind(FileKind::Vaak).value(), 39);
        assert_eq!(format_for_kind(FileKind::PdfData).value(), 39);
    }

    #[test]
    fn external_fmtはengine契約を保つsafe_resolverへ渡す() {
        let fast = FakeFastPath::new([]);
        let safe = FakeSafeExecutor::new([safe_success("/safe/external.fmt")]);
        let fast_calls = fast.calls.clone();
        let safe_calls = safe.calls.clone();
        let options = ResolverOptions::default()
            .with_external_format_search(ExternalFormatSearch::KpsewhichRtexEngine);
        let mut resolver = NativeFirstResolver::new(fast, KpsewhichResolver::new(options, safe));

        let resolved = resolver
            .resolve(
                FileKind::Format,
                &LogicalFileName::new(unique_name("external-enabled.fmt")),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            resolved.physical_path(),
            PathBuf::from("/safe/external.fmt")
        );
        assert_eq!(resolved.source(), ResolutionSource::Kpsewhich);
        assert_eq!(fast_calls.get(), 0);
        assert_eq!(safe_calls.get(), 1);
    }
}

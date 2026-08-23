//! Type 1 PDF font の論理資材を、一回だけ読んだ map から安全に組み立てる。
//!
//! この層は PostScript や map special を実行しない。探索には常に論理名と
//! `FileKind` を渡し、解決後の物理 path は bounded read にだけ用いる。

use super::afm::{AfmFont, AfmNumber, AfmParseError};
use super::encoding::{EncodingError, EncodingVector};
use super::map::{parse_map, EmbedPolicy, MapEntry, MapParseError, MapResource, ResourceMarker};
use super::type1::{
    extract_private_std_vw, parse_pfb, PfbError, Type1FontProgram, Type1MetadataError,
};
use crate::file_search::{FileKind, FileResolver, LogicalFileName, ResolveError, ResolvedFile};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub(crate) const DEFAULT_FONT_MAP: &str = "pdftex.map";
/// pdfTeX manualで、mapのfontflags省略時に定義されているSymbolic既定値。
pub(crate) const PDFTEX_DEFAULT_FONT_FLAGS: u32 = 4;

// 実在する TeX Live 資材より十分大きく、壊れた入力を無制限に複製しない上限。
pub(crate) const MAX_FONT_MAP_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_TYPE1_FILE_BYTES: usize = 128 * 1024 * 1024;
pub(crate) const MAX_AFM_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_ENCODING_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResourceLimits {
    font_map: usize,
    type1: usize,
    afm: usize,
    encoding: usize,
}

impl ResourceLimits {
    const STANDARD: Self = Self {
        font_map: MAX_FONT_MAP_BYTES,
        type1: MAX_TYPE1_FILE_BYTES,
        afm: MAX_AFM_BYTES,
        encoding: MAX_ENCODING_BYTES,
    };

    fn for_kind(self, kind: FileKind) -> Option<usize> {
        match kind {
            FileKind::FontMap => Some(self.font_map),
            FileKind::Type1 => Some(self.type1),
            FileKind::Afm => Some(self.afm),
            FileKind::Encoding => Some(self.encoding),
            _ => None,
        }
    }
}

/// parser の所有結果と、その探索名・実ファイルを一緒に保持する。
#[derive(Debug)]
pub(crate) struct LoadedResource<T> {
    pub(crate) resolved: ResolvedFile,
    pub(crate) value: T,
}

impl<T> LoadedResource<T> {
    pub(crate) fn logical_name(&self) -> &LogicalFileName {
        self.resolved.logical_name()
    }

    pub(crate) fn physical_path(&self) -> &Path {
        self.resolved.physical_path()
    }

    pub(crate) fn value(&self) -> &T {
        &self.value
    }

    pub(crate) fn into_parts(self) -> (ResolvedFile, T) {
        (self.resolved, self.value)
    }
}

/// 一つの map entry から解決・検査を終えた Type 1 font 資材。
#[derive(Debug)]
pub(crate) struct LoadedType1Font {
    pub(crate) tfm_name: String,
    /// map に PostScript 名が書かれていたかを失わない。
    pub(crate) declared_postscript_name: Option<String>,
    /// map 指定があれば検査済みの値、なければ AFM の FontName。
    pub(crate) postscript_name: String,
    /// mapにfontflagsが明記されていたかを、既定値と分けて保持する。
    pub(crate) declared_font_flags: Option<u32>,
    /// map明記値、またはpdfTeX map契約の既定値4。
    pub(crate) descriptor_flags: u32,
    /// AFMが`StdVW`を省略した場合だけ、Type 1 Private辞書から得た値。
    pub(crate) private_std_vw: Option<AfmNumber>,
    pub(crate) embedding: EmbedPolicy,
    pub(crate) font_program: LoadedResource<Type1FontProgram>,
    pub(crate) metrics: LoadedResource<AfmFont>,
    pub(crate) encoding: Option<LoadedResource<EncodingVector>>,
}

/// PDF backend が論理 TFM 名から Type 1 資材を一度だけ得るための境界。
///
/// generic な resolver は map/resource 読み込み側だけに閉じ込め、backend は font 定義時に
/// この object-safe trait を一度呼ぶ。node の描画loopへ dynamic dispatchを持ち込まない。
pub(crate) trait Type1ResourceLoader {
    fn load(&mut self, tfm_name: &str) -> Result<LoadedType1Font, FontResourceError>;
}

/// 一実行中に共有する map と resolver。
///
/// map は constructor で一回だけ解決・読み込み・parse される。個々の font を
/// 読むたびに map を再探索しない。
pub(crate) struct FontResourceLoader<R> {
    resolver: R,
    map: LoadedResource<BTreeMap<String, MapEntry>>,
    limits: ResourceLimits,
}

impl<R: FileResolver> FontResourceLoader<R> {
    pub(crate) fn with_default_map(resolver: R) -> Result<Self, FontResourceError> {
        Self::with_map(resolver, LogicalFileName::new(DEFAULT_FONT_MAP))
    }

    pub(crate) fn with_map(
        resolver: R,
        map_name: LogicalFileName,
    ) -> Result<Self, FontResourceError> {
        Self::with_map_and_limits(resolver, map_name, ResourceLimits::STANDARD)
    }

    fn with_map_and_limits(
        mut resolver: R,
        map_name: LogicalFileName,
        limits: ResourceLimits,
    ) -> Result<Self, FontResourceError> {
        let resolved = resolve_required(&mut resolver, FileKind::FontMap, &map_name)?;
        let bytes = read_bounded(
            &resolved,
            FileKind::FontMap,
            required_limit(limits, FileKind::FontMap)?,
        )?;
        let entries = parse_map(&bytes).map_err(|source| FontResourceError::MapParse {
            resource: resolved.clone(),
            source,
        })?;
        let mut indexed_entries = BTreeMap::new();
        for entry in entries {
            let tfm_name = entry.tfm_name.clone();
            if indexed_entries.contains_key(&tfm_name) {
                return Err(FontResourceError::DuplicateTfmEntry {
                    map: resolved,
                    tfm_name,
                });
            }
            indexed_entries.insert(tfm_name, entry);
        }

        Ok(Self {
            resolver,
            map: LoadedResource {
                resolved,
                value: indexed_entries,
            },
            limits,
        })
    }

    pub(crate) fn map_file(&self) -> &ResolvedFile {
        &self.map.resolved
    }

    pub(crate) fn load(&mut self, tfm_name: &str) -> Result<LoadedType1Font, FontResourceError> {
        let entry = self.map.value.get(tfm_name).cloned().ok_or_else(|| {
            FontResourceError::TfmEntryNotFound {
                map: self.map.resolved.clone(),
                tfm_name: tfm_name.to_owned(),
            }
        })?;

        let selected = select_entry_resources(&entry)?;
        let embedding = selected.embedding;
        let font_file = selected.font_file;
        let font_logical_name = LogicalFileName::new(font_file.name.as_str());
        let afm_logical_name = afm_name_from_font_file(&entry.tfm_name, &font_file.name)?;

        let font_resolved =
            resolve_required(&mut self.resolver, FileKind::Type1, &font_logical_name)?;
        let font_bytes = read_bounded(
            &font_resolved,
            FileKind::Type1,
            required_limit(self.limits, FileKind::Type1)?,
        )?;
        let font_program =
            parse_pfb(&font_bytes).map_err(|source| FontResourceError::PfbParse {
                resource: font_resolved.clone(),
                source,
            })?;
        let font_program = LoadedResource {
            resolved: font_resolved,
            value: font_program,
        };

        let afm_resolved = resolve_required(&mut self.resolver, FileKind::Afm, &afm_logical_name)?;
        let afm_bytes = read_bounded(
            &afm_resolved,
            FileKind::Afm,
            required_limit(self.limits, FileKind::Afm)?,
        )?;
        let metrics = AfmFont::parse(&afm_bytes).map_err(|source| FontResourceError::AfmParse {
            resource: afm_resolved.clone(),
            source,
        })?;
        let afm_postscript_name = metrics.descriptor.font_name.clone();
        if let Some(map_postscript_name) = &entry.postscript_name {
            if map_postscript_name != &afm_postscript_name {
                return Err(FontResourceError::PostScriptNameMismatch {
                    tfm_name: entry.tfm_name,
                    map_name: map_postscript_name.clone(),
                    afm_name: afm_postscript_name,
                    afm: afm_resolved,
                });
            }
        }
        let private_std_vw = if metrics.descriptor.std_vw.is_none() {
            extract_private_std_vw(font_program.value()).map_err(|source| {
                FontResourceError::Type1Metadata {
                    resource: font_program.resolved.clone(),
                    source,
                }
            })?
        } else {
            None
        };
        let metrics = LoadedResource {
            resolved: afm_resolved,
            value: metrics,
        };

        let encoding = if let Some(encoding_file) = selected.encoding_file {
            let logical_name = LogicalFileName::new(encoding_file.name.as_str());
            let resolved = resolve_required(&mut self.resolver, FileKind::Encoding, &logical_name)?;
            let bytes = read_bounded(
                &resolved,
                FileKind::Encoding,
                required_limit(self.limits, FileKind::Encoding)?,
            )?;
            let value = EncodingVector::parse(&bytes).map_err(|source| {
                FontResourceError::EncodingParse {
                    resource: resolved.clone(),
                    source,
                }
            })?;
            Some(LoadedResource { resolved, value })
        } else {
            None
        };

        let postscript_name = entry.postscript_name.clone().unwrap_or(afm_postscript_name);
        Ok(LoadedType1Font {
            tfm_name: entry.tfm_name,
            declared_postscript_name: entry.postscript_name,
            postscript_name,
            declared_font_flags: entry.font_flags,
            descriptor_flags: entry.font_flags.unwrap_or(PDFTEX_DEFAULT_FONT_FLAGS),
            private_std_vw,
            embedding,
            font_program,
            metrics,
            encoding,
        })
    }

    pub(crate) fn into_resolver(self) -> R {
        self.resolver
    }
}

impl<R: FileResolver> Type1ResourceLoader for FontResourceLoader<R> {
    fn load(&mut self, tfm_name: &str) -> Result<LoadedType1Font, FontResourceError> {
        FontResourceLoader::load(self, tfm_name)
    }
}

struct SelectedEntryResources {
    font_file: MapResource,
    embedding: EmbedPolicy,
    encoding_file: Option<MapResource>,
}

fn select_entry_resources(entry: &MapEntry) -> Result<SelectedEntryResources, FontResourceError> {
    if let Some(special) = &entry.special {
        return Err(FontResourceError::UnsupportedMapSpecial {
            tfm_name: entry.tfm_name.clone(),
            raw: special.raw.clone(),
            mentions_slant_font: special.mentions_slant_font,
            mentions_extend_font: special.mentions_extend_font,
        });
    }

    let mut font_file: Option<MapResource> = None;
    let mut encoding_file: Option<MapResource> = None;
    for resource in &entry.resources {
        let is_encoding = resource.marker == ResourceMarker::BracketedEncoding
            || extension_is(&resource.name, "enc");
        if is_encoding {
            if resource.marker == ResourceMarker::Full {
                return Err(FontResourceError::FullEmbeddingEncoding {
                    tfm_name: entry.tfm_name.clone(),
                    logical_name: resource.name.clone(),
                });
            }
            if let Some(first) = &encoding_file {
                return Err(FontResourceError::MultipleEncodingFiles {
                    tfm_name: entry.tfm_name.clone(),
                    first: first.name.clone(),
                    duplicate: resource.name.clone(),
                });
            }
            encoding_file = Some(resource.clone());
            continue;
        }

        if !extension_is(&resource.name, "pfb") {
            return Err(FontResourceError::UnsupportedResourceType {
                tfm_name: entry.tfm_name.clone(),
                logical_name: resource.name.clone(),
                expected_extension: "pfb",
            });
        }
        if let Some(first) = &font_file {
            return Err(FontResourceError::MultipleType1FontFiles {
                tfm_name: entry.tfm_name.clone(),
                first: first.name.clone(),
                duplicate: resource.name.clone(),
            });
        }
        font_file = Some(resource.clone());
    }

    let font_file = font_file.ok_or_else(|| FontResourceError::MissingFontFile {
        tfm_name: entry.tfm_name.clone(),
    })?;
    let embedding = match font_file.marker {
        ResourceMarker::Subset => EmbedPolicy::Subset,
        ResourceMarker::Full => EmbedPolicy::Full,
        ResourceMarker::BracketedEncoding => {
            return Err(FontResourceError::UnsupportedResourceType {
                tfm_name: entry.tfm_name.clone(),
                logical_name: font_file.name,
                expected_extension: "enc",
            })
        }
    };
    Ok(SelectedEntryResources {
        font_file,
        embedding,
        encoding_file,
    })
}

fn extension_is(logical_name: &str, expected: &str) -> bool {
    Path::new(logical_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn afm_name_from_font_file(
    tfm_name: &str,
    font_file_name: &str,
) -> Result<LogicalFileName, FontResourceError> {
    let stem = Path::new(font_file_name)
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| FontResourceError::InvalidFontFileName {
            tfm_name: tfm_name.to_owned(),
            logical_name: font_file_name.to_owned(),
        })?;
    let mut afm_name = OsString::from(stem);
    afm_name.push(".afm");
    Ok(LogicalFileName::new(afm_name))
}

fn required_limit(limits: ResourceLimits, kind: FileKind) -> Result<usize, FontResourceError> {
    limits
        .for_kind(kind)
        .ok_or(FontResourceError::MissingSizeLimit { kind })
}

fn resolve_required<R: FileResolver>(
    resolver: &mut R,
    kind: FileKind,
    logical_name: &LogicalFileName,
) -> Result<ResolvedFile, FontResourceError> {
    resolver
        .resolve(kind, logical_name)
        .map_err(|source| FontResourceError::Resolve {
            kind,
            logical_name: logical_name.clone(),
            source,
        })?
        .ok_or_else(|| FontResourceError::ResourceNotFound {
            kind,
            logical_name: logical_name.clone(),
        })
}

/// metadata の長さを信用せず、上限より一 byte だけ多く読むことで超過を検出する。
fn read_bounded(
    resource: &ResolvedFile,
    kind: FileKind,
    limit: usize,
) -> Result<Vec<u8>, FontResourceError> {
    let sentinel_limit = limit
        .checked_add(1)
        .ok_or(FontResourceError::InvalidSizeLimit { kind, limit })?;
    let sentinel_limit = u64::try_from(sentinel_limit)
        .map_err(|_| FontResourceError::InvalidSizeLimit { kind, limit })?;
    let file = File::open(resource.physical_path()).map_err(|source| FontResourceError::Io {
        operation: ResourceIoOperation::Open,
        kind,
        resource: resource.clone(),
        source,
    })?;
    let initial_capacity = limit.min(64 * 1024).saturating_add(1);
    let mut bytes = Vec::with_capacity(initial_capacity);
    file.take(sentinel_limit)
        .read_to_end(&mut bytes)
        .map_err(|source| FontResourceError::Io {
            operation: ResourceIoOperation::Read,
            kind,
            resource: resource.clone(),
            source,
        })?;
    if bytes.len() > limit {
        return Err(FontResourceError::ResourceTooLarge {
            kind,
            resource: resource.clone(),
            limit,
            observed_at_least: bytes.len(),
        });
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceIoOperation {
    Open,
    Read,
}

#[derive(Debug)]
pub(crate) enum FontResourceError {
    Resolve {
        kind: FileKind,
        logical_name: LogicalFileName,
        source: ResolveError,
    },
    ResourceNotFound {
        kind: FileKind,
        logical_name: LogicalFileName,
    },
    Io {
        operation: ResourceIoOperation,
        kind: FileKind,
        resource: ResolvedFile,
        source: io::Error,
    },
    InvalidSizeLimit {
        kind: FileKind,
        limit: usize,
    },
    MissingSizeLimit {
        kind: FileKind,
    },
    ResourceTooLarge {
        kind: FileKind,
        resource: ResolvedFile,
        limit: usize,
        observed_at_least: usize,
    },
    MapParse {
        resource: ResolvedFile,
        source: MapParseError,
    },
    DuplicateTfmEntry {
        map: ResolvedFile,
        tfm_name: String,
    },
    TfmEntryNotFound {
        map: ResolvedFile,
        tfm_name: String,
    },
    UnsupportedMapSpecial {
        tfm_name: String,
        raw: String,
        mentions_slant_font: bool,
        mentions_extend_font: bool,
    },
    MissingFontFile {
        tfm_name: String,
    },
    MultipleType1FontFiles {
        tfm_name: String,
        first: String,
        duplicate: String,
    },
    MultipleEncodingFiles {
        tfm_name: String,
        first: String,
        duplicate: String,
    },
    FullEmbeddingEncoding {
        tfm_name: String,
        logical_name: String,
    },
    UnsupportedResourceType {
        tfm_name: String,
        logical_name: String,
        expected_extension: &'static str,
    },
    InvalidFontFileName {
        tfm_name: String,
        logical_name: String,
    },
    PfbParse {
        resource: ResolvedFile,
        source: PfbError,
    },
    Type1Metadata {
        resource: ResolvedFile,
        source: Type1MetadataError,
    },
    AfmParse {
        resource: ResolvedFile,
        source: AfmParseError,
    },
    EncodingParse {
        resource: ResolvedFile,
        source: EncodingError,
    },
    PostScriptNameMismatch {
        tfm_name: String,
        map_name: String,
        afm_name: String,
        afm: ResolvedFile,
    },
}

impl fmt::Display for FontResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolve {
                kind,
                logical_name,
                source,
            } => write!(
                formatter,
                "cannot resolve {} `{}`: {source}",
                file_kind_name(*kind),
                logical_name.as_os_str().to_string_lossy()
            ),
            Self::ResourceNotFound { kind, logical_name } => write!(
                formatter,
                "{} `{}` was not found",
                file_kind_name(*kind),
                logical_name.as_os_str().to_string_lossy()
            ),
            Self::Io {
                operation,
                kind,
                resource,
                source,
            } => write!(
                formatter,
                "cannot {} {} `{}` at `{}`: {source}",
                operation_name(*operation),
                file_kind_name(*kind),
                resource.logical_name().as_os_str().to_string_lossy(),
                resource.physical_path().display()
            ),
            Self::InvalidSizeLimit { kind, limit } => write!(
                formatter,
                "{} byte limit {limit} cannot be represented for bounded reading",
                file_kind_name(*kind)
            ),
            Self::MissingSizeLimit { kind } => {
                write!(formatter, "{} has no bounded-read limit", file_kind_name(*kind))
            }
            Self::ResourceTooLarge {
                kind,
                resource,
                limit,
                observed_at_least,
            } => write!(
                formatter,
                "{} `{}` at `{}` is at least {observed_at_least} bytes, above the {limit}-byte limit",
                file_kind_name(*kind),
                resource.logical_name().as_os_str().to_string_lossy(),
                resource.physical_path().display()
            ),
            Self::MapParse { resource, source } => write!(
                formatter,
                "cannot parse font map `{}` at `{}`: {source}",
                resource.logical_name().as_os_str().to_string_lossy(),
                resource.physical_path().display()
            ),
            Self::DuplicateTfmEntry { map, tfm_name } => write!(
                formatter,
                "font map `{}` contains more than one entry for TFM `{tfm_name}`",
                map.logical_name().as_os_str().to_string_lossy()
            ),
            Self::TfmEntryNotFound { map, tfm_name } => write!(
                formatter,
                "font map `{}` has no entry for TFM `{tfm_name}`",
                map.logical_name().as_os_str().to_string_lossy()
            ),
            Self::UnsupportedMapSpecial {
                tfm_name,
                raw,
                mentions_slant_font,
                mentions_extend_font,
            } => write!(
                formatter,
                "TFM `{tfm_name}` uses an unsupported map special (SlantFont={mentions_slant_font}, ExtendFont={mentions_extend_font}): `{raw}`"
            ),
            Self::MissingFontFile { tfm_name } => {
                write!(formatter, "TFM `{tfm_name}` has no embedded font file")
            }
            Self::MultipleType1FontFiles {
                tfm_name,
                first,
                duplicate,
            } => write!(
                formatter,
                "TFM `{tfm_name}` names more than one Type 1 font file: `{first}` and `{duplicate}`"
            ),
            Self::MultipleEncodingFiles {
                tfm_name,
                first,
                duplicate,
            } => write!(
                formatter,
                "TFM `{tfm_name}` names more than one encoding file: `{first}` and `{duplicate}`"
            ),
            Self::FullEmbeddingEncoding {
                tfm_name,
                logical_name,
            } => write!(
                formatter,
                "TFM `{tfm_name}` uses the full-font marker for encoding `{logical_name}`"
            ),
            Self::UnsupportedResourceType {
                tfm_name,
                logical_name,
                expected_extension,
            } => write!(
                formatter,
                "TFM `{tfm_name}` names unsupported font resource `{logical_name}`; expected .{expected_extension}"
            ),
            Self::InvalidFontFileName {
                tfm_name,
                logical_name,
            } => write!(
                formatter,
                "TFM `{tfm_name}` has no usable font stem in `{logical_name}`"
            ),
            Self::PfbParse { resource, source } => write!(
                formatter,
                "cannot parse Type 1 font `{}` at `{}`: {source}",
                resource.logical_name().as_os_str().to_string_lossy(),
                resource.physical_path().display()
            ),
            Self::Type1Metadata { resource, source } => write!(
                formatter,
                "cannot read Type 1 metadata from `{}` at `{}`: {source}",
                resource.logical_name().as_os_str().to_string_lossy(),
                resource.physical_path().display()
            ),
            Self::AfmParse { resource, source } => write!(
                formatter,
                "cannot parse AFM `{}` at `{}`: {source}",
                resource.logical_name().as_os_str().to_string_lossy(),
                resource.physical_path().display()
            ),
            Self::EncodingParse { resource, source } => write!(
                formatter,
                "cannot parse encoding `{}` at `{}`: {source}",
                resource.logical_name().as_os_str().to_string_lossy(),
                resource.physical_path().display()
            ),
            Self::PostScriptNameMismatch {
                tfm_name,
                map_name,
                afm_name,
                afm,
            } => write!(
                formatter,
                "TFM `{tfm_name}` maps to PostScript name `{map_name}`, but AFM `{}` at `{}` declares `{afm_name}`",
                afm.logical_name().as_os_str().to_string_lossy(),
                afm.physical_path().display()
            ),
        }
    }
}

impl std::error::Error for FontResourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolve { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::MapParse { source, .. } => Some(source),
            Self::PfbParse { source, .. } => Some(source),
            Self::Type1Metadata { source, .. } => Some(source),
            Self::AfmParse { source, .. } => Some(source),
            Self::EncodingParse { source, .. } => Some(source),
            Self::ResourceNotFound { .. }
            | Self::InvalidSizeLimit { .. }
            | Self::MissingSizeLimit { .. }
            | Self::ResourceTooLarge { .. }
            | Self::DuplicateTfmEntry { .. }
            | Self::TfmEntryNotFound { .. }
            | Self::UnsupportedMapSpecial { .. }
            | Self::MissingFontFile { .. }
            | Self::MultipleType1FontFiles { .. }
            | Self::MultipleEncodingFiles { .. }
            | Self::FullEmbeddingEncoding { .. }
            | Self::UnsupportedResourceType { .. }
            | Self::InvalidFontFileName { .. }
            | Self::PostScriptNameMismatch { .. } => None,
        }
    }
}

fn file_kind_name(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Tex => "TeX input",
        FileKind::Format => "format",
        FileKind::Tfm => "TFM",
        FileKind::Vf => "VF",
        FileKind::FontMap => "font map",
        FileKind::Encoding => "encoding",
        FileKind::Type1 => "Type 1 font",
        FileKind::Afm => "AFM",
        FileKind::Vaak => "Vaak input",
        FileKind::PdfData => "PDF data",
    }
}

fn operation_name(operation: ResourceIoOperation) -> &'static str {
    match operation {
        ResourceIoOperation::Open => "open",
        ResourceIoOperation::Read => "read",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_search::{CommandExecutor, CommandOutput, KpsewhichResolver, ResolverOptions};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ResolveCall {
        kind: FileKind,
        logical_name: LogicalFileName,
    }

    #[derive(Clone)]
    struct FakeExecutor {
        responses: Rc<RefCell<VecDeque<io::Result<CommandOutput>>>>,
    }

    impl CommandExecutor for FakeExecutor {
        fn execute(
            &mut self,
            _program: &OsStr,
            _arguments: &[OsString],
        ) -> io::Result<CommandOutput> {
            self.responses.borrow_mut().pop_front().unwrap_or_else(|| {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "合成resolver応答が足りない",
                ))
            })
        }
    }

    struct FakeResolver {
        inner: KpsewhichResolver<FakeExecutor>,
        calls: Rc<RefCell<Vec<ResolveCall>>>,
    }

    impl FileResolver for FakeResolver {
        fn resolve(
            &mut self,
            kind: FileKind,
            logical_name: &LogicalFileName,
        ) -> Result<Option<ResolvedFile>, ResolveError> {
            self.calls.borrow_mut().push(ResolveCall {
                kind,
                logical_name: logical_name.clone(),
            });
            self.inner.resolve(kind, logical_name)
        }
    }

    enum FakeResolution {
        Found(PathBuf),
        Missing,
        Failure(io::ErrorKind),
    }

    fn fake_resolver(
        resolutions: impl IntoIterator<Item = FakeResolution>,
    ) -> (FakeResolver, Rc<RefCell<Vec<ResolveCall>>>) {
        let responses = resolutions
            .into_iter()
            .map(|resolution| match resolution {
                FakeResolution::Found(path) => {
                    let mut stdout = path
                        .to_str()
                        .expect("合成temp pathはUTF-8")
                        .as_bytes()
                        .to_vec();
                    stdout.extend_from_slice(b"\r\n");
                    Ok(CommandOutput {
                        code: Some(0),
                        stdout,
                        stderr: Vec::new(),
                    })
                }
                FakeResolution::Missing => Ok(CommandOutput {
                    code: Some(1),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }),
                FakeResolution::Failure(kind) => Err(io::Error::new(kind, "合成resolver失敗")),
            })
            .collect::<VecDeque<_>>();
        let executor = FakeExecutor {
            responses: Rc::new(RefCell::new(responses)),
        };
        let calls = Rc::new(RefCell::new(Vec::new()));
        let resolver = FakeResolver {
            inner: KpsewhichResolver::new(
                ResolverOptions::default().with_kpsewhich_program("fake-kpsewhich"),
                executor,
            ),
            calls: calls.clone(),
        };
        (resolver, calls)
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "rtex-type1-loader-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.path.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let is_own_directory = self.path.starts_with(std::env::temp_dir())
                && self
                    .path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("rtex-type1-loader-"));
            if is_own_directory {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }

    fn pfb_segment(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0x80, kind];
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn pfb() -> Vec<u8> {
        let mut bytes = pfb_segment(1, b"%!PS synthetic\n");
        bytes.extend_from_slice(&pfb_segment(2, &[1, 2, 3, 4]));
        bytes.extend_from_slice(&pfb_segment(1, b"cleartomark\n"));
        bytes.extend_from_slice(&[0x80, 3]);
        bytes
    }

    fn private辞書つきpfb(private: &[u8]) -> Vec<u8> {
        let mut key = 55_665u16;
        let encrypted: Vec<u8> = b"rand"
            .iter()
            .copied()
            .chain(private.iter().copied())
            .map(|plain| {
                let cipher = plain ^ (key >> 8) as u8;
                key = key
                    .wrapping_add(u16::from(cipher))
                    .wrapping_mul(52_845)
                    .wrapping_add(22_719);
                cipher
            })
            .collect();
        let mut bytes = pfb_segment(1, b"%!PS synthetic\ncurrentfile eexec\n");
        bytes.extend_from_slice(&pfb_segment(2, &encrypted));
        bytes.extend_from_slice(&pfb_segment(1, b"cleartomark\n"));
        bytes.extend_from_slice(&[0x80, 3]);
        bytes
    }

    fn afm(font_name: &str) -> Vec<u8> {
        format!(
            "StartFontMetrics 4.1\n\
             FontName {font_name}\n\
             FontBBox -10 -200 1000 900\n\
             ItalicAngle 0\n\
             IsFixedPitch false\n\
             CapHeight 700\n\
             Ascender 750\n\
             Descender -250\n\
             StdVW 80\n\
             StartCharMetrics 1\n\
             C 65 ; WX 600 ; N A ;\n\
             EndCharMetrics\n\
             EndFontMetrics\n"
        )
        .into_bytes()
    }

    fn encoding() -> Vec<u8> {
        let mut bytes = b"/SyntheticEncoding [\n".to_vec();
        for code in 0..256 {
            bytes.extend_from_slice(format!("/g{code} ").as_bytes());
        }
        bytes.extend_from_slice(b"] def\n");
        bytes
    }

    fn unique_names() -> (String, String, String, String, LogicalFileName) {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("rtex-loader-font-{}-{id}", std::process::id());
        (
            format!("rtex-loader-tfm-{}-{id}", std::process::id()),
            format!("{stem}.pfb"),
            format!("{stem}.afm"),
            format!("{stem}.enc"),
            LogicalFileName::new(format!("rtex-loader-map-{}-{id}.map", std::process::id())),
        )
    }

    #[test]
    fn 論理名から全資材を読みmapの指定を保つ() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, afm_name, encoding_name, map_name) = unique_names();
        let map_path = directory.write(
            "physical.map",
            format!("{tfm_name} SyntheticPS 42 <{encoding_name} <<{pfb_name}\n").as_bytes(),
        );
        let pfb_path = directory.write("physical-font.bin", &pfb());
        let afm_path = directory.write("physical-metrics.data", &afm("SyntheticPS"));
        let encoding_path = directory.write("physical-encoding.data", &encoding());
        let (resolver, calls) = fake_resolver([
            FakeResolution::Found(map_path.clone()),
            FakeResolution::Found(pfb_path.clone()),
            FakeResolution::Found(afm_path.clone()),
            FakeResolution::Found(encoding_path.clone()),
        ]);

        let mut loader = FontResourceLoader::with_map(resolver, map_name.clone()).unwrap();
        let loaded = loader.load(&tfm_name).unwrap();

        assert_eq!(loaded.tfm_name, tfm_name);
        assert_eq!(
            loaded.declared_postscript_name.as_deref(),
            Some("SyntheticPS")
        );
        assert_eq!(loaded.postscript_name, "SyntheticPS");
        assert_eq!(loaded.declared_font_flags, Some(42));
        assert_eq!(loaded.descriptor_flags, 42);
        assert_eq!(loaded.private_std_vw, None);
        assert_eq!(loaded.embedding, EmbedPolicy::Full);
        assert_eq!(
            loaded.font_program.logical_name().as_os_str(),
            pfb_name.as_str()
        );
        assert_eq!(loaded.font_program.physical_path(), pfb_path);
        assert_eq!(loaded.metrics.logical_name().as_os_str(), afm_name.as_str());
        assert_eq!(loaded.metrics.physical_path(), afm_path);
        let loaded_encoding = loaded.encoding.as_ref().unwrap();
        assert_eq!(
            loaded_encoding.logical_name().as_os_str(),
            encoding_name.as_str()
        );
        assert_eq!(loaded_encoding.physical_path(), encoding_path);
        assert_eq!(loaded_encoding.value().name(), b"SyntheticEncoding");

        let calls = calls.borrow();
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0].kind, FileKind::FontMap);
        assert_eq!(calls[0].logical_name, map_name);
        assert_eq!(calls[1].kind, FileKind::Type1);
        assert_eq!(calls[1].logical_name.as_os_str(), pfb_name.as_str());
        assert_eq!(calls[2].kind, FileKind::Afm);
        assert_eq!(calls[2].logical_name.as_os_str(), afm_name.as_str());
        assert_eq!(calls[3].kind, FileKind::Encoding);
        assert_eq!(calls[3].logical_name.as_os_str(), encoding_name.as_str());
    }

    #[test]
    fn ps名の宣言有無とflagsの既定値を分けて保つ() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, _afm_name, _encoding_name, map_name) = unique_names();
        let map_path = directory.write(
            "physical.map",
            format!("{tfm_name} <{pfb_name}\n").as_bytes(),
        );
        let pfb_path = directory.write("font.bin", &pfb());
        let afm_path = directory.write("metrics.data", &afm("NameFromAfm"));
        let (resolver, _) = fake_resolver([
            FakeResolution::Found(map_path),
            FakeResolution::Found(pfb_path),
            FakeResolution::Found(afm_path),
        ]);

        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();
        let loaded = loader.load(&tfm_name).unwrap();

        assert_eq!(loaded.declared_postscript_name, None);
        assert_eq!(loaded.postscript_name, "NameFromAfm");
        assert_eq!(loaded.declared_font_flags, None);
        assert_eq!(loaded.descriptor_flags, PDFTEX_DEFAULT_FONT_FLAGS);
        assert_eq!(loaded.private_std_vw, None);
        assert_eq!(loaded.embedding, EmbedPolicy::Subset);
        assert!(loaded.encoding.is_none());
    }

    #[test]
    fn afmにstdvwが無ければpfbのprivate辞書だけをfallbackにする() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, _afm_name, _encoding_name, map_name) = unique_names();
        let map_path = directory.write(
            "private-stdvw.map",
            format!("{tfm_name} NameFromAfm <<{pfb_name}\n").as_bytes(),
        );
        let pfb_path = directory.write(
            "font.bin",
            &private辞書つきpfb(b"/Private 1 dict dup begin\n/StdVW [69] ND\n/Subrs 0 array\n"),
        );
        let afm_without_std_vw = String::from_utf8(afm("NameFromAfm"))
            .unwrap()
            .replace("StdVW 80\n", "");
        let afm_path = directory.write("metrics.data", afm_without_std_vw.as_bytes());
        let (resolver, _) = fake_resolver([
            FakeResolution::Found(map_path),
            FakeResolution::Found(pfb_path),
            FakeResolution::Found(afm_path),
        ]);

        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();
        let loaded = loader.load(&tfm_name).unwrap();

        assert_eq!(loaded.metrics.value().descriptor.std_vw, None);
        assert_eq!(
            loaded.private_std_vw,
            Some(AfmNumber::checked_from_integer(69).unwrap())
        );
    }

    #[test]
    fn mapは一回だけ解決する() {
        let directory = TestDirectory::new();
        let (_, _, _, _, map_name) = unique_names();
        let map_path = directory.write("empty.map", b"% empty\n");
        let (resolver, calls) = fake_resolver([FakeResolution::Found(map_path)]);
        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();

        assert!(matches!(
            loader.load("absent-one"),
            Err(FontResourceError::TfmEntryNotFound { .. })
        ));
        assert!(matches!(
            loader.load("absent-two"),
            Err(FontResourceError::TfmEntryNotFound { .. })
        ));
        assert_eq!(calls.borrow().len(), 1);
        assert_eq!(calls.borrow()[0].kind, FileKind::FontMap);
    }

    #[test]
    fn tfm重複を黙って上書きしない() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, _, _, map_name) = unique_names();
        let map_path = directory.write(
            "duplicate.map",
            format!("{tfm_name} First <{pfb_name}\n{tfm_name} Second <other.pfb\n").as_bytes(),
        );
        let (resolver, _) = fake_resolver([FakeResolution::Found(map_path)]);

        assert!(matches!(
            FontResourceLoader::with_map(resolver, map_name),
            Err(FontResourceError::DuplicateTfmEntry { tfm_name: found, .. })
                if found == tfm_name
        ));
    }

    #[test]
    fn map_specialを資材解決前に拒む() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, _, _, map_name) = unique_names();
        let map_path = directory.write(
            "special.map",
            format!("{tfm_name} SyntheticPS \"0.2 SlantFont 1.1 ExtendFont\" <{pfb_name}\n")
                .as_bytes(),
        );
        let (resolver, calls) = fake_resolver([FakeResolution::Found(map_path)]);
        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();

        assert!(matches!(
            loader.load(&tfm_name),
            Err(FontResourceError::UnsupportedMapSpecial {
                mentions_slant_font: true,
                mentions_extend_font: true,
                ..
            })
        ));
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn 未使用entryの複数資源と分離markerはmap全体を止めない() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, _, _, map_name) = unique_names();
        let map_path = directory.write(
            "texlive-shapes.map",
            format!(
                "unused Clm \"HE8Encoding ReEncodeFont\" <he8.enc <<helper.t3 <font.pfb\n\
                 plimsoll < plimsoll.enc < plimsoll.pfb\n\
                 {tfm_name} SyntheticPS 6 <<{pfb_name}\n"
            )
            .as_bytes(),
        );
        let pfb_path = directory.write("font.bin", &pfb());
        let afm_path = directory.write("metrics.data", &afm("SyntheticPS"));
        let (resolver, calls) = fake_resolver([
            FakeResolution::Found(map_path),
            FakeResolution::Found(pfb_path),
            FakeResolution::Found(afm_path),
        ]);

        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();
        let loaded = loader.load(&tfm_name).unwrap();

        assert_eq!(loaded.postscript_name, "SyntheticPS");
        assert_eq!(calls.borrow().len(), 3);
    }

    #[test]
    fn 選んだentryの補助資源だけを局所的に拒む() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, _, encoding_name, map_name) = unique_names();
        let map_path = directory.write(
            "auxiliary.map",
            format!("{tfm_name} SyntheticPS <{encoding_name} <<helper.t3 <{pfb_name}\n").as_bytes(),
        );
        let (resolver, calls) = fake_resolver([FakeResolution::Found(map_path)]);
        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();

        assert!(matches!(
            loader.load(&tfm_name),
            Err(FontResourceError::UnsupportedResourceType {
                logical_name,
                expected_extension: "pfb",
                ..
            }) if logical_name == "helper.t3"
        ));
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn 選んだentryの複数pfbと複数encodingを黙って選ばない() {
        let directory = TestDirectory::new();
        let (first_tfm, first_pfb, _, first_encoding, map_name) = unique_names();
        let second_tfm = format!("{first_tfm}-encoding");
        let map_path = directory.write(
            "ambiguous.map",
            format!(
                "{first_tfm} PS <{first_pfb} <<second.pfb\n\
                 {second_tfm} PS <{first_encoding} <second.enc <<{first_pfb}\n"
            )
            .as_bytes(),
        );
        let (resolver, calls) = fake_resolver([FakeResolution::Found(map_path)]);
        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();

        assert!(matches!(
            loader.load(&first_tfm),
            Err(FontResourceError::MultipleType1FontFiles { first, duplicate, .. })
                if first == first_pfb && duplicate == "second.pfb"
        ));
        assert!(matches!(
            loader.load(&second_tfm),
            Err(FontResourceError::MultipleEncodingFiles { first, duplicate, .. })
                if first == first_encoding && duplicate == "second.enc"
        ));
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn pfb以外を資材解決前に拒む() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, _, _, map_name) = unique_names();
        let pfa_name = pfb_name.replace(".pfb", ".pfa");
        let map_path = directory.write(
            "pfa.map",
            format!("{tfm_name} SyntheticPS <{pfa_name}\n").as_bytes(),
        );
        let (resolver, calls) = fake_resolver([FakeResolution::Found(map_path)]);
        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();

        assert!(matches!(
            loader.load(&tfm_name),
            Err(FontResourceError::UnsupportedResourceType {
                logical_name,
                expected_extension: "pfb",
                ..
            }) if logical_name == pfa_name
        ));
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn mapとafmのps名不一致を報告する() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, _, _, map_name) = unique_names();
        let map_path = directory.write(
            "mismatch.map",
            format!("{tfm_name} NameFromMap <{pfb_name}\n").as_bytes(),
        );
        let pfb_path = directory.write("font.bin", &pfb());
        let afm_path = directory.write("metrics.data", &afm("NameFromAfm"));
        let (resolver, _) = fake_resolver([
            FakeResolution::Found(map_path),
            FakeResolution::Found(pfb_path),
            FakeResolution::Found(afm_path),
        ]);
        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();

        assert!(matches!(
            loader.load(&tfm_name),
            Err(FontResourceError::PostScriptNameMismatch {
                map_name,
                afm_name,
                ..
            }) if map_name == "NameFromMap" && afm_name == "NameFromAfm"
        ));
    }

    #[test]
    fn metadataを使わず上限の次の一byteで停止する() {
        let directory = TestDirectory::new();
        let (_, _, _, _, map_name) = unique_names();
        let map_path = directory.write("large.map", b"12345");
        let (resolver, _) = fake_resolver([FakeResolution::Found(map_path)]);
        let limits = ResourceLimits {
            font_map: 4,
            ..ResourceLimits::STANDARD
        };

        assert!(matches!(
            FontResourceLoader::with_map_and_limits(resolver, map_name, limits),
            Err(FontResourceError::ResourceTooLarge {
                kind: FileKind::FontMap,
                limit: 4,
                observed_at_least: 5,
                ..
            })
        ));
    }

    #[test]
    fn resolve失敗と不在を区別する() {
        let (_, _, _, _, missing_map) = unique_names();
        let (resolver, _) = fake_resolver([FakeResolution::Missing]);
        assert!(matches!(
            FontResourceLoader::with_map(resolver, missing_map),
            Err(FontResourceError::ResourceNotFound {
                kind: FileKind::FontMap,
                ..
            })
        ));

        let (_, _, _, _, failed_map) = unique_names();
        let (resolver, _) = fake_resolver([FakeResolution::Failure(io::ErrorKind::NotFound)]);
        assert!(matches!(
            FontResourceLoader::with_map(resolver, failed_map),
            Err(FontResourceError::Resolve {
                kind: FileKind::FontMap,
                ..
            })
        ));
    }

    #[test]
    fn parser失敗にも論理名と物理pathを残す() {
        let directory = TestDirectory::new();
        let (tfm_name, pfb_name, _, _, map_name) = unique_names();
        let map_path = directory.write(
            "invalid-pfb.map",
            format!("{tfm_name} SyntheticPS <{pfb_name}\n").as_bytes(),
        );
        let pfb_path = directory.write("broken.bin", b"not a PFB");
        let (resolver, _) = fake_resolver([
            FakeResolution::Found(map_path),
            FakeResolution::Found(pfb_path.clone()),
        ]);
        let mut loader = FontResourceLoader::with_map(resolver, map_name).unwrap();

        match loader.load(&tfm_name) {
            Err(FontResourceError::PfbParse { resource, .. }) => {
                assert_eq!(resource.logical_name().as_os_str(), pfb_name.as_str());
                assert_eq!(resource.physical_path(), pfb_path);
            }
            _ => panic!("PFB parse errorではない"),
        }
    }
}

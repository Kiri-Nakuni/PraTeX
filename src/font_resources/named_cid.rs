//! 非埋込みCID fontを明示選択する、一profileだけのhost-owned境界。
//!
//! JFMは文字幅とclassを持つが字形資材やPDF CID font名を持たない。このprofileは
//! その不足を暗黙探索やengine偽装で埋めず、利用者が選んだviewer側CID font契約を
//! 一つのJFM論理名へ結ぶ。物理pathは起動時に一度だけbounded readし、font定義後の
//! glyph loopにはprofile loaderやI/Oを残さない。

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub(crate) const MAX_NAMED_CID_PROFILE_BYTES: usize = 64 * 1024;
const PROFILE_HEADER: &str = "PraTeX-Named-CID-Profile 1";
const BUILT_IN_UPJISR_H_PROFILE: &[u8] = include_bytes!("../../docs/examples/upjisr-h.cidprofile");

/// このsliceで固定するPDF 1.4 predefined CMap。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NamedCidEncoding {
    UniJisUcs2H,
}

impl NamedCidEncoding {
    pub(crate) const fn pdf_name(self) -> &'static str {
        match self {
            Self::UniJisUcs2H => "UniJIS-UCS2-H",
        }
    }

    pub(crate) const fn registry(self) -> &'static str {
        "Adobe"
    }

    pub(crate) const fn ordering(self) -> &'static str {
        "Japan1"
    }

    pub(crate) const fn supplement(self) -> u8 {
        4
    }

    /// Unicode scalarを、Type 0 content stringでこのpredefined CMapへ渡すcodeへ写す。
    ///
    /// `UniJIS-UCS2-H`はCIDそのものではなく、Unicode UCS-2 codeを入力に取る。
    /// CIDはviewerがpredefined CMapを通して決めるので、ここでAdobe-Japan1のCIDを
    /// 推測したり、Unicode scalarをCIDと同じdomainとして扱ったりしない。
    pub(crate) fn encode_scalar(self, scalar: u32) -> Option<[u8; 2]> {
        match self {
            Self::UniJisUcs2H if scalar <= 0xffff && !(0xd800..=0xdfff).contains(&scalar) => {
                Some((scalar as u16).to_be_bytes())
            }
            Self::UniJisUcs2H => None,
        }
    }

    /// このencodingが受け取るcontent codeからUnicodeへ戻すPDF ToUnicode CMap。
    ///
    /// sourceはCIDではなく`encode_scalar`が作るUCS-2 codeである。surrogate帯を
    /// codespaceから除外した二rangeを同じBMP scalarへ戻すことで、CID番号との
    /// 誤った恒等写像を避ける。
    pub(crate) const fn to_unicode_cmap(self) -> &'static [u8] {
        match self {
            Self::UniJisUcs2H => {
                b"/CIDInit /ProcSet findresource begin\n\
12 dict begin\n\
begincmap\n\
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
/CMapName /PraTeX-UniJIS-UCS2-ToUnicode def\n\
/CMapType 2 def\n\
2 begincodespacerange\n\
<0000> <D7FF>\n\
<E000> <FFFF>\n\
endcodespacerange\n\
2 beginbfrange\n\
<0000> <D7FF> <0000>\n\
<E000> <FFFF> <E000>\n\
endbfrange\n\
endcmap\n\
CMapName currentdict /CMap defineresource pop\n\
end\n\
end"
            }
        }
    }
}

/// 一つのJFM論理名へ結ぶ、非埋込みnamed CID fontの検査前profile。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NamedCidFontProfile {
    jfm_name: String,
    base_font: String,
    flags: u32,
    font_bbox: [i32; 4],
    italic_angle: i32,
    ascent: i32,
    descent: i32,
    cap_height: i32,
    stem_v: i32,
    default_width: i32,
    encoding: NamedCidEncoding,
}

impl NamedCidFontProfile {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, NamedCidProfileError> {
        if let Some((offset, byte)) = bytes
            .iter()
            .copied()
            .enumerate()
            .find(|(_, byte)| !matches!(byte, b'\t' | b'\n' | b'\r' | b' '..=b'~'))
        {
            return Err(NamedCidProfileError::NonAscii { offset, byte });
        }
        let text = std::str::from_utf8(bytes).expect("ASCII is UTF-8");
        let mut lines = text.lines().enumerate();
        let Some((_, header)) = lines.next() else {
            return Err(NamedCidProfileError::MissingHeader);
        };
        if header != PROFILE_HEADER {
            return Err(NamedCidProfileError::InvalidHeader(header.to_owned()));
        }

        let mut fields = ProfileFields::default();
        let mut ended = false;
        for (index, line) in lines {
            let line_number = index + 1;
            if ended {
                return Err(NamedCidProfileError::TrailingLine { line: line_number });
            }
            if line == "EndProfile" {
                ended = true;
                continue;
            }
            if line.is_empty() {
                return Err(NamedCidProfileError::EmptyLine { line: line_number });
            }
            let mut words = line.split_ascii_whitespace();
            let key = words.next().expect("nonempty line has a word");
            match key {
                "JfmName" => set_text_field(
                    &mut fields.jfm_name,
                    key,
                    one_value(words, key, line_number)?,
                    line_number,
                )?,
                "BaseFont" => set_text_field(
                    &mut fields.base_font,
                    key,
                    one_value(words, key, line_number)?,
                    line_number,
                )?,
                "Flags" => set_field(
                    &mut fields.flags,
                    key,
                    parse_u32(one_value(words, key, line_number)?, key, line_number)?,
                    line_number,
                )?,
                "FontBBox" => {
                    let values: Vec<&str> = words.collect();
                    if values.len() != 4 {
                        return Err(NamedCidProfileError::MalformedField {
                            line: line_number,
                            field: key.to_owned(),
                            expected: "four decimal integers",
                        });
                    }
                    let bbox = [
                        parse_i32(values[0], key, line_number)?,
                        parse_i32(values[1], key, line_number)?,
                        parse_i32(values[2], key, line_number)?,
                        parse_i32(values[3], key, line_number)?,
                    ];
                    set_field(&mut fields.font_bbox, key, bbox, line_number)?;
                }
                "ItalicAngle" => set_i32(&mut fields.italic_angle, words, key, line_number)?,
                "Ascent" => set_i32(&mut fields.ascent, words, key, line_number)?,
                "Descent" => set_i32(&mut fields.descent, words, key, line_number)?,
                "CapHeight" => set_i32(&mut fields.cap_height, words, key, line_number)?,
                "StemV" => set_i32(&mut fields.stem_v, words, key, line_number)?,
                "DefaultWidth" => set_i32(&mut fields.default_width, words, key, line_number)?,
                other => {
                    return Err(NamedCidProfileError::UnknownField {
                        line: line_number,
                        field: other.to_owned(),
                    });
                }
            }
        }
        if !ended {
            return Err(NamedCidProfileError::MissingEndProfile);
        }

        Ok(Self {
            jfm_name: required(fields.jfm_name, "JfmName")?,
            base_font: required(fields.base_font, "BaseFont")?,
            flags: required(fields.flags, "Flags")?,
            font_bbox: required(fields.font_bbox, "FontBBox")?,
            italic_angle: required(fields.italic_angle, "ItalicAngle")?,
            ascent: required(fields.ascent, "Ascent")?,
            descent: required(fields.descent, "Descent")?,
            cap_height: required(fields.cap_height, "CapHeight")?,
            stem_v: required(fields.stem_v, "StemV")?,
            default_width: required(fields.default_width, "DefaultWidth")?,
            encoding: NamedCidEncoding::UniJisUcs2H,
        })
    }

    pub(crate) fn jfm_name(&self) -> &str {
        &self.jfm_name
    }

    pub(crate) fn base_font(&self) -> &str {
        &self.base_font
    }

    pub(crate) fn flags(&self) -> u32 {
        self.flags
    }

    pub(crate) fn font_bbox(&self) -> [i32; 4] {
        self.font_bbox
    }

    pub(crate) fn italic_angle(&self) -> i32 {
        self.italic_angle
    }

    pub(crate) fn ascent(&self) -> i32 {
        self.ascent
    }

    pub(crate) fn descent(&self) -> i32 {
        self.descent
    }

    pub(crate) fn cap_height(&self) -> i32 {
        self.cap_height
    }

    pub(crate) fn stem_v(&self) -> i32 {
        self.stem_v
    }

    pub(crate) fn default_width(&self) -> i32 {
        self.default_width
    }

    pub(crate) fn encoding(&self) -> NamedCidEncoding {
        self.encoding
    }
}

#[derive(Default)]
struct ProfileFields {
    jfm_name: Option<String>,
    base_font: Option<String>,
    flags: Option<u32>,
    font_bbox: Option<[i32; 4]>,
    italic_angle: Option<i32>,
    ascent: Option<i32>,
    descent: Option<i32>,
    cap_height: Option<i32>,
    stem_v: Option<i32>,
    default_width: Option<i32>,
}

fn one_value<'a>(
    mut words: impl Iterator<Item = &'a str>,
    field: &str,
    line: usize,
) -> Result<&'a str, NamedCidProfileError> {
    let Some(value) = words.next() else {
        return Err(NamedCidProfileError::MalformedField {
            line,
            field: field.to_owned(),
            expected: "one value",
        });
    };
    if words.next().is_some() {
        return Err(NamedCidProfileError::MalformedField {
            line,
            field: field.to_owned(),
            expected: "one value",
        });
    }
    Ok(value)
}

fn set_i32<'a>(
    slot: &mut Option<i32>,
    words: impl Iterator<Item = &'a str>,
    field: &str,
    line: usize,
) -> Result<(), NamedCidProfileError> {
    let value = parse_i32(one_value(words, field, line)?, field, line)?;
    set_field(slot, field, value, line)
}

fn parse_i32(value: &str, field: &str, line: usize) -> Result<i32, NamedCidProfileError> {
    value
        .parse()
        .map_err(|_| NamedCidProfileError::InvalidInteger {
            line,
            field: field.to_owned(),
            value: value.to_owned(),
        })
}

fn parse_u32(value: &str, field: &str, line: usize) -> Result<u32, NamedCidProfileError> {
    value
        .parse()
        .map_err(|_| NamedCidProfileError::InvalidInteger {
            line,
            field: field.to_owned(),
            value: value.to_owned(),
        })
}

fn set_text_field(
    slot: &mut Option<String>,
    field: &str,
    value: &str,
    line: usize,
) -> Result<(), NamedCidProfileError> {
    set_field(slot, field, value.to_owned(), line)
}

fn set_field<T>(
    slot: &mut Option<T>,
    field: &str,
    value: T,
    line: usize,
) -> Result<(), NamedCidProfileError> {
    if slot.replace(value).is_some() {
        return Err(NamedCidProfileError::DuplicateField {
            line,
            field: field.to_owned(),
        });
    }
    Ok(())
}

fn required<T>(slot: Option<T>, field: &'static str) -> Result<T, NamedCidProfileError> {
    slot.ok_or(NamedCidProfileError::MissingField(field))
}

/// BackendがJFM論理名からprofileをfont定義時に一度だけ得る境界。
pub(crate) trait NamedCidFontProfileLoader {
    fn load(&mut self, jfm_name: &str) -> Result<NamedCidFontProfile, NamedCidProfileError>;
}

/// 横組の既定JFMだけを、repositoryで検査したnamed CID契約へ結ぶloader。
///
/// 任意のJFM名を似たfontへ推測で結ばない。別JFMにはCLIの明示profileを要求する。
pub(crate) struct BuiltInNamedCidProfileLoader {
    upjisr_h: NamedCidFontProfile,
}

impl BuiltInNamedCidProfileLoader {
    pub(crate) fn new() -> Result<Self, NamedCidProfileError> {
        Ok(Self {
            upjisr_h: NamedCidFontProfile::parse(BUILT_IN_UPJISR_H_PROFILE)?,
        })
    }
}

impl NamedCidFontProfileLoader for BuiltInNamedCidProfileLoader {
    fn load(&mut self, jfm_name: &str) -> Result<NamedCidFontProfile, NamedCidProfileError> {
        if jfm_name != self.upjisr_h.jfm_name() {
            return Err(NamedCidProfileError::NoBuiltInProfile {
                requested_name: jfm_name.to_owned(),
            });
        }
        Ok(self.upjisr_h.clone())
    }
}

/// CLIで明示された一つの物理pathを、探索せず直接読むloader。
pub(crate) struct FileNamedCidProfileLoader {
    path: PathBuf,
    profile: NamedCidFontProfile,
}

impl FileNamedCidProfileLoader {
    pub(crate) fn from_path(path: impl AsRef<Path>) -> Result<Self, NamedCidProfileError> {
        Self::from_path_with_limit(path.as_ref(), MAX_NAMED_CID_PROFILE_BYTES)
    }

    fn from_path_with_limit(path: &Path, limit: usize) -> Result<Self, NamedCidProfileError> {
        let sentinel = limit
            .checked_add(1)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(NamedCidProfileError::InvalidSizeLimit(limit))?;
        let file = File::open(path).map_err(|source| NamedCidProfileError::Io {
            operation: ProfileIoOperation::Open,
            path: path.to_owned(),
            source,
        })?;
        let mut bytes = Vec::with_capacity(limit.min(4096).saturating_add(1));
        file.take(sentinel)
            .read_to_end(&mut bytes)
            .map_err(|source| NamedCidProfileError::Io {
                operation: ProfileIoOperation::Read,
                path: path.to_owned(),
                source,
            })?;
        if bytes.len() > limit {
            return Err(NamedCidProfileError::TooLarge {
                path: path.to_owned(),
                limit,
                observed_at_least: bytes.len(),
            });
        }
        let profile = NamedCidFontProfile::parse(&bytes)?;
        Ok(Self {
            path: path.to_owned(),
            profile,
        })
    }
}

impl NamedCidFontProfileLoader for FileNamedCidProfileLoader {
    fn load(&mut self, jfm_name: &str) -> Result<NamedCidFontProfile, NamedCidProfileError> {
        if self.profile.jfm_name() != jfm_name {
            return Err(NamedCidProfileError::JfmNameMismatch {
                path: Some(self.path.clone()),
                profile_name: self.profile.jfm_name().to_owned(),
                requested_name: jfm_name.to_owned(),
            });
        }
        Ok(self.profile.clone())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProfileIoOperation {
    Open,
    Read,
}

#[derive(Debug)]
pub(crate) enum NamedCidProfileError {
    Io {
        operation: ProfileIoOperation,
        path: PathBuf,
        source: io::Error,
    },
    InvalidSizeLimit(usize),
    TooLarge {
        path: PathBuf,
        limit: usize,
        observed_at_least: usize,
    },
    NonAscii {
        offset: usize,
        byte: u8,
    },
    MissingHeader,
    InvalidHeader(String),
    EmptyLine {
        line: usize,
    },
    UnknownField {
        line: usize,
        field: String,
    },
    DuplicateField {
        line: usize,
        field: String,
    },
    MalformedField {
        line: usize,
        field: String,
        expected: &'static str,
    },
    InvalidInteger {
        line: usize,
        field: String,
        value: String,
    },
    MissingField(&'static str),
    MissingEndProfile,
    TrailingLine {
        line: usize,
    },
    JfmNameMismatch {
        path: Option<PathBuf>,
        profile_name: String,
        requested_name: String,
    },
    NoBuiltInProfile {
        requested_name: String,
    },
}

impl fmt::Display for NamedCidProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "cannot {} named CID profile `{}`: {source}",
                match operation {
                    ProfileIoOperation::Open => "open",
                    ProfileIoOperation::Read => "read",
                },
                path.display()
            ),
            Self::InvalidSizeLimit(limit) => {
                write!(formatter, "invalid named CID profile size limit {limit}")
            }
            Self::TooLarge {
                path,
                limit,
                observed_at_least,
            } => write!(
                formatter,
                "named CID profile `{}` exceeds {limit} bytes (read at least {observed_at_least})",
                path.display()
            ),
            Self::NonAscii { offset, byte } => write!(
                formatter,
                "named CID profile byte {offset} is non-ASCII ({byte:#04x})"
            ),
            Self::MissingHeader => formatter.write_str("named CID profile header is missing"),
            Self::InvalidHeader(header) => write!(
                formatter,
                "invalid named CID profile header `{header}` (expected `{PROFILE_HEADER}`)"
            ),
            Self::EmptyLine { line } => {
                write!(formatter, "named CID profile line {line} is empty")
            }
            Self::UnknownField { line, field } => {
                write!(formatter, "unknown named CID profile field `{field}` on line {line}")
            }
            Self::DuplicateField { line, field } => write!(
                formatter,
                "named CID profile field `{field}` occurs again on line {line}"
            ),
            Self::MalformedField {
                line,
                field,
                expected,
            } => write!(
                formatter,
                "named CID profile field `{field}` on line {line} requires {expected}"
            ),
            Self::InvalidInteger { line, field, value } => write!(
                formatter,
                "named CID profile field `{field}` on line {line} has invalid decimal integer `{value}`"
            ),
            Self::MissingField(field) => {
                write!(formatter, "named CID profile field `{field}` is missing")
            }
            Self::MissingEndProfile => {
                formatter.write_str("named CID profile EndProfile marker is missing")
            }
            Self::TrailingLine { line } => write!(
                formatter,
                "named CID profile has content after EndProfile on line {line}"
            ),
            Self::JfmNameMismatch {
                path,
                profile_name,
                requested_name,
            } => {
                formatter.write_str("named CID profile")?;
                if let Some(path) = path {
                    write!(formatter, " `{}`", path.display())?;
                }
                write!(
                    formatter,
                    " is for JFM `{profile_name}`, not requested JFM `{requested_name}`"
                )
            }
            Self::NoBuiltInProfile { requested_name } => write!(
                formatter,
                "Japanese PDF JFM `{requested_name}` has no built-in named CID profile; pass --pdf-japanese-cid-profile=PATH"
            ),
        }
    }
}

impl std::error::Error for NamedCidProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn profile() -> &'static [u8] {
        b"PraTeX-Named-CID-Profile 1\n\
JfmName min10\n\
BaseFont HeiseiMin-W3\n\
Flags 6\n\
FontBBox -123 -257 1001 910\n\
ItalicAngle 0\n\
Ascent 880\n\
Descent -120\n\
CapHeight 700\n\
StemV 80\n\
DefaultWidth 1000\n\
EndProfile\n"
    }

    #[test]
    fn 一profileを固定cmapとcid_systemへ読む() {
        let parsed = NamedCidFontProfile::parse(profile()).unwrap();
        assert_eq!(parsed.jfm_name(), "min10");
        assert_eq!(parsed.base_font(), "HeiseiMin-W3");
        assert_eq!(parsed.flags(), 6);
        assert_eq!(parsed.font_bbox(), [-123, -257, 1001, 910]);
        assert_eq!(parsed.italic_angle(), 0);
        assert_eq!(parsed.ascent(), 880);
        assert_eq!(parsed.descent(), -120);
        assert_eq!(parsed.cap_height(), 700);
        assert_eq!(parsed.stem_v(), 80);
        assert_eq!(parsed.default_width(), 1000);
        assert_eq!(parsed.encoding().pdf_name(), "UniJIS-UCS2-H");
        assert_eq!(parsed.encoding().registry(), "Adobe");
        assert_eq!(parsed.encoding().ordering(), "Japan1");
        assert_eq!(parsed.encoding().supplement(), 4);
    }

    #[test]
    fn unicode_scalarはcidでなくunijisの二byte入力codeへ写す() {
        let encoding = NamedCidEncoding::UniJisUcs2H;
        assert_eq!(encoding.encode_scalar(0x005c), Some([0x00, 0x5c]));
        assert_eq!(encoding.encode_scalar(0x3042), Some([0x30, 0x42]));
        assert_eq!(encoding.encode_scalar(0xd800), None);
        assert_eq!(encoding.encode_scalar(0x1_0000), None);

        let cmap = String::from_utf8_lossy(encoding.to_unicode_cmap());
        assert!(cmap.contains("<0000> <D7FF> <0000>"));
        assert!(cmap.contains("<E000> <FFFF> <E000>"));
        assert!(!cmap.contains("<D800>"));
    }

    #[test]
    fn 未知重複欠損と終端後dataを黙認しない() {
        let unknown = profile().replace_ascii(b"StemV 80", b"StemH 80");
        assert!(matches!(
            NamedCidFontProfile::parse(&unknown),
            Err(NamedCidProfileError::UnknownField { field, .. }) if field == "StemH"
        ));

        let duplicate = profile().replace_ascii(b"EndProfile", b"StemV 81\nEndProfile");
        assert!(matches!(
            NamedCidFontProfile::parse(&duplicate),
            Err(NamedCidProfileError::DuplicateField { field, .. }) if field == "StemV"
        ));

        let missing = profile().replace_ascii(b"CapHeight 700\n", b"");
        assert!(matches!(
            NamedCidFontProfile::parse(&missing),
            Err(NamedCidProfileError::MissingField("CapHeight"))
        ));

        let trailing = profile().replace_ascii(b"EndProfile\n", b"EndProfile\nFlags 4\n");
        assert!(matches!(
            NamedCidFontProfile::parse(&trailing),
            Err(NamedCidProfileError::TrailingLine { .. })
        ));
    }

    #[test]
    fn 非asciiと不正整数をparse境界で拒む() {
        let mut non_ascii = profile().to_vec();
        non_ascii[0] = 0xff;
        assert!(matches!(
            NamedCidFontProfile::parse(&non_ascii),
            Err(NamedCidProfileError::NonAscii {
                offset: 0,
                byte: 0xff
            })
        ));

        let invalid = profile().replace_ascii(b"Flags 6", b"Flags -1");
        assert!(matches!(
            NamedCidFontProfile::parse(&invalid),
            Err(NamedCidProfileError::InvalidInteger { field, .. }) if field == "Flags"
        ));
    }

    #[test]
    fn 物理profileは上限の次の一byteで止めjfm名を照合する() {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "pratex-named-cid-profile-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, profile()).unwrap();
        assert!(matches!(
            FileNamedCidProfileLoader::from_path_with_limit(&path, 4),
            Err(NamedCidProfileError::TooLarge {
                limit: 4,
                observed_at_least: 5,
                ..
            })
        ));

        let mut loader = FileNamedCidProfileLoader::from_path(&path).unwrap();
        assert!(matches!(
            loader.load("goth10"),
            Err(NamedCidProfileError::JfmNameMismatch {
                profile_name,
                requested_name,
                path: Some(found_path),
            }) if profile_name == "min10"
                && requested_name == "goth10"
                && found_path == path
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn 既定横組jfmだけを内蔵profileへ結ぶ() {
        let mut loader = BuiltInNamedCidProfileLoader::new().unwrap();
        let profile = loader.load("upjisr-h").unwrap();
        assert_eq!(profile.jfm_name(), "upjisr-h");
        assert_eq!(profile.base_font(), "HeiseiMin-W3");
        assert!(matches!(
            loader.load("user-jfm"),
            Err(NamedCidProfileError::NoBuiltInProfile { requested_name })
                if requested_name == "user-jfm"
        ));
    }

    trait ReplaceAscii {
        fn replace_ascii(&self, from: &[u8], to: &[u8]) -> Vec<u8>;
    }

    impl ReplaceAscii for [u8] {
        fn replace_ascii(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
            let position = self
                .windows(from.len())
                .position(|window| window == from)
                .expect("test source contains replacement target");
            let mut result = Vec::with_capacity(self.len() - from.len() + to.len());
            result.extend_from_slice(&self[..position]);
            result.extend_from_slice(to);
            result.extend_from_slice(&self[position + from.len()..]);
            result
        }
    }
}

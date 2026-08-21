//! PDF 1.4 の Type 1 font object を組み立てる層。
//!
//! Adobe *PDF Reference, Third Edition, version 1.4* の 5.5、5.8 節だけを
//! PDF object の根拠にする。PFB・AFM・map・encoding の解釈は
//! [`crate::font_resources`] に閉じ込め、この層は検査済みの値を PDF の
//! `FontFile` / `FontDescriptor` / `Encoding` / `Font` に写すだけである。
//! <https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf>
//!
//! 外部由来の文字列は PDF name として `#XX` escape してから書く。現時点で
//! literal string object は一つも生成しない。PostScript program や encoding を
//! 実行する経路も持たない。

use crate::font_resources::afm::{AfmFont, AfmNumber};
use crate::font_resources::encoding::EncodingVector;
use crate::font_resources::map::EmbedPolicy;
use crate::font_resources::type1::Type1FontProgram;
use crate::pdf::{PdfObjectId, PdfWriter, PdfWriterError};

use std::fmt;
use std::io::Write;

const FIXED_PITCH_FLAG: u32 = 1 << 0;
const SERIF_FLAG: u32 = 1 << 1;
const SYMBOLIC_FLAG: u32 = 1 << 2;
const SCRIPT_FLAG: u32 = 1 << 3;
const NONSYMBOLIC_FLAG: u32 = 1 << 5;
const ITALIC_FLAG: u32 = 1 << 6;
const ALL_CAP_FLAG: u32 = 1 << 16;
const SMALL_CAP_FLAG: u32 = 1 << 17;
const FORCE_BOLD_FLAG: u32 = 1 << 18;
const MAX_PDF_NAME_BYTES: usize = 127;
const MAX_PDF_REAL_SCALED: i128 = 32_767_000_000;
const ALLOWED_DESCRIPTOR_FLAGS: u32 = FIXED_PITCH_FLAG
    | SERIF_FLAG
    | SYMBOLIC_FLAG
    | SCRIPT_FLAG
    | NONSYMBOLIC_FLAG
    | ITALIC_FLAG
    | ALL_CAP_FLAG
    | SMALL_CAP_FLAG
    | FORCE_BOLD_FLAG;

/// 一つの Type 1 font object を作るために必要な検査済み資材。
///
/// `used_codes` は実際に content stream から参照する byte code である。その最小値
/// から最大値までを `/FirstChar`、`/LastChar`、`/Widths` にする。PDF 1.4 は
/// array は連続範囲なので、間の未使用 code に AFM metric がなければ
/// `/MissingWidth 0` と同じ 0 を置く。実際に使用する code の metric は必須である。
pub(crate) struct PdfType1FontRequest<'a> {
    pub(crate) program: &'a Type1FontProgram,
    pub(crate) afm: &'a AfmFont,
    pub(crate) encoding: Option<&'a EncodingVector>,
    pub(crate) embedding: EmbedPolicy,
    pub(crate) descriptor_flags: u32,
    /// AFM が `StdVW` を省略したときだけ使う、呼び出し側の明示的な方針。
    pub(crate) missing_stem_v: MissingStemVPolicy,
    pub(crate) used_codes: &'a [u8],
}

/// 古い AFM が PDF で必須の `StemV` を持たない場合の方針。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MissingStemVPolicy {
    Reject,
    Use(AfmNumber),
}

/// PDF writer にまだ object を予約していない Type 1 font。
///
/// 構築時に font 資材をすべて検査する。`write` は `self` を消費するので、この
/// prepared value から同じ間接 object 群を二度書くことはできない。
pub(crate) struct PreparedPdfType1Font<'a> {
    program: &'a [u8],
    lengths: [i32; 3],
    descriptor_prefix: Vec<u8>,
    base_font_name: Vec<u8>,
    encoding_body: Option<Vec<u8>>,
    first_char: u8,
    last_char: u8,
    widths: Vec<AfmNumber>,
}

/// Page resource から参照する、書き込み済み Type 1 font の型付き handle。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PdfType1Font {
    object: PdfObjectId,
    first_char: u8,
    last_char: u8,
}

impl PdfType1Font {
    pub(crate) fn object(self) -> PdfObjectId {
        self.object
    }

    pub(crate) fn first_char(self) -> u8 {
        self.first_char
    }

    pub(crate) fn last_char(self) -> u8 {
        self.last_char
    }
}

/// I/O を行わず、Type 1 font object 四種に必要な値を検査・正規化する。
pub(crate) fn prepare_type1_font(
    request: PdfType1FontRequest<'_>,
) -> Result<PreparedPdfType1Font<'_>, PdfFontError> {
    if request.embedding != EmbedPolicy::Full {
        return Err(PdfFontError::SubsetEmbeddingUnsupported);
    }
    validate_descriptor_flags(request.descriptor_flags, request.afm)?;
    validate_font_bbox(request.afm)?;
    let lengths = validate_program(request.program)?;
    let stem_v = resolve_stem_v(request.afm, request.missing_stem_v)?;
    let (first_char, last_char, widths) =
        collect_widths(request.afm, request.encoding, request.used_codes)?;
    validate_pdf_numbers(request.afm, stem_v, &widths)?;
    validate_pdf_name(
        request.afm.descriptor.font_name.as_bytes(),
        PdfNameKind::FontName,
    )?;
    if let Some(encoding) = request.encoding {
        for code in 0..=u8::MAX {
            validate_pdf_name(encoding.glyph_name(code), PdfNameKind::EncodingGlyph(code))?;
        }
    }

    let base_font_name = pdf_name(request.afm.descriptor.font_name.as_bytes());
    let descriptor_prefix = build_descriptor_prefix(
        request.afm,
        request.descriptor_flags,
        &base_font_name,
        stem_v,
    );
    let encoding_body = request.encoding.map(build_encoding_body);

    Ok(PreparedPdfType1Font {
        program: &request.program.bytes,
        lengths,
        descriptor_prefix,
        base_font_name,
        encoding_body,
        first_char,
        last_char,
        widths,
    })
}

impl PreparedPdfType1Font<'_> {
    /// FontFile、FontDescriptor、任意 Encoding、Type 1 Font を一度ずつ書く。
    pub(crate) fn write<W: Write>(
        self,
        writer: &mut PdfWriter<W>,
    ) -> Result<PdfType1Font, PdfFontError> {
        let font_file = writer.reserve_object()?;
        let descriptor = writer.reserve_object()?;
        let encoding = if self.encoding_body.is_some() {
            Some(writer.reserve_object()?)
        } else {
            None
        };
        let font = writer.reserve_object()?;

        let font_file_dictionary = format!(
            "/Length1 {}\n/Length2 {}\n/Length3 {}",
            self.lengths[0], self.lengths[1], self.lengths[2]
        );
        writer.write_stream(font_file, font_file_dictionary.as_bytes(), self.program)?;

        let mut descriptor_body = self.descriptor_prefix;
        descriptor_body
            .extend_from_slice(format!("\n/FontFile {} 0 R\n>>", font_file.number()).as_bytes());
        writer.write_object(descriptor, &descriptor_body)?;

        if let (Some(encoding_object), Some(encoding_body)) = (encoding, self.encoding_body) {
            writer.write_object(encoding_object, &encoding_body)?;
        }

        let mut font_body = b"<<\n/Type /Font\n/Subtype /Type1\n/BaseFont ".to_vec();
        font_body.extend_from_slice(&self.base_font_name);
        font_body.extend_from_slice(
            format!(
                "\n/FirstChar {}\n/LastChar {}\n/Widths [",
                self.first_char, self.last_char
            )
            .as_bytes(),
        );
        for (index, width) in self.widths.iter().enumerate() {
            if index != 0 {
                font_body.push(b' ');
            }
            font_body.extend_from_slice(width.to_string().as_bytes());
        }
        font_body.extend_from_slice(
            format!("]\n/FontDescriptor {} 0 R", descriptor.number()).as_bytes(),
        );
        if let Some(encoding) = encoding {
            font_body
                .extend_from_slice(format!("\n/Encoding {} 0 R", encoding.number()).as_bytes());
        }
        font_body.extend_from_slice(b"\n>>");
        writer.write_object(font, &font_body)?;

        Ok(PdfType1Font {
            object: font,
            first_char: self.first_char,
            last_char: self.last_char,
        })
    }
}

fn validate_program(program: &Type1FontProgram) -> Result<[i32; 3], PdfFontError> {
    let declared_length = program
        .length1
        .checked_add(program.length2)
        .and_then(|length| length.checked_add(program.length3))
        .ok_or(PdfFontError::ProgramLengthOverflow)?;
    if declared_length != program.bytes.len() {
        return Err(PdfFontError::ProgramLengthMismatch {
            declared: declared_length,
            actual: program.bytes.len(),
        });
    }

    let lengths = [program.length1, program.length2, program.length3];
    let mut pdf_lengths = [0_i32; 3];
    for (index, length) in lengths.into_iter().enumerate() {
        pdf_lengths[index] =
            i32::try_from(length).map_err(|_| PdfFontError::ProgramPartTooLarge {
                part: index + 1,
                length,
            })?;
    }
    i32::try_from(program.bytes.len()).map_err(|_| PdfFontError::ProgramTooLarge {
        length: program.bytes.len(),
    })?;
    Ok(pdf_lengths)
}

fn validate_descriptor_flags(flags: u32, afm: &AfmFont) -> Result<(), PdfFontError> {
    if flags & !ALLOWED_DESCRIPTOR_FLAGS != 0 {
        return Err(PdfFontError::UnsupportedDescriptorFlags {
            flags,
            unsupported: flags & !ALLOWED_DESCRIPTOR_FLAGS,
        });
    }
    let symbolic = flags & SYMBOLIC_FLAG != 0;
    let nonsymbolic = flags & NONSYMBOLIC_FLAG != 0;
    if symbolic == nonsymbolic {
        return Err(PdfFontError::AmbiguousCharacterSetFlags { flags });
    }
    let flag_says_fixed = flags & FIXED_PITCH_FLAG != 0;
    if flag_says_fixed != afm.descriptor.is_fixed_pitch {
        return Err(PdfFontError::FixedPitchFlagMismatch {
            flags,
            afm_is_fixed_pitch: afm.descriptor.is_fixed_pitch,
        });
    }
    if afm.descriptor.encoding_scheme.as_deref() == Some("FontSpecific") && !symbolic {
        return Err(PdfFontError::EncodingSchemeFlagMismatch {
            flags,
            encoding_scheme: "FontSpecific".to_owned(),
        });
    }
    Ok(())
}

fn resolve_stem_v(afm: &AfmFont, policy: MissingStemVPolicy) -> Result<AfmNumber, PdfFontError> {
    let stem_v = match (afm.descriptor.std_vw, policy) {
        (Some(stem_v), _) => stem_v,
        (None, MissingStemVPolicy::Use(stem_v)) => stem_v,
        (None, MissingStemVPolicy::Reject) => return Err(PdfFontError::MissingStemV),
    };
    if stem_v <= AfmNumber::ZERO {
        return Err(PdfFontError::InvalidStemV(stem_v));
    }
    Ok(stem_v)
}

fn validate_font_bbox(afm: &AfmFont) -> Result<(), PdfFontError> {
    let [left, bottom, right, top] = afm.descriptor.font_bbox;
    if left > right || bottom > top {
        return Err(PdfFontError::InvalidFontBoundingBox);
    }
    Ok(())
}

fn validate_pdf_numbers(
    afm: &AfmFont,
    stem_v: AfmNumber,
    widths: &[AfmNumber],
) -> Result<(), PdfFontError> {
    let descriptor = &afm.descriptor;
    for number in descriptor.font_bbox {
        validate_pdf_number(number, "FontBBox")?;
    }
    validate_pdf_number(descriptor.italic_angle, "ItalicAngle")?;
    validate_pdf_number(descriptor.ascender, "Ascent")?;
    validate_pdf_number(descriptor.descender, "Descent")?;
    validate_pdf_number(descriptor.cap_height, "CapHeight")?;
    if let Some(x_height) = descriptor.x_height {
        validate_pdf_number(x_height, "XHeight")?;
    }
    validate_pdf_number(stem_v, "StemV")?;
    if let Some(stem_h) = descriptor.std_hw {
        validate_pdf_number(stem_h, "StemH")?;
    }
    for &width in widths {
        validate_pdf_number(width, "Widths")?;
    }
    Ok(())
}

fn validate_pdf_number(number: AfmNumber, field: &'static str) -> Result<(), PdfFontError> {
    let scaled = number.scaled();
    if scaled % AfmNumber::SCALE == 0 {
        let integer = scaled / AfmNumber::SCALE;
        i32::try_from(integer).map_err(|_| PdfFontError::NumberOutsidePdfRange {
            field,
            value: number,
        })?;
    } else if i128::from(scaled).abs() > MAX_PDF_REAL_SCALED {
        return Err(PdfFontError::NumberOutsidePdfRange {
            field,
            value: number,
        });
    }
    Ok(())
}

fn validate_pdf_name(name: &[u8], kind: PdfNameKind) -> Result<(), PdfFontError> {
    if name.len() > MAX_PDF_NAME_BYTES {
        return Err(PdfFontError::NameTooLong {
            kind,
            length: name.len(),
        });
    }
    Ok(())
}

fn collect_widths(
    afm: &AfmFont,
    encoding: Option<&EncodingVector>,
    used_codes: &[u8],
) -> Result<(u8, u8, Vec<AfmNumber>), PdfFontError> {
    if used_codes.is_empty() {
        return Err(PdfFontError::EmptyCodeSet);
    }

    let mut used = [false; 256];
    for &code in used_codes {
        let slot = &mut used[usize::from(code)];
        if *slot {
            return Err(PdfFontError::DuplicateUsedCode(code));
        }
        *slot = true;
    }
    let first = used
        .iter()
        .position(|is_used| *is_used)
        .and_then(|code| u8::try_from(code).ok())
        .ok_or(PdfFontError::EmptyCodeSet)?;
    let last = used
        .iter()
        .rposition(|is_used| *is_used)
        .and_then(|code| u8::try_from(code).ok())
        .ok_or(PdfFontError::EmptyCodeSet)?;

    let mut widths = Vec::with_capacity(usize::from(last - first) + 1);
    for code in first..=last {
        let (width, glyph_name) = lookup_width(afm, encoding, code)?;
        match (width, used[usize::from(code)]) {
            (Some(width), _) => widths.push(width),
            (None, true) => {
                return Err(PdfFontError::MissingGlyphMetric { code, glyph_name });
            }
            (None, false) => widths.push(AfmNumber::ZERO),
        }
    }
    Ok((first, last, widths))
}

fn lookup_width(
    afm: &AfmFont,
    encoding: Option<&EncodingVector>,
    code: u8,
) -> Result<(Option<AfmNumber>, Option<Vec<u8>>), PdfFontError> {
    if let Some(encoding) = encoding {
        let glyph_name = encoding.glyph_name(code);
        let glyph_name_text = std::str::from_utf8(glyph_name).map_err(|_| {
            PdfFontError::InvalidEncodingGlyphName {
                code,
                glyph_name: glyph_name.to_vec(),
            }
        })?;
        let width = afm
            .metrics_by_name
            .get(glyph_name_text)
            .map(|metric| metric.width_x);
        Ok((width, Some(glyph_name.to_vec())))
    } else {
        Ok((
            afm.metrics_by_code.get(&code).map(|metric| metric.width_x),
            None,
        ))
    }
}

fn build_descriptor_prefix(
    afm: &AfmFont,
    flags: u32,
    font_name: &[u8],
    stem_v: AfmNumber,
) -> Vec<u8> {
    let descriptor = &afm.descriptor;
    let mut body = b"<<\n/Type /FontDescriptor\n/FontName ".to_vec();
    body.extend_from_slice(font_name);
    body.extend_from_slice(
        format!(
            "\n/Flags {flags}\n/FontBBox [{} {} {} {}]\n/ItalicAngle {}\n/Ascent {}\n/Descent {}\n/CapHeight {}",
            descriptor.font_bbox[0],
            descriptor.font_bbox[1],
            descriptor.font_bbox[2],
            descriptor.font_bbox[3],
            descriptor.italic_angle,
            descriptor.ascender,
            descriptor.descender,
            descriptor.cap_height,
        )
        .as_bytes(),
    );
    if let Some(x_height) = descriptor.x_height {
        body.extend_from_slice(format!("\n/XHeight {x_height}").as_bytes());
    }
    body.extend_from_slice(format!("\n/StemV {stem_v}").as_bytes());
    if let Some(stem_h) = descriptor.std_hw {
        body.extend_from_slice(format!("\n/StemH {stem_h}").as_bytes());
    }
    body.extend_from_slice(b"\n/MissingWidth 0");
    body
}

fn build_encoding_body(encoding: &EncodingVector) -> Vec<u8> {
    let mut body = b"<<\n/Type /Encoding\n/Differences [0".to_vec();
    for code in 0..=u8::MAX {
        if code % 16 == 0 {
            body.push(b'\n');
        } else {
            body.push(b' ');
        }
        body.extend_from_slice(&pdf_name(encoding.glyph_name(code)));
    }
    body.extend_from_slice(b"\n]\n>>");
    body
}

fn pdf_name(name: &[u8]) -> Vec<u8> {
    EncodingVector::pdf_name(name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfNameKind {
    FontName,
    EncodingGlyph(u8),
}

#[derive(Debug)]
pub(crate) enum PdfFontError {
    Writer(PdfWriterError),
    SubsetEmbeddingUnsupported,
    EmptyCodeSet,
    DuplicateUsedCode(u8),
    MissingGlyphMetric {
        code: u8,
        glyph_name: Option<Vec<u8>>,
    },
    InvalidEncodingGlyphName {
        code: u8,
        glyph_name: Vec<u8>,
    },
    ProgramLengthOverflow,
    ProgramLengthMismatch {
        declared: usize,
        actual: usize,
    },
    ProgramPartTooLarge {
        part: usize,
        length: usize,
    },
    ProgramTooLarge {
        length: usize,
    },
    UnsupportedDescriptorFlags {
        flags: u32,
        unsupported: u32,
    },
    AmbiguousCharacterSetFlags {
        flags: u32,
    },
    FixedPitchFlagMismatch {
        flags: u32,
        afm_is_fixed_pitch: bool,
    },
    EncodingSchemeFlagMismatch {
        flags: u32,
        encoding_scheme: String,
    },
    MissingStemV,
    InvalidStemV(AfmNumber),
    InvalidFontBoundingBox,
    NumberOutsidePdfRange {
        field: &'static str,
        value: AfmNumber,
    },
    NameTooLong {
        kind: PdfNameKind,
        length: usize,
    },
}

impl fmt::Display for PdfFontError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Writer(error) => error.fmt(formatter),
            Self::SubsetEmbeddingUnsupported => formatter.write_str(
                "Type 1 subset embedding is not implemented; full embedding was not substituted",
            ),
            Self::EmptyCodeSet => formatter.write_str("Type 1 font has no used character codes"),
            Self::DuplicateUsedCode(code) => {
                write!(formatter, "Type 1 character code {code} is listed twice")
            }
            Self::MissingGlyphMetric { code, glyph_name } => {
                write!(formatter, "AFM has no width for character code {code}")?;
                if let Some(name) = glyph_name {
                    write!(formatter, " ({})", String::from_utf8_lossy(name))?;
                }
                Ok(())
            }
            Self::InvalidEncodingGlyphName { code, glyph_name } => write!(
                formatter,
                "encoding glyph name for code {code} is not UTF-8: {:?}",
                glyph_name
            ),
            Self::ProgramLengthOverflow => {
                formatter.write_str("Type 1 program part lengths overflow")
            }
            Self::ProgramLengthMismatch { declared, actual } => write!(
                formatter,
                "Type 1 part lengths total {declared} bytes but program has {actual} bytes"
            ),
            Self::ProgramPartTooLarge { part, length } => write!(
                formatter,
                "Type 1 program part {part} has {length} bytes and does not fit a PDF integer"
            ),
            Self::ProgramTooLarge { length } => write!(
                formatter,
                "Type 1 program has {length} bytes and does not fit a PDF integer"
            ),
            Self::UnsupportedDescriptorFlags { flags, unsupported } => write!(
                formatter,
                "FontDescriptor Flags {flags:#x} contain unsupported bits {unsupported:#x}"
            ),
            Self::AmbiguousCharacterSetFlags { flags } => write!(
                formatter,
                "FontDescriptor Flags {flags:#x} must set exactly one of Symbolic and Nonsymbolic"
            ),
            Self::FixedPitchFlagMismatch {
                flags,
                afm_is_fixed_pitch,
            } => write!(
                formatter,
                "FontDescriptor Flags {flags:#x} disagree with AFM IsFixedPitch={afm_is_fixed_pitch}"
            ),
            Self::EncodingSchemeFlagMismatch {
                flags,
                encoding_scheme,
            } => write!(
                formatter,
                "FontDescriptor Flags {flags:#x} are not Symbolic for AFM EncodingScheme {encoding_scheme}"
            ),
            Self::MissingStemV => formatter.write_str(
                "AFM has no StdVW; a deliberate PDF StemV fallback is required",
            ),
            Self::InvalidStemV(stem_v) => {
                write!(formatter, "PDF StemV must be positive, not {stem_v}")
            }
            Self::InvalidFontBoundingBox => {
                formatter.write_str("AFM FontBBox is not an ordered rectangle")
            }
            Self::NumberOutsidePdfRange { field, value } => {
                write!(formatter, "{field} value {value} is outside the PDF 1.4 number range")
            }
            Self::NameTooLong { kind, length } => write!(
                formatter,
                "{kind:?} has {length} bytes, above the PDF 1.4 127-byte name limit"
            ),
        }
    }
}

impl std::error::Error for PdfFontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Writer(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PdfWriterError> for PdfFontError {
    fn from(error: PdfWriterError) -> Self {
        Self::Writer(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_resources::afm::{AfmDescriptor, AfmGlyphMetric};

    fn number(integer: i64) -> AfmNumber {
        AfmNumber::checked_from_integer(integer).unwrap()
    }

    fn metric(code: Option<u8>, name: &str, width: AfmNumber) -> AfmGlyphMetric {
        AfmGlyphMetric {
            code,
            name: Some(name.to_owned()),
            width_x: width,
        }
    }

    fn synthetic_afm(fixed: bool) -> AfmFont {
        let a = metric(Some(65), "A#glyph", number(500));
        let b = metric(Some(66), "B", AfmNumber::from_scaled(625_250_000));
        let c = metric(Some(67), "C", number(750));
        let metrics_by_name = [a.clone(), b.clone(), c.clone()]
            .into_iter()
            .map(|metric| (metric.name.clone().unwrap(), metric))
            .collect();
        let metrics_by_code = [(65, a), (66, b), (67, c)].into_iter().collect();
        AfmFont {
            descriptor: AfmDescriptor {
                font_name: "Synthetic/Font#One".to_owned(),
                encoding_scheme: Some("FontSpecific".to_owned()),
                font_bbox: [number(-10), number(-200), number(1000), number(900)],
                italic_angle: AfmNumber::from_scaled(-12_500_000),
                is_fixed_pitch: fixed,
                cap_height: number(700),
                x_height: Some(number(450)),
                ascender: number(750),
                descender: number(-250),
                std_vw: Some(number(80)),
                std_hw: Some(number(70)),
            },
            metrics_by_name,
            metrics_by_code,
        }
    }

    fn synthetic_program() -> Type1FontProgram {
        Type1FontProgram {
            bytes: b"ascii\0bintail".to_vec(),
            length1: 5,
            length2: 4,
            length3: 4,
        }
    }

    fn synthetic_encoding() -> EncodingVector {
        let mut source = b"/SyntheticEncoding [".to_vec();
        for code in 0..=u8::MAX {
            let name = match code {
                65 => "A#glyph",
                66 => "B",
                67 => "C",
                _ => ".notdef",
            };
            source.extend_from_slice(format!(" /{name}").as_bytes());
        }
        source.extend_from_slice(b" ] def");
        EncodingVector::parse(&source).unwrap()
    }

    fn request<'a>(
        program: &'a Type1FontProgram,
        afm: &'a AfmFont,
        encoding: Option<&'a EncodingVector>,
        embedding: EmbedPolicy,
        used_codes: &'a [u8],
    ) -> PdfType1FontRequest<'a> {
        PdfType1FontRequest {
            program,
            afm,
            encoding,
            embedding,
            descriptor_flags: SYMBOLIC_FLAG | SERIF_FLAG,
            missing_stem_v: MissingStemVPolicy::Reject,
            used_codes,
        }
    }

    #[test]
    fn pfb三部とafmを四つのpdf_objectへ写す() {
        let program = synthetic_program();
        let afm = synthetic_afm(false);
        let prepared =
            prepare_type1_font(request(&program, &afm, None, EmbedPolicy::Full, &[65, 66]))
                .unwrap();
        let mut writer = PdfWriter::new(Vec::new()).unwrap();
        let font = prepared.write(&mut writer).unwrap();
        assert_eq!(font.object().number(), 3);
        assert_eq!((font.first_char(), font.last_char()), (65, 66));
        let pdf = writer.finish(font.object()).unwrap();
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("/Length1 5\n/Length2 4\n/Length3 4\n/Length 13"));
        assert!(text.contains("/FontName /Synthetic#2FFont#23One"));
        assert!(text.contains("/FontBBox [-10 -200 1000 900]"));
        assert!(text.contains("/ItalicAngle -12.5"));
        assert!(text.contains("/FontFile 1 0 R"));
        assert!(text.contains("/BaseFont /Synthetic#2FFont#23One"));
        assert!(text.contains("/FirstChar 65\n/LastChar 66\n/Widths [500 625.25]"));
        assert!(text.contains("/FontDescriptor 2 0 R"));
        assert!(!text.contains("/Encoding "));
    }

    #[test]
    fn encodingのglyph名をescapeして範囲内のafm幅を並べる() {
        let program = synthetic_program();
        let afm = synthetic_afm(false);
        let encoding = synthetic_encoding();
        let prepared = prepare_type1_font(request(
            &program,
            &afm,
            Some(&encoding),
            EmbedPolicy::Full,
            &[65, 67],
        ))
        .unwrap();
        let mut writer = PdfWriter::new(Vec::new()).unwrap();
        let font = prepared.write(&mut writer).unwrap();
        assert_eq!(font.object().number(), 4);
        let pdf = writer.finish(font.object()).unwrap();
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("/Differences [0\n/.notdef"));
        assert!(text.contains("/A#23glyph /B /C"));
        assert!(text.contains("/Widths [500 625.25 750]"));
        assert!(text.contains("/Encoding 3 0 R"));
    }

    #[test]
    fn 部分埋め込み要求を完全埋め込みへ変えない() {
        let program = synthetic_program();
        let afm = synthetic_afm(false);
        assert!(matches!(
            prepare_type1_font(request(&program, &afm, None, EmbedPolicy::Subset, &[65])),
            Err(PdfFontError::SubsetEmbeddingUnsupported)
        ));
    }

    #[test]
    fn 使用文字の重複と使用glyphのafm幅欠損を拒む() {
        let program = synthetic_program();
        let afm = synthetic_afm(false);
        assert!(matches!(
            prepare_type1_font(request(&program, &afm, None, EmbedPolicy::Full, &[65, 65])),
            Err(PdfFontError::DuplicateUsedCode(65))
        ));

        let mut with_gap = synthetic_afm(false);
        with_gap.metrics_by_code.remove(&66);
        let prepared = prepare_type1_font(request(
            &program,
            &with_gap,
            None,
            EmbedPolicy::Full,
            &[65, 67],
        ))
        .unwrap();
        assert_eq!(prepared.widths, [number(500), AfmNumber::ZERO, number(750)]);
        assert!(matches!(
            prepare_type1_font(request(&program, &afm, None, EmbedPolicy::Full, &[68])),
            Err(PdfFontError::MissingGlyphMetric {
                code: 68,
                glyph_name: None
            })
        ));
    }

    #[test]
    fn descriptorの旗をafmと公開仕様に照らす() {
        let program = synthetic_program();
        let afm = synthetic_afm(true);
        let mut invalid = request(&program, &afm, None, EmbedPolicy::Full, &[65]);
        invalid.descriptor_flags = SYMBOLIC_FLAG;
        assert!(matches!(
            prepare_type1_font(invalid),
            Err(PdfFontError::FixedPitchFlagMismatch { .. })
        ));

        let afm = synthetic_afm(false);
        let mut ambiguous = request(&program, &afm, None, EmbedPolicy::Full, &[65]);
        ambiguous.descriptor_flags = SYMBOLIC_FLAG | NONSYMBOLIC_FLAG;
        assert!(matches!(
            prepare_type1_font(ambiguous),
            Err(PdfFontError::AmbiguousCharacterSetFlags { .. })
        ));
    }

    #[test]
    fn afmにstemvがなければ明示値だけを使う() {
        let program = synthetic_program();
        let mut afm = synthetic_afm(false);
        afm.descriptor.std_vw = None;
        assert!(matches!(
            prepare_type1_font(request(&program, &afm, None, EmbedPolicy::Full, &[65])),
            Err(PdfFontError::MissingStemV)
        ));

        let mut with_fallback = request(&program, &afm, None, EmbedPolicy::Full, &[65]);
        with_fallback.missing_stem_v = MissingStemVPolicy::Use(number(81));
        let prepared = prepare_type1_font(with_fallback).unwrap();
        let mut writer = PdfWriter::new(Vec::new()).unwrap();
        let font = prepared.write(&mut writer).unwrap();
        let pdf = writer.finish(font.object()).unwrap();
        assert!(String::from_utf8_lossy(&pdf).contains("/StemV 81"));
    }

    #[test]
    fn pfbの三部長と実dataの不一致を拒む() {
        let mut program = synthetic_program();
        program.length3 = 3;
        let afm = synthetic_afm(false);
        assert!(matches!(
            prepare_type1_font(request(&program, &afm, None, EmbedPolicy::Full, &[65])),
            Err(PdfFontError::ProgramLengthMismatch {
                declared: 12,
                actual: 13
            })
        ));
    }

    #[test]
    fn pdf数値と名前の上限を出力前に検査する() {
        let program = synthetic_program();
        let mut afm = synthetic_afm(false);
        afm.descriptor.cap_height = AfmNumber::from_scaled(32_768_500_000);
        assert!(matches!(
            prepare_type1_font(request(&program, &afm, None, EmbedPolicy::Full, &[65])),
            Err(PdfFontError::NumberOutsidePdfRange {
                field: "CapHeight",
                ..
            })
        ));

        let mut afm = synthetic_afm(false);
        afm.descriptor.font_name = "x".repeat(128);
        assert!(matches!(
            prepare_type1_font(request(&program, &afm, None, EmbedPolicy::Full, &[65])),
            Err(PdfFontError::NameTooLong {
                kind: PdfNameKind::FontName,
                length: 128
            })
        ));
    }
}

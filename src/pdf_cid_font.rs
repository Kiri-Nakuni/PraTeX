//! PDF 1.4の非埋込みType 0 / CIDFontType0 / ToUnicode objectを組み立てる層。
//!
//! Adobe *PDF Reference, Third Edition, version 1.4* の5.6節に従い、明示profileを
//! `/Encoding /UniJIS-UCS2-H` とAdobe-Japan1-4のdescendantへ写す。JFMの幅は
//! shipout位置にだけ使い、CID fontの`/DW`や`/W`へ推測して複製しない。
//! <https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf>

use crate::font_resources::encoding::EncodingVector;
use crate::font_resources::named_cid::NamedCidFontProfile;
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
const ALLOWED_DESCRIPTOR_FLAGS: u32 = FIXED_PITCH_FLAG
    | SERIF_FLAG
    | SYMBOLIC_FLAG
    | SCRIPT_FLAG
    | NONSYMBOLIC_FLAG
    | ITALIC_FLAG
    | ALL_CAP_FLAG
    | SMALL_CAP_FLAG
    | FORCE_BOLD_FLAG;
const MAX_PDF_NAME_BYTES: usize = 127;

/// PDF object予約前に全profile値を検査したnamed CID font。
pub(crate) struct PreparedPdfNamedCidFont {
    descriptor_body: Vec<u8>,
    descendant_prefix: Vec<u8>,
    type0_base_font: Vec<u8>,
    encoding_name: &'static str,
    to_unicode_cmap: &'static [u8],
}

/// Page resourceから参照する、書き込み済みType 0 fontの型付きhandle。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PdfNamedCidFont {
    object: PdfObjectId,
    document_identity: u64,
}

impl PdfNamedCidFont {
    pub(crate) fn object(self) -> PdfObjectId {
        self.object
    }

    pub(crate) fn belongs_to(self, document_identity: u64) -> bool {
        self.document_identity == document_identity
    }

    pub(crate) fn bind_to_document(mut self, document_identity: u64) -> Self {
        self.document_identity = document_identity;
        self
    }
}

pub(crate) fn prepare_named_cid_font(
    profile: &NamedCidFontProfile,
) -> Result<PreparedPdfNamedCidFont, PdfNamedCidFontError> {
    validate_flags(profile.flags())?;
    let bbox = profile.font_bbox();
    if bbox[0] >= bbox[2] || bbox[1] >= bbox[3] {
        return Err(PdfNamedCidFontError::InvalidFontBoundingBox(bbox));
    }
    if profile.stem_v() <= 0 {
        return Err(PdfNamedCidFontError::InvalidStemV(profile.stem_v()));
    }
    if profile.default_width() <= 0 {
        return Err(PdfNamedCidFontError::InvalidDefaultWidth(
            profile.default_width(),
        ));
    }

    let base_font = profile.base_font().as_bytes();
    validate_name_length(base_font, PdfNamedCidNameKind::BaseFont)?;
    let encoding = profile.encoding();
    let mut composite_name = base_font.to_vec();
    composite_name.push(b'-');
    composite_name.extend_from_slice(encoding.pdf_name().as_bytes());
    validate_name_length(&composite_name, PdfNamedCidNameKind::Type0BaseFont)?;

    let escaped_base = EncodingVector::pdf_name(base_font);
    let type0_base_font = EncodingVector::pdf_name(&composite_name);
    let mut descriptor_body = b"<<\n/Type /FontDescriptor\n/FontName ".to_vec();
    descriptor_body.extend_from_slice(&escaped_base);
    descriptor_body.extend_from_slice(
        format!(
            "\n/Flags {}\n/FontBBox [{} {} {} {}]\n/ItalicAngle {}\n/Ascent {}\n/Descent {}\n/CapHeight {}\n/StemV {}\n>>",
            profile.flags(),
            bbox[0],
            bbox[1],
            bbox[2],
            bbox[3],
            profile.italic_angle(),
            profile.ascent(),
            profile.descent(),
            profile.cap_height(),
            profile.stem_v(),
        )
        .as_bytes(),
    );

    let mut descendant_prefix = b"<<\n/Type /Font\n/Subtype /CIDFontType0\n/BaseFont ".to_vec();
    descendant_prefix.extend_from_slice(&escaped_base);
    descendant_prefix.extend_from_slice(
        format!(
            "\n/CIDSystemInfo << /Registry ({}) /Ordering ({}) /Supplement {} >>\n/DW {}\n/FontDescriptor ",
            encoding.registry(),
            encoding.ordering(),
            encoding.supplement(),
            profile.default_width(),
        )
        .as_bytes(),
    );

    Ok(PreparedPdfNamedCidFont {
        descriptor_body,
        descendant_prefix,
        type0_base_font,
        encoding_name: encoding.pdf_name(),
        to_unicode_cmap: encoding.to_unicode_cmap(),
    })
}

impl PreparedPdfNamedCidFont {
    /// FontDescriptor、CIDFontType0、Type0、ToUnicodeを一度ずつ書く。FontFileは作らない。
    pub(crate) fn write<W: Write>(
        self,
        writer: &mut PdfWriter<W>,
    ) -> Result<PdfNamedCidFont, PdfNamedCidFontError> {
        let descriptor = writer.reserve_object()?;
        let descendant = writer.reserve_object()?;
        let type0 = writer.reserve_object()?;
        let to_unicode = writer.reserve_object()?;

        writer.write_object(descriptor, &self.descriptor_body)?;

        let mut descendant_body = self.descendant_prefix;
        descendant_body.extend_from_slice(format!("{} 0 R\n>>", descriptor.number()).as_bytes());
        writer.write_object(descendant, &descendant_body)?;

        writer.write_stream(to_unicode, b"", self.to_unicode_cmap)?;

        let mut type0_body = b"<<\n/Type /Font\n/Subtype /Type0\n/BaseFont ".to_vec();
        type0_body.extend_from_slice(&self.type0_base_font);
        type0_body.extend_from_slice(
            format!(
                "\n/Encoding /{}\n/DescendantFonts [{} 0 R]\n/ToUnicode {} 0 R\n>>",
                self.encoding_name,
                descendant.number(),
                to_unicode.number(),
            )
            .as_bytes(),
        );
        writer.write_object(type0, &type0_body)?;

        Ok(PdfNamedCidFont {
            object: type0,
            document_identity: 0,
        })
    }
}

fn validate_flags(flags: u32) -> Result<(), PdfNamedCidFontError> {
    let unsupported = flags & !ALLOWED_DESCRIPTOR_FLAGS;
    if unsupported != 0 {
        return Err(PdfNamedCidFontError::UnsupportedDescriptorFlags { flags, unsupported });
    }
    if flags & SYMBOLIC_FLAG == 0 || flags & NONSYMBOLIC_FLAG != 0 {
        return Err(PdfNamedCidFontError::SymbolicFlagRequired(flags));
    }
    Ok(())
}

fn validate_name_length(
    name: &[u8],
    kind: PdfNamedCidNameKind,
) -> Result<(), PdfNamedCidFontError> {
    if name.is_empty() {
        return Err(PdfNamedCidFontError::EmptyName(kind));
    }
    if name.len() > MAX_PDF_NAME_BYTES {
        return Err(PdfNamedCidFontError::NameTooLong {
            kind,
            length: name.len(),
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfNamedCidNameKind {
    BaseFont,
    Type0BaseFont,
}

#[derive(Debug)]
pub(crate) enum PdfNamedCidFontError {
    Writer(PdfWriterError),
    UnsupportedDescriptorFlags {
        flags: u32,
        unsupported: u32,
    },
    SymbolicFlagRequired(u32),
    InvalidFontBoundingBox([i32; 4]),
    InvalidStemV(i32),
    InvalidDefaultWidth(i32),
    EmptyName(PdfNamedCidNameKind),
    NameTooLong {
        kind: PdfNamedCidNameKind,
        length: usize,
    },
}

impl fmt::Display for PdfNamedCidFontError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Writer(error) => error.fmt(formatter),
            Self::UnsupportedDescriptorFlags { flags, unsupported } => write!(
                formatter,
                "named CID FontDescriptor Flags {flags:#x} contain unsupported bits {unsupported:#x}"
            ),
            Self::SymbolicFlagRequired(flags) => write!(
                formatter,
                "named CID FontDescriptor Flags {flags:#x} must set Symbolic and clear Nonsymbolic"
            ),
            Self::InvalidFontBoundingBox(bbox) => {
                write!(formatter, "named CID FontBBox {bbox:?} is not an ordered rectangle")
            }
            Self::InvalidStemV(value) => {
                write!(formatter, "named CID StemV must be positive, not {value}")
            }
            Self::InvalidDefaultWidth(value) => write!(
                formatter,
                "named CID DefaultWidth must be positive, not {value}"
            ),
            Self::EmptyName(kind) => write!(formatter, "named CID {kind:?} is empty"),
            Self::NameTooLong { kind, length } => write!(
                formatter,
                "named CID {kind:?} has {length} bytes, above the PDF 1.4 127-byte name limit"
            ),
        }
    }
}

impl std::error::Error for PdfNamedCidFontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Writer(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PdfWriterError> for PdfNamedCidFontError {
    fn from(error: PdfWriterError) -> Self {
        Self::Writer(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with(replacement: Option<(&[u8], &[u8])>) -> NamedCidFontProfile {
        let source = b"PraTeX-Named-CID-Profile 1\n\
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
EndProfile\n";
        let bytes = if let Some((from, to)) = replacement {
            let position = source
                .windows(from.len())
                .position(|window| window == from)
                .unwrap();
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&source[..position]);
            bytes.extend_from_slice(to);
            bytes.extend_from_slice(&source[position + from.len()..]);
            bytes
        } else {
            source.to_vec()
        };
        NamedCidFontProfile::parse(&bytes).unwrap()
    }

    #[test]
    fn named_cidとunicode逆写像を四objectへ写す() {
        let prepared = prepare_named_cid_font(&profile_with(None)).unwrap();
        let mut writer = PdfWriter::new(Vec::new()).unwrap();
        let font = prepared.write(&mut writer).unwrap();
        assert_eq!(font.object().number(), 3);
        let pdf = writer.finish(font.object()).unwrap();
        let text = String::from_utf8_lossy(&pdf);

        for required in [
            "/Type /FontDescriptor",
            "/FontName /HeiseiMin-W3",
            "/Flags 6",
            "/FontBBox [-123 -257 1001 910]",
            "/ItalicAngle 0",
            "/Ascent 880",
            "/Descent -120",
            "/CapHeight 700",
            "/StemV 80",
            "/Subtype /CIDFontType0",
            "/CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 4 >>",
            "/DW 1000",
            "/FontDescriptor 1 0 R",
            "/Subtype /Type0",
            "/BaseFont /HeiseiMin-W3-UniJIS-UCS2-H",
            "/Encoding /UniJIS-UCS2-H",
            "/DescendantFonts [2 0 R]",
            "/ToUnicode 4 0 R",
            "/CMapName /PraTeX-UniJIS-UCS2-ToUnicode",
            "/CMapType 2",
            "<0000> <D7FF> <0000>",
            "<E000> <FFFF> <E000>",
        ] {
            assert!(text.contains(required), "missing {required}");
        }
        assert!(!text.contains("/FontFile"));
        assert!(!text.contains("/W ["));
    }

    #[test]
    fn profileのpdf意味違反をobject予約前に拒む() {
        assert!(matches!(
            prepare_named_cid_font(&profile_with(Some((b"Flags 6", b"Flags 32")))),
            Err(PdfNamedCidFontError::SymbolicFlagRequired(32))
        ));
        assert!(matches!(
            prepare_named_cid_font(&profile_with(Some((
                b"FontBBox -123 -257 1001 910",
                b"FontBBox 1001 -257 -123 910"
            )))),
            Err(PdfNamedCidFontError::InvalidFontBoundingBox(_))
        ));
        assert!(matches!(
            prepare_named_cid_font(&profile_with(Some((
                b"DefaultWidth 1000",
                b"DefaultWidth 0"
            )))),
            Err(PdfNamedCidFontError::InvalidDefaultWidth(0))
        ));
    }
}

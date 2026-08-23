//! PDF 1.4 の文書構造と固定小数座標。
//!
//! 低水準の object serializer は [`crate::pdf`] に閉じ込め、この層では公開仕様の
//! Catalog / Pages / Page / Contents の関係だけを組み立てる。組版木の走査やフォント
//! 解決はさらに上の backend の責務である。

use crate::pdf::{PdfObjectId, PdfWriter, PdfWriterError};
use crate::pdf_cid_font::{PdfNamedCidFont, PdfNamedCidFontError, PreparedPdfNamedCidFont};
use crate::pdf_font::{PdfFontError, PdfType1Font, PreparedPdfType1Font};

use std::collections::BTreeSet;
use std::fmt;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_FONT_RESOURCES: usize = 4095;
static NEXT_DOCUMENT_IDENTITY: AtomicU64 = AtomicU64::new(1);

fn allocate_document_identity() -> Result<u64, PdfDocumentError> {
    NEXT_DOCUMENT_IDENTITY
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |identity| {
            identity.checked_add(1)
        })
        .map_err(|_| PdfDocumentError::DocumentIdentityExhausted)
}

/// PDF real を 10^-6 bp 単位で保持する。
///
/// 浮動小数点の指数表記やplatform差をPDFへ流さないため、変換と印字を整数で行う。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PdfCoordinate(i64);

impl PdfCoordinate {
    const UNITS_PER_BP: i128 = 1_000_000;
    pub(crate) const ONE_INCH: Self = Self(72_000_000);

    /// TeXのscaled pointをPDFのdefault user space（bp）へ変換する。
    ///
    /// `1pt = 1/72.27in`、`1bp = 1/72in`、`1pt = 65536sp` とmagをまとめた
    /// `sp * mag * 7200 / (1000 * 65536 * 7227)` を丸める。
    pub(crate) fn from_scaled(scaled: i32, mag: i32) -> Result<Self, PdfDocumentError> {
        let numerator = i128::from(scaled)
            .checked_mul(i128::from(mag))
            .and_then(|value| value.checked_mul(7200))
            .and_then(|value| value.checked_mul(Self::UNITS_PER_BP))
            .ok_or(PdfDocumentError::CoordinateOverflow)?;
        let denominator = 1000_i128 * 65536 * 7227;
        let rounded = if numerator >= 0 {
            (numerator + denominator / 2) / denominator
        } else {
            -((-numerator + denominator / 2) / denominator)
        };
        let value = i64::try_from(rounded).map_err(|_| PdfDocumentError::CoordinateOverflow)?;
        Ok(Self(value))
    }

    pub(crate) fn checked_add(self, other: Self) -> Result<Self, PdfDocumentError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(PdfDocumentError::CoordinateOverflow)
    }

    pub(crate) fn checked_sub(self, other: Self) -> Result<Self, PdfDocumentError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(PdfDocumentError::CoordinateOverflow)
    }

    pub(crate) fn is_positive(self) -> bool {
        self.0 > 0
    }
}

impl fmt::Display for PdfCoordinate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let negative = self.0 < 0;
        let absolute = i128::from(self.0).abs();
        let integer = absolute / Self::UNITS_PER_BP;
        let fraction = absolute % Self::UNITS_PER_BP;
        if negative {
            formatter.write_str("-")?;
        }
        write!(formatter, "{integer}")?;
        if fraction != 0 {
            let fraction = format!("{fraction:06}");
            formatter.write_str(".")?;
            formatter.write_str(fraction.trim_end_matches('0'))?;
        }
        Ok(())
    }
}

/// Standard 14 Courier をページの `/F1` resource として参照する型付きhandle。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PdfCourierFont {
    object: PdfObjectId,
    document_identity: u64,
}

/// Courier以外のpage fontをfirst-use順で一つのresource番号domainへ置く。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfPageFont {
    Type1(PdfType1Font),
    NamedCid(PdfNamedCidFont),
}

impl PdfPageFont {
    pub(crate) fn object(self) -> PdfObjectId {
        match self {
            Self::Type1(font) => font.object(),
            Self::NamedCid(font) => font.object(),
        }
    }

    fn belongs_to(self, document_identity: u64) -> bool {
        match self {
            Self::Type1(font) => font.belongs_to(document_identity),
            Self::NamedCid(font) => font.belongs_to(document_identity),
        }
    }
}

impl From<PdfType1Font> for PdfPageFont {
    fn from(font: PdfType1Font) -> Self {
        Self::Type1(font)
    }
}

impl From<PdfNamedCidFont> for PdfPageFont {
    fn from(font: PdfNamedCidFont) -> Self {
        Self::NamedCid(font)
    }
}

/// 一枚のページを構成するデータ。
pub(crate) struct PdfPage<'a> {
    pub(crate) width: PdfCoordinate,
    pub(crate) height: PdfCoordinate,
    pub(crate) courier_font: Option<PdfCourierFont>,
    /// 配列順に `/F1`、`/F2`、…へ割り当てるtyped font。
    /// Courier があるページでは Courier が `/F1` を保ち、この配列は `/F2` から始まる。
    pub(crate) fonts: &'a [PdfPageFont],
    /// 外側の `<< >>` を含まないresource dictionary entry。
    /// Font resource は型付きfieldだけで指定し、ここに `/Font` nameを書いてはならない。
    pub(crate) resource_entries: &'a [u8],
    pub(crate) content: &'a [u8],
}

/// PDF 1.4の一段page treeを逐次構成する。
pub(crate) struct PdfDocument<W: Write> {
    writer: PdfWriter<W>,
    catalog: PdfObjectId,
    pages: PdfObjectId,
    page_ids: Vec<PdfObjectId>,
    document_identity: u64,
    courier_font: Option<PdfCourierFont>,
    fonts: Vec<PdfPageFont>,
}

impl<W: Write> PdfDocument<W> {
    pub(crate) fn new(target: W) -> Result<Self, PdfDocumentError> {
        let document_identity = allocate_document_identity()?;
        let mut writer = PdfWriter::new(target)?;
        let catalog = writer.reserve_object()?;
        let pages = writer.reserve_object()?;
        Ok(Self {
            writer,
            catalog,
            pages,
            page_ids: Vec::new(),
            document_identity,
            courier_font: None,
            fonts: Vec::new(),
        })
    }

    /// Standard 14 Courier object を一度だけ作り、ページ用の型付きhandleを返す。
    pub(crate) fn add_standard_courier_font(&mut self) -> Result<PdfCourierFont, PdfDocumentError> {
        if let Some(font) = self.courier_font {
            return Ok(font);
        }
        let object = self.writer.reserve_object()?;
        self.writer.write_object(
            object,
            b"<<\n/Type /Font\n/Subtype /Type1\n/BaseFont /Courier\n/Encoding /WinAnsiEncoding\n>>",
        )?;
        let font = PdfCourierFont {
            object,
            document_identity: self.document_identity,
        };
        self.courier_font = Some(font);
        Ok(font)
    }

    /// 準備済みType 1 font object群をこの文書へ一度だけ書き、page用handleを返す。
    ///
    /// 返したhandleはfont sizeを含まないため、任意のpageとsizeから繰り返し参照できる。
    pub(crate) fn add_type1_font(
        &mut self,
        prepared: PreparedPdfType1Font<'_>,
    ) -> Result<PdfType1Font, PdfDocumentError> {
        let font = prepared
            .write(&mut self.writer)?
            .bind_to_document(self.document_identity);
        self.fonts.push(font.into());
        Ok(font)
    }

    /// 検査済み非埋込みCID font object群をこの文書へ一度だけ書く。
    pub(crate) fn add_named_cid_font(
        &mut self,
        prepared: PreparedPdfNamedCidFont,
    ) -> Result<PdfNamedCidFont, PdfDocumentError> {
        let font = prepared
            .write(&mut self.writer)?
            .bind_to_document(self.document_identity);
        self.fonts.push(font.into());
        Ok(font)
    }

    pub(crate) fn add_page(&mut self, page: PdfPage<'_>) -> Result<(), PdfDocumentError> {
        if !page.width.is_positive() || !page.height.is_positive() {
            return Err(PdfDocumentError::InvalidPageSize {
                width: page.width,
                height: page.height,
            });
        }
        self.validate_page_font_resources(&page)?;

        let page_id = self.writer.reserve_object()?;
        let content_id = self.writer.reserve_object()?;
        self.writer.write_stream(content_id, b"", page.content)?;

        let mut body = format!(
            "<<\n/Type /Page\n/Parent {} 0 R\n/MediaBox [0 0 {} {}]\n/Resources <<",
            self.pages.number(),
            page.width,
            page.height,
        )
        .into_bytes();
        append_font_resources(&mut body, page.courier_font, page.fonts);
        if !page.resource_entries.is_empty() {
            body.push(b'\n');
            body.extend_from_slice(page.resource_entries);
        }
        body.extend_from_slice(
            format!("\n>>\n/Contents {} 0 R\n>>", content_id.number()).as_bytes(),
        );
        self.writer.write_object(page_id, &body)?;
        self.page_ids.push(page_id);
        Ok(())
    }

    fn validate_page_font_resources(&self, page: &PdfPage<'_>) -> Result<(), PdfDocumentError> {
        if raw_resources_contain_font_name(page.resource_entries) {
            return Err(PdfDocumentError::RawFontResourceCollision);
        }
        let font_count = page
            .fonts
            .len()
            .checked_add(usize::from(page.courier_font.is_some()))
            .ok_or(PdfDocumentError::TooManyFontResources(usize::MAX))?;
        if font_count > MAX_FONT_RESOURCES {
            return Err(PdfDocumentError::TooManyFontResources(font_count));
        }
        if let Some(courier) = page.courier_font {
            if courier.document_identity != self.document_identity
                || self.courier_font != Some(courier)
            {
                return Err(PdfDocumentError::UnknownCourierFont(courier));
            }
        }

        let mut seen = BTreeSet::new();
        for &font in page.fonts {
            if !font.belongs_to(self.document_identity) || !self.fonts.contains(&font) {
                return Err(match font {
                    PdfPageFont::Type1(font) => PdfDocumentError::UnknownType1Font(font),
                    PdfPageFont::NamedCid(font) => PdfDocumentError::UnknownNamedCidFont(font),
                });
            }
            if !seen.insert(font.object().number()) {
                return Err(match font {
                    PdfPageFont::Type1(font) => PdfDocumentError::DuplicateType1Font(font),
                    PdfPageFont::NamedCid(font) => PdfDocumentError::DuplicateNamedCidFont(font),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn page_count(&self) -> usize {
        self.page_ids.len()
    }

    pub(crate) fn finish(mut self) -> Result<W, PdfDocumentError> {
        let mut kids = Vec::new();
        for page in &self.page_ids {
            if !kids.is_empty() {
                kids.push(b' ');
            }
            kids.extend_from_slice(format!("{} 0 R", page.number()).as_bytes());
        }
        let pages = format!(
            "<<\n/Type /Pages\n/Kids [{}]\n/Count {}\n>>",
            String::from_utf8(kids).expect("page references are ASCII"),
            self.page_ids.len(),
        );
        self.writer.write_object(self.pages, pages.as_bytes())?;

        let catalog = format!("<<\n/Type /Catalog\n/Pages {} 0 R\n>>", self.pages.number());
        self.writer.write_object(self.catalog, catalog.as_bytes())?;
        Ok(self.writer.finish(self.catalog)?)
    }
}

fn append_font_resources(
    body: &mut Vec<u8>,
    courier: Option<PdfCourierFont>,
    fonts: &[PdfPageFont],
) {
    if courier.is_none() && fonts.is_empty() {
        return;
    }
    body.extend_from_slice(b"\n/Font <<");
    let mut resource_number = 1usize;
    if let Some(courier) = courier {
        body.extend_from_slice(
            format!("\n/F{resource_number} {} 0 R", courier.object.number()).as_bytes(),
        );
        resource_number += 1;
    }
    for font in fonts {
        body.extend_from_slice(
            format!("\n/F{resource_number} {} 0 R", font.object().number()).as_bytes(),
        );
        resource_number += 1;
    }
    body.extend_from_slice(b"\n>>");
}

/// Raw resource entryを保ったままFontだけ型付き側へ一本化するための限定scanner。
/// Comment、literal string、hex string内の`/Font`はresource keyではないので飛ばす。
fn raw_resources_contain_font_name(bytes: &[u8]) -> bool {
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'%' => {
                cursor += 1;
                while cursor < bytes.len() && !matches!(bytes[cursor], b'\r' | b'\n') {
                    cursor += 1;
                }
            }
            b'(' => cursor = skip_pdf_literal_string(bytes, cursor + 1),
            b'<' if bytes.get(cursor + 1) != Some(&b'<') => {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor] != b'>' {
                    cursor += 1;
                }
                cursor = cursor.saturating_add(1).min(bytes.len());
            }
            b'/' => {
                let start = cursor + 1;
                cursor = start;
                while cursor < bytes.len()
                    && !is_pdf_whitespace(bytes[cursor])
                    && !is_pdf_delimiter(bytes[cursor])
                {
                    cursor += 1;
                }
                if pdf_name_equals(&bytes[start..cursor], b"Font") {
                    return true;
                }
            }
            _ => cursor += 1,
        }
    }
    false
}

fn skip_pdf_literal_string(bytes: &[u8], mut cursor: usize) -> usize {
    let mut depth = 1usize;
    while cursor < bytes.len() && depth != 0 {
        match bytes[cursor] {
            b'\\' => {
                cursor += 1;
                if bytes.get(cursor) == Some(&b'\r') {
                    cursor += 1;
                    if bytes.get(cursor) == Some(&b'\n') {
                        cursor += 1;
                    }
                } else if cursor < bytes.len() {
                    cursor += 1;
                }
            }
            b'(' => {
                depth += 1;
                cursor += 1;
            }
            b')' => {
                depth -= 1;
                cursor += 1;
            }
            _ => cursor += 1,
        }
    }
    cursor
}

fn pdf_name_equals(encoded: &[u8], expected: &[u8]) -> bool {
    let mut decoded = Vec::with_capacity(encoded.len());
    let mut cursor = 0;
    while cursor < encoded.len() {
        if encoded[cursor] == b'#' && cursor + 2 < encoded.len() {
            if let (Some(high), Some(low)) = (
                hex_value(encoded[cursor + 1]),
                hex_value(encoded[cursor + 2]),
            ) {
                decoded.push((high << 4) | low);
                cursor += 3;
                continue;
            }
        }
        decoded.push(encoded[cursor]);
        cursor += 1;
    }
    decoded == expected
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn is_pdf_whitespace(byte: u8) -> bool {
    matches!(byte, b'\0' | b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

fn is_pdf_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

#[derive(Debug)]
pub(crate) enum PdfDocumentError {
    Writer(PdfWriterError),
    Font(PdfFontError),
    NamedCidFont(PdfNamedCidFontError),
    DocumentIdentityExhausted,
    CoordinateOverflow,
    InvalidPageSize {
        width: PdfCoordinate,
        height: PdfCoordinate,
    },
    UnknownCourierFont(PdfCourierFont),
    UnknownType1Font(PdfType1Font),
    DuplicateType1Font(PdfType1Font),
    UnknownNamedCidFont(PdfNamedCidFont),
    DuplicateNamedCidFont(PdfNamedCidFont),
    TooManyFontResources(usize),
    RawFontResourceCollision,
}

impl fmt::Display for PdfDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Writer(error) => error.fmt(formatter),
            Self::Font(error) => error.fmt(formatter),
            Self::NamedCidFont(error) => error.fmt(formatter),
            Self::DocumentIdentityExhausted => {
                formatter.write_str("PDF document identity space is exhausted")
            }
            Self::CoordinateOverflow => formatter.write_str("PDF coordinate overflow"),
            Self::InvalidPageSize { width, height } => {
                write!(formatter, "invalid PDF page size {width} by {height}")
            }
            Self::UnknownCourierFont(font) => write!(
                formatter,
                "Courier font object {} does not belong to this PDF document",
                font.object.number()
            ),
            Self::UnknownType1Font(font) => write!(
                formatter,
                "Type 1 font object {} does not belong to this PDF document",
                font.object().number()
            ),
            Self::DuplicateType1Font(font) => write!(
                formatter,
                "Type 1 font object {} occurs twice in one page resource",
                font.object().number()
            ),
            Self::UnknownNamedCidFont(font) => write!(
                formatter,
                "named CID font object {} does not belong to this PDF document",
                font.object().number()
            ),
            Self::DuplicateNamedCidFont(font) => write!(
                formatter,
                "named CID font object {} occurs twice in one page resource",
                font.object().number()
            ),
            Self::TooManyFontResources(count) => {
                write!(
                    formatter,
                    "page has {count} font resources; PDF 1.4 permits 4095"
                )
            }
            Self::RawFontResourceCollision => formatter.write_str(
                "raw PDF resource entries must not define /Font; use typed font resources",
            ),
        }
    }
}

impl std::error::Error for PdfDocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Writer(error) => Some(error),
            Self::Font(error) => Some(error),
            Self::NamedCidFont(error) => Some(error),
            Self::DocumentIdentityExhausted
            | Self::CoordinateOverflow
            | Self::InvalidPageSize { .. }
            | Self::UnknownCourierFont(_)
            | Self::UnknownType1Font(_)
            | Self::DuplicateType1Font(_)
            | Self::UnknownNamedCidFont(_)
            | Self::DuplicateNamedCidFont(_)
            | Self::TooManyFontResources(_)
            | Self::RawFontResourceCollision => None,
        }
    }
}

impl From<PdfWriterError> for PdfDocumentError {
    fn from(error: PdfWriterError) -> Self {
        Self::Writer(error)
    }
}

impl From<PdfFontError> for PdfDocumentError {
    fn from(error: PdfFontError) -> Self {
        Self::Font(error)
    }
}

impl From<PdfNamedCidFontError> for PdfDocumentError {
    fn from(error: PdfNamedCidFontError) -> Self {
        Self::NamedCidFont(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{PdfCoordinate, PdfDocument, PdfDocumentError, PdfPage, PdfPageFont};
    use crate::font_resources::afm::{AfmDescriptor, AfmFont, AfmGlyphMetric, AfmNumber};
    use crate::font_resources::map::EmbedPolicy;
    use crate::font_resources::named_cid::NamedCidFontProfile;
    use crate::font_resources::type1::Type1FontProgram;
    use crate::pdf_cid_font::{prepare_named_cid_font, PdfNamedCidFont};
    use crate::pdf_font::{
        prepare_type1_font, MissingStemVPolicy, PdfType1Font, PdfType1FontRequest,
    };

    use std::collections::BTreeMap;

    fn afm_number(integer: i64) -> AfmNumber {
        AfmNumber::checked_from_integer(integer).unwrap()
    }

    fn 合成type1を加える(document: &mut PdfDocument<Vec<u8>>, name: &str) -> PdfType1Font {
        let program = Type1FontProgram {
            bytes: b"abc".to_vec(),
            length1: 1,
            length2: 1,
            length3: 1,
        };
        let metric = AfmGlyphMetric {
            code: Some(65),
            name: Some("A".to_owned()),
            width_x: afm_number(500),
        };
        let mut metrics_by_name = BTreeMap::new();
        metrics_by_name.insert("A".to_owned(), metric.clone());
        let mut metrics_by_code = BTreeMap::new();
        metrics_by_code.insert(65, metric);
        let afm = AfmFont {
            descriptor: AfmDescriptor {
                font_name: name.to_owned(),
                encoding_scheme: Some("FontSpecific".to_owned()),
                font_bbox: [
                    afm_number(-10),
                    afm_number(-200),
                    afm_number(1000),
                    afm_number(900),
                ],
                italic_angle: AfmNumber::ZERO,
                is_fixed_pitch: false,
                cap_height: afm_number(700),
                x_height: Some(afm_number(450)),
                ascender: afm_number(750),
                descender: afm_number(-250),
                std_vw: Some(afm_number(80)),
                std_hw: None,
            },
            metrics_by_name,
            metrics_by_code,
        };
        let prepared = prepare_type1_font(PdfType1FontRequest {
            program: &program,
            afm: &afm,
            encoding: None,
            embedding: EmbedPolicy::Full,
            descriptor_flags: 6,
            missing_stem_v: MissingStemVPolicy::Reject,
            used_codes: &[65],
        })
        .unwrap();
        document.add_type1_font(prepared).unwrap()
    }

    fn 合成named_cidを加える(document: &mut PdfDocument<Vec<u8>>) -> PdfNamedCidFont {
        let profile = NamedCidFontProfile::parse(
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
EndProfile\n",
        )
        .unwrap();
        document
            .add_named_cid_font(prepare_named_cid_font(&profile).unwrap())
            .unwrap()
    }

    #[test]
    fn scaled_pointを固定小数bpへ変換する() {
        let one_point = PdfCoordinate::from_scaled(65536, 1000).unwrap();
        assert_eq!(one_point.to_string(), "0.996264");
        let negative = PdfCoordinate::from_scaled(-65536, 1000).unwrap();
        assert_eq!(negative.to_string(), "-0.996264");
        let magnified = PdfCoordinate::from_scaled(65536, 1200).unwrap();
        assert_eq!(magnified.to_string(), "1.195517");
    }

    #[test]
    fn 一ページの必須objectを結ぶ() {
        let mut document = PdfDocument::new(Vec::new()).unwrap();
        document
            .add_page(PdfPage {
                width: PdfCoordinate(612_000_000),
                height: PdfCoordinate(792_000_000),
                courier_font: None,
                fonts: &[],
                resource_entries: b"",
                content: b"0 0 10 20 re f\n",
            })
            .unwrap();
        assert_eq!(document.page_count(), 1);
        let pdf = document.finish().unwrap();

        assert!(pdf
            .windows(b"/Type /Catalog".len())
            .any(|w| w == b"/Type /Catalog"));
        assert!(pdf
            .windows(b"/Type /Pages".len())
            .any(|w| w == b"/Type /Pages"));
        assert!(pdf
            .windows(b"/Type /Page".len())
            .any(|w| w == b"/Type /Page"));
        assert!(pdf
            .windows(b"/Kids [3 0 R]".len())
            .any(|w| w == b"/Kids [3 0 R]"));
        assert!(pdf.windows(b"/Count 1".len()).any(|w| w == b"/Count 1"));
        assert!(pdf
            .windows(b"/Parent 2 0 R".len())
            .any(|w| w == b"/Parent 2 0 R"));
        assert!(pdf
            .windows(b"/MediaBox [0 0 612 792]".len())
            .any(|w| { w == b"/MediaBox [0 0 612 792]" }));
        assert!(pdf
            .windows(b"/Resources <<\n>>".len())
            .any(|w| w == b"/Resources <<\n>>"));
        assert!(pdf
            .windows(b"0 0 10 20 re f\n".len())
            .any(|w| w == b"0 0 10 20 re f\n"));
    }

    #[test]
    fn 複数ページの順序と親を一段treeに保つ() {
        let mut document = PdfDocument::new(Vec::new()).unwrap();
        for content in [b"first".as_slice(), b"second".as_slice()] {
            document
                .add_page(PdfPage {
                    width: PdfCoordinate(10_000_000),
                    height: PdfCoordinate(20_000_000),
                    courier_font: None,
                    fonts: &[],
                    resource_entries: b"/ProcSet [/PDF]",
                    content,
                })
                .unwrap();
        }
        let pdf = document.finish().unwrap();
        assert!(pdf
            .windows(b"/Kids [3 0 R 5 0 R]".len())
            .any(|w| { w == b"/Kids [3 0 R 5 0 R]" }));
        assert!(pdf.windows(b"/Count 2".len()).any(|w| w == b"/Count 2"));
        assert_eq!(
            pdf.windows(b"/Parent 2 0 R".len())
                .filter(|w| *w == b"/Parent 2 0 R")
                .count(),
            2
        );
        let first = pdf.windows(5).position(|w| w == b"first").unwrap();
        let second = pdf.windows(6).position(|w| w == b"second").unwrap();
        assert!(first < second);
    }

    #[test]
    fn 零以下のmedia_boxを拒む() {
        let mut document = PdfDocument::new(Vec::new()).unwrap();
        let error = document
            .add_page(PdfPage {
                width: PdfCoordinate(0),
                height: PdfCoordinate(10_000_000),
                courier_font: None,
                fonts: &[],
                resource_entries: b"",
                content: b"",
            })
            .unwrap_err();
        assert!(matches!(error, PdfDocumentError::InvalidPageSize { .. }));
    }

    #[test]
    fn courierを型付きfont_resourceとして一度だけ作る() {
        let mut document = PdfDocument::new(Vec::new()).unwrap();
        let courier = document.add_standard_courier_font().unwrap();
        assert_eq!(document.add_standard_courier_font().unwrap(), courier);
        document
            .add_page(PdfPage {
                width: PdfCoordinate(10_000_000),
                height: PdfCoordinate(20_000_000),
                courier_font: Some(courier),
                fonts: &[],
                resource_entries: b"",
                content: b"BT /F1 10 Tf (A) Tj ET",
            })
            .unwrap();
        let pdf = document.finish().unwrap();

        for expected in [
            b"/Type /Font".as_slice(),
            b"/Subtype /Type1".as_slice(),
            b"/BaseFont /Courier".as_slice(),
            b"/Encoding /WinAnsiEncoding".as_slice(),
            b"/Font <<\n/F1 3 0 R\n>>".as_slice(),
        ] {
            assert!(pdf.windows(expected.len()).any(|window| window == expected));
        }
        assert_eq!(
            pdf.windows(b"/BaseFont /Courier".len())
                .filter(|window| *window == b"/BaseFont /Courier")
                .count(),
            1
        );
    }

    #[test]
    fn courierの後へtype1を配列順に割り当てる() {
        let mut document = PdfDocument::new(Vec::new()).unwrap();
        let courier = document.add_standard_courier_font().unwrap();
        let first = 合成type1を加える(&mut document, "FirstSynthetic");
        let second = 合成type1を加える(&mut document, "SecondSynthetic");
        let fonts = [PdfPageFont::Type1(first), PdfPageFont::Type1(second)];
        document
            .add_page(PdfPage {
                width: PdfCoordinate(10_000_000),
                height: PdfCoordinate(20_000_000),
                courier_font: Some(courier),
                fonts: &fonts,
                resource_entries: b"",
                content: b"BT /F1 10 Tf (A) Tj /F2 10 Tf (A) Tj /F3 10 Tf (A) Tj ET",
            })
            .unwrap();
        let pdf = document.finish().unwrap();
        assert!(pdf
            .windows(b"/Font <<\n/F1 3 0 R\n/F2 6 0 R\n/F3 9 0 R\n>>".len())
            .any(|window| window == b"/Font <<\n/F1 3 0 R\n/F2 6 0 R\n/F3 9 0 R\n>>"));
    }

    #[test]
    fn courier_type1_named_cidを一つのf番号列へ割り当てる() {
        let mut document = PdfDocument::new(Vec::new()).unwrap();
        let courier = document.add_standard_courier_font().unwrap();
        let type1 = 合成type1を加える(&mut document, "MixedSynthetic");
        let named_cid = 合成named_cidを加える(&mut document);
        let fonts = [PdfPageFont::Type1(type1), PdfPageFont::NamedCid(named_cid)];
        document
            .add_page(PdfPage {
                width: PdfCoordinate(10_000_000),
                height: PdfCoordinate(20_000_000),
                courier_font: Some(courier),
                fonts: &fonts,
                resource_entries: b"",
                content: b"BT /F1 10 Tf (A) Tj /F2 10 Tf <41> Tj /F3 10 Tf <3042> Tj ET",
            })
            .unwrap();
        let pdf = document.finish().unwrap();
        assert!(pdf
            .windows(b"/Font <<\n/F1 3 0 R\n/F2 6 0 R\n/F3 9 0 R\n>>".len())
            .any(|window| { window == b"/Font <<\n/F1 3 0 R\n/F2 6 0 R\n/F3 9 0 R\n>>" }));
    }

    #[test]
    fn 同じtype1handleをpage間で参照してもobjectを書き直さない() {
        let mut document = PdfDocument::new(Vec::new()).unwrap();
        let font = 合成type1を加える(&mut document, "ReusableSynthetic");
        for content in [b"first".as_slice(), b"second".as_slice()] {
            document
                .add_page(PdfPage {
                    width: PdfCoordinate(10_000_000),
                    height: PdfCoordinate(20_000_000),
                    courier_font: None,
                    fonts: &[PdfPageFont::Type1(font)],
                    resource_entries: b"",
                    content,
                })
                .unwrap();
        }
        let pdf = document.finish().unwrap();
        assert_eq!(
            pdf.windows(b"/F1 5 0 R".len())
                .filter(|window| *window == b"/F1 5 0 R")
                .count(),
            2
        );
        assert_eq!(
            pdf.windows(b"5 0 obj\n".len())
                .filter(|window| *window == b"5 0 obj\n")
                .count(),
            1
        );
        assert_eq!(
            pdf.windows(b"/BaseFont /ReusableSynthetic".len())
                .filter(|window| *window == b"/BaseFont /ReusableSynthetic")
                .count(),
            1
        );
    }

    #[test]
    fn page内の重複handleとraw_font資源を拒む() {
        let mut document = PdfDocument::new(Vec::new()).unwrap();
        let font = 合成type1を加える(&mut document, "UniqueSynthetic");
        let duplicate = document
            .add_page(PdfPage {
                width: PdfCoordinate(10_000_000),
                height: PdfCoordinate(20_000_000),
                courier_font: None,
                fonts: &[PdfPageFont::Type1(font), PdfPageFont::Type1(font)],
                resource_entries: b"",
                content: b"",
            })
            .unwrap_err();
        assert!(matches!(
            duplicate,
            PdfDocumentError::DuplicateType1Font(found) if found == font
        ));

        let raw_font = document
            .add_page(PdfPage {
                width: PdfCoordinate(10_000_000),
                height: PdfCoordinate(20_000_000),
                courier_font: None,
                fonts: &[],
                resource_entries: b"/#46ont <<>>",
                content: b"",
            })
            .unwrap_err();
        assert!(matches!(
            raw_font,
            PdfDocumentError::RawFontResourceCollision
        ));
        assert_eq!(document.page_count(), 0);

        document
            .add_page(PdfPage {
                width: PdfCoordinate(10_000_000),
                height: PdfCoordinate(20_000_000),
                courier_font: None,
                fonts: &[],
                resource_entries: b"/Properties << /Note (escaped /Font is text) >>",
                content: b"",
            })
            .unwrap();
    }

    #[test]
    fn object番号が同じでも別文書のtype1handleを拒む() {
        let mut other = PdfDocument::new(Vec::new()).unwrap();
        let foreign = 合成type1を加える(&mut other, "ForeignSynthetic");

        let mut document = PdfDocument::new(Vec::new()).unwrap();
        let local = 合成type1を加える(&mut document, "LocalSynthetic");
        assert_eq!(foreign.object(), local.object());
        let error = document
            .add_page(PdfPage {
                width: PdfCoordinate(10_000_000),
                height: PdfCoordinate(20_000_000),
                courier_font: None,
                fonts: &[PdfPageFont::Type1(foreign)],
                resource_entries: b"",
                content: b"",
            })
            .unwrap_err();
        assert!(matches!(
            error,
            PdfDocumentError::UnknownType1Font(found) if found == foreign
        ));

        document
            .add_page(PdfPage {
                width: PdfCoordinate(10_000_000),
                height: PdfCoordinate(20_000_000),
                courier_font: None,
                fonts: &[PdfPageFont::Type1(local)],
                resource_entries: b"",
                content: b"",
            })
            .unwrap();
    }

    #[test]
    fn object番号が同じでも別文書のcourier_handleを拒む() {
        let mut other = PdfDocument::new(Vec::new()).unwrap();
        let foreign = other.add_standard_courier_font().unwrap();

        let mut document = PdfDocument::new(Vec::new()).unwrap();
        let local = document.add_standard_courier_font().unwrap();
        assert_eq!(foreign.object, local.object);
        let error = document
            .add_page(PdfPage {
                width: PdfCoordinate(10_000_000),
                height: PdfCoordinate(20_000_000),
                courier_font: Some(foreign),
                fonts: &[],
                resource_entries: b"",
                content: b"",
            })
            .unwrap_err();
        assert!(matches!(
            error,
            PdfDocumentError::UnknownCourierFont(found) if found == foreign
        ));
    }
}

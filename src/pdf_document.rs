//! PDF 1.4 の文書構造と固定小数座標。
//!
//! 低水準の object serializer は [`crate::pdf`] に閉じ込め、この層では公開仕様の
//! Catalog / Pages / Page / Contents の関係だけを組み立てる。組版木の走査やフォント
//! 解決はさらに上の backend の責務である。

use crate::pdf::{PdfObjectId, PdfWriter, PdfWriterError};

use std::fmt;
use std::io::Write;

/// PDF real を 10^-6 bp 単位で保持する。
///
/// 浮動小数点の指数表記やplatform差をPDFへ流さないため、変換と印字を整数で行う。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PdfCoordinate(i64);

impl PdfCoordinate {
    const UNITS_PER_BP: i128 = 1_000_000;

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

    fn is_positive(self) -> bool {
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

/// 一枚のページを構成するデータ。
pub(crate) struct PdfPage<'a> {
    pub(crate) width: PdfCoordinate,
    pub(crate) height: PdfCoordinate,
    /// 外側の `<< >>` を含まないresource dictionary entry。
    pub(crate) resource_entries: &'a [u8],
    pub(crate) content: &'a [u8],
}

/// PDF 1.4の一段page treeを逐次構成する。
pub(crate) struct PdfDocument<W: Write> {
    writer: PdfWriter<W>,
    catalog: PdfObjectId,
    pages: PdfObjectId,
    page_ids: Vec<PdfObjectId>,
}

impl<W: Write> PdfDocument<W> {
    pub(crate) fn new(target: W) -> Result<Self, PdfDocumentError> {
        let mut writer = PdfWriter::new(target)?;
        let catalog = writer.reserve_object()?;
        let pages = writer.reserve_object()?;
        Ok(Self {
            writer,
            catalog,
            pages,
            page_ids: Vec::new(),
        })
    }

    pub(crate) fn add_page(&mut self, page: PdfPage<'_>) -> Result<(), PdfDocumentError> {
        if !page.width.is_positive() || !page.height.is_positive() {
            return Err(PdfDocumentError::InvalidPageSize {
                width: page.width,
                height: page.height,
            });
        }

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

#[derive(Debug)]
pub(crate) enum PdfDocumentError {
    Writer(PdfWriterError),
    CoordinateOverflow,
    InvalidPageSize {
        width: PdfCoordinate,
        height: PdfCoordinate,
    },
}

impl fmt::Display for PdfDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Writer(error) => error.fmt(formatter),
            Self::CoordinateOverflow => formatter.write_str("PDF coordinate overflow"),
            Self::InvalidPageSize { width, height } => {
                write!(formatter, "invalid PDF page size {width} by {height}")
            }
        }
    }
}

impl std::error::Error for PdfDocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Writer(error) => Some(error),
            Self::CoordinateOverflow | Self::InvalidPageSize { .. } => None,
        }
    }
}

impl From<PdfWriterError> for PdfDocumentError {
    fn from(error: PdfWriterError) -> Self {
        Self::Writer(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{PdfCoordinate, PdfDocument, PdfDocumentError, PdfPage};

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
                resource_entries: b"",
                content: b"",
            })
            .unwrap_err();
        assert!(matches!(error, PdfDocumentError::InvalidPageSize { .. }));
    }
}

use super::output_backend::ShipoutBackend;
use crate::pdf_document::{PdfCoordinate, PdfCourierFont, PdfDocument, PdfDocumentError, PdfPage};
use crate::scaled::Scaled;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};

/// PDFへ実際に渡ったbyte数を、seekせずに数えるsink。
struct CountingSink<W> {
    inner: W,
    byte_count: u64,
}

impl<W> CountingSink<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            byte_count: 0,
        }
    }

    fn into_parts(self) -> (W, u64) {
        (self.inner, self.byte_count)
    }
}

impl<W: Write> Write for CountingSink<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.byte_count = self
            .byte_count
            .checked_add(
                u64::try_from(written)
                    .map_err(|_| io::Error::other("PDF output byte count does not fit in u64"))?,
            )
            .ok_or_else(|| io::Error::other("PDF output byte count overflow"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[derive(Clone, Copy)]
struct Position {
    horizontal: Scaled,
    vertical: Scaled,
}

struct PageState {
    media_height: PdfCoordinate,
    declared_height: Scaled,
    min_horizontal: Scaled,
    max_horizontal: Scaled,
    min_vertical: Scaled,
    max_vertical: Scaled,
    position: Position,
    position_stack: Vec<Position>,
    content: Vec<u8>,
}

/// Shipoutの表示命令を、Standard 14 Courierだけで最小PDFへ写すbackend。
pub(crate) struct PdfBackend<W: Write> {
    document: PdfDocument<CountingSink<W>>,
    courier_font: PdfCourierFont,
    magnification: Scaled,
    font_sizes: HashMap<u32, Scaled>,
    current_font: Option<u32>,
    page: Option<PageState>,
}

impl<W: Write> PdfBackend<W> {
    pub(crate) fn new(target: W, magnification: Scaled) -> Result<Self, PdfBackendError> {
        if magnification <= 0 {
            return Err(PdfBackendError::InvalidMagnification(magnification));
        }
        let mut document = PdfDocument::new(CountingSink::new(target))?;
        let courier_font = document.add_standard_courier_font()?;
        Ok(Self {
            document,
            courier_font,
            magnification,
            font_sizes: HashMap::new(),
            current_font: None,
            page: None,
        })
    }

    /// PDFとbyte数に加えてsink自体を返す。Vec sinkの試験と将来の埋込み用。
    pub(crate) fn finish_with_target(self) -> Result<(W, usize), PdfBackendError> {
        if self.page.is_some() {
            return Err(PdfBackendError::PageStillOpen);
        }
        let sink = self.document.finish()?;
        let (target, byte_count) = sink.into_parts();
        let byte_count =
            usize::try_from(byte_count).map_err(|_| PdfBackendError::OutputTooLarge(byte_count))?;
        Ok((target, byte_count))
    }

    fn current_page_mut(&mut self) -> Result<&mut PageState, PdfBackendError> {
        self.page.as_mut().ok_or(PdfBackendError::NoOpenPage)
    }

    fn checked_move(value: Scaled, amount: Scaled) -> Result<Scaled, PdfBackendError> {
        value
            .checked_add(amount)
            .ok_or(PdfBackendError::PositionOverflow)
    }

    fn include_interval(minimum: &mut Scaled, maximum: &mut Scaled, first: Scaled, second: Scaled) {
        *minimum = (*minimum).min(first).min(second);
        *maximum = (*maximum).max(first).max(second);
    }

    fn page_position(
        page: &PageState,
        magnification: Scaled,
    ) -> Result<(PdfCoordinate, PdfCoordinate), PdfBackendError> {
        let horizontal = PdfCoordinate::from_scaled(page.position.horizontal, magnification)?;
        let vertical = PdfCoordinate::from_scaled(page.position.vertical, magnification)?;
        let x = PdfCoordinate::ONE_INCH.checked_add(horizontal)?;
        let y = page
            .media_height
            .checked_sub(PdfCoordinate::ONE_INCH)?
            .checked_sub(vertical)?;
        Ok((x, y))
    }

    fn append_rule(
        page: &mut PageState,
        magnification: Scaled,
        height: Scaled,
        width: Scaled,
    ) -> Result<(), PdfBackendError> {
        let (x, y) = Self::page_position(page, magnification)?;
        let height = PdfCoordinate::from_scaled(height, magnification)?;
        let width = PdfCoordinate::from_scaled(width, magnification)?;
        page.content
            .extend_from_slice(format!("{x} {y} {width} {height} re f\n").as_bytes());
        Ok(())
    }

    fn append_character(
        page: &mut PageState,
        magnification: Scaled,
        character: u8,
        at_size: Scaled,
    ) -> Result<(), PdfBackendError> {
        let (x, y) = Self::page_position(page, magnification)?;
        let font_size = PdfCoordinate::from_scaled(at_size, magnification)?;
        page.content
            .extend_from_slice(format!("BT\n/F1 {font_size} Tf\n1 0 0 1 {x} {y} Tm\n(").as_bytes());
        if matches!(character, b'(' | b')' | b'\\') {
            page.content.push(b'\\');
        }
        page.content.push(character);
        page.content.extend_from_slice(b") Tj\nET\n");
        Ok(())
    }
}

impl<W: Write> ShipoutBackend for PdfBackend<W> {
    type Error = PdfBackendError;

    fn start_page(
        &mut self,
        _counts: &[i32; 10],
        page_height: Scaled,
        page_width: Scaled,
    ) -> Result<(), Self::Error> {
        if self.page.is_some() {
            return Err(PdfBackendError::PageAlreadyOpen);
        }
        let double_margin = PdfCoordinate::ONE_INCH.checked_add(PdfCoordinate::ONE_INCH)?;
        // TeXは負幅・負高のboxも作れる。物理媒体まで負にせず、内容範囲を空として扱う。
        let declared_width = page_width.max(0);
        let declared_height = page_height.max(0);
        let media_width = PdfCoordinate::from_scaled(declared_width, self.magnification)?
            .checked_add(double_margin)?;
        let media_height = PdfCoordinate::from_scaled(declared_height, self.magnification)?
            .checked_add(double_margin)?;
        if !media_width.is_positive() || !media_height.is_positive() {
            return Err(PdfBackendError::InvalidPageSize {
                width: media_width,
                height: media_height,
            });
        }
        self.current_font = None;
        self.page = Some(PageState {
            media_height,
            declared_height,
            min_horizontal: 0,
            max_horizontal: declared_width,
            min_vertical: 0,
            max_vertical: declared_height,
            position: Position {
                horizontal: 0,
                vertical: 0,
            },
            position_stack: Vec::new(),
            content: Vec::new(),
        });
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), Self::Error> {
        let Some(page) = self.page.as_ref() else {
            return Err(PdfBackendError::NoOpenPage);
        };
        if !page.position_stack.is_empty() {
            return Err(PdfBackendError::UnbalancedPushes(page.position_stack.len()));
        }
        let page = self.page.take().expect("page presence checked above");
        let left = PdfCoordinate::from_scaled(page.min_horizontal, self.magnification)?;
        let right = PdfCoordinate::from_scaled(page.max_horizontal, self.magnification)?;
        let top = PdfCoordinate::from_scaled(page.min_vertical, self.magnification)?;
        let bottom = PdfCoordinate::from_scaled(page.max_vertical, self.magnification)?;
        let double_margin = PdfCoordinate::ONE_INCH.checked_add(PdfCoordinate::ONE_INCH)?;
        let media_width = right.checked_sub(left)?.checked_add(double_margin)?;
        let media_height = bottom.checked_sub(top)?.checked_add(double_margin)?;

        // 既に書いた座標は宣言寸法を基準にしている。観測した描画範囲がboxからはみ出す
        // 場合だけ平行移動し、左右上下の物理1inch余白を保つ。
        let zero = PdfCoordinate::from_scaled(0, self.magnification)?;
        let declared_height = PdfCoordinate::from_scaled(page.declared_height, self.magnification)?;
        let translate_x = zero.checked_sub(left)?;
        let translate_y = bottom.checked_sub(declared_height)?;
        let content = if translate_x != zero || translate_y != zero {
            let mut translated =
                format!("q\n1 0 0 1 {translate_x} {translate_y} cm\n").into_bytes();
            translated.extend_from_slice(&page.content);
            translated.extend_from_slice(b"Q\n");
            translated
        } else {
            page.content
        };
        self.document.add_page(PdfPage {
            width: media_width,
            height: media_height,
            courier_font: Some(self.courier_font),
            resource_entries: b"",
            content: &content,
        })?;
        self.current_font = None;
        Ok(())
    }

    fn push(&mut self) -> Result<(), Self::Error> {
        let page = self.current_page_mut()?;
        page.position_stack.push(page.position);
        Ok(())
    }

    fn pop(&mut self) -> Result<(), Self::Error> {
        let page = self.current_page_mut()?;
        page.position = page
            .position_stack
            .pop()
            .ok_or(PdfBackendError::PositionStackUnderflow)?;
        Ok(())
    }

    fn move_right(&mut self, amount: Scaled) -> Result<(), Self::Error> {
        let page = self.current_page_mut()?;
        page.position.horizontal = Self::checked_move(page.position.horizontal, amount)?;
        Ok(())
    }

    fn move_down(&mut self, amount: Scaled) -> Result<(), Self::Error> {
        let page = self.current_page_mut()?;
        page.position.vertical = Self::checked_move(page.position.vertical, amount)?;
        Ok(())
    }

    fn define_font(
        &mut self,
        font_number: u32,
        _checksum: u32,
        at_size: Scaled,
        _design_size: Scaled,
        _area: &[u8],
        _name: &[u8],
    ) -> Result<(), Self::Error> {
        if at_size <= 0 {
            return Err(PdfBackendError::InvalidFontSize {
                font_number,
                at_size,
            });
        }
        match self.font_sizes.entry(font_number) {
            Entry::Vacant(entry) => {
                entry.insert(at_size);
                Ok(())
            }
            Entry::Occupied(_) => Err(PdfBackendError::FontAlreadyDefined(font_number)),
        }
    }

    fn set_font(&mut self, font_number: u32) -> Result<(), Self::Error> {
        if self.page.is_none() {
            return Err(PdfBackendError::NoOpenPage);
        }
        if !self.font_sizes.contains_key(&font_number) {
            return Err(PdfBackendError::UndefinedFont(font_number));
        }
        self.current_font = Some(font_number);
        Ok(())
    }

    fn set_char(&mut self, character: u8, width: Scaled) -> Result<(), Self::Error> {
        if self.page.is_none() {
            return Err(PdfBackendError::NoOpenPage);
        }
        let font_number = self.current_font.ok_or(PdfBackendError::NoCurrentFont)?;
        let at_size = *self
            .font_sizes
            .get(&font_number)
            .ok_or(PdfBackendError::UndefinedFont(font_number))?;
        let magnification = self.magnification;
        let page = self.current_page_mut()?;
        let next_horizontal = Self::checked_move(page.position.horizontal, width)?;
        if (b' '..=b'~').contains(&character) {
            Self::append_character(page, magnification, character, at_size)?;
        }
        Self::include_interval(
            &mut page.min_horizontal,
            &mut page.max_horizontal,
            page.position.horizontal,
            next_horizontal,
        );
        Self::include_interval(
            &mut page.min_vertical,
            &mut page.max_vertical,
            page.position.vertical,
            page.position.vertical,
        );
        page.position.horizontal = next_horizontal;
        Ok(())
    }

    fn set_rule(&mut self, height: Scaled, width: Scaled) -> Result<(), Self::Error> {
        let magnification = self.magnification;
        let page = self.current_page_mut()?;
        let next_horizontal = Self::checked_move(page.position.horizontal, width)?;
        let top = page
            .position
            .vertical
            .checked_sub(height)
            .ok_or(PdfBackendError::PositionOverflow)?;
        Self::append_rule(page, magnification, height, width)?;
        Self::include_interval(
            &mut page.min_horizontal,
            &mut page.max_horizontal,
            page.position.horizontal,
            next_horizontal,
        );
        Self::include_interval(
            &mut page.min_vertical,
            &mut page.max_vertical,
            top,
            page.position.vertical,
        );
        page.position.horizontal = next_horizontal;
        Ok(())
    }

    fn put_rule(&mut self, height: Scaled, width: Scaled) -> Result<(), Self::Error> {
        let magnification = self.magnification;
        let page = self.current_page_mut()?;
        let horizontal_end = Self::checked_move(page.position.horizontal, width)?;
        let top = page
            .position
            .vertical
            .checked_sub(height)
            .ok_or(PdfBackendError::PositionOverflow)?;
        Self::append_rule(page, magnification, height, width)?;
        Self::include_interval(
            &mut page.min_horizontal,
            &mut page.max_horizontal,
            page.position.horizontal,
            horizontal_end,
        );
        Self::include_interval(
            &mut page.min_vertical,
            &mut page.max_vertical,
            top,
            page.position.vertical,
        );
        Ok(())
    }

    fn write_special(&mut self, _bytes: &[u8]) -> Result<(), Self::Error> {
        self.current_page_mut()?;
        Ok(())
    }

    fn page_count(&self) -> usize {
        self.document.page_count()
    }

    fn finish(self) -> Result<usize, Self::Error> {
        let (_, byte_count) = self.finish_with_target()?;
        Ok(byte_count)
    }
}

#[derive(Debug)]
pub(crate) enum PdfBackendError {
    Document(PdfDocumentError),
    InvalidMagnification(Scaled),
    InvalidPageSize {
        width: PdfCoordinate,
        height: PdfCoordinate,
    },
    PageAlreadyOpen,
    NoOpenPage,
    PageStillOpen,
    PositionOverflow,
    PositionStackUnderflow,
    UnbalancedPushes(usize),
    FontAlreadyDefined(u32),
    UndefinedFont(u32),
    NoCurrentFont,
    InvalidFontSize {
        font_number: u32,
        at_size: Scaled,
    },
    OutputTooLarge(u64),
}

impl fmt::Display for PdfBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::InvalidMagnification(magnification) => {
                write!(formatter, "invalid PDF magnification {magnification}")
            }
            Self::InvalidPageSize { width, height } => {
                write!(formatter, "invalid PDF page size {width} by {height}")
            }
            Self::PageAlreadyOpen => formatter.write_str("a PDF page is already open"),
            Self::NoOpenPage => formatter.write_str("no PDF page is open"),
            Self::PageStillOpen => formatter.write_str("cannot finish PDF with an open page"),
            Self::PositionOverflow => formatter.write_str("PDF shipout position overflow"),
            Self::PositionStackUnderflow => {
                formatter.write_str("PDF shipout position stack underflow")
            }
            Self::UnbalancedPushes(depth) => {
                write!(formatter, "PDF page ended with {depth} unmatched pushes")
            }
            Self::FontAlreadyDefined(font) => {
                write!(formatter, "PDF font number {font} was defined twice")
            }
            Self::UndefinedFont(font) => write!(formatter, "undefined PDF font number {font}"),
            Self::NoCurrentFont => formatter.write_str("no current PDF font"),
            Self::InvalidFontSize {
                font_number,
                at_size,
            } => write!(
                formatter,
                "invalid PDF font size {at_size} for font number {font_number}"
            ),
            Self::OutputTooLarge(byte_count) => write!(
                formatter,
                "PDF byte count {byte_count} does not fit in usize"
            ),
        }
    }
}

impl std::error::Error for PdfBackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PdfDocumentError> for PdfBackendError {
    fn from(error: PdfDocumentError) -> Self {
        Self::Document(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{PdfBackend, PdfBackendError};
    use crate::output::output_backend::ShipoutBackend;

    fn 含む(bytes: &[u8], expected: &[u8]) -> bool {
        bytes
            .windows(expected.len())
            .any(|window| window == expected)
    }

    #[test]
    fn 一inch余白を含む複数pageを閉じる() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend.start_page(&[0; 10], 65536, 65536).unwrap();
        backend.end_page().unwrap();
        backend.start_page(&[0; 10], 131072, 196608).unwrap();
        backend.end_page().unwrap();
        assert_eq!(backend.page_count(), 2);
        let (pdf, byte_count) = backend.finish_with_target().unwrap();

        assert_eq!(byte_count, pdf.len());
        assert!(含む(&pdf, b"/MediaBox [0 0 144.996264 144.996264]"));
        assert!(含む(&pdf, b"/MediaBox [0 0 146.988792 145.992528]"));
        assert!(含む(&pdf, b"/Count 2"));
    }

    #[test]
    fn 一inch余白はmagで拡大しない() {
        let mut backend = PdfBackend::new(Vec::new(), 1200).unwrap();
        backend.start_page(&[0; 10], 65536, 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"/MediaBox [0 0 145.195517 145.195517]"));
    }

    #[test]
    fn courierで文字ごとの絶対位置とfont_sizeを書く() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend
            .start_page(&[0; 10], 20 * 65536, 20 * 65536)
            .unwrap();
        backend
            .define_font(3, 0, 10 * 65536, 10 * 65536, b"", b"ignored")
            .unwrap();
        backend.set_font(3).unwrap();
        backend.move_right(65536).unwrap();
        backend.move_down(2 * 65536).unwrap();
        backend.set_char(b'(', 65536).unwrap();
        backend.set_char(0x80, 65536).unwrap();
        backend.set_char(b'A', 65536).unwrap();
        backend
            .define_font(7, 0, 20 * 65536, 20 * 65536, b"", b"also ignored")
            .unwrap();
        backend.set_font(7).unwrap();
        backend.set_char(b'\\', 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"/BaseFont /Courier"));
        assert!(含む(&pdf, b"/Font <<\n/F1 3 0 R\n>>"));
        assert!(含む(&pdf, b"/F1 9.96264 Tf"));
        assert!(含む(&pdf, b"1 0 0 1 72.996264 89.932752 Tm\n(\\() Tj"));
        assert!(含む(&pdf, b"1 0 0 1 74.988792 89.932752 Tm\n(A) Tj"));
        assert!(含む(&pdf, b"/F1 19.92528 Tf"));
        assert!(含む(&pdf, b"(\\\\) Tj"));
        assert_eq!(
            pdf.windows(b") Tj".len())
                .filter(|window| *window == b") Tj")
                .count(),
            3
        );
    }

    #[test]
    fn push_popとruleを座標へ写しspecialは捨てる() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend
            .start_page(&[0; 10], 10 * 65536, 10 * 65536)
            .unwrap();
        backend.move_right(65536).unwrap();
        backend.move_down(2 * 65536).unwrap();
        backend.push().unwrap();
        backend.move_right(65536).unwrap();
        backend.move_down(65536).unwrap();
        backend.put_rule(65536, 2 * 65536).unwrap();
        backend.put_rule(65536, 2 * 65536).unwrap();
        backend.pop().unwrap();
        backend.set_rule(2 * 65536, 3 * 65536).unwrap();
        backend.put_rule(65536, 65536).unwrap();
        backend
            .write_special(b"RAW-SPECIAL-MUST-NOT-APPEAR")
            .unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"73.992528 78.973848 1.992528 0.996264 re f"));
        assert!(含む(&pdf, b"72.996264 79.970112 2.988792 1.992528 re f"));
        assert!(含む(&pdf, b"75.985056 79.970112 0.996264 0.996264 re f"));
        assert_eq!(
            pdf.windows(b"73.992528 78.973848 1.992528 0.996264 re f".len())
                .filter(|window| { *window == b"73.992528 78.973848 1.992528 0.996264 re f" })
                .count(),
            2
        );
        assert!(!含む(&pdf, b"RAW-SPECIAL-MUST-NOT-APPEAR"));
    }

    #[test]
    fn 不正な状態と座標overflowを報せる() {
        assert!(matches!(
            PdfBackend::new(Vec::new(), 0),
            Err(PdfBackendError::InvalidMagnification(0))
        ));
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        assert!(matches!(backend.pop(), Err(PdfBackendError::NoOpenPage)));
        backend.start_page(&[0; 10], 0, 0).unwrap();
        assert!(matches!(
            backend.pop(),
            Err(PdfBackendError::PositionStackUnderflow)
        ));
        assert!(matches!(
            backend.set_char(b'A', 65536),
            Err(PdfBackendError::NoCurrentFont)
        ));
        assert!(matches!(
            backend.start_page(&[0; 10], 0, 0),
            Err(PdfBackendError::PageAlreadyOpen)
        ));
        assert!(matches!(
            backend.set_font(9),
            Err(PdfBackendError::UndefinedFont(9))
        ));
        backend
            .define_font(9, 0, 65536, 65536, b"", b"font")
            .unwrap();
        assert!(matches!(
            backend.define_font(9, 0, 65536, 65536, b"", b"font"),
            Err(PdfBackendError::FontAlreadyDefined(9))
        ));
        backend.move_right(i32::MAX).unwrap();
        assert!(matches!(
            backend.move_right(1),
            Err(PdfBackendError::PositionOverflow)
        ));
        backend.push().unwrap();
        assert!(matches!(
            backend.end_page(),
            Err(PdfBackendError::UnbalancedPushes(1))
        ));
        backend.pop().unwrap();
        backend.end_page().unwrap();
    }

    #[test]
    fn 開いたpageを残してfinishしない() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend.start_page(&[0; 10], 0, 0).unwrap();
        assert!(matches!(
            backend.finish_with_target(),
            Err(PdfBackendError::PageStillOpen)
        ));
    }

    #[test]
    fn 負の内容寸法でも正のmedia_boxを作る() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend.start_page(&[0; 10], -65536, -131072).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();
        assert!(含む(&pdf, b"/MediaBox [0 0 144 144]"));
    }

    #[test]
    fn 負幅のboxからはみ出すruleまでmedia_boxへ収める() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend.start_page(&[0; 10], 10 * 65536, -65536).unwrap();
        backend.move_down(10 * 65536).unwrap();
        backend.put_rule(10 * 65536, 200 * 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"/MediaBox [0 0 343.252802 153.96264]"));
        assert!(含む(&pdf, b"72 72 199.252802 9.96264 re f"));
    }
}

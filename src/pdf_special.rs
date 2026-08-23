//! PDF backendが意味を理解するDVI specialの限定parser。
//!
//! 生のspecialをPDF contentへ流さず、公開されたdriver契約を型付き値へ変換してから
//! backendへ渡す。未認識のspecialと、認識したが壊れているspecialを区別する。

use std::fmt;

const PAPER_SIZE_PREFIX: &[u8] = b"papersize=";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PdfPaperSize {
    pub(crate) width: PdfSpecialDimension,
    pub(crate) height: PdfSpecialDimension,
}

/// specialに書かれた物理寸法を、spを基底とする既約な整数比で保持する。
///
/// driver specialはTeX registerではないため、ここでsp整数へ丸めると`1in`すら僅かに
/// 変わる。PDF座標を実際に求める境界までこの比を保つ。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PdfSpecialDimension {
    numerator_sp: i128,
    denominator_sp: i128,
}

impl PdfSpecialDimension {
    pub(crate) fn sp_ratio(self) -> (i128, i128) {
        (self.numerator_sp, self.denominator_sp)
    }
}

impl fmt::Display for PdfSpecialDimension {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denominator_sp == 1 {
            write!(formatter, "{}sp", self.numerator_sp)
        } else {
            write!(formatter, "{}/{}sp", self.numerator_sp, self.denominator_sp)
        }
    }
}

/// `papersize=width,height`だけを認識する。
///
/// Dvipsの公開契約にあるTeX単位をsp基底の整数比へ直す。認識しないspecialは`Ok(None)`、
/// prefixまで一致した壊れたspecialはtyped errorにする。
pub(crate) fn parse_pdf_special(bytes: &[u8]) -> Result<Option<PdfPaperSize>, PdfSpecialError> {
    let bytes = trim_ascii(bytes);
    let Some(body) = bytes.strip_prefix(PAPER_SIZE_PREFIX) else {
        return Ok(None);
    };
    let Some(comma) = body.iter().position(|byte| *byte == b',') else {
        return Err(PdfSpecialError::MissingPaperSizeComma);
    };
    if body[comma + 1..].contains(&b',') {
        return Err(PdfSpecialError::TooManyPaperSizeDimensions);
    }
    let width = parse_positive_dimension(trim_ascii(&body[..comma]))
        .map_err(PdfSpecialError::InvalidPaperWidth)?;
    let height = parse_positive_dimension(trim_ascii(&body[comma + 1..]))
        .map_err(PdfSpecialError::InvalidPaperHeight)?;
    Ok(Some(PdfPaperSize { width, height }))
}

fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while matches!(bytes.first(), Some(byte) if byte.is_ascii_whitespace()) {
        bytes = &bytes[1..];
    }
    while matches!(bytes.last(), Some(byte) if byte.is_ascii_whitespace()) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn parse_positive_dimension(bytes: &[u8]) -> Result<PdfSpecialDimension, PdfDimensionError> {
    if bytes.is_empty() {
        return Err(PdfDimensionError::Empty);
    }
    let mut cursor = 0;
    let negative = match bytes[cursor] {
        b'+' => {
            cursor += 1;
            false
        }
        b'-' => {
            cursor += 1;
            true
        }
        _ => false,
    };

    let mut digits = 0_usize;
    let mut numerator = 0_i128;
    while let Some(byte @ b'0'..=b'9') = bytes.get(cursor).copied() {
        numerator = numerator
            .checked_mul(10)
            .and_then(|value| value.checked_add(i128::from(byte - b'0')))
            .ok_or(PdfDimensionError::Overflow)?;
        cursor += 1;
        digits += 1;
    }

    let mut decimal_denominator = 1_i128;
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        while let Some(byte @ b'0'..=b'9') = bytes.get(cursor).copied() {
            numerator = numerator
                .checked_mul(10)
                .and_then(|value| value.checked_add(i128::from(byte - b'0')))
                .ok_or(PdfDimensionError::Overflow)?;
            decimal_denominator = decimal_denominator
                .checked_mul(10)
                .ok_or(PdfDimensionError::Overflow)?;
            cursor += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return Err(PdfDimensionError::MissingNumber);
    }

    let unit_start = cursor;
    while matches!(bytes.get(cursor), Some(byte) if byte.is_ascii_alphabetic()) {
        cursor += 1;
    }
    if unit_start == cursor {
        return Err(PdfDimensionError::MissingUnit);
    }
    let unit = &bytes[unit_start..cursor];
    if !trim_ascii(&bytes[cursor..]).is_empty() {
        return Err(PdfDimensionError::TrailingGarbage(bytes[cursor..].to_vec()));
    }
    if negative || numerator == 0 {
        return Err(PdfDimensionError::NonPositive);
    }

    // unit_numerator / unit_denominator sp。すべて公開されたTeXの単位関係から作る。
    let (unit_numerator, unit_denominator) = match unit {
        b"sp" => (1_i128, 1_i128),
        b"pt" => (65_536, 1),
        b"pc" => (12 * 65_536, 1),
        b"in" => (7_227 * 65_536, 100),
        b"bp" => (7_227 * 65_536, 7_200),
        b"cm" => (7_227 * 65_536, 254),
        b"mm" => (7_227 * 65_536, 2_540),
        b"dd" => (1_238 * 65_536, 1_157),
        b"cc" => (12 * 1_238 * 65_536, 1_157),
        _ => return Err(PdfDimensionError::UnknownUnit(unit.to_vec())),
    };
    let numerator = numerator
        .checked_mul(unit_numerator)
        .ok_or(PdfDimensionError::Overflow)?;
    let denominator = decimal_denominator
        .checked_mul(unit_denominator)
        .ok_or(PdfDimensionError::Overflow)?;
    let divisor = greatest_common_divisor(numerator, denominator);
    Ok(PdfSpecialDimension {
        numerator_sp: numerator / divisor,
        denominator_sp: denominator / divisor,
    })
}

fn greatest_common_divisor(mut left: i128, mut right: i128) -> i128 {
    debug_assert!(left > 0 && right > 0);
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PdfSpecialError {
    MissingPaperSizeComma,
    TooManyPaperSizeDimensions,
    InvalidPaperWidth(PdfDimensionError),
    InvalidPaperHeight(PdfDimensionError),
}

impl fmt::Display for PdfSpecialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPaperSizeComma => {
                formatter.write_str("PDF papersize special needs `width,height`")
            }
            Self::TooManyPaperSizeDimensions => {
                formatter.write_str("PDF papersize special has more than two dimensions")
            }
            Self::InvalidPaperWidth(error) => {
                write!(formatter, "invalid PDF papersize width: {error}")
            }
            Self::InvalidPaperHeight(error) => {
                write!(formatter, "invalid PDF papersize height: {error}")
            }
        }
    }
}

impl std::error::Error for PdfSpecialError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPaperWidth(error) | Self::InvalidPaperHeight(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PdfDimensionError {
    Empty,
    MissingNumber,
    MissingUnit,
    UnknownUnit(Vec<u8>),
    TrailingGarbage(Vec<u8>),
    NonPositive,
    Overflow,
}

impl fmt::Display for PdfDimensionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("dimension is empty"),
            Self::MissingNumber => formatter.write_str("dimension has no number"),
            Self::MissingUnit => formatter.write_str("dimension has no unit"),
            Self::UnknownUnit(unit) => write!(
                formatter,
                "unknown dimension unit `{}`",
                String::from_utf8_lossy(unit)
            ),
            Self::TrailingGarbage(bytes) => write!(
                formatter,
                "trailing bytes `{}` after dimension",
                String::from_utf8_lossy(bytes)
            ),
            Self::NonPositive => formatter.write_str("dimension must be positive"),
            Self::Overflow => formatter.write_str("dimension integer ratio overflow"),
        }
    }
}

impl std::error::Error for PdfDimensionError {}

#[cfg(test)]
mod tests {
    use super::{greatest_common_divisor, parse_pdf_special, PdfDimensionError, PdfSpecialError};

    fn reduced(numerator: i128, denominator: i128) -> (i128, i128) {
        let divisor = greatest_common_divisor(numerator, denominator);
        (numerator / divisor, denominator / divisor)
    }

    #[test]
    fn dvipsのpapersizeをsp基底の既約比へ正確に直す() {
        for (source, expected) in [
            (b"papersize=1sp,2sp".as_slice(), ((1, 1), (2, 1))),
            (
                b"papersize=1pt,12pt".as_slice(),
                ((65_536, 1), (12 * 65_536, 1)),
            ),
            (
                b"papersize=1pc,1in".as_slice(),
                ((12 * 65_536, 1), (7_227 * 65_536, 100)),
            ),
            (
                b"papersize=1bp,1cm".as_slice(),
                ((7_227 * 65_536, 7_200), (7_227 * 65_536, 254)),
            ),
            (
                b"papersize=1mm,1dd".as_slice(),
                ((7_227 * 65_536, 2_540), (1_238 * 65_536, 1_157)),
            ),
            (
                b"papersize=1cc,.5pt".as_slice(),
                ((12 * 1_238 * 65_536, 1_157), (32_768, 1)),
            ),
        ] {
            let size = parse_pdf_special(source).unwrap().unwrap();
            let ((width_numerator, width_denominator), (height_numerator, height_denominator)) =
                expected;
            assert_eq!(
                size.width.sp_ratio(),
                reduced(width_numerator, width_denominator)
            );
            assert_eq!(
                size.height.sp_ratio(),
                reduced(height_numerator, height_denominator)
            );
        }
    }

    #[test]
    fn 外側とcomma周辺の空白を許す() {
        let size = parse_pdf_special(b" \tpapersize= 8.5in , 11in\r\n")
            .unwrap()
            .unwrap();
        assert_eq!(size.width.sp_ratio(), reduced(85 * 7_227 * 65_536, 1_000));
        assert_eq!(size.height.sp_ratio(), reduced(11 * 7_227 * 65_536, 100));
    }

    #[test]
    fn 未認識specialと壊れたpapersizeを区別する() {
        assert_eq!(parse_pdf_special(b"color push rgb 1 0 0").unwrap(), None);
        assert!(matches!(
            parse_pdf_special(b"papersize=10pt"),
            Err(PdfSpecialError::MissingPaperSizeComma)
        ));
        assert!(matches!(
            parse_pdf_special(b"papersize=10pt,20zz"),
            Err(PdfSpecialError::InvalidPaperHeight(
                PdfDimensionError::UnknownUnit(unit)
            )) if unit == b"zz"
        ));
        assert!(matches!(
            parse_pdf_special(b"papersize=0pt,20pt"),
            Err(PdfSpecialError::InvalidPaperWidth(
                PdfDimensionError::NonPositive
            ))
        ));
        assert!(matches!(
            parse_pdf_special(b"papersize=10pt,20pt,30pt"),
            Err(PdfSpecialError::TooManyPaperSizeDimensions)
        ));
    }

    #[test]
    fn 十進桁の整数比overflowを拒む() {
        assert!(matches!(
            parse_pdf_special(b"papersize=999999999999999999999999999999999999999pt,1pt"),
            Err(PdfSpecialError::InvalidPaperWidth(
                PdfDimensionError::Overflow
            ))
        ));
    }
}

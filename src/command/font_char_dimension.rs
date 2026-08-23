use crate::format::{Dumpable, FormatError};
use crate::print::Printer;

use std::io::Write;

/// e-TeX が 8-bit TFM の一文字から問い合わせる寸法。
///
/// primitive 名、format 表現、metric table の選択を同じ型へ集約し、四つの
/// primitive が別々の走査・欠落字判定を持たないようにする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontCharDimension {
    Width,
    Height,
    Depth,
    Italic,
}

impl FontCharDimension {
    pub const ALL: [Self; 4] = [Self::Width, Self::Height, Self::Depth, Self::Italic];

    pub fn primitive_name(self) -> &'static [u8] {
        match self {
            Self::Width => b"fontcharwd",
            Self::Height => b"fontcharht",
            Self::Depth => b"fontchardp",
            Self::Italic => b"fontcharic",
        }
    }

    pub fn display(self, printer: &mut impl Printer) {
        printer.print_esc_str(self.primitive_name());
    }
}

impl Dumpable for FontCharDimension {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(
            target,
            "{}",
            match self {
                Self::Width => "Width",
                Self::Height => "Height",
                Self::Depth => "Depth",
                Self::Italic => "Italic",
            }
        )
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "Width" => Ok(Self::Width),
            "Height" => Ok(Self::Height),
            "Depth" => Ok(Self::Depth),
            "Italic" => Ok(Self::Italic),
            _ => Err(FormatError::ParseError),
        }
    }
}

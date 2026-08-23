use crate::format::{Dumpable, FormatError};
use crate::print::Printer;

use std::io::Write;

/// e-TeX が通常の糊と数式糊のあいだで行う型変換。
///
/// 公式 e-TeX manual 3.5 の公開仕様にある二つの内部量を一つの型で表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlueConversion {
    MuToGlue,
    GlueToMu,
}

impl GlueConversion {
    pub const ALL: [Self; 2] = [Self::MuToGlue, Self::GlueToMu];

    pub fn primitive_name(self) -> &'static [u8] {
        match self {
            Self::MuToGlue => b"mutoglue",
            Self::GlueToMu => b"gluetomu",
        }
    }

    pub fn source_is_mu(self) -> bool {
        matches!(self, Self::MuToGlue)
    }

    pub fn display(self, printer: &mut impl Printer) {
        printer.print_esc_str(self.primitive_name());
    }
}

impl Dumpable for GlueConversion {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::MuToGlue => writeln!(target, "MuToGlue")?,
            Self::GlueToMu => writeln!(target, "GlueToMu")?,
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "MuToGlue" => Ok(Self::MuToGlue),
            "GlueToMu" => Ok(Self::GlueToMu),
            _ => Err(FormatError::ParseError),
        }
    }
}

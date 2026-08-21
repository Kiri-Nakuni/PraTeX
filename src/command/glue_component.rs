use crate::format::{Dumpable, FormatError};
use crate::print::Printer;

use std::io::Write;

/// e-TeX が糊から取り出す成分。
///
/// 公式 e-TeX manual 3.5, 5.1 にある四つの内部量を一つの型で表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlueComponent {
    Stretch,
    Shrink,
    StretchOrder,
    ShrinkOrder,
}

impl GlueComponent {
    pub const ALL: [Self; 4] = [
        Self::Stretch,
        Self::Shrink,
        Self::StretchOrder,
        Self::ShrinkOrder,
    ];

    pub fn primitive_name(self) -> &'static [u8] {
        match self {
            Self::Stretch => b"gluestretch",
            Self::Shrink => b"glueshrink",
            Self::StretchOrder => b"gluestretchorder",
            Self::ShrinkOrder => b"glueshrinkorder",
        }
    }

    pub fn display(self, printer: &mut impl Printer) {
        printer.print_esc_str(self.primitive_name());
    }
}

impl Dumpable for GlueComponent {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Stretch => writeln!(target, "Stretch")?,
            Self::Shrink => writeln!(target, "Shrink")?,
            Self::StretchOrder => writeln!(target, "StretchOrder")?,
            Self::ShrinkOrder => writeln!(target, "ShrinkOrder")?,
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Stretch" => Ok(Self::Stretch),
            "Shrink" => Ok(Self::Shrink),
            "StretchOrder" => Ok(Self::StretchOrder),
            "ShrinkOrder" => Ok(Self::ShrinkOrder),
            _ => Err(FormatError::ParseError),
        }
    }
}

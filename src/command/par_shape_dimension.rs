use crate::format::{Dumpable, FormatError};
use crate::print::Printer;

use std::io::Write;

/// e-TeXが現在の`\parshape`から取り出す寸法。
///
/// 公式e-TeX manual 3.4の三つの内部量を一つの型で表し、走査・表示・fmtの
/// 対応を別々のmatchへ重複させない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParShapeDimension {
    Indent,
    Length,
    Interleaved,
}

impl ParShapeDimension {
    pub const ALL: [Self; 3] = [Self::Indent, Self::Length, Self::Interleaved];

    pub fn primitive_name(self) -> &'static [u8] {
        match self {
            Self::Indent => b"parshapeindent",
            Self::Length => b"parshapelength",
            Self::Interleaved => b"parshapedimen",
        }
    }

    pub fn display(self, printer: &mut impl Printer) {
        printer.print_esc_str(self.primitive_name());
    }
}

impl Dumpable for ParShapeDimension {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Indent => writeln!(target, "Indent")?,
            Self::Length => writeln!(target, "Length")?,
            Self::Interleaved => writeln!(target, "Interleaved")?,
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Indent" => Ok(Self::Indent),
            "Length" => Ok(Self::Length),
            "Interleaved" => Ok(Self::Interleaved),
            _ => Err(FormatError::ParseError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 三種類をformat表現で往復する() {
        for query in ParShapeDimension::ALL {
            let mut bytes = Vec::new();
            query.dump(&mut bytes).unwrap();
            let text = String::from_utf8(bytes).unwrap();
            assert_eq!(ParShapeDimension::undump(&mut text.lines()).unwrap(), query);
        }
    }
}

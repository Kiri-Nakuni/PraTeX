use crate::eqtb::{Eqtb, RawStringVariable};
use crate::format::{Dumpable, FormatError};
use crate::input::Scanner;
use crate::logger::Logger;
use crate::print::Printer;

use std::io::Write;

/// `\rawstring<n>`と`\rawstringdef`で固定されたslotを同じcommandへ結ぶ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawStringCommand {
    Register,
    Variable(RawStringVariable),
}

impl RawStringCommand {
    pub(crate) fn scan_variable(
        self,
        scanner: &mut Scanner,
        eqtb: &mut Eqtb,
        logger: &mut Logger,
    ) -> RawStringVariable {
        match self {
            Self::Register => RawStringVariable::new(scanner.scan_register_index(eqtb, logger)),
            Self::Variable(variable) => variable,
        }
    }

    pub(crate) fn display(self, printer: &mut impl Printer) {
        match self {
            Self::Register => printer.print_esc_str(b"rawstring"),
            Self::Variable(variable) => printer.print_esc_str(&variable.to_string()),
        }
    }
}

impl Dumpable for RawStringCommand {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Register => writeln!(target, "Register")?,
            Self::Variable(variable) => {
                writeln!(target, "Variable")?;
                variable.dump(target)?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "Register" => Ok(Self::Register),
            "Variable" => Ok(Self::Variable(RawStringVariable::undump(lines)?)),
            _ => Err(FormatError::ParseError),
        }
    }
}

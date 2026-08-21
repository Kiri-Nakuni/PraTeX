use crate::print::Printer;
use crate::macros::Macro;

use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCall {
    pub long: bool,
    pub outer: bool,
    /// e-TeX の `\protected`。**展開する走査で展開されない**
    pub protected: bool,
    pub macro_def: Rc<Macro>,
}

impl MacroCall {
    pub fn display(&self, printer: &mut impl Printer) {
        // **前置詞のあとに空白を置く。** `\protected macro:->…` と出る
        if self.protected {
            if self.long || self.outer {
                printer.print_esc_str(b"protected");
            } else {
                printer.print_esc_str(b"protected ");
            }
        }
        if self.long {
            if self.outer {
                printer.print_esc_str(b"long");
                printer.print_esc_str(b"outer ");
            } else {
                printer.print_esc_str(b"long ");
            }
        } else if self.outer {
            printer.print_esc_str(b"outer ");
        }
        printer.print_str("macro");
    }
}

use crate::print::Printer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkClassOperand {
    /// The unnumbered TeX82 command, equivalent to class zero.
    Zero,
    /// An e-TeX command that scans a 15-bit class number when executed.
    Scan,
}

impl MarkClassOperand {
    pub fn is_classed(self) -> bool {
        matches!(self, Self::Scan)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkQuery {
    Top,
    First,
    Bot,
    SplitFirst,
    SplitBot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkCommand {
    pub query: MarkQuery,
    pub class: MarkClassOperand,
}

impl MarkCommand {
    pub const fn new(query: MarkQuery, class: MarkClassOperand) -> Self {
        Self { query, class }
    }

    pub fn display(&self, printer: &mut impl Printer) {
        let s: &[u8] = match (self.query, self.class) {
            (MarkQuery::Top, MarkClassOperand::Zero) => b"topmark",
            (MarkQuery::First, MarkClassOperand::Zero) => b"firstmark",
            (MarkQuery::Bot, MarkClassOperand::Zero) => b"botmark",
            (MarkQuery::SplitFirst, MarkClassOperand::Zero) => b"splitfirstmark",
            (MarkQuery::SplitBot, MarkClassOperand::Zero) => b"splitbotmark",
            (MarkQuery::Top, MarkClassOperand::Scan) => b"topmarks",
            (MarkQuery::First, MarkClassOperand::Scan) => b"firstmarks",
            (MarkQuery::Bot, MarkClassOperand::Scan) => b"botmarks",
            (MarkQuery::SplitFirst, MarkClassOperand::Scan) => b"splitfirstmarks",
            (MarkQuery::SplitBot, MarkClassOperand::Scan) => b"splitbotmarks",
        };
        printer.print_esc_str(s);
    }
}

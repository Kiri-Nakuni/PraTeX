use crate::print::Printer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IfTest {
    IfChar,
    IfCat,
    IfInt,
    IfDim,
    IfOdd,
    IfVmode,
    IfHmode,
    IfMmode,
    IfInner,
    IfVoid,
    IfHbox,
    IfVbox,
    Ifx,
    IfEof,
    IfTrue,
    IfFalse,
    IfCase,
    // ==== e-TeX ====
    /// `\ifdefined\cs` — **未定義でも作らない**
    IfDefined,
    /// `\ifcsname … \endcsname` — 同上
    IfCsName,
    /// `\iffontchar\font 番号`
    IfFontChar,
}

impl IfTest {
    /// e-TeX の `\currentiftype` が使う番号（0 起点）。
    /// **並びは TeX の順そのままである。**
    pub fn etex_code(&self) -> i32 {
        match self {
            Self::IfChar => 0,
            Self::IfCat => 1,
            Self::IfInt => 2,
            Self::IfDim => 3,
            Self::IfOdd => 4,
            Self::IfVmode => 5,
            Self::IfHmode => 6,
            Self::IfMmode => 7,
            Self::IfInner => 8,
            Self::IfVoid => 9,
            Self::IfHbox => 10,
            Self::IfVbox => 11,
            Self::Ifx => 12,
            Self::IfEof => 13,
            Self::IfTrue => 14,
            Self::IfFalse => 15,
            Self::IfCase => 16,
            // e-TeX は 17 以降に足した
            Self::IfDefined => 17,
            Self::IfCsName => 18,
            Self::IfFontChar => 19,
        }
    }

    pub fn display(&self, printer: &mut impl Printer) {
        let s: &[u8] = match self {
            Self::IfChar => b"if",
            Self::IfCat => b"ifcat",
            Self::IfInt => b"ifnum",
            Self::IfDim => b"ifdim",
            Self::IfOdd => b"ifodd",
            Self::IfVmode => b"ifvmode",
            Self::IfHmode => b"ifhmode",
            Self::IfMmode => b"ifmmode",
            Self::IfInner => b"ifinner",
            Self::IfVoid => b"ifvoid",
            Self::IfHbox => b"ifhbox",
            Self::IfVbox => b"ifvbox",
            Self::Ifx => b"ifx",
            Self::IfEof => b"ifeof",
            Self::IfTrue => b"iftrue",
            Self::IfFalse => b"iffalse",
            Self::IfCase => b"ifcase",
            Self::IfDefined => b"ifdefined",
            Self::IfCsName => b"ifcsname",
            Self::IfFontChar => b"iffontchar",
        };
        printer.print_esc_str(s);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiOrElse {
    Fi,
    Or,
    Else,
}

impl FiOrElse {
    pub fn display(&self, printer: &mut impl Printer) {
        let s: &[u8] = match self {
            Self::Fi => b"fi",
            Self::Or => b"or",
            Self::Else => b"else",
        };
        printer.print_esc_str(s);
    }
}

use super::box_dimension::BoxDimension;
use super::page_dimension::PageDimension;
use crate::eqtb::{
    CodeType, DimensionVariable, FontIndex, IntegerVariable, MathFontSize, SkipVariable,
    TokenListVariable,
};
use crate::scan_internal::ValueType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalCommand {
    CharGiven(u8),
    MathCharGiven(u16),
    LastPenalty,
    LastKern,
    LastSkip,
    Toks(ToksCommand),
    Badness,
    // ==== e-TeX / pdfTeX の問い合わせ ====
    /// `\eTeXversion`
    ETeXVersion,
    /// `\pdfshellescape`（読み取り専用）
    PdfShellEscape,
    /// `\currentgrouplevel`
    CurrentGroupLevel,
    /// `\currentgrouptype`
    CurrentGroupType,
    /// `\currentiflevel`
    CurrentIfLevel,
    /// `\currentiftype`
    CurrentIfType,
    /// `\currentifbranch`
    CurrentIfBranch,
    /// `\lastnodetype`
    LastNodeType,
    /// `\interactionmode`
    InteractionMode,
    InputLineNumber,
    Integer(IntegerVariable),
    Dimension(DimensionVariable),
    Glue(SkipVariable),
    MuGlue(SkipVariable),
    FontDimen,
    HyphenChar,
    SkewChar,
    SpaceFactor,
    PrevDepth,
    PrevGraf,
    PageDimen(PageDimension),
    DeadCycles,
    InsertPenalties,
    BoxDimen(BoxDimension),
    ParShape,
    CatCode,
    Code(CodeType),
    Register(ValueType),
    /// e-TeX の `\numexpr` `\dimexpr` `\glueexpr` `\muexpr`。
    ///
    /// **内部量として振る舞う。** `\count0=\numexpr 1+2\relax` のように、
    /// 値が要る場所ならどこにでも書ける。
    Expr(ValueType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToksCommand {
    TokenListRegister,
    TokenList(TokenListVariable),
    DefFamily(MathFontSize),
    SetFont(FontIndex),
    DefFont,
}

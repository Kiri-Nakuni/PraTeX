use super::box_dimension::BoxDimension;
use super::glue_component::GlueComponent;
use super::glue_conversion::GlueConversion;
use super::page_dimension::PageDimension;
use super::raw_string::RawStringCommand;
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
    RawString(RawStringCommand),
    Badness,
    // ==== e-TeX / pdfTeX の問い合わせ ====
    /// `\eTeXversion`
    ETeXVersion,
    /// `\pratexversion` — PraTeX 自身を識別する読み取り専用の版番号。
    PraTeXVersion,
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
    /// `\gluestretch` などの糊成分問い合わせ
    GlueComponent(GlueComponent),
    /// `\mutoglue` と `\gluetomu` の糊型変換
    GlueConversion(GlueConversion),
    InputLineNumber,
    Integer(IntegerVariable),
    LanguageRegion,
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
    KCatCode,
    XspCode,
    InhibitXspCode,
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

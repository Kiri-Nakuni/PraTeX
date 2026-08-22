mod arithmetic;
mod box_dimension;
mod conditionals;
mod convert;
mod def;
mod fraction;
mod glue_component;
mod glue_conversion;
mod internal;
mod limits;
mod macro_call;
mod make_box;
mod mark;
mod math_command;
mod page_dimension;
mod prefix;
pub(crate) mod prefixable;
mod remove_item;
mod shorthand_def;
mod show;
mod skip;

pub use arithmetic::ArithCommand;
pub use box_dimension::BoxDimension;
pub use conditionals::{FiOrElse, IfTest};
pub use convert::ConvertCommand;
pub use def::DefCommand;
pub use fraction::{FractionCommand, FractionType};
pub use glue_component::GlueComponent;
pub use glue_conversion::GlueConversion;
pub use internal::{InternalCommand, ToksCommand};
pub use limits::LimitType;
pub use macro_call::MacroCall;
pub use make_box::MakeBox;
pub use mark::{MarkClassOperand, MarkCommand, MarkQuery};
pub use math_command::MathCommand;
pub use page_dimension::PageDimension;
pub use prefix::Prefix;
pub use prefixable::{prefixed_command, PrefixableCommand};
pub use remove_item::RemoveItem;
pub use shorthand_def::ShorthandDef;
pub use show::ShowCommand;
pub use skip::{Hskip, Vskip};

use crate::fonts::FontInfo;
use crate::nodes::LeaderKind;
use crate::print::Printer;
use crate::token::CjkToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Unexpandable(UnexpandableCommand),
    Expandable(ExpandableCommand),
}

impl Command {
    /// See 298.
    pub fn display(&self, fonts: &Vec<FontInfo>, printer: &mut impl Printer) {
        match self {
            Self::Unexpandable(unexpandable_command) => {
                unexpandable_command.display(fonts, printer)
            }
            Self::Expandable(expandable_command) => expandable_command.display(printer),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnexpandableCommand {
    Relax { no_expand: bool },
    LeftBrace(u8),
    RightBrace(u8),
    MathShift(u8),
    TabMark(u8),
    Span,
    CarRet { weak: bool },
    MacParam(u8),
    Spacer,
    Letter(u8),
    Other(u8),
    CjkChar(CjkToken),
    ParEnd,
    Endv,
    CharNum,
    Math(MathCommand),
    /// e-TeX の式（`\numexpr` 系）。**内部量として振る舞う**
    Expr(crate::scan_internal::ValueType),
    /// e-TeX の糊成分問い合わせ。**内部量として振る舞う**
    GlueComponent(GlueComponent),
    /// e-TeX の通常糊・数式糊変換。**内部量として振る舞う**
    GlueConversion(GlueConversion),
    Kern,
    ShipOut,
    Leaders(LeaderKind),
    UpperCase,
    LowerCase,
    Mark(MarkClassOperand),
    MakeBox(MakeBox),
    MoveLeft,
    MoveRight,
    Raise,
    Lower,
    Show(ShowCommand),
    UnHbox { copy: bool },
    UnVbox { copy: bool },
    RemoveItem(RemoveItem),
    Hskip(Hskip),
    Vskip(Vskip),
    Halign,
    Valign,
    NoAlign,
    Vrule,
    Hrule,
    Insert,
    Vadjust,
    IgnoreSpaces,
    AfterAssignment,
    AfterGroup,
    Penalty,
    StartPar { is_indented: bool },
    ItalCorr,
    Accent,
    HyphenBreak,
    Discretionary,
    EqNo { is_left: bool },
    Message { is_err: bool },
    OpenOut,
    Write,
    CloseOut,
    Special,
    SetLanguage,
    Immediate,
    OpenIn,
    CloseIn,
    BeginGroup,
    EndGroup,
    Omit,
    ExSpace,
    NoBoundary,
    EndCsName,
    CharGiven(u8),
    LastPenalty,
    LastKern,
    LastSkip,
    Badness,
    // ==== e-TeX / pdfTeX の問い合わせ ====
    ETeXVersion,
    PdfShellEscape,
    CurrentGroupLevel,
    CurrentGroupType,
    CurrentIfLevel,
    CurrentIfType,
    CurrentIfBranch,
    LastNodeType,
    InputLineNumber,
    End { dumping: bool },
    Prefixable(PrefixableCommand),
}

impl UnexpandableCommand {
    /// If the command is "internal", return the corresponding InternalCommand.
    pub fn try_to_internal(&self) -> Option<InternalCommand> {
        match self {
            Self::Relax { .. }
            | Self::LeftBrace(_)
            | Self::RightBrace(_)
            | Self::MathShift(_)
            | Self::TabMark(_)
            | Self::Span
            | Self::CarRet { .. }
            | Self::MacParam(_)
            | Self::Spacer
            | Self::Letter(_)
            | Self::Other(_)
            | Self::CjkChar(_)
            | Self::ParEnd
            | Self::Endv
            | Self::CharNum
            | Self::Kern
            | Self::ShipOut
            | Self::Leaders(_)
            | Self::UpperCase
            | Self::LowerCase
            | Self::Mark(_)
            | Self::MakeBox(_)
            | Self::MoveLeft
            | Self::MoveRight
            | Self::Raise
            | Self::Lower
            | Self::Show(_)
            | Self::UnHbox { .. }
            | Self::UnVbox { .. }
            | Self::RemoveItem(_)
            | Self::Hskip(_)
            | Self::Vskip(_)
            | Self::Halign
            | Self::Valign
            | Self::NoAlign
            | Self::Vrule
            | Self::Hrule
            | Self::Insert
            | Self::Vadjust
            | Self::IgnoreSpaces
            | Self::AfterAssignment
            | Self::AfterGroup
            | Self::Penalty
            | Self::StartPar { .. }
            | Self::ItalCorr
            | Self::Accent
            | Self::HyphenBreak
            | Self::Discretionary
            | Self::EqNo { .. }
            | Self::Message { .. }
            | Self::OpenOut
            | Self::Write
            | Self::CloseOut
            | Self::Special
            | Self::SetLanguage
            | Self::Immediate
            | Self::OpenIn
            | Self::CloseIn
            | Self::BeginGroup
            | Self::EndGroup
            | Self::Omit
            | Self::ExSpace
            | Self::NoBoundary
            | Self::EndCsName
            | Self::End { .. } => None,
            Self::Math(math_command) => math_command.try_to_internal(),
            Self::CharGiven(c) => Some(InternalCommand::CharGiven(*c)),
            Self::LastPenalty => Some(InternalCommand::LastPenalty),
            Self::LastKern => Some(InternalCommand::LastKern),
            Self::LastSkip => Some(InternalCommand::LastSkip),
            Self::Badness => Some(InternalCommand::Badness),
            Self::ETeXVersion => Some(InternalCommand::ETeXVersion),
            Self::PdfShellEscape => Some(InternalCommand::PdfShellEscape),
            Self::CurrentGroupLevel => Some(InternalCommand::CurrentGroupLevel),
            Self::CurrentGroupType => Some(InternalCommand::CurrentGroupType),
            Self::CurrentIfLevel => Some(InternalCommand::CurrentIfLevel),
            Self::CurrentIfType => Some(InternalCommand::CurrentIfType),
            Self::CurrentIfBranch => Some(InternalCommand::CurrentIfBranch),
            Self::LastNodeType => Some(InternalCommand::LastNodeType),
            Self::GlueComponent(component) => Some(InternalCommand::GlueComponent(*component)),
            Self::GlueConversion(conversion) => {
                Some(InternalCommand::GlueConversion(*conversion))
            }
            Self::InputLineNumber => Some(InternalCommand::InputLineNumber),
            Self::Prefixable(prefixable_command) => prefixable_command.try_to_internal(),
            &Self::Expr(kind) => Some(InternalCommand::Expr(kind)),
        }
    }

    /// See 298.
    pub fn display(&self, fonts: &Vec<FontInfo>, printer: &mut impl Printer) {
        match self {
            Self::Relax { .. } => printer.print_esc_str(b"relax"),
            Self::LeftBrace(c) => chr_cmd("begin-group character ", *c, printer),
            Self::RightBrace(c) => chr_cmd("end-group character ", *c, printer),
            Self::MathShift(c) => chr_cmd("math shift character ", *c, printer),
            Self::TabMark(c) => {
                printer.print_str("alignment tab character ");
                printer.print(*c);
            }
            Self::Span => printer.print_esc_str(b"span"),
            Self::CarRet { weak } => {
                if *weak {
                    printer.print_esc_str(b"crcr");
                } else {
                    printer.print_esc_str(b"cr");
                }
            }
            Self::MacParam(c) => chr_cmd("macro parameter character ", *c, printer),
            Self::Spacer => chr_cmd("blank space ", b' ', printer),
            Self::Letter(c) => chr_cmd("the letter ", *c, printer),
            Self::Other(c) => chr_cmd("the character ", *c, printer),
            Self::CjkChar(token) => {
                printer.print_str("kanji character ");
                token.print_utf8(printer);
            }
            Self::ParEnd => printer.print_esc_str(b"par"),
            Self::Endv => printer.print_str("end of alignment template"),
            Self::CharNum => printer.print_esc_str(b"char"),
            Self::Math(math_command) => math_command.display(printer),
            Self::Kern => printer.print_esc_str(b"kern"),
            Self::Leaders(leader_kind) => leader_kind.display(printer),
            Self::ShipOut => printer.print_esc_str(b"shipout"),
            Self::UpperCase => printer.print_esc_str(b"uppercase"),
            Self::LowerCase => printer.print_esc_str(b"lowercase"),
            Self::Mark(class) => {
                printer.print_esc_str(if class.is_classed() { b"marks" } else { b"mark" })
            }
            Self::MakeBox(make_box) => make_box.display(printer),
            Self::MoveLeft => printer.print_esc_str(b"moveleft"),
            Self::MoveRight => printer.print_esc_str(b"moveright"),
            Self::Raise => printer.print_esc_str(b"raise"),
            Self::Lower => printer.print_esc_str(b"lower"),
            Self::Show(show_command) => show_command.display(printer),
            Self::UnHbox { copy } => {
                if *copy {
                    printer.print_esc_str(b"unhcopy");
                } else {
                    printer.print_esc_str(b"unhbox");
                }
            }
            Self::UnVbox { copy } => {
                if *copy {
                    printer.print_esc_str(b"unvcopy");
                } else {
                    printer.print_esc_str(b"unvbox");
                }
            }
            Self::RemoveItem(remove_item) => remove_item.display(printer),
            Self::Hskip(hskip) => hskip.display(printer),
            Self::Vskip(vskip) => vskip.display(printer),
            Self::Halign => printer.print_esc_str(b"halign"),
            Self::Valign => printer.print_esc_str(b"valign"),
            Self::NoAlign => printer.print_esc_str(b"noalign"),
            Self::Vrule => printer.print_esc_str(b"vrule"),
            Self::Hrule => printer.print_esc_str(b"hrule"),
            Self::Insert => printer.print_esc_str(b"insert"),
            Self::Vadjust => printer.print_esc_str(b"vadjust"),
            Self::IgnoreSpaces => printer.print_esc_str(b"ignorespaces"),
            Self::AfterAssignment => printer.print_esc_str(b"afterassignment"),
            Self::AfterGroup => printer.print_esc_str(b"aftergroup"),
            Self::Penalty => printer.print_esc_str(b"penalty"),
            Self::StartPar { is_indented } => {
                if *is_indented {
                    printer.print_esc_str(b"indent");
                } else {
                    printer.print_esc_str(b"noindent");
                }
            }
            Self::ItalCorr => printer.print_esc_str(b"/"),
            Self::Accent => printer.print_esc_str(b"accent"),
            Self::HyphenBreak => printer.print_esc_str(b"-"),
            Self::Discretionary => printer.print_esc_str(b"discretionary"),
            Self::EqNo { is_left } => {
                if *is_left {
                    printer.print_esc_str(b"leqno");
                } else {
                    printer.print_esc_str(b"eqno");
                }
            }
            Self::Message { is_err } => {
                if *is_err {
                    printer.print_esc_str(b"errmessage");
                } else {
                    printer.print_esc_str(b"message");
                }
            }
            Self::OpenOut => printer.print_esc_str(b"openout"),
            Self::Write => printer.print_esc_str(b"write"),
            Self::CloseOut => printer.print_esc_str(b"closeout"),
            Self::Special => printer.print_esc_str(b"special"),
            Self::SetLanguage => printer.print_esc_str(b"setlanguage"),
            Self::Immediate => printer.print_esc_str(b"immediate"),
            Self::OpenIn => printer.print_esc_str(b"openin"),
            Self::CloseIn => printer.print_esc_str(b"closein"),
            Self::BeginGroup => printer.print_esc_str(b"begingroup"),
            Self::EndGroup => printer.print_esc_str(b"endgroup"),
            Self::Omit => printer.print_esc_str(b"omit"),
            Self::ExSpace => printer.print_esc_str(b" "),
            Self::NoBoundary => printer.print_esc_str(b"noboundary"),
            Self::EndCsName => printer.print_esc_str(b"endcsname"),
            Self::CharGiven(c) => {
                printer.print_esc_str(b"char");
                printer.print_hex(*c as i32);
            }
            Self::LastPenalty => printer.print_esc_str(b"lastpenalty"),
            Self::LastKern => printer.print_esc_str(b"lastkern"),
            Self::LastSkip => printer.print_esc_str(b"lastskip"),
            Self::Badness => printer.print_esc_str(b"badness"),
            Self::ETeXVersion => printer.print_esc_str(b"eTeXversion"),
            Self::PdfShellEscape => printer.print_esc_str(b"pdfshellescape"),
            Self::CurrentGroupLevel => printer.print_esc_str(b"currentgrouplevel"),
            Self::CurrentGroupType => printer.print_esc_str(b"currentgrouptype"),
            Self::CurrentIfLevel => printer.print_esc_str(b"currentiflevel"),
            Self::CurrentIfType => printer.print_esc_str(b"currentiftype"),
            Self::CurrentIfBranch => printer.print_esc_str(b"currentifbranch"),
            Self::LastNodeType => printer.print_esc_str(b"lastnodetype"),
            Self::Expr(kind) => printer.print_esc_str(match kind {
                crate::scan_internal::ValueType::Int => b"numexpr".as_slice(),
                crate::scan_internal::ValueType::Dimen => b"dimexpr".as_slice(),
                crate::scan_internal::ValueType::Glue => b"glueexpr".as_slice(),
                crate::scan_internal::ValueType::Mu => b"muexpr".as_slice(),
            }),
            Self::GlueComponent(component) => component.display(printer),
            Self::GlueConversion(conversion) => conversion.display(printer),
            Self::InputLineNumber => printer.print_esc_str(b"inputlineno"),
            Self::End { dumping } => {
                if *dumping {
                    printer.print_esc_str(b"dump");
                } else {
                    printer.print_esc_str(b"end");
                }
            }
            &Self::Prefixable(prefixable_command) => prefixable_command.display(fonts, printer),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandableCommand {
    Undefined,
    ExpandAfter,
    NoExpand,
    Input,
    EndInput,
    IfTest(IfTest),
    FiOrElse(FiOrElse),
    CsName,
    Convert(ConvertCommand),
    The,
    Mark(MarkCommand),
    Macro(MacroCall),
    EndTemplate,
    /// `\directvaak{…}`。**中身を Vaak として走らせ、終了コードを10進で展開する。**
    ///
    /// 展開可能なのは、数値の走査中に呼ばれうるからである——
    /// `\count0=\directvaak{…}` で展開が空だと走査が壊れる。
    /// したがって**必ず数字を出す**（エラーでも `0`）。
    DirectVaak,
    /// `\vaakinput 名前.vaak` — **ファイルを読んで走らせる。**
    ///
    /// `\directvaak{…}` との違いは**字句器を通らない**ことである。
    ///
    /// | | `\directvaak{…}` | `\vaakinput` |
    /// |---|---|---|
    /// | 改行 | 空白に潰れる | **残る** |
    /// | `%` | TeX の注釈。閉じ括弧まで食う | **Vaak の注釈** |
    /// | `#` `$` `~` | TeX の意味で解釈される | **ただの文字** |
    /// | 名前空間の印 | 本体の中で発火する | **しない** |
    /// | マクロで組み立てる | **できる** | できない |
    ///
    /// LuaTeX が「長い Lua は別ファイルに置け」と言うのと同じ分担である——
    /// ただし**こちらは字句器を通さないので、本当に元のまま読める。**
    VaakInput,
    /// `\vaakdef\名前{本体}` で作られた命令。**本体は定義の時点で組んである。**
    ///
    /// `\directvaak{…}` は呼ぶたびに本体を字句として走査し直す——
    /// 同じ 24 字を組み直すのに、走らせる費用の倍が掛かっていた。
    /// **名前を付ければ、呼ぶときに本体を見ない。**
    ///
    /// 番号は本体の**内容で共有される**ので、同じ本体の二つの名前は `\ifx` で等しい。
    VaakCall(u32),
    /// `\namespace 名前\csname … \endcsname`。
    ///
    /// **名前空間の名前を集め、`\csname` の登録処理に介入する。**
    ///
    /// `\endcsname` を終端にする案を退けたのは、
    /// **`\csname` が global に作ってしまう**からである——
    /// 登録は `\endcsname` に達した一箇所で起きるので、そこへ名前空間を渡すしかない。
    Namespace,
    /// `\detokenize{…}` — **字句に直した文字列にする**（e-TeX）。
    ///
    /// 中身は展開しない。**`\string` を全部に掛けたのと同じ**である。
    Detokenize,
    /// `\unexpanded{…}` — **展開する走査の中でも展開しない**（e-TeX）。
    ///
    /// `\edef` の中で書くと、中身がそのまま写る。
    Unexpanded,
    /// `\expanded{…}` — **中身を展開しきってから戻す。**
    ///
    /// `\edef` と違って**名前に束縛しない。** その場に置き直す。
    /// expl3 が最も使う命令の一つである（本体で 23 箇所）。
    Expanded,
    /// e-TeX の `\unless` — **次の条件を反転する。**
    ///
    /// 取れるのは `\if…` だけである（`\ifcase` は取れない——腕が複数あるので）。
    Unless,
}

impl ExpandableCommand {
    /// See 266., 377., 385., 469., 488., 492., and 1295.
    pub fn display(&self, printer: &mut impl Printer) {
        match self {
            Self::Undefined => printer.print_str("undefined"),
            Self::ExpandAfter => printer.print_esc_str(b"expandafter"),
            Self::NoExpand => printer.print_esc_str(b"noexpand"),
            Self::Input => printer.print_esc_str(b"input"),
            Self::EndInput => printer.print_esc_str(b"endinput"),
            Self::IfTest(if_test) => if_test.display(printer),
            Self::FiOrElse(fi_or_else) => fi_or_else.display(printer),
            Self::CsName => printer.print_esc_str(b"csname"),
            Self::Convert(convert_command) => convert_command.display(printer),
            Self::The => printer.print_esc_str(b"the"),
            Self::DirectVaak => printer.print_esc_str(b"directvaak"),
            Self::VaakInput => printer.print_esc_str(b"vaakinput"),
            Self::Namespace => printer.print_esc_str(b"namespace"),
            Self::Unless => printer.print_esc_str(b"unless"),
            Self::Expanded => printer.print_esc_str(b"expanded"),
            Self::Detokenize => printer.print_esc_str(b"detokenize"),
            Self::Unexpanded => printer.print_esc_str(b"unexpanded"),
            &Self::VaakCall(id) => {
                printer.print_str("vaak:->");
                for c in crate::vaak::source_of(id) {
                    printer.print(c);
                }
            }
            Self::Mark(mark_command) => mark_command.display(printer),
            Self::Macro(macro_call) => macro_call.display(printer),
            Self::EndTemplate => printer.print_esc_str(b"outer endtemplate"),
        }
    }
}

/// This is a helper function for printing a command and a character.
/// See 298.
fn chr_cmd(command_description: &str, character: u8, printer: &mut impl Printer) {
    printer.print_str(command_description);
    printer.print(character);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::string::StringPrinter;
    use crate::token::CjkCategory;

    #[test]
    fn 和文文字命令をuptex互換のバイト列で表示する() {
        let cases: &[(u32, &[u8])] = &[
            (0x3042, b"kanji character \xE3\x81\x82"),
            (0xD800, b"kanji character \xED\xA0\x80"),
        ];
        for &(code_point, expected) in cases {
            let token = CjkToken::new(code_point, CjkCategory::Kana).unwrap();
            let command = UnexpandableCommand::CjkChar(token);
            let mut printer = StringPrinter::new(Some(b'\\'));
            command.display(&Vec::new(), &mut printer);
            assert_eq!(printer.into_string(), expected, "U+{code_point:04X}");
        }
    }
}

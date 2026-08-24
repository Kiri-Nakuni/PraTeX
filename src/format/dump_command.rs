use super::{Dumpable, FormatError};
use crate::command::{
    Command, ConvertCommand, ExpandableCommand, FiOrElse, FontCharDimension, GlueComponent,
    GlueConversion, Hskip, IfTest, MacroCall, MakeBox, MarkClassOperand, MarkCommand, MarkQuery,
    MathCommand, ParShapeDimension, PrefixableCommand, RemoveItem, ShowCommand,
    TextDirectionCommand, UnexpandableCommand, VerticalDiscardKind, Vskip,
};
use crate::eqtb::CatCode;
use crate::nodes::LeaderKind;
use crate::token::{CjkToken, LatinUcsToken};

use std::io::Write;

fn undump_latin_command<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    allowed_cat_codes: &[CatCode],
) -> Result<LatinUcsToken, FormatError> {
    let token = LatinUcsToken::undump(lines)?;
    if allowed_cat_codes.contains(&token.cat_code()) {
        Ok(token)
    } else {
        Err(FormatError::ParseError)
    }
}

impl Dumpable for Command {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Unexpandable(unexpandable_command) => {
                writeln!(target, "Unexpandable")?;
                unexpandable_command.dump(target)?;
            }
            Self::Expandable(expandable_command) => {
                writeln!(target, "Expandable")?;
                expandable_command.dump(target)?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Unexpandable" => {
                let unexpandable_command = UnexpandableCommand::undump(lines)?;
                Ok(Self::Unexpandable(unexpandable_command))
            }
            "Expandable" => {
                let expandable_command = ExpandableCommand::undump(lines)?;
                Ok(Self::Expandable(expandable_command))
            }
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for UnexpandableCommand {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Relax { no_expand } => {
                writeln!(target, "Relax")?;
                no_expand.dump(target)?;
            }
            Self::LeftBrace(c) => {
                writeln!(target, "LeftBrace")?;
                c.dump(target)?;
            }
            Self::RightBrace(c) => {
                writeln!(target, "RightBrace")?;
                c.dump(target)?;
            }
            Self::MathShift(c) => {
                writeln!(target, "MathShift")?;
                c.dump(target)?;
            }
            Self::TabMark(c) => {
                writeln!(target, "TabMark")?;
                c.dump(target)?;
            }
            Self::Span => writeln!(target, "Span")?,
            Self::CarRet { weak } => {
                writeln!(target, "CarRet")?;
                weak.dump(target)?;
            }
            Self::MacParam(c) => {
                writeln!(target, "MacParam")?;
                c.dump(target)?;
            }
            Self::Spacer => writeln!(target, "Spacer")?,
            Self::Letter(c) => {
                writeln!(target, "Letter")?;
                c.dump(target)?;
            }
            Self::Other(c) => {
                writeln!(target, "Other")?;
                c.dump(target)?;
            }
            Self::LatinUcsChar(token) => {
                writeln!(target, "LatinUcsChar")?;
                token.dump(target)?;
            }
            Self::LatinUcsLeftBrace(token) => {
                writeln!(target, "LatinUcsLeftBrace")?;
                token.dump(target)?;
            }
            Self::LatinUcsRightBrace(token) => {
                writeln!(target, "LatinUcsRightBrace")?;
                token.dump(target)?;
            }
            Self::LatinUcsMathShift(token) => {
                writeln!(target, "LatinUcsMathShift")?;
                token.dump(target)?;
            }
            Self::LatinUcsTabMark(token) => {
                writeln!(target, "LatinUcsTabMark")?;
                token.dump(target)?;
            }
            Self::LatinUcsMacParam(token) => {
                writeln!(target, "LatinUcsMacParam")?;
                token.dump(target)?;
            }
            Self::LatinUcsSupMark(token) => {
                writeln!(target, "LatinUcsSupMark")?;
                token.dump(target)?;
            }
            Self::LatinUcsSubMark(token) => {
                writeln!(target, "LatinUcsSubMark")?;
                token.dump(target)?;
            }
            Self::CjkChar(token) => {
                writeln!(target, "CjkChar")?;
                token.dump(target)?;
            }
            Self::ParEnd => writeln!(target, "ParEnd")?,
            Self::Endv => writeln!(target, "Endv")?,
            Self::CharNum => writeln!(target, "CharNum")?,
            Self::Math(math_command) => {
                writeln!(target, "Math")?;
                math_command.dump(target)?;
            }
            Self::Kern => writeln!(target, "Kern")?,
            Self::ShipOut => writeln!(target, "ShipOut")?,
            Self::Leaders(leader_kind) => {
                writeln!(target, "Leaders")?;
                leader_kind.dump(target)?;
            }
            Self::UpperCase => writeln!(target, "UpperCase")?,
            Self::LowerCase => writeln!(target, "LowerCase")?,
            Self::Raise => writeln!(target, "Raise")?,
            Self::Lower => writeln!(target, "Lower")?,
            Self::Mark(class) => {
                writeln!(target, "Mark")?;
                class.dump(target)?;
            }
            Self::MakeBox(make_box) => {
                writeln!(target, "MakeBox")?;
                make_box.dump(target)?;
            }
            Self::MoveLeft => writeln!(target, "MoveLeft")?,
            Self::MoveRight => writeln!(target, "MoveRight")?,
            Self::Show(show_command) => {
                writeln!(target, "Show")?;
                show_command.dump(target)?;
            }
            Self::UnHbox { copy } => {
                writeln!(target, "UnHbox")?;
                copy.dump(target)?;
            }
            Self::UnVbox { copy } => {
                writeln!(target, "UnVbox")?;
                copy.dump(target)?;
            }
            Self::VDiscards(kind) => {
                writeln!(target, "VDiscards")?;
                kind.dump(target)?;
            }
            Self::RemoveItem(remove_item) => {
                writeln!(target, "RemoveItem")?;
                remove_item.dump(target)?;
            }
            Self::Hskip(hskip) => {
                writeln!(target, "Hskip")?;
                hskip.dump(target)?;
            }
            Self::Vskip(vskip) => {
                writeln!(target, "Vskip")?;
                vskip.dump(target)?;
            }
            Self::Halign => writeln!(target, "Halign")?,
            Self::Valign => writeln!(target, "Valign")?,
            Self::NoAlign => writeln!(target, "NoAlign")?,
            Self::Vrule => writeln!(target, "Vrule")?,
            Self::Hrule => writeln!(target, "Hrule")?,
            Self::Insert => writeln!(target, "Insert")?,
            Self::Vadjust => writeln!(target, "Vadjust")?,
            Self::IgnoreSpaces => writeln!(target, "IgnoreSpaces")?,
            Self::AfterAssignment => writeln!(target, "AfterAssignment")?,
            Self::AfterGroup => writeln!(target, "AfterGroup")?,
            Self::Penalty => writeln!(target, "Penalty")?,
            Self::StartPar {
                is_indented: indent,
            } => {
                writeln!(target, "StartPar")?;
                indent.dump(target)?;
            }
            Self::ItalCorr => writeln!(target, "ItalCorr")?,
            Self::Accent => writeln!(target, "Accent")?,
            Self::HyphenBreak => writeln!(target, "HyphenBreak")?,
            Self::Discretionary => writeln!(target, "Discretionary")?,
            Self::EqNo { is_left } => {
                writeln!(target, "EqNo")?;
                is_left.dump(target)?;
            }
            Self::Message { is_err } => {
                writeln!(target, "Message")?;
                is_err.dump(target)?;
            }
            Self::OpenOut => writeln!(target, "OpenOut")?,
            Self::Write => writeln!(target, "Write")?,
            Self::CloseOut => writeln!(target, "CloseOut")?,
            Self::Special => writeln!(target, "Special")?,
            Self::SetLanguage => writeln!(target, "SetLanguage")?,
            Self::TextDirection(direction) => {
                writeln!(target, "TextDirection")?;
                direction.dump(target)?;
            }
            Self::Immediate => writeln!(target, "Immediate")?,
            Self::OpenIn => writeln!(target, "OpenIn")?,
            Self::CloseIn => writeln!(target, "CloseIn")?,
            Self::BeginGroup => writeln!(target, "BeginGroup")?,
            Self::EndGroup => writeln!(target, "EndGroup")?,
            Self::Omit => writeln!(target, "Omit")?,
            Self::ExSpace => writeln!(target, "ExSpace")?,
            Self::NoBoundary => writeln!(target, "NoBoundary")?,
            Self::EndCsName => writeln!(target, "EndCsName")?,
            Self::CharGiven(c) => {
                writeln!(target, "CharGiven")?;
                c.dump(target)?;
            }
            Self::LastPenalty => writeln!(target, "LastPenalty")?,
            Self::LastKern => writeln!(target, "LastKern")?,
            Self::LastSkip => writeln!(target, "LastSkip")?,
            Self::Badness => writeln!(target, "Badness")?,
            Self::FontCharDimension(dimension) => {
                writeln!(target, "FontCharDimension")?;
                dimension.dump(target)?;
            }
            Self::ParShapeDimension(dimension) => {
                writeln!(target, "ParShapeDimension")?;
                dimension.dump(target)?;
            }
            Self::ETeXVersion => writeln!(target, "ETeXVersion")?,
            Self::PraTeXVersion => writeln!(target, "PraTeXVersion")?,
            Self::PdfShellEscape => writeln!(target, "PdfShellEscape")?,
            Self::CurrentGroupLevel => writeln!(target, "CurrentGroupLevel")?,
            Self::CurrentGroupType => writeln!(target, "CurrentGroupType")?,
            Self::CurrentIfLevel => writeln!(target, "CurrentIfLevel")?,
            Self::CurrentIfType => writeln!(target, "CurrentIfType")?,
            Self::CurrentIfBranch => writeln!(target, "CurrentIfBranch")?,
            Self::LastNodeType => writeln!(target, "LastNodeType")?,
            Self::Expr(kind) => {
                writeln!(target, "Expr")?;
                kind.dump(target)?;
            }
            Self::GlueComponent(component) => {
                writeln!(target, "GlueComponent")?;
                component.dump(target)?;
            }
            Self::GlueConversion(conversion) => {
                writeln!(target, "GlueConversion")?;
                conversion.dump(target)?;
            }
            Self::InputLineNumber => writeln!(target, "InputLineNumber")?,
            Self::End { dumping } => {
                writeln!(target, "End")?;
                dumping.dump(target)?;
            }
            Self::Prefixable(prefixable_command) => {
                writeln!(target, "Prefixable")?;
                prefixable_command.dump(target)?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Relax" => {
                let no_expand = bool::undump(lines)?;
                Ok(Self::Relax { no_expand })
            }
            "LeftBrace" => {
                let c = u8::undump(lines)?;
                Ok(Self::LeftBrace(c))
            }
            "RightBrace" => {
                let c = u8::undump(lines)?;
                Ok(Self::RightBrace(c))
            }
            "MathShift" => {
                let c = u8::undump(lines)?;
                Ok(Self::MathShift(c))
            }
            "TabMark" => {
                let c = u8::undump(lines)?;
                Ok(Self::TabMark(c))
            }
            "Span" => Ok(Self::Span),
            "CarRet" => {
                let weak = bool::undump(lines)?;
                Ok(Self::CarRet { weak })
            }
            "MacParam" => {
                let c = u8::undump(lines)?;
                Ok(Self::MacParam(c))
            }
            "Spacer" => Ok(Self::Spacer),
            "Letter" => {
                let c = u8::undump(lines)?;
                Ok(Self::Letter(c))
            }
            "Other" => {
                let c = u8::undump(lines)?;
                Ok(Self::Other(c))
            }
            "LatinUcsChar" => Ok(Self::LatinUcsChar(undump_latin_command(
                lines,
                &[CatCode::Letter, CatCode::OtherChar],
            )?)),
            "LatinUcsLeftBrace" => Ok(Self::LatinUcsLeftBrace(undump_latin_command(
                lines,
                &[CatCode::LeftBrace],
            )?)),
            "LatinUcsRightBrace" => Ok(Self::LatinUcsRightBrace(undump_latin_command(
                lines,
                &[CatCode::RightBrace],
            )?)),
            "LatinUcsMathShift" => Ok(Self::LatinUcsMathShift(undump_latin_command(
                lines,
                &[CatCode::MathShift],
            )?)),
            "LatinUcsTabMark" => Ok(Self::LatinUcsTabMark(undump_latin_command(
                lines,
                &[CatCode::TabMark],
            )?)),
            "LatinUcsMacParam" => Ok(Self::LatinUcsMacParam(undump_latin_command(
                lines,
                &[CatCode::MacParam],
            )?)),
            "LatinUcsSupMark" => Ok(Self::LatinUcsSupMark(undump_latin_command(
                lines,
                &[CatCode::SupMark],
            )?)),
            "LatinUcsSubMark" => Ok(Self::LatinUcsSubMark(undump_latin_command(
                lines,
                &[CatCode::SubMark],
            )?)),
            "CjkChar" => Ok(Self::CjkChar(CjkToken::undump(lines)?)),
            "ParEnd" => Ok(Self::ParEnd),
            "Endv" => Ok(Self::Endv),
            "CharNum" => Ok(Self::CharNum),
            "Math" => {
                let math_command = MathCommand::undump(lines)?;
                Ok(Self::Math(math_command))
            }
            "Kern" => Ok(Self::Kern),
            "ShipOut" => Ok(Self::ShipOut),
            "Leaders" => {
                let leader_kind = LeaderKind::undump(lines)?;
                Ok(Self::Leaders(leader_kind))
            }
            "UpperCase" => Ok(Self::UpperCase),
            "LowerCase" => Ok(Self::LowerCase),
            "Raise" => Ok(Self::Raise),
            "Lower" => Ok(Self::Lower),
            "Mark" => Ok(Self::Mark(MarkClassOperand::undump(lines)?)),
            "MakeBox" => {
                let make_box = MakeBox::undump(lines)?;
                Ok(Self::MakeBox(make_box))
            }
            "MoveLeft" => Ok(Self::MoveLeft),
            "MoveRight" => Ok(Self::MoveRight),
            "Show" => {
                let show_command = ShowCommand::undump(lines)?;
                Ok(Self::Show(show_command))
            }
            "UnHbox" => {
                let copy = bool::undump(lines)?;
                Ok(Self::UnHbox { copy })
            }
            "UnVbox" => {
                let copy = bool::undump(lines)?;
                Ok(Self::UnVbox { copy })
            }
            "VDiscards" => Ok(Self::VDiscards(VerticalDiscardKind::undump(lines)?)),
            "RemoveItem" => {
                let remove_item = RemoveItem::undump(lines)?;
                Ok(Self::RemoveItem(remove_item))
            }
            "Hskip" => {
                let hskip = Hskip::undump(lines)?;
                Ok(Self::Hskip(hskip))
            }
            "Vskip" => {
                let vskip = Vskip::undump(lines)?;
                Ok(Self::Vskip(vskip))
            }
            "Halign" => Ok(Self::Halign),
            "Valign" => Ok(Self::Valign),
            "NoAlign" => Ok(Self::NoAlign),
            "Vrule" => Ok(Self::Vrule),
            "Hrule" => Ok(Self::Hrule),
            "Insert" => Ok(Self::Insert),
            "Vadjust" => Ok(Self::Vadjust),
            "IgnoreSpaces" => Ok(Self::IgnoreSpaces),
            "AfterAssignment" => Ok(Self::AfterAssignment),
            "AfterGroup" => Ok(Self::AfterGroup),
            "Penalty" => Ok(Self::Penalty),
            "StartPar" => {
                let is_indented = bool::undump(lines)?;
                Ok(Self::StartPar { is_indented })
            }
            "ItalCorr" => Ok(Self::ItalCorr),
            "Accent" => Ok(Self::Accent),
            "HyphenBreak" => Ok(Self::HyphenBreak),
            "Discretionary" => Ok(Self::Discretionary),
            "EqNo" => {
                let is_left = bool::undump(lines)?;
                Ok(Self::EqNo { is_left })
            }
            "Message" => {
                let is_err = bool::undump(lines)?;
                Ok(Self::Message { is_err })
            }
            "OpenOut" => Ok(Self::OpenOut),
            "Write" => Ok(Self::Write),
            "CloseOut" => Ok(Self::CloseOut),
            "Special" => Ok(Self::Special),
            "SetLanguage" => Ok(Self::SetLanguage),
            "TextDirection" => Ok(Self::TextDirection(TextDirectionCommand::undump(lines)?)),
            "Immediate" => Ok(Self::Immediate),
            "OpenIn" => Ok(Self::OpenIn),
            "CloseIn" => Ok(Self::CloseIn),
            "BeginGroup" => Ok(Self::BeginGroup),
            "EndGroup" => Ok(Self::EndGroup),
            "Omit" => Ok(Self::Omit),
            "ExSpace" => Ok(Self::ExSpace),
            "NoBoundary" => Ok(Self::NoBoundary),
            "EndCsName" => Ok(Self::EndCsName),
            "CharGiven" => {
                let c = u8::undump(lines)?;
                Ok(Self::CharGiven(c))
            }
            "LastPenalty" => Ok(Self::LastPenalty),
            "LastKern" => Ok(Self::LastKern),
            "LastSkip" => Ok(Self::LastSkip),
            "Badness" => Ok(Self::Badness),
            "FontCharDimension" => Ok(Self::FontCharDimension(FontCharDimension::undump(lines)?)),
            "ParShapeDimension" => Ok(Self::ParShapeDimension(ParShapeDimension::undump(lines)?)),
            "ETeXVersion" => Ok(Self::ETeXVersion),
            "PraTeXVersion" => Ok(Self::PraTeXVersion),
            "PdfShellEscape" => Ok(Self::PdfShellEscape),
            "CurrentGroupLevel" => Ok(Self::CurrentGroupLevel),
            "CurrentGroupType" => Ok(Self::CurrentGroupType),
            "CurrentIfLevel" => Ok(Self::CurrentIfLevel),
            "CurrentIfType" => Ok(Self::CurrentIfType),
            "CurrentIfBranch" => Ok(Self::CurrentIfBranch),
            "LastNodeType" => Ok(Self::LastNodeType),
            "Expr" => Ok(Self::Expr(crate::scan_internal::ValueType::undump(lines)?)),
            "GlueComponent" => Ok(Self::GlueComponent(GlueComponent::undump(lines)?)),
            "GlueConversion" => Ok(Self::GlueConversion(GlueConversion::undump(lines)?)),
            "InputLineNumber" => Ok(Self::InputLineNumber),
            "End" => {
                let dumping = bool::undump(lines)?;
                Ok(Self::End { dumping })
            }
            "Prefixable" => {
                let prefixable_command = PrefixableCommand::undump(lines)?;
                Ok(Self::Prefixable(prefixable_command))
            }
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for VerticalDiscardKind {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(
            target,
            "{}",
            match self {
                Self::Page => "Page",
                Self::Split => "Split",
            }
        )
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "Page" => Ok(Self::Page),
            "Split" => Ok(Self::Split),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for TextDirectionCommand {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(
            target,
            "{}",
            match self {
                Self::BeginLeft => "BeginLeft",
                Self::EndLeft => "EndLeft",
                Self::BeginRight => "BeginRight",
                Self::EndRight => "EndRight",
            }
        )
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "BeginLeft" => Ok(Self::BeginLeft),
            "EndLeft" => Ok(Self::EndLeft),
            "BeginRight" => Ok(Self::BeginRight),
            "EndRight" => Ok(Self::EndRight),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for ExpandableCommand {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Undefined => {
                writeln!(target, "Undefined")?;
            }
            Self::ExpandAfter => {
                writeln!(target, "ExpandAfter")?;
            }
            Self::NoExpand => {
                writeln!(target, "NoExpand")?;
            }
            Self::Input => {
                writeln!(target, "Input")?;
            }
            Self::EndInput => {
                writeln!(target, "EndInput")?;
            }
            Self::ScanTokens => {
                writeln!(target, "ScanTokens")?;
            }
            Self::IfTest(if_test) => {
                writeln!(target, "IfTest")?;
                if_test.dump(target)?;
            }
            Self::FiOrElse(fi_or_else) => {
                writeln!(target, "FiOrElse")?;
                fi_or_else.dump(target)?;
            }
            Self::CsName => {
                writeln!(target, "CsName")?;
            }
            Self::Convert(convert_command) => {
                writeln!(target, "Convert")?;
                convert_command.dump(target)?;
            }
            Self::The => {
                writeln!(target, "The")?;
            }
            Self::TheRawString => {
                writeln!(target, "TheRawString")?;
            }
            Self::Mark(mark_command) => {
                writeln!(target, "Mark")?;
                mark_command.dump(target)?;
            }
            Self::Macro(macro_call) => {
                writeln!(target, "Macro")?;
                macro_call.dump(target)?;
            }
            Self::EndTemplate => {
                writeln!(target, "EndTemplate")?;
            }
            Self::DirectVaak => {
                writeln!(target, "DirectVaak")?;
            }
            Self::Namespace => {
                writeln!(target, "Namespace")?;
            }
            Self::VaakInput => {
                writeln!(target, "VaakInput")?;
            }
            Self::Unless => {
                writeln!(target, "Unless")?;
            }
            Self::Expanded => {
                writeln!(target, "Expanded")?;
            }
            Self::Detokenize => {
                writeln!(target, "Detokenize")?;
            }
            Self::Unexpanded => {
                writeln!(target, "Unexpanded")?;
            }
            &Self::VaakCall(id) => {
                // **本体を書き出す。** 番号は走らせるたびに付け直されるので、
                // 書き出すべきは番号ではなく本体である
                writeln!(target, "VaakCall")?;
                let src = crate::vaak::source_of(id);
                let mut hex = String::with_capacity(src.len() * 2);
                for b in &src {
                    hex.push_str(&format!("{b:02x}"));
                }
                writeln!(target, "{hex}")?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "DirectVaak" => Ok(Self::DirectVaak),
            "Namespace" => Ok(Self::Namespace),
            "VaakInput" => Ok(Self::VaakInput),
            "Unless" => Ok(Self::Unless),
            "Expanded" => Ok(Self::Expanded),
            "Detokenize" => Ok(Self::Detokenize),
            "Unexpanded" => Ok(Self::Unexpanded),
            "VaakCall" => {
                let hex = lines.next().ok_or(FormatError::IncompleteFile)?;
                let mut src = Vec::with_capacity(hex.len() / 2);
                for pair in hex.as_bytes().chunks(2) {
                    let t = std::str::from_utf8(pair).map_err(|_| FormatError::IncompleteFile)?;
                    src.push(u8::from_str_radix(t, 16).map_err(|_| FormatError::IncompleteFile)?);
                }
                Ok(Self::VaakCall(crate::vaak::intern(&src)))
            }
            "Undefined" => Ok(Self::Undefined),
            "ExpandAfter" => Ok(Self::ExpandAfter),
            "NoExpand" => Ok(Self::NoExpand),
            "Input" => Ok(Self::Input),
            "EndInput" => Ok(Self::EndInput),
            "ScanTokens" => Ok(Self::ScanTokens),
            "IfTest" => {
                let if_test = IfTest::undump(lines)?;
                Ok(Self::IfTest(if_test))
            }
            "FiOrElse" => {
                let fi_or_else = FiOrElse::undump(lines)?;
                Ok(Self::FiOrElse(fi_or_else))
            }
            "CsName" => Ok(Self::CsName),
            "Convert" => {
                let convert_command = ConvertCommand::undump(lines)?;
                Ok(Self::Convert(convert_command))
            }
            "The" => Ok(Self::The),
            "TheRawString" => Ok(Self::TheRawString),
            "Mark" => {
                let mark_command = MarkCommand::undump(lines)?;
                Ok(Self::Mark(mark_command))
            }
            "Macro" => {
                let macro_call = MacroCall::undump(lines)?;
                Ok(Self::Macro(macro_call))
            }
            "EndTemplate" => Ok(Self::EndTemplate),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for IfTest {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::IfChar => {
                writeln!(target, "IfChar")?;
            }
            Self::IfCat => {
                writeln!(target, "IfCat")?;
            }
            Self::IfInt => {
                writeln!(target, "IfInt")?;
            }
            Self::IfDim => {
                writeln!(target, "IfDim")?;
            }
            Self::IfOdd => {
                writeln!(target, "IfOdd")?;
            }
            Self::IfVmode => {
                writeln!(target, "IfVmode")?;
            }
            Self::IfHmode => {
                writeln!(target, "IfHmode")?;
            }
            Self::IfMmode => {
                writeln!(target, "IfMmode")?;
            }
            Self::IfInner => {
                writeln!(target, "IfInner")?;
            }
            Self::IfVoid => {
                writeln!(target, "IfVoid")?;
            }
            Self::IfHbox => {
                writeln!(target, "IfHbox")?;
            }
            Self::IfVbox => {
                writeln!(target, "IfVbox")?;
            }
            Self::Ifx => {
                writeln!(target, "Ifx")?;
            }
            Self::IfEof => {
                writeln!(target, "IfEof")?;
            }
            Self::IfTrue => {
                writeln!(target, "IfTrue")?;
            }
            Self::IfFalse => {
                writeln!(target, "IfFalse")?;
            }
            Self::IfCase => {
                writeln!(target, "IfCase")?;
            }
            Self::IfDefined => {
                writeln!(target, "IfDefined")?;
            }
            Self::IfCsName => {
                writeln!(target, "IfCsName")?;
            }
            Self::IfFontChar => {
                writeln!(target, "IfFontChar")?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "IfChar" => Ok(Self::IfChar),
            "IfCat" => Ok(Self::IfCat),
            "IfInt" => Ok(Self::IfInt),
            "IfDim" => Ok(Self::IfDim),
            "IfOdd" => Ok(Self::IfOdd),
            "IfVmode" => Ok(Self::IfVmode),
            "IfHmode" => Ok(Self::IfHmode),
            "IfMmode" => Ok(Self::IfMmode),
            "IfInner" => Ok(Self::IfInner),
            "IfVoid" => Ok(Self::IfVoid),
            "IfHbox" => Ok(Self::IfHbox),
            "IfVbox" => Ok(Self::IfVbox),
            "Ifx" => Ok(Self::Ifx),
            "IfEof" => Ok(Self::IfEof),
            "IfTrue" => Ok(Self::IfTrue),
            "IfFalse" => Ok(Self::IfFalse),
            "IfCase" => Ok(Self::IfCase),
            "IfDefined" => Ok(Self::IfDefined),
            "IfCsName" => Ok(Self::IfCsName),
            "IfFontChar" => Ok(Self::IfFontChar),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for FiOrElse {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Fi => {
                writeln!(target, "Fi")?;
            }
            Self::Or => {
                writeln!(target, "Or")?;
            }
            Self::Else => {
                writeln!(target, "Else")?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Fi" => Ok(Self::Fi),
            "Or" => Ok(Self::Or),
            "Else" => Ok(Self::Else),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for ConvertCommand {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Number => {
                writeln!(target, "Number")?;
            }
            Self::RomanNumeral => {
                writeln!(target, "RomanNumeral")?;
            }
            Self::String => {
                writeln!(target, "String")?;
            }
            Self::Meaning => {
                writeln!(target, "Meaning")?;
            }
            Self::FontName => {
                writeln!(target, "FontName")?;
            }
            Self::JobName => {
                writeln!(target, "JobName")?;
            }
            Self::ETeXRevision => {
                writeln!(target, "ETeXRevision")?;
            }
            Self::PraTeXRevision => {
                writeln!(target, "PraTeXRevision")?;
            }
            Self::PdfFileSize => {
                writeln!(target, "PdfFileSize")?;
            }
            Self::PdfMdFiveSum => {
                writeln!(target, "PdfMdFiveSum")?;
            }
            Self::PdfStrCmp => {
                writeln!(target, "PdfStrCmp")?;
            }
            Self::PdfEscapeHex => {
                writeln!(target, "PdfEscapeHex")?;
            }
            Self::PdfUnescapeHex => {
                writeln!(target, "PdfUnescapeHex")?;
            }
            Self::PdfEscapeString => {
                writeln!(target, "PdfEscapeString")?;
            }
            Self::PdfEscapeName => {
                writeln!(target, "PdfEscapeName")?;
            }
            Self::PdfCreationDate => {
                writeln!(target, "PdfCreationDate")?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Number" => Ok(Self::Number),
            "RomanNumeral" => Ok(Self::RomanNumeral),
            "String" => Ok(Self::String),
            "Meaning" => Ok(Self::Meaning),
            "FontName" => Ok(Self::FontName),
            "JobName" => Ok(Self::JobName),
            "ETeXRevision" => Ok(Self::ETeXRevision),
            "PraTeXRevision" => Ok(Self::PraTeXRevision),
            "PdfFileSize" => Ok(Self::PdfFileSize),
            "PdfMdFiveSum" => Ok(Self::PdfMdFiveSum),
            "PdfStrCmp" => Ok(Self::PdfStrCmp),
            "PdfEscapeHex" => Ok(Self::PdfEscapeHex),
            "PdfUnescapeHex" => Ok(Self::PdfUnescapeHex),
            "PdfEscapeString" => Ok(Self::PdfEscapeString),
            "PdfEscapeName" => Ok(Self::PdfEscapeName),
            "PdfCreationDate" => Ok(Self::PdfCreationDate),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for MarkClassOperand {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Zero => writeln!(target, "Zero")?,
            Self::Scan => writeln!(target, "Scan")?,
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Zero" => Ok(Self::Zero),
            "Scan" => Ok(Self::Scan),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for MarkQuery {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(
            target,
            "{}",
            match self {
                Self::Top => "Top",
                Self::First => "First",
                Self::Bot => "Bot",
                Self::SplitFirst => "SplitFirst",
                Self::SplitBot => "SplitBot",
            }
        )
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "Top" => Ok(Self::Top),
            "First" => Ok(Self::First),
            "Bot" => Ok(Self::Bot),
            "SplitFirst" => Ok(Self::SplitFirst),
            "SplitBot" => Ok(Self::SplitBot),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for MarkCommand {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.query.dump(target)?;
        self.class.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self {
            query: MarkQuery::undump(lines)?,
            class: MarkClassOperand::undump(lines)?,
        })
    }
}

impl Dumpable for MacroCall {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.long.dump(target)?;
        self.outer.dump(target)?;
        self.protected.dump(target)?;
        self.macro_def.dump(target)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let long = bool::undump(lines)?;
        let outer = bool::undump(lines)?;
        let protected = bool::undump(lines)?;
        let macro_def = std::rc::Rc::undump(lines)?;
        Ok(Self {
            long,
            outer,
            protected,
            macro_def,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::Macro;
    use crate::token::CjkCategory;

    #[test]
    fn dump_command() {
        let unexpandable = Command::Unexpandable(UnexpandableCommand::Raise);
        let expandable = Command::Expandable(ExpandableCommand::Undefined);

        let mut file = Vec::new();
        unexpandable.dump(&mut file).unwrap();
        expandable.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        let unexpandable_undumped = Command::undump(&mut lines).unwrap();
        let expandable_undumped = Command::undump(&mut lines).unwrap();
        assert_eq!(unexpandable, unexpandable_undumped);
        assert_eq!(expandable, expandable_undumped);
    }

    #[test]
    fn dump_unexpandable_command() {
        let lower = UnexpandableCommand::Lower;

        let mut file = Vec::new();
        lower.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        let classic_undumped = UnexpandableCommand::undump(&mut lines).unwrap();
        assert_eq!(lower, classic_undumped);
    }

    #[test]
    fn fontchar寸法命令のformatを往復する() {
        for dimension in FontCharDimension::ALL {
            let command = UnexpandableCommand::FontCharDimension(dimension);
            let mut file = Vec::new();
            command.dump(&mut file).unwrap();
            let input = String::from_utf8(file).unwrap();
            assert_eq!(
                UnexpandableCommand::undump(&mut input.lines()).unwrap(),
                command
            );
        }
    }

    #[test]
    fn parshape寸法命令のformatを往復する() {
        for dimension in ParShapeDimension::ALL {
            let command = UnexpandableCommand::ParShapeDimension(dimension);
            let mut file = Vec::new();
            command.dump(&mut file).unwrap();
            let input = String::from_utf8(file).unwrap();
            assert_eq!(
                UnexpandableCommand::undump(&mut input.lines()).unwrap(),
                command
            );
        }
    }

    #[test]
    fn 和文文字命令のformatを往復する() {
        let token = CjkToken::new(0x10_FFFF, CjkCategory::Modifier).unwrap();
        let command = UnexpandableCommand::CjkChar(token);

        let mut file = Vec::new();
        command.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        assert_eq!(
            UnexpandableCommand::undump(&mut input.lines()).unwrap(),
            command
        );
    }

    #[test]
    fn 和文文字命令の壊れたformatを拒否する() {
        for input in [
            "CjkChar\n12354\n15\n",
            "CjkChar\n12354\n21\n",
            "CjkChar\n1114112\n18\n",
        ] {
            assert!(matches!(
                UnexpandableCommand::undump(&mut input.lines()),
                Err(FormatError::ParseError)
            ));
        }

        for input in ["CjkChar\n", "CjkChar\n12354\n"] {
            assert!(matches!(
                UnexpandableCommand::undump(&mut input.lines()),
                Err(FormatError::IncompleteFile)
            ));
        }
    }

    #[test]
    fn unicode欧文命令のvariantとcatcodeが違うformatを拒否する() {
        for input in [
            "LatinUcsChar\n256\nLeftBrace\n",
            "LatinUcsLeftBrace\n256\nRightBrace\n",
            "LatinUcsRightBrace\n256\nLetter\n",
            "LatinUcsMathShift\n256\nTabMark\n",
            "LatinUcsTabMark\n256\nMacParam\n",
            "LatinUcsMacParam\n256\nSupMark\n",
            "LatinUcsSupMark\n256\nSubMark\n",
            "LatinUcsSubMark\n256\nOtherChar\n",
        ] {
            assert!(matches!(
                UnexpandableCommand::undump(&mut input.lines()),
                Err(FormatError::ParseError)
            ));
        }
    }

    #[test]
    fn dump_expandable_command() {
        let undefined = ExpandableCommand::Undefined;
        let expand_after = ExpandableCommand::ExpandAfter;
        let no_expand = ExpandableCommand::NoExpand;
        let input_command = ExpandableCommand::Input;
        let end_input = ExpandableCommand::EndInput;
        let scan_tokens = ExpandableCommand::ScanTokens;
        let if_test = ExpandableCommand::IfTest(IfTest::IfChar);
        let fi_or_else = ExpandableCommand::FiOrElse(FiOrElse::Fi);
        let cs_name = ExpandableCommand::CsName;
        let convert = ExpandableCommand::Convert(ConvertCommand::Number);
        let the = ExpandableCommand::The;
        let mark =
            ExpandableCommand::Mark(MarkCommand::new(MarkQuery::Top, MarkClassOperand::Zero));
        let macro_call = ExpandableCommand::Macro(MacroCall {
            protected: false,
            long: false,
            outer: true,
            macro_def: std::rc::Rc::new(Macro::default()),
        });
        let end_template = ExpandableCommand::EndTemplate;

        let mut file = Vec::new();
        undefined.dump(&mut file).unwrap();
        expand_after.dump(&mut file).unwrap();
        no_expand.dump(&mut file).unwrap();
        input_command.dump(&mut file).unwrap();
        end_input.dump(&mut file).unwrap();
        scan_tokens.dump(&mut file).unwrap();
        if_test.dump(&mut file).unwrap();
        fi_or_else.dump(&mut file).unwrap();
        cs_name.dump(&mut file).unwrap();
        convert.dump(&mut file).unwrap();
        the.dump(&mut file).unwrap();
        mark.dump(&mut file).unwrap();
        macro_call.dump(&mut file).unwrap();
        end_template.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        let undefined_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let expand_after_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let no_expand_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let input_command_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let end_input_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let scan_tokens_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let if_test_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let fi_or_else_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let cs_name_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let convert_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let the_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let mark_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let macro_call_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        let end_template_undumped = ExpandableCommand::undump(&mut lines).unwrap();
        assert_eq!(undefined, undefined_undumped);
        assert_eq!(expand_after, expand_after_undumped);
        assert_eq!(no_expand, no_expand_undumped);
        assert_eq!(input_command, input_command_undumped);
        assert_eq!(end_input, end_input_undumped);
        assert_eq!(scan_tokens, scan_tokens_undumped);
        assert_eq!(if_test, if_test_undumped);
        assert_eq!(fi_or_else, fi_or_else_undumped);
        assert_eq!(cs_name, cs_name_undumped);
        assert_eq!(convert, convert_undumped);
        assert_eq!(the, the_undumped);
        assert_eq!(mark, mark_undumped);
        assert_eq!(macro_call, macro_call_undumped);
        assert_eq!(end_template, end_template_undumped);
    }

    #[test]
    fn dump_if_test() {
        let if_char = IfTest::IfChar;
        let if_cat = IfTest::IfCat;
        let if_int = IfTest::IfInt;
        let if_dim = IfTest::IfDim;
        let if_odd = IfTest::IfOdd;
        let if_vmode = IfTest::IfVmode;
        let if_hmode = IfTest::IfHmode;
        let if_mmode = IfTest::IfMmode;
        let if_inner = IfTest::IfInner;
        let if_void = IfTest::IfVoid;
        let if_hbox = IfTest::IfHbox;
        let if_vbox = IfTest::IfVbox;
        let ifx = IfTest::Ifx;
        let if_eof = IfTest::IfEof;
        let if_true = IfTest::IfTrue;
        let if_false = IfTest::IfFalse;
        let if_case = IfTest::IfCase;

        let mut file = Vec::new();
        if_char.dump(&mut file).unwrap();
        if_cat.dump(&mut file).unwrap();
        if_int.dump(&mut file).unwrap();
        if_dim.dump(&mut file).unwrap();
        if_odd.dump(&mut file).unwrap();
        if_vmode.dump(&mut file).unwrap();
        if_hmode.dump(&mut file).unwrap();
        if_mmode.dump(&mut file).unwrap();
        if_inner.dump(&mut file).unwrap();
        if_void.dump(&mut file).unwrap();
        if_hbox.dump(&mut file).unwrap();
        if_vbox.dump(&mut file).unwrap();
        ifx.dump(&mut file).unwrap();
        if_eof.dump(&mut file).unwrap();
        if_true.dump(&mut file).unwrap();
        if_false.dump(&mut file).unwrap();
        if_case.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        let if_char_undumped = IfTest::undump(&mut lines).unwrap();
        let if_cat_undumped = IfTest::undump(&mut lines).unwrap();
        let if_int_undumped = IfTest::undump(&mut lines).unwrap();
        let if_dim_undumped = IfTest::undump(&mut lines).unwrap();
        let if_odd_undumped = IfTest::undump(&mut lines).unwrap();
        let if_vmode_undumped = IfTest::undump(&mut lines).unwrap();
        let if_hmode_undumped = IfTest::undump(&mut lines).unwrap();
        let if_mmode_undumped = IfTest::undump(&mut lines).unwrap();
        let if_inner_undumped = IfTest::undump(&mut lines).unwrap();
        let if_void_undumped = IfTest::undump(&mut lines).unwrap();
        let if_hbox_undumped = IfTest::undump(&mut lines).unwrap();
        let if_vbox_undumped = IfTest::undump(&mut lines).unwrap();
        let ifx_undumped = IfTest::undump(&mut lines).unwrap();
        let if_eof_undumped = IfTest::undump(&mut lines).unwrap();
        let if_true_undumped = IfTest::undump(&mut lines).unwrap();
        let if_false_undumped = IfTest::undump(&mut lines).unwrap();
        let if_case_undumped = IfTest::undump(&mut lines).unwrap();
        assert_eq!(if_char, if_char_undumped);
        assert_eq!(if_cat, if_cat_undumped);
        assert_eq!(if_int, if_int_undumped);
        assert_eq!(if_dim, if_dim_undumped);
        assert_eq!(if_odd, if_odd_undumped);
        assert_eq!(if_vmode, if_vmode_undumped);
        assert_eq!(if_hmode, if_hmode_undumped);
        assert_eq!(if_mmode, if_mmode_undumped);
        assert_eq!(if_inner, if_inner_undumped);
        assert_eq!(if_void, if_void_undumped);
        assert_eq!(if_hbox, if_hbox_undumped);
        assert_eq!(if_vbox, if_vbox_undumped);
        assert_eq!(ifx, ifx_undumped);
        assert_eq!(if_eof, if_eof_undumped);
        assert_eq!(if_true, if_true_undumped);
        assert_eq!(if_false, if_false_undumped);
        assert_eq!(if_case, if_case_undumped);
    }

    #[test]
    fn dump_fi_or_else() {
        let fi_command = FiOrElse::Fi;
        let or_command = FiOrElse::Or;
        let else_command = FiOrElse::Else;

        let mut file = Vec::new();
        fi_command.dump(&mut file).unwrap();
        or_command.dump(&mut file).unwrap();
        else_command.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        let fi_command_undumped = FiOrElse::undump(&mut lines).unwrap();
        let or_command_undumped = FiOrElse::undump(&mut lines).unwrap();
        let else_command_undumped = FiOrElse::undump(&mut lines).unwrap();
        assert_eq!(fi_command, fi_command_undumped);
        assert_eq!(or_command, or_command_undumped);
        assert_eq!(else_command, else_command_undumped);
    }

    #[test]
    fn dump_convert_command() {
        let number = ConvertCommand::Number;
        let roman_numeral = ConvertCommand::RomanNumeral;
        let string = ConvertCommand::String;
        let meaning = ConvertCommand::Meaning;
        let font_name = ConvertCommand::FontName;
        let job_name = ConvertCommand::JobName;

        let mut file = Vec::new();
        number.dump(&mut file).unwrap();
        roman_numeral.dump(&mut file).unwrap();
        string.dump(&mut file).unwrap();
        meaning.dump(&mut file).unwrap();
        font_name.dump(&mut file).unwrap();
        job_name.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        let number_undumped = ConvertCommand::undump(&mut lines).unwrap();
        let roman_numeral_undumped = ConvertCommand::undump(&mut lines).unwrap();
        let string_undumped = ConvertCommand::undump(&mut lines).unwrap();
        let meaning_undumped = ConvertCommand::undump(&mut lines).unwrap();
        let font_name_undumped = ConvertCommand::undump(&mut lines).unwrap();
        let job_name_undumped = ConvertCommand::undump(&mut lines).unwrap();
        assert_eq!(number, number_undumped);
        assert_eq!(roman_numeral, roman_numeral_undumped);
        assert_eq!(string, string_undumped);
        assert_eq!(meaning, meaning_undumped);
        assert_eq!(font_name, font_name_undumped);
        assert_eq!(job_name, job_name_undumped);
    }

    #[test]
    fn 拡張変換命令を書き出して読める() {
        let commands = [
            ConvertCommand::ETeXRevision,
            ConvertCommand::PraTeXRevision,
            ConvertCommand::PdfFileSize,
            ConvertCommand::PdfMdFiveSum,
            ConvertCommand::PdfStrCmp,
            ConvertCommand::PdfEscapeHex,
            ConvertCommand::PdfUnescapeHex,
            ConvertCommand::PdfEscapeString,
            ConvertCommand::PdfEscapeName,
            ConvertCommand::PdfCreationDate,
        ];
        let mut file = Vec::new();
        for command in &commands {
            command.dump(&mut file).unwrap();
        }
        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        for command in commands {
            assert_eq!(command, ConvertCommand::undump(&mut lines).unwrap());
        }
    }

    #[test]
    fn dump_mark_command() {
        let commands = [
            MarkCommand::new(MarkQuery::Top, MarkClassOperand::Zero),
            MarkCommand::new(MarkQuery::First, MarkClassOperand::Zero),
            MarkCommand::new(MarkQuery::Bot, MarkClassOperand::Zero),
            MarkCommand::new(MarkQuery::SplitFirst, MarkClassOperand::Zero),
            MarkCommand::new(MarkQuery::SplitBot, MarkClassOperand::Zero),
            MarkCommand::new(MarkQuery::Top, MarkClassOperand::Scan),
            MarkCommand::new(MarkQuery::First, MarkClassOperand::Scan),
            MarkCommand::new(MarkQuery::Bot, MarkClassOperand::Scan),
            MarkCommand::new(MarkQuery::SplitFirst, MarkClassOperand::Scan),
            MarkCommand::new(MarkQuery::SplitBot, MarkClassOperand::Scan),
        ];

        let mut file = Vec::new();
        for command in commands {
            command.dump(&mut file).unwrap();
        }

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        for command in commands {
            assert_eq!(command, MarkCommand::undump(&mut lines).unwrap());
        }
    }
}

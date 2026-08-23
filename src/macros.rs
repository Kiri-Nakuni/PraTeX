use crate::eqtb::{Eqtb, MAX_LATIN_UCS_CODE};
use crate::format::{Dumpable, FormatError};
use crate::print::pseudo::PseudoPrinter;
use crate::print::Printer;
use crate::token::{print_uptex_code_point, Token};

use std::io::Write;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Macro {
    pub parameter_text: Vec<ParamToken>,
    pub replacement_text: Vec<MacroToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamToken {
    Normal(Token),
    Match(u32),
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroToken {
    Normal(Token),
    OutParam(u8),
}

impl Macro {
    fn format_state_is_valid(&self) -> bool {
        // Older in-memory/default command entries use an empty parameter
        // vector for an empty macro. Keep that representation round-trippable;
        // expansion treats it exactly like a sole End marker.
        let parameter_prefix = match self.parameter_text.split_last() {
            None => &[][..],
            Some((ParamToken::End, parameter_prefix)) => parameter_prefix,
            Some(_) => return false,
        };
        if parameter_prefix
            .iter()
            .any(|token| *token == ParamToken::End)
        {
            return false;
        }
        let parameter_count = parameter_prefix
            .iter()
            .filter(|token| matches!(token, ParamToken::Match(_)))
            .count();
        parameter_count <= 9
            && self.replacement_text.iter().all(|token| match token {
                MacroToken::Normal(_) => true,
                MacroToken::OutParam(number) => {
                    (1..=parameter_count).contains(&usize::from(*number))
                }
            })
    }
}

/// See 295.
pub fn macro_show(macro_def: &Macro, printer: &mut impl Printer, eqtb: &Eqtb) {
    show_macro_def(macro_def, 10_000_000, printer, eqtb);
}

/// Prints a token list to the selected Printer.
/// See 292.
pub fn show_macro_def(macro_def: &Macro, limit: usize, printer: &mut impl Printer, eqtb: &Eqtb) {
    printer.reset_tally();
    let match_chr = print_macro_parameters(macro_def, limit, printer, eqtb);
    print_replacement_text(&macro_def.replacement_text, match_chr, limit, printer, eqtb);
}

/// Pseudo prints the given macro.
/// See 292.
pub fn show_macro_pseudo(
    macro_def: &Macro,
    next_node: usize,
    pseudo_printer: &mut PseudoPrinter,
    eqtb: &Eqtb,
) {
    pseudo_printer.reset_tally();
    // The arbitrary bound 100_000 comes from 319.
    let limit = 100_000;
    let match_chr = print_macro_parameters(macro_def, limit, pseudo_printer, eqtb);
    let (read_text, unread_text) = &macro_def.replacement_text.split_at(next_node);
    print_replacement_text(read_text, match_chr, limit, pseudo_printer, eqtb);
    pseudo_printer.switch_to_unread_part();
    print_replacement_text(unread_text, match_chr, limit, pseudo_printer, eqtb);
}

/// Prints the parameter list of a macro to the selected Printer and returns the last use match
/// character.
/// See 292.
fn print_macro_parameters(
    macro_def: &Macro,
    limit: usize,
    printer: &mut impl Printer,
    eqtb: &Eqtb,
) -> u32 {
    let mut match_chr = u32::from(b'#');
    let mut n = b'0';
    for &param_token in &macro_def.parameter_text {
        if printer.get_tally() >= limit {
            printer.print_esc_str(b"ETC.");
            return match_chr;
        }
        match param_token {
            ParamToken::Normal(token) => token.display(printer, eqtb),
            ParamToken::Match(c) => {
                match_chr = c;
                print_uptex_code_point(c, printer);
                n += 1;
                printer.print_char(n);
                if n > b'9' {
                    return match_chr;
                }
            }
            ParamToken::End => printer.print_str("->"),
        }
    }
    match_chr
}

/// Prints the replacement text of a macro to the selected Printer.
/// See 292.
fn print_replacement_text(
    replacement_text: &[MacroToken],
    match_chr: u32,
    limit: usize,
    printer: &mut impl Printer,
    eqtb: &Eqtb,
) {
    for &macro_token in replacement_text {
        if printer.get_tally() >= limit {
            printer.print_esc_str(b"ETC.");
            return;
        }
        match macro_token {
            MacroToken::Normal(token) => token.display(printer, eqtb),
            MacroToken::OutParam(number) => {
                print_uptex_code_point(match_chr, printer);
                if number <= 9 {
                    printer.print_char(number + b'0');
                } else {
                    printer.print_char(b'!');
                    return;
                }
            }
        }
    }
}

impl Dumpable for Macro {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.parameter_text.dump(target)?;
        self.replacement_text.dump(target)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let parameter_text = Vec::undump(lines)?;
        let replacement_text = Vec::undump(lines)?;
        let macro_def = Self {
            parameter_text,
            replacement_text,
        };
        macro_def
            .format_state_is_valid()
            .then_some(macro_def)
            .ok_or(FormatError::ParseError)
    }
}

impl Dumpable for ParamToken {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Normal(token) => {
                writeln!(target, "Normal")?;
                token.dump(target)?;
            }
            Self::Match(c) => {
                writeln!(target, "Match")?;
                c.dump(target)?;
            }
            Self::End => writeln!(target, "End")?,
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Normal" => {
                let token = Token::undump(lines)?;
                Ok(Self::Normal(token))
            }
            "Match" => {
                let c = u32::undump(lines)?;
                if c > MAX_LATIN_UCS_CODE {
                    return Err(FormatError::ParseError);
                }
                Ok(Self::Match(c))
            }
            "End" => Ok(Self::End),
            _ => Err(FormatError::ParseError),
        }
    }
}

#[cfg(test)]
mod latin_ucs_tests {
    use super::*;

    #[test]
    fn macro_match文字のformatをunicode欧文上限に限る() {
        assert!(matches!(
            ParamToken::undump(&mut "Match\n11903\n".lines()),
            Ok(ParamToken::Match(c)) if c == MAX_LATIN_UCS_CODE
        ));
        assert!(matches!(
            ParamToken::undump(&mut "Match\n11904\n".lines()),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn macroの壊れた引数構造をformatから読まない() {
        for macro_def in [
            Macro {
                parameter_text: vec![ParamToken::End, ParamToken::Normal(Token::OtherChar(b'x'))],
                replacement_text: Vec::new(),
            },
            Macro {
                parameter_text: vec![ParamToken::Match(u32::from(b'#')); 10]
                    .into_iter()
                    .chain([ParamToken::End])
                    .collect(),
                replacement_text: Vec::new(),
            },
            Macro {
                parameter_text: vec![ParamToken::Match(u32::from(b'#')), ParamToken::End],
                replacement_text: vec![MacroToken::OutParam(0)],
            },
            Macro {
                parameter_text: vec![ParamToken::Match(u32::from(b'#')), ParamToken::End],
                replacement_text: vec![MacroToken::OutParam(2)],
            },
            Macro {
                parameter_text: Vec::new(),
                replacement_text: vec![MacroToken::OutParam(1)],
            },
        ] {
            let mut bytes = Vec::new();
            macro_def.dump(&mut bytes).unwrap();
            let input = String::from_utf8(bytes).unwrap();
            assert!(matches!(
                Macro::undump(&mut input.lines()),
                Err(FormatError::ParseError)
            ));
        }
    }
}

impl Dumpable for MacroToken {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Normal(token) => {
                writeln!(target, "Normal")?;
                token.dump(target)?;
            }
            Self::OutParam(c) => {
                writeln!(target, "OutParam")?;
                c.dump(target)?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Normal" => {
                let token = Token::undump(lines)?;
                Ok(Self::Normal(token))
            }
            "OutParam" => {
                let c = u8::undump(lines)?;
                Ok(Self::OutParam(c))
            }
            _ => Err(FormatError::ParseError),
        }
    }
}

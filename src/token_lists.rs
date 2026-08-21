use crate::command::{Command, ConvertCommand, ExpandableCommand, MacroCall, UnexpandableCommand};
use crate::eqtb::{ControlSequence, Eqtb};
use crate::fonts::scan_font_ident;
use crate::input::expansion::get_x_token;
use crate::input::{Scanner, ScannerStatus};
use crate::integer::{Integer, IntegerExt};
use crate::logger::Logger;
use crate::macros::macro_show;
use crate::print::pseudo::PseudoPrinter;
use crate::print::string::StringPrinter;
use crate::print::Printer;
use crate::scan_internal::{scan_internal_toks, InternalValue};
use crate::token::Token;

use std::os::unix::ffi::OsStrExt;

pub type RcTokenList = std::rc::Rc<Vec<Token>>;

/// See 295.
pub fn token_show(token_list: &[Token], printer: &mut impl Printer, eqtb: &Eqtb) {
    show_token_list(token_list, 10_000_000, printer, eqtb);
}

/// Prints a token list to the selected Printer.
/// See 292.
pub fn show_token_list(list: &[Token], limit: usize, printer: &mut impl Printer, eqtb: &Eqtb) {
    printer.reset_tally();
    for &token in list {
        if printer.get_tally() >= limit {
            printer.print_esc_str(b"ETC.");
            return;
        }
        token.display(printer, eqtb);
    }
}

/// Pseudo prints the given token list.
/// See 292.
pub fn show_token_list_pseudo(
    token_list: &[Token],
    next_node: usize,
    pseudo_printer: &mut PseudoPrinter,
    eqtb: &Eqtb,
) {
    // The arbitrary bound 100_000 comes from 319.
    for (i, &token) in token_list.iter().enumerate() {
        if pseudo_printer.get_tally() >= 100_000 {
            pseudo_printer.print_esc_str(b"ETC.");
            return;
        }
        if i == next_node {
            pseudo_printer.switch_to_unread_part();
        }
        token.display(pseudo_printer, eqtb);
    }
}

/// See 296.
pub fn print_meaning(command: Command, printer: &mut impl Printer, eqtb: &Eqtb) {
    command.display(&eqtb.fonts, printer);

    if let Command::Expandable(ExpandableCommand::Macro(MacroCall { macro_def, .. })) = &command {
        printer.print_char(b':');
        printer.print_ln();
        macro_show(macro_def, printer, eqtb);
    }
    // NOTE: We keep this for now for compatibility.
    if let Command::Expandable(ExpandableCommand::EndTemplate) = command {
        printer.print_char(b':');
        printer.print_ln();
    }
    if let Command::Expandable(ExpandableCommand::Mark(mark_command)) = command {
        printer.print_char(b':');
        printer.print_ln();
        if let Some(token_list) = eqtb.marks.get(mark_command) {
            show_token_list(token_list, 10_000_000, printer, eqtb);
        }
    }
}

/// Takes a string and makes it into a token list.
/// Spaces become Spacer's, everything else becomes an OtherChar.
/// See 464.
pub fn str_toks(s: &[u8]) -> Vec<Token> {
    let mut list = Vec::new();
    for &c in s {
        if c == b' ' {
            list.push(Token::SPACE_TOKEN);
        } else {
            list.push(Token::OtherChar(c));
        }
    }
    list
}

/// See 465.
pub fn the_toks(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) -> Vec<Token> {
    let (unexpandable_command, token) = get_x_token(scanner, eqtb, logger);
    let value = if let Some(internal_command) = unexpandable_command.try_to_internal() {
        scan_internal_toks(internal_command, token, scanner, eqtb, logger)
    } else {
        complain_that_the_cant_do_this(unexpandable_command, scanner, eqtb, logger);
        InternalValue::Int(0)
    };
    let mut string_printer = StringPrinter::new(eqtb.get_current_escape_character());
    match value {
        InternalValue::TokenList(token_list) => {
            return token_list;
        }
        InternalValue::Ident(font_index) => {
            return vec![Token::CSToken {
                cs: ControlSequence::FontId(font_index),
            }];
        }
        InternalValue::MuGlue(glue_spec) => {
            glue_spec.print_spec(Some("mu"), &mut string_printer);
        }
        InternalValue::Glue(glue_spec) => {
            glue_spec.print_spec(Some("pt"), &mut string_printer);
        }
        InternalValue::Dimen(w) => {
            string_printer.print_scaled(w);
            string_printer.print_str("pt");
        }
        InternalValue::Int(w) => {
            string_printer.print_int(w);
        }
    }
    let s = string_printer.into_string();
    str_toks(&s)
}

/// See 467.
pub fn ins_the_toks(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    let toks = the_toks(scanner, eqtb, logger);
    scanner.ins_list(toks, eqtb, logger);
}

/// See 470.
pub fn conv_toks(
    convert_command: ConvertCommand,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    let mut string_printer = StringPrinter::new(eqtb.get_current_escape_character());
    scan_and_print_argument_for_convert_command(
        convert_command,
        &mut string_printer,
        scanner,
        eqtb,
        logger,
    );
    let s = string_printer.into_string();
    scanner.ins_list(str_toks(&s), eqtb, logger);
}

/// See 471. and 472.
fn scan_and_print_argument_for_convert_command(
    convert_command: ConvertCommand,
    string_printer: &mut StringPrinter,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    match convert_command {
        ConvertCommand::Number => {
            let value = Integer::scan_int(scanner, eqtb, logger);
            string_printer.print_int(value);
        }
        ConvertCommand::RomanNumeral => {
            let value = Integer::scan_int(scanner, eqtb, logger);
            string_printer.print_roman_int(value);
        }
        ConvertCommand::String => {
            let save_scanner_status =
                std::mem::replace(&mut scanner.scanner_status, ScannerStatus::Normal);
            let token = scanner.get_token(eqtb, logger);
            scanner.scanner_status = save_scanner_status;
            match token {
                Token::LeftBrace(c)
                | Token::RightBrace(c)
                | Token::MathShift(c)
                | Token::TabMark(c)
                | Token::MacParam(c)
                | Token::SuperMark(c)
                | Token::SubMark(c)
                | Token::Spacer(c)
                | Token::Letter(c)
                | Token::OtherChar(c) => string_printer.print_char(c),
                Token::CSToken { cs } => cs.sprint_cs(eqtb, string_printer),
                Token::Null => {
                    panic!("Should not appear here")
                }
            }
        }
        ConvertCommand::Meaning => {
            let save_scanner_status =
                std::mem::replace(&mut scanner.scanner_status, ScannerStatus::Normal);
            let (command, _) = scanner.get_command_and_token(eqtb, logger);
            scanner.scanner_status = save_scanner_status;
            print_meaning(command, string_printer, eqtb);
        }
        ConvertCommand::FontName => {
            let font_index = scan_font_ident(scanner, eqtb, logger);
            let font = &eqtb.fonts[font_index as usize];
            string_printer.slow_print_str(&font.name);
            if font.size != font.dsize {
                string_printer.print_str(" at ");
                string_printer.print_scaled(font.size);
                string_printer.print_str("pt");
            }
        }
        ConvertCommand::JobName => {
            if logger.job_name.is_none() {
                logger.open_log_file(&scanner.input_stack, eqtb);
            }
            string_printer.slow_print_str(logger.job_name.as_ref().unwrap().as_bytes());
        }
        // ==== pdfTeX 由来。**組版に触らない道具** ====
        //
        // expl3 が engine の見分けに使う。中身は文字列とハッシュだけである
        ConvertCommand::PdfFileSize => {
            let name = scanner.scan_file_name(eqtb, logger);
            let path = std::path::PathBuf::from(String::from_utf8_lossy(&name).into_owned());
            // **無ければ空を返す。** 誤りにしない（pdfTeX と同じ）
            if let Ok(m) = std::fs::metadata(&path) {
                string_printer.print_int(m.len() as i32);
            }
        }
        ConvertCommand::PdfMdFiveSum => {
            let s = scan_general_text_as_string(scanner, eqtb, logger);
            for b in crate::md5::md5(&s) {
                string_printer.slow_print_str(format!("{b:02X}").as_bytes());
            }
        }
        ConvertCommand::PdfEscapeHex => {
            let s = scan_general_text_as_string(scanner, eqtb, logger);
            for b in s {
                string_printer.slow_print_str(format!("{b:02X}").as_bytes());
            }
        }
        ConvertCommand::PdfUnescapeHex => {
            let s = scan_general_text_as_string(scanner, eqtb, logger);
            let hex: Vec<u8> = s.into_iter().filter(|c| c.is_ascii_hexdigit()).collect();
            for pair in hex.chunks(2) {
                let hi = (pair[0] as char).to_digit(16).unwrap() as u8;
                // **半端な桁は零で埋める**（pdfTeX と同じ）
                let lo = pair.get(1).map_or(0, |c| (*c as char).to_digit(16).unwrap() as u8);
                string_printer.print(hi * 16 + lo);
            }
        }
        ConvertCommand::PdfEscapeString => {
            let s = scan_general_text_as_string(scanner, eqtb, logger);
            for b in s {
                match b {
                    b'(' | b')' | b'\\' => {
                        string_printer.print(b'\\');
                        string_printer.print(b);
                    }
                    0x20..=0x7E => string_printer.print(b),
                    _ => string_printer.slow_print_str(format!("\\{b:03o}").as_bytes()),
                }
            }
        }
        ConvertCommand::PdfEscapeName => {
            let s = scan_general_text_as_string(scanner, eqtb, logger);
            for b in s {
                if b.is_ascii_alphanumeric() {
                    string_printer.print(b);
                } else {
                    string_printer.slow_print_str(format!("#{b:02X}").as_bytes());
                }
            }
        }
        ConvertCommand::PdfCreationDate => {
            // **時刻は固定である**（rtex は 1776 年 7 月 4 日正午に止めてある）
            string_printer.slow_print_str(b"D:17760704120000+00'00'");
        }
    }
}

/// `\detokenize{…}` の中身。**展開せずに字句へ直す**（e-TeX）。
pub fn detokenize_toks(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) -> Vec<Token> {
    let toks = nested_scan_toks(scanner, false, eqtb, logger);
    let mut p = StringPrinter::new(eqtb.get_current_escape_character());
    token_show(&toks, &mut p, eqtb);
    str_toks(&p.into_string())
}

/// `\unexpanded{…}` の中身。**そのまま返す**（e-TeX）。
pub fn unexpanded_toks(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) -> Vec<Token> {
    nested_scan_toks(scanner, false, eqtb, logger)
}

/// 走査の**途中で**もう一度走査する。
///
/// **`scan_toks` は `def_ref` を作り直す。** 外側が溜めていたものが消えるので、
/// 控えてから呼び、戻す——`\message{[\detokenize{…}]}` の `[` が消えた原因である。
pub fn nested_scan_toks(
    scanner: &mut Scanner,
    xpand: bool,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> Vec<Token> {
    let saved_def_ref = std::mem::take(&mut scanner.def_ref);
    let saved_status = scanner.scanner_status;
    let saved_index = scanner.warning_index;
    let toks = scanner.scan_toks(ControlSequence::NullCs, xpand, eqtb, logger);
    scanner.def_ref = saved_def_ref;
    scanner.scanner_status = saved_status;
    scanner.warning_index = saved_index;
    toks
}

/// See 428.
fn complain_that_the_cant_do_this(
    unexpandable_command: UnexpandableCommand,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    logger.print_err("You can't use `");
    unexpandable_command.display(&eqtb.fonts, logger);
    logger.print_str("' after ");
    logger.print_esc_str(b"the");
    let help = &["I'm forgetting what you said and using zero instead."];
    logger.error(help, scanner, eqtb);
}

/// `⟨general text⟩` を展開しきって文字列にする。
///
/// `\pdfmdfivesum{…}` のような pdfTeX 由来の命令が使う。
fn scan_general_text_as_string(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> Vec<u8> {
    let toks = scanner.scan_toks(ControlSequence::NullCs, true, eqtb, logger);
    let mut p = StringPrinter::new(eqtb.get_current_escape_character());
    token_show(&toks, &mut p, eqtb);
    p.into_string()
}

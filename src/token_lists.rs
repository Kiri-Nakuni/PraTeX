use crate::command::{
    Command, ConvertCommand, ExpandableCommand, MacroCall, MarkClassOperand, UnexpandableCommand,
};
use crate::eqtb::{ControlSequence, Eqtb, KCatCode};
use crate::fonts::scan_font_ident;
use crate::input::expansion::get_x_token;
use crate::input::{Scanner, ScannerStatus};
use crate::integer::{Integer, IntegerExt};
use crate::logger::Logger;
use crate::macros::macro_show;
use crate::print::Printer;
use crate::print::pseudo::PseudoPrinter;
use crate::print::string::StringPrinter;
use crate::scan_internal::{InternalValue, scan_internal_toks};
use crate::token::{CjkCategory, CjkToken, Token, decode_uptex_input_code_point};

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
        if mark_command.class == MarkClassOperand::Zero {
            printer.print_char(b':');
            printer.print_ln();
            if let Some(token_list) = eqtb.marks.get(mark_command.query, 0) {
                show_token_list(token_list, 10_000_000, printer, eqtb);
            }
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

/// Retokenize bytes produced by a [`Printer`] using the current upTeX
/// Japanese character categories.
///
/// This is deliberately separate from [`str_toks`]: the latter is also the
/// byte-oriented boundary used by Vaak and must keep one token per byte.
pub(crate) fn printed_str_toks(s: &[u8], eqtb: &Eqtb) -> Vec<Token> {
    let mut list = Vec::with_capacity(s.len());
    let mut pos = 0;
    while pos < s.len() {
        let byte = s[pos];
        if byte.is_ascii() {
            list.push(if byte == b' ' {
                Token::SPACE_TOKEN
            } else {
                Token::OtherChar(byte)
            });
            pos += 1;
            continue;
        }

        let Some((code_point, len)) = decode_uptex_input_code_point(&s[pos..]) else {
            // Invalid input recovers one byte at a time, just like `str_toks`.
            list.push(Token::OtherChar(byte));
            pos += 1;
            continue;
        };

        let category = match eqtb.kcat_code(code_point) {
            KCatCode::Kanji => Some(CjkCategory::Kanji),
            KCatCode::Kana => Some(CjkCategory::Kana),
            KCatCode::OtherKChar => Some(CjkCategory::OtherKChar),
            KCatCode::Hangul => Some(CjkCategory::Hangul),
            KCatCode::Modifier => Some(CjkCategory::Modifier),
            // The public engine's printer-to-token path keeps this as one
            // Japanese token even when the current table says `not_cjk`.
            KCatCode::NotCjk => Some(CjkCategory::OtherKChar),
            // TODO(upTeX stage 4c): emit one 16-bit-catcode Unicode European
            // token.  That token type does not exist yet, so retain bytes.
            KCatCode::LatinUcs => None,
        };
        if let Some(category) = category {
            if let Some(token) = CjkToken::new(code_point, category) {
                list.push(Token::CjkChar(token));
            } else {
                list.extend(s[pos..pos + len].iter().copied().map(Token::OtherChar));
            }
        } else {
            list.extend(s[pos..pos + len].iter().copied().map(Token::OtherChar));
        }
        pos += len;
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
    printed_str_toks(&s, eqtb)
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
    scanner.ins_list(printed_str_toks(&s, eqtb), eqtb, logger);
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
                Token::CjkChar(token) => token.print_utf8(string_printer),
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
            string_printer.slow_print_str(logger.job_name.as_ref().unwrap().as_encoded_bytes());
        }
        // ==== pdfTeX 由来。**組版に触らない道具** ====
        //
        // expl3 が engine の見分けに使う。中身は文字列とハッシュだけである
        ConvertCommand::PdfFileSize => {
            // `\pdffilesize` の引数は ⟨general text⟩。展開した結果を名前にする。
            // ファイル名走査を使うと `{name}` の括弧まで名前として読んでしまう。
            let name = scan_general_text_as_string(scanner, eqtb, logger);
            let path = std::path::PathBuf::from(crate::os_string_from_bytes(name));
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
        ConvertCommand::PdfStrCmp => {
            let left = scan_general_text_as_string(scanner, eqtb, logger);
            let right = scan_general_text_as_string(scanner, eqtb, logger);
            let ordering = match left.cmp(&right) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            string_printer.print_int(ordering);
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
                let lo = pair
                    .get(1)
                    .map_or(0, |c| (*c as char).to_digit(16).unwrap() as u8);
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
    printed_str_toks(&p.into_string(), eqtb)
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
/// `\pdfmdfivesum{…}` や `\directvaak{…}` のような展開可能命令が使う。
pub(crate) fn scan_general_text_as_string(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> Vec<u8> {
    // `\message{before\pdfescapehex{...}after}` の内側でも呼ばれる。
    // 外側の `def_ref` を失わない道は `nested_scan_toks` 一箇所に置く。
    let toks = nested_scan_toks(scanner, true, eqtb, logger);
    let mut p = StringPrinter::new(eqtb.get_current_escape_character());
    token_show(&toks, &mut p, eqtb);
    p.into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logger::{InteractionMode, Logger};

    fn cjk(code_point: u32, category: CjkCategory) -> Token {
        Token::CjkChar(CjkToken::new(code_point, category).unwrap())
    }

    fn 入力器を作る() -> (Scanner, Eqtb, Logger) {
        (
            Scanner::new(Vec::new(), 0),
            Eqtb::new(),
            Logger::new(String::new(), InteractionMode::Batch),
        )
    }

    #[test]
    fn 印字列を現在の和文カテゴリーで字句化する() {
        let mut eqtb = Eqtb::new();
        let bytes = "あ".as_bytes();
        assert_eq!(
            printed_str_toks(bytes, &eqtb),
            vec![cjk(0x3042, CjkCategory::Kana)]
        );

        eqtb.kcat_code_define(0x3042, KCatCode::Hangul, true);
        assert_eq!(
            printed_str_toks(bytes, &eqtb),
            vec![cjk(0x3042, CjkCategory::Hangul)]
        );

        eqtb.kcat_code_define(0x3042, KCatCode::NotCjk, true);
        assert_eq!(
            printed_str_toks(bytes, &eqtb),
            vec![cjk(0x3042, CjkCategory::OtherKChar)]
        );
    }

    #[test]
    fn 欧文符号位置と不正列はバイト字句へ戻す() {
        let mut eqtb = Eqtb::new();
        eqtb.kcat_code_define(0x2E00, KCatCode::LatinUcs, true);
        assert_eq!(
            printed_str_toks("⸀".as_bytes(), &eqtb),
            str_toks("⸀".as_bytes())
        );

        let invalid = [b' ', 0xE3, b'A', 0x81];
        assert_eq!(printed_str_toks(&invalid, &eqtb), str_toks(&invalid));
    }

    #[test]
    fn stringは印字後の現在カテゴリーで和文字句を作る() {
        let (mut scanner, mut eqtb, mut logger) = 入力器を作る();
        scanner.ins_list(vec![cjk(0x3042, CjkCategory::Kana)], &eqtb, &mut logger);
        eqtb.kcat_code_define(0x3042, KCatCode::Hangul, true);

        conv_toks(ConvertCommand::String, &mut scanner, &mut eqtb, &mut logger);

        assert_eq!(
            scanner.get_token(&mut eqtb, &mut logger),
            cjk(0x3042, CjkCategory::Hangul)
        );
    }

    #[test]
    fn detokenizeは印字後の現在カテゴリーで和文字句を作る() {
        let (mut scanner, mut eqtb, mut logger) = 入力器を作る();
        scanner.ins_list(
            vec![
                Token::LEFT_BRACE_TOKEN,
                cjk(0x3042, CjkCategory::Kana),
                Token::RIGHT_BRACE_TOKEN,
            ],
            &eqtb,
            &mut logger,
        );
        eqtb.kcat_code_define(0x3042, KCatCode::Modifier, true);

        assert_eq!(
            detokenize_toks(&mut scanner, &mut eqtb, &mut logger),
            vec![cjk(0x3042, CjkCategory::Modifier)]
        );
    }
}

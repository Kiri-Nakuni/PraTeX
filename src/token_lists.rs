use crate::command::{
    Command, ConvertCommand, ExpandableCommand, MacroCall, MarkClassOperand, UnexpandableCommand,
};
use crate::eqtb::{ControlSequence, Eqtb, KCatCode};
use crate::file_search::FileKind;
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
use crate::token::{
    CjkCategory, CjkToken, LatinUcsToken, Token, decode_uptex_input_code_point,
};

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
    printed_str_toks_with_latin_catcode(s, eqtb, Some(crate::eqtb::CatCode::OtherChar))
}

fn printed_str_toks_with_latin_catcode(
    s: &[u8],
    eqtb: &Eqtb,
    latin_catcode: Option<crate::eqtb::CatCode>,
) -> Vec<Token> {
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

        // Non-canonical UTF-8 spellings of ASCII still have canonical byte
        // identity. This boundary can receive arbitrary byte strings, so it
        // must make the same low-code decision as the line lexer before
        // consulting the Japanese character table.
        if code_point <= 0x7F {
            let byte = code_point as u8;
            list.push(if byte == b' ' {
                Token::SPACE_TOKEN
            } else {
                Token::OtherChar(byte)
            });
            pos += len;
            continue;
        }

        let category = match eqtb.kcat_code(code_point) {
            KCatCode::Kanji => Some(CjkCategory::Kanji),
            KCatCode::Kana => Some(CjkCategory::Kana),
            KCatCode::OtherKChar => Some(CjkCategory::OtherKChar),
            KCatCode::Hangul => Some(CjkCategory::Hangul),
            KCatCode::Modifier => Some(CjkCategory::Modifier),
            // The public engine's printer-to-token path keeps this as one
            // Japanese token even when the current table says `not_cjk`.
            KCatCode::NotCjk => Some(CjkCategory::OtherKChar),
            KCatCode::LatinUcs => {
                let cat_code =
                    latin_catcode.unwrap_or_else(|| eqtb.latin_ucs_cat_code(code_point));
                if cat_code == crate::eqtb::CatCode::InvalidChar {
                    list.push(Token::CjkChar(
                        CjkToken::new(code_point, CjkCategory::OtherKChar)
                            .expect("latin_ucs range is a valid CJK token code point"),
                    ));
                } else if let Some(token) = LatinUcsToken::new(code_point, cat_code) {
                    list.push(Token::LatinUcsChar(token));
                } else {
                    list.extend(s[pos..pos + len].iter().copied().map(Token::OtherChar));
                }
                pos += len;
                continue;
            }
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
    let value = scan_the_value(scanner, eqtb, logger);
    match value_to_the_toks(value, eqtb) {
        Ok(tokens) => tokens,
        Err(_) => {
            // LF/CRLFのsnapshot境界規則が決まるまで、公開の`\the`経路へ仮の意味を
            // 入れない。値は変更せず、この展開だけを空にする。
            logger.print_err("Raw string snapshot tokenization is not specified yet");
            let help = &[
                "Use \\therawstring for exact byte tokens.",
                "I'm expanding this \\the operation to nothing.",
            ];
            logger.error(help, scanner, eqtb);
            Vec::new()
        }
    }
}

pub(crate) fn scan_the_value(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    let (unexpandable_command, token) = get_x_token(scanner, eqtb, logger);
    if let Some(internal_command) = unexpandable_command.try_to_internal() {
        scan_internal_toks(internal_command, token, scanner, eqtb, logger)
    } else {
        complain_that_the_cant_do_this(unexpandable_command, scanner, eqtb, logger);
        InternalValue::Int(0)
    }
}

pub(crate) fn value_to_the_toks(
    value: InternalValue,
    eqtb: &Eqtb,
) -> Result<Vec<Token>, crate::eqtb::RcRawString> {
    let mut string_printer = StringPrinter::new(eqtb.get_current_escape_character());
    match value {
        InternalValue::TokenList(token_list) => {
            return Ok(token_list);
        }
        InternalValue::Ident(font_index) => {
            return Ok(vec![Token::CSToken {
                cs: ControlSequence::FontId(font_index),
            }]);
        }
        InternalValue::RawString(value) => {
            return Err(value);
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
    Ok(printed_str_toks(&s, eqtb))
}

/// `\therawstring`専用。catcode・comment・改行を一切解釈しない。
pub(crate) fn the_raw_string_toks(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> Vec<Token> {
    match scan_the_value(scanner, eqtb, logger) {
        InternalValue::RawString(value) => crate::eqtb::raw_bytes_as_other_tokens(&value),
        _ => {
            logger.print_err("A raw string register was expected after \\therawstring");
            let help = &["I'm expanding this operation to nothing."];
            logger.error(help, scanner, eqtb);
            Vec::new()
        }
    }
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
                Token::LatinUcsChar(token) => token.print_utf8(string_printer),
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
            let logical_path = std::path::PathBuf::from(crate::os_string_from_bytes(name));
            let path = scanner
                .resolve_file_path(FileKind::Tex, &logical_path)
                .ok()
                .flatten();
            // **無ければ空を返す。** 誤りにしない（pdfTeX と同じ）
            if let Some(path) = path {
                if let Ok(metadata) = std::fs::metadata(path) {
                    string_printer.print_int(metadata.len() as i32);
                }
            }
        }
        ConvertCommand::PdfMdFiveSum => {
            // pdfTeX's public syntax is
            // `\pdfmdfivesum [file] <general text>`.  Keep the historical
            // string form byte-for-byte, and only enter file lookup after the
            // optional expanded keyword has been consumed.
            let file_mode = scanner.scan_keyword(b"file", eqtb, logger);
            let text = scan_general_text_as_string(scanner, eqtb, logger);
            if file_mode {
                // web2c accepts a matching outer quote pair around a file name.
                // Strip it before the direct-path/resolver boundary as well, so
                // quoted UTF-8 names do not require an external kpsewhich call.
                let name = strip_matching_file_name_quotes(&text);
                let logical_path =
                    std::path::PathBuf::from(crate::os_string_from_bytes(name.to_vec()));
                let digest = scanner
                    .resolve_file_path(FileKind::Tex, &logical_path)
                    .ok()
                    .flatten()
                    .and_then(|path| std::fs::File::open(path).ok())
                    .and_then(|mut file| crate::md5::md5_reader(&mut file).ok());
                // pdfTeX expands to an empty token list when lookup or reading
                // fails.  In particular, this is not a TeX error.
                if let Some(digest) = digest {
                    print_md_five_digest(digest, string_printer);
                }
            } else {
                print_md_five_sum(&text, string_printer);
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
            let creation_date = eqtb.run_date_time().pdf_creation_date();
            string_printer.slow_print_str(creation_date.as_bytes());
        }
        ConvertCommand::PraTeXRevision => {
            string_printer.slow_print_str(crate::version::PRATEX_REVISION.as_bytes());
        }
    }
}

fn strip_matching_file_name_quotes(name: &[u8]) -> &[u8] {
    name.strip_prefix(b"\"")
        .and_then(|name| name.strip_suffix(b"\""))
        .unwrap_or(name)
}

fn print_md_five_sum(input: &[u8], string_printer: &mut StringPrinter) {
    print_md_five_digest(crate::md5::md5(input), string_printer);
}

fn print_md_five_digest(digest: [u8; 16], string_printer: &mut StringPrinter) {
    for byte in digest {
        string_printer.slow_print_str(format!("{byte:02X}").as_bytes());
    }
}

/// `\detokenize{…}` の中身。**展開せずに字句へ直す**（e-TeX）。
pub fn detokenize_toks(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) -> Vec<Token> {
    let toks = nested_scan_toks(scanner, false, eqtb, logger);
    let mut p = StringPrinter::new(eqtb.get_current_escape_character());
    token_show(&toks, &mut p, eqtb);
    printed_str_toks_with_latin_catcode(
        &p.into_string(),
        eqtb,
        Some(crate::eqtb::CatCode::OtherChar),
    )
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
    use crate::eqtb::RawStringVariable;
    use crate::logger::{InteractionMode, Logger};
    use std::rc::Rc;

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
    fn therawstringは登録値を分類せずother_byteへする() {
        let (_, mut eqtb, mut logger) = 入力器を作る();
        eqtb.put_primitives_into_hash_table();
        eqtb.fix_date_and_time(crate::runtime_clock::RunDateTime::capture().unwrap());
        let bytes = vec![b' ', 0, b'\n', b'\\', b'%', 0xE3, 0x81, 0x82, 0xFF];
        eqtb.raw_string_define(
            RawStringVariable::new(7),
            Rc::new(bytes.clone()),
            true,
        )
        .unwrap();
        let mut scanner = Scanner::new(b"\\rawstring7 ".to_vec(), 0);

        assert_eq!(
            the_raw_string_toks(&mut scanner, &mut eqtb, &mut logger),
            bytes.into_iter().map(Token::OtherChar).collect::<Vec<_>>()
        );
    }

    #[test]
    fn pdfmdfivesum_fileは両側が揃った引用符だけを除く() {
        assert_eq!(
            strip_matching_file_name_quotes(b"\"paired.tex\""),
            b"paired.tex"
        );
        assert_eq!(
            strip_matching_file_name_quotes(b"\"left.tex"),
            b"\"left.tex"
        );
        assert_eq!(
            strip_matching_file_name_quotes(b"right.tex\""),
            b"right.tex\""
        );
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
    fn unicode欧文符号位置は一文字になり不正列だけバイトへ戻す() {
        let mut eqtb = Eqtb::new();
        eqtb.kcat_code_define(0x2E00, KCatCode::LatinUcs, true);
        eqtb.latin_ucs_cat_code_define(0x2E00, crate::eqtb::CatCode::Letter, true);
        assert_eq!(
            printed_str_toks("⸀".as_bytes(), &eqtb),
            vec![Token::LatinUcsChar(
                LatinUcsToken::new(0x2E00, crate::eqtb::CatCode::OtherChar).unwrap()
            )]
        );

        let invalid = [b' ', 0xE3, b'A', 0x81];
        assert_eq!(printed_str_toks(&invalid, &eqtb), str_toks(&invalid));
    }

    #[test]
    fn 印字列の非正規utf8_asciiは一つのbyte_tokenに正規化する() {
        let eqtb = Eqtb::new();
        assert_eq!(
            printed_str_toks(&[0xE0, 0x81, 0x81, 0xE0, 0x80, 0xA0], &eqtb),
            vec![Token::OtherChar(b'A'), Token::SPACE_TOKEN]
        );
    }

    #[test]
    fn stringのunicode欧文は現在値によらずcatcode十二になる() {
        let (mut scanner, mut eqtb, mut logger) = 入力器を作る();
        let token = LatinUcsToken::new(0x0100, crate::eqtb::CatCode::LeftBrace).unwrap();
        scanner.ins_list(vec![Token::LatinUcsChar(token)], &eqtb, &mut logger);
        eqtb.kcat_code_define(0x0100, KCatCode::LatinUcs, true);
        eqtb.latin_ucs_cat_code_define(0x0100, crate::eqtb::CatCode::LeftBrace, true);

        conv_toks(ConvertCommand::String, &mut scanner, &mut eqtb, &mut logger);

        assert_eq!(
            scanner.get_token(&mut eqtb, &mut logger),
            Token::LatinUcsChar(
                LatinUcsToken::new(0x0100, crate::eqtb::CatCode::OtherChar).unwrap()
            )
        );
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


    #[test]
    fn detokenizeしたunicode欧文は現在値によらずcatcode十二になる() {
        let (mut scanner, mut eqtb, mut logger) = 入力器を作る();
        eqtb.kcat_code_define(0x00DF, KCatCode::LatinUcs, true);
        eqtb.latin_ucs_cat_code_define(0x00DF, crate::eqtb::CatCode::Letter, true);
        scanner.ins_list(
            vec![
                Token::LEFT_BRACE_TOKEN,
                Token::LatinUcsChar(
                    LatinUcsToken::new(0x00DF, crate::eqtb::CatCode::Letter).unwrap(),
                ),
                Token::RIGHT_BRACE_TOKEN,
            ],
            &eqtb,
            &mut logger,
        );

        assert_eq!(
            detokenize_toks(&mut scanner, &mut eqtb, &mut logger),
            vec![Token::LatinUcsChar(
                LatinUcsToken::new(0x00DF, crate::eqtb::CatCode::OtherChar).unwrap()
            )]
        );
    }
}

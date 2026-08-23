use super::conditional::{conditional, terminate_current_conditional_and_skip_to_fi};
use super::macro_expand::macro_expand;
use super::pseudo_file::PseudoFilePrinter;
use super::{Scanner, TokenSourceType};
use crate::command::{
    Command, ExpandableCommand, MarkClassOperand, MarkCommand, UnexpandableCommand,
};
use crate::eqtb::{
    ControlSequence, ControlSequenceId, ControlSequenceNameUnit, Eqtb, IntegerVariable,
    NamespaceId,
};
use crate::error::overflow;
use crate::logger::Logger;
use crate::print::Printer;
use crate::token::Token;
use crate::token_lists::{conv_toks, ins_the_toks, nested_scan_toks, token_show};

/// `\csname` が集める名前。通常byteだけなら既存hashへそのまま流し、
/// 最初のUnicode文字を見た時だけtyped名へ昇格する。
pub(super) enum ManufacturedCsName {
    Bytes(Vec<u8>),
    Wide(Vec<ControlSequenceNameUnit>),
}

impl ManufacturedCsName {
    pub(super) fn new() -> Self {
        Self::Bytes(Vec::new())
    }

    pub(super) fn push_byte(&mut self, byte: u8) {
        match self {
            Self::Bytes(bytes) => bytes.push(byte),
            Self::Wide(units) => units.push(ControlSequenceNameUnit::Byte(byte)),
        }
    }

    pub(super) fn push_unicode(&mut self, code_point: u32) {
        match self {
            Self::Bytes(bytes) => {
                let mut units = Vec::with_capacity(bytes.len() + 1);
                units.extend(bytes.drain(..).map(ControlSequenceNameUnit::Byte));
                units.push(ControlSequenceNameUnit::Unicode(code_point));
                *self = Self::Wide(units);
            }
            Self::Wide(units) => units.push(ControlSequenceNameUnit::Unicode(code_point)),
        }
    }

    pub(super) fn lookup(&self, eqtb: &Eqtb) -> Option<ControlSequence> {
        match self {
            Self::Bytes(bytes) => eqtb.lookup(bytes),
            Self::Wide(units) => eqtb.lookup_wide(units),
        }
    }

    fn lookup_or_create(
        &self,
        namespace: Option<NamespaceId>,
        eqtb: &mut Eqtb,
    ) -> Result<ControlSequence, ()> {
        match (namespace, self) {
            (None, Self::Bytes(bytes)) => eqtb.lookup_or_create(bytes),
            (None, Self::Wide(units)) => eqtb.lookup_or_create_wide(units),
            (Some(namespace), Self::Bytes(bytes)) => {
                eqtb.lookup_or_create_ns(Some(namespace), bytes, None)
            }
            (Some(namespace), Self::Wide(units)) => {
                eqtb.lookup_or_create_ns_wide(Some(namespace), units)
            }
        }
    }
}

/// Expands the current command.
/// NOTE: Expects that the command code is larger than MAX_COMMAND
/// and less than DONT_EXPAND.
/// See 366. and 367.
pub fn expand(
    expandable_command: ExpandableCommand,
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    if eqtb.integer(IntegerVariable::TracingCommands) > 1 {
        if let ExpandableCommand::Macro(_) | ExpandableCommand::EndTemplate = expandable_command {
            // These commands are not traced.
        } else {
            show_expandable(&expandable_command, scanner, eqtb, logger);
        }
    }
    match expandable_command {
        ExpandableCommand::Undefined => complain_about_undefined_macro(scanner, eqtb, logger),
        ExpandableCommand::Macro(macro_call) => {
            macro_expand(macro_call, token, scanner, eqtb, logger)
        }
        ExpandableCommand::ExpandAfter => expand_token_after_next_token(scanner, eqtb, logger),
        ExpandableCommand::NoExpand => scanner.suppress_expansion_of_next_token(eqtb, logger),
        ExpandableCommand::IfTest(if_test) => conditional(if_test, scanner, eqtb, logger),
        ExpandableCommand::FiOrElse(fi_or_else) => {
            terminate_current_conditional_and_skip_to_fi(fi_or_else, token, scanner, eqtb, logger)
        }
        ExpandableCommand::CsName => {
            manufacture_control_sequence_name(None, scanner, eqtb, logger)
        }
        ExpandableCommand::Namespace => scan_namespace(scanner, eqtb, logger),
        ExpandableCommand::DirectVaak => {
            crate::vaak::direct_vaak(token, scanner, eqtb, logger)
        }
        ExpandableCommand::VaakInput => crate::vaak::vaak_input(scanner, eqtb, logger),
        ExpandableCommand::Unless => negate_next_conditional(scanner, eqtb, logger),
        // **中身を展開せずに、字句へ直す**（e-TeX）。`\string` を全部に掛けたのと同じ
        ExpandableCommand::Detokenize => {
            let toks = crate::token_lists::detokenize_toks(scanner, eqtb, logger);
            scanner.ins_list(toks, eqtb, logger);
        }
        // **展開する走査の中でも展開しない**（e-TeX）。そのまま置き直す
        ExpandableCommand::Unexpanded => {
            let toks = crate::token_lists::unexpanded_toks(scanner, eqtb, logger);
            scanner.ins_list(toks, eqtb, logger);
        }
        ExpandableCommand::Expanded => {
            // **展開しきってから、その場に置き直す。**
            // `\edef` の走査と同じものを使う——意味を二箇所に書かない。
            // **走査の途中で呼ばれうる**ので、外側の溜めを控える
            let toks = crate::token_lists::nested_scan_toks(scanner, true, eqtb, logger);
            scanner.ins_list(toks, eqtb, logger);
        }
        ExpandableCommand::VaakCall(id) => {
            crate::vaak::vaak_call(id, scanner, eqtb, logger)
        }
        ExpandableCommand::Convert(convert_command) => {
            conv_toks(convert_command, scanner, eqtb, logger)
        }
        ExpandableCommand::The => ins_the_toks(scanner, eqtb, logger),
        ExpandableCommand::Mark(mark_command) => {
            insert_appropriate_mark_text_into_scanner(mark_command, scanner, eqtb, logger)
        }
        ExpandableCommand::EndTemplate => {
            insert_token_containing_frozen_endv(scanner, eqtb, logger)
        }
        ExpandableCommand::Input => initiate_input_from_file(token, scanner, eqtb, logger),
        ExpandableCommand::EndInput => scanner.end_input_from_current_file(),
        ExpandableCommand::ScanTokens => scan_tokens_as_pseudo_file(scanner, eqtb, logger),
    }
}

/// e-TeX `\scantokens`。実fileや一時fileへ逃がさず、同じ行字句器へ型付きbufferを積む。
fn scan_tokens_as_pseudo_file(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    let trace_opened = eqtb.integer(IntegerVariable::TracingScanTokens) > 0;
    let newline_char = eqtb.get_current_newline_character();
    let escape_char = eqtb.get_current_escape_character();

    // `scan_toks` は外側の `def_ref` を作り直すため、必ずnested境界を通す。
    let tokens = nested_scan_toks(scanner, false, eqtb, logger);
    let mut printer = PseudoFilePrinter::new(newline_char, escape_char);
    token_show(&tokens, &mut printer, eqtb);
    let text = match printer.finish() {
        Ok(text) => text,
        Err(error) => overflow(
            error.resource,
            error.limit,
            &scanner.input_stack,
            eqtb,
            logger,
        ),
    };
    scanner
        .input_stack
        .input_from_pseudo_file(text, trace_opened, eqtb, logger);
}

/// Expand the next token after expanding the one following it.
/// See 368.
fn expand_token_after_next_token(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    // Get the first token and save it.
    let first_token = scanner.get_token(eqtb, logger);
    // Get the second token.
    let (second_command, second_token) = scanner.get_command_and_token(eqtb, logger);
    // Expand the second token if appropriate.
    match second_command {
        Command::Expandable(expandable_command) => {
            expand(expandable_command, second_token, scanner, eqtb, logger)
        }
        Command::Unexpandable(_) => scanner.back_input(second_token, eqtb, logger),
    }
    // Push the first, unexpanded token in front again.
    scanner.back_input(first_token, eqtb, logger);
}

/// See 370.
fn complain_about_undefined_macro(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    logger.print_err("Undefined control sequence");
    let help = &[
        "The control sequence at the end of the top line",
        "of your error message was never \\def'ed. If you have",
        "misspelled it (e.g., `\\hobx'), type `I' and the correct",
        "spelling (e.g., `I\\hbox'). Otherwise just continue,",
        "and I'll forget about whatever was undefined.",
    ];
    logger.error(help, scanner, eqtb)
}

/// `\namespace 名前\csname … \endcsname`。
///
/// # なぜ `get_x_token` が使えないか
///
/// **名前空間の名前の終わりを知らせるのが `\csname` 自身だから**である。
/// `get_x_token` は制御が戻る前にそれを展開してしまう。
///
/// だから `get_next` で回し、`\csname` を見つけたらそこで止めて、
/// **`\csname` の登録処理を自分で呼ぶ。**
///
/// # 入れ子は禁じない
///
/// ```tex
/// \namespace \namespace hoge\csname fuga\endcsname \csname bar\endcsname
/// ```
///
/// **二つの `\namespace` は同じ `\csname` を奪い合わない。**
/// 内側は最初の一組を消費して `*hoge\fuga` を作り、それが展開されて
/// 外側の名前空間名の文字になる。外側は自分の `\csname` を別に持つ——
/// **括弧のように入れ子になるだけである。**
///
/// 禁じれば非対称が生まれる。`\namespace \ns\csname bar\endcsname` は
/// `\ns` が global のマクロなら通るのに、名前空間つきのマクロだと通らない。
/// **名前空間名は展開で作られた文字列にすぎない。**
///
/// # なぜ `\endcsname` を終端にしなかったか
///
/// **`\csname` が global に作ってしまう**からである。
/// 登録も `\relax` 化も `\endcsname` に達した一箇所で起きるので、
/// そこへ名前空間を渡す以外に道が無い。
fn scan_namespace(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    let mut name = Vec::new();
    loop {
        let (command, token) = scanner.get_next(false, eqtb, logger);
        match command {
            Command::Expandable(ExpandableCommand::CsName) => {
                // **ここで登録処理へ入る。** 名前空間を持たせて
                manufacture_control_sequence_name(Some(&name), scanner, eqtb, logger);
                return;
            }
            // **文字は名前に取り込む。** 展開可能なものは展開して続ける
            Command::Unexpandable(_) => match token {
                Token::LeftBrace(c)
                | Token::RightBrace(c)
                | Token::MathShift(c)
                | Token::TabMark(c)
                | Token::MacParam(c)
                | Token::SuperMark(c)
                | Token::SubMark(c)
                | Token::Spacer(c)
                | Token::Letter(c)
                | Token::OtherChar(c) => name.push(c),
                Token::LatinUcsChar(c) => c.push_utf8(&mut name),
                Token::CjkChar(c) => c.push_utf8(&mut name),
                _ => {
                    complain_about_missing_csname(token, scanner, eqtb, logger);
                    return;
                }
            },
            Command::Expandable(ExpandableCommand::Macro(macro_call)) => {
                macro_expand(macro_call, token, scanner, eqtb, logger)
            }
            Command::Expandable(expandable_command) => {
                expand(expandable_command, token, scanner, eqtb, logger)
            }
        }
    }
}

fn complain_about_missing_csname(
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    logger.print_err("Missing ");
    logger.print_esc_str(b"csname");
    logger.print_str(" inserted");
    let help = &[
        "A namespace prefix must be followed by \\csname.",
        "The control sequence marked <to be read again> should not appear there.",
    ];
    scanner.back_input(token, eqtb, logger);
    logger.error(help, scanner, eqtb);
}

/// Reads a control sequence name enclosed by \csname and \endcsname
/// in and replaces it by the corresponding control sequence token.
/// See 372.
fn manufacture_control_sequence_name(
    ns: Option<&[u8]>,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    // Collect the token in `name`.
    let mut cs_name = ManufacturedCsName::new();
    // Consume all char tokens after expansion.
    let (finishing_command, finishing_token) = loop {
        let (command, token) = get_x_token(scanner, eqtb, logger);
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
            | Token::OtherChar(c) => {
                cs_name.push_byte(c);
            }
            Token::CjkChar(c) => cs_name.push_unicode(c.code_point()),
            Token::LatinUcsChar(c) => cs_name.push_unicode(c.code_point()),
            token @ Token::CSToken { .. } => break (command, token),
            Token::Null => {
                panic!("Should not appear here")
            }
        }
    };
    // Complain if the ending control sequence is not endcsname.
    if let UnexpandableCommand::EndCsName = finishing_command {
        // Do nothing.
    } else {
        complain_about_missing_endcsname(finishing_token, scanner, eqtb, logger);
    }
    // Look up the name and return the corresponding control sequence.
    //
    // **名前空間があればそちらへ登録する。** 登録はここ一箇所で起きるので、
    // `\namespace` はここへ名前を渡せば足りる
    let namespace = match ns {
        None => None,
        // **空の名前空間名は global そのものである**（仕様どおり）
        Some(n) if n.is_empty() => None,
        Some(n) => Some(eqtb.control_sequences.intern_namespace(n)),
    };
    let created = cs_name.lookup_or_create(namespace, eqtb);
    let Ok(cs) = created else {
        overflow(
            "hash size",
            ControlSequenceId::MAX as usize,
            &scanner.input_stack,
            eqtb,
            logger,
        );
    };

    // An undefined csname is set to be equivalent to \relax
    if let Command::Expandable(ExpandableCommand::Undefined) = eqtb.control_sequences.get(cs) {
        let command = Command::Unexpandable(UnexpandableCommand::Relax { no_expand: false });
        eqtb.cs_define(cs, command, false);
    }
    let token = Token::CSToken { cs };
    scanner.back_input(token, eqtb, logger);
}

/// See 373.
fn complain_about_missing_endcsname(
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    logger.print_err("Missing ");
    logger.print_esc_str(b"endcsname");
    logger.print_str(" inserted");
    let help = &[
        "The control sequence marked <to be read again> should",
        "not appear between \\csname and \\endcsname.",
    ];
    scanner.back_error(token, help, eqtb, logger)
}

/// See 375.
fn insert_token_containing_frozen_endv(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    let token = Token::CSToken {
        cs: ControlSequence::FrozenEndv,
    };
    scanner.back_input(token, eqtb, logger)
}

/// See 378.
fn initiate_input_from_file(
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    // Don't allow an \input command while scanning a file name.
    if scanner.name_in_progress {
        scanner.insert_relax(token, eqtb, logger)
    } else {
        scanner.start_input(eqtb, logger)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtectedMacroExpansion {
    Expand,
    Preserve,
}

/// 展開可能tokenを進め、指定した境界で止まる。
///
/// 通常走査とalignmentの特殊な先読みが別々にmacro展開を実装すると、
/// `\protected`を片方だけで見落とす。macro、`\endtemplate`、その他の展開命令を
/// 進める判断はここだけに置き、呼出側は保護macroを停止点にするかだけを選ぶ。
fn get_x_command_and_token(
    protected_macro_expansion: ProtectedMacroExpansion,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> (Command, Token) {
    loop {
        let (command, token) = scanner.get_next(false, eqtb, logger);
        match command {
            Command::Unexpandable(_) => return (command, token),
            Command::Expandable(ExpandableCommand::EndTemplate) => {
                let command = UnexpandableCommand::Endv;
                let token = Token::CSToken {
                    cs: ControlSequence::FrozenEndv,
                };
                return (Command::Unexpandable(command), token);
            }
            Command::Expandable(ExpandableCommand::Macro(macro_call))
                if protected_macro_expansion == ProtectedMacroExpansion::Preserve
                    && macro_call.protected =>
            {
                return (
                    Command::Expandable(ExpandableCommand::Macro(macro_call)),
                    token,
                );
            }
            Command::Expandable(ExpandableCommand::Macro(macro_call)) => {
                macro_expand(macro_call, token, scanner, eqtb, logger)
            }
            Command::Expandable(expandable_command) => {
                expand(expandable_command, token, scanner, eqtb, logger);
            }
        }
    }
}

/// See 380.
pub fn get_x_token(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> (UnexpandableCommand, Token) {
    let (command, token) =
        get_x_command_and_token(ProtectedMacroExpansion::Expand, scanner, eqtb, logger);
    let Command::Unexpandable(unexpandable_command) = command else {
        unreachable!("normal expansion cannot stop at an expandable command")
    };
    (unexpandable_command, token)
}

/// e-TeX alignmentの`\noalign` / `\omit`先読み。
///
/// 空白と通常macroは展開して進むが、保護macroは通常の行・欄入力として返す。
pub(crate) fn get_next_non_blank_x_token_for_alignment(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> (Command, Token) {
    loop {
        let (command, token) =
            get_x_command_and_token(ProtectedMacroExpansion::Preserve, scanner, eqtb, logger);
        if !matches!(command, Command::Unexpandable(UnexpandableCommand::Spacer)) {
            return (command, token);
        }
    }
}

/// Keeps expanding as long as possible and returns the first unexpandable token.
/// See 381.
pub fn x_token(
    mut command: Command,
    mut token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> (UnexpandableCommand, Token) {
    loop {
        match command {
            Command::Unexpandable(unexpandable_command) => return (unexpandable_command, token),
            Command::Expandable(expandable_command) => {
                expand(expandable_command, token, scanner, eqtb, logger);
                (command, token) = scanner.get_next(false, eqtb, logger);
            }
        }
    }
}

/// See 386.
fn insert_appropriate_mark_text_into_scanner(
    mark_command: MarkCommand,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    let class = match mark_command.class {
        MarkClassOperand::Zero => 0,
        MarkClassOperand::Scan => scanner.scan_mark_class_index(eqtb, logger),
    };
    let mark = eqtb.marks.get(mark_command.query, class);
    if let Some(token_list) = mark {
        scanner.input_stack.begin_token_list(
            token_list.clone(),
            TokenSourceType::MarkText,
            eqtb,
            logger,
        );
    }
}

/// Prints the given command code and character together with the mode.
/// See 299.
fn show_expandable(
    expandable_command: &ExpandableCommand,
    scanner: &Scanner,
    eqtb: &Eqtb,
    logger: &mut Logger,
) {
    logger.begin_diagnostic(eqtb.tracing_online());
    logger.print_nl_str("{");
    if scanner.scanning_write_tokens {
        if logger.shown_mode.is_some() {
            logger.print_str("no mode: ");
            logger.shown_mode = None;
        }
    } else if Some(eqtb.mode()) != logger.shown_mode {
        eqtb.mode().display(logger);
        logger.print_str(": ");
        logger.shown_mode = Some(eqtb.mode());
    }
    expandable_command.display(logger);
    logger.print_char(b'}');
    logger.end_diagnostic(false);
}

/// `\unless\if…` — **次の条件を反転する。**
///
/// 取れるのは真偽を出す条件だけである。`\ifcase` は腕が複数あるので取れない。
fn negate_next_conditional(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    let (command, token) = scanner.get_next(false, eqtb, logger);
    match command {
        Command::Expandable(ExpandableCommand::IfTest(if_test))
            if !matches!(if_test, crate::command::IfTest::IfCase) =>
        {
            crate::input::conditional::conditional_negated(if_test, scanner, eqtb, logger)
        }
        _ => {
            logger.print_err("You can't use `");
            logger.print_esc_str(b"unless");
            logger.print_str("' here");
            let help = &["Continue, and I'll forget that it ever happened."];
            scanner.back_input(token, eqtb, logger);
            logger.error(help, scanner, eqtb);
        }
    }
}

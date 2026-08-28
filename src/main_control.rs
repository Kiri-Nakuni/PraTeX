use crate::alignment::AlignState;
use crate::box_building::{
    align_error, append_discretionary, append_italic_correction, begin_adjust, begin_insert,
    cs_error, delete_last, end_graf, get_hskip, get_mu_glue, get_vskip, handle_right_brace,
    head_for_vmode, hyphen_discretionary, indent_in_hmode, indent_in_mmode, make_accent, make_mark,
    make_penalty, new_graf, no_align_error, normal_paragraph, off_save, omit_error, scan_kern,
    scan_mu_kern, unpackage, BoxContext,
};
use crate::command::{
    prefixed_command, Command, MathCommand, PrefixableCommand, TextDirectionCommand,
    UnexpandableCommand,
};
use crate::dimension::scan_normal_dimen;
use crate::eqtb::save_stack::{GroupType, SubformulaType};
use crate::eqtb::{DimensionVariable, Eqtb, IntegerVariable, SkipVariable, TokenListVariable};
use crate::hyphenation::Hyphenator;
use crate::horizontal_mode::{HorizontalMode, HorizontalModeType};
use crate::input::expansion::get_x_token;
use crate::input::token_source::TokenSourceType;
use crate::input::Scanner;
use crate::integer::{Integer, IntegerExt};
use crate::japanese_fonts::{load_japanese_font_info, JapaneseFontError, JapaneseFontIndex};
use crate::logger::Logger;
use crate::main_loop::{main_loop, WordScanner};
use crate::math::{
    after_math, append_choices, complain_that_user_should_have_said_mathaccent, init_math, math_ac,
    math_fraction, math_left, math_limit_switch, math_middle, math_radical, math_right,
    scan_math_char, scan_subformula_enclosed_in_braces, set_math_char, start_eq_no, subscript,
    superscript, treat_cur_chr_as_active_character,
};
use crate::mode_independent::{
    close_read_file, do_special, issue_message, open_read_file, set_language, shift_to_lower_case,
    shift_to_upper_case, show_whatever,
};
use crate::nodes::noads::{Noad, NormalNoad, OverNoad, StyleNode, UnderNoad};
use crate::nodes::{
    DimensionOrder, GlueNode, GlueSpec, GlueType, HigherOrderDimension, KernNode, ListNode,
    MathNode, MathNodeKind, Node, PenaltyNode, RuleNode, TextDirection, WideCharNode,
};
use crate::output::Output;
use crate::packaging::scan_spec;
use crate::page_breaking::PageBuilder;
use crate::print::Printer;
use crate::scaled::{xn_over_d, UNITY};
use crate::scan_boxes::{begin_box, scan_box, scan_leader_box};
use crate::scan_internal::ValueType;
use crate::semantic_nest::{Mode, RichMode, SemanticState};
use crate::token::{CjkToken, LatinUcsToken, Token};
use crate::vertical_mode::{VerticalMode, IGNORE_DEPTH};
use crate::write_streams::{do_close_out, do_open_out, do_write, implement_immediate};

use std::path::Path;
use std::rc::Rc;

const DEFAULT_HORIZONTAL_JFM: &str = "upjisr-h";

/// Returns true when dumping, else false.
/// See 1030.
pub fn main_control(
    align_state: &mut AlignState,
    hyphenator: &mut Hyphenator,
    page_builder: &mut PageBuilder,
    output: &mut Output,
    nest: &mut SemanticState,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> bool {
    let mut word_scanner = WordScanner::new();

    if let Some(every_job) = eqtb.token_lists.get(TokenListVariable::EveryJob) {
        scanner.input_stack.begin_token_list(
            every_job.clone(),
            TokenSourceType::EveryJobText,
            eqtb,
            logger,
        );
    }
    let (mut unexpandable_command, mut token) = get_x_token(scanner, eqtb, logger);
    loop {
        if check_interrupt_and_show_trace(unexpandable_command, token, scanner, eqtb, logger) {
            (unexpandable_command, token) = get_x_token(scanner, eqtb, logger);
            continue;
        }
        match nest.mode_mut() {
            RichMode::Horizontal(hmode) => match unexpandable_command {
                UnexpandableCommand::Relax { .. } => {
                    hmode.break_jfm_pair_continuity(eqtb);
                }
                UnexpandableCommand::LeftBrace(_) | UnexpandableCommand::LatinUcsLeftBrace(_) => {
                    hmode.break_jfm_pair_continuity(eqtb);
                    eqtb.new_save_level(GroupType::Simple, &scanner.input_stack, logger)
                }
                UnexpandableCommand::RightBrace(_) | UnexpandableCommand::LatinUcsRightBrace(_) => {
                    hmode.break_jfm_pair_continuity(eqtb);
                    handle_right_brace(
                        token,
                        align_state,
                        hyphenator,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    )
                }
                UnexpandableCommand::MathShift(_) | UnexpandableCommand::LatinUcsMathShift(_) => {
                    init_math(
                        hmode.subtype,
                        hyphenator,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    )
                }
                UnexpandableCommand::ParEnd => {
                    if scanner.align_state < 0 {
                        off_save(unexpandable_command, token, scanner, eqtb, logger);
                    }
                    end_graf(hyphenator, nest, scanner, eqtb, logger);
                    // If the end_graf took us back to a unrestricted vertical mode.
                    if eqtb.mode() == Mode::Vertical {
                        page_builder.build_page(output, nest, scanner, eqtb, logger);
                    }
                }
                UnexpandableCommand::Endv => align_state.do_endv(
                    token,
                    hyphenator,
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::Spacer => {
                    if hmode.space_factor == 1000 {
                        hmode.append_node(normal_space(eqtb), eqtb);
                    } else {
                        hmode.append_node(space_node(hmode.space_factor, eqtb), eqtb);
                    }
                }
                UnexpandableCommand::Letter(c)
                | UnexpandableCommand::Other(c)
                | UnexpandableCommand::CharGiven(c) => {
                    (unexpandable_command, token) =
                        main_loop(hmode, c, false, &mut word_scanner, scanner, eqtb, logger);
                    continue;
                }
                UnexpandableCommand::CjkChar(c) => {
                    append_cjk_character(hmode, c, scanner, eqtb, logger)
                }
                UnexpandableCommand::LatinUcsChar(c) => {
                    report_latin_ucs_typesetting_unavailable(c, scanner, eqtb, logger)
                }
                UnexpandableCommand::CharNum => {
                    let c = scanner.scan_char_num(eqtb, logger);
                    (unexpandable_command, token) =
                        main_loop(hmode, c, false, &mut word_scanner, scanner, eqtb, logger);
                    continue;
                }
                UnexpandableCommand::NoBoundary => {
                    (unexpandable_command, token) = get_x_token(scanner, eqtb, logger);
                    match unexpandable_command {
                        UnexpandableCommand::Letter(c)
                        | UnexpandableCommand::Other(c)
                        | UnexpandableCommand::CharGiven(c) => {
                            (unexpandable_command, token) =
                                main_loop(hmode, c, true, &mut word_scanner, scanner, eqtb, logger);
                            continue;
                        }
                        UnexpandableCommand::CharNum => {
                            let c = scanner.scan_char_num(eqtb, logger);
                            (unexpandable_command, token) =
                                main_loop(hmode, c, true, &mut word_scanner, scanner, eqtb, logger);
                            continue;
                        }
                        UnexpandableCommand::CjkChar(c) => {
                            append_cjk_character(hmode, c, scanner, eqtb, logger);
                            continue;
                        }
                        UnexpandableCommand::LatinUcsChar(c) => {
                            report_latin_ucs_typesetting_unavailable(c, scanner, eqtb, logger);
                            continue;
                        }
                        _ => continue,
                    }
                }
                UnexpandableCommand::SetInhibitGlue { inhibit } => {
                    hmode.set_inhibit_glue(inhibit)
                }
                UnexpandableCommand::ExSpace => hmode.append_node(normal_space(eqtb), eqtb),
                UnexpandableCommand::Raise => {
                    let dimen = scan_normal_dimen(scanner, eqtb, logger);
                    scan_box(
                        BoxContext::Shift(-dimen),
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::Lower => {
                    let dimen = scan_normal_dimen(scanner, eqtb, logger);
                    scan_box(
                        BoxContext::Shift(dimen),
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::Show(show_command) => {
                    hmode.break_jfm_pair_continuity(eqtb);
                    show_whatever(
                        show_command,
                        token,
                        nest,
                        page_builder,
                        scanner,
                        eqtb,
                        logger,
                    )
                }
                UnexpandableCommand::MakeBox(make_box) => begin_box(
                    make_box,
                    BoxContext::Shift(0),
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::UnHbox { copy } => {
                    unpackage(copy, nest, scanner, eqtb, logger)
                }
                UnexpandableCommand::RemoveItem(remove_item) => {
                    hmode.break_jfm_pair_continuity(eqtb);
                    delete_last(remove_item, nest, scanner, eqtb, logger)
                }
                UnexpandableCommand::Hskip(hskip) => {
                    let glue_node = get_hskip(hskip, scanner, eqtb, logger);
                    hmode.append_node(Node::Glue(glue_node), eqtb);
                }
                UnexpandableCommand::Valign => align_state.init_align(
                    token,
                    hyphenator,
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::Vrule => {
                    let rule_node = RuleNode::scan_vrule_spec(scanner, eqtb, logger);
                    hmode.append_node(Node::Rule(rule_node), eqtb);
                    hmode.set_space_factor(1000, eqtb);
                }
                UnexpandableCommand::Insert => begin_insert(nest, scanner, eqtb, logger),
                UnexpandableCommand::Vadjust => begin_adjust(nest, scanner, eqtb, logger),
                UnexpandableCommand::IgnoreSpaces => {
                    (unexpandable_command, token) =
                        scanner.get_next_non_blank_non_call_token(eqtb, logger);
                    continue;
                }
                UnexpandableCommand::AfterAssignment => {
                    token = scanner.get_token(eqtb, logger);
                    scanner.after_token = Some(token);
                }
                UnexpandableCommand::AfterGroup => {
                    token = scanner.get_token(eqtb, logger);
                    eqtb.save_for_after(token);
                }
                UnexpandableCommand::Penalty => {
                    hmode.append_node(make_penalty(scanner, eqtb, logger), eqtb)
                }
                UnexpandableCommand::StartPar { is_indented } => {
                    if is_indented {
                        indent_in_hmode(hmode, eqtb);
                    }
                }
                UnexpandableCommand::ItalCorr => append_italic_correction(hmode, eqtb),
                UnexpandableCommand::Accent => make_accent(
                    hyphenator,
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::HyphenBreak => {
                    hmode.append_node(hyphen_discretionary(eqtb, logger), eqtb)
                }
                UnexpandableCommand::Discretionary => {
                    append_discretionary(nest, scanner, eqtb, logger)
                }
                UnexpandableCommand::Message { is_err } => {
                    hmode.break_jfm_pair_continuity(eqtb);
                    issue_message(is_err, token, scanner, eqtb, logger)
                }
                UnexpandableCommand::OpenOut => do_open_out(nest, scanner, eqtb, logger),
                UnexpandableCommand::Write => do_write(token, nest, scanner, eqtb, logger),
                UnexpandableCommand::CloseOut => do_close_out(nest, scanner, eqtb, logger),
                UnexpandableCommand::Special => do_special(token, nest, scanner, eqtb, logger),
                UnexpandableCommand::SetLanguage => set_language(nest, scanner, eqtb, logger),
                UnexpandableCommand::TextDirection(direction) => {
                    append_text_direction(direction, hmode, scanner, eqtb, logger)
                }
                UnexpandableCommand::Immediate => {
                    implement_immediate(output, scanner, eqtb, logger)
                }
                UnexpandableCommand::OpenIn => open_read_file(scanner, eqtb, logger),
                UnexpandableCommand::CloseIn => close_read_file(scanner, eqtb, logger),
                UnexpandableCommand::BeginGroup => {
                    hmode.break_jfm_pair_continuity(eqtb);
                    eqtb.new_save_level(GroupType::SemiSimple, &scanner.input_stack, logger)
                }
                UnexpandableCommand::EndGroup => {
                    hmode.break_jfm_pair_continuity(eqtb);
                    if let GroupType::SemiSimple = eqtb.cur_group.typ {
                        eqtb.unsave(scanner, logger);
                    } else {
                        off_save(unexpandable_command, token, scanner, eqtb, logger);
                    }
                }
                UnexpandableCommand::Math(_)
                | UnexpandableCommand::LatinUcsSupMark(_)
                | UnexpandableCommand::LatinUcsSubMark(_) => {
                    insert_dollar_sign(token, scanner, eqtb, logger)
                }
                UnexpandableCommand::Kern => {
                    hmode.append_node(scan_kern(scanner, eqtb, logger), eqtb)
                }
                UnexpandableCommand::ShipOut => {
                    scan_box(
                        BoxContext::Shipout,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::Leaders(leader_kind) => {
                    scan_leader_box(
                        leader_kind,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::UpperCase => shift_to_upper_case(token, scanner, eqtb, logger),
                UnexpandableCommand::LowerCase => shift_to_lower_case(token, scanner, eqtb, logger),
                UnexpandableCommand::Mark(class) => {
                    hmode.append_node(make_mark(token, class, scanner, eqtb, logger), eqtb)
                }
                UnexpandableCommand::Vskip(_)
                | UnexpandableCommand::Halign
                | UnexpandableCommand::Hrule
                | UnexpandableCommand::UnVbox { .. }
                | UnexpandableCommand::VDiscards(_)
                | UnexpandableCommand::End { .. } => {
                    head_for_vmode(unexpandable_command, token, scanner, eqtb, logger)
                }
                UnexpandableCommand::Omit => omit_error(scanner, eqtb, logger),
                UnexpandableCommand::EndCsName => cs_error(scanner, eqtb, logger),
                UnexpandableCommand::NoAlign => no_align_error(scanner, eqtb, logger),
                UnexpandableCommand::TabMark(_)
                | UnexpandableCommand::LatinUcsTabMark(_)
                | UnexpandableCommand::Span
                | UnexpandableCommand::CarRet { .. } => {
                    align_error(unexpandable_command, token, scanner, eqtb, logger)
                }
                UnexpandableCommand::LastPenalty
                | UnexpandableCommand::LastKern
                | UnexpandableCommand::LastSkip
                | UnexpandableCommand::Badness
                | UnexpandableCommand::FontCharDimension(_)
                | UnexpandableCommand::ParShapeDimension(_)
                | UnexpandableCommand::ETeXVersion
                | UnexpandableCommand::PraTeXVersion
                | UnexpandableCommand::PdfShellEscape
                | UnexpandableCommand::CurrentGroupLevel
                | UnexpandableCommand::CurrentGroupType
                | UnexpandableCommand::CurrentIfLevel
                | UnexpandableCommand::CurrentIfType
                | UnexpandableCommand::CurrentIfBranch
                | UnexpandableCommand::LastNodeType
                | UnexpandableCommand::Expr(_)
                | UnexpandableCommand::GlueComponent(_)
                | UnexpandableCommand::GlueConversion(_)
                | UnexpandableCommand::InputLineNumber
                | UnexpandableCommand::EqNo { .. }
                | UnexpandableCommand::MoveLeft
                | UnexpandableCommand::MoveRight
                | UnexpandableCommand::MacParam(_)
                | UnexpandableCommand::LatinUcsMacParam(_) => {
                    report_illegal_case(unexpandable_command, scanner, eqtb, logger)
                }
                // Cases that don't depend on mode
                UnexpandableCommand::Prefixable(prefixable_command) => {
                    // 公式e-upTeXで`）\count0=0（`がclass 0の二境界になることを
                    // black-box確認した範囲だけを早期挿入へ接続する。
                    if matches!(
                        prefixable_command,
                        PrefixableCommand::Register(ValueType::Int)
                    ) {
                        hmode.break_jfm_pair_continuity(eqtb);
                    }
                    prefixed_command(
                        prefixable_command,
                        token,
                        hyphenator,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    )
                }
            },
            RichMode::Vertical(vmode) => match unexpandable_command {
                UnexpandableCommand::Relax { .. }
                | UnexpandableCommand::SetInhibitGlue { .. } => {
                    // Do nothing.
                }
                UnexpandableCommand::LeftBrace(_) | UnexpandableCommand::LatinUcsLeftBrace(_) => {
                    eqtb.new_save_level(GroupType::Simple, &scanner.input_stack, logger)
                }
                UnexpandableCommand::RightBrace(_) | UnexpandableCommand::LatinUcsRightBrace(_) => {
                    handle_right_brace(
                        token,
                        align_state,
                        hyphenator,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    )
                }
                UnexpandableCommand::Spacer => {
                    // Do nothing.
                }
                UnexpandableCommand::ParEnd => {
                    normal_paragraph(eqtb, logger);
                    // If the current mode is unrestricted.
                    if eqtb.mode() == Mode::Vertical {
                        page_builder.build_page(output, nest, scanner, eqtb, logger);
                    }
                }
                UnexpandableCommand::Endv => align_state.do_endv(
                    token,
                    hyphenator,
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::Message { is_err } => {
                    issue_message(is_err, token, scanner, eqtb, logger)
                }
                UnexpandableCommand::OpenOut => do_open_out(nest, scanner, eqtb, logger),
                UnexpandableCommand::Write => do_write(token, nest, scanner, eqtb, logger),
                UnexpandableCommand::CloseOut => do_close_out(nest, scanner, eqtb, logger),
                UnexpandableCommand::Special => do_special(token, nest, scanner, eqtb, logger),
                UnexpandableCommand::SetLanguage => set_language(nest, scanner, eqtb, logger),
                UnexpandableCommand::Immediate => {
                    implement_immediate(output, scanner, eqtb, logger)
                }
                UnexpandableCommand::MoveLeft => {
                    let dimen = scan_normal_dimen(scanner, eqtb, logger);
                    scan_box(
                        BoxContext::Shift(-dimen),
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::MoveRight => {
                    let dimen = scan_normal_dimen(scanner, eqtb, logger);
                    scan_box(
                        BoxContext::Shift(dimen),
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::OpenIn => open_read_file(scanner, eqtb, logger),
                UnexpandableCommand::CloseIn => close_read_file(scanner, eqtb, logger),
                UnexpandableCommand::BeginGroup => {
                    eqtb.new_save_level(GroupType::SemiSimple, &scanner.input_stack, logger)
                }
                UnexpandableCommand::EndGroup => {
                    if let GroupType::SemiSimple = eqtb.cur_group.typ {
                        eqtb.unsave(scanner, logger);
                    } else {
                        off_save(unexpandable_command, token, scanner, eqtb, logger);
                    }
                }
                UnexpandableCommand::MakeBox(make_box) => begin_box(
                    make_box,
                    BoxContext::Shift(0),
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::UnVbox { copy } => {
                    unpackage(copy, nest, scanner, eqtb, logger)
                }
                UnexpandableCommand::VDiscards(kind) => {
                    for node in page_builder.take_vdiscards(kind) {
                        vmode.append_node(node, eqtb);
                    }
                }
                UnexpandableCommand::RemoveItem(remove_item) => {
                    delete_last(remove_item, nest, scanner, eqtb, logger)
                }
                UnexpandableCommand::CjkChar(c) => {
                    match ensure_horizontal_japanese_font(scanner, eqtb) {
                        Ok(_) => {
                            // 横組fontを選んだ和文は、欧文文字と同じく外部vertical modeから
                            // paragraphを開始し、horizontal main loopでWideCharへする。
                            scanner.back_input(token, eqtb, logger);
                            new_graf(
                                true,
                                hyphenator,
                                page_builder,
                                output,
                                nest,
                                scanner,
                                eqtb,
                                logger,
                            );
                        }
                        Err(error) => report_cjk_typesetting_unavailable(
                            c,
                            Some(error),
                            scanner,
                            eqtb,
                            logger,
                        ),
                    }
                }
                UnexpandableCommand::LatinUcsChar(c) => {
                    report_latin_ucs_typesetting_unavailable(c, scanner, eqtb, logger)
                }
                UnexpandableCommand::Letter(_)
                | UnexpandableCommand::Other(_)
                | UnexpandableCommand::CharGiven(_)
                | UnexpandableCommand::CharNum
                | UnexpandableCommand::MathShift(_)
                | UnexpandableCommand::LatinUcsMathShift(_)
                | UnexpandableCommand::ExSpace
                | UnexpandableCommand::NoBoundary
                | UnexpandableCommand::HyphenBreak
                | UnexpandableCommand::Discretionary
                | UnexpandableCommand::Accent
                | UnexpandableCommand::Vrule
                | UnexpandableCommand::Valign
                | UnexpandableCommand::Hskip(_)
                | UnexpandableCommand::UnHbox { .. } => {
                    scanner.back_input(token, eqtb, logger);
                    new_graf(
                        true,
                        hyphenator,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::Vskip(vskip) => {
                    let glue_node = get_vskip(vskip, scanner, eqtb, logger);
                    vmode.append_node(Node::Glue(glue_node), eqtb);
                }
                UnexpandableCommand::Halign => align_state.init_align(
                    token,
                    hyphenator,
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::Hrule => {
                    let rule_node = RuleNode::scan_hrule_spec(scanner, eqtb, logger);
                    vmode.append_node(Node::Rule(rule_node), eqtb);
                    vmode.set_prev_depth(IGNORE_DEPTH, eqtb);
                }
                UnexpandableCommand::Insert => begin_insert(nest, scanner, eqtb, logger),
                UnexpandableCommand::Mark(class) => {
                    vmode.append_node(make_mark(token, class, scanner, eqtb, logger), eqtb)
                }
                UnexpandableCommand::Show(show_command) => show_whatever(
                    show_command,
                    token,
                    nest,
                    page_builder,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::IgnoreSpaces => {
                    (unexpandable_command, token) =
                        scanner.get_next_non_blank_non_call_token(eqtb, logger);
                    continue;
                }
                UnexpandableCommand::AfterAssignment => {
                    token = scanner.get_token(eqtb, logger);
                    scanner.after_token = Some(token);
                }
                UnexpandableCommand::AfterGroup => {
                    token = scanner.get_token(eqtb, logger);
                    eqtb.save_for_after(token);
                }
                UnexpandableCommand::Penalty => {
                    vmode.append_node(make_penalty(scanner, eqtb, logger), eqtb);
                    if !vmode.internal {
                        page_builder.build_page(output, nest, scanner, eqtb, logger);
                    }
                }
                UnexpandableCommand::StartPar { is_indented } => new_graf(
                    is_indented,
                    hyphenator,
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::Math(_)
                | UnexpandableCommand::LatinUcsSupMark(_)
                | UnexpandableCommand::LatinUcsSubMark(_) => {
                    insert_dollar_sign(token, scanner, eqtb, logger)
                }
                UnexpandableCommand::Kern => {
                    vmode.append_node(scan_kern(scanner, eqtb, logger), eqtb)
                }
                UnexpandableCommand::ShipOut => {
                    scan_box(
                        BoxContext::Shipout,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::Leaders(leader_kind) => {
                    scan_leader_box(
                        leader_kind,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::UpperCase => shift_to_upper_case(token, scanner, eqtb, logger),
                UnexpandableCommand::LowerCase => shift_to_lower_case(token, scanner, eqtb, logger),
                UnexpandableCommand::Omit => omit_error(scanner, eqtb, logger),
                UnexpandableCommand::EndCsName => cs_error(scanner, eqtb, logger),
                UnexpandableCommand::NoAlign => no_align_error(scanner, eqtb, logger),
                UnexpandableCommand::End { dumping } => {
                    if its_all_over(
                        unexpandable_command,
                        token,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    ) {
                        return dumping;
                    }
                }
                UnexpandableCommand::TabMark(_)
                | UnexpandableCommand::LatinUcsTabMark(_)
                | UnexpandableCommand::Span
                | UnexpandableCommand::CarRet { .. } => {
                    align_error(unexpandable_command, token, scanner, eqtb, logger)
                }
                UnexpandableCommand::LastPenalty
                | UnexpandableCommand::LastKern
                | UnexpandableCommand::LastSkip
                | UnexpandableCommand::Badness
                | UnexpandableCommand::FontCharDimension(_)
                | UnexpandableCommand::ParShapeDimension(_)
                | UnexpandableCommand::ETeXVersion
                | UnexpandableCommand::PraTeXVersion
                | UnexpandableCommand::PdfShellEscape
                | UnexpandableCommand::CurrentGroupLevel
                | UnexpandableCommand::CurrentGroupType
                | UnexpandableCommand::CurrentIfLevel
                | UnexpandableCommand::CurrentIfType
                | UnexpandableCommand::CurrentIfBranch
                | UnexpandableCommand::LastNodeType
                | UnexpandableCommand::Expr(_)
                | UnexpandableCommand::GlueComponent(_)
                | UnexpandableCommand::GlueConversion(_)
                | UnexpandableCommand::InputLineNumber
                | UnexpandableCommand::TextDirection(_)
                | UnexpandableCommand::EqNo { .. }
                | UnexpandableCommand::ItalCorr
                | UnexpandableCommand::Vadjust
                | UnexpandableCommand::Raise
                | UnexpandableCommand::Lower
                | UnexpandableCommand::MacParam(_)
                | UnexpandableCommand::LatinUcsMacParam(_) => {
                    report_illegal_case(unexpandable_command, scanner, eqtb, logger)
                }
                // Cases that don't depend on mode
                UnexpandableCommand::Prefixable(prefixable_command) => prefixed_command(
                    prefixable_command,
                    token,
                    hyphenator,
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
            },
            RichMode::Math(mmode) => match unexpandable_command {
                UnexpandableCommand::Spacer
                | UnexpandableCommand::Relax { .. }
                | UnexpandableCommand::SetInhibitGlue { .. } => {
                    // Do nothing.
                }
                UnexpandableCommand::LeftBrace(_) | UnexpandableCommand::LatinUcsLeftBrace(_) => {
                    mmode.append_node(Node::Noad(Box::new(Noad::Normal(NormalNoad::new()))), eqtb);
                    scanner.back_input(token, eqtb, logger);
                    scan_subformula_enclosed_in_braces(
                        SubformulaType::Nucleus,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::RightBrace(_) | UnexpandableCommand::LatinUcsRightBrace(_) => {
                    handle_right_brace(
                        token,
                        align_state,
                        hyphenator,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    )
                }
                UnexpandableCommand::MathShift(_) | UnexpandableCommand::LatinUcsMathShift(_) => {
                    if let GroupType::MathShift { kind } = eqtb.cur_group.typ {
                        after_math(
                            kind,
                            hyphenator,
                            page_builder,
                            output,
                            nest,
                            scanner,
                            eqtb,
                            logger,
                        );
                    } else {
                        off_save(unexpandable_command, token, scanner, eqtb, logger);
                    }
                }
                UnexpandableCommand::Letter(c)
                | UnexpandableCommand::Other(c)
                | UnexpandableCommand::CharGiven(c) => {
                    let code = eqtb.math_code(c as usize);
                    if code >= 0o100000 {
                        treat_cur_chr_as_active_character(c, scanner, eqtb, logger);
                    } else {
                        set_math_char(mmode, code as u16, eqtb);
                    }
                }
                UnexpandableCommand::CjkChar(c) => {
                    report_cjk_typesetting_unavailable(c, None, scanner, eqtb, logger)
                }
                UnexpandableCommand::LatinUcsChar(c) => {
                    report_latin_ucs_typesetting_unavailable(c, scanner, eqtb, logger)
                }
                UnexpandableCommand::CharNum => {
                    let chr = scanner.scan_char_num(eqtb, logger);
                    let code = eqtb.math_code(chr as usize);
                    if code >= 0o100000 {
                        treat_cur_chr_as_active_character(chr, scanner, eqtb, logger);
                    } else {
                        set_math_char(mmode, code as u16, eqtb);
                    }
                }
                UnexpandableCommand::NoBoundary => {
                    // Do nothing.
                }
                UnexpandableCommand::Raise => {
                    let dimen = scan_normal_dimen(scanner, eqtb, logger);
                    scan_box(
                        BoxContext::Shift(-dimen),
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::Lower => {
                    let dimen = scan_normal_dimen(scanner, eqtb, logger);
                    scan_box(
                        BoxContext::Shift(dimen),
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::MakeBox(make_box) => begin_box(
                    make_box,
                    BoxContext::Shift(0),
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::UnHbox { copy } => {
                    unpackage(copy, nest, scanner, eqtb, logger)
                }
                UnexpandableCommand::RemoveItem(remove_item) => {
                    delete_last(remove_item, nest, scanner, eqtb, logger)
                }
                UnexpandableCommand::Hskip(hskip) => {
                    let glue_node = get_hskip(hskip, scanner, eqtb, logger);
                    mmode.append_node(Node::Glue(glue_node), eqtb);
                }
                UnexpandableCommand::Halign => {
                    if privileged(unexpandable_command, scanner, eqtb, logger) {
                        if let GroupType::MathShift { .. } = eqtb.cur_group.typ {
                            align_state.init_align(
                                token,
                                hyphenator,
                                page_builder,
                                output,
                                nest,
                                scanner,
                                eqtb,
                                logger,
                            );
                        } else {
                            off_save(unexpandable_command, token, scanner, eqtb, logger);
                        }
                    }
                }
                UnexpandableCommand::Vrule => {
                    let rule_node = RuleNode::scan_vrule_spec(scanner, eqtb, logger);
                    mmode.append_node(Node::Rule(rule_node), eqtb);
                }
                UnexpandableCommand::Insert => begin_insert(nest, scanner, eqtb, logger),
                UnexpandableCommand::Mark(class) => {
                    mmode.append_node(make_mark(token, class, scanner, eqtb, logger), eqtb)
                }
                UnexpandableCommand::Show(show_command) => show_whatever(
                    show_command,
                    token,
                    nest,
                    page_builder,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::Vadjust => begin_adjust(nest, scanner, eqtb, logger),
                UnexpandableCommand::IgnoreSpaces => {
                    (unexpandable_command, token) =
                        scanner.get_next_non_blank_non_call_token(eqtb, logger);
                    continue;
                }
                UnexpandableCommand::AfterAssignment => {
                    token = scanner.get_token(eqtb, logger);
                    scanner.after_token = Some(token);
                }
                UnexpandableCommand::AfterGroup => {
                    token = scanner.get_token(eqtb, logger);
                    eqtb.save_for_after(token);
                }
                UnexpandableCommand::Penalty => {
                    mmode.append_node(make_penalty(scanner, eqtb, logger), eqtb)
                }
                UnexpandableCommand::StartPar { is_indented } => {
                    if is_indented {
                        indent_in_mmode(mmode, eqtb);
                    }
                }
                UnexpandableCommand::ItalCorr => {
                    mmode.append_node(Node::Kern(KernNode::new(0)), eqtb)
                }
                UnexpandableCommand::ExSpace => mmode.append_node(normal_space(eqtb), eqtb),
                UnexpandableCommand::HyphenBreak => {
                    mmode.append_node(hyphen_discretionary(eqtb, logger), eqtb)
                }
                UnexpandableCommand::Discretionary => {
                    append_discretionary(nest, scanner, eqtb, logger)
                }
                UnexpandableCommand::EqNo { is_left } => {
                    if privileged(unexpandable_command, scanner, eqtb, logger) {
                        if let GroupType::MathShift { .. } = eqtb.cur_group.typ {
                            start_eq_no(is_left, nest, scanner, eqtb, logger);
                        } else {
                            off_save(unexpandable_command, token, scanner, eqtb, logger);
                        }
                    }
                }
                UnexpandableCommand::Message { is_err } => {
                    issue_message(is_err, token, scanner, eqtb, logger)
                }
                UnexpandableCommand::OpenOut => do_open_out(nest, scanner, eqtb, logger),
                UnexpandableCommand::Write => do_write(token, nest, scanner, eqtb, logger),
                UnexpandableCommand::CloseOut => do_close_out(nest, scanner, eqtb, logger),
                UnexpandableCommand::Special => do_special(token, nest, scanner, eqtb, logger),
                UnexpandableCommand::SetLanguage => set_language(nest, scanner, eqtb, logger),
                UnexpandableCommand::Immediate => {
                    implement_immediate(output, scanner, eqtb, logger)
                }
                UnexpandableCommand::OpenIn => open_read_file(scanner, eqtb, logger),
                UnexpandableCommand::CloseIn => close_read_file(scanner, eqtb, logger),
                UnexpandableCommand::BeginGroup => {
                    eqtb.new_save_level(GroupType::SemiSimple, &scanner.input_stack, logger)
                }
                UnexpandableCommand::EndGroup => {
                    if let GroupType::SemiSimple = eqtb.cur_group.typ {
                        eqtb.unsave(scanner, logger);
                    } else {
                        off_save(unexpandable_command, token, scanner, eqtb, logger);
                    }
                }
                // Cases never allowed in math mode.
                UnexpandableCommand::Vskip(_)
                | UnexpandableCommand::Hrule
                | UnexpandableCommand::Valign
                | UnexpandableCommand::Endv
                | UnexpandableCommand::UnVbox { .. }
                | UnexpandableCommand::VDiscards(_)
                | UnexpandableCommand::End { .. }
                | UnexpandableCommand::ParEnd => insert_dollar_sign(token, scanner, eqtb, logger),
                UnexpandableCommand::Math(math_command) => match math_command {
                    MathCommand::Superscript(_) => superscript(nest, scanner, eqtb, logger),
                    MathCommand::Subscript(_) => subscript(nest, scanner, eqtb, logger),
                    MathCommand::Left => {
                        math_left(nest, scanner, eqtb, logger);
                    }
                    MathCommand::Middle => {
                        math_middle(token, nest, scanner, eqtb, logger);
                    }
                    MathCommand::Right => {
                        math_right(token, nest, scanner, eqtb, logger);
                    }
                    MathCommand::MSkip => {
                        let glue_node = get_mu_glue(scanner, eqtb, logger);
                        mmode.append_node(Node::Glue(glue_node), eqtb);
                    }
                    MathCommand::MKern => {
                        mmode.append_node(scan_mu_kern(scanner, eqtb, logger), eqtb)
                    }
                    MathCommand::MathAccent => {
                        math_ac(nest, scanner, eqtb, logger);
                    }
                    MathCommand::NormalNoad(noad_type) => {
                        let mut noad = NormalNoad::new();
                        noad.noad_type = noad_type;
                        if let Some(noad_field) = scan_math_char(scanner, eqtb, logger) {
                            noad.nucleus = Some(Box::new(noad_field));
                            mmode.append_node(Node::Noad(Box::new(Noad::Normal(noad))), eqtb);
                        } else {
                            mmode.append_node(Node::Noad(Box::new(Noad::Normal(noad))), eqtb);
                            scan_subformula_enclosed_in_braces(
                                SubformulaType::Nucleus,
                                nest,
                                scanner,
                                eqtb,
                                logger,
                            );
                        }
                    }
                    MathCommand::Underline => {
                        let mut noad = UnderNoad {
                            nucleus: None,
                            subscript: None,
                            superscript: None,
                        };
                        if let Some(noad_field) = scan_math_char(scanner, eqtb, logger) {
                            noad.nucleus = Some(Box::new(noad_field));
                            mmode.append_node(Node::Noad(Box::new(Noad::Under(noad))), eqtb);
                        } else {
                            mmode.append_node(Node::Noad(Box::new(Noad::Under(noad))), eqtb);
                            scan_subformula_enclosed_in_braces(
                                SubformulaType::Nucleus,
                                nest,
                                scanner,
                                eqtb,
                                logger,
                            );
                        }
                    }
                    MathCommand::Overline => {
                        let mut noad = OverNoad {
                            nucleus: None,
                            subscript: None,
                            superscript: None,
                        };
                        if let Some(noad_field) = scan_math_char(scanner, eqtb, logger) {
                            noad.nucleus = Some(Box::new(noad_field));
                            mmode.append_node(Node::Noad(Box::new(Noad::Over(noad))), eqtb);
                        } else {
                            mmode.append_node(Node::Noad(Box::new(Noad::Over(noad))), eqtb);
                            scan_subformula_enclosed_in_braces(
                                SubformulaType::Nucleus,
                                nest,
                                scanner,
                                eqtb,
                                logger,
                            );
                        }
                    }
                    MathCommand::Limits(limit_type) => {
                        math_limit_switch(limit_type, nest, scanner, eqtb, logger)
                    }
                    MathCommand::Fraction(fraction_command) => {
                        math_fraction(mmode, fraction_command, scanner, eqtb, logger)
                    }
                    MathCommand::Style(math_style) => {
                        mmode.append_node(Node::Style(StyleNode::new(math_style)), eqtb)
                    }
                    MathCommand::Choice => append_choices(nest, scanner, eqtb, logger),
                    MathCommand::NonScript => {
                        let mut glue_node = GlueNode::new(GlueSpec::zero_glue());
                        glue_node.subtype = GlueType::NonScript;
                        mmode.append_node(Node::Glue(glue_node), eqtb);
                    }
                    MathCommand::Vcenter => {
                        let spec = scan_spec(scanner, eqtb, logger);
                        eqtb.new_save_level(
                            GroupType::Vcenter { spec },
                            &scanner.input_stack,
                            logger,
                        );
                        scanner.scan_left_brace(eqtb, logger);
                        normal_paragraph(eqtb, logger);
                        nest.push_nest(
                            RichMode::Vertical(VerticalMode::new_internal()),
                            &scanner.input_stack,
                            eqtb,
                            logger,
                        );
                        if let Some(every_vbox) = eqtb.token_lists.get(TokenListVariable::EveryVbox)
                        {
                            scanner.input_stack.begin_token_list(
                                every_vbox.clone(),
                                TokenSourceType::EveryVboxText,
                                eqtb,
                                logger,
                            );
                        }
                    }
                    MathCommand::DelimNum => {
                        let delim = Integer::scan_twenty_seven_bit_int(scanner, eqtb, logger);
                        set_math_char(mmode, (delim / 0o10000) as u16, eqtb);
                    }
                    MathCommand::MathCharNum => {
                        let math_char = Integer::scan_fifteen_bit_int(scanner, eqtb, logger) as u16;
                        set_math_char(mmode, math_char, eqtb);
                    }
                    MathCommand::MathCharGiven(c) => set_math_char(mmode, c, eqtb),
                    MathCommand::Radical => math_radical(nest, scanner, eqtb, logger),
                },
                UnexpandableCommand::LatinUcsSupMark(_) => superscript(nest, scanner, eqtb, logger),
                UnexpandableCommand::LatinUcsSubMark(_) => subscript(nest, scanner, eqtb, logger),
                UnexpandableCommand::Kern => {
                    mmode.append_node(scan_kern(scanner, eqtb, logger), eqtb)
                }
                UnexpandableCommand::ShipOut => {
                    scan_box(
                        BoxContext::Shipout,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::Leaders(leader_kind) => {
                    scan_leader_box(
                        leader_kind,
                        page_builder,
                        output,
                        nest,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::UpperCase => shift_to_upper_case(token, scanner, eqtb, logger),
                UnexpandableCommand::LowerCase => shift_to_lower_case(token, scanner, eqtb, logger),
                UnexpandableCommand::Accent => {
                    complain_that_user_should_have_said_mathaccent(scanner, eqtb, logger);
                    math_ac(nest, scanner, eqtb, logger);
                }
                UnexpandableCommand::Omit => omit_error(scanner, eqtb, logger),
                UnexpandableCommand::EndCsName => cs_error(scanner, eqtb, logger),
                UnexpandableCommand::NoAlign => no_align_error(scanner, eqtb, logger),
                UnexpandableCommand::TabMark(_)
                | UnexpandableCommand::LatinUcsTabMark(_)
                | UnexpandableCommand::Span
                | UnexpandableCommand::CarRet { .. } => {
                    align_error(unexpandable_command, token, scanner, eqtb, logger)
                }
                UnexpandableCommand::LastPenalty
                | UnexpandableCommand::LastKern
                | UnexpandableCommand::LastSkip
                | UnexpandableCommand::Badness
                | UnexpandableCommand::FontCharDimension(_)
                | UnexpandableCommand::ParShapeDimension(_)
                | UnexpandableCommand::ETeXVersion
                | UnexpandableCommand::PraTeXVersion
                | UnexpandableCommand::PdfShellEscape
                | UnexpandableCommand::CurrentGroupLevel
                | UnexpandableCommand::CurrentGroupType
                | UnexpandableCommand::CurrentIfLevel
                | UnexpandableCommand::CurrentIfType
                | UnexpandableCommand::CurrentIfBranch
                | UnexpandableCommand::LastNodeType
                | UnexpandableCommand::Expr(_)
                | UnexpandableCommand::GlueComponent(_)
                | UnexpandableCommand::GlueConversion(_)
                | UnexpandableCommand::InputLineNumber
                | UnexpandableCommand::TextDirection(_)
                | UnexpandableCommand::MoveLeft
                | UnexpandableCommand::MoveRight
                | UnexpandableCommand::MacParam(_)
                | UnexpandableCommand::LatinUcsMacParam(_) => {
                    report_illegal_case(unexpandable_command, scanner, eqtb, logger)
                }
                // Cases that don't depend on mode
                UnexpandableCommand::Prefixable(prefixable_command) => prefixed_command(
                    prefixable_command,
                    token,
                    hyphenator,
                    page_builder,
                    output,
                    nest,
                    scanner,
                    eqtb,
                    logger,
                ),
            },
        }
        (unexpandable_command, token) = get_x_token(scanner, eqtb, logger);
    }
}

/// See 1031.
fn check_interrupt_and_show_trace(
    unexpandable_command: UnexpandableCommand,
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> bool {
    if logger.interrupt != 0 && logger.ok_to_interrupt {
        scanner.back_input(token, eqtb, logger);
        logger.check_interrupt(scanner, eqtb);
        return true;
    }
    if eqtb.integer(IntegerVariable::TracingCommands) > 0 {
        show_current_command(
            &Command::Unexpandable(unexpandable_command),
            scanner,
            eqtb,
            logger,
        );
    }
    false
}

/// Returns a normal space.
/// If spaceskip is set use that, otherwise use the font default.
/// See 1041.
fn normal_space(eqtb: &mut Eqtb) -> Node {
    let space_skip = eqtb.skips.get(SkipVariable::SpaceSkip);
    let space_glue = if **space_skip == GlueSpec::ZERO_GLUE {
        // Is spaceskip is not set, use the normal font space
        let spec = find_glue_specification_for_text_spaces(eqtb);
        GlueNode::new(spec)
    } else {
        // Else use spaceskip
        GlueNode::new_param(SkipVariable::SpaceSkip, eqtb)
    };
    Node::Glue(space_glue)
}

/// See 1042.
fn find_glue_specification_for_text_spaces(eqtb: &mut Eqtb) -> Rc<GlueSpec> {
    let cur_font_index = eqtb.cur_font();
    let cur_font = &mut eqtb.fonts[cur_font_index as usize];
    match &cur_font.glue {
        None => {
            // We need to take the 1-indexing into account.
            let glue_spec = Rc::new(GlueSpec {
                width: cur_font.space(),
                stretch: HigherOrderDimension {
                    order: DimensionOrder::Normal,
                    value: cur_font.space_stretch(),
                },
                shrink: HigherOrderDimension {
                    order: DimensionOrder::Normal,
                    value: cur_font.space_shrink(),
                },
            });
            cur_font.glue = Some(glue_spec.clone());
            glue_spec
        }
        Some(glue_spec) => glue_spec.clone(),
    }
}

/// Append a space to the current list.
/// If spacefactor is larger than 2000, use xspaceskip if set.
/// Otherwise use spaceskip set if set.
/// If spaceskip is set use that, otherwise use the font default.
/// See 1043.
fn space_node(space_factor: u16, eqtb: &mut Eqtb) -> Node {
    let xspace_skip = eqtb.skips.get(SkipVariable::XspaceSkip);
    let glue = if space_factor >= 2000 && **xspace_skip != GlueSpec::ZERO_GLUE {
        // If space_factor is larger than 2000 and xspaceskip is set use that
        GlueNode::new_param(SkipVariable::XspaceSkip, eqtb)
    } else {
        let space_skip = eqtb.skips.get(SkipVariable::SpaceSkip);
        // If spaceskip is set, use that; otherwise use default font space.
        let spec = if **space_skip != GlueSpec::ZERO_GLUE {
            space_skip.clone()
        } else {
            find_glue_specification_for_text_spaces(eqtb)
        };
        let mut spec = (*spec).clone();
        modify_glue_specification_according_to_space_factor(space_factor, &mut spec, eqtb);
        GlueNode::new(Rc::new(spec))
    };
    Node::Glue(glue)
}

/// See 1044.
fn modify_glue_specification_according_to_space_factor(
    space_factor: u16,
    spec: &mut GlueSpec,
    eqtb: &Eqtb,
) {
    if space_factor >= 2000 {
        spec.width += eqtb.fonts[eqtb.cur_font() as usize].extra_space();
    }
    spec.stretch.value = xn_over_d(spec.stretch.value, space_factor as i32, 1000)
        .expect("Overflow in space stretching");
    spec.shrink.value = xn_over_d(spec.shrink.value, 1000, space_factor as i32)
        .expect("Overflow in space shrinking");
}

/// See 1047.
fn insert_dollar_sign(token: Token, scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    scanner.back_input(token, eqtb, logger);
    logger.print_err("Missing $ inserted");
    let help = &[
        "I've inserted a begin-math/end-math symbol since I think",
        "you left one out. Proceed, with fingers crossed.",
    ];
    let math_shift = Token::MATH_SHIFT_TOKEN;
    scanner.ins_error(math_shift, help, eqtb, logger);
}

fn append_cjk_character(
    hmode: &mut crate::horizontal_mode::HorizontalMode,
    token: CjkToken,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    let font_index = match ensure_horizontal_japanese_font(scanner, eqtb) {
        Ok(font_index) => font_index,
        Err(error) => {
            report_cjk_typesetting_unavailable(token, Some(error), scanner, eqtb, logger);
            return;
        }
    };
    let code_point = token.code_point();
    let Some(font) = eqtb.japanese_fonts.get(font_index.position()) else {
        report_cjk_typesetting_unavailable(token, None, scanner, eqtb, logger);
        return;
    };
    let metrics = font.metrics_for_unicode(code_point);
    hmode.append_node(
        Node::WideChar(WideCharNode {
            font_index,
            character: code_point,
            class: metrics.class,
            width: metrics.width,
            height: metrics.height,
            depth: metrics.depth,
            italic: metrics.italic,
            jfm_boundary_before: crate::nodes::JfmBoundaryBefore::Continuous,
        }),
        eqtb,
    );
}

/// 明示選択を優先し、未選択時だけTeX Live標準の横組JFMを遅延して選ぶ。
///
/// 英文だけのrunでは和文資材を要求せず、最初のCJK文字で初めてTFM/JFM resolverを使う。
/// See 1041.
fn ensure_horizontal_japanese_font(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
) -> Result<JapaneseFontIndex, JapaneseFontError> {
    if let Some(font_index) = eqtb.cur_japanese_font() {
        return Ok(font_index);
    }

    let logical_path = Path::new(DEFAULT_HORIZONTAL_JFM);
    let requested_size = 10 * UNITY;
    if let Some(font_index) = eqtb
        .japanese_fonts
        .iter()
        .enumerate()
        .find_map(|(position, font)| {
            font.same_identity(logical_path, requested_size)
                .then(|| JapaneseFontIndex::from_position(position))
                .flatten()
        })
    {
        eqtb.japanese_font_define(Some(font_index), true);
        return Ok(font_index);
    }

    let font_index = JapaneseFontIndex::from_position(eqtb.japanese_fonts.len())
        .ok_or(JapaneseFontError::TooManyFonts)?;
    let mut font = load_japanese_font_info(
        logical_path,
        crate::fonts::SizeIndicator::AtSize(requested_size),
        scanner,
    )?;
    font.bind_index(font_index);
    eqtb.japanese_fonts.push(font);
    eqtb.japanese_font_define(Some(font_index), true);
    Ok(font_index)
}

/// resolverでも既定JFMを選べない時は、Unicodeをbyte分解せず明示診断を保つ。
fn report_cjk_typesetting_unavailable(
    token: CjkToken,
    default_font_error: Option<JapaneseFontError>,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    logger.print_err("CJK typesetting needs a Japanese font metric");
    logger.print_str(" (`");
    token.print_utf8(logger);
    logger.print_str("' was ignored)");
    if let Some(error) = default_font_error {
        logger.print_str("; automatic `");
        logger.print_str(DEFAULT_HORIZONTAL_JFM);
        logger.print_str(" at 10pt' selection failed: ");
        logger.print_str(&error.to_string());
    }
    let help = &[
        "Install upjisr-h.tfm in TeX Live, put it in the working directory,",
        "or select another bounded horizontal JFM with \\pratexjfont.",
        "I'll ignore this character and continue without splitting it into bytes.",
    ];
    logger.error(help, scanner, eqtb)
}

/// Stage 4cはUnicode欧文tokenを失わずに字句・patternへ渡す。OFM/文字node
/// が接続される前に8 bit fontへ切り詰めることはしない。
fn report_latin_ucs_typesetting_unavailable(
    token: LatinUcsToken,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    logger.print_err("Unicode European typesetting needs an OFM font metric");
    logger.print_str(" (`");
    token.print_utf8(logger);
    logger.print_str("' was ignored)");
    let help = &[
        "This engine can preserve this latin_ucs token, but OFM Level-0",
        "and the corresponding character node are not connected yet.",
        "I'll ignore this character without splitting its UTF-8 bytes.",
    ];
    logger.error(help, scanner, eqtb)
}

/// See 1051.
fn privileged(
    unexpandable_command: UnexpandableCommand,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> bool {
    if let Mode::Vertical | Mode::Horizontal | Mode::DisplayMath = eqtb.mode() {
        true
    } else {
        report_illegal_case(unexpandable_command, scanner, eqtb, logger);
        false
    }
}

/// See 1049.
pub fn you_cant(
    unexpandable_command: UnexpandableCommand,
    scanner: &Scanner,
    eqtb: &Eqtb,
    logger: &mut Logger,
) {
    logger.print_err("You can't use `");
    unexpandable_command.display(&eqtb.fonts, logger);
    logger.print_str("' in ");
    if scanner.scanning_write_tokens {
        logger.print_str("no mode");
    } else {
        eqtb.mode().display(logger);
    }
}

/// TeX--XeT の最初の縦sliceは restricted hbox 内だけへ閉じる。
///
/// 段落では改行ごとのLR stack補修が必要なので、direction nodeを置くだけの半端な対応を
/// 行わない。stateが無効な時のprimitiveも通常nodeへ黙って読み替えない。
fn append_text_direction(
    command: TextDirectionCommand,
    hmode: &mut HorizontalMode,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    if eqtb.integer(IntegerVariable::TeXXeTState) <= 0 {
        logger.print_err("Improper ");
        UnexpandableCommand::TextDirection(command).display(&eqtb.fonts, logger);
        let help = &[
            "Mixed-direction typesetting is disabled.",
            "Set \\TeXXeTstate to a positive value before using this command.",
        ];
        logger.error(help, scanner, eqtb);
        return;
    }
    if !matches!(hmode.subtype, HorizontalModeType::Restricted)
        || !hmode.accepts_text_direction_boundaries
    {
        report_illegal_case(
            UnexpandableCommand::TextDirection(command),
            scanner,
            eqtb,
            logger,
        );
        return;
    }

    let kind = match command {
        TextDirectionCommand::BeginLeft => MathNodeKind::Begin(TextDirection::LeftToRight),
        TextDirectionCommand::EndLeft => MathNodeKind::End(TextDirection::LeftToRight),
        TextDirectionCommand::BeginRight => MathNodeKind::Begin(TextDirection::RightToLeft),
        TextDirectionCommand::EndRight => MathNodeKind::End(TextDirection::RightToLeft),
    };
    hmode.break_jfm_pair_continuity(eqtb);
    hmode.append_node(Node::Math(MathNode { kind, width: 0 }), eqtb);
}

/// See 1050.
pub fn report_illegal_case(
    unexpandable_command: UnexpandableCommand,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    you_cant(unexpandable_command, scanner, eqtb, logger);
    let help = &[
        "Sorry, but I'm not programmed to handle this case;",
        "I'll just pretend that you didn't ask for it.",
        "If you're in the wrong mode, you might be able to",
        "return to the right one by typing `I}' or `I$' or `I\\par'.",
    ];
    logger.error(help, scanner, eqtb)
}

/// See 1054.
fn its_all_over(
    unexpandable_command: UnexpandableCommand,
    token: Token,
    page_builder: &mut PageBuilder,
    output: &mut Output,
    nest: &mut SemanticState,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> bool {
    if privileged(unexpandable_command, scanner, eqtb, logger) {
        if page_builder.page.is_empty() && nest.cur_list_is_empty() && eqtb.dead_cycles == 0 {
            return true;
        }
        scanner.back_input(token, eqtb, logger);
        let mut hbox = ListNode::new_empty_hbox();
        hbox.width = eqtb.dimen(DimensionVariable::Hsize);
        nest.tail_push(Node::List(Box::new(hbox)), eqtb);
        nest.tail_push(Node::Glue(GlueNode::new(GlueSpec::fill_glue())), eqtb);
        nest.tail_push(Node::Penalty(PenaltyNode::new(-0o100_0000_0000)), eqtb);
        page_builder.build_page(output, nest, scanner, eqtb, logger);
    }
    false
}

/// Prints the given command code and character together with the mode.
/// See 299.
fn show_current_command(command: &Command, scanner: &Scanner, eqtb: &Eqtb, logger: &mut Logger) {
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
    command.display(&eqtb.fonts, logger);
    logger.print_char(b'}');
    logger.end_diagnostic(false);
}

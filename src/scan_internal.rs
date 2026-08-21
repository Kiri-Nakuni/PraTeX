use crate::command::{BoxDimension, InternalCommand, PageDimension, ToksCommand};
use crate::dimension::{Dimension, MAX_DIMEN};
use crate::eqtb::Eqtb;
use crate::eqtb::{
    CodeType, DimensionVariable, FontIndex, IntegerVariable, LastNodeInfo, ParShapeVariable,
    SkipVariable, TokenListVariable,
};
use crate::error::mu_error;
use crate::fonts::{find_font_dimen, scan_font_ident};
use crate::format::{Dumpable, FormatError};
use crate::input::Scanner;
use crate::logger::Logger;
use crate::nodes::{GlueSpec, HigherOrderDimension};
use crate::page_breaking::PageContents;
use crate::print::Printer;
use crate::semantic_nest::Mode;
use crate::token::Token;

use std::io::Write;
use std::rc::Rc;

/// See 410.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Int,
    Dimen,
    Glue,
    Mu,
}

/// See 410.
pub enum InternalValue {
    Int(i32),
    Dimen(Dimension),
    Glue(Rc<GlueSpec>),
    MuGlue(Rc<GlueSpec>),
    Ident(FontIndex),
    TokenList(Vec<Token>),
}

/// See 413.
pub fn scan_internal_toks(
    internal_command: InternalCommand,
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    scan_something_internal(internal_command, token, true, scanner, eqtb, logger)
}

/// See 413.
pub fn scan_internal_integer(
    internal_command: InternalCommand,
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> i32 {
    let value = scan_something_internal(internal_command, token, false, scanner, eqtb, logger);
    // Coerce internal value to integer.
    match value {
        InternalValue::MuGlue(glue_spec) => {
            mu_error(scanner, eqtb, logger);
            glue_spec.width
        }
        InternalValue::Glue(glue_spec) => glue_spec.width,
        InternalValue::Dimen(dimen) => dimen,
        InternalValue::Int(integer) => integer,
        InternalValue::Ident(_) | InternalValue::TokenList(_) => panic!("Should not be possible"),
    }
}

/// See 413.
pub fn scan_internal_dimension(
    internal_command: InternalCommand,
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    let mut value = scan_something_internal(internal_command, token, false, scanner, eqtb, logger);
    // Coerce internal value to dimension.
    if let InternalValue::MuGlue(glue_spec) = &value {
        mu_error(scanner, eqtb, logger);
        value = InternalValue::Glue(glue_spec.clone());
    }
    if let InternalValue::Glue(glue_spec) = &value {
        value = InternalValue::Dimen(glue_spec.width);
    }
    value
}

/// See 413.
pub fn scan_internal_glue(
    internal_command: InternalCommand,
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    let mut value = scan_something_internal(internal_command, token, false, scanner, eqtb, logger);
    // Coerce internal value to Glue.
    if let InternalValue::MuGlue(glue_spec) = value {
        mu_error(scanner, eqtb, logger);
        value = InternalValue::Glue(glue_spec);
    }
    value
}

/// See 413.
pub fn scan_internal_mu_glue(
    internal_command: InternalCommand,
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    scan_something_internal(internal_command, token, false, scanner, eqtb, logger)
}

/// Returns the value and value level.
/// See 413.
fn scan_something_internal(
    internal_command: InternalCommand,
    token: Token,
    toks_allowed: bool,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    let value = match internal_command {
        InternalCommand::CharGiven(c) => InternalValue::Int(c as i32),
        InternalCommand::MathCharGiven(c) => InternalValue::Int(c as i32),
        InternalCommand::LastPenalty => fetch_last_penalty(scanner, eqtb),
        InternalCommand::LastKern => fetch_last_kern(scanner, eqtb),
        InternalCommand::LastSkip => fetch_last_skip(scanner, eqtb),
        InternalCommand::Badness => InternalValue::Int(eqtb.last_badness),
        // ==== e-TeX / pdfTeX の問い合わせ ====
        InternalCommand::ETeXVersion => InternalValue::Int(2),
        // 外部コマンドの実行を許さないため、読み取り専用の無効状態だけを答える。
        InternalCommand::PdfShellEscape => InternalValue::Int(0),
        InternalCommand::CurrentGroupLevel => {
            InternalValue::Int(eqtb.cur_level_for_etex())
        }
        InternalCommand::CurrentGroupType => InternalValue::Int(eqtb.cur_group_for_etex()),
        InternalCommand::CurrentIfLevel => InternalValue::Int(scanner.cur_if_level_for_etex()),
        InternalCommand::CurrentIfType => InternalValue::Int(scanner.cur_if_type_for_etex()),
        InternalCommand::CurrentIfBranch => {
            InternalValue::Int(scanner.cur_if_branch_for_etex())
        }
        InternalCommand::LastNodeType => InternalValue::Int(eqtb.last_node_type_for_etex()),
        InternalCommand::InteractionMode => InternalValue::Int(logger.interaction as i32),
        InternalCommand::InputLineNumber => InternalValue::Int(eqtb.line_number() as i32),
        InternalCommand::Toks(toks_command) => fetch_token_list_or_font_identifier(
            toks_command,
            toks_allowed,
            token,
            scanner,
            eqtb,
            logger,
        ),
        InternalCommand::Expr(kind) => scan_expr(kind, scanner, eqtb, logger),
        InternalCommand::Integer(int_var) => InternalValue::Int(eqtb.integer(int_var)),
        InternalCommand::Dimension(dim_var) => InternalValue::Dimen(eqtb.dimen(dim_var)),
        InternalCommand::Glue(glue_var) => InternalValue::Glue(eqtb.skips.get(glue_var).clone()),
        InternalCommand::MuGlue(mu_glue_var) => {
            InternalValue::MuGlue(eqtb.skips.get(mu_glue_var).clone())
        }
        InternalCommand::FontDimen => fetch_font_dimension(scanner, eqtb, logger),
        InternalCommand::HyphenChar => {
            let font_index = scan_font_ident(scanner, eqtb, logger);
            InternalValue::Int(eqtb.fonts[font_index as usize].hyphen_char)
        }
        InternalCommand::SkewChar => {
            let font_index = scan_font_ident(scanner, eqtb, logger);
            InternalValue::Int(eqtb.fonts[font_index as usize].skew_char)
        }
        InternalCommand::SpaceFactor => fetch_space_factor(toks_allowed, scanner, eqtb, logger),
        InternalCommand::PrevDepth => fetch_prev_depth(toks_allowed, scanner, eqtb, logger),
        InternalCommand::PrevGraf => fetch_prev_graf(scanner, eqtb),
        InternalCommand::PageDimen(page_dimension) => {
            fetch_something_on_page_so_far(page_dimension, eqtb)
        }
        InternalCommand::DeadCycles => InternalValue::Int(eqtb.dead_cycles),
        InternalCommand::InsertPenalties => InternalValue::Int(eqtb.insert_penalties),
        InternalCommand::BoxDimen(box_dimension) => {
            fetch_box_dimension(box_dimension, scanner, eqtb, logger)
        }
        InternalCommand::ParShape => fetch_par_shape_size(eqtb),
        InternalCommand::CatCode => fetch_category_code(scanner, eqtb, logger),
        InternalCommand::Code(code) => fetch_character_code(code, scanner, eqtb, logger),
        InternalCommand::Register(value_type) => fetch_register(value_type, scanner, eqtb, logger),
    };
    value
}

/// See 414.
fn fetch_category_code(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    let chr = scanner.scan_char_num(eqtb, logger);
    let cat_code = *eqtb.cat_codes.get(chr);
    InternalValue::Int(cat_code as i32)
}

/// See 414.
fn fetch_character_code(
    code: CodeType,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    let n = scanner.scan_char_num(eqtb, logger) as usize;
    let code_var = code.to_variable(n);
    let value = *eqtb.codes.get(code_var);
    InternalValue::Int(value)
}

/// See 415.
fn fetch_token_list_or_font_identifier(
    toks_command: ToksCommand,
    toks_allowed: bool,
    token: Token,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    if toks_allowed {
        match toks_command {
            ToksCommand::TokenListRegister => {
                let register = scanner.scan_register_index(eqtb, logger);
                let toks_var = TokenListVariable::Toks(register);
                if let Some(list) = eqtb.token_lists.get(toks_var) {
                    // Clone the underlying vector
                    InternalValue::TokenList((**list).clone())
                } else {
                    InternalValue::TokenList(Vec::new())
                }
            }
            ToksCommand::TokenList(toks_var) => {
                if let Some(list) = eqtb.token_lists.get(toks_var) {
                    // Clone the underlying vector
                    InternalValue::TokenList((**list).clone())
                } else {
                    InternalValue::TokenList(Vec::new())
                }
            }
            ToksCommand::DefFamily(_) | ToksCommand::DefFont | ToksCommand::SetFont(_) => {
                scanner.back_input(token, eqtb, logger);
                let font_index = scan_font_ident(scanner, eqtb, logger);
                InternalValue::Ident(font_index)
            }
        }
    } else {
        logger.print_err("Missing number, treated as zero");
        let help = &[
            "A number should have been here; I inserted `0'.",
            "(If you can't figure out why I needed to see a number,",
            "look up `weird error' in the index to The TeXbook.)",
        ];
        scanner.back_error(token, help, eqtb, logger);
        InternalValue::Dimen(0)
    }
}

/// See 418.
fn fetch_space_factor(
    toks_allowed: bool,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    match eqtb.mode().base() {
        Mode::Horizontal if !scanner.scanning_write_tokens => {
            InternalValue::Int(eqtb.space_factor as i32)
        }
        _ => {
            logger.print_err("Improper ");
            logger.print_esc_str(b"spacefactor");
            let help = &[
                "You can refer to \\spacefactor only in horizontal mode;",
                "you can refer to \\prevdepth only in vertical mode; and",
                "neither of these is meaningful inside \\write. So",
                "I'm forgetting what you said and using zero instead.",
            ];
            logger.error(help, scanner, eqtb);
            if !toks_allowed {
                InternalValue::Dimen(0)
            } else {
                InternalValue::Int(0)
            }
        }
    }
}

/// See 418.
fn fetch_prev_depth(
    toks_allowed: bool,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    if let Mode::Vertical = eqtb.mode().base() {
        InternalValue::Dimen(eqtb.prev_depth)
    } else {
        logger.print_err("Improper ");
        logger.print_esc_str(b"prevdepth");
        let help = &[
            "You can refer to \\spacefactor only in horizontal mode;",
            "you can refer to \\prevdepth only in vertical mode; and",
            "neither of these is meaningful inside \\write. So",
            "I'm forgetting what you said and using zero instead.",
        ];
        logger.error(help, scanner, eqtb);
        if !toks_allowed {
            InternalValue::Dimen(0)
        } else {
            InternalValue::Int(0)
        }
    }
}

/// See 420.
fn fetch_box_dimension(
    box_dimension: BoxDimension,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    let register = scanner.scan_register_index(eqtb, logger);
    let boks = eqtb.boks(register);
    let value = match boks {
        None => 0,
        Some(list_node) => match box_dimension {
            BoxDimension::Width => list_node.width,
            BoxDimension::Height => list_node.height,
            BoxDimension::Depth => list_node.depth,
        },
    };
    InternalValue::Dimen(value)
}

/// See 421.
fn fetch_something_on_page_so_far(page_dimension: PageDimension, eqtb: &Eqtb) -> InternalValue {
    let value = if eqtb.page_contents == PageContents::Empty && !eqtb.output_active {
        if let PageDimension::PageGoal = page_dimension {
            MAX_DIMEN
        } else {
            0
        }
    } else {
        match page_dimension {
            PageDimension::PageGoal => eqtb.page_dims.page_goal,
            PageDimension::Height => eqtb.page_dims.height,
            PageDimension::Stretch => eqtb.page_dims.stretch.normal,
            PageDimension::FilStretch => eqtb.page_dims.stretch.fil,
            PageDimension::FillStretch => eqtb.page_dims.stretch.fill,
            PageDimension::FilllStretch => eqtb.page_dims.stretch.filll,
            PageDimension::Shrink => eqtb.page_dims.shrink,
            PageDimension::Depth => eqtb.page_dims.depth,
        }
    };
    InternalValue::Dimen(value)
}

/// See 422.
fn fetch_prev_graf(scanner: &Scanner, eqtb: &Eqtb) -> InternalValue {
    if scanner.scanning_write_tokens {
        InternalValue::Int(0)
    } else {
        InternalValue::Int(eqtb.prev_graf)
    }
}

/// See 423.
fn fetch_par_shape_size(eqtb: &Eqtb) -> InternalValue {
    let par_shape = eqtb.par_shape.get(ParShapeVariable);
    InternalValue::Int(par_shape.len() as i32)
}

/// See 424.
fn fetch_last_penalty(scanner: &Scanner, eqtb: &Eqtb) -> InternalValue {
    if scanner.scanning_write_tokens {
        InternalValue::Int(0)
    } else if let LastNodeInfo::Penalty(penalty) = eqtb.last_node_info {
        InternalValue::Int(penalty)
    } else {
        InternalValue::Int(0)
    }
}

/// See 424.
fn fetch_last_kern(scanner: &Scanner, eqtb: &Eqtb) -> InternalValue {
    if scanner.scanning_write_tokens {
        InternalValue::Dimen(0)
    } else if let LastNodeInfo::Kern(dimen) = eqtb.last_node_info {
        InternalValue::Dimen(dimen)
    } else {
        InternalValue::Dimen(0)
    }
}

/// See 424.
fn fetch_last_skip(scanner: &Scanner, eqtb: &Eqtb) -> InternalValue {
    if scanner.scanning_write_tokens {
        InternalValue::Glue(GlueSpec::zero_glue())
    } else {
        match &eqtb.last_node_info {
            LastNodeInfo::Glue(glue_spec) => InternalValue::Glue(glue_spec.clone()),
            LastNodeInfo::MuGlue(glue_spec) => InternalValue::MuGlue(glue_spec.clone()),
            _ => InternalValue::Glue(GlueSpec::zero_glue()),
        }
    }
}

/// See 425.
fn fetch_font_dimension(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    match find_font_dimen(false, scanner, eqtb, logger) {
        Some((font_index, param_index)) => {
            InternalValue::Dimen(eqtb.fonts[font_index as usize].params[param_index])
        }
        None => InternalValue::Dimen(0),
    }
}

/// See 427.
fn fetch_register(
    value_type: ValueType,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    let register = scanner.scan_register_index(eqtb, logger);
    match value_type {
        ValueType::Int => InternalValue::Int(eqtb.integer(IntegerVariable::Count(register))),
        ValueType::Dimen => InternalValue::Dimen(eqtb.dimen(DimensionVariable::Dimen(register))),
        ValueType::Glue => {
            InternalValue::Glue(eqtb.skips.get(SkipVariable::Skip(register)).clone())
        }
        ValueType::Mu => {
            InternalValue::MuGlue(eqtb.skips.get(SkipVariable::MuSkip(register)).clone())
        }
    }
}

impl Dumpable for ValueType {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Int => writeln!(target, "Int")?,
            Self::Dimen => writeln!(target, "Dimen")?,
            Self::Glue => writeln!(target, "Glue")?,
            Self::Mu => writeln!(target, "Mu")?,
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Int" => Ok(Self::Int),
            "Dimen" => Ok(Self::Dimen),
            "Glue" => Ok(Self::Glue),
            "Mu" => Ok(Self::Mu),
            _ => Err(FormatError::ParseError),
        }
    }
}


// ================= e-TeX の式（`\numexpr` 系）=================
//
// **文法は二段だけである。**
//
// ```text
// expr ::= term | expr + term | expr - term
// term ::= factor | term * integer | term / integer
// ```
//
// `\relax` があれば食う。無ければ戻す——**e-TeX がそう決めている。**
//
// # なぜ乗除が特別か
//
// `\numexpr 7*8/3\relax` は **56/3 を丸めて 19** である。**23 ではない。**
// 掛けてから割るあいだ、**中間結果は 32 ビットに収まらなくてよい。**
// これが `\numexpr` の存在理由であり、
// `\multiply` `\divide` を並べるのとの違いである。
//
// 丸めは**四捨五入**（半分は絶対値の大きい方へ）。TeX の `xn_over_d` と同じ向き。

/// 掛けてから割る。**中間は 64 ビット、丸めは四捨五入。**
fn mult_and_add(x: i64, n: i64, d: i64, max: i64) -> Result<i64, ()> {
    if d == 0 {
        return Err(());
    }
    let prod = x.checked_mul(n).ok_or(())?;
    // **半分は絶対値の大きい方へ寄せる**
    let half = d / 2;
    let q = if prod >= 0 { (prod + half) / d } else { -((-prod + half) / d) };
    if q.abs() > max {
        return Err(());
    }
    Ok(q)
}

fn expr_max(kind: ValueType) -> i64 {
    match kind {
        ValueType::Int => 0x7FFF_FFFF,
        _ => crate::dimension::MAX_DIMEN as i64,
    }
}

/// 一つの式を読む。
fn scan_expr(
    kind: ValueType,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> InternalValue {
    let mut overflow = false;
    let v = scan_expr_sum(kind, &mut overflow, scanner, eqtb, logger);
    // **末尾の `\relax` は食う。** 無ければ戻す
    let (cmd, token) = scanner.get_next_non_blank_non_call_token(eqtb, logger);
    if !matches!(cmd, crate::command::UnexpandableCommand::Relax { .. }) {
        scanner.back_input(token, eqtb, logger);
    }
    if overflow {
        arith_overflow(scanner, eqtb, logger);
    }
    match kind {
        ValueType::Int => InternalValue::Int(v.width as i32),
        ValueType::Dimen => InternalValue::Dimen(v.width),
        ValueType::Glue => InternalValue::Glue(std::rc::Rc::new(v)),
        ValueType::Mu => InternalValue::MuGlue(std::rc::Rc::new(v)),
    }
}

fn arith_overflow(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    logger.print_err("Arithmetic overflow");
    let help = &[
        "I can't evaluate this expression,",
        "since the result is out of range.",
    ];
    logger.error(help, scanner, eqtb);
}

/// `expr ::= term | expr + term | expr - term`
fn scan_expr_sum(
    kind: ValueType,
    overflow: &mut bool,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> GlueSpec {
    let mut acc = scan_expr_term(kind, overflow, scanner, eqtb, logger);
    loop {
        let (cmd, token) = scanner.get_next_non_blank_non_call_token(eqtb, logger);
        let plus = match cmd {
            crate::command::UnexpandableCommand::Other(b'+') => true,
            crate::command::UnexpandableCommand::Other(b'-') => false,
            _ => {
                scanner.back_input(token, eqtb, logger);
                return acc;
            }
        };
        let rhs = scan_expr_term(kind, overflow, scanner, eqtb, logger);
        acc = add_spec(acc, rhs, plus, kind, overflow);
    }
}

fn add_spec(
    a: GlueSpec,
    b: GlueSpec,
    plus: bool,
    kind: ValueType,
    overflow: &mut bool,
) -> GlueSpec {
    let max = expr_max(kind);
    let add = |x: i32, y: i32, o: &mut bool| -> i32 {
        let r = x as i64 + if plus { y as i64 } else { -(y as i64) };
        if r.abs() > max {
            *o = true;
            return if r < 0 { -(max as i32) } else { max as i32 };
        }
        r as i32
    };
    let mut out = a.clone();
    out.width = add(a.width, b.width, overflow);
    // **伸縮は `\glueexpr` と `\muexpr` でだけ意味を持つ。**
    // 次数が違えば**大きい次数が勝つ**——TeX の糊の規則そのものである
    if matches!(kind, ValueType::Glue | ValueType::Mu) {
        out.stretch = combine(a.stretch.clone(), b.stretch.clone(), plus, overflow, max);
        out.shrink = combine(a.shrink.clone(), b.shrink.clone(), plus, overflow, max);
    }
    out

}

fn combine(
    a: HigherOrderDimension,
    b: HigherOrderDimension,
    plus: bool,
    overflow: &mut bool,
    max: i64,
) -> HigherOrderDimension {
    let bv = if plus { b.value } else { -b.value };
    if a.value == 0 {
        return HigherOrderDimension { order: b.order, value: bv };
    }
    if bv == 0 {
        return a;
    }
    match order_rank(a.order).cmp(&order_rank(b.order)) {
        std::cmp::Ordering::Greater => a,
        std::cmp::Ordering::Less => HigherOrderDimension { order: b.order, value: bv },
        std::cmp::Ordering::Equal => {
            let r = a.value as i64 + bv as i64;
            if r.abs() > max {
                *overflow = true;
            }
            HigherOrderDimension { order: a.order, value: r.clamp(-max, max) as i32 }
        }
    }
}

/// 次数の強さ。**`DimensionOrder` に順序が入っていない**ので、ここで与える
fn order_rank(o: crate::nodes::DimensionOrder) -> u8 {
    use crate::nodes::DimensionOrder::*;
    match o {
        Normal => 0,
        Fil => 1,
        Fill => 2,
        Filll => 3,
    }
}

/// `term ::= factor | term * integer | term / integer`
fn scan_expr_term(
    kind: ValueType,
    overflow: &mut bool,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> GlueSpec {
    let mut acc = scan_expr_factor(kind, overflow, scanner, eqtb, logger);
    // **掛けと割りは溜めてから一度に行う**——中間結果を 32 ビットに落とさないため
    let mut num: i64 = 1;
    let mut den: i64 = 1;
    loop {
        let (cmd, token) = scanner.get_next_non_blank_non_call_token(eqtb, logger);
        let mul = match cmd {
            crate::command::UnexpandableCommand::Other(b'*') => true,
            crate::command::UnexpandableCommand::Other(b'/') => false,
            _ => {
                scanner.back_input(token, eqtb, logger);
                break;
            }
        };
        // **掛ける数・割る数の側にも括弧を書ける。** `(1+2)*(3+4)`
        let n = scan_expr_int_operand(overflow, scanner, eqtb, logger) as i64;
        if mul {
            num *= n;
        } else {
            den *= n;
        }
    }
    if num != 1 || den != 1 {
        let max = expr_max(kind);
        let mut apply = |x: i32, o: &mut bool| -> i32 {
            match mult_and_add(x as i64, num, den, max) {
                Ok(v) => v as i32,
                Err(()) => {
                    *o = true;
                    0
                }
            }
        };
        acc.width = apply(acc.width, overflow);
        if matches!(kind, ValueType::Glue | ValueType::Mu) {
            acc.stretch.value = apply(acc.stretch.value, overflow);
            acc.shrink.value = apply(acc.shrink.value, overflow);
        }
    }
    acc
}

/// 掛ける数・割る数。**整数だが、括弧なら中は式である。**
fn scan_expr_int_operand(
    overflow: &mut bool,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> i32 {
    let (cmd, token) = scanner.get_next_non_blank_non_call_token(eqtb, logger);
    if matches!(cmd, crate::command::UnexpandableCommand::Other(b'(')) {
        let v = scan_expr_sum(ValueType::Int, overflow, scanner, eqtb, logger);
        let (cmd, token) = scanner.get_next_non_blank_non_call_token(eqtb, logger);
        if !matches!(cmd, crate::command::UnexpandableCommand::Other(b')')) {
            scanner.back_input(token, eqtb, logger);
            missing_paren(scanner, eqtb, logger);
        }
        return v.width;
    }
    scanner.back_input(token, eqtb, logger);
    <i32 as crate::integer::IntegerExt>::scan_int(scanner, eqtb, logger)
}

fn missing_paren(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    logger.print_err("Missing ) inserted for expression");
    let help = &["I was expecting to see `+', `-', `*', `/', or `)'. Didn't."];
    logger.error(help, scanner, eqtb);
}

/// `factor ::= <値> | ( expr )`
fn scan_expr_factor(
    kind: ValueType,
    overflow: &mut bool,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> GlueSpec {
    let (cmd, token) = scanner.get_next_non_blank_non_call_token(eqtb, logger);
    if matches!(cmd, crate::command::UnexpandableCommand::Other(b'(')) {
        let v = scan_expr_sum(kind, overflow, scanner, eqtb, logger);
        let (cmd, token) = scanner.get_next_non_blank_non_call_token(eqtb, logger);
        if !matches!(cmd, crate::command::UnexpandableCommand::Other(b')')) {
            scanner.back_input(token, eqtb, logger);
            missing_paren(scanner, eqtb, logger);
        }
        return v;
    }
    scanner.back_input(token, eqtb, logger);
    let mut spec = GlueSpec::ZERO_GLUE;
    match kind {
        ValueType::Int => spec.width = <i32 as crate::integer::IntegerExt>::scan_int(scanner, eqtb, logger),
        ValueType::Dimen => spec.width = crate::dimension::scan_normal_dimen(scanner, eqtb, logger),
        ValueType::Glue => spec = crate::glue::scan_glue(false, scanner, eqtb, logger).as_ref().clone(),
        ValueType::Mu => spec = crate::glue::scan_glue(true, scanner, eqtb, logger).as_ref().clone(),
    }
    spec
}

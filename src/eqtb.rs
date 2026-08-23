mod boxes;
mod catcodes;
mod character_classifier;
mod codes;
mod control_sequences;
mod dimensions;
mod extended_registers;
mod fonts;
mod integers;
mod kcatcodes;
mod language_region;
mod levels;
mod parshape;
mod penalties;
mod primitives;
mod raw_strings;
pub mod save_stack;
mod skips;
mod tokenlists;

use crate::command::{Command, ExpandableCommand, MacroCall, UnexpandableCommand};
use crate::dimension::Dimension;
use crate::error::overflow;
use crate::fonts::FontInfo;
use crate::format::{Dumpable, FormatError};
use crate::input::{InputStack, Scanner};
use crate::integer::Integer;
use crate::japanese_fonts::{JapaneseFontIndex, JapaneseFontInfo};
use crate::logger::Logger;
use crate::macros::show_macro_def;
use crate::nodes::{show_list_node, GlueSpec, GlueType, ListNode, Node};
use crate::page_breaking::{Marks, PageContents, PageDimensions};
use crate::print::Printer;
use crate::runtime_clock::RunDateTime;
use crate::script_spacing::planner::{
    AutoSpacingState, AutoSpacingVariable, InhibitXspCodeTable, LayoutCharacterCode,
    SpacingStateError, XspCode, XspCodeTable,
};
use crate::semantic_nest::Mode;
use crate::token::Token;
use crate::token_lists::{show_token_list, RcTokenList};
use crate::vertical_mode::IGNORE_DEPTH;

use save_stack::{Group, GroupType, SaveEntry};

use boxes::BoxParameters;
pub use boxes::BoxVariable;
pub use catcodes::CatCode;
use catcodes::CatCodes;
use catcodes::CARRIAGE_RETURN;
#[cfg(test)]
pub(crate) use character_classifier::CallbackClassifier;
pub(crate) use character_classifier::{
    CharacterClassifier, ClassificationContext, InputCategory,
};
use codes::CodeParameters;
pub use codes::{
    CodeType, CodeVariable, MAX_LATIN_UCS_CASE_CODE, MAX_LATIN_UCS_CODE, VAR_CODE,
};
use control_sequences::ControlSequenceStore;
pub use control_sequences::{
    ControlSequence, ControlSequenceId, ControlSequenceNameUnit, NamespaceId,
};
use dimensions::DimensionParameters;
pub use dimensions::DimensionVariable;
use fonts::FontParameters;
pub use fonts::{FontIndex, FontVariable, MathFontSize, NULL_FONT};
use integers::IntegerParameters;
pub use integers::IntegerVariable;
use kcatcodes::KCatCodes;
pub use kcatcodes::{KCatCode, KCatCodeBlock};
pub use language_region::LanguageRegion;
use levels::{Level, VariableLevels};
use parshape::ParShapeParameter;
pub use parshape::{ParShapeVariable, ParagraphShape};
use penalties::PenaltyArrayParameters;
pub use penalties::{PenaltyArray, PenaltyArrayVariable};
use raw_strings::RawStringRegisters;
pub use raw_strings::RawStringVariable;
pub(crate) use raw_strings::{
    print_raw_diagnostic, raw_bytes_as_other_tokens, RcRawString, RawStringStorageError,
};
#[cfg(test)]
use raw_strings::MAX_RAW_STRING_BYTES;
pub use skips::SkipVariable;
use skips::{Skip, SkipParameters};
use tokenlists::TokenListParameters;
pub use tokenlists::TokenListVariable;

use std::convert::TryFrom;
use std::collections::{BTreeSet, HashMap};
use std::io::Write;
use std::rc::Rc;

const MAX_GROUPING_DEPTH: usize = 255;

/// e-TeX の通常レジスタ番号。上限は型の上限ではなく15 bitである。
pub type RegisterIndex = u16;
pub const MAX_REGISTER_INDEX: RegisterIndex = extended_registers::MAX_EXTENDED_REGISTER_INDEX;

/// 挿入クラス番号。通常レジスタと違い、e-TeX でも 0..=254 のまま。
pub type InsertionIndex = u8;
pub const MAX_INSERTION_INDEX: InsertionIndex = 254;
pub(crate) const VADJUST_INSERTION_CODE: InsertionIndex = 255;

/// e-TeX の mark class。通常レジスタと同じ15 bitだが、意味を混ぜない別名にする。
pub type MarkClassIndex = u16;
pub const MAX_MARK_CLASS_INDEX: MarkClassIndex = 32_767;

/// 書式ファイルから通常レジスタ番号を読む。型として表現できても、e-TeX の
/// 15 bit 上限を越える値は後段のストレージへ渡さない。
pub(crate) fn undump_register_index<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
) -> Result<RegisterIndex, FormatError> {
    let register = RegisterIndex::undump(lines)?;
    if register <= MAX_REGISTER_INDEX {
        Ok(register)
    } else {
        Err(FormatError::ParseError)
    }
}

pub(crate) fn undump_insertion_index<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
) -> Result<InsertionIndex, FormatError> {
    let index = InsertionIndex::undump(lines)?;
    if index <= MAX_INSERTION_INDEX {
        Ok(index)
    } else {
        Err(FormatError::ParseError)
    }
}

pub(crate) fn undump_mark_class_index<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
) -> Result<MarkClassIndex, FormatError> {
    let index = MarkClassIndex::undump(lines)?;
    if index <= MAX_MARK_CLASS_INDEX {
        Ok(index)
    } else {
        Err(FormatError::ParseError)
    }
}

/// See Part 19.
pub struct Eqtb {
    pub cur_level: Level,
    pub cur_group: Group,
    pub save_stack: Vec<Group>,
    pub max_save_stack: usize,

    pub variable_levels: VariableLevels,

    // Regions 1 and 2
    pub control_sequences: ControlSequenceStore,

    // Region 3
    pub skips: SkipParameters,

    // Region 4
    pub par_shape: ParShapeParameter,
    penalty_arrays: PenaltyArrayParameters,
    language_region: LanguageRegion,
    /// **参照時に探しに行く名前空間。** `\usingnamespace` が足す。
    ///
    /// 空なら**素の TeX82 とまったく同じ道**を通る——
    /// 使わない機能に費用を持たせない
    pub using_namespaces: Vec<NamespaceId>,
    /// `\lastnodetype`（e-TeX）。**-1 は「無い」。**
    ///
    /// 種類の番号は e-TeX のもの：0 文字 / 1 hlist / 2 vlist / 3 罫 / 4 差し込み /
    /// 5 印 / 6 調整 / 7 合字 / 8 分割 / 9 whatsit / 10 数式 / 11 糊 / 12 カーン /
    /// 13 罰点 / 14 未設定
    pub last_node_type: i32,
    pub token_lists: TokenListParameters,
    raw_strings: RawStringRegisters,
    pub boxes: BoxParameters,
    pub font_params: FontParameters,
    cur_japanese_font: Option<JapaneseFontIndex>,
    pub cat_codes: CatCodes,
    kcat_codes: KCatCodes,
    auto_spacing: AutoSpacingState,
    xsp_codes: XspCodeTable,
    inhibit_xsp_codes: InhibitXspCodeTable,
    pub codes: CodeParameters,

    // Region 5
    pub integers: IntegerParameters,

    // Region 6
    pub dimensions: DimensionParameters,

    // Store permanently the ControlSequence and Token corresponding to \par.
    // See 334.
    pub par_cs: ControlSequence,
    pub par_token: Token,
    // Store permanently the ControlSequence to \write.
    // See 1344.
    pub write_cs: ControlSequence,

    // See 286.
    mag_set: i32,

    /// fmtには保存しない、一回のrunだけが所有する時刻。
    run_date_time: Option<RunDateTime>,

    // See 549.
    pub fonts: Vec<FontInfo>,
    pub(crate) japanese_fonts: Vec<JapaneseFontInfo>,

    /// See 646.
    pub last_badness: i32,

    /// See 592.
    pub dead_cycles: i32,
    /// 989.
    pub output_active: bool,

    /// See 982.
    pub insert_penalties: i32,

    pub page_dims: PageDimensions,
    pub page_contents: PageContents,
    pub last_node_on_page: LastNodeInfo,
    /// `\lastnodetype` 用に、page builder が最後に調べた node の型も控える。
    pub last_node_type_on_page: i32,
    pub marks: Marks,

    // The following members are copies of internal variables that need to be accessible when
    // scanning for internal values. We copy that we can passing the corresponding data structures
    // around. They need to be kept in sync with the originals.
    pub line_number: usize,
    pub mode_type: Mode,
    pub prev_graf: i32,
    pub prev_depth: Dimension,
    pub space_factor: u16,
    pub last_node_info: LastNodeInfo,
}

impl Eqtb {
    pub fn new() -> Self {
        Self {
            cur_level: 0,
            cur_group: Group {
                typ: GroupType::BottomLevel,
                saved_definitions: Vec::new(),
                after_tokens: Vec::new(),
            },
            save_stack: Vec::new(),
            max_save_stack: 0,
            variable_levels: VariableLevels::new(),

            control_sequences: ControlSequenceStore::new(),
            skips: SkipParameters::new(),
            par_shape: ParShapeParameter::new(),
            penalty_arrays: PenaltyArrayParameters::new(),
            language_region: LanguageRegion::default(),
            using_namespaces: Vec::new(),
            last_node_type: -1,
            token_lists: TokenListParameters::new(),
            raw_strings: RawStringRegisters::new(),
            boxes: BoxParameters::new(),
            font_params: FontParameters::new(),
            cur_japanese_font: None,
            cat_codes: CatCodes::new(),
            kcat_codes: KCatCodes::new(),
            auto_spacing: AutoSpacingState::ENABLED,
            xsp_codes: XspCodeTable::ptex_initex(),
            inhibit_xsp_codes: InhibitXspCodeTable::default(),
            codes: CodeParameters::new(),
            integers: IntegerParameters::new(),
            dimensions: DimensionParameters::new(),
            par_cs: ControlSequence::Undefined,
            write_cs: ControlSequence::Undefined,
            par_token: Token::CSToken {
                cs: ControlSequence::Undefined,
            },
            mag_set: 0,
            run_date_time: None,
            fonts: vec![FontInfo::null_font()],
            japanese_fonts: Vec::new(),
            last_badness: 0,
            dead_cycles: 0,
            output_active: false,
            insert_penalties: 0,
            page_dims: PageDimensions::new(),
            page_contents: PageContents::Empty,
            last_node_on_page: LastNodeInfo::Other,
            last_node_type_on_page: -1,
            marks: Marks::new(),
            line_number: 0,
            mode_type: Mode::Vertical,
            prev_graf: 0,
            prev_depth: IGNORE_DEPTH,
            space_factor: 0,
            last_node_info: LastNodeInfo::Other,
        }
    }

    /// Store the primitives in the ControlSequenceStore and store the addresses
    /// of \par and \write internally.
    /// See 1336.
    pub fn init_prim(&mut self) {
        let permanent_adresses = self.put_primitives_into_hash_table();
        self.par_cs = permanent_adresses.par_cs;
        self.write_cs = permanent_adresses.write_cs;
        self.par_token = Token::CSToken { cs: self.par_cs };
    }

    /// run開始時に一度だけ得た時刻をTeXの整数parameterへ写す。
    ///
    /// parameterは文書から代入できるため、出力metadata用の不変snapshotも別に保つ。
    /// See 241.
    pub(crate) fn fix_date_and_time(&mut self, run_date_time: RunDateTime) {
        self.integers
            .set(IntegerVariable::Time, run_date_time.tex_time());
        self.integers
            .set(IntegerVariable::Day, run_date_time.day());
        self.integers
            .set(IntegerVariable::Month, run_date_time.month());
        self.integers
            .set(IntegerVariable::Year, run_date_time.year());
        self.run_date_time = Some(run_date_time);
    }

    pub(crate) fn run_date_time(&self) -> RunDateTime {
        self.run_date_time
            .expect("run date and time must be fixed before TeX input starts")
    }

    /// See 236.
    pub fn end_line_char(&self) -> i32 {
        self.integer(IntegerVariable::EndLineChar)
    }

    /// See 360.
    pub fn end_line_char_inactive(&self) -> bool {
        self.end_line_char() < 0 || self.end_line_char() > 255
    }

    /// See 236.
    pub fn error_context_lines(&self) -> i32 {
        self.integer(IntegerVariable::ErrorContextLines)
    }

    /// See 236.
    pub fn pausing(&self) -> i32 {
        self.integer(IntegerVariable::Pausing)
    }

    pub fn tracing_online(&self) -> i32 {
        self.integer(IntegerVariable::TracingOnline)
    }

    pub fn tracing_output(&self) -> i32 {
        self.integer(IntegerVariable::TracingOutput)
    }

    pub fn tracing_paragraphs(&self) -> i32 {
        self.integer(IntegerVariable::TracingParagraphs)
    }

    pub fn tracing_restores(&self) -> i32 {
        self.integer(IntegerVariable::TracingRestores)
    }

    // We need to avoid the keyword `box`
    pub fn boks(&self, n: RegisterIndex) -> &Option<ListNode> {
        self.boxes.get(BoxVariable(n))
    }

    pub fn cur_font(&self) -> FontIndex {
        *self.font_params.get(FontVariable::CurFont)
    }

    pub(crate) const fn cur_japanese_font(&self) -> Option<JapaneseFontIndex> {
        self.cur_japanese_font
    }

    pub(crate) fn current_japanese_font_info(&self) -> Option<&JapaneseFontInfo> {
        self.cur_japanese_font
            .and_then(|index| self.japanese_fonts.get(index.position()))
    }

    pub(crate) fn zw_for_cur_japanese_font(&self) -> Option<Dimension> {
        self.current_japanese_font_info().map(JapaneseFontInfo::zw)
    }

    pub(crate) fn zh_for_cur_japanese_font(&self) -> Option<Dimension> {
        self.current_japanese_font_info().map(JapaneseFontInfo::zh)
    }

    pub fn fam_fnt(&self, size: MathFontSize, number: usize) -> FontIndex {
        let font_var = FontVariable::MathFont { size, number };
        *self.font_params.get(font_var)
    }

    /// See 558.
    pub fn em_width_for_cur_font(&self) -> Dimension {
        self.fonts[self.cur_font() as usize].quad()
    }

    /// See 559.
    pub fn x_height_for_cur_font(&self) -> Dimension {
        self.fonts[self.cur_font() as usize].x_height()
    }

    pub fn cat_code(&self, chr: u8) -> CatCode {
        *self.cat_codes.get(chr)
    }

    /// upTeX `latin_ucs` の欧文 catcode 表を引く。
    ///
    /// ASCII/8 bit の呼出し口は上の `u8` 専用関数のまま保ち、Unicode
    /// decoderを通った場合だけこの道へ入る。
    pub(crate) fn latin_ucs_cat_code(&self, code_point: u32) -> CatCode {
        self.cat_codes.get_latin_ucs(code_point)
    }

    pub(crate) fn kcat_code(&self, code_point: u32) -> KCatCode {
        self.kcat_codes.get(code_point)
    }

    pub(crate) const fn auto_spacing_state(&self) -> AutoSpacingState {
        self.auto_spacing
    }

    pub(crate) fn xsp_code(&self, character: u8) -> XspCode {
        self.xsp_codes.get(
            LayoutCharacterCode::from_public_integer(u32::from(character))
                .expect("an eight-bit character is a Unicode scalar"),
        )
    }

    pub(crate) const fn xsp_codes(&self) -> &XspCodeTable {
        &self.xsp_codes
    }

    pub(crate) fn inhibit_xsp_code(&self, character: LayoutCharacterCode) -> XspCode {
        self.inhibit_xsp_codes.get(character)
    }

    pub(crate) const fn inhibit_xsp_codes(&self) -> &InhibitXspCodeTable {
        &self.inhibit_xsp_codes
    }

    pub fn lc_code(&self, c: usize) -> i32 {
        *self.codes.get(CodeVariable::LcCode(c))
    }

    pub fn uc_code(&self, c: usize) -> i32 {
        *self.codes.get(CodeVariable::UcCode(c))
    }

    pub fn sf_code(&self, c: usize) -> i32 {
        *self.codes.get(CodeVariable::SfCode(c))
    }

    pub fn math_code(&self, c: usize) -> i32 {
        *self.codes.get(CodeVariable::MathCode(c))
    }

    pub fn del_code(&self, n: usize) -> i32 {
        *self.codes.get(CodeVariable::DelCode(n))
    }

    pub fn left_skip(&self) -> &Rc<GlueSpec> {
        self.skips.get(SkipVariable::LeftSkip)
    }

    pub fn right_skip(&self) -> &Rc<GlueSpec> {
        self.skips.get(SkipVariable::RightSkip)
    }

    pub fn integer(&self, int_var: IntegerVariable) -> Integer {
        *self.integers.get(int_var)
    }

    pub fn language_region(&self) -> LanguageRegion {
        self.language_region
    }

    pub fn dimen(&self, dim_var: DimensionVariable) -> Dimension {
        *self.dimensions.get(dim_var)
    }

    /// See 243.
    pub fn get_current_escape_character(&self) -> Option<u8> {
        let c = self.integer(IntegerVariable::EscapeChar);
        if c >= 0 && c < 256 {
            Some(c as u8)
        } else {
            None
        }
    }

    /// See 244.
    pub fn get_current_newline_character(&self) -> Option<u8> {
        let c = self.integer(IntegerVariable::NewLineChar);
        if c >= 0 && c < 256 {
            Some(c as u8)
        } else {
            None
        }
    }

    /// The current input line.
    pub fn line_number(&self) -> usize {
        self.line_number
    }

    /// The current mode.
    pub fn mode(&self) -> Mode {
        self.mode_type
    }

    /// A `ControlSequence` version of `define`.
    /// See 1214., 277., and 279.
    pub fn cs_define(&mut self, cs: ControlSequence, command: Command, global: bool) {
        self.define(Definition::ControlSequence(cs, command), global);
    }

    /// A skip version of `define`.
    /// See 1214., 277., and 279.
    pub fn skip_define(&mut self, skip_var: SkipVariable, value: Skip, global: bool) {
        self.define(Definition::Skip(skip_var, value), global);
    }

    /// A paragraph shape version of `define`.
    /// See 1214., 277., and 279.
    /// **参照時に探す名前空間の一覧を差し替える。** 保存スタックを通す
    pub fn using_namespaces_define(&mut self, value: Vec<NamespaceId>, global: bool) {
        self.define(Definition::UsingNamespaces(value), global);
    }

    pub fn par_shape_define(&mut self, value: ParagraphShape, global: bool) {
        self.define(Definition::ParShape(value), global);
    }

    pub fn penalty_array_define(
        &mut self,
        variable: PenaltyArrayVariable,
        value: PenaltyArray,
        global: bool,
    ) {
        self.define(Definition::PenaltyArray(variable, value), global);
    }

    pub fn penalty_array_query(&self, variable: PenaltyArrayVariable, index: i32) -> i32 {
        self.penalty_arrays.query(variable, index)
    }

    pub fn penalty_array_value(
        &self,
        variable: PenaltyArrayVariable,
        index: usize,
    ) -> Option<i32> {
        self.penalty_arrays.value_at(variable, index)
    }

    pub fn language_region_define(&mut self, value: LanguageRegion, global: bool) {
        self.define(Definition::LanguageRegion(value), global);
    }

    /// A token list version of `define`.
    /// See 1214., 277., and 279.
    pub fn token_list_define(
        &mut self,
        tok_list_var: TokenListVariable,
        value: Option<RcTokenList>,
        global: bool,
    ) {
        self.define(Definition::TokenList(tok_list_var, value), global);
    }

    pub(crate) fn raw_string(&self, variable: RawStringVariable) -> &RcRawString {
        self.raw_strings.get(variable)
    }

    /// 生文字列を既存のsave stackへ載せる。容量違反時は現在値を一切変えない。
    pub(crate) fn raw_string_define(
        &mut self,
        variable: RawStringVariable,
        value: RcRawString,
        global: bool,
    ) -> Result<(), RawStringStorageError> {
        let (other_restore_bytes, existing_target_restore_len) =
            self.raw_string_restore_budget_excluding(variable)?;
        let previous_level = self.variable_levels.get(Variable::RawString(variable));
        let target_restore_len = if global {
            // level 0にするので、同じslotの古いsave entryはすべてretainedになる。
            0
        } else if previous_level != self.cur_level {
            // このlevelで初めての局所代入は現在値もsaveする。
            existing_target_restore_len.max(self.raw_string(variable).len())
        } else {
            // 同じlevelでの再代入は現在値を新しくsaveしない。
            existing_target_restore_len
        };
        self.raw_strings.can_set_with_restore_budget(
            variable,
            &value,
            other_restore_bytes,
            target_restore_len,
        )?;
        self.define(Definition::RawString(variable, value), global);
        Ok(())
    }

    /// 対象外slotのrestore余白の和と、対象slot自身の最大restore長を返す。
    ///
    /// save entryを単純に全加算すると、途中のglobal定義でlevel 0になり実際には捨てられる
    /// 古いentryまで予約してしまう。`unsave`と同じ内側→外側、各groupの逆順でlevelを
    /// simulationし、実際に復元され得る値だけを数える。
    fn raw_string_restore_budget_excluding(
        &self,
        target: RawStringVariable,
    ) -> Result<(usize, usize), RawStringStorageError> {
        #[derive(Clone, Copy)]
        struct RestoreState {
            simulated_level: Level,
            maximum_len: usize,
        }

        let mut states = HashMap::<RawStringVariable, RestoreState>::new();
        for group in std::iter::once(&self.cur_group).chain(self.save_stack.iter().rev()) {
            for entry in group.saved_definitions.iter().rev() {
                let Definition::RawString(variable, ref value) = entry.definition else {
                    continue;
                };
                let state = states.entry(variable).or_insert_with(|| RestoreState {
                    simulated_level: self.variable_levels.get(Variable::RawString(variable)),
                    maximum_len: 0,
                });
                if state.simulated_level != 0 {
                    state.maximum_len = state.maximum_len.max(value.len());
                    state.simulated_level = entry.level;
                }
            }
        }

        let target_restore_len = states
            .get(&target)
            .map_or(0, |state| state.maximum_len);
        let mut other_restore_bytes = 0_usize;
        for (variable, state) in states {
            if variable == target {
                continue;
            }
            let extra = state
                .maximum_len
                .saturating_sub(self.raw_string(variable).len());
            other_restore_bytes = other_restore_bytes
                .checked_add(extra)
                .ok_or(RawStringStorageError::StorageTooLarge)?;
        }
        Ok((other_restore_bytes, target_restore_len))
    }

    /// A box version of `define`.
    /// See 1214., 277., and 279.
    pub fn box_define(&mut self, register: RegisterIndex, value: Option<ListNode>, global: bool) {
        self.define(
            Definition::BoxRegister(BoxVariable(register), value),
            global,
        );
    }

    /// A font version of `define`.
    /// See 1214., 277., and 279.
    pub fn font_define(&mut self, font_var: FontVariable, font_index: FontIndex, global: bool) {
        self.define(Definition::Font(font_var, font_index), global);
    }

    pub(crate) fn japanese_font_define(
        &mut self,
        font_index: Option<JapaneseFontIndex>,
        global: bool,
    ) {
        self.define(Definition::JapaneseFont(font_index), global);
    }

    /// A category code version of `define`.
    /// See 1214., 277., and 279.
    pub fn cat_code_define(&mut self, chr: u8, cat_code: CatCode, global: bool) {
        self.define(Definition::CatCode(chr as usize, cat_code), global);
    }

    pub(crate) fn latin_ucs_cat_code_define(
        &mut self,
        code_point: u16,
        cat_code: CatCode,
        global: bool,
    ) {
        self.define(Definition::CatCode(code_point as usize, cat_code), global);
    }

    /// upTeX の和文カテゴリーを Unicode block 単位で定義する。
    pub(crate) fn kcat_code_define(&mut self, code_point: u32, kcat_code: KCatCode, global: bool) {
        let block = KCatCodes::block_for(code_point);
        self.define(Definition::KCatCode(block, kcat_code), global);
    }

    pub(crate) fn auto_spacing_define(
        &mut self,
        variable: AutoSpacingVariable,
        enabled: bool,
        global: bool,
    ) {
        if self.auto_spacing.get(variable) != enabled || global {
            self.define(Definition::AutoSpacing(variable, enabled), global);
        }
    }

    pub(crate) fn xsp_code_define(&mut self, character: u8, value: XspCode, global: bool) {
        if self.xsp_code(character) != value || global {
            self.define(Definition::XspCode(character, value), global);
        }
    }

    pub(crate) fn inhibit_xsp_code_define(
        &mut self,
        character: LayoutCharacterCode,
        value: XspCode,
        global: bool,
    ) -> Result<(), SpacingStateError> {
        if self.inhibit_xsp_code(character) == value && !global {
            return Ok(());
        }
        let other_restore_reservations =
            self.inhibit_xsp_restore_reservations_excluding(character);
        if !self
            .inhibit_xsp_codes
            .can_set(character, value, other_restore_reservations)
        {
            return Err(SpacingStateError::InhibitXspCodeTableFull);
        }
        self.define(Definition::InhibitXspCode(character, value), global);
        Ok(())
    }

    /// 局所的に既定値3へ戻されているentryも、group終了時には一枠を必要とする。
    ///
    /// この復元義務を現在の疎表長とは別に数え、group内のglobal追加が予約済み枠を
    /// 消費しないようにする。同じ文字の入れ子保存は一枠だけを予約する。
    fn inhibit_xsp_restore_reservations_excluding(
        &self,
        target: LayoutCharacterCode,
    ) -> usize {
        let mut characters = BTreeSet::new();
        for group in self
            .save_stack
            .iter()
            .chain(std::iter::once(&self.cur_group))
        {
            for entry in &group.saved_definitions {
                let Definition::InhibitXspCode(character, old_value) = entry.definition else {
                    continue;
                };
                let variable = Variable::InhibitXspCode(character);
                if old_value != XspCode::BOTH
                    && self.variable_levels.get(variable) != 0
                    && self.inhibit_xsp_code(character) == XspCode::BOTH
                {
                    if character != target {
                        characters.insert(character);
                    }
                }
            }
        }
        characters.len()
    }

    /// A code version of `define`.
    /// See 1214., 277., and 279.
    pub fn code_define(&mut self, code_var: CodeVariable, value: i32, global: bool) {
        self.define(Definition::Code(code_var, value), global);
    }

    /// An integer version of `word_define`.
    /// See 1214., 278., and 279.
    pub fn int_define(
        &mut self,
        int_var: IntegerVariable,
        value: i32,
        global: bool,
        logger: &mut Logger,
    ) {
        // We update the Logger's copies of escapechar and newlinechar here.
        if let IntegerVariable::EscapeChar = int_var {
            logger.escape_char = match u8::try_from(value) {
                Ok(c) => Some(c),
                Err(_) => None,
            };
        } else if let IntegerVariable::NewLineChar = int_var {
            logger.newline_char = match u8::try_from(value) {
                Ok(c) => Some(c),
                Err(_) => None,
            };
        }

        // At the bottom level, local and global definitions have identical
        // semantics: there is no enclosing value to save and every variable
        // level is already zero.  Integer arithmetic is frequent enough that
        // avoiding the Definition/Variable dispatch here matters.  See 278.
        if self.cur_level == 0 {
            debug_assert_eq!(
                self.variable_levels.get(Variable::Integer(int_var)),
                0,
                "an integer at the bottom level must have level zero"
            );
            self.integers.set(int_var, value);
            return;
        }

        self.define(Definition::Integer(int_var, value), global);
    }

    /// A dimen version of `word_define`.
    /// See 1214., 278., and 279.
    pub fn dimen_define(&mut self, dimen_var: DimensionVariable, value: i32, global: bool) {
        self.define(Definition::Dimen(dimen_var, value), global);
    }

    /// See 1214., 278., and 279.
    fn define(&mut self, definition: Definition, global: bool) {
        let variable = definition.to_variable();
        if global {
            self.apply_definition(definition);
            self.variable_levels.set(variable, 0);
        } else {
            let prev_definition = self.apply_definition(definition);
            let prev_level = self.variable_levels.set(variable, self.cur_level);
            if prev_level != self.cur_level {
                self.cur_group.saved_definitions.push(SaveEntry {
                    definition: prev_definition,
                    level: prev_level,
                });
            }
        }
    }

    fn apply_definition(&mut self, definition: Definition) -> Definition {
        match definition {
            Definition::ControlSequence(control_sequence, command) => {
                let prev_command = self.control_sequences.set(control_sequence, command);
                Definition::ControlSequence(control_sequence, prev_command)
            }
            Definition::Skip(skip_variable, skip) => {
                let prev_skip = self.skips.set(skip_variable, skip);
                Definition::Skip(skip_variable, prev_skip)
            }
            Definition::ParShape(paragraph_shape) => {
                let prev_paragraph_shape = self.par_shape.set(ParShapeVariable, paragraph_shape);
                Definition::ParShape(prev_paragraph_shape)
            }
            Definition::PenaltyArray(variable, value) => {
                let previous = self.penalty_arrays.set(variable, value);
                Definition::PenaltyArray(variable, previous)
            }
            Definition::LanguageRegion(language_region) => {
                let previous = std::mem::replace(&mut self.language_region, language_region);
                Definition::LanguageRegion(previous)
            }
            Definition::UsingNamespaces(list) => {
                let prev = std::mem::replace(&mut self.using_namespaces, list);
                Definition::UsingNamespaces(prev)
            }
            Definition::TokenList(token_list_variable, token_list) => {
                let prev_token_list = self.token_lists.set(token_list_variable, token_list);
                Definition::TokenList(token_list_variable, prev_token_list)
            }
            Definition::RawString(variable, value) => {
                let previous = self.raw_strings.replace_reserved(variable, value);
                Definition::RawString(variable, previous)
            }
            Definition::BoxRegister(box_variable, list_node) => {
                let prev_list_node = self.boxes.set(box_variable, list_node);
                Definition::BoxRegister(box_variable, prev_list_node)
            }
            Definition::Font(font_variable, font_index) => {
                let prev_font_index = self.font_params.set(font_variable, font_index);
                Definition::Font(font_variable, prev_font_index)
            }
            Definition::JapaneseFont(font_index) => {
                let previous = std::mem::replace(&mut self.cur_japanese_font, font_index);
                Definition::JapaneseFont(previous)
            }
            Definition::CatCode(chr, cat_code) => {
                let prev_cat_code = self.cat_codes.set_latin_ucs(chr as u16, cat_code);
                Definition::CatCode(chr, prev_cat_code)
            }
            Definition::KCatCode(block, kcat_code) => {
                let previous = self.kcat_codes.set_block(block, kcat_code);
                Definition::KCatCode(block, previous)
            }
            Definition::AutoSpacing(variable, enabled) => {
                let previous = self.auto_spacing.set(variable, enabled);
                Definition::AutoSpacing(variable, previous)
            }
            Definition::XspCode(character, value) => {
                let previous = self
                    .xsp_codes
                    .set(u32::from(character), value)
                    .expect("an eight-bit xspcode index is valid");
                Definition::XspCode(character, previous)
            }
            Definition::InhibitXspCode(character, value) => {
                let previous = self
                    .inhibit_xsp_codes
                    .set(character, value)
                    .expect("inhibitxspcode capacity was checked before definition");
                Definition::InhibitXspCode(character, previous)
            }
            Definition::Code(code_variable, val) => {
                let prev_val = self.codes.set(code_variable, val);
                Definition::Code(code_variable, prev_val)
            }
            Definition::Integer(integer_variable, val) => {
                let prev_val = self.integers.set(integer_variable, val);
                Definition::Integer(integer_variable, prev_val)
            }
            Definition::Dimen(dimension_variable, dimension) => {
                let prev_dimension = self.dimensions.set(dimension_variable, dimension);
                Definition::Dimen(dimension_variable, prev_dimension)
            }
        }
    }

    /// See 284.
    #[cfg(feature = "stats")]
    fn restore_trace(&self, variable: Variable, s: &str, tracing_online: i32, logger: &mut Logger) {
        logger.begin_diagnostic(tracing_online);
        logger.print_char(b'{');
        logger.print_str(s);
        logger.print_char(b' ');
        self.show_equivalent_of_variable(variable, logger);
        logger.print_char(b'}');
        logger.end_diagnostic(false);
    }

    /// See 274.
    pub fn new_save_level(&mut self, c: GroupType, input_stack: &InputStack, logger: &mut Logger) {
        if self.cur_level == MAX_GROUPING_DEPTH {
            overflow(
                "grouping levels",
                MAX_GROUPING_DEPTH,
                input_stack,
                self,
                logger,
            );
        }
        let new_group = Group {
            typ: c,
            saved_definitions: Vec::new(),
            after_tokens: Vec::new(),
        };
        let saved_group = std::mem::replace(&mut self.cur_group, new_group);
        self.save_stack.push(saved_group);
        self.cur_level += 1;
    }

    /// See 280.
    pub fn save_for_after(&mut self, token: Token) {
        if self.cur_level > 0 {
            self.cur_group.after_tokens.push(token);
        }
    }

    /// Closes the current save level, restores definitions where appropriate and inserts
    /// AfterTokens.
    /// See 281. and 282.
    pub fn unsave(&mut self, scanner: &mut Scanner, logger: &mut Logger) {
        if self.cur_level > 0 {
            self.cur_level -= 1;
            let prev_group = self
                .save_stack
                .pop()
                .expect("Save stack should not be empty here");
            let finished_group = std::mem::replace(&mut self.cur_group, prev_group);

            for entry in finished_group.saved_definitions.into_iter().rev() {
                self.restore_or_discard_saved_definition(entry.definition, entry.level, logger);
            }
            for token in finished_group.after_tokens.into_iter().rev() {
                scanner.insert_token_into_input(token, self, logger);
            }
        } else {
            panic!("Unsave should never be called if we are at base level");
        }
    }

    /// See 283.
    fn restore_or_discard_saved_definition(
        &mut self,
        definition: Definition,
        level: Level,
        logger: &mut Logger,
    ) {
        let variable = definition.to_variable();
        if let Definition::Integer(int_var, value) = definition {
            // We update the Logger's copies of escapechar and newlinechar here.
            if let IntegerVariable::EscapeChar = int_var {
                let cur_level = self.variable_levels.get(variable);
                // If the value will be restored.
                if cur_level != 0 {
                    logger.escape_char = match u8::try_from(value) {
                        Ok(c) => Some(c),
                        Err(_) => None,
                    };
                }
            } else if let IntegerVariable::NewLineChar = int_var {
                let cur_level = self.variable_levels.get(variable);
                // If the value will be restored.
                if cur_level != 0 {
                    logger.newline_char = match u8::try_from(value) {
                        Ok(c) => Some(c),
                        Err(_) => None,
                    };
                }
            }
        }
        let restoring = self.unsave_definition(definition, level);
        if cfg!(feature = "stats") {
            if self.tracing_restores() > 0 {
                if restoring {
                    self.restore_trace(variable, "restoring", self.tracing_online(), logger);
                } else {
                    self.restore_trace(variable, "retaining", self.tracing_online(), logger);
                }
            }
        }
    }

    /// See 283.
    fn unsave_definition(&mut self, definition: Definition, level: Level) -> bool {
        let variable = definition.to_variable();
        let cur_level = self.variable_levels.get(variable);

        if cur_level != 0 {
            self.apply_definition(definition);
            self.variable_levels.set(variable, level);
            true
        } else {
            false
        }
    }

    /// See 288.
    pub fn prepare_mag(&mut self, scanner: &mut Scanner, logger: &mut Logger) {
        let mag = self.integer(IntegerVariable::Mag);
        if self.mag_set > 0 && mag != self.mag_set {
            logger.print_err("Incompatible magnification (");
            logger.print_int(mag);
            logger.print_str(");");
            logger.print_nl_str(" the previous value will be retained");
            let help = &[
                "I can handle only one magnification ratio per job. So I've",
                "reverted to the magnification you used earlier on this run.",
            ];
            logger.int_error(self.mag_set, help, scanner, self);
            self.int_define(IntegerVariable::Mag, self.mag_set, true, logger);
        }
        if mag <= 0 || mag > 32768 {
            logger.print_err("Illegal magnification has been changed to 1000");
            let help = &["The magnification ratio must be between 1 and 32768."];
            logger.int_error(mag, help, scanner, self);
            self.int_define(IntegerVariable::Mag, 1000, true, logger);
        }
        self.mag_set = mag;
    }

    /// See 252.
    #[cfg(feature = "stats")]
    fn show_equivalent_of_variable(&self, variable: Variable, logger: &mut Logger) {
        match variable {
            Variable::ControlSequence(control_sequence) => {
                self.show_equivalent_of_control_sequence(control_sequence, logger)
            }
            Variable::Skip(skip_var) => self.show_equivalent_of_skip_variable(skip_var, logger),
            Variable::ParShape => self.show_equivalent_of_par_shape(logger),
            Variable::PenaltyArray(variable) => {
                logger.print_esc_str(variable.primitive_name());
                logger.print_char(b'=');
                logger.print_int(self.penalty_array_query(variable, 0));
            }
            Variable::LanguageRegion => {
                logger.print_esc_str(b"pratexregion");
                logger.print_char(b'=');
                logger.print_int(i32::from(self.language_region.code()));
            }
            Variable::UsingNamespaces => self.show_using_namespaces(logger),
            Variable::TokenList(token_list_var) => {
                self.show_equivalent_of_token_list_variable(token_list_var, logger)
            }
            // 内容を表示・字句化しない。raw値の診断は`\showthe`だけが担う。
            Variable::RawString(variable) => logger.print_esc_str(&variable.to_string()),
            Variable::BoxRegister(box_var) => self.show_equivalent_of_box_variable(box_var, logger),
            Variable::Font(font_var) => self.show_equivalent_of_font_variable(font_var, logger),
            Variable::JapaneseFont => {
                logger.print_str("current Japanese font=");
                if let Some(index) = self.cur_japanese_font {
                    logger.print_int(index.position() as i32);
                } else {
                    logger.print_str("none");
                }
            }
            Variable::CatCode(chr) => self.show_equivalent_of_cat_code_variable(chr, logger),
            Variable::KCatCode(block) => {
                logger.print_esc_str(b"kcatcode");
                logger.print_str(" block ");
                logger.print_int(block.index() as i32);
                logger.print_char(b'=');
                logger.print_int(self.kcat_codes.get_block(block).public_number());
            }
            Variable::AutoSpacing(variable) => {
                logger.print_esc_str(match variable {
                    AutoSpacingVariable::Kanji => b"autospacing",
                    AutoSpacingVariable::XKanji => b"autoxspacing",
                });
                logger.print_char(b'=');
                logger.print_int(i32::from(self.auto_spacing.get(variable)));
            }
            Variable::XspCode(character) => {
                logger.print_esc_str(b"xspcode");
                logger.print_int(i32::from(character));
                logger.print_char(b'=');
                logger.print_int(self.xsp_code(character).to_public_integer());
            }
            Variable::InhibitXspCode(character) => {
                logger.print_esc_str(b"inhibitxspcode");
                logger.print_int(character.to_public_integer() as i32);
                logger.print_char(b'=');
                logger.print_int(self.inhibit_xsp_code(character).to_public_integer());
            }
            Variable::Code(code_var) => self.show_equivalent_of_code_variable(code_var, logger),
            Variable::Integer(int_var) => self.show_equivalent_of_integer_variable(int_var, logger),
            Variable::Dimen(dimen_var) => {
                self.show_equivalent_of_dimension_variable(dimen_var, logger)
            }
        }
    }

    /// See 223.
    fn show_equivalent_of_control_sequence(&self, cs: ControlSequence, logger: &mut Logger) {
        cs.sprint_cs(self, logger);
        logger.print_char(b'=');
        let command = self.control_sequences.get(cs);
        command.display(&self.fonts, logger);
        if let Command::Expandable(ExpandableCommand::Macro(MacroCall { macro_def, .. })) = command
        {
            logger.print_char(b':');
            show_macro_def(macro_def, 32, logger, self);
        }
        // NOTE We keep this for now for compatibility.
        if let Command::Expandable(ExpandableCommand::EndTemplate) = command {
            logger.print_char(b':');
        }
    }

    /// See 229.
    fn show_equivalent_of_skip_variable(&self, skip_var: SkipVariable, logger: &mut Logger) {
        logger.print_esc_str(&skip_var.to_string());
        logger.print_char(b'=');
        let glue_spec = self.skips.get(skip_var);
        if let SkipVariable::ThinMuSkip
        | SkipVariable::MedMuSkip
        | SkipVariable::ThickMuSkip
        | SkipVariable::MuSkip(_) = skip_var
        {
            glue_spec.print_spec(Some("mu"), logger);
        } else {
            glue_spec.print_spec(Some("pt"), logger);
        }
    }

    /// See 233.
    fn show_equivalent_of_par_shape(&self, logger: &mut Logger) {
        logger.print_esc_str(&ParShapeVariable.to_string());
        logger.print_char(b'=');
        let par_shape = self.par_shape.get(ParShapeVariable);
        logger.print_int(par_shape.len() as i32);
    }

    /// See 233.
    fn show_equivalent_of_token_list_variable(
        &self,
        tok_list_var: TokenListVariable,
        logger: &mut Logger,
    ) {
        logger.print_esc_str(&tok_list_var.to_string());
        logger.print_char(b'=');
        let list = self.token_lists.get(tok_list_var);
        if let Some(list) = list {
            show_token_list(list, 32, logger, self);
        }
    }

    /// See 233.
    fn show_equivalent_of_box_variable(&self, box_var: BoxVariable, logger: &mut Logger) {
        logger.print_esc_str(&box_var.to_string());
        logger.print_char(b'=');
        let node_list = self.boxes.get(box_var);
        match node_list {
            None => {
                logger.print_str("void");
            }
            Some(list_node) => {
                let depth_max = 0;
                let breadth_max = 1;
                show_list_node(list_node, b"", depth_max, breadth_max, self, logger);
            }
        }
    }

    /// See 233. and 234.
    fn show_equivalent_of_font_variable(&self, font_var: FontVariable, logger: &mut Logger) {
        // The current font should print only "current font" without
        // the escape character.
        match font_var {
            FontVariable::CurFont => logger.print_str("current font"),
            FontVariable::MathFont { size, number } => {
                logger.print_esc_str(format!("{}{}", size.as_str(), number).as_bytes())
            }
        }
        logger.print_char(b'=');
        let f = *self.font_params.get(font_var);
        logger.print_esc_str(self.control_sequences.text(ControlSequence::FontId(f)));
    }

    /// See 236.
    pub fn get_max_depth_and_breadth(&self) -> (i32, usize) {
        let max_depth = self.integer(IntegerVariable::ShowBoxDepth);
        let show_box_breadth = self.integer(IntegerVariable::ShowBoxBreadth);
        let max_breadth = if show_box_breadth <= 0 {
            5
        } else {
            show_box_breadth as usize
        };
        (max_depth, max_breadth)
    }

    /// See 242.
    fn show_equivalent_of_integer_variable(&self, int_var: IntegerVariable, logger: &mut Logger) {
        logger.print_esc_str(&int_var.to_string());
        logger.print_char(b'=');
        let int_val = *self.integers.get(int_var);
        logger.print_int(int_val);
    }

    /// See 233., 235. and 242.
    fn show_equivalent_of_cat_code_variable(&self, chr: usize, logger: &mut Logger) {
        let name = format!("catcode{}", chr).as_bytes().to_vec();
        logger.print_esc_str(&name);
        logger.print_char(b'=');
        let cat_code = self.cat_codes.get_latin_ucs(chr as u32);
        logger.print_int(cat_code.public_number());
    }

    /// See 233., 235. and 242.
    fn show_equivalent_of_code_variable(&self, code_var: CodeVariable, logger: &mut Logger) {
        logger.print_esc_str(&code_var.to_string());
        logger.print_char(b'=');
        let code_val = *self.codes.get(code_var);
        logger.print_int(code_val);
    }

    /// See 251.
    fn show_equivalent_of_dimension_variable(
        &self,
        dimen_var: DimensionVariable,
        logger: &mut Logger,
    ) {
        logger.print_esc_str(&dimen_var.to_string());
        logger.print_char(b'=');
        logger.print_scaled(*self.dimensions.get(dimen_var));
        logger.print_str("pt");
    }

    /// See 700.
    fn mathsy(&self, size: MathFontSize, param: usize) -> Dimension {
        // We remove the 1-indexing here.
        self.fonts[self.fam_fnt(size, 2) as usize].params[param - 1]
    }

    /// See 700.
    pub fn math_x_height(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 5)
    }

    /// See 700.
    pub fn math_quad(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 6)
    }

    /// See 700.
    pub fn num1(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 8)
    }

    /// See 700.
    pub fn num2(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 9)
    }

    /// See 700.
    pub fn num3(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 10)
    }

    /// See 700.
    pub fn denom1(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 11)
    }

    /// See 700.
    pub fn denom2(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 12)
    }

    /// See 700.
    pub fn sup1(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 13)
    }

    /// See 700.
    pub fn sup2(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 14)
    }

    /// See 700.
    pub fn sup3(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 15)
    }

    /// See 700.
    pub fn sub1(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 16)
    }

    /// See 700.
    pub fn sub2(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 17)
    }

    /// See 700.
    pub fn sup_drop(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 18)
    }

    /// See 700.
    pub fn sub_drop(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 19)
    }

    /// See 700.
    pub fn delim1(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 20)
    }

    /// See 700.
    pub fn delim2(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 21)
    }

    /// See 700.
    pub fn axis_height(&self, size: MathFontSize) -> Dimension {
        self.mathsy(size, 22)
    }

    /// See 701.
    pub fn mathex(&self, size: MathFontSize, param: usize) -> Dimension {
        // We remove the 1-indexing here.
        self.fonts[self.fam_fnt(size, 3) as usize].params[param - 1]
    }

    /// See 701.
    pub fn default_rule_thickness(&self, size: MathFontSize) -> Dimension {
        self.mathex(size, 8)
    }

    /// See 701.
    pub fn big_op_spacing1(&self, size: MathFontSize) -> Dimension {
        self.mathex(size, 9)
    }

    /// See 701.
    pub fn big_op_spacing2(&self, size: MathFontSize) -> Dimension {
        self.mathex(size, 10)
    }

    /// See 701.
    pub fn big_op_spacing3(&self, size: MathFontSize) -> Dimension {
        self.mathex(size, 11)
    }

    /// See 701.
    pub fn big_op_spacing4(&self, size: MathFontSize) -> Dimension {
        self.mathex(size, 12)
    }

    /// See 701.
    pub fn big_op_spacing5(&self, size: MathFontSize) -> Dimension {
        self.mathex(size, 13)
    }

    /// Gets the `ControlSequence` corresponding to the name or returns None.
    /// See 374.
    pub fn lookup(&self, name: &[u8]) -> Option<ControlSequence> {
        // **使っている名前空間があれば、そちらも探す**（`\usingnamespace`）。
        // 無ければ**素の TeX82 とまったく同じ道**を通る
        if let Some(cs) = self.search_using(name) {
            return Some(cs);
        }
        match name.len() {
            0 => Some(ControlSequence::NullCs),
            1 => Some(ControlSequence::Single(name[0])),
            _ => self
                .control_sequences
                .id_lookup(name)
                .map(ControlSequence::Escaped),
        }
    }

    /// Unicode単位を含む制御綴を引く。
    ///
    /// byte名とは別のhashを使うため、表示bytesが同じでも別identityになる。
    pub fn lookup_wide(&self, name: &[ControlSequenceNameUnit]) -> Option<ControlSequence> {
        if let Some(cs) = self.search_using_wide(false, name) {
            return Some(cs);
        }
        self.control_sequences
            .id_lookup_wide(name)
            .map(ControlSequence::Escaped)
    }

    /// Unicode活性文字を引く。通常の一文字制御記号とは別identity。
    pub fn lookup_wide_active(&self, code_point: u32) -> Option<ControlSequence> {
        let name = [ControlSequenceNameUnit::Unicode(code_point)];
        if let Some(cs) = self.search_using_wide(true, &name) {
            return Some(cs);
        }
        self.control_sequences
            .id_lookup_wide_active(&name)
            .map(ControlSequence::Escaped)
    }

    /// 一文字の制御綴（`\x`）。**探索に参加する。**
    ///
    /// `to_token` は今まで `Single(c)` へ直に落としていたので、
    /// **一文字だけ探索から漏れていた。**
    pub fn lookup_symbol(&self, c: u8) -> ControlSequence {
        self.search_using_kind(false, &[c])
            .unwrap_or(ControlSequence::Single(c))
    }

    /// 活性文字（`~`）。同上。
    pub fn lookup_active(&self, c: u8) -> ControlSequence {
        self.search_using_kind(true, &[c])
            .unwrap_or(ControlSequence::Active(c))
    }

    /// 使っている名前空間を**追加順に**探す。
    ///
    /// **global が優先である。** global に定義があればそれを使い、
    /// 無ければ（＝未定義なら）名前空間へ降りる。
    ///
    /// > フォーマットを名前空間で上書きする用途は非目標である。
    ///
    /// 使っている名前空間が無ければ**何もしない**——この一行が
    /// 「使わない機能は費用を持たない」を守っている。
    fn search_using(&self, name: &[u8]) -> Option<ControlSequence> {
        self.search_using_kind(false, name)
    }

    fn search_using_wide(
        &self,
        active: bool,
        name: &[ControlSequenceNameUnit],
    ) -> Option<ControlSequence> {
        if self.using_namespaces.is_empty() {
            return None;
        }
        let global = if active {
            self.control_sequences.id_lookup_wide_active(name)
        } else {
            self.control_sequences.id_lookup_wide(name)
        };
        if let Some(n) = global {
            let cs = ControlSequence::Escaped(n);
            if self.is_defined(cs) {
                return Some(cs);
            }
        }
        for ns in &self.using_namespaces {
            if let Some(n) = self
                .control_sequences
                .id_lookup_ns_wide(Some(*ns), active, name)
            {
                let cs = ControlSequence::Escaped(n);
                if self.is_defined(cs) {
                    return Some(cs);
                }
            }
        }
        None
    }

    fn search_using_kind(&self, active: bool, name: &[u8]) -> Option<ControlSequence> {
        if self.using_namespaces.is_empty() || name.is_empty() {
            return None;
        }
        // global に**定義が**あればそれ。無い／未定義なら名前空間へ
        let global = if active {
            Some(ControlSequence::Active(name[0]))
        } else {
            match name.len() {
                1 => Some(ControlSequence::Single(name[0])),
                _ => self
                    .control_sequences
                    .id_lookup(name)
                    .map(ControlSequence::Escaped),
            }
        };
        if let Some(cs) = global {
            if self.is_defined(cs) {
                return Some(cs);
            }
        }
        for ns in &self.using_namespaces {
            if let Some(n) = self.control_sequences.id_lookup_ns(Some(*ns), active, name) {
                let cs = ControlSequence::Escaped(n);
                if self.is_defined(cs) {
                    return Some(cs);
                }
            }
        }
        None
    }

    fn is_defined(&self, cs: ControlSequence) -> bool {
        !matches!(
            self.control_sequences.get(cs),
            Command::Expandable(ExpandableCommand::Undefined)
        )
    }

    /// 名前空間つきで引く。`None` は global で、上と同じ。
    ///
    /// **一文字と空の短絡は名前空間版では行わない。**
    /// 一文字は対の鍵に入れねばならず、空は `NullCs` へ退化させる（決定事項）。
    pub fn lookup_ns(
        &self,
        ns: Option<crate::eqtb::NamespaceId>,
        name: &[u8],
    ) -> Option<ControlSequence> {
        self.lookup_ns_kind(ns, false, name)
    }

    /// Unicode単位を含む制御綴を名前空間つきで引く。`None` はglobal。
    pub fn lookup_ns_wide(
        &self,
        ns: Option<NamespaceId>,
        name: &[ControlSequenceNameUnit],
    ) -> Option<ControlSequence> {
        let Some(ns) = ns else {
            return self.lookup_wide(name);
        };
        self.control_sequences
            .id_lookup_ns_wide(Some(ns), false, name)
            .map(ControlSequence::Escaped)
    }

    pub fn lookup_ns_kind(
        &self,
        ns: Option<crate::eqtb::NamespaceId>,
        active: bool,
        name: &[u8],
    ) -> Option<ControlSequence> {
        let Some(ns) = ns else {
            return self.lookup(name);
        };
        if name.is_empty() {
            // **空は global へ落ちる。** これを統一規則とする
            return Some(ControlSequence::NullCs);
        }
        self.control_sequences
            .id_lookup_ns(Some(ns), active, name)
            .map(ControlSequence::Escaped)
    }

    /// 名前空間つきで引くか、無ければ作る。
    pub fn lookup_or_create_ns(
        &mut self,
        ns: Option<crate::eqtb::NamespaceId>,
        name: &[u8],
        active: Option<u8>,
    ) -> Result<ControlSequence, ()> {
        let Some(ns) = ns else {
            return self.lookup_or_create(name);
        };
        if name.is_empty() {
            return Ok(ControlSequence::NullCs);
        }
        if let Some(n) = self
            .control_sequences
            .id_lookup_ns(Some(ns), active.is_some(), name)
        {
            return Ok(ControlSequence::Escaped(n));
        }
        let n = self.control_sequences.add_command_ns(
            Some(ns),
            name,
            active,
            &mut self.variable_levels,
        )?;
        Ok(ControlSequence::Escaped(n))
    }

    /// Unicode単位を含む制御綴を名前空間つきで引くか、無ければ作る。
    pub fn lookup_or_create_ns_wide(
        &mut self,
        ns: Option<NamespaceId>,
        name: &[ControlSequenceNameUnit],
    ) -> Result<ControlSequence, ()> {
        let Some(ns) = ns else {
            return self.lookup_or_create_wide(name);
        };
        if let Some(n) = self
            .control_sequences
            .id_lookup_ns_wide(Some(ns), false, name)
        {
            return Ok(ControlSequence::Escaped(n));
        }
        let n = self.control_sequences.add_wide_command_ns(
            Some(ns),
            name,
            None,
            &mut self.variable_levels,
        )?;
        Ok(ControlSequence::Escaped(n))
    }

    /// Gets the `ControlSequence` corresponding to the name or creates a new
    /// one for this name.
    /// See 374.
    pub fn lookup_or_create(&mut self, name: &[u8]) -> Result<ControlSequence, ()> {
        // **探索は作る前に行う。** 見つかればそれを使い、global に穴を開けない
        if let Some(cs) = self.search_using(name) {
            return Ok(cs);
        }
        let cs = match name.len() {
            0 => ControlSequence::NullCs,
            1 => ControlSequence::Single(name[0]),
            _ => match self.control_sequences.id_lookup(name) {
                Some(n) => ControlSequence::Escaped(n),
                None => {
                    let n = self
                        .control_sequences
                        .add_command(name, &mut self.variable_levels)?;
                    ControlSequence::Escaped(n)
                }
            },
        };
        Ok(cs)
    }

    /// Unicode単位を含む制御綴を引くか、無ければglobalに作る。
    pub fn lookup_or_create_wide(
        &mut self,
        name: &[ControlSequenceNameUnit],
    ) -> Result<ControlSequence, ()> {
        if let Some(cs) = self.search_using_wide(false, name) {
            return Ok(cs);
        }
        let n = match self.control_sequences.id_lookup_wide(name) {
            Some(n) => n,
            None => self
                .control_sequences
                .add_wide_command(name, &mut self.variable_levels)?,
        };
        Ok(ControlSequence::Escaped(n))
    }

    /// Unicode活性文字を引くか、無ければglobalに作る。
    pub fn lookup_or_create_wide_active(
        &mut self,
        code_point: u32,
    ) -> Result<ControlSequence, ()> {
        if let Some(cs) = self.lookup_wide_active(code_point) {
            return Ok(cs);
        }
        let n = self
            .control_sequences
            .add_wide_active_command(code_point, &mut self.variable_levels)?;
        Ok(ControlSequence::Escaped(n))
    }

    /// See 264.
    pub fn primitive_unexpandable(
        &mut self,
        name: &[u8],
        unexpandable_command: UnexpandableCommand,
    ) -> ControlSequence {
        let cs = self
            .lookup_or_create(name)
            .expect("There should always be enough space for the primitives");
        let command = Command::Unexpandable(unexpandable_command);
        // There should not be a previous definition.
        self.control_sequences.set(cs, command);
        self.control_sequences.set_text(cs, name);

        cs
    }

    /// See 264.
    pub fn primitive_expandable(
        &mut self,
        name: &[u8],
        expandable_command: ExpandableCommand,
    ) -> ControlSequence {
        let cs = self
            .lookup_or_create(name)
            .expect("There should always be enough space for the primitives");
        let command = Command::Expandable(expandable_command);
        // There should not be a previous definition.
        self.control_sequences.set(cs, command);
        self.control_sequences.set_text(cs, name);

        cs
    }

    /// `\lastnodetype`（e-TeX）。**-1 は「無い」。**
    pub fn last_node_type_for_etex(&self) -> i32 {
        self.last_node_type
    }

    /// `\currentgrouplevel`（e-TeX）。
    pub fn cur_level_for_etex(&self) -> i32 {
        self.cur_level as i32
    }

    /// `\currentgrouptype`（e-TeX）。**番号は TeX のもの。**
    ///
    /// rtex には `AlignEntry` という TeX に無い群があるので、
    /// **並び順ではなく明示の対応で写す。**
    pub fn cur_group_for_etex(&self) -> i32 {
        use crate::eqtb::save_stack::GroupType::*;
        match self.cur_group.typ {
            BottomLevel => 0,
            Simple => 1,
            Hbox { .. } => 2,
            AdjustedHbox { .. } => 3,
            Vbox { .. } => 4,
            Vtop { .. } => 5,
            Align { .. } | AlignEntry { .. } => 6,
            NoAlign { .. } => 7,
            Output { .. } => 8,
            Math { .. } => 9,
            Disc { .. } => 10,
            Insert { .. } => 11,
            Vcenter { .. } => 12,
            MathChoice { .. } => 13,
            SemiSimple => 14,
            MathShift { .. } => 15,
            MathLeft { .. } => 16,
        }
    }

    /// `\showthe\usingnamespace` 相当。使っている名前空間を並べる。
    fn show_using_namespaces(&self, logger: &mut Logger) {
        logger.print_esc_str(b"usingnamespace");
        logger.print_char(b'=');
        if self.using_namespaces.is_empty() {
            logger.print_str("(none)");
            return;
        }
        for (i, ns) in self.using_namespaces.iter().enumerate() {
            if i > 0 {
                logger.print_char(b',');
            }
            let name = self.control_sequences.namespace_name(*ns).to_vec();
            for b in name {
                logger.print(b);
            }
        }
    }

    fn last_node_state(last_node: Option<&Node>) -> (i32, LastNodeInfo) {
        let node_type = match last_node {
            None => -1,
            Some(Node::Char(_) | Node::WideChar(_)) => 0,
            Some(Node::List(l)) => {
                if matches!(l.list, crate::nodes::HlistOrVlist::Hlist(_)) {
                    1
                } else {
                    2
                }
            }
            Some(Node::Rule(_)) => 3,
            Some(Node::Ins(_)) => 4,
            Some(Node::Mark(_)) => 5,
            Some(Node::Adjust(_)) => 6,
            Some(Node::Ligature(_)) => 7,
            Some(Node::Disc(_)) => 8,
            Some(Node::Whatsit(_)) => 9,
            Some(Node::Math(_)) => 10,
            Some(Node::Glue(_)) => 11,
            Some(Node::Kern(_)) => 12,
            Some(Node::Penalty(_)) => 13,
            Some(Node::Unset(_)) => 14,
            // 数式の内側の節。**e-TeX も 15 以降を持たない**ので数式扱いにする
            Some(_) => 10,
        };
        let info = if let Some(last_node) = last_node {
            match last_node {
                Node::Penalty(penalty_node) => LastNodeInfo::Penalty(penalty_node.penalty),
                Node::Kern(kern_node) => LastNodeInfo::Kern(kern_node.width),
                Node::Disc(disc_node) => {
                    if let Some(Node::Kern(kern_node)) = disc_node.no_break.last() {
                        LastNodeInfo::Kern(kern_node.width)
                    } else {
                        LastNodeInfo::Other
                    }
                }
                Node::Glue(glue_node) => {
                    if let GlueType::MuGlue = glue_node.subtype {
                        LastNodeInfo::MuGlue(glue_node.glue_spec.clone())
                    } else {
                        LastNodeInfo::Glue(glue_node.glue_spec.clone())
                    }
                }
                _ => LastNodeInfo::Other,
            }
        } else {
            LastNodeInfo::Other
        };
        (node_type, info)
    }

    /// Updates the condensed info about the last node of the current list.
    /// NOTE: We need to ensure that this is called whenever the last node of the current list has
    /// been changed.
    pub fn update_last_node_info(&mut self, last_node: Option<&Node>) {
        // **e-TeX の種類も控える。** `\lastnodetype` が要る（LaTeX が 11/12/13/1 を見る）
        (self.last_node_type, self.last_node_info) = Self::last_node_state(last_node);
    }

    /// Page builder が最後に調べた node を、base vertical list が空のときの値として控える。
    /// See 996.
    pub fn update_last_node_on_page(&mut self, last_node: Option<&Node>) {
        (self.last_node_type_on_page, self.last_node_on_page) = Self::last_node_state(last_node);
    }

    /// 空の base vertical list では、current page 側の控えを公開する。
    pub fn restore_last_node_from_page(&mut self) {
        self.last_node_type = self.last_node_type_on_page;
        self.last_node_info = self.last_node_on_page.clone();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variable {
    ControlSequence(ControlSequence),
    Skip(SkipVariable),
    ParShape,
    PenaltyArray(PenaltyArrayVariable),
    LanguageRegion,
    UsingNamespaces,
    TokenList(TokenListVariable),
    RawString(RawStringVariable),
    BoxRegister(BoxVariable),
    Font(FontVariable),
    JapaneseFont,
    CatCode(usize),
    KCatCode(KCatCodeBlock),
    AutoSpacing(AutoSpacingVariable),
    XspCode(u8),
    InhibitXspCode(LayoutCharacterCode),
    Code(CodeVariable),
    Integer(IntegerVariable),
    Dimen(DimensionVariable),
}

/// A variable together with its definition.
#[derive(Debug)]
pub enum Definition {
    ControlSequence(ControlSequence, Command),
    Skip(SkipVariable, Skip),
    ParShape(ParagraphShape),
    PenaltyArray(PenaltyArrayVariable, PenaltyArray),
    LanguageRegion(LanguageRegion),
    UsingNamespaces(Vec<NamespaceId>),
    TokenList(TokenListVariable, Option<RcTokenList>),
    RawString(RawStringVariable, RcRawString),
    BoxRegister(BoxVariable, Option<ListNode>),
    Font(FontVariable, FontIndex),
    JapaneseFont(Option<JapaneseFontIndex>),
    CatCode(usize, CatCode),
    KCatCode(KCatCodeBlock, KCatCode),
    AutoSpacing(AutoSpacingVariable, bool),
    XspCode(u8, XspCode),
    InhibitXspCode(LayoutCharacterCode, XspCode),
    Code(CodeVariable, i32),
    Integer(IntegerVariable, Integer),
    Dimen(DimensionVariable, Dimension),
}

impl Definition {
    fn to_variable(&self) -> Variable {
        match *self {
            Self::ControlSequence(control_sequence, _) => {
                Variable::ControlSequence(control_sequence)
            }
            Self::Skip(skip_variable, _) => Variable::Skip(skip_variable),
            Self::ParShape(_) => Variable::ParShape,
            Self::PenaltyArray(variable, _) => Variable::PenaltyArray(variable),
            Self::LanguageRegion(_) => Variable::LanguageRegion,
            Self::UsingNamespaces(_) => Variable::UsingNamespaces,
            Self::TokenList(token_list_variable, _) => Variable::TokenList(token_list_variable),
            Self::RawString(variable, _) => Variable::RawString(variable),
            Self::BoxRegister(box_variable, _) => Variable::BoxRegister(box_variable),
            Self::Font(font_variable, _) => Variable::Font(font_variable),
            Self::JapaneseFont(_) => Variable::JapaneseFont,
            Self::CatCode(chr, _) => Variable::CatCode(chr),
            Self::KCatCode(block, _) => Variable::KCatCode(block),
            Self::AutoSpacing(variable, _) => Variable::AutoSpacing(variable),
            Self::XspCode(character, _) => Variable::XspCode(character),
            Self::InhibitXspCode(character, _) => Variable::InhibitXspCode(character),
            Self::Code(code_variable, _) => Variable::Code(code_variable),
            Self::Integer(integer_variable, _) => Variable::Integer(integer_variable),
            Self::Dimen(dimension_variable, _) => Variable::Dimen(dimension_variable),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LastNodeInfo {
    Penalty(i32),
    Kern(Dimension),
    Glue(Rc<GlueSpec>),
    MuGlue(Rc<GlueSpec>),
    Other,
}

impl Dumpable for Variable {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::ControlSequence(control_sequence) => {
                writeln!(target, "ControlSequence")?;
                control_sequence.dump(target)?;
            }
            Self::Skip(skip_variable) => {
                writeln!(target, "Skip")?;
                skip_variable.dump(target)?;
            }
            Self::UsingNamespaces => {
                writeln!(target, "UsingNamespaces")?;
            }
            Self::ParShape => {
                writeln!(target, "ParShape")?;
            }
            Self::PenaltyArray(variable) => {
                writeln!(target, "PenaltyArray")?;
                variable.dump(target)?;
            }
            Self::LanguageRegion => {
                writeln!(target, "LanguageRegion")?;
            }
            Self::TokenList(token_list_variable) => {
                writeln!(target, "TokenList")?;
                token_list_variable.dump(target)?;
            }
            Self::RawString(variable) => {
                writeln!(target, "RawString")?;
                variable.dump(target)?;
            }
            Self::BoxRegister(box_variable) => {
                writeln!(target, "BoxRegister")?;
                box_variable.dump(target)?;
            }
            Self::Font(font_variable) => {
                writeln!(target, "Font")?;
                font_variable.dump(target)?;
            }
            Self::JapaneseFont => {
                writeln!(target, "JapaneseFont")?;
            }
            Self::CatCode(chr) => {
                if *chr > MAX_LATIN_UCS_CODE as usize {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "catcode index is out of range",
                    ));
                }
                writeln!(target, "CatCode")?;
                chr.dump(target)?;
            }
            Self::KCatCode(block) => {
                writeln!(target, "KCatCode")?;
                block.dump(target)?;
            }
            Self::AutoSpacing(variable) => {
                writeln!(target, "AutoSpacing")?;
                variable.dump(target)?;
            }
            Self::XspCode(character) => {
                writeln!(target, "XspCode")?;
                character.dump(target)?;
            }
            Self::InhibitXspCode(character) => {
                writeln!(target, "InhibitXspCode")?;
                character.dump(target)?;
            }
            Self::Code(code_variable) => {
                writeln!(target, "Code")?;
                code_variable.dump(target)?;
            }
            Self::Integer(integer_variable) => {
                writeln!(target, "Integer")?;
                integer_variable.dump(target)?;
            }
            Self::Dimen(dimension_variable) => {
                writeln!(target, "Dimen")?;
                dimension_variable.dump(target)?;
            }
        }

        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "ControlSequence" => {
                let control_sequence = ControlSequence::undump(lines)?;
                Ok(Self::ControlSequence(control_sequence))
            }
            "Skip" => {
                let skip_variable = SkipVariable::undump(lines)?;
                Ok(Self::Skip(skip_variable))
            }
            "ParShape" => Ok(Self::ParShape),
            "PenaltyArray" => Ok(Self::PenaltyArray(PenaltyArrayVariable::undump(lines)?)),
            "LanguageRegion" => Ok(Self::LanguageRegion),
            "UsingNamespaces" => Ok(Self::UsingNamespaces),
            "TokenList" => {
                let token_list_variable = TokenListVariable::undump(lines)?;
                Ok(Self::TokenList(token_list_variable))
            }
            "RawString" => Ok(Self::RawString(RawStringVariable::undump(lines)?)),
            "BoxRegister" => {
                let box_variable = BoxVariable::undump(lines)?;
                Ok(Self::BoxRegister(box_variable))
            }
            "Font" => {
                let font_variable = FontVariable::undump(lines)?;
                Ok(Self::Font(font_variable))
            }
            "JapaneseFont" => Ok(Self::JapaneseFont),
            "CatCode" => {
                let chr = usize::undump(lines)?;
                if chr <= MAX_LATIN_UCS_CODE as usize {
                    Ok(Self::CatCode(chr))
                } else {
                    Err(FormatError::ParseError)
                }
            }
            "KCatCode" => {
                let block = KCatCodeBlock::undump(lines)?;
                Ok(Self::KCatCode(block))
            }
            "AutoSpacing" => Ok(Self::AutoSpacing(AutoSpacingVariable::undump(lines)?)),
            "XspCode" => Ok(Self::XspCode(u8::undump(lines)?)),
            "InhibitXspCode" => Ok(Self::InhibitXspCode(LayoutCharacterCode::undump(lines)?)),
            "Code" => {
                let code_variable = CodeVariable::undump(lines)?;
                Ok(Self::Code(code_variable))
            }
            "Integer" => {
                let integer_variable = IntegerVariable::undump(lines)?;
                Ok(Self::Integer(integer_variable))
            }
            "Dimen" => {
                let dimension_variable = DimensionVariable::undump(lines)?;
                Ok(Self::Dimen(dimension_variable))
            }
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for Eqtb {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.variable_levels.dump(target)?;

        self.control_sequences.dump(target)?;
        self.skips.dump(target)?;
        self.par_shape.dump(target)?;
        self.penalty_arrays.dump(target)?;
        self.language_region.dump(target)?;
        self.using_namespaces.dump(target)?;
        self.token_lists.dump(target)?;
        self.raw_strings.dump(target)?;
        self.boxes.dump(target)?;
        self.font_params.dump(target)?;
        self.cur_japanese_font.dump(target)?;
        self.cat_codes.dump(target)?;
        self.kcat_codes.dump(target)?;
        self.auto_spacing.dump(target)?;
        self.xsp_codes.dump(target)?;
        self.inhibit_xsp_codes.dump(target)?;
        self.codes.dump(target)?;
        self.integers.dump(target)?;
        self.dimensions.dump(target)?;

        self.par_cs.dump(target)?;
        self.par_token.dump(target)?;
        self.write_cs.dump(target)?;

        self.fonts.dump(target)?;
        self.japanese_fonts.dump(target)?;

        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variable_levels = VariableLevels::undump(lines)?;

        let control_sequences = ControlSequenceStore::undump(lines)?;
        let skips = SkipParameters::undump(lines)?;
        let par_shape = ParShapeParameter::undump(lines)?;
        let penalty_arrays = PenaltyArrayParameters::undump(lines)?;
        let language_region = LanguageRegion::undump(lines)?;
        let using_namespaces: Vec<NamespaceId> = Vec::undump(lines)?;
        let token_lists = TokenListParameters::undump(lines)?;
        let raw_strings = RawStringRegisters::undump(lines)?;
        let boxes = BoxParameters::undump(lines)?;
        let font_params = FontParameters::undump(lines)?;
        let cur_japanese_font = Option::<JapaneseFontIndex>::undump(lines)?;
        let cat_codes = CatCodes::undump(lines)?;
        let kcat_codes = KCatCodes::undump(lines)?;
        let auto_spacing = AutoSpacingState::undump(lines)?;
        let xsp_codes = XspCodeTable::undump(lines)?;
        let inhibit_xsp_codes = InhibitXspCodeTable::undump(lines)?;
        let codes = CodeParameters::undump(lines)?;
        let integers = IntegerParameters::undump(lines)?;
        let dimensions = DimensionParameters::undump(lines)?;

        let par_cs = ControlSequence::undump(lines)?;
        let par_token = Token::undump(lines)?;
        let write_cs = ControlSequence::undump(lines)?;

        let fonts = Vec::undump(lines)?;
        let mut japanese_fonts: Vec<JapaneseFontInfo> = Vec::undump(lines)?;
        for (position, font) in japanese_fonts.iter_mut().enumerate() {
            let index = JapaneseFontIndex::from_position(position).ok_or(FormatError::ParseError)?;
            font.bind_index(index);
        }
        if cur_japanese_font
            .is_some_and(|index| index.position() >= japanese_fonts.len())
        {
            return Err(FormatError::ParseError);
        }

        Ok(Self {
            cur_level: 0,
            cur_group: Group {
                typ: GroupType::BottomLevel,
                saved_definitions: Vec::new(),
                after_tokens: Vec::new(),
            },
            save_stack: Vec::new(),
            max_save_stack: 0,
            variable_levels,

            control_sequences,
            skips,
            par_shape,
            penalty_arrays,
            language_region,
            using_namespaces,
            last_node_type: -1,
            token_lists,
            raw_strings,
            boxes,
            font_params,
            cur_japanese_font,
            cat_codes,
            kcat_codes,
            auto_spacing,
            xsp_codes,
            inhibit_xsp_codes,
            codes,
            integers,
            dimensions,
            par_cs,
            write_cs,
            par_token,
            mag_set: 0,
            run_date_time: None,
            fonts,
            japanese_fonts,
            last_badness: 0,
            dead_cycles: 0,
            output_active: false,
            insert_penalties: 0,
            page_dims: PageDimensions::new(),
            page_contents: PageContents::Empty,
            last_node_on_page: LastNodeInfo::Other,
            last_node_type_on_page: -1,
            marks: Marks::new(),
            line_number: 0,
            mode_type: Mode::Vertical,
            prev_graf: 0,
            prev_depth: IGNORE_DEPTH,
            space_factor: 0,
            last_node_info: LastNodeInfo::Other,
        })
    }
}

#[cfg(test)]
mod register_index_tests {
    use super::*;

    #[test]
    fn 書式の通常レジスタ番号は十五ビットに限る() {
        for text in ["0", "255", "256", "32767"] {
            let mut lines = std::iter::once(text);
            assert_eq!(
                undump_register_index(&mut lines).unwrap(),
                text.parse().unwrap()
            );
        }
        for text in ["32768", "65535"] {
            let mut lines = std::iter::once(text);
            assert!(matches!(
                undump_register_index(&mut lines),
                Err(FormatError::ParseError)
            ));
        }
    }

    #[test]
    fn 書式の挿入番号は二百五十四までに限る() {
        let mut valid = std::iter::once("254");
        assert_eq!(undump_insertion_index(&mut valid).unwrap(), 254);

        let mut reserved = std::iter::once("255");
        assert!(matches!(
            undump_insertion_index(&mut reserved),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 書式のmark_class番号は十五ビットに限る() {
        let mut valid = std::iter::once("32767");
        assert_eq!(undump_mark_class_index(&mut valid).unwrap(), 32767);

        let mut invalid = std::iter::once("32768");
        assert!(matches!(
            undump_mark_class_index(&mut invalid),
            Err(FormatError::ParseError)
        ));
    }
}

#[cfg(test)]
mod raw_string_group_tests {
    use super::*;
    use crate::logger::InteractionMode;

    fn 試験環境() -> (Eqtb, Scanner, Logger) {
        (
            Eqtb::new(),
            Scanner::new(Vec::new(), 0),
            Logger::new(String::new(), InteractionMode::Batch),
        )
    }

    fn 群を開く(eqtb: &mut Eqtb, scanner: &Scanner, logger: &mut Logger) {
        eqtb.new_save_level(GroupType::Simple, &scanner.input_stack, logger);
    }

    #[test]
    fn 局所的に隠した値の復元予約を他slotが消費しない() {
        let (mut eqtb, mut scanner, mut logger) = 試験環境();
        let full_slot = Rc::new(vec![b'x'; MAX_RAW_STRING_BYTES]);
        for register in 0..4 {
            eqtb.raw_string_define(
                RawStringVariable::new(register),
                full_slot.clone(),
                true,
            )
            .unwrap();
        }

        群を開く(&mut eqtb, &scanner, &mut logger);
        eqtb.raw_string_define(RawStringVariable::new(0), Rc::new(Vec::new()), false)
            .unwrap();
        assert_eq!(
            eqtb.raw_string_define(RawStringVariable::new(4), full_slot.clone(), true),
            Err(RawStringStorageError::StorageTooLarge)
        );

        // 拒否されたglobal追加が復元余白を壊していないので、群終了は失敗経路を持たない。
        eqtb.unsave(&mut scanner, &mut logger);
        assert!(Rc::ptr_eq(
            eqtb.raw_string(RawStringVariable::new(0)),
            &full_slot
        ));
    }

    #[test]
    fn global再定義は対象slotの復元予約を解消する() {
        let (mut eqtb, mut scanner, mut logger) = 試験環境();
        let full_slot = Rc::new(vec![b'x'; MAX_RAW_STRING_BYTES]);
        for register in 0..4 {
            eqtb.raw_string_define(
                RawStringVariable::new(register),
                full_slot.clone(),
                true,
            )
            .unwrap();
        }

        群を開く(&mut eqtb, &scanner, &mut logger);
        eqtb.raw_string_define(RawStringVariable::new(0), Rc::new(Vec::new()), false)
            .unwrap();
        eqtb.raw_string_define(RawStringVariable::new(0), Rc::new(Vec::new()), true)
            .unwrap();
        eqtb.raw_string_define(RawStringVariable::new(4), full_slot.clone(), true)
            .unwrap();
        eqtb.unsave(&mut scanner, &mut logger);

        assert!(eqtb.raw_string(RawStringVariable::new(0)).is_empty());
        assert!(Rc::ptr_eq(
            eqtb.raw_string(RawStringVariable::new(4)),
            &full_slot
        ));
    }

    #[test]
    fn 同level再代入は復元されない現在値を予約し続けない() {
        let (mut eqtb, mut scanner, mut logger) = 試験環境();
        let full_slot = Rc::new(vec![b'x'; MAX_RAW_STRING_BYTES]);
        群を開く(&mut eqtb, &scanner, &mut logger);

        eqtb.raw_string_define(
            RawStringVariable::new(0),
            full_slot.clone(),
            false,
        )
        .unwrap();
        eqtb.raw_string_define(RawStringVariable::new(0), Rc::new(Vec::new()), false)
            .unwrap();
        for register in 1..=4 {
            eqtb.raw_string_define(
                RawStringVariable::new(register),
                full_slot.clone(),
                true,
            )
            .unwrap();
        }

        eqtb.unsave(&mut scanner, &mut logger);
        assert!(eqtb.raw_string(RawStringVariable::new(0)).is_empty());
    }

    #[test]
    fn 入れ子の復元予約は同slotの最大値と他slotの和を取る() {
        let (mut eqtb, mut scanner, mut logger) = 試験環境();
        let half_slot = Rc::new(vec![b'h'; MAX_RAW_STRING_BYTES / 2]);
        let full_slot = Rc::new(vec![b'x'; MAX_RAW_STRING_BYTES]);
        let target = RawStringVariable::new(0);
        eqtb.raw_string_define(target, half_slot.clone(), true)
            .unwrap();
        群を開く(&mut eqtb, &scanner, &mut logger);
        eqtb.raw_string_define(target, full_slot.clone(), false)
            .unwrap();
        群を開く(&mut eqtb, &scanner, &mut logger);
        eqtb.raw_string_define(target, Rc::new(Vec::new()), false)
            .unwrap();

        let (other, target_maximum) = eqtb.raw_string_restore_budget_excluding(target).unwrap();
        assert_eq!(other, 0);
        assert_eq!(target_maximum, MAX_RAW_STRING_BYTES);
        for register in 1..=3 {
            eqtb.raw_string_define(
                RawStringVariable::new(register),
                full_slot.clone(),
                true,
            )
            .unwrap();
        }
        assert_eq!(
            eqtb.raw_string_define(RawStringVariable::new(4), full_slot, true),
            Err(RawStringStorageError::StorageTooLarge)
        );

        eqtb.unsave(&mut scanner, &mut logger);
        assert_eq!(eqtb.raw_string(target).len(), MAX_RAW_STRING_BYTES);
        eqtb.unsave(&mut scanner, &mut logger);
        assert!(Rc::ptr_eq(eqtb.raw_string(target), &half_slot));
    }

    #[test]
    fn eqtb_fmt往復は高位raw値と固定slot_commandを同じsection順で戻す() {
        let mut before = Eqtb::new();
        before.put_primitives_into_hash_table();
        let variable = RawStringVariable::new(32_767);
        let bytes = Rc::new(vec![0, b'\n', b'\r', 0xE3, 0x81, 0xFF]);
        before
            .raw_string_define(variable, bytes.clone(), true)
            .unwrap();
        let cs = before.lookup_or_create(b"rawalias").unwrap();
        before.cs_define(
            cs,
            Command::Unexpandable(UnexpandableCommand::Prefixable(
                crate::command::PrefixableCommand::RawString(
                    crate::command::RawStringCommand::Variable(variable),
                ),
            )),
            true,
        );

        let mut dumped = Vec::new();
        before.dump(&mut dumped).unwrap();
        let text = String::from_utf8(dumped).unwrap();
        let mut lines = text.lines();
        let after = Eqtb::undump(&mut lines).unwrap();

        assert_eq!(after.raw_string(variable).as_slice(), bytes.as_slice());
        let after_cs = after.lookup(b"rawalias").unwrap();
        assert_eq!(
            after.control_sequences.get(after_cs),
            &Command::Unexpandable(UnexpandableCommand::Prefixable(
                crate::command::PrefixableCommand::RawString(
                    crate::command::RawStringCommand::Variable(variable),
                ),
            ))
        );
        assert_eq!(lines.next(), None);
    }
}

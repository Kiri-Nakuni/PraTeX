//! 横組み日本語 list を閉じるときに使う、割当てを行わない境界 planner。
//!
//! この module は node を所有しない。main loop が記録した元の文字境界と、list 終端時の
//! parameter snapshot だけから action を再計算する。このため自動 K/X を除去して同じ入力を
//! 再評価しても、planner 自身は重複 node を作らない。

use super::{FixedGlue, ScriptSpacingActivationId};
use crate::dimension::MAX_DIMEN;
use crate::eqtb::LanguageRegion;
use crate::format::{Dumpable, FormatError};
use crate::nodes::{DimensionOrder, GlueSpec, HigherOrderDimension};

use std::io::Write;

pub(crate) const MAX_INHIBIT_XSP_CODES: usize = 1_024;
pub(crate) const MAX_KINSOKU_CODES: usize = 1_024;
pub(crate) const MAX_JFM_CLASSES: u16 = 256;
const PTEX_SPACING_STATE_DUMP_HEADER: &str = "ptex-spacing-state-v1";

/// `\xspcode` / `\inhibitxspcode` の公開値が許可する遷移方向。
///
/// bit 0 は和文→欧文、bit 1 は欧文→和文である。二つの primitive は説明上の
/// 視点が異なるが、この遷移方向へ正規化すると同じ 0--3 codec になる。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InterScriptDirection {
    JapaneseToLatin,
    LatinToJapanese,
}

impl InterScriptDirection {
    const fn mask(self) -> u8 {
        match self {
            Self::JapaneseToLatin => 1,
            Self::LatinToJapanese => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PtexSpacingCodecError {
    XspCodeOutOfRange(i32),
    XspCharacterOutOfRange(u32),
    NonUnicodeCharacter(u32),
}

/// pTeX 公開数値 0--3 を、生の整数のまま hot path へ流さないための型。
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XspCode(u8);

impl XspCode {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const JAPANESE_TO_LATIN: Self = Self(1);
    pub(crate) const LATIN_TO_JAPANESE: Self = Self(2);
    pub(crate) const BOTH: Self = Self(3);

    pub(crate) const fn from_public_integer(value: i32) -> Result<Self, PtexSpacingCodecError> {
        if value < 0 || value > 3 {
            Err(PtexSpacingCodecError::XspCodeOutOfRange(value))
        } else {
            Ok(Self(value as u8))
        }
    }

    pub(crate) const fn to_public_integer(self) -> i32 {
        self.0 as i32
    }

    pub(crate) const fn allows(self, direction: InterScriptDirection) -> bool {
        self.0 & direction.mask() != 0
    }
}

/// 文字 identity。入力 catcode、script class、JFM class のいずれでもない。
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LayoutCharacterCode(u32);

impl LayoutCharacterCode {
    pub(crate) const fn from_scalar(value: char) -> Self {
        Self(value as u32)
    }

    /// 現在 PraTeX が一文字 token として保持できる Unicode scalar の codec。
    /// upTeX の合成文字用私用 code は、identity model を追加する段階まで受理しない。
    pub(crate) const fn from_public_integer(value: u32) -> Result<Self, PtexSpacingCodecError> {
        if value > char::MAX as u32 || (value >= 0xd800 && value <= 0xdfff) {
            Err(PtexSpacingCodecError::NonUnicodeCharacter(value))
        } else {
            Ok(Self(value))
        }
    }

    pub(crate) const fn to_public_integer(self) -> u32 {
        self.0
    }
}

/// INITEX の `\xspcode` 表。英数字だけが両側許可で、残りは両側禁止。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct XspCodeTable {
    values: [XspCode; 256],
}

impl XspCodeTable {
    pub(crate) fn ptex_initex() -> Self {
        let mut values = [XspCode::NONE; 256];
        for byte in b'0'..=b'9' {
            values[byte as usize] = XspCode::BOTH;
        }
        for byte in b'A'..=b'Z' {
            values[byte as usize] = XspCode::BOTH;
        }
        for byte in b'a'..=b'z' {
            values[byte as usize] = XspCode::BOTH;
        }
        Self { values }
    }

    #[inline]
    pub(crate) fn get(&self, character: LayoutCharacterCode) -> XspCode {
        usize::try_from(character.0)
            .ok()
            .and_then(|index| self.values.get(index))
            .copied()
            .unwrap_or(XspCode::NONE)
    }

    pub(crate) fn set(
        &mut self,
        character: u32,
        value: XspCode,
    ) -> Result<XspCode, PtexSpacingCodecError> {
        let index = usize::try_from(character)
            .ok()
            .filter(|&index| index < self.values.len())
            .ok_or(PtexSpacingCodecError::XspCharacterOutOfRange(character))?;
        let old = self.values[index];
        self.values[index] = value;
        Ok(old)
    }
}

impl Default for XspCodeTable {
    fn default() -> Self {
        Self::ptex_initex()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InhibitXspCodeEntry {
    character: LayoutCharacterCode,
    value: XspCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpacingStateError {
    InhibitXspCodeTableFull,
    KinsokuTableFull,
}

/// 未登録を 3 とする sparse `\inhibitxspcode` 表。
///
/// 読み取りは binary search 一回で割当てなし。3 の代入は entry を除去する。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct InhibitXspCodeTable {
    entries: Vec<InhibitXspCodeEntry>,
}

impl InhibitXspCodeTable {
    #[inline]
    pub(crate) fn get(&self, character: LayoutCharacterCode) -> XspCode {
        self.entries
            .binary_search_by_key(&character, |entry| entry.character)
            .ok()
            .map(|index| self.entries[index].value)
            .unwrap_or(XspCode::BOTH)
    }

    pub(crate) fn set(
        &mut self,
        character: LayoutCharacterCode,
        value: XspCode,
    ) -> Result<XspCode, SpacingStateError> {
        match self
            .entries
            .binary_search_by_key(&character, |entry| entry.character)
        {
            Ok(index) => {
                let old = self.entries[index].value;
                if value == XspCode::BOTH {
                    self.entries.remove(index);
                } else {
                    self.entries[index].value = value;
                }
                Ok(old)
            }
            Err(_) if value == XspCode::BOTH => Ok(XspCode::BOTH),
            Err(index) => {
                if self.entries.len() == MAX_INHIBIT_XSP_CODES {
                    return Err(SpacingStateError::InhibitXspCodeTableFull);
                }
                self.entries
                    .insert(index, InhibitXspCodeEntry { character, value });
                Ok(XspCode::BOTH)
            }
        }
    }

    /// `Eqtb` の save stack へ渡す前に、失敗し得る疎表の増加だけを検査する。
    ///
    /// 実際の書換えは `Eqtb::apply_definition` の一箇所で行うため、ここでは現在値を
    /// 変更しない。既存entryの更新と既定値3への復帰は容量を増やさない。
    pub(crate) fn can_set(
        &self,
        character: LayoutCharacterCode,
        value: XspCode,
        other_restore_reservations: usize,
    ) -> bool {
        value == XspCode::BOTH
            || self
                .entries
                .binary_search_by_key(&character, |entry| entry.character)
                .is_ok()
            || self
                .entries
                .len()
                .saturating_add(other_restore_reservations)
                < MAX_INHIBIT_XSP_CODES
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KinsokuPosition {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KinsokuEntry {
    character: LayoutCharacterCode,
    position: KinsokuPosition,
    value: i32,
}

/// `\prebreakpenalty` と `\postbreakpenalty` が共有する sparse 表。
///
/// 同じ文字への後の代入が位置を含めて置換する、という pTeX の公開契約を型で保つ。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct KinsokuPenaltyTable {
    entries: Vec<KinsokuEntry>,
}

impl KinsokuPenaltyTable {
    pub(crate) fn pre(&self, character: LayoutCharacterCode) -> i32 {
        self.get(character, KinsokuPosition::Before)
    }

    pub(crate) fn post(&self, character: LayoutCharacterCode) -> i32 {
        self.get(character, KinsokuPosition::After)
    }

    pub(crate) fn set_pre(
        &mut self,
        character: LayoutCharacterCode,
        value: i32,
    ) -> Result<Option<(KinsokuPosition, i32)>, SpacingStateError> {
        self.set(character, KinsokuPosition::Before, value)
    }

    pub(crate) fn set_post(
        &mut self,
        character: LayoutCharacterCode,
        value: i32,
    ) -> Result<Option<(KinsokuPosition, i32)>, SpacingStateError> {
        self.set(character, KinsokuPosition::After, value)
    }

    fn get(&self, character: LayoutCharacterCode, position: KinsokuPosition) -> i32 {
        self.entries
            .binary_search_by_key(&character, |entry| entry.character)
            .ok()
            .map(|index| self.entries[index])
            .filter(|entry| entry.position == position)
            .map(|entry| entry.value)
            .unwrap_or(0)
    }

    fn set(
        &mut self,
        character: LayoutCharacterCode,
        position: KinsokuPosition,
        value: i32,
    ) -> Result<Option<(KinsokuPosition, i32)>, SpacingStateError> {
        match self
            .entries
            .binary_search_by_key(&character, |entry| entry.character)
        {
            Ok(index) => {
                let old = self.entries[index];
                if value == 0 {
                    self.entries.remove(index);
                } else {
                    self.entries[index] = KinsokuEntry {
                        character,
                        position,
                        value,
                    };
                }
                Ok(Some((old.position, old.value)))
            }
            Err(_) if value == 0 => Ok(None),
            Err(index) => {
                if self.entries.len() == MAX_KINSOKU_CODES {
                    return Err(SpacingStateError::KinsokuTableFull);
                }
                self.entries.insert(
                    index,
                    KinsokuEntry {
                        character,
                        position,
                        value,
                    },
                );
                Ok(None)
            }
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

impl FixedGlue {
    pub(crate) const ZERO: Self = Self {
        width: 0,
        stretch: HigherOrderDimension {
            order: DimensionOrder::Normal,
            value: 0,
        },
        shrink: HigherOrderDimension {
            order: DimensionOrder::Normal,
            value: 0,
        },
    };

    pub(crate) const fn from_parts(
        width: i32,
        stretch: HigherOrderDimension,
        shrink: HigherOrderDimension,
    ) -> Self {
        Self {
            width,
            stretch,
            shrink,
        }
    }

    /// list 終端時の eqtb glue を参照数なしの最終値へ写す。
    pub(crate) const fn snapshot(glue: &GlueSpec) -> Self {
        Self {
            width: glue.width,
            stretch: glue.stretch,
            shrink: glue.shrink,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AutoSpacingState {
    kanji: bool,
    xkanji: bool,
}

/// pTeX の二つの自動間隔switchを、save stack上で互いに独立させる添字。
///
/// 状態全体を一変数として保存すると、一方への大域代入が他方の局所値まで大域化する。
/// 公開commandは別でも保存単位を誤って結合しないため、明示的な二要素domainにする。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AutoSpacingVariable {
    Kanji,
    XKanji,
}

impl AutoSpacingVariable {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Kanji => 0,
            Self::XKanji => 1,
        }
    }
}

impl AutoSpacingState {
    pub(crate) const ENABLED: Self = Self {
        kanji: true,
        xkanji: true,
    };

    pub(crate) const fn new(kanji: bool, xkanji: bool) -> Self {
        Self { kanji, xkanji }
    }

    pub(crate) const fn kanji(self) -> bool {
        self.kanji
    }

    pub(crate) const fn xkanji(self) -> bool {
        self.xkanji
    }

    pub(crate) fn set_kanji(&mut self, value: bool) {
        self.kanji = value;
    }

    pub(crate) fn set_xkanji(&mut self, value: bool) {
        self.xkanji = value;
    }

    pub(crate) const fn get(self, variable: AutoSpacingVariable) -> bool {
        match variable {
            AutoSpacingVariable::Kanji => self.kanji,
            AutoSpacingVariable::XKanji => self.xkanji,
        }
    }

    pub(crate) fn set(&mut self, variable: AutoSpacingVariable, value: bool) -> bool {
        match variable {
            AutoSpacingVariable::Kanji => std::mem::replace(&mut self.kanji, value),
            AutoSpacingVariable::XKanji => std::mem::replace(&mut self.xkanji, value),
        }
    }
}

impl Default for AutoSpacingState {
    fn default() -> Self {
        Self::ENABLED
    }
}

/// list close 時に一度だけ snapshot する pTeX profile の state。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PtexSpacingState {
    auto: AutoSpacingState,
    kanji_skip: FixedGlue,
    xkanji_skip: FixedGlue,
    xsp_codes: XspCodeTable,
    inhibit_xsp_codes: InhibitXspCodeTable,
    penalties: KinsokuPenaltyTable,
}

/// JLReq Appendix A.1/A.2のうち、横組和文JFM経路で扱う括弧対。
///
/// ASCII括弧はLatin経路のままとし、JLReq Appendix Aの注記に従って和文で使う
/// fullwidth互換形を登録する。横組限定の欧文引用符、guillemet、縦組限定の
/// double primeはこのbounded subsetに含めない。
const BUILT_IN_HORIZONTAL_JAPANESE_BRACKET_PAIRS: [(char, char); 12] = [
    ('（', '）'),
    ('〔', '〕'),
    ('［', '］'),
    ('｛', '｝'),
    ('〈', '〉'),
    ('《', '》'),
    ('「', '」'),
    ('『', '』'),
    ('【', '】'),
    ('⦅', '⦆'),
    ('〘', '〙'),
    ('〖', '〗'),
];

/// BuiltIn最小禁則の唯一の文字集合。state初期化とmain-loop照会はともにここを通る。
fn visit_built_in_kinsoku(mut visitor: impl FnMut(LayoutCharacterCode, KinsokuPosition, i32)) {
    for character in ['、', '。'] {
        visitor(
            LayoutCharacterCode::from_scalar(character),
            KinsokuPosition::Before,
            10_000,
        );
    }
    for (opening, closing) in BUILT_IN_HORIZONTAL_JAPANESE_BRACKET_PAIRS {
        visitor(
            LayoutCharacterCode::from_scalar(opening),
            KinsokuPosition::After,
            10_000,
        );
        visitor(
            LayoutCharacterCode::from_scalar(closing),
            KinsokuPosition::Before,
            10_000,
        );
    }
}

fn built_in_kinsoku(character: LayoutCharacterCode, position: KinsokuPosition) -> i32 {
    let mut value = 0;
    visit_built_in_kinsoku(|candidate, candidate_position, candidate_value| {
        if character == candidate && position == candidate_position {
            value = candidate_value;
        }
    });
    value
}

impl PtexSpacingState {
    pub(crate) fn initex() -> Self {
        Self {
            auto: AutoSpacingState::ENABLED,
            kanji_skip: FixedGlue::ZERO,
            xkanji_skip: FixedGlue::ZERO,
            xsp_codes: XspCodeTable::ptex_initex(),
            inhibit_xsp_codes: InhibitXspCodeTable::default(),
            penalties: KinsokuPenaltyTable::default(),
        }
    }

    /// PraTeX native横組みの最小BuiltIn snapshot。
    ///
    /// 禁則文字はW3C JLReq 3.1.7/3.1.8の全class実装ではなく、現在の合成JFMと
    /// production経路を固定する代表subsetだけである。公開primitiveを生やす前に、
    /// code point表をconsumerへ散らさないためplanner所有の一箇所で構築する。
    pub(crate) fn built_in_minimal(kanji_skip: FixedGlue, xkanji_skip: FixedGlue) -> Self {
        let mut state = Self::initex();
        state.set_kanji_skip(kanji_skip);
        state.set_xkanji_skip(xkanji_skip);
        visit_built_in_kinsoku(|character, position, value| {
            let result = match position {
                KinsokuPosition::Before => state.penalties_mut().set_pre(character, value),
                KinsokuPosition::After => state.penalties_mut().set_post(character, value),
            };
            result.expect("the fixed BuiltIn subset is below the bounded kinsoku table limit");
        });
        state
    }

    /// production `Eqtb` が所有するswitchと許可表を、list終端時のK/Xと一緒にsnapshotする。
    ///
    /// 禁則subsetの決定箇所は `built_in_minimal` に保ち、consumer側で文字表を複製しない。
    pub(crate) fn built_in_minimal_with_controls(
        kanji_skip: FixedGlue,
        xkanji_skip: FixedGlue,
        auto: AutoSpacingState,
        xsp_codes: &XspCodeTable,
        inhibit_xsp_codes: &InhibitXspCodeTable,
    ) -> Self {
        let mut state = Self::built_in_minimal(kanji_skip, xkanji_skip);
        state.auto = auto;
        state.xsp_codes.clone_from(xsp_codes);
        state.inhibit_xsp_codes.clone_from(inhibit_xsp_codes);
        state
    }

    pub(crate) const fn auto(&self) -> AutoSpacingState {
        self.auto
    }

    pub(crate) fn auto_mut(&mut self) -> &mut AutoSpacingState {
        &mut self.auto
    }

    pub(crate) const fn kanji_skip(&self) -> FixedGlue {
        self.kanji_skip
    }

    pub(crate) fn set_kanji_skip(&mut self, value: FixedGlue) {
        self.kanji_skip = value;
    }

    pub(crate) const fn xkanji_skip(&self) -> FixedGlue {
        self.xkanji_skip
    }

    pub(crate) fn set_xkanji_skip(&mut self, value: FixedGlue) {
        self.xkanji_skip = value;
    }

    pub(crate) fn xsp_codes(&self) -> &XspCodeTable {
        &self.xsp_codes
    }

    pub(crate) fn xsp_codes_mut(&mut self) -> &mut XspCodeTable {
        &mut self.xsp_codes
    }

    pub(crate) fn inhibit_xsp_codes(&self) -> &InhibitXspCodeTable {
        &self.inhibit_xsp_codes
    }

    pub(crate) fn inhibit_xsp_codes_mut(&mut self) -> &mut InhibitXspCodeTable {
        &mut self.inhibit_xsp_codes
    }

    pub(crate) fn penalties(&self) -> &KinsokuPenaltyTable {
        &self.penalties
    }

    pub(crate) fn penalties_mut(&mut self) -> &mut KinsokuPenaltyTable {
        &mut self.penalties
    }
}

impl Default for PtexSpacingState {
    fn default() -> Self {
        Self::initex()
    }
}

impl Dumpable for XspCode {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let value = u8::undump(lines)?;
        Self::from_public_integer(i32::from(value)).map_err(|_| FormatError::ParseError)
    }
}

impl Dumpable for LayoutCharacterCode {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Self::from_public_integer(u32::undump(lines)?).map_err(|_| FormatError::ParseError)
    }
}

impl Dumpable for XspCodeTable {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.values.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self {
            values: <[XspCode; 256]>::undump(lines)?,
        })
    }
}

impl Dumpable for InhibitXspCodeEntry {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.character.dump(target)?;
        self.value.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self {
            character: LayoutCharacterCode::undump(lines)?,
            value: XspCode::undump(lines)?,
        })
    }
}

impl Dumpable for InhibitXspCodeTable {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.entries.len().dump(target)?;
        for entry in &self.entries {
            entry.dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let len = usize::undump(lines)?;
        if len > MAX_INHIBIT_XSP_CODES {
            return Err(FormatError::ParseError);
        }
        let mut entries = Vec::with_capacity(len);
        for _ in 0..len {
            let entry = InhibitXspCodeEntry::undump(lines)?;
            if entry.value == XspCode::BOTH
                || entries
                    .last()
                    .is_some_and(|previous: &InhibitXspCodeEntry| {
                        previous.character >= entry.character
                    })
            {
                return Err(FormatError::ParseError);
            }
            entries.push(entry);
        }
        Ok(Self { entries })
    }
}

impl Dumpable for KinsokuPosition {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(
            target,
            "{}",
            match self {
                Self::Before => "Before",
                Self::After => "After",
            }
        )
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "Before" => Ok(Self::Before),
            "After" => Ok(Self::After),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for KinsokuEntry {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.character.dump(target)?;
        self.position.dump(target)?;
        self.value.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self {
            character: LayoutCharacterCode::undump(lines)?,
            position: KinsokuPosition::undump(lines)?,
            value: i32::undump(lines)?,
        })
    }
}

impl Dumpable for KinsokuPenaltyTable {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.entries.len().dump(target)?;
        for entry in &self.entries {
            entry.dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let len = usize::undump(lines)?;
        if len > MAX_KINSOKU_CODES {
            return Err(FormatError::ParseError);
        }
        let mut entries = Vec::with_capacity(len);
        for _ in 0..len {
            let entry = KinsokuEntry::undump(lines)?;
            if entry.value == 0
                || entries
                    .last()
                    .is_some_and(|previous: &KinsokuEntry| previous.character >= entry.character)
            {
                return Err(FormatError::ParseError);
            }
            entries.push(entry);
        }
        Ok(Self { entries })
    }
}

impl Dumpable for FixedGlue {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.width.dump(target)?;
        self.stretch.dump(target)?;
        self.shrink.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let glue = Self {
            width: i32::undump(lines)?,
            stretch: HigherOrderDimension::undump(lines)?,
            shrink: HigherOrderDimension::undump(lines)?,
        };
        if [glue.width, glue.stretch.value, glue.shrink.value]
            .into_iter()
            .all(|value| (-MAX_DIMEN..=MAX_DIMEN).contains(&value))
        {
            Ok(glue)
        } else {
            Err(FormatError::ParseError)
        }
    }
}

impl Dumpable for AutoSpacingState {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.kanji.dump(target)?;
        self.xkanji.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self {
            kanji: bool::undump(lines)?,
            xkanji: bool::undump(lines)?,
        })
    }
}

impl Dumpable for AutoSpacingVariable {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(
            target,
            "{}",
            match self {
                Self::Kanji => "Kanji",
                Self::XKanji => "XKanji",
            }
        )
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "Kanji" => Ok(Self::Kanji),
            "XKanji" => Ok(Self::XKanji),
            _ => Err(FormatError::ParseError),
        }
    }
}

impl Dumpable for PtexSpacingState {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{PTEX_SPACING_STATE_DUMP_HEADER}")?;
        self.auto.dump(target)?;
        self.kanji_skip.dump(target)?;
        self.xkanji_skip.dump(target)?;
        self.xsp_codes.dump(target)?;
        self.inhibit_xsp_codes.dump(target)?;
        self.penalties.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let header = lines.next().ok_or(FormatError::IncompleteFile)?;
        if header != PTEX_SPACING_STATE_DUMP_HEADER {
            return Err(FormatError::ParseError);
        }
        Ok(Self {
            auto: AutoSpacingState::undump(lines)?,
            kanji_skip: FixedGlue::undump(lines)?,
            xkanji_skip: FixedGlue::undump(lines)?,
            xsp_codes: XspCodeTable::undump(lines)?,
            inhibit_xsp_codes: InhibitXspCodeTable::undump(lines)?,
            penalties: KinsokuPenaltyTable::undump(lines)?,
        })
    }
}

/// Engine identity とは独立した、明示的な組版 profile 選択。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JapaneseSpacingProfile {
    BuiltInPtex,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JfmFontId(u32);

impl JfmFontId {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JfmMetricId(u32);

impl JfmMetricId {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlannerJfmClassId(u8);

impl PlannerJfmClassId {
    pub(crate) const fn new(value: u8) -> Self {
        Self(value)
    }

    const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JfmPairSpacing {
    Glue(FixedGlue),
    Kern(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JfmPairSpacingRule {
    left: PlannerJfmClassId,
    right: PlannerJfmClassId,
    spacing: JfmPairSpacing,
}

impl JfmPairSpacingRule {
    pub(crate) const fn new(
        left: PlannerJfmClassId,
        right: PlannerJfmClassId,
        spacing: JfmPairSpacing,
    ) -> Self {
        Self {
            left,
            right,
            spacing,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JfmPairSpacingTableError {
    EmptyClassSet,
    TooManyClasses {
        actual: u16,
        maximum: u16,
    },
    ClassOutOfBounds {
        rule_index: usize,
        class: u8,
        class_count: u16,
    },
    DuplicateRule {
        first_rule_index: usize,
        second_rule_index: usize,
    },
    TableTooLarge,
}

/// font load 時に scale 済み JFM glue/kern を dense class-pair 表へする。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompiledJfmPairSpacingTable {
    metric: JfmMetricId,
    class_count: u16,
    pairs: Vec<Option<JfmPairSpacing>>,
}

impl CompiledJfmPairSpacingTable {
    pub(crate) fn compile(
        metric: JfmMetricId,
        class_count: u16,
        rules: &[JfmPairSpacingRule],
    ) -> Result<Self, JfmPairSpacingTableError> {
        if class_count == 0 {
            return Err(JfmPairSpacingTableError::EmptyClassSet);
        }
        if class_count > MAX_JFM_CLASSES {
            return Err(JfmPairSpacingTableError::TooManyClasses {
                actual: class_count,
                maximum: MAX_JFM_CLASSES,
            });
        }
        let pair_count = usize::from(class_count)
            .checked_mul(usize::from(class_count))
            .ok_or(JfmPairSpacingTableError::TableTooLarge)?;
        let mut pairs = vec![None; pair_count];
        let mut owners = vec![None; pair_count];
        for (rule_index, rule) in rules.iter().copied().enumerate() {
            for class in [rule.left, rule.right] {
                if class.index() >= usize::from(class_count) {
                    return Err(JfmPairSpacingTableError::ClassOutOfBounds {
                        rule_index,
                        class: class.0,
                        class_count,
                    });
                }
            }
            let index = rule.left.index() * usize::from(class_count) + rule.right.index();
            if let Some(first_rule_index) = owners[index] {
                return Err(JfmPairSpacingTableError::DuplicateRule {
                    first_rule_index,
                    second_rule_index: rule_index,
                });
            }
            owners[index] = Some(rule_index);
            pairs[index] = Some(rule.spacing);
        }
        Ok(Self {
            metric,
            class_count,
            pairs,
        })
    }

    #[inline]
    pub(crate) fn get(
        &self,
        metric: JfmMetricId,
        left: PlannerJfmClassId,
        right: PlannerJfmClassId,
    ) -> Option<JfmPairSpacing> {
        if metric != self.metric
            || left.index() >= usize::from(self.class_count)
            || right.index() >= usize::from(self.class_count)
        {
            return None;
        }
        self.pairs[left.index() * usize::from(self.class_count) + right.index()]
    }

    /// Font instanceの最終indexが決まった時だけmetric identityを結び直す。
    /// class対表そのものはJFM load時に一度だけcompile済みである。
    pub(crate) fn rebind_metric(&mut self, metric: JfmMetricId) {
        self.metric = metric;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LatinBoundary {
    leading_character: LayoutCharacterCode,
    trailing_character: LayoutCharacterCode,
}

impl LatinBoundary {
    pub(crate) const fn new(character: LayoutCharacterCode) -> Self {
        Self {
            leading_character: character,
            trailing_character: character,
        }
    }

    /// Ligatureの左右でxspcodeへ渡す元文字を失わない。
    pub(crate) const fn ligature(
        leading_character: LayoutCharacterCode,
        trailing_character: LayoutCharacterCode,
    ) -> Self {
        Self {
            leading_character,
            trailing_character,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JapaneseBoundary {
    character: LayoutCharacterCode,
    font: JfmFontId,
    metric: JfmMetricId,
    class: PlannerJfmClassId,
}

impl JapaneseBoundary {
    pub(crate) const fn new(
        character: LayoutCharacterCode,
        font: JfmFontId,
        metric: JfmMetricId,
        class: PlannerJfmClassId,
    ) -> Self {
        Self {
            character,
            font,
            metric,
            class,
        }
    }

    /// JFM の文字タイプ0を、実文字identityとは別のpair endpointとして作る。
    ///
    /// nodeを作らない制御列で和文main loopが途切れた時だけ使う。禁則には
    /// 元の実文字対を渡すため、このendpointの`character`はJFM表引き以外へ出ない。
    pub(crate) const fn default_class_endpoint(self) -> Self {
        Self {
            class: PlannerJfmClassId::new(0),
            ..self
        }
    }

    pub(crate) fn font_position(self) -> Option<usize> {
        usize::try_from(self.font.0).ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BoundaryAtom {
    Latin(LatinBoundary),
    Japanese(JapaneseBoundary),
}

/// 和文main loopが一つの入力eventから早期materializeする境界。
///
/// `BreakAfterJapanese` / `ResumeBeforeJapanese`はJFM class 0を挟むが、禁則は
/// `ResumeBeforeJapanese`が持つ実文字対へ一度だけ適用する。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MainLoopBoundaryEvent {
    Direct {
        left: BoundaryAtom,
        right: BoundaryAtom,
    },
    BreakAfterJapanese {
        left: JapaneseBoundary,
    },
    ResumeBeforeJapanese {
        logical_left: JapaneseBoundary,
        right: JapaneseBoundary,
    },
}

impl BoundaryAtom {
    pub(crate) const fn leading_character(self) -> LayoutCharacterCode {
        match self {
            Self::Latin(atom) => atom.leading_character,
            Self::Japanese(atom) => atom.character,
        }
    }

    pub(crate) const fn trailing_character(self) -> LayoutCharacterCode {
        match self {
            Self::Latin(atom) => atom.trailing_character,
            Self::Japanese(atom) => atom.character,
        }
    }

    pub(crate) const fn is_japanese(self) -> bool {
        matches!(self, Self::Japanese(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JfmPairContinuity {
    Continuous,
    Broken,
    /// class 0側のmain-loop JFMが元pair spacingを置換した境界。
    ReplacedByMainLoopJfm,
}

impl Default for JfmPairContinuity {
    fn default() -> Self {
        Self::Continuous
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JfmGlueControl {
    Allow,
    Inhibit,
}

impl Default for JfmGlueControl {
    fn default() -> Self {
        Self::Allow
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BoundaryContext {
    pub(crate) jfm_continuity: JfmPairContinuity,
    pub(crate) jfm_glue: JfmGlueControl,
}

impl BoundaryContext {
    pub(crate) const DEFAULT: Self = Self {
        jfm_continuity: JfmPairContinuity::Continuous,
        jfm_glue: JfmGlueControl::Allow,
    };
}

impl Default for BoundaryContext {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// action variant 自体が、自動 node の provenance と material/implicit の別を持つ。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlannedSpacingAction {
    KinsokuPenalty { value: i32 },
    JfmGlue { glue: FixedGlue },
    JfmKern { width: i32 },
    ImplicitKanjiSkip { glue: FixedGlue, active: bool },
    MaterialXKanjiSkip { glue: FixedGlue, active: bool },
}

/// JFM/禁則は main loop で観測可能な node として早期に置き、K/X は list 終端の
/// parameter 最終値から作る。planner の決定を consumer 側で再分類しないための phase。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpacingActionPhase {
    MainLoop,
    ListFinalizer,
}

impl PlannedSpacingAction {
    pub(crate) const fn phase(self) -> SpacingActionPhase {
        match self {
            Self::KinsokuPenalty { .. } | Self::JfmGlue { .. } | Self::JfmKern { .. } => {
                SpacingActionPhase::MainLoop
            }
            Self::ImplicitKanjiSkip { .. } | Self::MaterialXKanjiSkip { .. } => {
                SpacingActionPhase::ListFinalizer
            }
        }
    }
}

const MAX_BOUNDARY_ACTIONS: usize = 2;

/// 一境界の固定長 plan。通常は penalty と spacing が各一つなので heap を使わない。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BoundaryActionPlan {
    actions: [Option<PlannedSpacingAction>; MAX_BOUNDARY_ACTIONS],
    len: u8,
    jfm_spacing_inhibited: bool,
}

impl BoundaryActionPlan {
    const EMPTY: Self = Self {
        actions: [None; MAX_BOUNDARY_ACTIONS],
        len: 0,
        jfm_spacing_inhibited: false,
    };

    fn push(&mut self, action: PlannedSpacingAction) {
        debug_assert!((self.len as usize) < self.actions.len());
        self.actions[self.len as usize] = Some(action);
        self.len += 1;
    }

    pub(crate) const fn len(self) -> usize {
        self.len as usize
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub(crate) fn actions(&self) -> impl ExactSizeIterator<Item = PlannedSpacingAction> + '_ {
        self.actions[..self.len as usize].iter().map(|action| {
            action.expect("the initialized prefix of a boundary plan contains actions")
        })
    }

    pub(crate) fn actions_for_phase(
        &self,
        phase: SpacingActionPhase,
    ) -> impl Iterator<Item = PlannedSpacingAction> + '_ {
        self.actions().filter(move |action| action.phase() == phase)
    }

    /// class 0境界を作った側が、実際にJFM spacingをmaterializeしたかを返す。
    pub(crate) fn has_jfm_spacing(&self) -> bool {
        self.actions().any(|action| {
            matches!(
                action,
                PlannedSpacingAction::JfmGlue { .. } | PlannedSpacingAction::JfmKern { .. }
            )
        })
    }

    /// `\inhibitglue`が実在するJFM pairを抑止した時だけ真にする。
    ///
    /// pair自体が無い境界は暗黙K候補なので、単にcommandがpendingだっただけでは真にしない。
    pub(crate) const fn jfm_spacing_inhibited(&self) -> bool {
        self.jfm_spacing_inhibited
    }
}

/// listごとのASCII fast gateと、nodeを作らない入力eventのJFM連続性。
///
/// node-less eventの意味は、右側WideCharへ型付きprovenanceとして移すまでだけ保持する。
/// list全体のglyph通し番号や可変長side tableを持たない。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ScriptSpacingListState {
    needs_script_spacing: bool,
    previous_main_loop_boundary: Option<BoundaryAtom>,
    pending_jfm_continuity: JfmPairContinuity,
    pending_jfm_glue: JfmGlueControl,
    broken_left_jfm_was_inhibited: bool,
    compiled_profile: CompiledProfileObservation,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CompiledProfileObservation {
    #[default]
    Unseen,
    Stable {
        activation: Option<ScriptSpacingActivationId>,
        region: LanguageRegion,
    },
    Inconsistent,
}

impl ScriptSpacingListState {
    #[inline]
    pub(crate) fn observe(&mut self, atom: BoundaryAtom) {
        self.needs_script_spacing |= atom.is_japanese();
    }

    /// Records the run-local profile and layout region seen by each direct glyph in this list.
    ///
    /// Until region markers are carried by nodes, a mixed-region list cannot be classified
    /// faithfully. Such a list, or one spanning an activation replacement, is therefore sent to
    /// the built-in profile as one atomic unit.
    #[inline]
    pub(crate) fn observe_with_profile(
        &mut self,
        atom: BoundaryAtom,
        activation: Option<ScriptSpacingActivationId>,
        region: LanguageRegion,
    ) {
        self.observe(atom);
        self.needs_script_spacing |= activation.is_some();
        self.compiled_profile = match self.compiled_profile {
            CompiledProfileObservation::Unseen => {
                CompiledProfileObservation::Stable { activation, region }
            }
            CompiledProfileObservation::Stable {
                activation: previous_activation,
                region: previous_region,
            } if previous_activation == activation
                && (activation.is_none() || previous_region == region) =>
            {
                CompiledProfileObservation::Stable {
                    activation,
                    region: previous_region,
                }
            }
            CompiledProfileObservation::Stable { .. }
            | CompiledProfileObservation::Inconsistent => CompiledProfileObservation::Inconsistent,
        };
    }

    pub(crate) const fn compiled_profile_context(
        &self,
    ) -> Option<(ScriptSpacingActivationId, LanguageRegion)> {
        match self.compiled_profile {
            CompiledProfileObservation::Stable {
                activation: Some(activation),
                region,
            } => Some((activation, region)),
            CompiledProfileObservation::Unseen
            | CompiledProfileObservation::Stable {
                activation: None, ..
            }
            | CompiledProfileObservation::Inconsistent => None,
        }
    }

    /// Node追加時のASCII fast gate用。分類tableやprovider registryを引かない。
    #[inline]
    pub(crate) fn observe_japanese(&mut self) {
        self.needs_script_spacing = true;
    }

    /// A bulk-unboxed compiled node has no trustworthy run-local generation provenance. Ensure
    /// finalization removes it, but never reuse the close-time active table for the old list.
    pub(crate) fn observe_existing_compiled_spacing(&mut self) {
        self.needs_script_spacing = true;
        self.compiled_profile = CompiledProfileObservation::Inconsistent;
    }

    pub(crate) const fn needs_script_spacing(&self) -> bool {
        self.needs_script_spacing
    }

    /// 実glyphを一つ追加する直前に、前の実glyphとJFM連続性を返して状態を進める。
    pub(crate) fn observe_main_loop_boundary(
        &mut self,
        atom: BoundaryAtom,
    ) -> Option<(BoundaryAtom, JfmPairContinuity, bool)> {
        let previous = self.previous_main_loop_boundary.map(|previous| {
            (
                previous,
                self.pending_jfm_continuity,
                self.broken_left_jfm_was_inhibited,
            )
        });
        self.previous_main_loop_boundary = Some(atom);
        self.pending_jfm_continuity = JfmPairContinuity::Continuous;
        self.broken_left_jfm_was_inhibited = false;
        previous
    }

    /// node-less eventで左側JFMをclass 0へ閉じる必要がある時だけ一度返す。
    pub(crate) fn break_after_japanese(&mut self) -> Option<JapaneseBoundary> {
        if self.pending_jfm_continuity == JfmPairContinuity::Broken {
            return None;
        }
        let BoundaryAtom::Japanese(left) = self.previous_main_loop_boundary? else {
            return None;
        };
        self.pending_jfm_continuity = JfmPairContinuity::Broken;
        Some(left)
    }

    /// class 0へ閉じる左半分に存在したJFM spacingを抑止した事実を、右glyphまで保持する。
    pub(crate) fn record_broken_left_jfm_inhibition(&mut self, inhibited: bool) {
        self.broken_left_jfm_was_inhibited |= inhibited;
    }

    /// `\inhibitglue` / `\disinhibitglue`は現在のhlistだけのone-shot状態である。
    pub(crate) fn set_jfm_glue_inhibited(&mut self, inhibited: bool) {
        self.pending_jfm_glue = if inhibited {
            JfmGlueControl::Inhibit
        } else {
            JfmGlueControl::Allow
        };
    }

    pub(crate) const fn pending_jfm_glue(&self) -> JfmGlueControl {
        self.pending_jfm_glue
    }

    /// 実node一個の追加でpendingを消費する。node-less eventはこの入口を呼ばない。
    #[inline]
    pub(crate) fn take_pending_jfm_glue(&mut self) -> JfmGlueControl {
        if self.pending_jfm_glue == JfmGlueControl::Inhibit {
            self.pending_jfm_glue = JfmGlueControl::Allow;
            JfmGlueControl::Inhibit
        } else {
            JfmGlueControl::Allow
        }
    }

    /// 明示nodeや追跡外のbulk appendを越えてmain-loop JFMを作らない。
    pub(crate) fn reset_main_loop_boundary(&mut self) {
        self.previous_main_loop_boundary = None;
        self.pending_jfm_continuity = JfmPairContinuity::Continuous;
        self.broken_left_jfm_was_inhibited = false;
    }

    /// Generic callback is monomorphized; false のとき callback/table lookup/allocation は 0。
    #[inline]
    pub(crate) fn finalize_if_needed<T>(&self, finalize: impl FnOnce() -> T) -> Option<T> {
        self.needs_script_spacing.then(finalize)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JapaneseSpacingPlanner {
    profile: JapaneseSpacingProfile,
}

impl JapaneseSpacingPlanner {
    pub(crate) const fn new(profile: JapaneseSpacingProfile) -> Self {
        Self { profile }
    }

    pub(crate) const fn built_in_ptex() -> Self {
        Self::new(JapaneseSpacingProfile::BuiltInPtex)
    }

    /// 元の二 atom と最終 state だけを読む純粋関数。既存の自動 action は入力に取らない。
    pub(crate) fn plan_boundary(
        self,
        left: BoundaryAtom,
        right: BoundaryAtom,
        context: BoundaryContext,
        state: &PtexSpacingState,
        jfm_pairs: Option<&CompiledJfmPairSpacingTable>,
    ) -> BoundaryActionPlan {
        match self.profile {
            JapaneseSpacingProfile::BuiltInPtex => {
                self.plan_built_in_ptex(left, right, context, state, jfm_pairs)
            }
        }
    }

    /// JFM/禁則だけを文字処理中に決める。K/Xはこの入口から生成しない。
    ///
    /// class 0を挟むJFM二境界と、実文字同士の禁則境界を型で分離し、consumerが
    /// action variantを見て規則を組み直さないようにする。
    pub(crate) fn plan_main_loop_event(
        self,
        event: MainLoopBoundaryEvent,
        jfm_glue: JfmGlueControl,
        jfm_pairs: Option<&CompiledJfmPairSpacingTable>,
    ) -> BoundaryActionPlan {
        let mut plan = BoundaryActionPlan::EMPTY;
        match event {
            MainLoopBoundaryEvent::Direct { left, right } => {
                self.append_profile_kinsoku_action(&mut plan, left, right);
                if let (BoundaryAtom::Japanese(left), BoundaryAtom::Japanese(right)) = (left, right)
                {
                    self.append_jfm_action(&mut plan, left, right, jfm_glue, jfm_pairs);
                }
            }
            MainLoopBoundaryEvent::BreakAfterJapanese { left } => {
                self.append_jfm_action(
                    &mut plan,
                    left,
                    left.default_class_endpoint(),
                    jfm_glue,
                    jfm_pairs,
                );
            }
            MainLoopBoundaryEvent::ResumeBeforeJapanese {
                logical_left,
                right,
            } => {
                self.append_profile_kinsoku_action(
                    &mut plan,
                    BoundaryAtom::Japanese(logical_left),
                    BoundaryAtom::Japanese(right),
                );
                self.append_jfm_action(
                    &mut plan,
                    logical_left.default_class_endpoint(),
                    right,
                    jfm_glue,
                    jfm_pairs,
                );
            }
        }
        plan
    }

    fn append_profile_kinsoku_action(
        self,
        plan: &mut BoundaryActionPlan,
        left: BoundaryAtom,
        right: BoundaryAtom,
    ) {
        let penalty = match self.profile {
            JapaneseSpacingProfile::BuiltInPtex => {
                built_in_kinsoku(left.trailing_character(), KinsokuPosition::After).saturating_add(
                    built_in_kinsoku(right.leading_character(), KinsokuPosition::Before),
                )
            }
        };
        if penalty != 0 {
            plan.push(PlannedSpacingAction::KinsokuPenalty { value: penalty });
        }
    }

    fn append_kinsoku_action(
        self,
        plan: &mut BoundaryActionPlan,
        left: BoundaryAtom,
        right: BoundaryAtom,
        state: &PtexSpacingState,
    ) {
        let penalty = state
            .penalties
            .post(left.trailing_character())
            .saturating_add(state.penalties.pre(right.leading_character()));
        if penalty != 0 {
            plan.push(PlannedSpacingAction::KinsokuPenalty { value: penalty });
        }
    }

    fn append_jfm_action(
        self,
        plan: &mut BoundaryActionPlan,
        left: JapaneseBoundary,
        right: JapaneseBoundary,
        control: JfmGlueControl,
        jfm_pairs: Option<&CompiledJfmPairSpacingTable>,
    ) -> bool {
        let Some(pair_spacing) = self.jfm_pair_spacing(left, right, jfm_pairs) else {
            return false;
        };
        if control == JfmGlueControl::Inhibit {
            plan.jfm_spacing_inhibited = true;
            return true;
        }
        match pair_spacing {
            JfmPairSpacing::Glue(glue) => {
                plan.push(PlannedSpacingAction::JfmGlue { glue });
                true
            }
            JfmPairSpacing::Kern(width) => {
                plan.push(PlannedSpacingAction::JfmKern { width });
                true
            }
        }
    }

    fn jfm_pair_spacing(
        self,
        left: JapaneseBoundary,
        right: JapaneseBoundary,
        jfm_pairs: Option<&CompiledJfmPairSpacingTable>,
    ) -> Option<JfmPairSpacing> {
        (left.font == right.font && left.metric == right.metric)
            .then(|| jfm_pairs.and_then(|table| table.get(left.metric, left.class, right.class)))
            .flatten()
    }

    fn plan_built_in_ptex(
        self,
        left: BoundaryAtom,
        right: BoundaryAtom,
        context: BoundaryContext,
        state: &PtexSpacingState,
        jfm_pairs: Option<&CompiledJfmPairSpacingTable>,
    ) -> BoundaryActionPlan {
        if !left.is_japanese() && !right.is_japanese() {
            return BoundaryActionPlan::EMPTY;
        }

        let mut plan = BoundaryActionPlan::EMPTY;
        self.append_kinsoku_action(&mut plan, left, right, state);

        match (left, right) {
            (BoundaryAtom::Japanese(left), BoundaryAtom::Japanese(right)) => {
                let has_jfm_pair = context.jfm_continuity == JfmPairContinuity::Continuous
                    && self.append_jfm_action(
                        &mut plan,
                        left,
                        right,
                        context.jfm_glue,
                        jfm_pairs,
                    );
                if !has_jfm_pair
                    && context.jfm_continuity != JfmPairContinuity::ReplacedByMainLoopJfm
                {
                    let active = state.auto.kanji();
                    plan.push(PlannedSpacingAction::ImplicitKanjiSkip {
                        glue: if active {
                            state.kanji_skip
                        } else {
                            FixedGlue::ZERO
                        },
                        active,
                    });
                }
            }
            (BoundaryAtom::Japanese(japanese), BoundaryAtom::Latin(latin)) => {
                self.plan_xkanji(
                    &mut plan,
                    japanese,
                    latin,
                    InterScriptDirection::JapaneseToLatin,
                    state,
                );
            }
            (BoundaryAtom::Latin(latin), BoundaryAtom::Japanese(japanese)) => {
                self.plan_xkanji(
                    &mut plan,
                    japanese,
                    latin,
                    InterScriptDirection::LatinToJapanese,
                    state,
                );
            }
            (BoundaryAtom::Latin(_), BoundaryAtom::Latin(_)) => unreachable!(),
        }
        plan
    }

    fn plan_xkanji(
        self,
        plan: &mut BoundaryActionPlan,
        japanese: JapaneseBoundary,
        latin: LatinBoundary,
        direction: InterScriptDirection,
        state: &PtexSpacingState,
    ) {
        let latin_character = match direction {
            InterScriptDirection::JapaneseToLatin => latin.leading_character,
            InterScriptDirection::LatinToJapanese => latin.trailing_character,
        };
        let latin_permission = state.xsp_codes.get(latin_character);
        let japanese_permission = state.inhibit_xsp_codes.get(japanese.character);
        if !latin_permission.allows(direction) || !japanese_permission.allows(direction) {
            return;
        }
        let active = state.auto.xkanji();
        plan.push(PlannedSpacingAction::MaterialXKanjiSkip {
            glue: if active {
                state.xkanji_skip
            } else {
                FixedGlue::ZERO
            },
            active,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glue(width: i32) -> FixedGlue {
        FixedGlue::from_parts(
            width,
            HigherOrderDimension {
                order: DimensionOrder::Normal,
                value: width / 2,
            },
            HigherOrderDimension {
                order: DimensionOrder::Normal,
                value: width / 4,
            },
        )
    }

    fn latin(character: char) -> BoundaryAtom {
        BoundaryAtom::Latin(LatinBoundary::new(LayoutCharacterCode::from_scalar(
            character,
        )))
    }

    fn japanese(character: char, font: u32, metric: u32, class: u8) -> BoundaryAtom {
        BoundaryAtom::Japanese(JapaneseBoundary::new(
            LayoutCharacterCode::from_scalar(character),
            JfmFontId::new(font),
            JfmMetricId::new(metric),
            PlannerJfmClassId::new(class),
        ))
    }

    fn state() -> PtexSpacingState {
        let mut state = PtexSpacingState::initex();
        state.set_kanji_skip(glue(10));
        state.set_xkanji_skip(glue(20));
        state
    }

    #[test]
    fn built_in最小禁則は和文括弧だけをjlreqの行端に固定する() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        let state = PtexSpacingState::built_in_minimal(glue(10), glue(20));
        assert_eq!(
            actions(planner.plan_boundary(
                japanese('あ', 1, 1, 0),
                japanese('。', 1, 1, 0),
                BoundaryContext::DEFAULT,
                &state,
                None,
            )),
            vec![
                PlannedSpacingAction::KinsokuPenalty { value: 10_000 },
                PlannedSpacingAction::ImplicitKanjiSkip {
                    glue: glue(10),
                    active: true,
                },
            ]
        );
        for (opening, closing) in BUILT_IN_HORIZONTAL_JAPANESE_BRACKET_PAIRS {
            assert_eq!(
                state
                    .penalties()
                    .post(LayoutCharacterCode::from_scalar(opening)),
                10_000,
                "U+{:04X}",
                opening as u32
            );
            assert_eq!(
                state
                    .penalties()
                    .pre(LayoutCharacterCode::from_scalar(closing)),
                10_000,
                "U+{:04X}",
                closing as u32
            );
            assert_eq!(
                actions(planner.plan_boundary(
                    japanese(opening, 1, 1, 0),
                    japanese('あ', 1, 1, 0),
                    BoundaryContext::DEFAULT,
                    &state,
                    None,
                ))[0],
                PlannedSpacingAction::KinsokuPenalty { value: 10_000 }
            );
            assert_eq!(
                actions(planner.plan_boundary(
                    japanese('あ', 1, 1, 0),
                    japanese(closing, 1, 1, 0),
                    BoundaryContext::DEFAULT,
                    &state,
                    None,
                ))[0],
                PlannedSpacingAction::KinsokuPenalty { value: 10_000 }
            );
        }
        assert_eq!(state.penalties().len(), 26);

        for (opening, closing) in [
            ('(', ')'),
            ('[', ']'),
            ('{', '}'),
            ('‘', '’'),
            ('“', '”'),
            ('«', '»'),
            ('〝', '〟'),
            ('｟', '｠'),
        ] {
            assert_eq!(
                state
                    .penalties()
                    .post(LayoutCharacterCode::from_scalar(opening)),
                0,
                "excluded U+{:04X}",
                opening as u32
            );
            assert_eq!(
                state
                    .penalties()
                    .pre(LayoutCharacterCode::from_scalar(closing)),
                0,
                "excluded U+{:04X}",
                closing as u32
            );
        }
    }

    #[test]
    fn ligatureは和欧の向きごとに先頭文字と末尾文字を使う() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        let state = state();
        let ligature = BoundaryAtom::Latin(LatinBoundary::ligature(
            LayoutCharacterCode::from_scalar('A'),
            LayoutCharacterCode::from_scalar('/'),
        ));
        assert_eq!(
            actions(planner.plan_boundary(
                japanese('あ', 1, 1, 0),
                ligature,
                BoundaryContext::DEFAULT,
                &state,
                None,
            )),
            vec![PlannedSpacingAction::MaterialXKanjiSkip {
                glue: glue(20),
                active: true,
            }]
        );
        assert!(planner
            .plan_boundary(
                ligature,
                japanese('あ', 1, 1, 0),
                BoundaryContext::DEFAULT,
                &state,
                None,
            )
            .is_empty());
    }

    fn actions(plan: BoundaryActionPlan) -> Vec<PlannedSpacingAction> {
        plan.actions().collect()
    }

    #[test]
    fn xsp公開値を二方向bitへ往復する() {
        for (value, japanese_to_latin, latin_to_japanese) in [
            (0, false, false),
            (1, true, false),
            (2, false, true),
            (3, true, true),
        ] {
            let code = XspCode::from_public_integer(value).unwrap();
            assert_eq!(code.to_public_integer(), value);
            assert_eq!(
                code.allows(InterScriptDirection::JapaneseToLatin),
                japanese_to_latin
            );
            assert_eq!(
                code.allows(InterScriptDirection::LatinToJapanese),
                latin_to_japanese
            );
        }
        assert_eq!(
            XspCode::from_public_integer(-1),
            Err(PtexSpacingCodecError::XspCodeOutOfRange(-1))
        );
        assert_eq!(
            XspCode::from_public_integer(4),
            Err(PtexSpacingCodecError::XspCodeOutOfRange(4))
        );
    }

    #[test]
    fn initexのxsp表は英数字だけ両側を許可する() {
        let table = XspCodeTable::ptex_initex();
        for character in ['0', '9', 'A', 'Z', 'a', 'z'] {
            assert_eq!(
                table.get(LayoutCharacterCode::from_scalar(character)),
                XspCode::BOTH
            );
        }
        for character in ['/', ':', '@', '[', '`', '{', '\u{00ff}'] {
            assert_eq!(
                table.get(LayoutCharacterCode::from_scalar(character)),
                XspCode::NONE
            );
        }
    }

    #[test]
    fn inhibit表は未登録を三とし三の代入で疎表から除く() {
        let character = LayoutCharacterCode::from_scalar('。');
        let mut table = InhibitXspCodeTable::default();
        assert_eq!(table.get(character), XspCode::BOTH);
        assert_eq!(
            table.set(character, XspCode::JAPANESE_TO_LATIN),
            Ok(XspCode::BOTH)
        );
        assert_eq!(table.get(character), XspCode::JAPANESE_TO_LATIN);
        assert_eq!(table.len(), 1);
        assert_eq!(
            table.set(character, XspCode::BOTH),
            Ok(XspCode::JAPANESE_TO_LATIN)
        );
        assert_eq!(table.get(character), XspCode::BOTH);
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn 和和境界は暗黙kを計画し自動間隔を切っても零境界を残す() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        let mut state = state();
        let left = japanese('漢', 1, 1, 0);
        let right = japanese('字', 1, 1, 0);
        assert_eq!(
            actions(planner.plan_boundary(left, right, BoundaryContext::DEFAULT, &state, None)),
            vec![PlannedSpacingAction::ImplicitKanjiSkip {
                glue: glue(10),
                active: true,
            }]
        );

        state.auto_mut().set_kanji(false);
        assert_eq!(
            actions(planner.plan_boundary(left, right, BoundaryContext::DEFAULT, &state, None)),
            vec![PlannedSpacingAction::ImplicitKanjiSkip {
                glue: FixedGlue::ZERO,
                active: false,
            }]
        );
    }

    #[test]
    fn jfm対空白をkより優先し連続性が切れればkへ戻る() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        let state = state();
        let metric = JfmMetricId::new(7);
        let table = CompiledJfmPairSpacingTable::compile(
            metric,
            2,
            &[JfmPairSpacingRule::new(
                PlannerJfmClassId::new(0),
                PlannerJfmClassId::new(1),
                JfmPairSpacing::Glue(glue(33)),
            )],
        )
        .unwrap();
        let left = japanese('）', 4, 7, 0);
        let right = japanese('（', 4, 7, 1);
        assert_eq!(
            actions(planner.plan_boundary(
                left,
                right,
                BoundaryContext::DEFAULT,
                &state,
                Some(&table)
            )),
            vec![PlannedSpacingAction::JfmGlue { glue: glue(33) }]
        );

        assert_eq!(
            actions(planner.plan_boundary(
                left,
                right,
                BoundaryContext {
                    jfm_continuity: JfmPairContinuity::Broken,
                    jfm_glue: JfmGlueControl::Allow,
                },
                &state,
                Some(&table),
            )),
            vec![PlannedSpacingAction::ImplicitKanjiSkip {
                glue: glue(10),
                active: true,
            }]
        );

        let inhibited = planner.plan_boundary(
            left,
            right,
            BoundaryContext {
                jfm_continuity: JfmPairContinuity::Continuous,
                jfm_glue: JfmGlueControl::Inhibit,
            },
            &state,
            Some(&table),
        );
        assert!(inhibited.is_empty());
        assert!(inhibited.jfm_spacing_inhibited());

        assert_eq!(
            actions(planner.plan_boundary(
                left,
                right,
                BoundaryContext {
                    jfm_continuity: JfmPairContinuity::Continuous,
                    jfm_glue: JfmGlueControl::Inhibit,
                },
                &state,
                None,
            )),
            vec![PlannedSpacingAction::ImplicitKanjiSkip {
                glue: glue(10),
                active: true,
            }],
            "抑止対象のJFM pairが無い境界ではKへ戻る"
        );

        assert!(
            planner
                .plan_boundary(
                    left,
                    right,
                    BoundaryContext {
                        jfm_continuity: JfmPairContinuity::ReplacedByMainLoopJfm,
                        jfm_glue: JfmGlueControl::Allow,
                    },
                    &state,
                    Some(&table),
                )
                .is_empty(),
            "class 0側JFMで置換済みの元pairへKを足さない"
        );
    }

    #[test]
    fn font変更ではjfm対を横断せず同じfontのclass変更は表を引く() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        let state = state();
        let table = CompiledJfmPairSpacingTable::compile(
            JfmMetricId::new(8),
            3,
            &[
                JfmPairSpacingRule::new(
                    PlannerJfmClassId::new(0),
                    PlannerJfmClassId::new(1),
                    JfmPairSpacing::Kern(41),
                ),
                JfmPairSpacingRule::new(
                    PlannerJfmClassId::new(0),
                    PlannerJfmClassId::new(2),
                    JfmPairSpacing::Glue(glue(42)),
                ),
            ],
        )
        .unwrap();
        let left = japanese('漢', 10, 8, 0);
        assert_eq!(
            actions(planner.plan_boundary(
                left,
                japanese('、', 10, 8, 1),
                BoundaryContext::DEFAULT,
                &state,
                Some(&table)
            )),
            vec![PlannedSpacingAction::JfmKern { width: 41 }]
        );
        assert_eq!(
            actions(planner.plan_boundary(
                left,
                japanese('。', 10, 8, 2),
                BoundaryContext::DEFAULT,
                &state,
                Some(&table)
            )),
            vec![PlannedSpacingAction::JfmGlue { glue: glue(42) }]
        );
        assert_eq!(
            actions(planner.plan_boundary(
                left,
                japanese('、', 11, 8, 1),
                BoundaryContext::DEFAULT,
                &state,
                Some(&table)
            )),
            vec![PlannedSpacingAction::ImplicitKanjiSkip {
                glue: glue(10),
                active: true,
            }]
        );
    }

    #[test]
    fn 和欧と欧和は左右の許可bitの論理積でxを決める() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        for direction in [
            InterScriptDirection::JapaneseToLatin,
            InterScriptDirection::LatinToJapanese,
        ] {
            for latin_value in 0..=3 {
                for japanese_value in 0..=3 {
                    let mut state = state();
                    state
                        .xsp_codes_mut()
                        .set(
                            b'A' as u32,
                            XspCode::from_public_integer(latin_value).unwrap(),
                        )
                        .unwrap();
                    state
                        .inhibit_xsp_codes_mut()
                        .set(
                            LayoutCharacterCode::from_scalar('漢'),
                            XspCode::from_public_integer(japanese_value).unwrap(),
                        )
                        .unwrap();
                    let (left, right) = match direction {
                        InterScriptDirection::JapaneseToLatin => {
                            (japanese('漢', 1, 1, 0), latin('A'))
                        }
                        InterScriptDirection::LatinToJapanese => {
                            (latin('A'), japanese('漢', 1, 1, 0))
                        }
                    };
                    let plan =
                        planner.plan_boundary(left, right, BoundaryContext::DEFAULT, &state, None);
                    let expected = XspCode::from_public_integer(latin_value)
                        .unwrap()
                        .allows(direction)
                        && XspCode::from_public_integer(japanese_value)
                            .unwrap()
                            .allows(direction);
                    assert_eq!(
                        actions(plan),
                        if expected {
                            vec![PlannedSpacingAction::MaterialXKanjiSkip {
                                glue: glue(20),
                                active: true,
                            }]
                        } else {
                            Vec::new()
                        },
                        "direction={direction:?}, latin={latin_value}, japanese={japanese_value}"
                    );
                }
            }
        }
    }

    #[test]
    fn 自動和欧文間隔を切っても許可された境界へmaterial零xを残す() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        let mut state = state();
        state.auto_mut().set_xkanji(false);
        assert_eq!(
            actions(planner.plan_boundary(
                japanese('漢', 1, 1, 0),
                latin('A'),
                BoundaryContext::DEFAULT,
                &state,
                None
            )),
            vec![PlannedSpacingAction::MaterialXKanjiSkip {
                glue: FixedGlue::ZERO,
                active: false,
            }]
        );
    }

    #[test]
    fn 句読点禁則は左postと右preを合成しjfm空白より先に置く() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        let mut state = state();
        state
            .penalties_mut()
            .set_post(LayoutCharacterCode::from_scalar('（'), 300)
            .unwrap();
        state
            .penalties_mut()
            .set_pre(LayoutCharacterCode::from_scalar('、'), 700)
            .unwrap();
        let table = CompiledJfmPairSpacingTable::compile(
            JfmMetricId::new(2),
            2,
            &[JfmPairSpacingRule::new(
                PlannerJfmClassId::new(0),
                PlannerJfmClassId::new(1),
                JfmPairSpacing::Glue(glue(50)),
            )],
        )
        .unwrap();
        let plan = planner.plan_boundary(
            japanese('（', 1, 2, 0),
            japanese('、', 1, 2, 1),
            BoundaryContext::DEFAULT,
            &state,
            Some(&table),
        );
        assert_eq!(
            actions(plan),
            vec![
                PlannedSpacingAction::KinsokuPenalty { value: 1_000 },
                PlannedSpacingAction::JfmGlue { glue: glue(50) },
            ]
        );
        assert_eq!(
            plan.actions_for_phase(SpacingActionPhase::MainLoop)
                .collect::<Vec<_>>(),
            actions(plan)
        );
        assert_eq!(
            plan.actions_for_phase(SpacingActionPhase::ListFinalizer)
                .count(),
            0
        );
    }

    #[test]
    fn 同じ文字への後の禁則代入が位置を含めて置換する() {
        let character = LayoutCharacterCode::from_scalar('、');
        let mut table = KinsokuPenaltyTable::default();
        assert_eq!(table.set_pre(character, 800), Ok(None));
        assert_eq!(table.pre(character), 800);
        assert_eq!(table.post(character), 0);
        assert_eq!(
            table.set_post(character, 900),
            Ok(Some((KinsokuPosition::Before, 800)))
        );
        assert_eq!(table.pre(character), 0);
        assert_eq!(table.post(character), 900);
        assert_eq!(table.len(), 1);
        assert_eq!(
            table.set_post(character, 0),
            Ok(Some((KinsokuPosition::After, 900)))
        );
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn 境界はlist終端の最終値で冪等に再評価できる() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        let left = japanese('漢', 1, 1, 0);
        let right = latin('A');
        let mut state = state();
        let first = planner.plan_boundary(left, right, BoundaryContext::DEFAULT, &state, None);
        state.set_xkanji_skip(glue(99));
        let final_plan = planner.plan_boundary(left, right, BoundaryContext::DEFAULT, &state, None);
        assert_eq!(
            actions(first),
            vec![PlannedSpacingAction::MaterialXKanjiSkip {
                glue: glue(20),
                active: true,
            }]
        );
        assert_eq!(
            actions(final_plan),
            vec![PlannedSpacingAction::MaterialXKanjiSkip {
                glue: glue(99),
                active: true,
            }]
        );
        assert_eq!(
            final_plan,
            planner.plan_boundary(left, right, BoundaryContext::DEFAULT, &state, None)
        );
    }

    #[test]
    fn xspとinhibitとswitchもlist終端の最終値を使う() {
        let planner = JapaneseSpacingPlanner::built_in_ptex();
        let left = japanese('漢', 1, 1, 0);
        let right = latin('A');
        let mut state = state();

        state
            .xsp_codes_mut()
            .set(b'A' as u32, XspCode::NONE)
            .unwrap();
        assert!(planner
            .plan_boundary(left, right, BoundaryContext::DEFAULT, &state, None)
            .is_empty());

        state
            .xsp_codes_mut()
            .set(b'A' as u32, XspCode::BOTH)
            .unwrap();
        state
            .inhibit_xsp_codes_mut()
            .set(LayoutCharacterCode::from_scalar('漢'), XspCode::NONE)
            .unwrap();
        assert!(planner
            .plan_boundary(left, right, BoundaryContext::DEFAULT, &state, None)
            .is_empty());

        state
            .inhibit_xsp_codes_mut()
            .set(LayoutCharacterCode::from_scalar('漢'), XspCode::BOTH)
            .unwrap();
        state.auto_mut().set_xkanji(false);
        assert_eq!(
            actions(planner.plan_boundary(left, right, BoundaryContext::DEFAULT, &state, None)),
            vec![PlannedSpacingAction::MaterialXKanjiSkip {
                glue: FixedGlue::ZERO,
                active: false,
            }]
        );
    }

    #[test]
    fn 純欧文listはplanner_callbackも表引きも起動しない() {
        let mut list = ScriptSpacingListState::default();
        for character in "plain TeX 123".chars() {
            list.observe(latin(character));
        }
        let mut calls = 0;
        let result = list.finalize_if_needed(|| {
            calls += 1;
            JapaneseSpacingPlanner::built_in_ptex()
        });
        assert_eq!(result, None);
        assert_eq!(calls, 0);
        assert!(!list.needs_script_spacing());

        list.observe(japanese('漢', 1, 1, 0));
        assert!(list.finalize_if_needed(|| 17).is_some());
    }

    #[test]
    fn jfm対表は範囲外と重複をcompile時に拒む() {
        let metric = JfmMetricId::new(1);
        assert_eq!(
            CompiledJfmPairSpacingTable::compile(metric, 0, &[]),
            Err(JfmPairSpacingTableError::EmptyClassSet)
        );
        assert_eq!(
            CompiledJfmPairSpacingTable::compile(metric, MAX_JFM_CLASSES + 1, &[]),
            Err(JfmPairSpacingTableError::TooManyClasses {
                actual: MAX_JFM_CLASSES + 1,
                maximum: MAX_JFM_CLASSES,
            })
        );
        assert_eq!(
            CompiledJfmPairSpacingTable::compile(
                metric,
                1,
                &[JfmPairSpacingRule::new(
                    PlannerJfmClassId::new(0),
                    PlannerJfmClassId::new(1),
                    JfmPairSpacing::Kern(0),
                )]
            ),
            Err(JfmPairSpacingTableError::ClassOutOfBounds {
                rule_index: 0,
                class: 1,
                class_count: 1,
            })
        );
        let duplicate = JfmPairSpacingRule::new(
            PlannerJfmClassId::new(0),
            PlannerJfmClassId::new(0),
            JfmPairSpacing::Kern(1),
        );
        assert_eq!(
            CompiledJfmPairSpacingTable::compile(metric, 1, &[duplicate, duplicate]),
            Err(JfmPairSpacingTableError::DuplicateRule {
                first_rule_index: 0,
                second_rule_index: 1,
            })
        );
    }

    #[test]
    fn unicode公開codecは代理対と範囲外を拒む() {
        assert_eq!(
            LayoutCharacterCode::from_public_integer('漢' as u32),
            Ok(LayoutCharacterCode::from_scalar('漢'))
        );
        assert_eq!(
            LayoutCharacterCode::from_public_integer(0xd800),
            Err(PtexSpacingCodecError::NonUnicodeCharacter(0xd800))
        );
        assert_eq!(
            LayoutCharacterCode::from_public_integer(0x11_0000),
            Err(PtexSpacingCodecError::NonUnicodeCharacter(0x11_0000))
        );
    }

    #[test]
    fn ptex間隔stateを版付きfmtで全成分往復する() {
        let mut state = state();
        state.auto = AutoSpacingState::new(false, true);
        state.set_kanji_skip(FixedGlue::from_parts(
            11,
            HigherOrderDimension {
                order: DimensionOrder::Fil,
                value: 12,
            },
            HigherOrderDimension {
                order: DimensionOrder::Fill,
                value: 13,
            },
        ));
        state.set_xkanji_skip(FixedGlue::from_parts(
            14,
            HigherOrderDimension {
                order: DimensionOrder::Filll,
                value: 15,
            },
            HigherOrderDimension {
                order: DimensionOrder::Fil,
                value: 16,
            },
        ));
        state
            .xsp_codes_mut()
            .set(b'+' as u32, XspCode::JAPANESE_TO_LATIN)
            .unwrap();
        state
            .inhibit_xsp_codes_mut()
            .set(
                LayoutCharacterCode::from_scalar('。'),
                XspCode::LATIN_TO_JAPANESE,
            )
            .unwrap();
        state
            .penalties_mut()
            .set_post(LayoutCharacterCode::from_scalar('（'), 10_000)
            .unwrap();

        let mut dumped = Vec::new();
        state.dump(&mut dumped).unwrap();
        let text = String::from_utf8(dumped).unwrap();
        let mut lines = text.lines();
        let loaded = PtexSpacingState::undump(&mut lines).unwrap();
        assert_eq!(loaded, state);
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn fmtは不正codecと巨大疎表と非昇順entryを拒む() {
        let mut bad_xsp = "4".lines();
        assert!(matches!(
            XspCode::undump(&mut bad_xsp),
            Err(FormatError::ParseError)
        ));

        let too_many = (MAX_INHIBIT_XSP_CODES + 1).to_string();
        assert!(matches!(
            InhibitXspCodeTable::undump(&mut too_many.lines()),
            Err(FormatError::ParseError)
        ));

        let mut descending = "2\n12290\n1\n12289\n2".lines();
        assert!(matches!(
            InhibitXspCodeTable::undump(&mut descending),
            Err(FormatError::ParseError)
        ));

        let mut wrong_header = "ptex-spacing-state-v0".lines();
        assert!(matches!(
            PtexSpacingState::undump(&mut wrong_header),
            Err(FormatError::ParseError)
        ));
    }
}

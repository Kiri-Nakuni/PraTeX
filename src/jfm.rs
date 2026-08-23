//! 和文フォントメトリクス (JFM) の境界付き parser と中間表現。
//!
//! 日本語 TeX 開発コミュニティの公開仕様だけを入力として実装する。
//! <https://tug.ctan.org/info/ptex-manual/jfm.pdf>
//!
//! JFM は拡張子を TFM と共有するが、先頭の識別子と表構成が異なる。既存の
//! 8-bit TFM reader に判定を散らさず、ここで全長と全参照を検査してから
//! `FontInfo` へ渡せる形にする。

use crate::format::{Dumpable, FormatError};
use crate::scaled::Scaled;

use std::fmt;
use std::io::Write;

const HORIZONTAL_JFM_ID: u16 = 11;
const VERTICAL_JFM_ID: u16 = 9;
const SIZE_FIELD_WORDS: usize = 7;
const MINIMUM_HEADER_WORDS: usize = 2;
const FIX_WORD_FRACTION_BITS: u32 = 20;
const MAX_CHARACTER_TYPE: u16 = 255;
// `lf < 2^15`でkern以外に最低15語を要するため、実在indexは0x7fffへ届かない。
// 最上位bitを種類、0xffffを規則なしに使っても衝突しない。
const NO_PAIR_ADJUSTMENT: u16 = u16::MAX;
const KERN_PAIR_ADJUSTMENT: u16 = 1 << 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JfmDirection {
    Horizontal,
    Vertical,
}

/// JFM の文字タイプ。kcatcode や将来の組版 script class とは混同しない。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JfmClassId(u8);

impl JfmClassId {
    const fn index(self) -> usize {
        self.0 as usize
    }

    pub(crate) const fn number(self) -> u8 {
        self.0
    }
}

impl Dumpable for JfmClassId {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self(u8::undump(lines)?))
    }
}

/// JFM の 12.20 fixed word を丸めずに保持する。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JfmFixWord(i32);

impl JfmFixWord {
    pub(crate) const ZERO: Self = Self(0);

    pub(crate) const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    pub(crate) const fn raw(self) -> i32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JfmGlue {
    pub(crate) width: JfmFixWord,
    pub(crate) stretch: JfmFixWord,
    pub(crate) shrink: JfmFixWord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JfmAdjustment {
    Glue(JfmGlue),
    Kern(JfmFixWord),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JfmCharTag {
    None,
    GlueKern(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JfmCharInfo {
    pub(crate) width_index: usize,
    pub(crate) height_index: usize,
    pub(crate) depth_index: usize,
    pub(crate) italic_index: usize,
    pub(crate) tag: JfmCharTag,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CharacterTypeEntry {
    character_code: u32,
    class: JfmClassId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GlueKernStep {
    skip: u8,
    next_character_type: u8,
    operation: u8,
    remainder: u8,
}

/// 検査済みの JFM。表への index は parse 時にすべて範囲検査されている。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Jfm {
    pub(crate) direction: JfmDirection,
    pub(crate) check: u32,
    /// TeX の scaled point 表現へ 12.20 から下位4 bitを落として移した design size。
    pub(crate) design_size: Scaled,
    character_types: Vec<CharacterTypeEntry>,
    pub(crate) char_infos: Vec<JfmCharInfo>,
    pub(crate) widths: Vec<JfmFixWord>,
    pub(crate) heights: Vec<JfmFixWord>,
    pub(crate) depths: Vec<JfmFixWord>,
    pub(crate) italics: Vec<JfmFixWord>,
    pub(crate) kerns: Vec<JfmFixWord>,
    pub(crate) glues: Vec<JfmGlue>,
    pub(crate) params: Vec<JfmFixWord>,
    /// 行優先の class 対表。JFM program は font 読み込み時に一度だけ実行する。
    pair_adjustments: Vec<u16>,
}

impl Jfm {
    pub(crate) fn parse(input: &[u8]) -> Result<Self, JfmParseError> {
        let mut cursor = Cursor::new(input);
        let header = SizeFields::read(&mut cursor)?;
        header.validate(input.len())?;

        let check = u32::from_be_bytes(cursor.read_word()?);
        let design_size_raw = u32::from_be_bytes(cursor.read_word()?);
        if design_size_raw & 0x8000_0000 != 0 || design_size_raw < (1_u32 << FIX_WORD_FRACTION_BITS)
        {
            return Err(JfmParseError::InvalidDesignSize {
                raw: design_size_raw,
            });
        }
        let design_size = (design_size_raw >> 4) as Scaled;
        for _ in MINIMUM_HEADER_WORDS..header.lh {
            cursor.read_word()?;
        }

        let character_types = read_character_types(&mut cursor, &header)?;
        let char_infos = read_character_infos(&mut cursor, &header)?;
        let widths = read_relative_fix_words(&mut cursor, header.nw, "width")?;
        let heights = read_relative_fix_words(&mut cursor, header.nh, "height")?;
        let depths = read_relative_fix_words(&mut cursor, header.nd, "depth")?;
        let italics = read_relative_fix_words(&mut cursor, header.ni, "italic")?;
        validate_zero_metric("width", &widths)?;
        validate_zero_metric("height", &heights)?;
        validate_zero_metric("depth", &depths)?;
        validate_zero_metric("italic", &italics)?;

        let glue_kern_steps = read_glue_kern_steps(&mut cursor, header.nl)?;
        let kerns = read_relative_fix_words(&mut cursor, header.nk, "kern")?;
        let glues = read_glues(&mut cursor, header.ng)?;
        let params = read_parameters(&mut cursor, header.np)?;

        let pair_adjustments = compile_glue_kern_programs(
            &char_infos,
            &glue_kern_steps,
            kerns.len(),
            glues.len(),
            header.ec as u8,
        )?;

        if cursor.offset != input.len() {
            return Err(JfmParseError::InternalLengthMismatch {
                consumed: cursor.offset,
                length: input.len(),
            });
        }

        Ok(Self {
            direction: header.direction,
            check,
            design_size,
            character_types,
            char_infos,
            widths,
            heights,
            depths,
            italics,
            kerns,
            glues,
            params,
            pair_adjustments,
        })
    }

    /// 表に無い raw 文字コードは JFM 仕様どおり文字タイプ0になる。
    ///
    /// JFM 自体はこの24-bit値が Unicode か JIS かを自己記述しない。入力文字から
    /// raw code への変換は、和文 font を定義する側が明示した encoding の責任とする。
    pub(crate) fn class_of_raw_code(&self, character_code: u32) -> JfmClassId {
        if character_code > 0x00ff_ffff {
            return JfmClassId(0);
        }
        self.character_types
            .binary_search_by_key(&character_code, |entry| entry.character_code)
            .map(|index| self.character_types[index].class)
            .unwrap_or(JfmClassId(0))
    }

    pub(crate) fn char_info(&self, class: JfmClassId) -> &JfmCharInfo {
        &self.char_infos[class.index()]
    }

    pub(crate) fn relative_width(&self, class: JfmClassId) -> JfmFixWord {
        self.widths[self.char_info(class).width_index]
    }

    pub(crate) fn relative_height(&self, class: JfmClassId) -> JfmFixWord {
        self.heights[self.char_info(class).height_index]
    }

    pub(crate) fn relative_depth(&self, class: JfmClassId) -> JfmFixWord {
        self.depths[self.char_info(class).depth_index]
    }

    pub(crate) fn relative_italic(&self, class: JfmClassId) -> JfmFixWord {
        self.italics[self.char_info(class).italic_index]
    }

    /// `zw` は parameter 6 ではなく、実際の pTeX の挙動に合わせて文字タイプ0の幅。
    pub(crate) fn relative_zw(&self) -> JfmFixWord {
        self.widths[self.char_infos[0].width_index]
    }

    pub(crate) fn relative_default_kanji_skip(&self) -> Option<JfmGlue> {
        Some(JfmGlue {
            width: *self.params.get(1)?,
            stretch: *self.params.get(2)?,
            shrink: *self.params.get(3)?,
        })
    }

    pub(crate) fn relative_default_xkanji_skip(&self) -> Option<JfmGlue> {
        Some(JfmGlue {
            width: *self.params.get(6)?,
            stretch: *self.params.get(7)?,
            shrink: *self.params.get(8)?,
        })
    }

    pub(crate) fn adjustment_for_codes(
        &self,
        left_character_code: u32,
        right_character_code: u32,
    ) -> Option<JfmAdjustment> {
        self.adjustment_by_class(
            self.class_of_raw_code(left_character_code),
            self.class_of_raw_code(right_character_code),
        )
    }

    /// 組版の hot path は raw code を再検索せず、wide glyph node に保持した class で引く。
    pub(crate) fn adjustment_by_class(
        &self,
        left: JfmClassId,
        right: JfmClassId,
    ) -> Option<JfmAdjustment> {
        let class_count = self.char_infos.len();
        let encoded = *self.pair_adjustments.get(
            left.index()
                .checked_mul(class_count)?
                .checked_add(right.index())?,
        )?;
        if encoded == NO_PAIR_ADJUSTMENT {
            None
        } else if encoded & KERN_PAIR_ADJUSTMENT == 0 {
            self.glues
                .get(encoded as usize)
                .copied()
                .map(JfmAdjustment::Glue)
        } else {
            self.kerns
                .get((encoded & !KERN_PAIR_ADJUSTMENT) as usize)
                .copied()
                .map(JfmAdjustment::Kern)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SizeFields {
    direction: JfmDirection,
    nt: usize,
    lf: usize,
    lh: usize,
    bc: u16,
    ec: u16,
    nw: usize,
    nh: usize,
    nd: usize,
    ni: usize,
    nl: usize,
    nk: usize,
    ng: usize,
    np: usize,
}

impl SizeFields {
    fn read(cursor: &mut Cursor<'_>) -> Result<Self, JfmParseError> {
        let id = cursor.read_small_halfword("id")?;
        let direction = match id {
            HORIZONTAL_JFM_ID => JfmDirection::Horizontal,
            VERTICAL_JFM_ID => JfmDirection::Vertical,
            _ => return Err(JfmParseError::UnknownDirectionId { id }),
        };
        Ok(Self {
            direction,
            nt: cursor.read_small_halfword("nt")? as usize,
            lf: cursor.read_small_halfword("lf")? as usize,
            lh: cursor.read_small_halfword("lh")? as usize,
            bc: cursor.read_small_halfword("bc")?,
            ec: cursor.read_small_halfword("ec")?,
            nw: cursor.read_small_halfword("nw")? as usize,
            nh: cursor.read_small_halfword("nh")? as usize,
            nd: cursor.read_small_halfword("nd")? as usize,
            ni: cursor.read_small_halfword("ni")? as usize,
            nl: cursor.read_small_halfword("nl")? as usize,
            nk: cursor.read_small_halfword("nk")? as usize,
            ng: cursor.read_small_halfword("ng")? as usize,
            np: cursor.read_small_halfword("np")? as usize,
        })
    }

    fn validate(&self, actual_bytes: usize) -> Result<(), JfmParseError> {
        if self.nt == 0 {
            return Err(JfmParseError::EmptyCharacterTypeTable);
        }
        if self.bc != 0 || self.ec > MAX_CHARACTER_TYPE {
            return Err(JfmParseError::InvalidCharacterTypeBounds {
                bc: self.bc,
                ec: self.ec,
            });
        }
        if self.lh < MINIMUM_HEADER_WORDS {
            return Err(JfmParseError::HeaderTooShort { words: self.lh });
        }
        for (table, words) in [
            ("width", self.nw),
            ("height", self.nh),
            ("depth", self.nd),
            ("italic", self.ni),
        ] {
            if words == 0 {
                return Err(JfmParseError::EmptyMetricTable { table });
            }
        }
        if self.ng % 3 != 0 {
            return Err(JfmParseError::InvalidGlueTableLength { words: self.ng });
        }

        let char_info_words = self.ec as usize + 1;
        let computed_words = [
            SIZE_FIELD_WORDS,
            self.nt,
            self.lh,
            char_info_words,
            self.nw,
            self.nh,
            self.nd,
            self.ni,
            self.nl,
            self.nk,
            self.ng,
            self.np,
        ]
        .into_iter()
        .sum();
        if self.lf != computed_words {
            return Err(JfmParseError::InvalidSizeLayout {
                declared_words: self.lf,
                computed_words,
            });
        }

        let declared_bytes = self.lf * 4;
        if declared_bytes != actual_bytes {
            return Err(JfmParseError::FileLengthMismatch {
                declared_words: self.lf,
                actual_bytes,
            });
        }
        Ok(())
    }
}

fn read_character_types(
    cursor: &mut Cursor<'_>,
    header: &SizeFields,
) -> Result<Vec<CharacterTypeEntry>, JfmParseError> {
    let mut entries: Vec<CharacterTypeEntry> = Vec::with_capacity(header.nt);
    for index in 0..header.nt {
        let [middle, low, high, character_type] = cursor.read_word()?;
        let character_code = (u32::from(high) << 16) | (u32::from(middle) << 8) | u32::from(low);
        if index == 0 && (character_code != 0 || character_type != 0) {
            return Err(JfmParseError::InvalidDefaultCharacterType {
                character_code,
                character_type,
            });
        }
        if let Some(previous) = entries.last() {
            if character_code <= previous.character_code {
                return Err(JfmParseError::CharacterTypesNotSorted {
                    index,
                    previous: previous.character_code,
                    current: character_code,
                });
            }
        }
        if u16::from(character_type) > header.ec {
            return Err(JfmParseError::CharacterTypeOutOfRange {
                index,
                character_type,
                maximum: header.ec as u8,
            });
        }
        entries.push(CharacterTypeEntry {
            character_code,
            class: JfmClassId(character_type),
        });
    }
    Ok(entries)
}

fn read_character_infos(
    cursor: &mut Cursor<'_>,
    header: &SizeFields,
) -> Result<Vec<JfmCharInfo>, JfmParseError> {
    let mut infos = Vec::with_capacity(header.ec as usize + 1);
    for character_type in 0..=header.ec as u8 {
        let [width, height_depth, italic_tag, remainder] = cursor.read_word()?;
        let width_index = width as usize;
        let height_index = (height_depth >> 4) as usize;
        let depth_index = (height_depth & 0x0f) as usize;
        let italic_index = (italic_tag >> 2) as usize;
        for (table, index, length) in [
            ("width", width_index, header.nw),
            ("height", height_index, header.nh),
            ("depth", depth_index, header.nd),
            ("italic", italic_index, header.ni),
        ] {
            if index >= length {
                return Err(JfmParseError::MetricIndexOutOfRange {
                    character_type,
                    table,
                    index,
                    length,
                });
            }
        }

        let tag = match italic_tag & 0x03 {
            0 => JfmCharTag::None,
            1 => {
                let start = remainder as usize;
                if start >= header.nl {
                    return Err(JfmParseError::ProgramStartOutOfRange {
                        character_type,
                        start,
                        length: header.nl,
                    });
                }
                JfmCharTag::GlueKern(start)
            }
            tag => {
                return Err(JfmParseError::UnsupportedCharacterTag {
                    character_type,
                    tag,
                })
            }
        };
        infos.push(JfmCharInfo {
            width_index,
            height_index,
            depth_index,
            italic_index,
            tag,
        });
    }
    Ok(infos)
}

fn read_relative_fix_words(
    cursor: &mut Cursor<'_>,
    count: usize,
    table: &'static str,
) -> Result<Vec<JfmFixWord>, JfmParseError> {
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let bytes = cursor.read_word()?;
        if bytes[0] != 0 && bytes[0] != 255 {
            return Err(JfmParseError::InvalidRelativeFixWord {
                table,
                index,
                raw: i32::from_be_bytes(bytes),
            });
        }
        values.push(JfmFixWord::from_raw(i32::from_be_bytes(bytes)));
    }
    Ok(values)
}

fn validate_zero_metric(table: &'static str, values: &[JfmFixWord]) -> Result<(), JfmParseError> {
    if values.first() != Some(&JfmFixWord::ZERO) {
        return Err(JfmParseError::NonzeroFirstMetric {
            table,
            raw: values[0].raw(),
        });
    }
    Ok(())
}

fn read_glue_kern_steps(
    cursor: &mut Cursor<'_>,
    count: usize,
) -> Result<Vec<GlueKernStep>, JfmParseError> {
    let mut steps = Vec::with_capacity(count);
    for _ in 0..count {
        let [skip, next_character_type, operation, remainder] = cursor.read_word()?;
        steps.push(GlueKernStep {
            skip,
            next_character_type,
            operation,
            remainder,
        });
    }
    Ok(steps)
}

fn read_glues(cursor: &mut Cursor<'_>, words: usize) -> Result<Vec<JfmGlue>, JfmParseError> {
    let mut glues = Vec::with_capacity(words / 3);
    for index in 0..words / 3 {
        let width = read_one_relative_fix_word(cursor, "glue width", index)?;
        let stretch = read_one_relative_fix_word(cursor, "glue stretch", index)?;
        let shrink = read_one_relative_fix_word(cursor, "glue shrink", index)?;
        glues.push(JfmGlue {
            width,
            stretch,
            shrink,
        });
    }
    Ok(glues)
}

fn read_one_relative_fix_word(
    cursor: &mut Cursor<'_>,
    table: &'static str,
    index: usize,
) -> Result<JfmFixWord, JfmParseError> {
    let bytes = cursor.read_word()?;
    if bytes[0] != 0 && bytes[0] != 255 {
        return Err(JfmParseError::InvalidRelativeFixWord {
            table,
            index,
            raw: i32::from_be_bytes(bytes),
        });
    }
    Ok(JfmFixWord::from_raw(i32::from_be_bytes(bytes)))
}

fn read_parameters(
    cursor: &mut Cursor<'_>,
    count: usize,
) -> Result<Vec<JfmFixWord>, JfmParseError> {
    let mut params = Vec::with_capacity(count);
    for index in 0..count {
        let bytes = cursor.read_word()?;
        if index > 0 && bytes[0] != 0 && bytes[0] != 255 {
            return Err(JfmParseError::InvalidRelativeFixWord {
                table: "parameter",
                index,
                raw: i32::from_be_bytes(bytes),
            });
        }
        params.push(JfmFixWord::from_raw(i32::from_be_bytes(bytes)));
    }
    Ok(params)
}

fn compile_glue_kern_programs(
    infos: &[JfmCharInfo],
    steps: &[GlueKernStep],
    kern_count: usize,
    glue_count: usize,
    maximum_character_type: u8,
) -> Result<Vec<u16>, JfmParseError> {
    let class_count = infos.len();
    let mut compiled_starts: Vec<Option<Vec<u16>>> = vec![None; steps.len()];
    let mut pairs = vec![NO_PAIR_ADJUSTMENT; class_count * class_count];
    for (left, info) in infos.iter().enumerate() {
        let JfmCharTag::GlueKern(start) = info.tag else {
            continue;
        };
        let row = &mut pairs[left * class_count..(left + 1) * class_count];
        if let Some(compiled) = &compiled_starts[start] {
            row.copy_from_slice(compiled);
        } else {
            let compiled = compile_glue_kern_program(
                start,
                steps,
                kern_count,
                glue_count,
                maximum_character_type,
                class_count,
            )?;
            row.copy_from_slice(&compiled);
            compiled_starts[start] = Some(compiled);
        }
    }
    Ok(pairs)
}

fn compile_glue_kern_program(
    start: usize,
    steps: &[GlueKernStep],
    kern_count: usize,
    glue_count: usize,
    maximum_character_type: u8,
    class_count: usize,
) -> Result<Vec<u16>, JfmParseError> {
    let mut row = vec![NO_PAIR_ADJUSTMENT; class_count];
    let first = steps[start];
    let mut position = if first.skip > 128 {
        let target = glue_kern_index(first.operation, first.remainder);
        if target >= steps.len() {
            return Err(JfmParseError::RelocationTargetOutOfRange {
                start,
                target,
                length: steps.len(),
            });
        }
        target
    } else {
        start
    };

    loop {
        let step = steps[position];
        if step.skip > 128 {
            return Ok(row);
        }
        if step.next_character_type > maximum_character_type {
            return Err(JfmParseError::ProgramCharacterTypeOutOfRange {
                position,
                character_type: step.next_character_type,
                maximum: maximum_character_type,
            });
        }
        let adjustment = if step.operation <= 127 {
            let index = glue_kern_index(step.operation, step.remainder);
            if index >= glue_count {
                return Err(JfmParseError::GlueIndexOutOfRange {
                    position,
                    index,
                    length: glue_count,
                });
            }
            index as u16
        } else {
            let index = glue_kern_index(step.operation - 128, step.remainder);
            if index >= kern_count {
                return Err(JfmParseError::KernIndexOutOfRange {
                    position,
                    index,
                    length: kern_count,
                });
            }
            KERN_PAIR_ADJUSTMENT | index as u16
        };
        let slot = &mut row[step.next_character_type as usize];
        if *slot == NO_PAIR_ADJUSTMENT {
            *slot = adjustment;
        }
        if step.skip == 128 {
            return Ok(row);
        }
        let next = position + step.skip as usize + 1;
        if next >= steps.len() {
            return Err(JfmParseError::ProgramJumpOutOfRange {
                position,
                target: next,
                length: steps.len(),
            });
        }
        position = next;
    }
}

const fn glue_kern_index(high: u8, low: u8) -> usize {
    high as usize * 256 + low as usize
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn read_small_halfword(&mut self, field: &'static str) -> Result<u16, JfmParseError> {
        let bytes = self.read_bytes::<2>()?;
        let value = u16::from_be_bytes(bytes);
        if value >= 1 << 15 {
            return Err(JfmParseError::HalfwordOutOfRange { field, value });
        }
        Ok(value)
    }

    fn read_word(&mut self) -> Result<[u8; 4], JfmParseError> {
        self.read_bytes::<4>()
    }

    fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N], JfmParseError> {
        let end = self.offset.checked_add(N).ok_or(JfmParseError::Truncated {
            offset: self.offset,
            needed: N,
            length: self.input.len(),
        })?;
        let source = self
            .input
            .get(self.offset..end)
            .ok_or(JfmParseError::Truncated {
                offset: self.offset,
                needed: N,
                length: self.input.len(),
            })?;
        let mut bytes = [0; N];
        bytes.copy_from_slice(source);
        self.offset = end;
        Ok(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum JfmParseError {
    Truncated {
        offset: usize,
        needed: usize,
        length: usize,
    },
    HalfwordOutOfRange {
        field: &'static str,
        value: u16,
    },
    UnknownDirectionId {
        id: u16,
    },
    EmptyCharacterTypeTable,
    InvalidCharacterTypeBounds {
        bc: u16,
        ec: u16,
    },
    HeaderTooShort {
        words: usize,
    },
    EmptyMetricTable {
        table: &'static str,
    },
    InvalidGlueTableLength {
        words: usize,
    },
    InvalidSizeLayout {
        declared_words: usize,
        computed_words: usize,
    },
    FileLengthMismatch {
        declared_words: usize,
        actual_bytes: usize,
    },
    InvalidDesignSize {
        raw: u32,
    },
    InvalidDefaultCharacterType {
        character_code: u32,
        character_type: u8,
    },
    CharacterTypesNotSorted {
        index: usize,
        previous: u32,
        current: u32,
    },
    CharacterTypeOutOfRange {
        index: usize,
        character_type: u8,
        maximum: u8,
    },
    MetricIndexOutOfRange {
        character_type: u8,
        table: &'static str,
        index: usize,
        length: usize,
    },
    UnsupportedCharacterTag {
        character_type: u8,
        tag: u8,
    },
    ProgramStartOutOfRange {
        character_type: u8,
        start: usize,
        length: usize,
    },
    InvalidRelativeFixWord {
        table: &'static str,
        index: usize,
        raw: i32,
    },
    NonzeroFirstMetric {
        table: &'static str,
        raw: i32,
    },
    RelocationTargetOutOfRange {
        start: usize,
        target: usize,
        length: usize,
    },
    ProgramCharacterTypeOutOfRange {
        position: usize,
        character_type: u8,
        maximum: u8,
    },
    GlueIndexOutOfRange {
        position: usize,
        index: usize,
        length: usize,
    },
    KernIndexOutOfRange {
        position: usize,
        index: usize,
        length: usize,
    },
    ProgramJumpOutOfRange {
        position: usize,
        target: usize,
        length: usize,
    },
    InternalLengthMismatch {
        consumed: usize,
        length: usize,
    },
}

impl fmt::Display for JfmParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated {
                offset,
                needed,
                length,
            } => write!(
                formatter,
                "JFM needs {needed} bytes at byte {offset}, but its length is {length}"
            ),
            Self::HalfwordOutOfRange { field, value } => {
                write!(formatter, "JFM size field {field} has its high bit set: {value}")
            }
            Self::UnknownDirectionId { id } => {
                write!(formatter, "JFM direction identifier is {id}, not 9 or 11")
            }
            Self::EmptyCharacterTypeTable => {
                formatter.write_str("JFM character type table is empty")
            }
            Self::InvalidCharacterTypeBounds { bc, ec } => write!(
                formatter,
                "JFM character type bounds must be bc=0 and ec<=255, not {bc}..{ec}"
            ),
            Self::HeaderTooShort { words } => {
                write!(formatter, "JFM header has {words} words; at least 2 are required")
            }
            Self::EmptyMetricTable { table } => {
                write!(formatter, "JFM {table} table is empty")
            }
            Self::InvalidGlueTableLength { words } => write!(
                formatter,
                "JFM glue table has {words} words, which is not divisible by 3"
            ),
            Self::InvalidSizeLayout {
                declared_words,
                computed_words,
            } => write!(
                formatter,
                "JFM declares {declared_words} words but its table sizes total {computed_words}"
            ),
            Self::FileLengthMismatch {
                declared_words,
                actual_bytes,
            } => write!(
                formatter,
                "JFM declares {declared_words} words but contains {actual_bytes} bytes"
            ),
            Self::InvalidDesignSize { raw } => {
                write!(formatter, "JFM design size 0x{raw:08x} is invalid")
            }
            Self::InvalidDefaultCharacterType {
                character_code,
                character_type,
            } => write!(
                formatter,
                "JFM default character code is 0x{character_code:06X}/type {character_type}, not zero"
            ),
            Self::CharacterTypesNotSorted {
                index,
                previous,
                current,
            } => write!(
                formatter,
                "JFM character type entry {index} has raw code 0x{current:06X} after 0x{previous:06X}"
            ),
            Self::CharacterTypeOutOfRange {
                index,
                character_type,
                maximum,
            } => write!(
                formatter,
                "JFM character type entry {index} uses type {character_type}, above {maximum}"
            ),
            Self::MetricIndexOutOfRange {
                character_type,
                table,
                index,
                length,
            } => write!(
                formatter,
                "JFM type {character_type} uses {table}[{index}], but its length is {length}"
            ),
            Self::UnsupportedCharacterTag {
                character_type,
                tag,
            } => write!(
                formatter,
                "JFM type {character_type} uses reserved char_info tag {tag}"
            ),
            Self::ProgramStartOutOfRange {
                character_type,
                start,
                length,
            } => write!(
                formatter,
                "JFM type {character_type} starts glue/kern program at {start}, but its length is {length}"
            ),
            Self::InvalidRelativeFixWord { table, index, raw } => write!(
                formatter,
                "JFM {table}[{index}] has out-of-range relative fix_word 0x{:08x}",
                *raw as u32
            ),
            Self::NonzeroFirstMetric { table, raw } => write!(
                formatter,
                "JFM {table}[0] is 0x{:08x}, not zero",
                *raw as u32
            ),
            Self::RelocationTargetOutOfRange {
                start,
                target,
                length,
            } => write!(
                formatter,
                "JFM glue/kern program {start} relocates to {target}, but its length is {length}"
            ),
            Self::ProgramCharacterTypeOutOfRange {
                position,
                character_type,
                maximum,
            } => write!(
                formatter,
                "JFM glue/kern step {position} matches type {character_type}, above {maximum}"
            ),
            Self::GlueIndexOutOfRange {
                position,
                index,
                length,
            } => write!(
                formatter,
                "JFM glue/kern step {position} uses glue {index}, but there are {length}"
            ),
            Self::KernIndexOutOfRange {
                position,
                index,
                length,
            } => write!(
                formatter,
                "JFM glue/kern step {position} uses kern {index}, but there are {length}"
            ),
            Self::ProgramJumpOutOfRange {
                position,
                target,
                length,
            } => write!(
                formatter,
                "JFM glue/kern step {position} jumps to {target}, but its length is {length}"
            ),
            Self::InternalLengthMismatch { consumed, length } => write!(
                formatter,
                "JFM parser consumed {consumed} of {length} bytes after validating its layout"
            ),
        }
    }
}

impl std::error::Error for JfmParseError {}

#[cfg(test)]
mod tests {
    use super::{
        Jfm, JfmAdjustment, JfmClassId, JfmDirection, JfmFixWord, JfmGlue, JfmParseError,
        HORIZONTAL_JFM_ID, VERTICAL_JFM_ID,
    };

    const ZERO: i32 = 0;
    const HALF: i32 = 0x0008_0000;
    const ONE: i32 = 0x0010_0000;
    const QUARTER: i32 = 0x0004_0000;

    #[derive(Clone)]
    struct Fixture {
        direction: u16,
        char_types: Vec<[u8; 4]>,
        char_infos: Vec<[u8; 4]>,
        widths: Vec<i32>,
        heights: Vec<i32>,
        depths: Vec<i32>,
        italics: Vec<i32>,
        steps: Vec<[u8; 4]>,
        kerns: Vec<i32>,
        glue_words: Vec<i32>,
        params: Vec<i32>,
    }

    impl Fixture {
        fn 現行仕様() -> Self {
            Self {
                direction: HORIZONTAL_JFM_ID,
                char_types: vec![
                    char_type(0, 0),
                    char_type(0x003001, 1),
                    char_type(0x01f600, 2),
                    char_type(0xabcdef, 2),
                ],
                char_infos: vec![[1, 0x10, 1, 0], [2, 0x10, 0, 0], [1, 0x10, 0, 0]],
                widths: vec![ZERO, ONE, HALF],
                heights: vec![ZERO, HALF],
                depths: vec![ZERO],
                italics: vec![ZERO],
                steps: vec![
                    [129, 0, 0, 2],
                    [129, 0, 0, 0],
                    [0, 1, 0, 0],
                    [128, 2, 128, 0],
                ],
                kerns: vec![-QUARTER],
                glue_words: vec![HALF, QUARTER, QUARTER],
                params: vec![
                    ZERO, HALF, QUARTER, QUARTER, ONE, ONE, HALF, QUARTER, QUARTER,
                ],
            }
        }

        fn bytes(&self) -> Vec<u8> {
            let nt = self.char_types.len();
            let lh = 2;
            let bc = 0;
            let ec = self.char_infos.len() - 1;
            let nw = self.widths.len();
            let nh = self.heights.len();
            let nd = self.depths.len();
            let ni = self.italics.len();
            let nl = self.steps.len();
            let nk = self.kerns.len();
            let ng = self.glue_words.len();
            let np = self.params.len();
            let lf = 7 + nt + lh + (ec - bc + 1) + nw + nh + nd + ni + nl + nk + ng + np;

            let mut bytes = Vec::with_capacity(lf * 4);
            for value in [
                self.direction,
                nt as u16,
                lf as u16,
                lh as u16,
                bc as u16,
                ec as u16,
                nw as u16,
                nh as u16,
                nd as u16,
                ni as u16,
                nl as u16,
                nk as u16,
                ng as u16,
                np as u16,
            ] {
                bytes.extend_from_slice(&value.to_be_bytes());
            }
            bytes.extend_from_slice(&0x1234_5678_u32.to_be_bytes());
            bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());
            for table in [&self.char_types, &self.char_infos] {
                for word in table {
                    bytes.extend_from_slice(word);
                }
            }
            for table in [&self.widths, &self.heights, &self.depths, &self.italics] {
                for value in table {
                    bytes.extend_from_slice(&value.to_be_bytes());
                }
            }
            for word in &self.steps {
                bytes.extend_from_slice(word);
            }
            for table in [&self.kerns, &self.glue_words, &self.params] {
                for value in table {
                    bytes.extend_from_slice(&value.to_be_bytes());
                }
            }
            bytes
        }
    }

    fn char_type(character_code: u32, character_type: u8) -> [u8; 4] {
        [
            ((character_code >> 8) & 0xff) as u8,
            (character_code & 0xff) as u8,
            ((character_code >> 16) & 0xff) as u8,
            character_type,
        ]
    }

    #[test]
    fn 現行仕様の横組jfmを読む() {
        let jfm = Jfm::parse(&Fixture::現行仕様().bytes()).unwrap();

        assert_eq!(jfm.direction, JfmDirection::Horizontal);
        assert_eq!(jfm.check, 0x1234_5678);
        assert_eq!(jfm.design_size, 10 * 65_536);
        let class_1 = jfm.class_of_raw_code(0x3001);
        assert_eq!(class_1, JfmClassId(1));
        assert_eq!(jfm.class_of_raw_code(0x1f600), JfmClassId(2));
        assert_eq!(jfm.class_of_raw_code(0xabcdef), JfmClassId(2));
        assert_eq!(jfm.class_of_raw_code('漢' as u32), JfmClassId(0));
        assert_eq!(jfm.class_of_raw_code(0x0100_0000), JfmClassId(0));
        assert_eq!(jfm.relative_width(class_1), JfmFixWord::from_raw(HALF));
        assert_eq!(jfm.relative_height(class_1), JfmFixWord::from_raw(HALF));
        assert_eq!(jfm.relative_depth(class_1), JfmFixWord::ZERO);
        assert_eq!(jfm.relative_italic(class_1), JfmFixWord::ZERO);
        assert_eq!(jfm.relative_zw(), JfmFixWord::from_raw(ONE));
        assert_eq!(
            jfm.relative_default_kanji_skip(),
            Some(JfmGlue {
                width: JfmFixWord::from_raw(HALF),
                stretch: JfmFixWord::from_raw(QUARTER),
                shrink: JfmFixWord::from_raw(QUARTER),
            })
        );
        assert_eq!(
            jfm.relative_default_xkanji_skip(),
            jfm.relative_default_kanji_skip()
        );
        assert_eq!(
            jfm.adjustment_for_codes('漢' as u32, 0x3001),
            Some(JfmAdjustment::Glue(JfmGlue {
                width: JfmFixWord::from_raw(HALF),
                stretch: JfmFixWord::from_raw(QUARTER),
                shrink: JfmFixWord::from_raw(QUARTER),
            }))
        );
        assert_eq!(
            jfm.adjustment_for_codes('漢' as u32, 0x1f600),
            Some(JfmAdjustment::Kern(JfmFixWord::from_raw(-QUARTER)))
        );
        assert_eq!(jfm.adjustment_for_codes(0x3001, '漢' as u32), None);
        assert_eq!(
            jfm.adjustment_by_class(JfmClassId(0), JfmClassId(1)),
            jfm.adjustment_for_codes('漢' as u32, 0x3001)
        );
    }

    #[test]
    fn 縦組識別子を横組と区別する() {
        let mut fixture = Fixture::現行仕様();
        fixture.direction = VERTICAL_JFM_ID;
        let jfm = Jfm::parse(&fixture.bytes()).unwrap();
        assert_eq!(jfm.direction, JfmDirection::Vertical);
    }

    #[test]
    fn すべての切断位置をエラーとして扱う() {
        let bytes = Fixture::現行仕様().bytes();
        for length in 0..bytes.len() {
            assert!(Jfm::parse(&bytes[..length]).is_err(), "length {length}");
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            Jfm::parse(&trailing),
            Err(JfmParseError::FileLengthMismatch { .. })
        ));
    }

    #[test]
    fn 各byteの破損をpanicせず処理する() {
        let bytes = Fixture::現行仕様().bytes();
        for index in 0..bytes.len() {
            let mut damaged = bytes.clone();
            damaged[index] ^= 0xff;
            let outcome = std::panic::catch_unwind(|| Jfm::parse(&damaged));
            assert!(outcome.is_ok(), "byte {index}");
        }
    }

    #[test]
    fn 先頭の小halfwordと表長の制約を検査する() {
        let mut bytes = Fixture::現行仕様().bytes();
        bytes[0..2].copy_from_slice(&10_u16.to_be_bytes());
        assert!(matches!(
            Jfm::parse(&bytes),
            Err(JfmParseError::UnknownDirectionId { id: 10 })
        ));

        let mut bytes = Fixture::現行仕様().bytes();
        bytes[2..4].copy_from_slice(&0x8001_u16.to_be_bytes());
        assert!(matches!(
            Jfm::parse(&bytes),
            Err(JfmParseError::HalfwordOutOfRange { field: "nt", .. })
        ));

        let mut fixture = Fixture::現行仕様();
        fixture.glue_words.pop();
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::InvalidGlueTableLength { .. })
        ));

        let mut bytes = Fixture::現行仕様().bytes();
        bytes[4..6].copy_from_slice(&1_u16.to_be_bytes());
        assert!(matches!(
            Jfm::parse(&bytes),
            Err(JfmParseError::InvalidSizeLayout { .. })
        ));
    }

    #[test]
    fn 文字型表はゼロから始まる昇順に限る() {
        let mut fixture = Fixture::現行仕様();
        fixture.char_types[0] = char_type(1, 0);
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::InvalidDefaultCharacterType { .. })
        ));

        let mut fixture = Fixture::現行仕様();
        fixture.char_types.swap(1, 2);
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::CharacterTypesNotSorted { .. })
        ));

        let mut fixture = Fixture::現行仕様();
        fixture.char_types[1][3] = 3;
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::CharacterTypeOutOfRange { .. })
        ));
    }

    #[test]
    fn 予約タグと範囲外の表参照を拒む() {
        let mut fixture = Fixture::現行仕様();
        fixture.char_infos[1][2] = 2;
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::UnsupportedCharacterTag { .. })
        ));

        let mut fixture = Fixture::現行仕様();
        fixture.char_infos[1][0] = 3;
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::MetricIndexOutOfRange { .. })
        ));

        let mut fixture = Fixture::現行仕様();
        fixture.steps[0] = [129, 0, 0, 9];
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::RelocationTargetOutOfRange { .. })
        ));
    }

    #[test]
    fn glueとkernの参照を実在する表に限る() {
        let mut fixture = Fixture::現行仕様();
        fixture.steps[2][2] = 1;
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::GlueIndexOutOfRange { index: 256, .. })
        ));

        let mut fixture = Fixture::現行仕様();
        fixture.steps[3][2] = 129;
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::KernIndexOutOfRange { index: 256, .. })
        ));
    }

    #[test]
    fn skipは不一致時だけ指定語数を飛び越す() {
        let mut fixture = Fixture::現行仕様();
        fixture.char_infos[0][3] = 0;
        fixture.steps = vec![[1, 1, 0, 0], [129, 0, 255, 255], [128, 2, 128, 0]];
        let jfm = Jfm::parse(&fixture.bytes()).unwrap();

        assert!(matches!(
            jfm.adjustment_for_codes('漢' as u32, 0x3001),
            Some(JfmAdjustment::Glue(_))
        ));
        assert_eq!(
            jfm.adjustment_for_codes('漢' as u32, 0x1f600),
            Some(JfmAdjustment::Kern(JfmFixWord::from_raw(-QUARTER)))
        );
    }

    #[test]
    fn 開始後の大きなskipは参照を解釈せず終了する() {
        let mut fixture = Fixture::現行仕様();
        fixture.char_infos[0][3] = 0;
        fixture.steps = vec![[0, 1, 0, 0], [129, 255, 255, 255]];
        let jfm = Jfm::parse(&fixture.bytes()).unwrap();

        assert!(matches!(
            jfm.adjustment_for_codes('漢' as u32, 0x3001),
            Some(JfmAdjustment::Glue(_))
        ));
        assert_eq!(jfm.adjustment_for_codes('漢' as u32, 0x1f600), None);
    }

    #[test]
    fn 二百五十六語を越える位置へ再配置する() {
        let mut fixture = Fixture::現行仕様();
        fixture.char_infos[0][3] = 0;
        fixture.steps = vec![[129, 0, 1, 1]];
        fixture.steps.resize(257, [129, 255, 255, 255]);
        fixture.steps.push([128, 1, 0, 0]);
        let jfm = Jfm::parse(&fixture.bytes()).unwrap();

        assert!(matches!(
            jfm.adjustment_for_codes('漢' as u32, 0x3001),
            Some(JfmAdjustment::Glue(_))
        ));
    }

    #[test]
    fn 二百五十六を超えるglue番号を保持する() {
        let mut fixture = Fixture::現行仕様();
        fixture.glue_words = vec![ZERO; 257 * 3];
        let base = 256 * 3;
        fixture.glue_words[base] = ONE;
        fixture.glue_words[base + 1] = HALF;
        fixture.glue_words[base + 2] = QUARTER;
        fixture.steps[2][2] = 1;
        let jfm = Jfm::parse(&fixture.bytes()).unwrap();
        assert_eq!(
            jfm.adjustment_for_codes('漢' as u32, 0x3001),
            Some(JfmAdjustment::Glue(JfmGlue {
                width: JfmFixWord::from_raw(ONE),
                stretch: JfmFixWord::from_raw(HALF),
                shrink: JfmFixWord::from_raw(QUARTER),
            }))
        );
    }

    #[test]
    fn 二百五十六を超えるkern番号を保持する() {
        let mut fixture = Fixture::現行仕様();
        fixture.kerns = vec![ZERO; 257];
        fixture.kerns[256] = -HALF;
        fixture.steps[3][2] = 129;
        let jfm = Jfm::parse(&fixture.bytes()).unwrap();
        assert_eq!(
            jfm.adjustment_for_codes('漢' as u32, 0x1f600),
            Some(JfmAdjustment::Kern(JfmFixWord::from_raw(-HALF)))
        );
    }

    #[test]
    fn 寸法表のゼロ番とfixword範囲を検査する() {
        let mut fixture = Fixture::現行仕様();
        fixture.widths[0] = 1;
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::NonzeroFirstMetric { table: "width", .. })
        ));

        let mut fixture = Fixture::現行仕様();
        fixture.kerns[0] = 0x0100_0000;
        assert!(matches!(
            Jfm::parse(&fixture.bytes()),
            Err(JfmParseError::InvalidRelativeFixWord { table: "kern", .. })
        ));
    }

    #[test]
    #[ignore = "PRATEX_JFM_ORACLE に TeX Live の upjisr-h.tfm を指定して実行する"]
    fn 配布upjisr横組を公開仕様どおりに読む() {
        let path = std::env::var_os("PRATEX_JFM_ORACLE")
            .expect("PRATEX_JFM_ORACLE must point to upjisr-h.tfm");
        let bytes = std::fs::read(path).unwrap();
        let jfm = Jfm::parse(&bytes).unwrap();

        assert_eq!(jfm.direction, JfmDirection::Horizontal);
        assert_eq!(jfm.design_size, 10 * 65_536);
        assert_eq!(jfm.char_infos.len(), 7);
        assert_eq!(jfm.class_of_raw_code(0x00ab), JfmClassId(1));
        assert_eq!(jfm.class_of_raw_code(0x00b7), JfmClassId(3));
        assert_eq!(jfm.class_of_raw_code(0x2014), JfmClassId(5));
        assert_eq!(
            jfm.widths,
            vec![
                JfmFixWord::ZERO,
                JfmFixWord::from_raw(HALF),
                JfmFixWord::from_raw(ONE),
            ]
        );
        assert_eq!(jfm.params.len(), 9);
        assert_eq!(jfm.params[2], JfmFixWord::from_raw(0x0001_999a));
        assert_eq!(jfm.relative_zw(), JfmFixWord::from_raw(ONE));
    }

    #[test]
    #[ignore = "PRATEX_JFM_ORACLE_DIR に TeX Live 2026のptex/uptex-fontsを指定して実行する"]
    fn 配布jfm九十六件をすべて読む() {
        fn tfmを集める(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    tfmを集める(&path, files);
                } else if path.extension() == Some(std::ffi::OsStr::new("tfm")) {
                    files.push(path);
                }
            }
        }

        let root = std::env::var_os("PRATEX_JFM_ORACLE_DIR")
            .expect("PRATEX_JFM_ORACLE_DIR must contain copied TeX Live JFMs");
        let mut files = Vec::new();
        tfmを集める(std::path::Path::new(&root), &mut files);
        files.sort();

        let mut horizontal = 0;
        let mut vertical = 0;
        for path in files {
            let bytes = std::fs::read(&path).unwrap();
            if !matches!(bytes.get(..2), Some([0, 9] | [0, 11])) {
                continue;
            }
            match Jfm::parse(&bytes)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
                .direction
            {
                JfmDirection::Horizontal => horizontal += 1,
                JfmDirection::Vertical => vertical += 1,
            }
        }

        assert_eq!((horizontal, vertical), (56, 40));
    }
}

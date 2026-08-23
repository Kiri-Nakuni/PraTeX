use crate::format::{Dumpable, FormatError};

use std::io::Write;

/// upTeX 2.02 が公開している和文カテゴリー。
///
/// 出典は uptex-base の `01uptex_doc_utf8.txt` (2026-02-15, Ver. 2.02)。
/// 値は公開インターフェースなので列挙子の並びではなく明示値で固定する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KCatCode {
    LatinUcs = 14,
    NotCjk = 15,
    Kanji = 16,
    Kana = 17,
    OtherKChar = 18,
    Hangul = 19,
    Modifier = 20,
}

impl Default for KCatCode {
    fn default() -> Self {
        Self::OtherKChar
    }
}

impl KCatCode {
    /// upTeX互換 `\kcatcode` が読み書きする公開番号。
    ///
    /// 入力分類へ渡す前に意味へ写し、`CatCode` の公開番号として cast しない。
    pub const fn public_number(self) -> i32 {
        match self {
            Self::LatinUcs => 14,
            Self::NotCjk => 15,
            Self::Kanji => 16,
            Self::Kana => 17,
            Self::OtherKChar => 18,
            Self::Hangul => 19,
            Self::Modifier => 20,
        }
    }

    pub fn from_public_number(value: i32) -> Result<Self, ()> {
        match value {
            14 => Ok(Self::LatinUcs),
            15 => Ok(Self::NotCjk),
            16 => Ok(Self::Kanji),
            17 => Ok(Self::Kana),
            18 => Ok(Self::OtherKChar),
            19 => Ok(Self::Hangul),
            20 => Ok(Self::Modifier),
            _ => Err(()),
        }
    }

    pub(crate) fn is_valid_for(self, code_point: u32) -> bool {
        self != Self::LatinUcs || code_point <= 0x2E7F
    }
}

impl TryFrom<i32> for KCatCode {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_public_number(value)
    }
}

impl Dumpable for KCatCode {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.public_number().dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Self::try_from(i32::undump(lines)?).map_err(|_| FormatError::ParseError)
    }
}

/// Unicode block 346 個、upTeX の擬似境界 12 個、例外集合 7 個。
pub(crate) const KCAT_CODE_BLOCK_COUNT: usize = KCAT_CODE_BLOCK_STARTS.len() + EXCEPTION_COUNT;
const EXCEPTION_COUNT: usize = 7;
const MAX_UNICODE_CODE_POINT: u32 = 0x10_FFFF;
const DUMP_HEADER: &str = "KCatCodes/upTeX-2.02/Unicode-17.0.0";

/// 保存スタックで Unicode block 単位の代入を識別する番号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KCatCodeBlock(u16);

impl KCatCodeBlock {
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }

    fn from_index(index: usize) -> Self {
        debug_assert!(index < KCAT_CODE_BLOCK_COUNT);
        Self(index as u16)
    }
}

impl Dumpable for KCatCodeBlock {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let value = u16::undump(lines)?;
        if (value as usize) < KCAT_CODE_BLOCK_COUNT {
            Ok(Self(value))
        } else {
            Err(FormatError::ParseError)
        }
    }
}

/// `\kcatcode` の現在値。
///
/// 通常の文字読み取り経路からは参照しないため、ASCII の字句解析には費用を
/// 加えない。問い合わせ・代入時だけ固定境界を二分探索する。
pub(crate) struct KCatCodes {
    values: [KCatCode; KCAT_CODE_BLOCK_COUNT],
}

impl KCatCodes {
    pub(crate) fn new() -> Self {
        let values = std::array::from_fn(default_for_block);
        Self { values }
    }

    pub(crate) fn get(&self, code_point: u32) -> KCatCode {
        self.get_block(Self::block_for(code_point))
    }

    pub(crate) fn get_block(&self, block: KCatCodeBlock) -> KCatCode {
        self.values[block.index()]
    }

    pub(crate) fn set_block(&mut self, block: KCatCodeBlock, new_value: KCatCode) -> KCatCode {
        std::mem::replace(&mut self.values[block.index()], new_value)
    }

    pub(crate) fn block_for(code_point: u32) -> KCatCodeBlock {
        if code_point > MAX_UNICODE_CODE_POINT {
            return KCatCodeBlock::from_index(0);
        }
        if let Some(exception) = exception_index(code_point) {
            return KCatCodeBlock::from_index(exception_block_start() + exception);
        }

        // U+0000 が最初の境界なので Err(0) にはならない。Blocks.txt の
        // block 間にある未割当区間は、公開処理系と同じく直前の開始境界の
        // slot に含める。
        let index = match KCAT_CODE_BLOCK_STARTS.binary_search(&code_point) {
            Ok(index) => index,
            Err(next_index) => next_index - 1,
        };
        KCatCodeBlock::from_index(index)
    }
}

impl Dumpable for KCatCodes {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{DUMP_HEADER}")?;
        KCAT_CODE_BLOCK_COUNT.dump(target)?;
        for value in self.values {
            value.dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        if lines.next().ok_or(FormatError::IncompleteFile)? != DUMP_HEADER {
            return Err(FormatError::ParseError);
        }
        if usize::undump(lines)? != KCAT_CODE_BLOCK_COUNT {
            return Err(FormatError::ParseError);
        }

        let mut values = [KCatCode::OtherKChar; KCAT_CODE_BLOCK_COUNT];
        for (index, value) in values.iter_mut().enumerate() {
            *value = KCatCode::undump(lines)?;
            if *value == KCatCode::LatinUcs && !block_allows_latin_ucs(index) {
                return Err(FormatError::ParseError);
            }
        }
        Ok(Self { values })
    }
}

fn default_for_block(index: usize) -> KCatCode {
    if index >= exception_block_start() {
        return match index - exception_block_start() {
            0 | 1 | 2 | 3 => KCatCode::Modifier,
            4 => KCatCode::NotCjk,
            5 | 6 => KCatCode::Kana,
            _ => unreachable!("fixed kcatcode exception index"),
        };
    }

    match KCAT_CODE_BLOCK_STARTS[index] {
        0x0000 | 0x0100 | 0x0180 | 0x1E00 => KCatCode::NotCjk,
        0x1100 | 0x3130 | 0xA960 | 0xAC00 | 0xD7B0 => KCatCode::Hangul,
        0x3040 | 0x30A0 | 0x31F0 | 0x1AFF0 | 0x1B000 | 0x1B100 | 0x1B130 => KCatCode::Kana,
        0x2E80 | 0x2F00 | 0x3100 | 0x3190 | 0x31A0 | 0x31C0 | 0x3400 | 0x4E00 | 0xF900
        | 0x20000 | 0x2A700 | 0x2B740 | 0x2B820 | 0x2CEB0 | 0x2EBF0 | 0x2F800 | 0x30000
        | 0x31350 | 0x323B0 | 0x33480 => KCatCode::Kanji,
        0xFE00 | 0xE0100 => KCatCode::Modifier,
        _ => KCatCode::OtherKChar,
    }
}

const fn exception_block_start() -> usize {
    KCAT_CODE_BLOCK_STARTS.len()
}

fn block_allows_latin_ucs(index: usize) -> bool {
    if index < KCAT_CODE_BLOCK_STARTS.len() {
        KCatCode::LatinUcs.is_valid_for(KCAT_CODE_BLOCK_STARTS[index])
    } else {
        matches!(index - exception_block_start(), 1 | 4)
    }
}

fn exception_index(code_point: u32) -> Option<usize> {
    match code_point {
        0x3099 | 0x309A => Some(0),
        0x20E3 => Some(1),
        0x1F1E6..=0x1F1FF => Some(2),
        0x1F3FB..=0x1F3FF => Some(3),
        0x00AA | 0x00BA | 0x00C0..=0x00D6 | 0x00D8..=0x00F6 | 0x00F8..=0x00FF => Some(4),
        0xFF10..=0xFF19 | 0xFF21..=0xFF3A | 0xFF41..=0xFF5A => Some(5),
        0xFF66..=0xFF6F | 0xFF71..=0xFF9D => Some(6),
        _ => None,
    }
}

/// Unicode 17.0.0 `Blocks.txt` (2025-08-01) と upTeX 2.02 の開始境界。
///
/// Unicode の末尾には upTeX 公開表の block 番号と公式処理系の観測が一致する
/// 12 個の擬似境界を足す。名前は実行時に不要なので昇順境界だけを固定する。
const KCAT_CODE_BLOCK_STARTS: [u32; 358] = [
    0x0000, 0x0080, 0x0100, 0x0180, 0x0250, 0x02B0, 0x0300, 0x0370, 0x0400, 0x0500, 0x0530, 0x0590,
    0x0600, 0x0700, 0x0750, 0x0780, 0x07C0, 0x0800, 0x0840, 0x0860, 0x0870, 0x08A0, 0x0900, 0x0980,
    0x0A00, 0x0A80, 0x0B00, 0x0B80, 0x0C00, 0x0C80, 0x0D00, 0x0D80, 0x0E00, 0x0E80, 0x0F00, 0x1000,
    0x10A0, 0x1100, 0x1200, 0x1380, 0x13A0, 0x1400, 0x1680, 0x16A0, 0x1700, 0x1720, 0x1740, 0x1760,
    0x1780, 0x1800, 0x18B0, 0x1900, 0x1950, 0x1980, 0x19E0, 0x1A00, 0x1A20, 0x1AB0, 0x1B00, 0x1B80,
    0x1BC0, 0x1C00, 0x1C50, 0x1C80, 0x1C90, 0x1CC0, 0x1CD0, 0x1D00, 0x1D80, 0x1DC0, 0x1E00, 0x1F00,
    0x2000, 0x2070, 0x20A0, 0x20D0, 0x2100, 0x2150, 0x2190, 0x2200, 0x2300, 0x2400, 0x2440, 0x2460,
    0x2500, 0x2580, 0x25A0, 0x2600, 0x2700, 0x27C0, 0x27F0, 0x2800, 0x2900, 0x2980, 0x2A00, 0x2B00,
    0x2C00, 0x2C60, 0x2C80, 0x2D00, 0x2D30, 0x2D80, 0x2DE0, 0x2E00, 0x2E80, 0x2F00, 0x2FF0, 0x3000,
    0x3040, 0x30A0, 0x3100, 0x3130, 0x3190, 0x31A0, 0x31C0, 0x31F0, 0x3200, 0x3300, 0x3400, 0x4DC0,
    0x4E00, 0xA000, 0xA490, 0xA4D0, 0xA500, 0xA640, 0xA6A0, 0xA700, 0xA720, 0xA800, 0xA830, 0xA840,
    0xA880, 0xA8E0, 0xA900, 0xA930, 0xA960, 0xA980, 0xA9E0, 0xAA00, 0xAA60, 0xAA80, 0xAAE0, 0xAB00,
    0xAB30, 0xAB70, 0xABC0, 0xAC00, 0xD7B0, 0xD800, 0xDB80, 0xDC00, 0xE000, 0xF900, 0xFB00, 0xFB50,
    0xFE00, 0xFE10, 0xFE20, 0xFE30, 0xFE50, 0xFE70, 0xFF00, 0xFFF0, 0x10000, 0x10080, 0x10100,
    0x10140, 0x10190, 0x101D0, 0x10280, 0x102A0, 0x102E0, 0x10300, 0x10330, 0x10350, 0x10380,
    0x103A0, 0x10400, 0x10450, 0x10480, 0x104B0, 0x10500, 0x10530, 0x10570, 0x105C0, 0x10600,
    0x10780, 0x10800, 0x10840, 0x10860, 0x10880, 0x108E0, 0x10900, 0x10920, 0x10940, 0x10980,
    0x109A0, 0x10A00, 0x10A60, 0x10A80, 0x10AC0, 0x10B00, 0x10B40, 0x10B60, 0x10B80, 0x10C00,
    0x10C80, 0x10D00, 0x10D40, 0x10E60, 0x10E80, 0x10EC0, 0x10F00, 0x10F30, 0x10F70, 0x10FB0,
    0x10FE0, 0x11000, 0x11080, 0x110D0, 0x11100, 0x11150, 0x11180, 0x111E0, 0x11200, 0x11280,
    0x112B0, 0x11300, 0x11380, 0x11400, 0x11480, 0x11580, 0x11600, 0x11660, 0x11680, 0x116D0,
    0x11700, 0x11800, 0x118A0, 0x11900, 0x119A0, 0x11A00, 0x11A50, 0x11AB0, 0x11AC0, 0x11B00,
    0x11B60, 0x11BC0, 0x11C00, 0x11C70, 0x11D00, 0x11D60, 0x11DB0, 0x11EE0, 0x11F00, 0x11FB0,
    0x11FC0, 0x12000, 0x12400, 0x12480, 0x12F90, 0x13000, 0x13430, 0x13460, 0x14400, 0x16100,
    0x16800, 0x16A40, 0x16A70, 0x16AD0, 0x16B00, 0x16D40, 0x16E40, 0x16EA0, 0x16F00, 0x16FE0,
    0x17000, 0x18800, 0x18B00, 0x18D00, 0x18D80, 0x1AFF0, 0x1B000, 0x1B100, 0x1B130, 0x1B170,
    0x1BC00, 0x1BCA0, 0x1CC00, 0x1CEC0, 0x1CF00, 0x1D000, 0x1D100, 0x1D200, 0x1D2C0, 0x1D2E0,
    0x1D300, 0x1D360, 0x1D400, 0x1D800, 0x1DF00, 0x1E000, 0x1E030, 0x1E100, 0x1E290, 0x1E2C0,
    0x1E4D0, 0x1E5D0, 0x1E6C0, 0x1E7E0, 0x1E800, 0x1E900, 0x1EC70, 0x1ED00, 0x1EE00, 0x1F000,
    0x1F030, 0x1F0A0, 0x1F100, 0x1F200, 0x1F300, 0x1F600, 0x1F650, 0x1F680, 0x1F700, 0x1F780,
    0x1F800, 0x1F900, 0x1FA00, 0x1FA70, 0x1FB00, 0x20000, 0x2A700, 0x2B740, 0x2B820, 0x2CEB0,
    0x2EBF0, 0x2F800, 0x30000, 0x31350, 0x323B0, 0x33480, 0x40000, 0x50000, 0x60000, 0x70000,
    0x80000, 0x90000, 0xA0000, 0xB0000, 0xC0000, 0xD0000, 0xE0000, 0xE0100, 0xE01F0, 0xF0000,
    0x100000,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 公開表の既定値を分類する() {
        let table = KCatCodes::new();
        for (code, expected) in [
            (0x0041, KCatCode::NotCjk),
            (0x00AA, KCatCode::NotCjk),
            (0x00A1, KCatCode::OtherKChar),
            (0x3042, KCatCode::Kana),
            (0x30FC, KCatCode::Kana),
            (0x4E00, KCatCode::Kanji),
            (0xAC00, KCatCode::Hangul),
            (0x3099, KCatCode::Modifier),
            (0xE0100, KCatCode::Modifier),
            (0xFF21, KCatCode::Kana),
        ] {
            assert_eq!(table.get(code), expected, "U+{code:04X}");
        }
    }

    #[test]
    fn 公開表の非十八区間を端点と中点で照合する() {
        let table = KCatCodes::new();
        let ranges = [
            (0x0000, 0x007F, KCatCode::NotCjk),
            (0x0100, 0x017F, KCatCode::NotCjk),
            (0x0180, 0x024F, KCatCode::NotCjk),
            (0x1E00, 0x1EFF, KCatCode::NotCjk),
            (0x1100, 0x11FF, KCatCode::Hangul),
            (0x3130, 0x318F, KCatCode::Hangul),
            (0xA960, 0xA97F, KCatCode::Hangul),
            (0xAC00, 0xD7AF, KCatCode::Hangul),
            (0xD7B0, 0xD7FF, KCatCode::Hangul),
            (0x2E80, 0x2EFF, KCatCode::Kanji),
            (0x2F00, 0x2FEF, KCatCode::Kanji),
            (0x3100, 0x312F, KCatCode::Kanji),
            (0x3190, 0x319F, KCatCode::Kanji),
            (0x31A0, 0x31BF, KCatCode::Kanji),
            (0x31C0, 0x31EF, KCatCode::Kanji),
            (0x3400, 0x4DBF, KCatCode::Kanji),
            (0x4E00, 0x9FFF, KCatCode::Kanji),
            (0xF900, 0xFAFF, KCatCode::Kanji),
            (0x20000, 0x2A6FF, KCatCode::Kanji),
            (0x2A700, 0x2B73F, KCatCode::Kanji),
            (0x2B740, 0x2B81F, KCatCode::Kanji),
            (0x2B820, 0x2CEAF, KCatCode::Kanji),
            (0x2CEB0, 0x2EBEF, KCatCode::Kanji),
            (0x2EBF0, 0x2F7FF, KCatCode::Kanji),
            (0x2F800, 0x2FFFF, KCatCode::Kanji),
            (0x30000, 0x3134F, KCatCode::Kanji),
            (0x31350, 0x323AF, KCatCode::Kanji),
            (0x323B0, 0x3347F, KCatCode::Kanji),
            (0x3040, 0x3098, KCatCode::Kana),
            (0x309B, 0x309F, KCatCode::Kana),
            (0x30A0, 0x30FF, KCatCode::Kana),
            (0x31F0, 0x31FF, KCatCode::Kana),
            (0x1AFF0, 0x1AFFF, KCatCode::Kana),
            (0x1B000, 0x1B0FF, KCatCode::Kana),
            (0x1B100, 0x1B12F, KCatCode::Kana),
            (0x1B130, 0x1B16F, KCatCode::Kana),
            (0xFE00, 0xFE0F, KCatCode::Modifier),
            (0xE0100, 0xE01EF, KCatCode::Modifier),
        ];
        for (start, end, expected) in ranges {
            for code in [start, start + (end - start) / 2, end] {
                assert_eq!(table.get(code), expected, "U+{code:04X}");
            }
        }
        assert!(!table.values.contains(&KCatCode::LatinUcs));
    }

    #[test]
    fn 七つの例外集合は境界の外へ漏れない() {
        let table = KCatCodes::new();
        for code in [0x3099, 0x309A, 0x20E3, 0x1F1E6, 0x1F1FF, 0x1F3FB, 0x1F3FF] {
            assert_eq!(table.get(code), KCatCode::Modifier, "U+{code:04X}");
        }
        for code in [
            0x3098, 0x309B, 0x20E2, 0x20E4, 0x1F1E5, 0x1F200, 0x1F3FA, 0x1F400,
        ] {
            assert_ne!(table.get(code), KCatCode::Modifier, "U+{code:04X}");
        }

        for code in [0xAA, 0xBA, 0xC0, 0xD6, 0xD8, 0xF6, 0xF8, 0xFF] {
            assert_eq!(table.get(code), KCatCode::NotCjk, "U+{code:04X}");
        }
        for code in [0xA9, 0xAB, 0xB9, 0xBB, 0xD7, 0xF7] {
            assert_eq!(table.get(code), KCatCode::OtherKChar, "U+{code:04X}");
        }

        for code in [
            0xFF10, 0xFF19, 0xFF21, 0xFF3A, 0xFF41, 0xFF5A, 0xFF66, 0xFF6F, 0xFF71, 0xFF9D,
        ] {
            assert_eq!(table.get(code), KCatCode::Kana, "U+{code:04X}");
        }
        for code in [
            0xFF0F, 0xFF1A, 0xFF20, 0xFF3B, 0xFF40, 0xFF5B, 0xFF65, 0xFF70, 0xFF9E,
        ] {
            assert_eq!(table.get(code), KCatCode::OtherKChar, "U+{code:04X}");
        }
    }

    #[test]
    fn 代入単位はブロックと例外集合である() {
        let mut table = KCatCodes::new();
        let hiragana = KCatCodes::block_for(0x3042);
        assert_eq!(hiragana, KCatCodes::block_for(0x3098));
        assert_ne!(hiragana, KCatCodes::block_for(0x3099));
        table.set_block(hiragana, KCatCode::NotCjk);
        assert_eq!(table.get(0x3042), KCatCode::NotCjk);
        assert_eq!(table.get(0x3098), KCatCode::NotCjk);
        assert_eq!(table.get(0x3099), KCatCode::Modifier);

        let latin_letters = KCatCodes::block_for(0x00AA);
        assert_eq!(latin_letters, KCatCodes::block_for(0x00FF));
        assert_ne!(latin_letters, KCatCodes::block_for(0x00A1));

        for members in [
            &[0x3099, 0x309A][..],
            &[0x1F1E6, 0x1F1FF][..],
            &[0x1F3FB, 0x1F3FF][..],
            &[0xFF10, 0xFF21, 0xFF41, 0xFF5A][..],
            &[0xFF66, 0xFF6F, 0xFF71, 0xFF9D][..],
        ] {
            let block = KCatCodes::block_for(members[0]);
            assert!(members
                .iter()
                .all(|&code| KCatCodes::block_for(code) == block));
            table.set_block(block, KCatCode::Hangul);
            assert!(members
                .iter()
                .all(|&code| table.get(code) == KCatCode::Hangul));
        }

        let extension_f = KCatCodes::block_for(0x2CEB0);
        let extension_i = KCatCodes::block_for(0x2EBF0);
        assert_ne!(extension_f, extension_i);
        assert_eq!(extension_f, KCatCodes::block_for(0x2EBEF));
        assert_eq!(extension_i, KCatCodes::block_for(0x2EE5F));
        assert_eq!(extension_i, KCatCodes::block_for(0x2EE60));
        assert_eq!(extension_i, KCatCodes::block_for(0x2F7FF));
    }

    #[test]
    fn サロゲート整数値も範囲内として分類する() {
        let table = KCatCodes::new();
        assert_eq!(table.get(0xD7FF), KCatCode::Hangul);
        assert_eq!(table.get(0xD800), KCatCode::OtherKChar);
        assert_eq!(table.get(0xDB80), KCatCode::OtherKChar);
        assert_eq!(table.get(0xDFFF), KCatCode::OtherKChar);
        assert_eq!(table.get(0xE000), KCatCode::OtherKChar);

        assert_ne!(KCatCodes::block_for(0xD7FF), KCatCodes::block_for(0xD800));
        assert_ne!(KCatCodes::block_for(0xDB7F), KCatCodes::block_for(0xDB80));
        assert_ne!(KCatCodes::block_for(0xDBFF), KCatCodes::block_for(0xDC00));
        assert_ne!(KCatCodes::block_for(0xDFFF), KCatCodes::block_for(0xE000));
    }

    #[test]
    fn 末尾の擬似境界は独立した保存単位である() {
        let table = KCatCodes::new();
        for (code, expected) in [
            (0x323B0, KCatCode::Kanji),
            (0x3347F, KCatCode::Kanji),
            (0x33480, KCatCode::Kanji),
            (0x3FFFF, KCatCode::Kanji),
            (0x40000, KCatCode::OtherKChar),
            (0xDFFFF, KCatCode::OtherKChar),
            (0xE0000, KCatCode::OtherKChar),
            (0xE00FF, KCatCode::OtherKChar),
            (0xE0100, KCatCode::Modifier),
            (0xE01EF, KCatCode::Modifier),
            (0xE01F0, KCatCode::OtherKChar),
            (0xEFFFF, KCatCode::OtherKChar),
            (0xF0000, KCatCode::OtherKChar),
            (0x10FFFF, KCatCode::OtherKChar),
        ] {
            assert_eq!(table.get(code), expected, "U+{code:05X}");
        }

        assert_ne!(KCatCodes::block_for(0x3347F), KCatCodes::block_for(0x33480));
        assert_eq!(KCatCodes::block_for(0x33480), KCatCodes::block_for(0x3FFFF));
        for (left, right) in [
            (0x3FFFF, 0x40000),
            (0x4FFFF, 0x50000),
            (0xCFFFF, 0xD0000),
            (0xDFFFF, 0xE0000),
            (0xE00FF, 0xE0100),
            (0xE01EF, 0xE01F0),
            (0xEFFFF, 0xF0000),
            (0xFFFFF, 0x100000),
        ] {
            assert_ne!(KCatCodes::block_for(left), KCatCodes::block_for(right));
        }
    }

    #[test]
    fn 書式は版と個数と値域を厳密に検証する() {
        let table = KCatCodes::new();
        let mut dumped = Vec::new();
        table.dump(&mut dumped).unwrap();
        let text = String::from_utf8(dumped).unwrap();
        let mut lines = text.lines();
        let loaded = KCatCodes::undump(&mut lines).unwrap();
        assert_eq!(loaded.get(0x3042), KCatCode::Kana);

        let wrong_header_text = format!("old-table\n{KCAT_CODE_BLOCK_COUNT}\n");
        let mut wrong_header = wrong_header_text.lines();
        assert!(matches!(
            KCatCodes::undump(&mut wrong_header),
            Err(FormatError::ParseError)
        ));

        let wrong_count_text = format!("{DUMP_HEADER}\n0\n");
        let mut wrong_count = wrong_count_text.lines();
        assert!(matches!(
            KCatCodes::undump(&mut wrong_count),
            Err(FormatError::ParseError)
        ));

        let truncated_text = format!("{DUMP_HEADER}\n{KCAT_CODE_BLOCK_COUNT}\n15\n");
        let mut truncated = truncated_text.lines();
        assert!(matches!(
            KCatCodes::undump(&mut truncated),
            Err(FormatError::IncompleteFile)
        ));

        let invalid = text.replacen("\n15\n", "\n13\n", 1);
        let mut invalid_lines = invalid.lines();
        assert!(matches!(
            KCatCodes::undump(&mut invalid_lines),
            Err(FormatError::ParseError)
        ));

        let mut illegal_latin = KCatCodes::new();
        illegal_latin.set_block(KCatCodes::block_for(0x3042), KCatCode::LatinUcs);
        let mut illegal_dump = Vec::new();
        illegal_latin.dump(&mut illegal_dump).unwrap();
        let illegal_text = String::from_utf8(illegal_dump).unwrap();
        let mut illegal_lines = illegal_text.lines();
        assert!(matches!(
            KCatCodes::undump(&mut illegal_lines),
            Err(FormatError::ParseError)
        ));

        let last_valid_text = (KCAT_CODE_BLOCK_COUNT - 1).to_string();
        let mut last_valid = std::iter::once(last_valid_text.as_str());
        assert_eq!(
            KCatCodeBlock::undump(&mut last_valid).unwrap().index(),
            KCAT_CODE_BLOCK_COUNT - 1
        );
        let invalid_block_text = KCAT_CODE_BLOCK_COUNT.to_string();
        let mut invalid_block = std::iter::once(invalid_block_text.as_str());
        assert!(matches!(
            KCatCodeBlock::undump(&mut invalid_block),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn ブロック境界は昇順で最大値を覆う() {
        assert_eq!(KCAT_CODE_BLOCK_STARTS.len(), 358);
        assert_eq!(KCAT_CODE_BLOCK_STARTS[0], 0);
        assert!(KCAT_CODE_BLOCK_STARTS
            .windows(2)
            .all(|pair| pair[0] < pair[1]));
        assert!(KCAT_CODE_BLOCK_STARTS[KCAT_CODE_BLOCK_STARTS.len() - 1] <= MAX_UNICODE_CODE_POINT);
        assert_eq!(
            KCatCodes::block_for(MAX_UNICODE_CODE_POINT).index(),
            KCAT_CODE_BLOCK_STARTS.len() - 1
        );
    }
}

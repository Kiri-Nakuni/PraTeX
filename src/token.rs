use crate::eqtb::{CatCode, ControlSequence, Eqtb, MAX_LATIN_UCS_CODE};
use crate::format::{Dumpable, FormatError};
use crate::print::Printer;

use std::io::Write;

const MAX_CJK_CODE_POINT: u32 = 0x10_FFFF;
const CJK_CODE_POINT_MASK: u32 = 0x00FF_FFFF;
const CJK_CATEGORY_SHIFT: u32 = 24;
const LATIN_UCS_CODE_MASK: u32 = 0xFFFF;
const LATIN_UCS_CAT_CODE_SHIFT: u32 = 16;

fn encode_uptex_utf8(code_point: u32) -> Option<([u8; 4], usize)> {
    if code_point > MAX_CJK_CODE_POINT {
        return None;
    }
    Some(if code_point <= 0x7F {
        ([code_point as u8, 0, 0, 0], 1)
    } else if code_point <= 0x7FF {
        (
            [
                0xC0 | (code_point >> 6) as u8,
                0x80 | (code_point & 0x3F) as u8,
                0,
                0,
            ],
            2,
        )
    } else if code_point <= 0xFFFF {
        (
            [
                0xE0 | (code_point >> 12) as u8,
                0x80 | ((code_point >> 6) & 0x3F) as u8,
                0x80 | (code_point & 0x3F) as u8,
                0,
            ],
            3,
        )
    } else {
        (
            [
                0xF0 | (code_point >> 18) as u8,
                0x80 | ((code_point >> 12) & 0x3F) as u8,
                0x80 | ((code_point >> 6) & 0x3F) as u8,
                0x80 | (code_point & 0x3F) as u8,
            ],
            4,
        )
    })
}

fn decode_uptex_utf8(bytes: &[u8], maximum: u32) -> Option<(u32, usize)> {
    let first = *bytes.first()?;
    let (len, mut code_point) = match first {
        0xC2..=0xDF => (2, u32::from(first & 0x1F)),
        0xE0..=0xEF => (3, u32::from(first & 0x0F)),
        0xF0..=0xF4 => (4, u32::from(first & 0x07)),
        _ => return None,
    };
    for &byte in bytes.get(1..len)? {
        if !(0x80..=0xBF).contains(&byte) {
            return None;
        }
        code_point = (code_point << 6) | u32::from(byte & 0x3F);
    }
    (1..=maximum)
        .contains(&code_point)
        .then_some((code_point, len))
}

/// Decode one input candidate using the acceptance boundary exposed by upTeX.
///
/// This deliberately accepts overlong encodings and surrogate values, while
/// rejecting zero, U+10FFFF, and values above it. Recovery belongs to the
/// caller and consumes only the first byte after `None`.
pub(crate) fn decode_uptex_input_code_point(bytes: &[u8]) -> Option<(u32, usize)> {
    decode_uptex_utf8(bytes, 0x10_FFFE)
}

/// Decode bytes previously emitted by [`encode_uptex_utf8`].
///
/// Internally manufactured format data may contain U+10FFFF even though the
/// public input boundary rejects it, so diagnostic printing has the wider
/// upper bound.
pub(crate) fn decode_printed_uptex_code_point(bytes: &[u8]) -> Option<(u32, usize)> {
    decode_uptex_utf8(bytes, MAX_CJK_CODE_POINT)
}

/// Append upTeX's byte representation without going through Rust `char`.
/// Callers validate the numeric range before storing a code point.
pub(crate) fn push_uptex_utf8(code_point: u32, target: &mut Vec<u8>) {
    let (bytes, len) = encode_uptex_utf8(code_point).expect("validated upTeX code point");
    target.extend_from_slice(&bytes[..len]);
}

/// Print one numeric upTeX character without converting it to Rust `char`.
pub(crate) fn print_uptex_code_point(code_point: u32, printer: &mut impl Printer) {
    let (bytes, len) = encode_uptex_utf8(code_point).expect("validated upTeX code point");
    printer.print_uptex_char(code_point, &bytes[..len]);
}

/// The category carried by a Japanese character token.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CjkCategory {
    Kanji = 16,
    Kana = 17,
    OtherKChar = 18,
    Hangul = 19,
    Modifier = 20,
}

impl TryFrom<u8> for CjkCategory {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            16 => Ok(Self::Kanji),
            17 => Ok(Self::Kana),
            18 => Ok(Self::OtherKChar),
            19 => Ok(Self::Hangul),
            20 => Ok(Self::Modifier),
            _ => Err(()),
        }
    }
}

impl Dumpable for CjkCategory {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        (*self as u8).dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Self::try_from(u8::undump(lines)?).map_err(|_| FormatError::ParseError)
    }
}

/// A Unicode code point and the Japanese character category assigned to it.
///
/// The lower 24 bits hold the code point and the upper byte holds the category.
/// The field is private so invalid packed values cannot be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CjkToken(u32);

impl CjkToken {
    pub(crate) const fn new(code_point: u32, category: CjkCategory) -> Option<Self> {
        if code_point <= MAX_CJK_CODE_POINT {
            Some(Self(code_point | ((category as u32) << CJK_CATEGORY_SHIFT)))
        } else {
            None
        }
    }

    pub(crate) const fn code_point(self) -> u32 {
        self.0 & CJK_CODE_POINT_MASK
    }

    pub(crate) fn category(self) -> CjkCategory {
        match (self.0 >> CJK_CATEGORY_SHIFT) as u8 {
            16 => CjkCategory::Kanji,
            17 => CjkCategory::Kana,
            18 => CjkCategory::OtherKChar,
            19 => CjkCategory::Hangul,
            20 => CjkCategory::Modifier,
            _ => unreachable!("private CJK token invariant"),
        }
    }

    /// Print the upTeX-compatible UTF-8 byte representation of the code point.
    ///
    /// This deliberately accepts surrogate code points. Rust's `char` and
    /// `str` types cannot represent them, but upTeX character tokens can.
    pub(crate) fn print_utf8(self, printer: &mut impl Printer) {
        print_uptex_code_point(self.code_point(), printer);
    }

    /// Append the upTeX byte representation at an explicit byte-oriented
    /// boundary such as a file or namespace name.
    pub(crate) fn push_utf8(self, target: &mut Vec<u8>) {
        let (bytes, len) = self.utf8_bytes();
        target.extend_from_slice(&bytes[..len]);
    }

    fn utf8_bytes(self) -> ([u8; 4], usize) {
        encode_uptex_utf8(self.code_point()).expect("private CJK token invariant")
    }
}

impl Dumpable for CjkToken {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.code_point().dump(target)?;
        self.category().dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let code_point = u32::undump(lines)?;
        let category = CjkCategory::undump(lines)?;
        Self::new(code_point, category).ok_or(FormatError::ParseError)
    }
}

/// upTeX `latin_ucs` の Unicode 欧文一文字 token。
///
/// 低い16 bitに符号位置、次のbyteに入力時のcatcodeを保持する。現在の
/// `\catcode` 表を後から変更しても、既に読んだtokenのcatcodeは変わらない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LatinUcsToken(u32);

impl LatinUcsToken {
    pub(crate) const fn new(code_point: u32, cat_code: CatCode) -> Option<Self> {
        // ASCII has dedicated compact token variants. Keeping the ranges
        // disjoint prevents a corrupt format from creating two identities
        // for the same character code.
        if code_point >= 0x80 && code_point <= MAX_LATIN_UCS_CODE {
            Some(Self(
                code_point | ((cat_code.public_number() as u32) << LATIN_UCS_CAT_CODE_SHIFT),
            ))
        } else {
            None
        }
    }

    pub(crate) const fn code_point(self) -> u32 {
        self.0 & LATIN_UCS_CODE_MASK
    }

    pub(crate) fn cat_code(self) -> CatCode {
        CatCode::from_public_number((self.0 >> LATIN_UCS_CAT_CODE_SHIFT) as i32)
            .expect("private latin_ucs token invariant")
    }

    pub(crate) fn print_utf8(self, printer: &mut impl Printer) {
        print_uptex_code_point(self.code_point(), printer);
    }

    pub(crate) fn push_utf8(self, target: &mut Vec<u8>) {
        push_uptex_utf8(self.code_point(), target);
    }

    pub(crate) fn has_raw_token_cat_code(self) -> bool {
        matches!(
            self.cat_code(),
            CatCode::LeftBrace
                | CatCode::RightBrace
                | CatCode::MathShift
                | CatCode::TabMark
                | CatCode::MacParam
                | CatCode::SupMark
                | CatCode::SubMark
                | CatCode::Spacer
                | CatCode::Letter
                | CatCode::OtherChar
        )
    }
}

impl Dumpable for LatinUcsToken {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.code_point().dump(target)?;
        self.cat_code().dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let code_point = u32::undump(lines)?;
        let cat_code = CatCode::undump(lines)?;
        Self::new(code_point, cat_code).ok_or(FormatError::ParseError)
    }
}

/// A token.
///
/// A token is either a character token or a control sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    LeftBrace(u8),
    RightBrace(u8),
    MathShift(u8),
    TabMark(u8),
    MacParam(u8),
    SuperMark(u8),
    SubMark(u8),
    /// A space token has character code 32 unless changed by \uppercase or \lowercase.
    Spacer(u8),
    Letter(u8),
    OtherChar(u8),
    LatinUcsChar(LatinUcsToken),
    CjkChar(CjkToken),

    /// Used to indicate end of stream.
    Null,

    /// A control sequence token.
    CSToken {
        cs: ControlSequence,
    },
}

impl Token {
    pub(crate) fn from_character_code_and_cat_code(
        code_point: u32,
        cat_code: CatCode,
    ) -> Option<Self> {
        // In upTeX case tables the right-hand side is a Unicode code point.
        // Only ASCII has a dedicated byte-token representation here: values
        // U+0080..U+00FF must remain UTF-8 latin_ucs tokens (not raw bytes).
        if code_point > 0x7F {
            return LatinUcsToken::new(code_point, cat_code).map(Self::LatinUcsChar);
        }
        let c = code_point as u8;
        Some(match cat_code {
            CatCode::LeftBrace => Self::LeftBrace(c),
            CatCode::RightBrace => Self::RightBrace(c),
            CatCode::MathShift => Self::MathShift(c),
            CatCode::TabMark => Self::TabMark(c),
            CatCode::MacParam => Self::MacParam(c),
            CatCode::SupMark => Self::SuperMark(c),
            CatCode::SubMark => Self::SubMark(c),
            CatCode::Spacer => Self::Spacer(c),
            CatCode::Letter => Self::Letter(c),
            CatCode::OtherChar => Self::OtherChar(c),
            _ => return None,
        })
    }

    pub(crate) fn character_code_and_cat_code(self) -> Option<(u32, CatCode)> {
        let pair = match self {
            Self::LeftBrace(c) => (u32::from(c), CatCode::LeftBrace),
            Self::RightBrace(c) => (u32::from(c), CatCode::RightBrace),
            Self::MathShift(c) => (u32::from(c), CatCode::MathShift),
            Self::TabMark(c) => (u32::from(c), CatCode::TabMark),
            Self::MacParam(c) => (u32::from(c), CatCode::MacParam),
            Self::SuperMark(c) => (u32::from(c), CatCode::SupMark),
            Self::SubMark(c) => (u32::from(c), CatCode::SubMark),
            Self::Spacer(c) => (u32::from(c), CatCode::Spacer),
            Self::Letter(c) => (u32::from(c), CatCode::Letter),
            Self::OtherChar(c) => (u32::from(c), CatCode::OtherChar),
            Self::LatinUcsChar(token) => (token.code_point(), token.cat_code()),
            Self::CjkChar(_) | Self::Null | Self::CSToken { .. } => return None,
        };
        Some(pair)
    }

    /// A space token with character code 32.
    /// See 289.
    pub const SPACE_TOKEN: Token = Token::Spacer(b' ');

    /// Starts an octal constant.
    /// See 438.
    pub const OCTAL_TOKEN: Token = Token::OtherChar(b'\'');
    /// Starts a hex constant.
    /// See 438.
    pub const HEX_TOKEN: Token = Token::OtherChar(b'"');
    /// Starts an alphabetic constant.
    /// See 438.
    pub const ALPHA_TOKEN: Token = Token::OtherChar(b'`');
    /// A decimal point.
    /// See 438.
    pub const POINT_TOKEN: Token = Token::OtherChar(b'.');
    /// A decimal comma as used in parts of Europe.
    /// See 438.
    pub const CONTINENTAL_POINT_TOKEN: Token = Token::OtherChar(b',');
    /// A plus sign.
    pub const PLUS_TOKEN: Token = Token::OtherChar(b'+');
    /// A minus sign.
    pub const MINUS_TOKEN: Token = Token::OtherChar(b'-');
    /// A right-brace token where the character is '}'.
    pub const RIGHT_BRACE_TOKEN: Token = Token::RightBrace(b'}');
    /// A left-brace token where the character is '{'.
    pub const LEFT_BRACE_TOKEN: Token = Token::LeftBrace(b'{');
    /// A math-shift token where the character is '$'.
    pub const MATH_SHIFT_TOKEN: Token = Token::MathShift(b'$');

    pub fn is_left_brace(&self) -> bool {
        matches!(self, Self::LeftBrace(_))
    }

    pub fn is_right_brace(&self) -> bool {
        matches!(self, Self::RightBrace(_))
    }

    pub fn is_command_left_brace(&self) -> bool {
        self.is_left_brace()
            || matches!(self, Self::LatinUcsChar(token) if token.cat_code() == CatCode::LeftBrace)
    }

    pub fn is_command_right_brace(&self) -> bool {
        self.is_right_brace()
            || matches!(self, Self::LatinUcsChar(token) if token.cat_code() == CatCode::RightBrace)
    }

    pub fn alignment_delta(self) -> i32 {
        if self.is_command_left_brace() {
            1
        } else if self.is_command_right_brace() {
            -1
        } else {
            0
        }
    }

    /// See 293. and 294.
    pub fn display(&self, printer: &mut impl Printer, eqtb: &Eqtb) {
        match *self {
            Token::LeftBrace(c)
            | Token::RightBrace(c)
            | Token::MathShift(c)
            | Token::TabMark(c)
            | Token::SuperMark(c)
            | Token::SubMark(c)
            | Token::Spacer(c)
            | Token::Letter(c)
            | Token::OtherChar(c) => printer.print(c),
            Token::LatinUcsChar(token) => {
                token.print_utf8(printer);
                if token.cat_code() == CatCode::MacParam {
                    token.print_utf8(printer);
                }
            }
            Token::CjkChar(token) => token.print_utf8(printer),
            Token::MacParam(c) => {
                printer.print(c);
                printer.print(c);
            }
            Token::CSToken { cs } => cs.print_cs(eqtb, printer),
            Token::Null => panic!("Should not be possible"),
        }
    }
}

/// fmt で `Token` を表す一行の符号。
///
/// NOTE: 変種名と値を別の行に書くと、`latex.fmt` では token 一個につき二行以上
/// になり、行数の大きな部分を占める。読む側も変種名を文字列比較で振り分ける。
/// 文字系の変種は「種別 * 256 + 文字コード」で一つの数に収まるので、一行の
/// 整数にまとめる。残りの四種は数のあとに従来どおり中身を続ける。
mod token_code {
    /// 文字系変種の個数。`LeftBrace` から `OtherChar` まで。
    pub(super) const CHAR_KINDS: u32 = 10;
    pub(super) const NULL: u32 = CHAR_KINDS * 256;
    pub(super) const LATIN_UCS: u32 = NULL + 1;
    pub(super) const CJK: u32 = NULL + 2;
    pub(super) const CS: u32 = NULL + 3;
    /// `MacroToken` はこれより大きい符号を自分の変種へ使う。
    pub(super) const MAX: u32 = NULL + 3;
}

impl Token {
    /// `MacroToken` が符号を先に読んでから中身を組み立てるための入口。
    pub(crate) const MAX_DUMP_CODE: u32 = token_code::MAX;

    /// この token を表す一行の符号を返す。文字系はここで値まで畳み込む。
    fn dump_code(&self) -> u32 {
        let kind = match self {
            Self::LeftBrace(_) => 0,
            Self::RightBrace(_) => 1,
            Self::MathShift(_) => 2,
            Self::TabMark(_) => 3,
            Self::MacParam(_) => 4,
            Self::SuperMark(_) => 5,
            Self::SubMark(_) => 6,
            Self::Spacer(_) => 7,
            Self::Letter(_) => 8,
            Self::OtherChar(_) => 9,
            Self::Null => return token_code::NULL,
            Self::LatinUcsChar(_) => return token_code::LATIN_UCS,
            Self::CjkChar(_) => return token_code::CJK,
            Self::CSToken { .. } => return token_code::CS,
        };
        let character = match *self {
            Self::LeftBrace(c)
            | Self::RightBrace(c)
            | Self::MathShift(c)
            | Self::TabMark(c)
            | Self::MacParam(c)
            | Self::SuperMark(c)
            | Self::SubMark(c)
            | Self::Spacer(c)
            | Self::Letter(c)
            | Self::OtherChar(c) => c,
            _ => unreachable!("文字系の変種だけがここへ来る"),
        };
        kind * 256 + character as u32
    }

    /// 符号のあとに続く中身を書く。文字系は符号に畳み込んであるので何も書かない。
    pub(crate) fn dump_after_code(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::LatinUcsChar(token) => token.dump(target),
            Self::CjkChar(token) => token.dump(target),
            Self::CSToken { cs } => cs.dump(target),
            _ => Ok(()),
        }
    }

    /// 読み終えた符号から token を組み立てる。
    pub(crate) fn undump_from_code<'a>(
        code: u32,
        lines: &mut impl Iterator<Item = &'a str>,
    ) -> Result<Self, FormatError> {
        match code {
            token_code::NULL => Ok(Self::Null),
            token_code::LATIN_UCS => {
                let token = LatinUcsToken::undump(lines)?;
                // 生の文字 token になり得る catcode だけを受ける。fmt は信用しない。
                if token.has_raw_token_cat_code() {
                    Ok(Self::LatinUcsChar(token))
                } else {
                    Err(FormatError::ParseError)
                }
            }
            token_code::CJK => Ok(Self::CjkChar(CjkToken::undump(lines)?)),
            token_code::CS => Ok(Self::CSToken {
                cs: ControlSequence::undump(lines)?,
            }),
            _ => {
                let kind = code / 256;
                let character = (code % 256) as u8;
                match kind {
                    0 => Ok(Self::LeftBrace(character)),
                    1 => Ok(Self::RightBrace(character)),
                    2 => Ok(Self::MathShift(character)),
                    3 => Ok(Self::TabMark(character)),
                    4 => Ok(Self::MacParam(character)),
                    5 => Ok(Self::SuperMark(character)),
                    6 => Ok(Self::SubMark(character)),
                    7 => Ok(Self::Spacer(character)),
                    8 => Ok(Self::Letter(character)),
                    9 => Ok(Self::OtherChar(character)),
                    _ => Err(FormatError::ParseError),
                }
            }
        }
    }
}

impl Dumpable for Token {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self.dump_code())?;
        self.dump_after_code(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let code = u32::undump(lines)?;
        if code > token_code::MAX {
            return Err(FormatError::ParseError);
        }
        Self::undump_from_code(code, lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cjk_token(code_point: u32, category: CjkCategory) -> CjkToken {
        CjkToken::new(code_point, category).unwrap()
    }

    #[test]
    fn 和文カテゴリーは十六から二十だけを受け入れる() {
        let expected = [
            CjkCategory::Kanji,
            CjkCategory::Kana,
            CjkCategory::OtherKChar,
            CjkCategory::Hangul,
            CjkCategory::Modifier,
        ];
        for (value, category) in (16..=20).zip(expected) {
            assert_eq!(CjkCategory::try_from(value), Ok(category));
        }
        assert!(CjkCategory::try_from(15).is_err());
        assert!(CjkCategory::try_from(21).is_err());
    }

    #[test]
    fn 和文字句はサロゲートを含む全unicode値を保持する() {
        for code_point in [0, 0xD7FF, 0xD800, 0xDFFF, 0xE000, 0x10_FFFF] {
            let token = cjk_token(code_point, CjkCategory::OtherKChar);
            assert_eq!(token.code_point(), code_point);
            assert_eq!(token.category(), CjkCategory::OtherKChar);
        }
        assert!(CjkToken::new(0x11_0000, CjkCategory::OtherKChar).is_none());
    }

    #[test]
    fn 和文字句をuptex互換のutf八バイト列にする() {
        let cases: &[(u32, &[u8])] = &[
            (0x7F, &[0x7F]),
            (0x80, &[0xC2, 0x80]),
            (0x7FF, &[0xDF, 0xBF]),
            (0x800, &[0xE0, 0xA0, 0x80]),
            (0xD800, &[0xED, 0xA0, 0x80]),
            (0xDFFF, &[0xED, 0xBF, 0xBF]),
            (0x1_0000, &[0xF0, 0x90, 0x80, 0x80]),
            (0x10_FFFF, &[0xF4, 0x8F, 0xBF, 0xBF]),
        ];
        for &(code_point, expected) in cases {
            let token = cjk_token(code_point, CjkCategory::OtherKChar);
            let (bytes, len) = token.utf8_bytes();
            assert_eq!(&bytes[..len], expected, "U+{code_point:04X}");
        }
    }

    #[test]
    fn dump_token() {
        let char_token = Token::LeftBrace(12);
        let latin_token = Token::LatinUcsChar(
            LatinUcsToken::new(0x00DF, CatCode::Letter).unwrap(),
        );
        let cjk_token = Token::CjkChar(cjk_token(0xD800, CjkCategory::Kana));
        let cs_token = Token::CSToken {
            cs: ControlSequence::Escaped(12),
        };

        let mut file = Vec::new();
        char_token.dump(&mut file).unwrap();
        latin_token.dump(&mut file).unwrap();
        cjk_token.dump(&mut file).unwrap();
        cs_token.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        let char_token_undumped = Token::undump(&mut lines).unwrap();
        let latin_token_undumped = Token::undump(&mut lines).unwrap();
        let cjk_token_undumped = Token::undump(&mut lines).unwrap();
        let cs_token_undumped = Token::undump(&mut lines).unwrap();
        assert_eq!(char_token, char_token_undumped);
        assert_eq!(latin_token, latin_token_undumped);
        assert_eq!(cjk_token, cjk_token_undumped);
        assert_eq!(cs_token, cs_token_undumped);
    }

    #[test]
    fn 和文字句の壊れたformatを拒否する() {
        for input in [
            // `2562` は `CjkChar` の符号である。
            "2562\n12354\n15\n",
            "2562\n12354\n21\n",
            "2562\n1114112\n18\n",
        ] {
            assert!(matches!(
                Token::undump(&mut input.lines()),
                Err(FormatError::ParseError)
            ));
        }

        for input in ["2562\n", "2562\n12354\n"] {
            assert!(matches!(
                Token::undump(&mut input.lines()),
                Err(FormatError::IncompleteFile)
            ));
        }
    }

    #[test]
    fn unicode欧文tokenは符号位置とcatcodeを固定する() {
        assert!(LatinUcsToken::new(0x7F, CatCode::Letter).is_none());
        assert!(LatinUcsToken::new(0x80, CatCode::Letter).is_some());
        let token = LatinUcsToken::new(0x2E7F, CatCode::Letter).unwrap();
        assert_eq!(token.code_point(), 0x2E7F);
        assert_eq!(token.cat_code(), CatCode::Letter);
        assert!(LatinUcsToken::new(0x2E80, CatCode::OtherChar).is_none());

        for input in [
            // `2561` は `LatinUcsChar` の符号である。
            "2561\n127\nLetter\n",
            "2561\n11904\nLetter\n",
            "2561\n223\nnot-a-catcode\n",
            "2561\n223\nEscape\n",
            "2561\n223\nActiveChar\n",
            "2561\n223\nInvalidChar\n",
        ] {
            assert!(Token::undump(&mut input.lines()).is_err());
        }
    }
}

use crate::eqtb::{ControlSequence, Eqtb};
use crate::format::{Dumpable, FormatError};
use crate::print::Printer;

use std::io::Write;

const MAX_CJK_CODE_POINT: u32 = 0x10_FFFF;
const CJK_CODE_POINT_MASK: u32 = 0x00FF_FFFF;
const CJK_CATEGORY_SHIFT: u32 = 24;

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
    CjkChar(CjkToken),

    /// Used to indicate end of stream.
    Null,

    /// A control sequence token.
    CSToken {
        cs: ControlSequence,
    },
}

impl Token {
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

impl Dumpable for Token {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::LeftBrace(c) => {
                writeln!(target, "LeftBrace")?;
                c.dump(target)?;
            }
            Self::RightBrace(c) => {
                writeln!(target, "RightBrace")?;
                c.dump(target)?;
            }
            Self::MathShift(c) => {
                writeln!(target, "MathShift")?;
                c.dump(target)?;
            }
            Self::TabMark(c) => {
                writeln!(target, "TabMark")?;
                c.dump(target)?;
            }
            Self::MacParam(c) => {
                writeln!(target, "MacParam")?;
                c.dump(target)?;
            }
            Self::SuperMark(c) => {
                writeln!(target, "SuperMark")?;
                c.dump(target)?;
            }
            Self::SubMark(c) => {
                writeln!(target, "SubMark")?;
                c.dump(target)?;
            }
            Self::Spacer(c) => {
                writeln!(target, "Spacer")?;
                c.dump(target)?;
            }
            Self::Letter(c) => {
                writeln!(target, "Letter")?;
                c.dump(target)?;
            }
            Self::OtherChar(c) => {
                writeln!(target, "OtherChar")?;
                c.dump(target)?;
            }
            Self::CjkChar(token) => {
                writeln!(target, "CjkChar")?;
                token.dump(target)?;
            }
            Self::Null => writeln!(target, "Null")?,
            Self::CSToken { cs } => {
                writeln!(target, "CSToken")?;
                cs.dump(target)?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "LeftBrace" => {
                let c = u8::undump(lines)?;
                Ok(Self::LeftBrace(c))
            }
            "RightBrace" => {
                let c = u8::undump(lines)?;
                Ok(Self::RightBrace(c))
            }
            "MathShift" => {
                let c = u8::undump(lines)?;
                Ok(Self::MathShift(c))
            }
            "TabMark" => {
                let c = u8::undump(lines)?;
                Ok(Self::TabMark(c))
            }
            "MacParam" => {
                let c = u8::undump(lines)?;
                Ok(Self::MacParam(c))
            }
            "SuperMark" => {
                let c = u8::undump(lines)?;
                Ok(Self::SuperMark(c))
            }
            "SubMark" => {
                let c = u8::undump(lines)?;
                Ok(Self::SubMark(c))
            }
            "Spacer" => {
                let c = u8::undump(lines)?;
                Ok(Self::Spacer(c))
            }
            "Letter" => {
                let c = u8::undump(lines)?;
                Ok(Self::Letter(c))
            }
            "OtherChar" => {
                let c = u8::undump(lines)?;
                Ok(Self::OtherChar(c))
            }
            "CjkChar" => Ok(Self::CjkChar(CjkToken::undump(lines)?)),
            "Null" => Ok(Self::Null),
            "CSToken" => {
                let cs = ControlSequence::undump(lines)?;
                Ok(Self::CSToken { cs })
            }
            _ => Err(FormatError::ParseError),
        }
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
        let cjk_token = Token::CjkChar(cjk_token(0xD800, CjkCategory::Kana));
        let cs_token = Token::CSToken {
            cs: ControlSequence::Escaped(12),
        };

        let mut file = Vec::new();
        char_token.dump(&mut file).unwrap();
        cjk_token.dump(&mut file).unwrap();
        cs_token.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        let char_token_undumped = Token::undump(&mut lines).unwrap();
        let cjk_token_undumped = Token::undump(&mut lines).unwrap();
        let cs_token_undumped = Token::undump(&mut lines).unwrap();
        assert_eq!(char_token, char_token_undumped);
        assert_eq!(cjk_token, cjk_token_undumped);
        assert_eq!(cs_token, cs_token_undumped);
    }

    #[test]
    fn 和文字句の壊れたformatを拒否する() {
        for input in [
            "CjkChar\n12354\n15\n",
            "CjkChar\n12354\n21\n",
            "CjkChar\n1114112\n18\n",
        ] {
            assert!(matches!(
                Token::undump(&mut input.lines()),
                Err(FormatError::ParseError)
            ));
        }

        for input in ["CjkChar\n", "CjkChar\n12354\n"] {
            assert!(matches!(
                Token::undump(&mut input.lines()),
                Err(FormatError::IncompleteFile)
            ));
        }
    }
}

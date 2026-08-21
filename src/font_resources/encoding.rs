//! dvips/pdfTeX系 `.enc` の限定されたPostScript encoding vector parser。
//!
//! PostScriptを実行せず、`/Name [ /glyph ... ] def` という宣言一個だけを読む。
//! PDFへはvectorそのものを埋めず、glyph nameを型付きresource層でDifferencesへ写す。

use std::fmt;

const GLYPH_COUNT: usize = 256;
const MAX_ENCODING_BYTES: usize = 1024 * 1024;
const MAX_NAME_BYTES: usize = 255;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EncodingVector {
    name: Vec<u8>,
    glyph_names: Vec<Vec<u8>>,
}

impl EncodingVector {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, EncodingError> {
        if bytes.len() > MAX_ENCODING_BYTES {
            return Err(EncodingError::FileTooLarge(bytes.len()));
        }
        if bytes.contains(&0) {
            return Err(EncodingError::NulByte);
        }

        let mut lexer = Lexer::new(bytes);
        let name = expect_literal_name(&mut lexer, "encoding name")?;
        expect_token(
            &mut lexer,
            TokenKind::LeftBracket,
            "`[` after encoding name",
        )?;

        let mut glyph_names = Vec::with_capacity(GLYPH_COUNT);
        loop {
            let token = lexer.next_token()?;
            match token {
                Some(Token::LiteralName { bytes, .. }) if glyph_names.len() < GLYPH_COUNT => {
                    glyph_names.push(bytes)
                }
                Some(Token::LiteralName { .. }) => {
                    return Err(EncodingError::WrongGlyphCount(GLYPH_COUNT + 1));
                }
                Some(Token::RightBracket { .. }) => break,
                Some(token) => {
                    return Err(EncodingError::UnexpectedToken {
                        offset: token.offset(),
                        expected: "glyph name or `]`",
                    });
                }
                None => return Err(EncodingError::UnexpectedEnd("`]`")),
            }
        }
        if glyph_names.len() != GLYPH_COUNT {
            return Err(EncodingError::WrongGlyphCount(glyph_names.len()));
        }
        match lexer.next_token()? {
            Some(Token::Word { bytes, .. }) if bytes == b"def" => {}
            Some(token) => {
                return Err(EncodingError::UnexpectedToken {
                    offset: token.offset(),
                    expected: "`def`",
                });
            }
            None => return Err(EncodingError::UnexpectedEnd("`def`")),
        }
        if let Some(token) = lexer.next_token()? {
            return Err(EncodingError::TrailingToken(token.offset()));
        }

        Ok(Self { name, glyph_names })
    }

    pub(crate) fn name(&self) -> &[u8] {
        &self.name
    }

    pub(crate) fn glyph_name(&self, code: u8) -> &[u8] {
        &self.glyph_names[usize::from(code)]
    }

    /// PDF name objectのdelimiterと非ASCII byteを `#XX` でescapeする。
    pub(crate) fn pdf_name(name: &[u8]) -> Vec<u8> {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let mut escaped = Vec::with_capacity(name.len() + 1);
        escaped.push(b'/');
        for &byte in name {
            if (b'!'..=b'~').contains(&byte)
                && !matches!(
                    byte,
                    b'#' | b'%' | b'(' | b')' | b'/' | b'<' | b'>' | b'[' | b']' | b'{' | b'}'
                )
            {
                escaped.push(byte);
            } else {
                escaped.push(b'#');
                escaped.push(HEX[usize::from(byte >> 4)]);
                escaped.push(HEX[usize::from(byte & 0x0f)]);
            }
        }
        escaped
    }
}

fn expect_literal_name(
    lexer: &mut Lexer<'_>,
    expected: &'static str,
) -> Result<Vec<u8>, EncodingError> {
    match lexer.next_token()? {
        Some(Token::LiteralName { bytes, .. }) => Ok(bytes),
        Some(token) => Err(EncodingError::UnexpectedToken {
            offset: token.offset(),
            expected,
        }),
        None => Err(EncodingError::UnexpectedEnd(expected)),
    }
}

fn expect_token(
    lexer: &mut Lexer<'_>,
    expected_kind: TokenKind,
    expected: &'static str,
) -> Result<(), EncodingError> {
    match lexer.next_token()? {
        Some(token) if token.kind() == expected_kind => Ok(()),
        Some(token) => Err(EncodingError::UnexpectedToken {
            offset: token.offset(),
            expected,
        }),
        None => Err(EncodingError::UnexpectedEnd(expected)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenKind {
    LiteralName,
    LeftBracket,
    RightBracket,
    Word,
}

#[derive(Debug)]
enum Token {
    LiteralName { bytes: Vec<u8>, offset: usize },
    LeftBracket { offset: usize },
    RightBracket { offset: usize },
    Word { bytes: Vec<u8>, offset: usize },
}

impl Token {
    fn kind(&self) -> TokenKind {
        match self {
            Self::LiteralName { .. } => TokenKind::LiteralName,
            Self::LeftBracket { .. } => TokenKind::LeftBracket,
            Self::RightBracket { .. } => TokenKind::RightBracket,
            Self::Word { .. } => TokenKind::Word,
        }
    }

    fn offset(&self) -> usize {
        match self {
            Self::LiteralName { offset, .. }
            | Self::LeftBracket { offset }
            | Self::RightBracket { offset }
            | Self::Word { offset, .. } => *offset,
        }
    }
}

struct Lexer<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Lexer<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn next_token(&mut self) -> Result<Option<Token>, EncodingError> {
        self.skip_space_and_comments();
        if self.position == self.bytes.len() {
            return Ok(None);
        }
        let offset = self.position;
        let first = self.bytes[self.position];
        self.position += 1;
        match first {
            b'[' => Ok(Some(Token::LeftBracket { offset })),
            b']' => Ok(Some(Token::RightBracket { offset })),
            b'/' => {
                let start = self.position;
                while self.position < self.bytes.len()
                    && !is_space(self.bytes[self.position])
                    && !is_delimiter(self.bytes[self.position])
                {
                    let byte = self.bytes[self.position];
                    if !byte.is_ascii_graphic() {
                        return Err(EncodingError::InvalidNameByte {
                            byte,
                            offset: self.position,
                        });
                    }
                    self.position += 1;
                    if self.position - start > MAX_NAME_BYTES {
                        return Err(EncodingError::NameTooLong(offset));
                    }
                }
                if self.position == start {
                    return Err(EncodingError::EmptyName(offset));
                }
                Ok(Some(Token::LiteralName {
                    bytes: self.bytes[start..self.position].to_vec(),
                    offset,
                }))
            }
            byte if byte.is_ascii_graphic() && !is_delimiter(byte) => {
                let start = offset;
                while self.position < self.bytes.len()
                    && !is_space(self.bytes[self.position])
                    && !is_delimiter(self.bytes[self.position])
                {
                    if !self.bytes[self.position].is_ascii_graphic() {
                        return Err(EncodingError::InvalidTokenByte {
                            byte: self.bytes[self.position],
                            offset: self.position,
                        });
                    }
                    self.position += 1;
                }
                Ok(Some(Token::Word {
                    bytes: self.bytes[start..self.position].to_vec(),
                    offset,
                }))
            }
            byte => Err(EncodingError::InvalidTokenByte { byte, offset }),
        }
    }

    fn skip_space_and_comments(&mut self) {
        loop {
            while self.position < self.bytes.len() && is_space(self.bytes[self.position]) {
                self.position += 1;
            }
            if self.position == self.bytes.len() || self.bytes[self.position] != b'%' {
                return;
            }
            while self.position < self.bytes.len()
                && !matches!(self.bytes[self.position], b'\r' | b'\n')
            {
                self.position += 1;
            }
        }
    }
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b'\0' | b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

fn is_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum EncodingError {
    FileTooLarge(usize),
    NulByte,
    EmptyName(usize),
    NameTooLong(usize),
    InvalidNameByte {
        byte: u8,
        offset: usize,
    },
    InvalidTokenByte {
        byte: u8,
        offset: usize,
    },
    UnexpectedToken {
        offset: usize,
        expected: &'static str,
    },
    UnexpectedEnd(&'static str),
    WrongGlyphCount(usize),
    TrailingToken(usize),
}

impl fmt::Display for EncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileTooLarge(size) => {
                write!(formatter, "encoding file is too large ({size} bytes)")
            }
            Self::NulByte => formatter.write_str("encoding file contains a NUL byte"),
            Self::EmptyName(offset) => write!(formatter, "empty PostScript name at byte {offset}"),
            Self::NameTooLong(offset) => {
                write!(formatter, "PostScript name at byte {offset} is too long")
            }
            Self::InvalidNameByte { byte, offset } => {
                write!(
                    formatter,
                    "invalid byte {byte:#04x} in PostScript name at byte {offset}"
                )
            }
            Self::InvalidTokenByte { byte, offset } => {
                write!(
                    formatter,
                    "invalid encoding token byte {byte:#04x} at byte {offset}"
                )
            }
            Self::UnexpectedToken { offset, expected } => {
                write!(
                    formatter,
                    "unexpected encoding token at byte {offset}; expected {expected}"
                )
            }
            Self::UnexpectedEnd(expected) => {
                write!(formatter, "encoding file ended before {expected}")
            }
            Self::WrongGlyphCount(count) => {
                write!(
                    formatter,
                    "encoding vector has {count} glyph names instead of 256"
                )
            }
            Self::TrailingToken(offset) => {
                write!(formatter, "trailing encoding token at byte {offset}")
            }
        }
    }
}

impl std::error::Error for EncodingError {}

#[cfg(test)]
mod tests {
    use super::{EncodingError, EncodingVector};

    fn encoding(count: usize) -> Vec<u8> {
        let mut bytes = b"% synthetic only\r\n/TestEncoding [\n".to_vec();
        for index in 0..count {
            bytes.extend_from_slice(format!("/g{index} ").as_bytes());
            if index % 16 == 15 {
                bytes.extend_from_slice(b"% row\r\n");
            }
        }
        bytes.extend_from_slice(b"] def\n");
        bytes
    }

    #[test]
    fn 二百五十六個のglyphとcommentを読む() {
        let parsed = EncodingVector::parse(&encoding(256)).unwrap();
        assert_eq!(parsed.name(), b"TestEncoding");
        assert_eq!(parsed.glyph_name(0), b"g0");
        assert_eq!(parsed.glyph_name(255), b"g255");
    }

    #[test]
    fn glyph数の過不足を拒む() {
        assert_eq!(
            EncodingVector::parse(&encoding(255)).unwrap_err(),
            EncodingError::WrongGlyphCount(255)
        );
        assert_eq!(
            EncodingVector::parse(&encoding(257)).unwrap_err(),
            EncodingError::WrongGlyphCount(257)
        );
    }

    #[test]
    fn postscriptを実行する余分なtokenを拒む() {
        let mut bytes = encoding(256);
        bytes.splice(0..0, b"save ".iter().copied());
        assert!(matches!(
            EncodingVector::parse(&bytes),
            Err(EncodingError::UnexpectedToken { .. })
        ));

        let mut trailing = encoding(256);
        trailing.extend_from_slice(b"readonly");
        assert!(matches!(
            EncodingVector::parse(&trailing),
            Err(EncodingError::TrailingToken(_))
        ));
    }

    #[test]
    fn nulと非ascii名を拒む() {
        let mut nul = encoding(256);
        nul.push(0);
        assert_eq!(EncodingVector::parse(&nul), Err(EncodingError::NulByte));

        let mut invalid = encoding(256);
        let position = invalid
            .windows(3)
            .position(|window| window == b"/g0")
            .unwrap()
            + 2;
        invalid[position] = 0xff;
        assert!(matches!(
            EncodingVector::parse(&invalid),
            Err(EncodingError::InvalidNameByte { byte: 0xff, .. })
        ));
    }

    #[test]
    fn pdf名のdelimiterとbyteをescapeする() {
        assert_eq!(EncodingVector::pdf_name(b"a b/#\xff"), b"/a#20b#2F#23#FF");
    }
}

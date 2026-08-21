//! pdfTeX互換map行の、I/Oを行わない限定parser。
//!
//! 公開されているpdfTeX manualのfield順
//! `tfmname psname fontflags special encodingfile fontfile` だけを扱う。
//! quoted specialはPostScriptとして実行・解釈せず、後段が未対応機能を
//! 明示的に拒めるよう、生文字列と既知の変形keywordの有無を保存する。

use std::fmt;

/// 一個のTFM名に対応するmap entry。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MapEntry {
    pub(crate) tfm_name: String,
    pub(crate) postscript_name: Option<String>,
    pub(crate) font_flags: Option<u32>,
    pub(crate) special: Option<QuotedSpecial>,
    pub(crate) encoding_file: Option<String>,
    pub(crate) font_file: Option<FontFile>,
}

/// 埋め込むfont resourceと、map markerが指定した埋め込み方。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FontFile {
    pub(crate) name: String,
    pub(crate) embedding: EmbedPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EmbedPolicy {
    /// `<font.pfb`: 使用glyphだけを部分埋め込みする。
    Subset,
    /// `<<font.pfb`: font file全体を埋め込む。
    Full,
}

/// 二重引用符で囲まれたspecial。
///
/// `raw` は引用符を除いた内容をそのまま保持する。このparserは値を評価せず、
/// とくにPostScript・shell command・file accessを一切実行しない。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuotedSpecial {
    pub(crate) raw: String,
    pub(crate) mentions_slant_font: bool,
    pub(crate) mentions_extend_font: bool,
}

/// map file全体を一行ずつ読む。
///
/// 空行と、quoted specialの外側にある`%`以降は無視する。同じmap file内の
/// 同一TFM entryは呼び出し側のcollision policyに委ね、ここでは順番どおり返す。
pub(crate) fn parse_map(bytes: &[u8]) -> Result<Vec<MapEntry>, MapParseError> {
    let mut entries = Vec::new();
    for (line_index, line_bytes) in bytes.split(|&byte| byte == b'\n').enumerate() {
        let line_number = line_index + 1;
        if let Some(offset) = line_bytes.iter().position(|&byte| byte == 0) {
            return Err(MapParseError::new(
                line_number,
                offset + 1,
                MapParseErrorKind::NulByte,
            ));
        }
        let line = std::str::from_utf8(line_bytes).map_err(|error| {
            MapParseError::new(
                line_number,
                error.valid_up_to() + 1,
                MapParseErrorKind::InvalidUtf8,
            )
        })?;
        if let Some(entry) = parse_line(line, line_number)? {
            entries.push(entry);
        }
    }
    Ok(entries)
}

fn parse_line(line: &str, line_number: usize) -> Result<Option<MapEntry>, MapParseError> {
    let tokens = lex_line(line, line_number)?;
    if tokens.is_empty() {
        return Ok(None);
    }

    let tfm_name = match &tokens[0].kind {
        TokenKind::Bare(value) if !value.starts_with('<') => value.clone(),
        _ => {
            return Err(MapParseError::new(
                line_number,
                tokens[0].column,
                MapParseErrorKind::ExpectedTfmName,
            ));
        }
    };

    let mut entry = MapEntry {
        tfm_name,
        postscript_name: None,
        font_flags: None,
        special: None,
        encoding_file: None,
        font_file: None,
    };

    let mut index = 1;
    if let Some(token) = tokens.get(index) {
        if let TokenKind::Bare(value) = &token.kind {
            if !value.starts_with('<') {
                if looks_like_decimal_integer(value) {
                    entry.font_flags = Some(parse_font_flags(value, line_number, token.column)?);
                } else {
                    entry.postscript_name = Some(value.clone());
                }
                index += 1;
            }
        }
    }

    if entry.postscript_name.is_some() {
        if let Some(token) = tokens.get(index) {
            if let TokenKind::Bare(value) = &token.kind {
                if looks_like_decimal_integer(value) {
                    entry.font_flags = Some(parse_font_flags(value, line_number, token.column)?);
                    index += 1;
                }
            }
        }
    }

    for token in &tokens[index..] {
        match &token.kind {
            TokenKind::Quoted(raw) => {
                if entry.special.is_some() {
                    return Err(MapParseError::new(
                        line_number,
                        token.column,
                        MapParseErrorKind::DuplicateResource(ResourceKind::Special),
                    ));
                }
                let words: Vec<&str> = raw.split_ascii_whitespace().collect();
                entry.special = Some(QuotedSpecial {
                    raw: raw.clone(),
                    mentions_slant_font: words.contains(&"SlantFont"),
                    mentions_extend_font: words.contains(&"ExtendFont"),
                });
            }
            TokenKind::Bare(value) if value.starts_with('<') => {
                parse_resource(value, line_number, token.column, &mut entry)?;
            }
            TokenKind::Bare(value) if looks_like_decimal_integer(value) => {
                return Err(MapParseError::new(
                    line_number,
                    token.column,
                    MapParseErrorKind::MisplacedFontFlags,
                ));
            }
            TokenKind::Bare(_) => {
                return Err(MapParseError::new(
                    line_number,
                    token.column,
                    MapParseErrorKind::UnexpectedBareField,
                ));
            }
        }
    }

    Ok(Some(entry))
}

fn parse_font_flags(value: &str, line: usize, column: usize) -> Result<u32, MapParseError> {
    value
        .parse()
        .map_err(|_| MapParseError::new(line, column, MapParseErrorKind::InvalidFontFlags))
}

fn looks_like_decimal_integer(value: &str) -> bool {
    let digits = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn parse_resource(
    value: &str,
    line: usize,
    column: usize,
    entry: &mut MapEntry,
) -> Result<(), MapParseError> {
    let (marker, name) = if let Some(name) = value.strip_prefix("<<") {
        (ResourceMarker::Full, name)
    } else if let Some(name) = value.strip_prefix("<[") {
        (ResourceMarker::BracketedEncoding, name)
    } else {
        (ResourceMarker::Subset, &value[1..])
    };

    if name.is_empty() {
        return Err(MapParseError::new(
            line,
            column,
            MapParseErrorKind::EmptyResourceName,
        ));
    }

    if name.ends_with(".enc") {
        if marker == ResourceMarker::Full {
            return Err(MapParseError::new(
                line,
                column,
                MapParseErrorKind::FullEmbeddingEncoding,
            ));
        }
        if entry.encoding_file.is_some() {
            return Err(MapParseError::new(
                line,
                column,
                MapParseErrorKind::DuplicateResource(ResourceKind::Encoding),
            ));
        }
        entry.encoding_file = Some(name.to_owned());
        return Ok(());
    }

    if marker == ResourceMarker::BracketedEncoding {
        return Err(MapParseError::new(
            line,
            column,
            MapParseErrorKind::BracketedResourceMustBeEncoding,
        ));
    }
    if entry.font_file.is_some() {
        return Err(MapParseError::new(
            line,
            column,
            MapParseErrorKind::DuplicateResource(ResourceKind::Font),
        ));
    }
    let embedding = match marker {
        ResourceMarker::Subset => EmbedPolicy::Subset,
        ResourceMarker::Full => EmbedPolicy::Full,
        ResourceMarker::BracketedEncoding => {
            return Err(MapParseError::new(
                line,
                column,
                MapParseErrorKind::BracketedResourceMustBeEncoding,
            ));
        }
    };
    entry.font_file = Some(FontFile {
        name: name.to_owned(),
        embedding,
    });
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResourceMarker {
    Subset,
    Full,
    BracketedEncoding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Token {
    kind: TokenKind,
    /// 1-based byte column。map formatのresource名はbyte-orientedなのでbyteで報告する。
    column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TokenKind {
    Bare(String),
    Quoted(String),
}

fn lex_line(line: &str, line_number: usize) -> Result<Vec<Token>, MapParseError> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == bytes.len() || bytes[cursor] == b'%' {
            break;
        }

        let start = cursor;
        if bytes[cursor] == b'"' {
            cursor += 1;
            let content_start = cursor;
            while cursor < bytes.len() && bytes[cursor] != b'"' {
                cursor += 1;
            }
            if cursor == bytes.len() {
                return Err(MapParseError::new(
                    line_number,
                    start + 1,
                    MapParseErrorKind::UnterminatedQuotedSpecial,
                ));
            }
            let raw = line[content_start..cursor].to_owned();
            cursor += 1;
            if cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'%'
            {
                return Err(MapParseError::new(
                    line_number,
                    cursor + 1,
                    MapParseErrorKind::MissingFieldSeparator,
                ));
            }
            tokens.push(Token {
                kind: TokenKind::Quoted(raw),
                column: start + 1,
            });
        } else {
            while cursor < bytes.len()
                && !bytes[cursor].is_ascii_whitespace()
                && bytes[cursor] != b'%'
            {
                if bytes[cursor] == b'"' {
                    return Err(MapParseError::new(
                        line_number,
                        cursor + 1,
                        MapParseErrorKind::UnexpectedQuote,
                    ));
                }
                cursor += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Bare(line[start..cursor].to_owned()),
                column: start + 1,
            });
        }
    }
    Ok(tokens)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceKind {
    Encoding,
    Font,
    Special,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MapParseError {
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) kind: MapParseErrorKind,
}

impl MapParseError {
    fn new(line: usize, column: usize, kind: MapParseErrorKind) -> Self {
        Self { line, column, kind }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MapParseErrorKind {
    InvalidUtf8,
    NulByte,
    ExpectedTfmName,
    InvalidFontFlags,
    MisplacedFontFlags,
    UnterminatedQuotedSpecial,
    UnexpectedQuote,
    MissingFieldSeparator,
    UnexpectedBareField,
    EmptyResourceName,
    FullEmbeddingEncoding,
    BracketedResourceMustBeEncoding,
    DuplicateResource(ResourceKind),
}

impl fmt::Display for MapParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "map line {}, byte column {}: {}",
            self.line, self.column, self.kind
        )
    }
}

impl fmt::Display for MapParseErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 => formatter.write_str("input is not UTF-8"),
            Self::NulByte => formatter.write_str("NUL byte is not allowed"),
            Self::ExpectedTfmName => formatter.write_str("expected a TFM name as the first field"),
            Self::InvalidFontFlags => {
                formatter.write_str("font flags are not a 32-bit unsigned integer")
            }
            Self::MisplacedFontFlags => {
                formatter.write_str("font flags are not in the fixed leading fields")
            }
            Self::UnterminatedQuotedSpecial => {
                formatter.write_str("quoted special is not terminated")
            }
            Self::UnexpectedQuote => {
                formatter.write_str("quote must start a separate special field")
            }
            Self::MissingFieldSeparator => {
                formatter.write_str("quoted special must be followed by whitespace")
            }
            Self::UnexpectedBareField => formatter.write_str("unexpected unmarked field"),
            Self::EmptyResourceName => formatter.write_str("resource marker has no file name"),
            Self::FullEmbeddingEncoding => {
                formatter.write_str("an encoding cannot use the full-font marker")
            }
            Self::BracketedResourceMustBeEncoding => {
                formatter.write_str("`<[` must name an .enc file")
            }
            Self::DuplicateResource(kind) => write!(formatter, "duplicate {kind:?} resource"),
        }
    }
}

impl std::error::Error for MapParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn コメントと空行を読み飛ばす() {
        let entries = parse_map(b"% first\n \t\r\ncmr10 CMR10 % tail\n").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tfm_name, "cmr10");
        assert_eq!(entries[0].postscript_name.as_deref(), Some("CMR10"));
    }

    #[test]
    fn 部分埋め込みを読む() {
        let entry = &parse_map(b"cmr10 CMR10 <cmr10.pfb\n").unwrap()[0];
        assert_eq!(
            entry.font_file,
            Some(FontFile {
                name: "cmr10.pfb".to_owned(),
                embedding: EmbedPolicy::Subset,
            })
        );
    }

    #[test]
    fn 完全埋め込みを区別する() {
        let entry = &parse_map(b"fullfont PSName <<font.pfb").unwrap()[0];
        assert_eq!(
            entry.font_file.as_ref().unwrap().embedding,
            EmbedPolicy::Full
        );
    }

    #[test]
    fn 符号化とフォントの順序を問わない() {
        let entries = parse_map(
            b"first FirstPS <first.enc <first.pfb\nsecond SecondPS <second.pfb <second.enc",
        )
        .unwrap();
        assert_eq!(entries[0].encoding_file.as_deref(), Some("first.enc"));
        assert_eq!(entries[0].font_file.as_ref().unwrap().name, "first.pfb");
        assert_eq!(entries[1].encoding_file.as_deref(), Some("second.enc"));
        assert_eq!(entries[1].font_file.as_ref().unwrap().name, "second.pfb");
    }

    #[test]
    fn 角括弧つき符号化指定を読む() {
        let entry = &parse_map(b"encoded Encoded <[vector.enc <font.pfb").unwrap()[0];
        assert_eq!(entry.encoding_file.as_deref(), Some("vector.enc"));
    }

    #[test]
    fn フォント旗はps名の有無によらず読める() {
        let entries = parse_map(b"withps WithPS 34 <one.pfb\nwithoutps 4 <two.pfb").unwrap();
        assert_eq!(entries[0].postscript_name.as_deref(), Some("WithPS"));
        assert_eq!(entries[0].font_flags, Some(34));
        assert_eq!(entries[1].postscript_name, None);
        assert_eq!(entries[1].font_flags, Some(4));
    }

    #[test]
    fn tfm名だけの行も保持する() {
        let entry = &parse_map(b"scalabletype3").unwrap()[0];
        assert_eq!(entry.tfm_name, "scalabletype3");
        assert_eq!(entry.postscript_name, None);
        assert_eq!(entry.font_file, None);
    }

    #[test]
    fn specialは実行せず生文字列と変形印を保持する() {
        let entry = &parse_map(
            b"slanted Slanted \"0.167 SlantFont 1.2 ExtendFont (never-run) run\" <font.pfb",
        )
        .unwrap()[0];
        let special = entry.special.as_ref().unwrap();
        assert_eq!(
            special.raw,
            "0.167 SlantFont 1.2 ExtendFont (never-run) run"
        );
        assert!(special.mentions_slant_font);
        assert!(special.mentions_extend_font);
    }

    #[test]
    fn special内の百分率はコメントにしない() {
        let entry = &parse_map(b"font PS \"a%b ReEncodeFont\" <font.pfb % comment").unwrap()[0];
        assert_eq!(entry.special.as_ref().unwrap().raw, "a%b ReEncodeFont");
    }

    #[test]
    fn 同種の資源を二度指定できない() {
        let encoding = parse_map(b"font PS <one.enc <two.enc").unwrap_err();
        assert_eq!(
            encoding.kind,
            MapParseErrorKind::DuplicateResource(ResourceKind::Encoding)
        );
        let font = parse_map(b"font PS <one.pfb <<two.pfb").unwrap_err();
        assert_eq!(
            font.kind,
            MapParseErrorKind::DuplicateResource(ResourceKind::Font)
        );
        let special = parse_map(b"font PS \"one\" \"two\"").unwrap_err();
        assert_eq!(
            special.kind,
            MapParseErrorKind::DuplicateResource(ResourceKind::Special)
        );
    }

    #[test]
    fn 壊れた引用符を誤りにする() {
        assert_eq!(
            parse_map(b"font PS \"not closed").unwrap_err().kind,
            MapParseErrorKind::UnterminatedQuotedSpecial
        );
        assert_eq!(
            parse_map(b"font PS\"bad\"").unwrap_err().kind,
            MapParseErrorKind::UnexpectedQuote
        );
    }

    #[test]
    fn 空または誤種別の資源名を拒む() {
        assert_eq!(
            parse_map(b"font PS <").unwrap_err().kind,
            MapParseErrorKind::EmptyResourceName
        );
        assert_eq!(
            parse_map(b"font PS <[font.pfb").unwrap_err().kind,
            MapParseErrorKind::BracketedResourceMustBeEncoding
        );
        assert_eq!(
            parse_map(b"font PS <<vector.enc").unwrap_err().kind,
            MapParseErrorKind::FullEmbeddingEncoding
        );
    }

    #[test]
    fn 範囲外のフォント旗を拒む() {
        let error = parse_map(b"font 999999999999999999999").unwrap_err();
        assert_eq!(error.kind, MapParseErrorKind::InvalidFontFlags);
    }

    #[test]
    fn nulを位置つきで拒む() {
        let error = parse_map(b"ok PS\nbad\0 PS").unwrap_err();
        assert_eq!(error.line, 2);
        assert_eq!(error.column, 4);
        assert_eq!(error.kind, MapParseErrorKind::NulByte);
    }

    #[test]
    fn utf8でない入力を位置つきで拒む() {
        let error = parse_map(b"ok PS\nbad \xff").unwrap_err();
        assert_eq!(error.line, 2);
        assert_eq!(error.column, 5);
        assert_eq!(error.kind, MapParseErrorKind::InvalidUtf8);
    }
}

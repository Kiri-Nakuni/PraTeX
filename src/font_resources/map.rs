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
    /// map行に書かれた順序を保ったdownload resource。
    ///
    /// parserはsuffixからencoding/font/headerを決めない。対応可否と重複policyは、
    /// 実際にそのTFMを使うloaderが決める。
    pub(crate) resources: Vec<MapResource>,
}

/// map markerと、その直後または次のfieldに書かれたresource名。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MapResource {
    pub(crate) name: String,
    pub(crate) marker: ResourceMarker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceMarker {
    /// `<font.pfb`または`< font.pfb`。
    Subset,
    /// `<<font.pfb`または`<< font.pfb`。
    Full,
    /// `<[vector.enc`または`<[ vector.enc`。
    BracketedEncoding,
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
        resources: Vec::new(),
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

    while let Some(token) = tokens.get(index) {
        match &token.kind {
            TokenKind::Quoted(raw) => {
                if entry.special.is_some() {
                    return Err(MapParseError::new(
                        line_number,
                        token.column,
                        MapParseErrorKind::DuplicateSpecial,
                    ));
                }
                let words: Vec<&str> = raw.split_ascii_whitespace().collect();
                entry.special = Some(QuotedSpecial {
                    raw: raw.clone(),
                    mentions_slant_font: words.contains(&"SlantFont"),
                    mentions_extend_font: words.contains(&"ExtendFont"),
                });
                index += 1;
            }
            TokenKind::Bare(value) if value.starts_with('<') => {
                let (resource, consumed) =
                    parse_resource(value, tokens.get(index + 1), line_number, token.column)?;
                entry.resources.push(resource);
                index += consumed;
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
    next: Option<&Token>,
    line: usize,
    column: usize,
) -> Result<(MapResource, usize), MapParseError> {
    let (marker, name) = if let Some(name) = value.strip_prefix("<<") {
        (ResourceMarker::Full, name)
    } else if let Some(name) = value.strip_prefix("<[") {
        (ResourceMarker::BracketedEncoding, name)
    } else {
        (ResourceMarker::Subset, &value[1..])
    };

    if !name.is_empty() {
        return Ok((
            MapResource {
                name: name.to_owned(),
                marker,
            },
            1,
        ));
    }

    let Some(Token {
        kind: TokenKind::Bare(name),
        ..
    }) = next
    else {
        return Err(MapParseError::new(
            line,
            column,
            MapParseErrorKind::EmptyResourceName,
        ));
    };
    if name.starts_with('<') {
        return Err(MapParseError::new(
            line,
            column,
            MapParseErrorKind::EmptyResourceName,
        ));
    }
    Ok((
        MapResource {
            name: name.clone(),
            marker,
        },
        2,
    ))
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
    DuplicateSpecial,
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
            Self::DuplicateSpecial => formatter.write_str("duplicate quoted special"),
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
            entry.resources,
            vec![MapResource {
                name: "cmr10.pfb".to_owned(),
                marker: ResourceMarker::Subset,
            }]
        );
    }

    #[test]
    fn 完全埋め込みを区別する() {
        let entry = &parse_map(b"fullfont PSName <<font.pfb").unwrap()[0];
        assert_eq!(entry.resources[0].marker, ResourceMarker::Full);
    }

    #[test]
    fn 符号化とフォントの順序を問わない() {
        let entries = parse_map(
            b"first FirstPS <first.enc <first.pfb\nsecond SecondPS <second.pfb <second.enc",
        )
        .unwrap();
        assert_eq!(entries[0].resources[0].name, "first.enc");
        assert_eq!(entries[0].resources[1].name, "first.pfb");
        assert_eq!(entries[1].resources[0].name, "second.pfb");
        assert_eq!(entries[1].resources[1].name, "second.enc");
    }

    #[test]
    fn 角括弧つき符号化指定を読む() {
        let entry = &parse_map(b"encoded Encoded <[vector.enc <font.pfb").unwrap()[0];
        assert_eq!(
            entry.resources[0],
            MapResource {
                name: "vector.enc".to_owned(),
                marker: ResourceMarker::BracketedEncoding,
            }
        );
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
        assert!(entry.resources.is_empty());
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
    fn specialを二度指定できない() {
        let special = parse_map(b"font PS \"one\" \"two\"").unwrap_err();
        assert_eq!(special.kind, MapParseErrorKind::DuplicateSpecial);
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
    fn 空の資源名を拒む() {
        assert_eq!(
            parse_map(b"font PS <").unwrap_err().kind,
            MapParseErrorKind::EmptyResourceName
        );
    }

    #[test]
    fn 実物clm行の複数資源を順序と印つきで保つ() {
        let entry = &parse_map(
            b"frankClmNkd FrankRuehlCLM-Medium-Menukad \" HE8Encoding ReEncodeFont \" <he8.enc <<FrankRuehlCLM-Medium-Menukad.t3 <FrankRuehlCLM-Medium.pfb",
        )
        .unwrap()[0];
        assert_eq!(
            entry.resources,
            vec![
                MapResource {
                    name: "he8.enc".to_owned(),
                    marker: ResourceMarker::Subset,
                },
                MapResource {
                    name: "FrankRuehlCLM-Medium-Menukad.t3".to_owned(),
                    marker: ResourceMarker::Full,
                },
                MapResource {
                    name: "FrankRuehlCLM-Medium.pfb".to_owned(),
                    marker: ResourceMarker::Subset,
                },
            ]
        );
    }

    #[test]
    fn 単独の小なり記号と次の語を一つの資源として読む() {
        let entry = &parse_map(b"plimsoll < plimsoll.enc < plimsoll.pfb").unwrap()[0];
        assert_eq!(
            entry.resources,
            vec![
                MapResource {
                    name: "plimsoll.enc".to_owned(),
                    marker: ResourceMarker::Subset,
                },
                MapResource {
                    name: "plimsoll.pfb".to_owned(),
                    marker: ResourceMarker::Subset,
                },
            ]
        );
    }

    #[test]
    fn 単独の資源印が行末なら位置つきで拒む() {
        for (line, expected_column) in [("font PS <", 9), ("font PS <<", 9), ("font PS <[", 9)] {
            let error = parse_map(line.as_bytes()).unwrap_err();
            assert_eq!(error.line, 1);
            assert_eq!(error.column, expected_column);
            assert_eq!(error.kind, MapParseErrorKind::EmptyResourceName);
        }
    }

    #[test]
    fn 符号化と補助資源とfont資源を混在順のまま保つ() {
        let entry =
            &parse_map(b"font PS <font.pfb <<helper.t3 <[vector.data <<font.otf").unwrap()[0];
        assert_eq!(
            entry.resources,
            vec![
                MapResource {
                    name: "font.pfb".to_owned(),
                    marker: ResourceMarker::Subset,
                },
                MapResource {
                    name: "helper.t3".to_owned(),
                    marker: ResourceMarker::Full,
                },
                MapResource {
                    name: "vector.data".to_owned(),
                    marker: ResourceMarker::BracketedEncoding,
                },
                MapResource {
                    name: "font.otf".to_owned(),
                    marker: ResourceMarker::Full,
                },
            ]
        );
    }

    #[test]
    fn 同名資源もparserでは失わず二個保つ() {
        let entry = &parse_map(b"font PS <same.pfb <<same.pfb").unwrap()[0];
        assert_eq!(entry.resources.len(), 2);
        assert_eq!(entry.resources[0].name, "same.pfb");
        assert_eq!(entry.resources[0].marker, ResourceMarker::Subset);
        assert_eq!(entry.resources[1].name, "same.pfb");
        assert_eq!(entry.resources[1].marker, ResourceMarker::Full);
    }

    #[test]
    fn 分離した三種の資源印を同じ構造へ正規化する() {
        let entry = &parse_map(b"font PS < subset.pfb << full.t3 <[ vector.enc").unwrap()[0];
        assert_eq!(
            entry.resources,
            vec![
                MapResource {
                    name: "subset.pfb".to_owned(),
                    marker: ResourceMarker::Subset,
                },
                MapResource {
                    name: "full.t3".to_owned(),
                    marker: ResourceMarker::Full,
                },
                MapResource {
                    name: "vector.enc".to_owned(),
                    marker: ResourceMarker::BracketedEncoding,
                },
            ]
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

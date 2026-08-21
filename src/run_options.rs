//! TeXへ渡す入力行から、rtex自身の少数のoptionだけを分ける。

use std::ffi::{OsStr, OsString};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputFormat {
    Dvi,
    Pdf,
}

impl OutputFormat {
    fn parse(value: &str) -> Result<Self, RunOptionError> {
        match value {
            "dvi" => Ok(Self::Dvi),
            "pdf" => Ok(Self::Pdf),
            other => Err(RunOptionError::UnknownOutputFormat(other.to_owned())),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParsedArguments {
    pub(crate) output_format: OutputFormat,
    /// Type 1 埋込みを明示的に有効にする map の論理名または物理 path。
    pub(crate) pdf_font_map: Option<OsString>,
    pub(crate) tex_arguments: Vec<OsString>,
}

/// OS文字列からASCIIのoption prefixだけを外す。
///
/// `--pdf-font-map=...` の値はファイル名なので、UTF-8へ変換してから分割しない。
#[cfg(unix)]
fn strip_os_prefix(value: &OsStr, prefix: &str) -> Option<OsString> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    value
        .as_bytes()
        .strip_prefix(prefix.as_bytes())
        .map(|remainder| OsString::from_vec(remainder.to_vec()))
}

#[cfg(windows)]
fn strip_os_prefix(value: &OsStr, prefix: &str) -> Option<OsString> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let value: Vec<u16> = value.encode_wide().collect();
    let prefix: Vec<u16> = prefix.encode_utf16().collect();
    value
        .strip_prefix(prefix.as_slice())
        .map(OsString::from_wide)
}

#[cfg(not(any(unix, windows)))]
fn strip_os_prefix(value: &OsStr, prefix: &str) -> Option<OsString> {
    value
        .to_str()
        .and_then(|value| value.strip_prefix(prefix))
        .map(OsString::from)
}

/// web2c系で一般的な `-output-format=pdf` と、そのlong-option表記を読む。
///
/// rtexが知らない引数は従来どおりTeXの最初の入力行へ残す。`--` より後ろもすべて
/// TeXへ渡すので、optionに似たファイル名や制御綴も失わない。
pub(crate) fn parse_arguments(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<ParsedArguments, RunOptionError> {
    let mut output_format = OutputFormat::Dvi;
    let mut pdf_font_map = None;
    let mut tex_arguments = Vec::new();
    let mut arguments = arguments.into_iter();
    let mut parsing_options = true;

    while let Some(argument) = arguments.next() {
        if parsing_options && argument == "--" {
            parsing_options = false;
            continue;
        }
        if parsing_options {
            if let Some(argument_text) = argument.to_str() {
                if let Some(value) = argument_text
                    .strip_prefix("-output-format=")
                    .or_else(|| argument_text.strip_prefix("--output-format="))
                {
                    output_format = OutputFormat::parse(value)?;
                    continue;
                }
            }
            if argument == "-output-format" || argument == "--output-format" {
                let value = arguments
                    .next()
                    .ok_or(RunOptionError::MissingOutputFormat)?;
                output_format = value.to_str().map(OutputFormat::parse).unwrap_or_else(|| {
                    Err(RunOptionError::UnknownOutputFormat(
                        value.to_string_lossy().into_owned(),
                    ))
                })?;
                continue;
            }
            if let Some(value) = strip_os_prefix(&argument, "--pdf-font-map=") {
                if value.is_empty() {
                    return Err(RunOptionError::MissingPdfFontMap);
                }
                pdf_font_map = Some(value);
                continue;
            }
            if argument == "--pdf-font-map" {
                let value = arguments
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or(RunOptionError::MissingPdfFontMap)?;
                pdf_font_map = Some(value);
                continue;
            }
        }
        tex_arguments.push(argument);
    }

    if pdf_font_map.is_some() && output_format != OutputFormat::Pdf {
        return Err(RunOptionError::PdfFontMapRequiresPdf);
    }

    Ok(ParsedArguments {
        output_format,
        pdf_font_map,
        tex_arguments,
    })
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RunOptionError {
    MissingOutputFormat,
    UnknownOutputFormat(String),
    MissingPdfFontMap,
    PdfFontMapRequiresPdf,
}

impl fmt::Display for RunOptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOutputFormat => formatter.write_str("missing value for -output-format"),
            Self::UnknownOutputFormat(format) => {
                write!(
                    formatter,
                    "unknown output format `{format}` (expected dvi or pdf)"
                )
            }
            Self::MissingPdfFontMap => formatter.write_str("missing value for --pdf-font-map"),
            Self::PdfFontMapRequiresPdf => {
                formatter.write_str("--pdf-font-map requires --output-format=pdf")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_arguments, OutputFormat, RunOptionError};
    use std::ffi::OsString;

    fn strings(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn 既定はdviで入力順を変えない() {
        let parsed = parse_arguments(strings(&["&plain", "hello.tex"])).unwrap();
        assert_eq!(parsed.output_format, OutputFormat::Dvi);
        assert_eq!(parsed.pdf_font_map, None);
        assert_eq!(parsed.tex_arguments, strings(&["&plain", "hello.tex"]));
    }

    #[test]
    fn 二種類の綴りと分離値でpdfを選べる() {
        for arguments in [
            strings(&["-output-format=pdf", "hello.tex"]),
            strings(&["--output-format=pdf", "hello.tex"]),
            strings(&["-output-format", "pdf", "hello.tex"]),
            strings(&["--output-format", "pdf", "hello.tex"]),
        ] {
            let parsed = parse_arguments(arguments).unwrap();
            assert_eq!(parsed.output_format, OutputFormat::Pdf);
            assert_eq!(parsed.pdf_font_map, None);
            assert_eq!(parsed.tex_arguments, strings(&["hello.tex"]));
        }
    }

    #[test]
    fn 後の指定が勝ち二重dash以後はtexへ渡す() {
        let parsed = parse_arguments(strings(&[
            "-output-format=pdf",
            "--output-format=dvi",
            "--",
            "--output-format=pdf",
            "hello.tex",
        ]))
        .unwrap();
        assert_eq!(parsed.output_format, OutputFormat::Dvi);
        assert_eq!(parsed.pdf_font_map, None);
        assert_eq!(
            parsed.tex_arguments,
            strings(&["--output-format=pdf", "hello.tex"])
        );
    }

    #[test]
    fn pdf_font_mapは結合値と分離値をos文字列のまま受け取る() {
        for arguments in [
            strings(&[
                "--output-format=pdf",
                "--pdf-font-map=地図/pdftex.map",
                "hello.tex",
            ]),
            strings(&[
                "--pdf-font-map",
                "地図/pdftex.map",
                "--output-format",
                "pdf",
                "hello.tex",
            ]),
        ] {
            let parsed = parse_arguments(arguments).unwrap();
            assert_eq!(parsed.output_format, OutputFormat::Pdf);
            assert_eq!(parsed.pdf_font_map, Some(OsString::from("地図/pdftex.map")));
            assert_eq!(parsed.tex_arguments, strings(&["hello.tex"]));
        }
    }

    #[test]
    fn pdf_font_mapはpdf以外や空値へ黙って効かせない() {
        assert_eq!(
            parse_arguments(strings(&["--pdf-font-map=fonts.map"])),
            Err(RunOptionError::PdfFontMapRequiresPdf)
        );
        assert_eq!(
            parse_arguments(strings(&[
                "--output-format=pdf",
                "--pdf-font-map=fonts.map",
                "--output-format=dvi",
            ])),
            Err(RunOptionError::PdfFontMapRequiresPdf)
        );
        assert_eq!(
            parse_arguments(strings(&["--output-format=pdf", "--pdf-font-map="])),
            Err(RunOptionError::MissingPdfFontMap)
        );
        assert_eq!(
            parse_arguments(strings(&["--output-format=pdf", "--pdf-font-map"])),
            Err(RunOptionError::MissingPdfFontMap)
        );
    }

    #[test]
    fn 不明値と欠けた値を早く報せる() {
        assert_eq!(
            parse_arguments(strings(&["-output-format=xps"])),
            Err(RunOptionError::UnknownOutputFormat("xps".to_owned()))
        );
        assert_eq!(
            parse_arguments(strings(&["--output-format"])),
            Err(RunOptionError::MissingOutputFormat)
        );
    }

    #[cfg(unix)]
    #[test]
    fn 非utf8の未知引数をtex側へそのまま残す() {
        use std::os::unix::ffi::OsStringExt;

        let argument = OsString::from_vec(vec![b'n', 0xff, b'.', b't', b'e', b'x']);
        let parsed = parse_arguments([argument.clone()]).unwrap();
        assert_eq!(parsed.output_format, OutputFormat::Dvi);
        assert_eq!(parsed.pdf_font_map, None);
        assert_eq!(parsed.tex_arguments, vec![argument]);
    }

    #[cfg(unix)]
    #[test]
    fn 非utf8の結合map値もbyteのまま保つ() {
        use std::os::unix::ffi::OsStringExt;

        let map_name = OsString::from_vec(vec![b'm', 0xff, b'.', b'm', b'a', b'p']);
        let mut option = OsString::from("--pdf-font-map=");
        option.push(&map_name);
        let parsed = parse_arguments([OsString::from("--output-format=pdf"), option]).unwrap();
        assert_eq!(parsed.pdf_font_map, Some(map_name));
    }
}

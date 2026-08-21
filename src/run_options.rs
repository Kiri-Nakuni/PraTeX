//! TeXへ渡す入力行から、rtex自身の少数のoptionだけを分ける。

use std::ffi::OsString;
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
    pub(crate) tex_arguments: Vec<OsString>,
}

/// web2c系で一般的な `-output-format=pdf` と、そのlong-option表記を読む。
///
/// rtexが知らない引数は従来どおりTeXの最初の入力行へ残す。`--` より後ろもすべて
/// TeXへ渡すので、optionに似たファイル名や制御綴も失わない。
pub(crate) fn parse_arguments(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<ParsedArguments, RunOptionError> {
    let mut output_format = OutputFormat::Dvi;
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
        }
        tex_arguments.push(argument);
    }

    Ok(ParsedArguments {
        output_format,
        tex_arguments,
    })
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RunOptionError {
    MissingOutputFormat,
    UnknownOutputFormat(String),
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
        assert_eq!(
            parsed.tex_arguments,
            strings(&["--output-format=pdf", "hello.tex"])
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
        assert_eq!(parsed.tex_arguments, vec![argument]);
    }
}

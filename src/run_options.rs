//! TeXへ渡す入力行から、rtex自身の少数のoptionだけを分ける。

use crate::logger::InteractionMode;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImmediateAction {
    Help,
    Version,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParsedArguments {
    /// TeX engineを起動せず、CLI自身が答えて終了する指定。
    pub(crate) immediate_action: Option<ImmediateAction>,
    /// fmtに保存された値より後で適用する、run-scoped interaction指定。
    pub(crate) interaction: Option<InteractionMode>,
    /// Web2C互換の明示format selector。先頭の`&fmt`があればそちらが優先する。
    pub(crate) format_name: Option<OsString>,
    /// initial engineを明示的に選び、format dumpを可能にする。
    pub(crate) ini: bool,
    /// 最初の回復可能TeX errorでprocessを失敗終了させる。
    pub(crate) halt_on_error: bool,
    /// 明示値は空文字やpath separatorも含め、OS文字列のまま出力名へ使う。
    pub(crate) job_name: Option<OsString>,
    /// DVI preambleへ入れるbyte列。PDF modeでは受理するが使用しない。
    pub(crate) output_comment: Option<Vec<u8>>,
    /// PraTeXはshell escapeを実装していない。正方向のoptionはparserが拒否する。
    pub(crate) shell_escape_enabled: bool,
    /// PraTeX resolverはmktex生成を行わない。file typeごとの不変条件を明示する。
    pub(crate) mktex_tex_enabled: bool,
    pub(crate) mktex_tfm_enabled: bool,
    pub(crate) output_format: OutputFormat,
    /// Type 1 埋込みを明示的に有効にする map の論理名または物理 path。
    pub(crate) pdf_font_map: Option<OsString>,
    /// 非埋込み和文CID fontを一つのJFMへ結ぶ、明示物理profile path。
    pub(crate) pdf_japanese_cid_profile: Option<OsString>,
    /// TeX文書が明示した出力を残し、自動進捗だけを端末から隠す。
    pub(crate) quiet: bool,
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

fn parse_interaction(value: &OsStr) -> Result<InteractionMode, RunOptionError> {
    let Some(value) = value.to_str() else {
        return Err(RunOptionError::UnknownInteractionMode(
            value.to_string_lossy().into_owned(),
        ));
    };
    InteractionMode::from_cli_name(value)
        .ok_or_else(|| RunOptionError::UnknownInteractionMode(value.to_owned()))
}

fn disable_mktex(file_type: &OsStr) -> Result<(), RunOptionError> {
    match file_type.to_str() {
        Some("tex" | "tfm") => Ok(()),
        Some(other) => Err(RunOptionError::UnknownMktexFileType(other.to_owned())),
        None => Err(RunOptionError::UnknownMktexFileType(
            file_type.to_string_lossy().into_owned(),
        )),
    }
}

/// Web2C互換optionとPraTeX固有optionをTeXの最初の入力行から分ける。
///
/// `--` より前の未知のdash始まりは綴り違いを黙ってTeXへ流さず、明示errorにする。
/// `--` より後ろはすべてTeXへ渡すので、optionに似たファイル名や制御綴も失わない。
pub(crate) fn parse_arguments(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<ParsedArguments, RunOptionError> {
    let mut immediate_action = None;
    let mut interaction = None;
    let mut format_name = None;
    let mut ini = false;
    let mut halt_on_error = false;
    let mut job_name = None;
    let mut output_comment = None;
    let shell_escape_enabled = false;
    let mktex_tex_enabled = false;
    let mktex_tfm_enabled = false;
    let mut output_format = OutputFormat::Dvi;
    let mut pdf_font_map = None;
    let mut pdf_japanese_cid_profile = None;
    let mut quiet = false;
    let mut tex_arguments = Vec::new();
    let mut arguments = arguments.into_iter();
    let mut parsing_options = true;

    while let Some(argument) = arguments.next() {
        if parsing_options && argument == "--" {
            parsing_options = false;
            continue;
        }
        if parsing_options {
            if argument == "--help" || argument == "-help" {
                immediate_action = Some(ImmediateAction::Help);
                continue;
            }
            if argument == "--version" || argument == "-version" {
                immediate_action = Some(ImmediateAction::Version);
                continue;
            }
            if argument == "-ini" || argument == "--ini" {
                ini = true;
                continue;
            }
            if argument == "-halt-on-error" || argument == "--halt-on-error" {
                halt_on_error = true;
                continue;
            }
            if let Some(value) = strip_os_prefix(&argument, "-jobname=")
                .or_else(|| strip_os_prefix(&argument, "--jobname="))
            {
                job_name = Some(value);
                continue;
            }
            if argument == "-jobname" || argument == "--jobname" {
                job_name = Some(arguments.next().ok_or(RunOptionError::MissingJobName)?);
                continue;
            }
            if let Some(value) = strip_os_prefix(&argument, "-output-comment=")
                .or_else(|| strip_os_prefix(&argument, "--output-comment="))
            {
                output_comment = Some(crate::os_str_to_bytes(&value));
                continue;
            }
            if argument == "-output-comment" || argument == "--output-comment" {
                let value = arguments
                    .next()
                    .ok_or(RunOptionError::MissingOutputComment)?;
                output_comment = Some(crate::os_str_to_bytes(&value));
                continue;
            }
            if argument == "-no-shell-escape" || argument == "--no-shell-escape" {
                // Disabled is already the only available PraTeX capability state.
                continue;
            }
            if let Some(value) = strip_os_prefix(&argument, "-no-mktex=")
                .or_else(|| strip_os_prefix(&argument, "--no-mktex="))
            {
                if value.is_empty() {
                    return Err(RunOptionError::MissingMktexFileType);
                }
                disable_mktex(&value)?;
                continue;
            }
            if argument == "-no-mktex" || argument == "--no-mktex" {
                let value = arguments
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or(RunOptionError::MissingMktexFileType)?;
                disable_mktex(&value)?;
                continue;
            }
            if let Some(value) =
                strip_os_prefix(&argument, "-fmt=").or_else(|| strip_os_prefix(&argument, "--fmt="))
            {
                if value.is_empty() {
                    return Err(RunOptionError::MissingFormatName);
                }
                format_name = Some(value);
                continue;
            }
            if argument == "-fmt" || argument == "--fmt" {
                let value = arguments
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or(RunOptionError::MissingFormatName)?;
                format_name = Some(value);
                continue;
            }
            if let Some(value) = strip_os_prefix(&argument, "-interaction=")
                .or_else(|| strip_os_prefix(&argument, "--interaction="))
            {
                if value.is_empty() {
                    return Err(RunOptionError::MissingInteractionMode);
                }
                interaction = Some(parse_interaction(&value)?);
                continue;
            }
            if argument == "-interaction" || argument == "--interaction" {
                let value = arguments
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or(RunOptionError::MissingInteractionMode)?;
                interaction = Some(parse_interaction(&value)?);
                continue;
            }
            if argument == "--quiet" {
                quiet = true;
                continue;
            }
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
            if let Some(value) = strip_os_prefix(&argument, "--pdf-japanese-cid-profile=") {
                if value.is_empty() {
                    return Err(RunOptionError::MissingPdfJapaneseCidProfile);
                }
                pdf_japanese_cid_profile = Some(value);
                continue;
            }
            if argument == "--pdf-japanese-cid-profile" {
                let value = arguments
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or(RunOptionError::MissingPdfJapaneseCidProfile)?;
                pdf_japanese_cid_profile = Some(value);
                continue;
            }
            if strip_os_prefix(&argument, "-").is_some() {
                return Err(RunOptionError::UnknownOption(argument));
            }
        }
        tex_arguments.push(argument);
    }

    if pdf_font_map.is_some() && output_format != OutputFormat::Pdf {
        return Err(RunOptionError::PdfFontMapRequiresPdf);
    }
    if pdf_japanese_cid_profile.is_some() && output_format != OutputFormat::Pdf {
        return Err(RunOptionError::PdfJapaneseCidProfileRequiresPdf);
    }
    if output_format == OutputFormat::Dvi {
        if let Some(comment) = &output_comment {
            if comment.len() > u8::MAX as usize {
                return Err(RunOptionError::OutputCommentTooLong(comment.len()));
            }
        }
    }

    Ok(ParsedArguments {
        immediate_action,
        interaction,
        format_name,
        ini,
        halt_on_error,
        job_name,
        output_comment,
        shell_escape_enabled,
        mktex_tex_enabled,
        mktex_tfm_enabled,
        output_format,
        pdf_font_map,
        pdf_japanese_cid_profile,
        quiet,
        tex_arguments,
    })
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RunOptionError {
    MissingJobName,
    MissingOutputComment,
    OutputCommentTooLong(usize),
    MissingMktexFileType,
    UnknownMktexFileType(String),
    MissingFormatName,
    MissingInteractionMode,
    UnknownInteractionMode(String),
    UnknownOption(OsString),
    MissingOutputFormat,
    UnknownOutputFormat(String),
    MissingPdfFontMap,
    PdfFontMapRequiresPdf,
    MissingPdfJapaneseCidProfile,
    PdfJapaneseCidProfileRequiresPdf,
}

impl fmt::Display for RunOptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingJobName => formatter.write_str("missing value for -jobname"),
            Self::MissingOutputComment => {
                formatter.write_str("missing value for -output-comment")
            }
            Self::OutputCommentTooLong(length) => write!(
                formatter,
                "DVI output comment is {length} bytes (maximum is 255)"
            ),
            Self::MissingMktexFileType => formatter.write_str("missing value for -no-mktex"),
            Self::UnknownMktexFileType(file_type) => write!(
                formatter,
                "unknown -no-mktex file type `{file_type}` (expected tex or tfm)"
            ),
            Self::MissingFormatName => formatter.write_str("missing value for -fmt"),
            Self::MissingInteractionMode => {
                formatter.write_str("missing value for -interaction")
            }
            Self::UnknownInteractionMode(mode) => write!(
                formatter,
                "unknown interaction mode `{mode}` (expected batchmode, nonstopmode, scrollmode, or errorstopmode)"
            ),
            Self::UnknownOption(option) => write!(
                formatter,
                "unknown option `{}` (use `--` before TeX input that begins with `-`)",
                option.to_string_lossy()
            ),
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
            Self::MissingPdfJapaneseCidProfile => {
                formatter.write_str("missing value for --pdf-japanese-cid-profile")
            }
            Self::PdfJapaneseCidProfileRequiresPdf => {
                formatter.write_str("--pdf-japanese-cid-profile requires --output-format=pdf")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_arguments, ImmediateAction, OutputFormat, RunOptionError};
    use crate::logger::InteractionMode;
    use std::ffi::OsString;

    fn strings(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn 既定はdviで入力順を変えない() {
        let parsed = parse_arguments(strings(&["&plain", "hello.tex"])).unwrap();
        assert_eq!(parsed.output_format, OutputFormat::Dvi);
        assert_eq!(parsed.pdf_font_map, None);
        assert_eq!(parsed.pdf_japanese_cid_profile, None);
        assert_eq!(parsed.immediate_action, None);
        assert_eq!(parsed.interaction, None);
        assert_eq!(parsed.format_name, None);
        assert!(!parsed.ini);
        assert!(!parsed.halt_on_error);
        assert_eq!(parsed.job_name, None);
        assert_eq!(parsed.output_comment, None);
        assert!(!parsed.shell_escape_enabled);
        assert!(!parsed.mktex_tex_enabled);
        assert!(!parsed.mktex_tfm_enabled);
        assert!(!parsed.quiet);
        assert_eq!(parsed.tex_arguments, strings(&["&plain", "hello.tex"]));
    }

    #[test]
    fn helpとversionは一重二重dashのどちらでも選べる() {
        for (argument, expected) in [
            ("-help", ImmediateAction::Help),
            ("--help", ImmediateAction::Help),
            ("-version", ImmediateAction::Version),
            ("--version", ImmediateAction::Version),
        ] {
            let parsed = parse_arguments(strings(&[argument])).unwrap();
            assert_eq!(parsed.immediate_action, Some(expected));
            assert!(parsed.tex_arguments.is_empty());
        }
    }

    #[test]
    fn interactionの四modeを結合値と分離値で選べる() {
        for (name, expected) in [
            ("batchmode", InteractionMode::Batch),
            ("nonstopmode", InteractionMode::Nonstop),
            ("scrollmode", InteractionMode::Scroll),
            ("errorstopmode", InteractionMode::ErrorStop),
        ] {
            for arguments in [
                strings(&[&format!("-interaction={name}"), "hello.tex"]),
                strings(&["--interaction", name, "hello.tex"]),
            ] {
                let parsed = parse_arguments(arguments).unwrap();
                assert_eq!(parsed.interaction, Some(expected));
                assert_eq!(parsed.tex_arguments, strings(&["hello.tex"]));
            }
        }
    }

    #[test]
    fn fmtはos文字列を結合値と分離値で保つ() {
        for arguments in [
            strings(&["-fmt=形式/latex", "hello.tex"]),
            strings(&["--fmt", "形式/latex", "hello.tex"]),
        ] {
            let parsed = parse_arguments(arguments).unwrap();
            assert_eq!(parsed.format_name, Some(OsString::from("形式/latex")));
            assert_eq!(parsed.tex_arguments, strings(&["hello.tex"]));
        }
    }

    #[test]
    fn iniとhalt_on_errorをrun_policyとして選べる() {
        let parsed = parse_arguments(strings(&["-ini", "--halt-on-error", "hello.tex"])).unwrap();
        assert!(parsed.ini);
        assert!(parsed.halt_on_error);
        assert_eq!(parsed.tex_arguments, strings(&["hello.tex"]));
    }

    #[test]
    fn jobnameは空値とpathを含むos文字列を保つ() {
        for (arguments, expected) in [
            (strings(&["-jobname=dir/name", "hello.tex"]), "dir/name"),
            (strings(&["--jobname", "dot.name", "hello.tex"]), "dot.name"),
            (strings(&["-jobname=", "hello.tex"]), ""),
            (strings(&["--jobname", "", "hello.tex"]), ""),
        ] {
            let parsed = parse_arguments(arguments).unwrap();
            assert_eq!(parsed.job_name, Some(OsString::from(expected)));
            assert_eq!(parsed.tex_arguments, strings(&["hello.tex"]));
        }
    }

    #[test]
    fn output_commentは空から二百五十五byteまでdviへ保持する() {
        for arguments in [
            strings(&["-output-comment=CLI-COMMENT", "hello.tex"]),
            strings(&["--output-comment", "CLI-COMMENT", "hello.tex"]),
        ] {
            let parsed = parse_arguments(arguments).unwrap();
            assert_eq!(parsed.output_comment, Some(b"CLI-COMMENT".to_vec()));
        }

        let empty = parse_arguments(strings(&["-output-comment=", "hello.tex"])).unwrap();
        assert_eq!(empty.output_comment, Some(Vec::new()));

        let maximum = "A".repeat(255);
        let parsed = parse_arguments([OsString::from(format!(
            "-output-comment={maximum}"
        ))])
        .unwrap();
        assert_eq!(parsed.output_comment.unwrap().len(), 255);

        let overlong = "B".repeat(256);
        assert_eq!(
            parse_arguments([OsString::from(format!("-output-comment={overlong}"))]),
            Err(RunOptionError::OutputCommentTooLong(256))
        );
        let pdf = parse_arguments([
            OsString::from("-output-format=pdf"),
            OsString::from(format!("-output-comment={overlong}")),
        ])
        .unwrap();
        assert_eq!(pdf.output_comment.unwrap().len(), 256);
    }

    #[test]
    fn 外部実行を無効にする指定だけを受理する() {
        let parsed = parse_arguments(strings(&[
            "--no-shell-escape",
            "-no-mktex=tex",
            "--no-mktex",
            "tfm",
            "hello.tex",
        ]))
        .unwrap();
        assert!(!parsed.shell_escape_enabled);
        assert!(!parsed.mktex_tex_enabled);
        assert!(!parsed.mktex_tfm_enabled);
        assert_eq!(parsed.tex_arguments, strings(&["hello.tex"]));

        for positive in ["-shell-escape", "--shell-restricted", "-mktex=tex"] {
            assert_eq!(
                parse_arguments(strings(&[positive])),
                Err(RunOptionError::UnknownOption(OsString::from(positive)))
            );
        }
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
            assert_eq!(parsed.pdf_japanese_cid_profile, None);
            assert!(!parsed.quiet);
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
        assert!(!parsed.quiet);
        assert_eq!(
            parsed.tex_arguments,
            strings(&["--output-format=pdf", "hello.tex"])
        );
    }

    #[test]
    fn quietは重ねてもよく二重dash以後ならtexへ渡す() {
        let parsed = parse_arguments(strings(&[
            "--quiet",
            "--quiet",
            "--",
            "--quiet",
            "hello.tex",
        ]))
        .unwrap();
        assert!(parsed.quiet);
        assert_eq!(parsed.tex_arguments, strings(&["--quiet", "hello.tex"]));
    }

    #[test]
    fn 未知のdash始まりをtex入力へ黙って流さない() {
        for value in ["-quiet", "--quiet=true"] {
            assert_eq!(
                parse_arguments(strings(&[value])),
                Err(RunOptionError::UnknownOption(OsString::from(value)))
            );
        }

        let parsed = parse_arguments(strings(&["--", "-quiet", "hello.tex"])).unwrap();
        assert_eq!(parsed.tex_arguments, strings(&["-quiet", "hello.tex"]));
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
    fn 和文cid_profileは物理pathをos文字列のままpdfへだけ渡す() {
        for arguments in [
            strings(&[
                "--output-format=pdf",
                "--pdf-japanese-cid-profile=資材/min10.cidprofile",
                "hello.tex",
            ]),
            strings(&[
                "--pdf-japanese-cid-profile",
                "資材/min10.cidprofile",
                "--output-format",
                "pdf",
                "hello.tex",
            ]),
        ] {
            let parsed = parse_arguments(arguments).unwrap();
            assert_eq!(
                parsed.pdf_japanese_cid_profile,
                Some(OsString::from("資材/min10.cidprofile"))
            );
            assert_eq!(parsed.tex_arguments, strings(&["hello.tex"]));
        }

        assert_eq!(
            parse_arguments(strings(&["--pdf-japanese-cid-profile=min10.cidprofile"])),
            Err(RunOptionError::PdfJapaneseCidProfileRequiresPdf)
        );
        assert_eq!(
            parse_arguments(strings(&[
                "--output-format=pdf",
                "--pdf-japanese-cid-profile="
            ])),
            Err(RunOptionError::MissingPdfJapaneseCidProfile)
        );
        assert_eq!(
            parse_arguments(strings(&[
                "--output-format=pdf",
                "--pdf-japanese-cid-profile"
            ])),
            Err(RunOptionError::MissingPdfJapaneseCidProfile)
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
        assert_eq!(
            parse_arguments(strings(&["-interaction=dialogmode"])),
            Err(RunOptionError::UnknownInteractionMode(
                "dialogmode".to_owned()
            ))
        );
        assert_eq!(
            parse_arguments(strings(&["--interaction="])),
            Err(RunOptionError::MissingInteractionMode)
        );
        assert_eq!(
            parse_arguments(strings(&["-interaction"])),
            Err(RunOptionError::MissingInteractionMode)
        );
        assert_eq!(
            parse_arguments(strings(&["-fmt="])),
            Err(RunOptionError::MissingFormatName)
        );
        assert_eq!(
            parse_arguments(strings(&["--fmt"])),
            Err(RunOptionError::MissingFormatName)
        );
        assert_eq!(
            parse_arguments(strings(&["-jobname"])),
            Err(RunOptionError::MissingJobName)
        );
        assert_eq!(
            parse_arguments(strings(&["-output-comment"])),
            Err(RunOptionError::MissingOutputComment)
        );
        assert_eq!(
            parse_arguments(strings(&["-no-mktex="])),
            Err(RunOptionError::MissingMktexFileType)
        );
        assert_eq!(
            parse_arguments(strings(&["--no-mktex", "pk"])),
            Err(RunOptionError::UnknownMktexFileType("pk".to_owned()))
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
        assert!(!parsed.quiet);
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

use crate::print::Printer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertCommand {
    Number,
    RomanNumeral,
    String,
    Meaning,
    FontName,
    JobName,
    /// `\eTeXrevision` — e-TeX 2.0の公開revision文字列。
    ETeXRevision,
    /// `\pratexrevision` — 末尾の零も保つPraTeX自身の版文字列。
    PraTeXRevision,
    // ==== pdfTeX 由来。**組版に触らない道具** ====
    /// `\pdffilesize{名前}` — 大きさをバイト数で。**無ければ空**
    PdfFileSize,
    /// `\pdfmdfivesum{文字列}` — MD5 を十六進で
    PdfMdFiveSum,
    /// `\pdfstrcmp{文字列1}{文字列2}` — 辞書順を -1 / 0 / 1 で
    PdfStrCmp,
    /// `\pdfescapehex` / `\pdfunescapehex` / `\pdfescapestring` / `\pdfescapename`
    PdfEscapeHex,
    PdfUnescapeHex,
    PdfEscapeString,
    PdfEscapeName,
    /// `\pdfcreationdate` — `D:YYYYMMDDHHmmSS+00'00'`
    PdfCreationDate,
}

impl ConvertCommand {
    pub fn display(&self, printer: &mut impl Printer) {
        let s: &[u8] = match self {
            Self::Number => b"number",
            Self::RomanNumeral => b"romannumeral",
            Self::String => b"string",
            Self::Meaning => b"meaning",
            Self::FontName => b"fontname",
            Self::JobName => b"jobname",
            Self::ETeXRevision => b"eTeXrevision",
            Self::PraTeXRevision => b"pratexrevision",
            Self::PdfFileSize => b"pdffilesize",
            Self::PdfMdFiveSum => b"pdfmdfivesum",
            Self::PdfStrCmp => b"pdfstrcmp",
            Self::PdfEscapeHex => b"pdfescapehex",
            Self::PdfUnescapeHex => b"pdfunescapehex",
            Self::PdfEscapeString => b"pdfescapestring",
            Self::PdfEscapeName => b"pdfescapename",
            Self::PdfCreationDate => b"pdfcreationdate",
        };
        printer.print_esc_str(s);
    }
}

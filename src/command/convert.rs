use crate::print::Printer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertCommand {
    Number,
    RomanNumeral,
    String,
    Meaning,
    FontName,
    JobName,
    // ==== pdfTeX 由来。**組版に触らない道具** ====
    /// `\pdffilesize{名前}` — 大きさをバイト数で。**無ければ空**
    PdfFileSize,
    /// `\pdfmdfivesum{文字列}` — MD5 を十六進で
    PdfMdFiveSum,
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
            Self::PdfFileSize => b"pdffilesize",
            Self::PdfMdFiveSum => b"pdfmdfivesum",
            Self::PdfEscapeHex => b"pdfescapehex",
            Self::PdfUnescapeHex => b"pdfunescapehex",
            Self::PdfEscapeString => b"pdfescapestring",
            Self::PdfEscapeName => b"pdfescapename",
            Self::PdfCreationDate => b"pdfcreationdate",
        };
        printer.print_esc_str(s);
    }
}

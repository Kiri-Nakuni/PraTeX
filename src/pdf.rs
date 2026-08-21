//! PDF 1.4 の低水準 serializer。
//!
//! Adobe の *PDF Reference, Third Edition, version 1.4* の 3.2 節（object）、
//! 3.3 節（stream）、3.4 節（file structure）だけを境界として実装する。
//! <https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf>
//! ページ組版や DVI 命令の変換はここへ持ち込まず、間接 object と従来型 xref を
//! 正しい byte offset で並べることだけを受け持つ。

use std::fmt;
use std::io::{self, Write};

const PDF_1_4_HEADER: &[u8] = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n";

// PDF 1.4 の従来型 xref entry で byte offset に割り当てられるのは 10 桁である。
const MAX_XREF_OFFSET: u64 = 9_999_999_999;

type PdfWriterResult<T> = Result<T, PdfWriterError>;

/// 一つの PDF writer の中で予約した間接 object の番号。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PdfObjectId(u32);

impl PdfObjectId {
    /// dictionary や array で `n 0 R` を組み立てるときに使う object 番号。
    pub(crate) fn number(self) -> u32 {
        self.0
    }
}

/// Seek を使わず、書いた byte 数を xref のために数える。
struct CountingWriter<W> {
    inner: W,
    position: u64,
}

impl<W> CountingWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner, position: 0 }
    }

    fn position(&self) -> u64 {
        self.position
    }

    fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.position = self
            .position
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::other("PDF byte position overflow"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// 間接 object、stream、xref、trailer を順次書く PDF 1.4 writer。
///
/// Object は先に予約できるので、相互参照を組み立ててから任意の順で書ける。
/// `finish` は予約した全 object が一度ずつ書かれたことを確かめる。
pub(crate) struct PdfWriter<W: Write> {
    target: CountingWriter<W>,
    object_offsets: Vec<Option<u64>>,
}

impl<W: Write> PdfWriter<W> {
    pub(crate) fn new(target: W) -> PdfWriterResult<Self> {
        let mut writer = Self {
            target: CountingWriter::new(target),
            object_offsets: Vec::new(),
        };
        writer.write_all(PDF_1_4_HEADER)?;
        Ok(writer)
    }

    /// Object 番号を確保する。書き込み順は予約順と同じでなくてもよい。
    pub(crate) fn reserve_object(&mut self) -> PdfWriterResult<PdfObjectId> {
        let next = self
            .object_offsets
            .len()
            .checked_add(1)
            .and_then(|number| u32::try_from(number).ok())
            .filter(|&number| number < u32::MAX)
            .ok_or(PdfWriterError::TooManyObjects)?;
        self.object_offsets.push(None);
        Ok(PdfObjectId(next))
    }

    /// 呼び出し側で組み立てた PDF object 本体を間接 object として書く。
    pub(crate) fn write_object(&mut self, object: PdfObjectId, body: &[u8]) -> PdfWriterResult<()> {
        let slot = self.prepare_object(object)?;
        let offset = self.target.position();
        self.write_all(format!("{} 0 obj\n", object.number()).as_bytes())?;
        self.write_all(body)?;
        self.write_all(b"\nendobj\n")?;
        self.object_offsets[slot] = Some(offset);
        Ok(())
    }

    /// Stream dictionary に正しい `/Length` を加え、data を一度だけ書く。
    ///
    /// `dictionary_entries` は外側の `<<` `>>` と `/Length` を含めない raw PDF
    /// dictionary entry である。Stream data は途中の NUL や改行もそのまま保つ。
    pub(crate) fn write_stream(
        &mut self,
        object: PdfObjectId,
        dictionary_entries: &[u8],
        data: &[u8],
    ) -> PdfWriterResult<()> {
        let slot = self.prepare_object(object)?;
        let offset = self.target.position();
        self.write_all(format!("{} 0 obj\n", object.number()).as_bytes())?;
        self.write_all(b"<<\n")?;
        if !dictionary_entries.is_empty() {
            self.write_all(dictionary_entries)?;
            if !dictionary_entries
                .last()
                .is_some_and(|byte| byte.is_ascii_whitespace())
            {
                self.write_all(b"\n")?;
            }
        }
        self.write_all(format!("/Length {}\n", data.len()).as_bytes())?;
        self.write_all(b">>\nstream\n")?;
        self.write_all(data)?;
        self.write_all(b"\nendstream\nendobj\n")?;
        self.object_offsets[slot] = Some(offset);
        Ok(())
    }

    /// 従来型 xref table と trailer を書いて target を返す。
    pub(crate) fn finish(mut self, root: PdfObjectId) -> PdfWriterResult<W> {
        let root_slot = self.object_slot(root)?;
        if self.object_offsets[root_slot].is_none() {
            return Err(PdfWriterError::UnwrittenObject(root));
        }
        if let Some((slot, _)) = self
            .object_offsets
            .iter()
            .enumerate()
            .find(|(_, offset)| offset.is_none())
        {
            return Err(PdfWriterError::UnwrittenObject(PdfObjectId(
                u32::try_from(slot + 1).expect("reserved object numbers fit in u32"),
            )));
        }

        let xref_offset = self.target.position();
        self.write_all(format!("xref\n0 {}\n", self.object_offsets.len() + 1).as_bytes())?;

        // PDF 1.4, 3.4.3: 各 entry は CRLF を含むちょうど 20 byte。
        self.write_all(b"0000000000 65535 f\r\n")?;
        for slot in 0..self.object_offsets.len() {
            let offset = self.object_offsets[slot].expect("all objects checked above");
            self.write_all(format!("{offset:010} 00000 n\r\n").as_bytes())?;
        }

        self.write_all(
            format!(
                "trailer\n<<\n/Size {}\n/Root {} 0 R\n>>\nstartxref\n{xref_offset}\n%%EOF\n",
                self.object_offsets.len() + 1,
                root.number()
            )
            .as_bytes(),
        )?;
        self.target.flush()?;
        Ok(self.target.into_inner())
    }

    fn prepare_object(&self, object: PdfObjectId) -> PdfWriterResult<usize> {
        let slot = self.object_slot(object)?;
        if self.object_offsets[slot].is_some() {
            return Err(PdfWriterError::ObjectAlreadyWritten(object));
        }
        if self.target.position() > MAX_XREF_OFFSET {
            return Err(PdfWriterError::ObjectOffsetTooLarge(self.target.position()));
        }
        Ok(slot)
    }

    fn object_slot(&self, object: PdfObjectId) -> PdfWriterResult<usize> {
        let slot = usize::try_from(object.number())
            .ok()
            .and_then(|number| number.checked_sub(1))
            .filter(|&slot| slot < self.object_offsets.len())
            .ok_or(PdfWriterError::UnknownObject(object))?;
        Ok(slot)
    }

    fn write_all(&mut self, bytes: &[u8]) -> PdfWriterResult<()> {
        self.target.write_all(bytes)?;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) enum PdfWriterError {
    Io(io::Error),
    TooManyObjects,
    UnknownObject(PdfObjectId),
    ObjectAlreadyWritten(PdfObjectId),
    UnwrittenObject(PdfObjectId),
    ObjectOffsetTooLarge(u64),
}

impl fmt::Display for PdfWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "PDF output failed: {error}"),
            Self::TooManyObjects => write!(formatter, "too many PDF objects"),
            Self::UnknownObject(object) => {
                write!(formatter, "unknown PDF object {}", object.number())
            }
            Self::ObjectAlreadyWritten(object) => {
                write!(
                    formatter,
                    "PDF object {} was written twice",
                    object.number()
                )
            }
            Self::UnwrittenObject(object) => {
                write!(formatter, "PDF object {} was not written", object.number())
            }
            Self::ObjectOffsetTooLarge(offset) => {
                write!(formatter, "PDF object offset {offset} does not fit in xref")
            }
        }
    }
}

impl std::error::Error for PdfWriterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for PdfWriterError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{PdfWriter, PdfWriterError};

    fn byte位置(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("needle must occur")
    }

    #[test]
    fn xrefは書き込み順でなくobject番号を指す() {
        let mut writer = PdfWriter::new(Vec::new()).unwrap();
        let catalog = writer.reserve_object().unwrap();
        let pages = writer.reserve_object().unwrap();

        writer
            .write_object(pages, b"<< /Type /Pages /Kids [] /Count 0 >>")
            .unwrap();
        writer
            .write_object(
                catalog,
                format!("<< /Type /Catalog /Pages {} 0 R >>", pages.number()).as_bytes(),
            )
            .unwrap();
        let pdf = writer.finish(catalog).unwrap();

        assert!(pdf.starts_with(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n"));
        let catalog_offset = byte位置(&pdf, b"1 0 obj\n");
        let pages_offset = byte位置(&pdf, b"2 0 obj\n");
        let xref_offset = byte位置(&pdf, b"xref\n");
        let expected_xref = format!(
            "xref\n0 3\n0000000000 65535 f\r\n{catalog_offset:010} 00000 n\r\n{pages_offset:010} 00000 n\r\n"
        );
        assert_eq!(
            &pdf[xref_offset..xref_offset + expected_xref.len()],
            expected_xref.as_bytes()
        );
        assert!(pdf.ends_with(
            format!("trailer\n<<\n/Size 3\n/Root 1 0 R\n>>\nstartxref\n{xref_offset}\n%%EOF\n")
                .as_bytes()
        ));
    }

    #[test]
    fn streamの長さはdataだけを数える() {
        let mut writer = PdfWriter::new(Vec::new()).unwrap();
        let stream = writer.reserve_object().unwrap();
        let data = b"q\n\0Q";
        writer
            .write_stream(stream, b"/Filter /ASCIIHexDecode", data)
            .unwrap();
        let pdf = writer.finish(stream).unwrap();

        let body = b"1 0 obj\n<<\n/Filter /ASCIIHexDecode\n/Length 4\n>>\nstream\nq\n\0Q\nendstream\nendobj\n";
        assert!(pdf.windows(body.len()).any(|window| window == body));
    }

    #[test]
    fn 予約したobjectの書き忘れをtrailerより前に拒む() {
        let mut writer = PdfWriter::new(Vec::new()).unwrap();
        let catalog = writer.reserve_object().unwrap();
        let missing = writer.reserve_object().unwrap();
        writer
            .write_object(catalog, b"<< /Type /Catalog >>")
            .unwrap();

        match writer.finish(catalog) {
            Err(PdfWriterError::UnwrittenObject(object)) => assert_eq!(object, missing),
            _ => panic!("unwritten object must be rejected"),
        }
    }

    #[test]
    fn 同じobjectを二度書かない() {
        let mut writer = PdfWriter::new(Vec::new()).unwrap();
        let object = writer.reserve_object().unwrap();
        writer.write_object(object, b"null").unwrap();

        match writer.write_object(object, b"null") {
            Err(PdfWriterError::ObjectAlreadyWritten(found)) => assert_eq!(found, object),
            _ => panic!("duplicate object must be rejected"),
        }
    }
}

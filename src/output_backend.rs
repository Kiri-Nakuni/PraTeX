use crate::dvi::{DviWriter, DviWriterError};
use crate::scaled::Scaled;

use std::fmt::Debug;
use std::io::Write;

/// TeX が読み込んだ一つの論理fontをbackendへ渡す借用view。
///
/// `area` / `name` は DVI や map lookup に使う論理名であり、TFM resolver が
/// 開いた物理pathへ置き換えない。PDF側が文字幅objectを作れるよう、TFMの文字範囲と
/// `char_exists` が真になるcode集合も同じ境界で渡す。
#[derive(Clone, Copy, Debug)]
pub(crate) struct OutputFontDefinition<'a> {
    pub(crate) font_number: u32,
    pub(crate) checksum: u32,
    pub(crate) at_size: Scaled,
    pub(crate) design_size: Scaled,
    pub(crate) area: &'a [u8],
    pub(crate) name: &'a [u8],
    pub(crate) first_char: u8,
    pub(crate) last_char: u8,
    /// TFM の `char_exists` が真になる8-bit code。一時sliceは `define_font` 内だけで使う。
    pub(crate) existing_codes: &'a [u8],
}

/// 版面を一度だけ走査して、出力形式ごとの命令へ渡す境界。
///
/// 文字幅は DVI の命令には含まれないが、文字配置を自分で組み立てる出力形式も
/// 同じ走査結果を使えるように、文字コードと一緒に渡す。
pub(crate) trait ShipoutBackend {
    type Error: Debug;

    fn start_page(
        &mut self,
        counts: &[i32; 10],
        page_height: Scaled,
        page_width: Scaled,
    ) -> Result<(), Self::Error>;
    fn end_page(&mut self) -> Result<(), Self::Error>;
    fn push(&mut self) -> Result<(), Self::Error>;
    fn pop(&mut self) -> Result<(), Self::Error>;
    fn move_right(&mut self, amount: Scaled) -> Result<(), Self::Error>;
    fn move_down(&mut self, amount: Scaled) -> Result<(), Self::Error>;
    fn define_font(&mut self, font: OutputFontDefinition<'_>) -> Result<(), Self::Error>;
    fn set_font(&mut self, font_number: u32) -> Result<(), Self::Error>;
    fn set_char(&mut self, character: u8, width: Scaled) -> Result<(), Self::Error>;
    fn set_rule(&mut self, height: Scaled, width: Scaled) -> Result<(), Self::Error>;
    fn put_rule(&mut self, height: Scaled, width: Scaled) -> Result<(), Self::Error>;
    fn write_special(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
    fn page_count(&self) -> usize;
    fn finish(self) -> Result<usize, Self::Error>
    where
        Self: Sized;
}

/// 現行 DVI writer を shipout 境界へ接続するアダプタ。
pub(crate) struct DviBackend<T: Write> {
    writer: DviWriter<T>,
}

impl<T: Write> DviBackend<T> {
    pub(crate) fn new(target: T, mag: Scaled, comment: &[u8]) -> Result<Self, DviWriterError> {
        Ok(Self {
            writer: DviWriter::new(target, mag, comment)?,
        })
    }
}

impl<T: Write> ShipoutBackend for DviBackend<T> {
    type Error = DviWriterError;

    fn start_page(
        &mut self,
        counts: &[i32; 10],
        page_height: Scaled,
        page_width: Scaled,
    ) -> Result<(), Self::Error> {
        self.writer.start_page(counts, page_height, page_width)
    }

    fn end_page(&mut self) -> Result<(), Self::Error> {
        self.writer.end_page()
    }

    fn push(&mut self) -> Result<(), Self::Error> {
        self.writer.dvi_push()
    }

    fn pop(&mut self) -> Result<(), Self::Error> {
        self.writer.dvi_pop()
    }

    fn move_right(&mut self, amount: Scaled) -> Result<(), Self::Error> {
        self.writer.right(amount)
    }

    fn move_down(&mut self, amount: Scaled) -> Result<(), Self::Error> {
        self.writer.down(amount)
    }

    fn define_font(&mut self, font: OutputFontDefinition<'_>) -> Result<(), Self::Error> {
        self.writer.dvi_font_def(
            font.font_number,
            font.checksum,
            font.at_size,
            font.design_size,
            font.area,
            font.name,
        )
    }

    fn set_font(&mut self, font_number: u32) -> Result<(), Self::Error> {
        self.writer.set_font(font_number)
    }

    fn set_char(&mut self, character: u8, _width: Scaled) -> Result<(), Self::Error> {
        self.writer.set_char(character)
    }

    fn set_rule(&mut self, height: Scaled, width: Scaled) -> Result<(), Self::Error> {
        self.writer.set_rule(height, width)
    }

    fn put_rule(&mut self, height: Scaled, width: Scaled) -> Result<(), Self::Error> {
        self.writer.put_rule(height, width)
    }

    fn write_special(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.writer.write_special(bytes)
    }

    fn page_count(&self) -> usize {
        self.writer.get_total_pages()
    }

    fn finish(self) -> Result<usize, Self::Error> {
        self.writer.write_postamble()
    }
}

#[cfg(test)]
mod tests {
    use super::{DviBackend, OutputFontDefinition, ShipoutBackend};
    use crate::dvi::DviWriter;

    use std::cell::RefCell;
    use std::io::{self, Write};
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct SharedWriter(Rc<RefCell<Vec<u8>>>);

    impl SharedWriter {
        fn bytes(&self) -> Vec<u8> {
            self.0.borrow().clone()
        }
    }

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn dviアダプタは従来と同じバイト列を書く() {
        let counts = [1, -2, 3, 0, 0, 0, 0, 0, 0, 4];
        let direct_target = SharedWriter::default();
        let mut direct = DviWriter::new(direct_target.clone(), 1000, b"backend test").unwrap();
        direct.start_page(&counts, 20_000, 30_000).unwrap();
        direct.dvi_push().unwrap();
        direct.right(123).unwrap();
        direct.down(-456).unwrap();
        direct
            .dvi_font_def(0, 0x1234_5678, 655_360, 655_360, b"", b"cmr10")
            .unwrap();
        direct.set_font(0).unwrap();
        direct.set_char(b'A').unwrap();
        direct.set_rule(20, 30).unwrap();
        direct.put_rule(7, 11).unwrap();
        direct.write_special(b"rtex").unwrap();
        direct.dvi_pop().unwrap();
        direct.end_page().unwrap();
        let direct_size = direct.write_postamble().unwrap();

        let adapted_target = SharedWriter::default();
        let mut adapted = DviBackend::new(adapted_target.clone(), 1000, b"backend test").unwrap();
        adapted.start_page(&counts, 20_000, 30_000).unwrap();
        adapted.push().unwrap();
        adapted.move_right(123).unwrap();
        adapted.move_down(-456).unwrap();
        adapted
            .define_font(OutputFontDefinition {
                font_number: 0,
                checksum: 0x1234_5678,
                at_size: 655_360,
                design_size: 655_360,
                area: b"",
                name: b"cmr10",
                first_char: 0,
                last_char: 127,
                existing_codes: &[b'A'],
            })
            .unwrap();
        adapted.set_font(0).unwrap();
        adapted.set_char(b'A', 321).unwrap();
        adapted.set_rule(20, 30).unwrap();
        adapted.put_rule(7, 11).unwrap();
        adapted.write_special(b"rtex").unwrap();
        adapted.pop().unwrap();
        adapted.end_page().unwrap();
        assert_eq!(adapted.page_count(), 1);
        let adapted_size = adapted.finish().unwrap();

        assert_eq!(adapted_size, direct_size);
        assert_eq!(adapted_target.bytes(), direct_target.bytes());
    }
}

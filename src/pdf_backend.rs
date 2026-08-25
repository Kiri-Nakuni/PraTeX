use super::output_backend::{OutputFontDefinition, OutputFontKind, ShipoutBackend};
use crate::font_resources::loader::{FontResourceError, Type1ResourceLoader};
use crate::font_resources::named_cid::{
    NamedCidEncoding, NamedCidFontProfileLoader, NamedCidProfileError,
};
use crate::pdf_cid_font::{prepare_named_cid_font, PdfNamedCidFont};
use crate::pdf_document::{
    PdfCoordinate, PdfCourierFont, PdfDocument, PdfDocumentError, PdfPage, PdfPageFont,
};
use crate::pdf_font::{
    prepare_type1_font, MissingStemVPolicy, PdfType1Font, PdfType1FontRequest, PreparedPdfType1Font,
};
use crate::pdf_special::{parse_pdf_special, PdfPaperSize, PdfSpecialError};
use crate::scaled::Scaled;

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};

/// PDFへ実際に渡ったbyte数を、seekせずに数えるsink。
struct CountingSink<W> {
    inner: W,
    byte_count: u64,
}

impl<W> CountingSink<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            byte_count: 0,
        }
    }

    fn into_parts(self) -> (W, u64) {
        (self.inner, self.byte_count)
    }
}

impl<W: Write> Write for CountingSink<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.byte_count = self
            .byte_count
            .checked_add(
                u64::try_from(written)
                    .map_err(|_| io::Error::other("PDF output byte count does not fit in u64"))?,
            )
            .ok_or_else(|| io::Error::other("PDF output byte count overflow"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[derive(Clone, Copy)]
struct Position {
    horizontal: Scaled,
    vertical: Scaled,
}

struct PageState {
    /// このpageのcontent座標を作り始めた時点の媒体高。
    /// 第一pageの途中でpapersize specialを読む場合も、既出contentと基底を混ぜない。
    coordinate_media_height: PdfCoordinate,
    paper_size: Option<PdfPaperSize>,
    declared_height: Scaled,
    min_horizontal: Scaled,
    max_horizontal: Scaled,
    min_vertical: Scaled,
    max_vertical: Scaled,
    position: Position,
    position_stack: Vec<Position>,
    /// First-use order. PdfDocument assigns these `/F2`, `/F3`, ... after Courier `/F1`.
    fonts: Vec<PdfPageFont>,
    content: Vec<u8>,
}

#[derive(Clone, Copy)]
enum PdfFontState {
    Byte {
        at_size: Scaled,
        type1: Option<PdfType1Font>,
    },
    Japanese {
        at_size: Scaled,
        named_cid: PdfNamedCidFont,
        encoding: NamedCidEncoding,
    },
}

/// PDF font objectを共有してよい、sizeに依存しないTFM identity。
///
/// `at_size`はcontent streamの`Tf`だけを変え、FontFile、FontDescriptor、Widthsは
/// 変えない。同じ論理TFMでもchecksum、design size、存在code集合が異なる定義は
/// 壊れた入力を共有しないよう別identityにする。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Type1FontIdentity {
    logical_name: String,
    checksum: u32,
    design_size: Scaled,
    existing_codes: [u64; 4],
}

impl Type1FontIdentity {
    fn new(logical_name: &str, font: OutputFontDefinition<'_>) -> Self {
        let mut existing_codes = [0; 4];
        for &code in font.existing_codes {
            let code = usize::from(code);
            existing_codes[code / 64] |= 1_u64 << (code % 64);
        }
        Self {
            logical_name: logical_name.to_owned(),
            checksum: font.checksum,
            design_size: font.design_size,
            existing_codes,
        }
    }
}

/// Shipoutの表示命令を、既定ではStandard 14 Courierだけで最小PDFへ写すbackend。
/// Type 1 loaderを明示注入した場合だけ、font定義時に埋込みobjectを作る。
pub(crate) struct PdfBackend<W: Write> {
    document: PdfDocument<CountingSink<W>>,
    courier_font: PdfCourierFont,
    magnification: Scaled,
    type1_loader: Option<Box<dyn Type1ResourceLoader>>,
    named_cid_loader: Option<Box<dyn NamedCidFontProfileLoader>>,
    /// 同じTFMを異なるsizeで選んでも、埋込みobject群は文書内で一組だけ作る。
    type1_fonts: HashMap<Type1FontIdentity, PdfType1Font>,
    fonts: HashMap<u32, PdfFontState>,
    current_font: Option<u32>,
    page: Option<PageState>,
    /// 第一pageで確定した物理媒体寸法。後続pageはspecialがなくても継承する。
    paper_size: Option<PdfPaperSize>,
}

impl<W: Write> PdfBackend<W> {
    pub(crate) fn new(target: W, magnification: Scaled) -> Result<Self, PdfBackendError> {
        Self::with_loaders(target, magnification, None, None)
    }

    /// Type 1 map/resource 解決を明示的に有効にする constructor。
    ///
    /// loader は font 定義時だけ呼ばれ、文字nodeを描く hot loopでは型付きhandleだけを使う。
    pub(crate) fn with_type1_loader<L>(
        target: W,
        magnification: Scaled,
        loader: L,
    ) -> Result<Self, PdfBackendError>
    where
        L: Type1ResourceLoader + 'static,
    {
        Self::with_loaders(target, magnification, Some(Box::new(loader)), None)
    }

    /// Named CID profileを明示注入するconstructor。loaderはfont定義時だけ呼ぶ。
    pub(crate) fn with_named_cid_loader<L>(
        target: W,
        magnification: Scaled,
        loader: L,
    ) -> Result<Self, PdfBackendError>
    where
        L: NamedCidFontProfileLoader + 'static,
    {
        Self::with_loaders(target, magnification, None, Some(Box::new(loader)))
    }

    /// CLI/hostがType 1とnamed CIDの独立loaderを同時に注入する境界。
    pub(crate) fn with_loaders(
        target: W,
        magnification: Scaled,
        type1_loader: Option<Box<dyn Type1ResourceLoader>>,
        named_cid_loader: Option<Box<dyn NamedCidFontProfileLoader>>,
    ) -> Result<Self, PdfBackendError> {
        if magnification <= 0 {
            return Err(PdfBackendError::InvalidMagnification(magnification));
        }
        let mut document = PdfDocument::new(CountingSink::new(target))?;
        let courier_font = document.add_standard_courier_font()?;
        Ok(Self {
            document,
            courier_font,
            magnification,
            type1_loader,
            named_cid_loader,
            type1_fonts: HashMap::new(),
            fonts: HashMap::new(),
            current_font: None,
            page: None,
            paper_size: None,
        })
    }

    /// PDFとbyte数に加えてsink自体を返す。Vec sinkの試験と将来の埋込み用。
    pub(crate) fn finish_with_target(self) -> Result<(W, usize), PdfBackendError> {
        if self.page.is_some() {
            return Err(PdfBackendError::PageStillOpen);
        }
        let sink = self.document.finish()?;
        let (target, byte_count) = sink.into_parts();
        let byte_count =
            usize::try_from(byte_count).map_err(|_| PdfBackendError::OutputTooLarge(byte_count))?;
        Ok((target, byte_count))
    }

    fn current_page_mut(&mut self) -> Result<&mut PageState, PdfBackendError> {
        self.page.as_mut().ok_or(PdfBackendError::NoOpenPage)
    }

    fn load_type1_font(
        &mut self,
        logical_name: &str,
        font: OutputFontDefinition<'_>,
    ) -> Result<PdfType1Font, PdfBackendError> {
        let identity = Type1FontIdentity::new(logical_name, font);
        if let Some(&cached) = self.type1_fonts.get(&identity) {
            return Ok(cached);
        }

        let loader = self
            .type1_loader
            .as_mut()
            .expect("Type 1 loader presence was checked before loading");
        let loaded = loader.load(logical_name)?;
        let missing_stem_v = loaded
            .private_std_vw
            .map_or(MissingStemVPolicy::Reject, MissingStemVPolicy::Use);
        let prepared = prepare_type1_font(PdfType1FontRequest {
            program: loaded.font_program.value(),
            afm: loaded.metrics.value(),
            encoding: loaded.encoding.as_ref().map(|encoding| encoding.value()),
            embedding: loaded.embedding,
            descriptor_flags: loaded.descriptor_flags,
            missing_stem_v,
            used_codes: font.existing_codes,
        })
        .map_err(PdfDocumentError::from)?;
        let embedded = self.document.add_type1_font(prepared)?;
        self.type1_fonts.insert(identity, embedded);
        Ok(embedded)
    }

    /// 既に検査した Type 1 objectをTeX font定義へ直接結ぶ低水準API。
    /// productionのloader経路は`define_font`内で同じ型付きhandleを作る。
    pub(crate) fn attach_type1_font(
        &mut self,
        font_number: u32,
        prepared: PreparedPdfType1Font<'_>,
    ) -> Result<(), PdfBackendError> {
        let state = self
            .fonts
            .get(&font_number)
            .ok_or(PdfBackendError::UndefinedFont(font_number))?;
        let PdfFontState::Byte { type1, .. } = state else {
            return Err(PdfBackendError::FontKindMismatch {
                font_number,
                operation: PdfGlyphOperation::AttachType1,
                actual: OutputFontKind::Japanese,
            });
        };
        if type1.is_some() {
            return Err(PdfBackendError::Type1FontAlreadyAttached(font_number));
        }
        let handle = self.document.add_type1_font(prepared)?;
        let state = self
            .fonts
            .get_mut(&font_number)
            .ok_or(PdfBackendError::UndefinedFont(font_number))?;
        let PdfFontState::Byte { type1, .. } = state else {
            unreachable!("font kind was checked above");
        };
        *type1 = Some(handle);
        Ok(())
    }

    fn checked_move(value: Scaled, amount: Scaled) -> Result<Scaled, PdfBackendError> {
        value
            .checked_add(amount)
            .ok_or(PdfBackendError::PositionOverflow)
    }

    /// papersize specialの値は物理寸法なので、文書の`\mag`とは独立にPDF座標へする。
    fn physical_paper_coordinates(
        paper_size: PdfPaperSize,
    ) -> Result<(PdfCoordinate, PdfCoordinate), PdfBackendError> {
        let (width_numerator, width_denominator) = paper_size.width.sp_ratio();
        let (height_numerator, height_denominator) = paper_size.height.sp_ratio();
        let width = PdfCoordinate::from_scaled_ratio(width_numerator, width_denominator, 1000)?;
        let height = PdfCoordinate::from_scaled_ratio(height_numerator, height_denominator, 1000)?;
        if !width.is_positive() || !height.is_positive() {
            return Err(PdfBackendError::InvalidPageSize { width, height });
        }
        Ok((width, height))
    }

    fn include_interval(minimum: &mut Scaled, maximum: &mut Scaled, first: Scaled, second: Scaled) {
        *minimum = (*minimum).min(first).min(second);
        *maximum = (*maximum).max(first).max(second);
    }

    fn page_position(
        page: &PageState,
        magnification: Scaled,
    ) -> Result<(PdfCoordinate, PdfCoordinate), PdfBackendError> {
        let horizontal = PdfCoordinate::from_scaled(page.position.horizontal, magnification)?;
        let vertical = PdfCoordinate::from_scaled(page.position.vertical, magnification)?;
        let x = PdfCoordinate::ONE_INCH.checked_add(horizontal)?;
        let y = page
            .coordinate_media_height
            .checked_sub(PdfCoordinate::ONE_INCH)?
            .checked_sub(vertical)?;
        Ok((x, y))
    }

    fn append_rule(
        page: &mut PageState,
        magnification: Scaled,
        height: Scaled,
        width: Scaled,
    ) -> Result<(), PdfBackendError> {
        let (x, y) = Self::page_position(page, magnification)?;
        let height = PdfCoordinate::from_scaled(height, magnification)?;
        let width = PdfCoordinate::from_scaled(width, magnification)?;
        page.content
            .extend_from_slice(format!("{x} {y} {width} {height} re f\n").as_bytes());
        Ok(())
    }

    fn append_courier_character(
        page: &mut PageState,
        magnification: Scaled,
        character: u8,
        at_size: Scaled,
    ) -> Result<(), PdfBackendError> {
        let (x, y) = Self::page_position(page, magnification)?;
        let font_size = PdfCoordinate::from_scaled(at_size, magnification)?;
        page.content
            .extend_from_slice(format!("BT\n/F1 {font_size} Tf\n1 0 0 1 {x} {y} Tm\n(").as_bytes());
        if matches!(character, b'(' | b')' | b'\\') {
            page.content.push(b'\\');
        }
        page.content.push(character);
        page.content.extend_from_slice(b") Tj\nET\n");
        Ok(())
    }

    fn append_type1_character(
        page: &mut PageState,
        magnification: Scaled,
        character: u8,
        at_size: Scaled,
        resource_number: usize,
    ) -> Result<(), PdfBackendError> {
        let (x, y) = Self::page_position(page, magnification)?;
        let font_size = PdfCoordinate::from_scaled(at_size, magnification)?;
        page.content.extend_from_slice(
            format!(
                "BT\n/F{resource_number} {font_size} Tf\n1 0 0 1 {x} {y} Tm\n<{character:02X}> Tj\nET\n"
            )
            .as_bytes(),
        );
        Ok(())
    }

    fn named_cid_character_content(
        page: &PageState,
        magnification: Scaled,
        source_code: [u8; 2],
        at_size: Scaled,
        resource_number: usize,
    ) -> Result<Vec<u8>, PdfBackendError> {
        let (x, y) = Self::page_position(page, magnification)?;
        let font_size = PdfCoordinate::from_scaled(at_size, magnification)?;
        Ok(format!(
            "BT\n/F{resource_number} {font_size} Tf\n1 0 0 1 {x} {y} Tm\n<{:02X}{:02X}> Tj\nET\n",
            source_code[0], source_code[1],
        )
        .into_bytes())
    }
}

fn logical_font_name(font_number: u32, name: &[u8]) -> Result<&str, PdfBackendError> {
    std::str::from_utf8(name).map_err(|_| PdfBackendError::InvalidLogicalFontName {
        font_number,
        name: name.to_vec(),
    })
}

impl<W: Write> ShipoutBackend for PdfBackend<W> {
    type Error = PdfBackendError;

    fn start_page(
        &mut self,
        _counts: &[i32; 10],
        page_height: Scaled,
        page_width: Scaled,
    ) -> Result<(), Self::Error> {
        if self.page.is_some() {
            return Err(PdfBackendError::PageAlreadyOpen);
        }
        let double_margin = PdfCoordinate::ONE_INCH.checked_add(PdfCoordinate::ONE_INCH)?;
        // TeXは負幅・負高のboxも作れる。物理媒体まで負にせず、内容範囲を空として扱う。
        let declared_width = page_width.max(0);
        let declared_height = page_height.max(0);
        let (media_width, media_height) = if let Some(paper_size) = self.paper_size {
            Self::physical_paper_coordinates(paper_size)?
        } else {
            (
                PdfCoordinate::from_scaled(declared_width, self.magnification)?
                    .checked_add(double_margin)?,
                PdfCoordinate::from_scaled(declared_height, self.magnification)?
                    .checked_add(double_margin)?,
            )
        };
        if !media_width.is_positive() || !media_height.is_positive() {
            return Err(PdfBackendError::InvalidPageSize {
                width: media_width,
                height: media_height,
            });
        }
        self.current_font = None;
        self.page = Some(PageState {
            coordinate_media_height: media_height,
            paper_size: self.paper_size,
            declared_height,
            min_horizontal: 0,
            max_horizontal: declared_width,
            min_vertical: 0,
            max_vertical: declared_height,
            position: Position {
                horizontal: 0,
                vertical: 0,
            },
            position_stack: Vec::new(),
            fonts: Vec::new(),
            content: Vec::new(),
        });
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), Self::Error> {
        let Some(page) = self.page.as_ref() else {
            return Err(PdfBackendError::NoOpenPage);
        };
        if !page.position_stack.is_empty() {
            return Err(PdfBackendError::UnbalancedPushes(page.position_stack.len()));
        }
        let page = self.page.take().expect("page presence checked above");
        let (media_width, media_height, translate_x, translate_y) =
            if let Some(paper_size) = page.paper_size {
                let (media_width, media_height) = Self::physical_paper_coordinates(paper_size)?;
                // specialはpage contentの途中に現れ得る。座標基底をその場で変えず、pageを
                // 閉じる時にcontent全体を一度だけ新しい上端へ合わせる。
                let zero = PdfCoordinate::from_scaled(0, 1000)?;
                let translate_y = media_height.checked_sub(page.coordinate_media_height)?;
                (media_width, media_height, zero, translate_y)
            } else {
                let left = PdfCoordinate::from_scaled(page.min_horizontal, self.magnification)?;
                let right = PdfCoordinate::from_scaled(page.max_horizontal, self.magnification)?;
                let top = PdfCoordinate::from_scaled(page.min_vertical, self.magnification)?;
                let bottom = PdfCoordinate::from_scaled(page.max_vertical, self.magnification)?;
                let double_margin = PdfCoordinate::ONE_INCH.checked_add(PdfCoordinate::ONE_INCH)?;
                let media_width = right.checked_sub(left)?.checked_add(double_margin)?;
                let media_height = bottom.checked_sub(top)?.checked_add(double_margin)?;

                // 既に書いた座標は宣言寸法を基準にしている。観測した描画範囲がboxから
                // はみ出す場合だけ平行移動し、左右上下の物理1inch余白を保つ。
                let zero = PdfCoordinate::from_scaled(0, self.magnification)?;
                let declared_height =
                    PdfCoordinate::from_scaled(page.declared_height, self.magnification)?;
                let translate_x = zero.checked_sub(left)?;
                let translate_y = bottom.checked_sub(declared_height)?;
                (media_width, media_height, translate_x, translate_y)
            };
        let zero = PdfCoordinate::from_scaled(0, 1000)?;
        let content = if translate_x != zero || translate_y != zero {
            let mut translated =
                format!("q\n1 0 0 1 {translate_x} {translate_y} cm\n").into_bytes();
            translated.extend_from_slice(&page.content);
            translated.extend_from_slice(b"Q\n");
            translated
        } else {
            page.content
        };
        self.document.add_page(PdfPage {
            width: media_width,
            height: media_height,
            courier_font: Some(self.courier_font),
            fonts: &page.fonts,
            resource_entries: b"",
            content: &content,
        })?;
        self.current_font = None;
        Ok(())
    }

    fn push(&mut self) -> Result<(), Self::Error> {
        let page = self.current_page_mut()?;
        page.position_stack.push(page.position);
        Ok(())
    }

    fn pop(&mut self) -> Result<(), Self::Error> {
        let page = self.current_page_mut()?;
        page.position = page
            .position_stack
            .pop()
            .ok_or(PdfBackendError::PositionStackUnderflow)?;
        Ok(())
    }

    fn move_right(&mut self, amount: Scaled) -> Result<(), Self::Error> {
        let page = self.current_page_mut()?;
        page.position.horizontal = Self::checked_move(page.position.horizontal, amount)?;
        Ok(())
    }

    fn move_down(&mut self, amount: Scaled) -> Result<(), Self::Error> {
        let page = self.current_page_mut()?;
        page.position.vertical = Self::checked_move(page.position.vertical, amount)?;
        Ok(())
    }

    fn define_font(&mut self, font: OutputFontDefinition<'_>) -> Result<(), Self::Error> {
        if font.at_size <= 0 {
            return Err(PdfBackendError::InvalidFontSize {
                font_number: font.font_number,
                at_size: font.at_size,
            });
        }
        if self.fonts.contains_key(&font.font_number) {
            return Err(PdfBackendError::FontAlreadyDefined(font.font_number));
        }

        let state = match font.kind {
            OutputFontKind::Byte => {
                let type1 = if self.type1_loader.is_some() {
                    let tfm_name = logical_font_name(font.font_number, font.name)?;
                    Some(self.load_type1_font(tfm_name, font)?)
                } else {
                    None
                };
                PdfFontState::Byte {
                    at_size: font.at_size,
                    type1,
                }
            }
            OutputFontKind::Japanese => {
                let jfm_name = logical_font_name(font.font_number, font.name)?;
                let loader = self.named_cid_loader.as_mut().ok_or_else(|| {
                    PdfBackendError::MissingNamedCidProfile {
                        font_number: font.font_number,
                        jfm_name: jfm_name.to_owned(),
                    }
                })?;
                let profile = loader.load(jfm_name)?;
                if profile.jfm_name() != jfm_name {
                    return Err(PdfBackendError::NamedCidProfile(
                        NamedCidProfileError::JfmNameMismatch {
                            path: None,
                            profile_name: profile.jfm_name().to_owned(),
                            requested_name: jfm_name.to_owned(),
                        },
                    ));
                }
                let prepared = prepare_named_cid_font(&profile).map_err(PdfDocumentError::from)?;
                let named_cid = self.document.add_named_cid_font(prepared)?;
                PdfFontState::Japanese {
                    at_size: font.at_size,
                    named_cid,
                    encoding: profile.encoding(),
                }
            }
        };

        self.fonts.insert(font.font_number, state);
        Ok(())
    }

    fn set_font(&mut self, font_number: u32) -> Result<(), Self::Error> {
        if self.page.is_none() {
            return Err(PdfBackendError::NoOpenPage);
        }
        if !self.fonts.contains_key(&font_number) {
            return Err(PdfBackendError::UndefinedFont(font_number));
        }
        self.current_font = Some(font_number);
        Ok(())
    }

    fn set_char(&mut self, character: u8, width: Scaled) -> Result<(), Self::Error> {
        if self.page.is_none() {
            return Err(PdfBackendError::NoOpenPage);
        }
        let font_number = self.current_font.ok_or(PdfBackendError::NoCurrentFont)?;
        let font = *self
            .fonts
            .get(&font_number)
            .ok_or(PdfBackendError::UndefinedFont(font_number))?;
        let (at_size, type1) = match font {
            PdfFontState::Byte { at_size, type1 } => (at_size, type1),
            PdfFontState::Japanese { .. } => {
                return Err(PdfBackendError::FontKindMismatch {
                    font_number,
                    operation: PdfGlyphOperation::ByteCharacter,
                    actual: OutputFontKind::Japanese,
                });
            }
        };
        let magnification = self.magnification;
        let page = self.current_page_mut()?;
        let next_horizontal = Self::checked_move(page.position.horizontal, width)?;
        if let Some(type1) = type1 {
            if character < type1.first_char() || character > type1.last_char() {
                return Err(PdfBackendError::CharacterOutsideEmbeddedFont {
                    font_number,
                    character,
                    first_char: type1.first_char(),
                    last_char: type1.last_char(),
                });
            }
            if !type1.contains_code(character) {
                return Err(PdfBackendError::CharacterNotPreparedForEmbeddedFont {
                    font_number,
                    character,
                });
            }
            let resource_index = match page
                .fonts
                .iter()
                .position(|&existing| existing == PdfPageFont::Type1(type1))
            {
                Some(index) => index,
                None => {
                    page.fonts.push(PdfPageFont::Type1(type1));
                    page.fonts.len() - 1
                }
            };
            // Courier remains `/F1`; typed embedded fonts start at `/F2`.
            let resource_number = resource_index + 2;
            Self::append_type1_character(page, magnification, character, at_size, resource_number)?;
        } else if (b' '..=b'~').contains(&character) {
            Self::append_courier_character(page, magnification, character, at_size)?;
        }
        Self::include_interval(
            &mut page.min_horizontal,
            &mut page.max_horizontal,
            page.position.horizontal,
            next_horizontal,
        );
        Self::include_interval(
            &mut page.min_vertical,
            &mut page.max_vertical,
            page.position.vertical,
            page.position.vertical,
        );
        page.position.horizontal = next_horizontal;
        Ok(())
    }

    fn set_wide_char(&mut self, character: u32, width: Scaled) -> Result<(), Self::Error> {
        let page = self.page.as_ref().ok_or(PdfBackendError::NoOpenPage)?;
        let font_number = self.current_font.ok_or(PdfBackendError::NoCurrentFont)?;
        let font = *self
            .fonts
            .get(&font_number)
            .ok_or(PdfBackendError::UndefinedFont(font_number))?;
        let (at_size, named_cid, encoding) = match font {
            PdfFontState::Japanese {
                at_size,
                named_cid,
                encoding,
            } => (at_size, named_cid, encoding),
            PdfFontState::Byte { .. } => {
                return Err(PdfBackendError::FontKindMismatch {
                    font_number,
                    operation: PdfGlyphOperation::WideCharacter,
                    actual: OutputFontKind::Byte,
                });
            }
        };
        if character > 0xffff {
            return Err(PdfBackendError::NonBmpCharacter {
                font_number,
                character,
            });
        }
        if (0xd800..=0xdfff).contains(&character) {
            return Err(PdfBackendError::InvalidBmpScalar {
                font_number,
                character,
            });
        }
        let source_code = encoding
            .encode_scalar(character)
            .expect("range checks and named CID encoding contract must agree");

        // ここまでと以下の計算をすべて済ませてからpage stateを変更する。失敗した
        // glyphがresource/content/位置のどれかだけを残すことはない。
        let next_horizontal = Self::checked_move(page.position.horizontal, width)?;
        let page_font = PdfPageFont::NamedCid(named_cid);
        let existing_index = page.fonts.iter().position(|&font| font == page_font);
        let resource_index = existing_index.unwrap_or(page.fonts.len());
        let resource_number = resource_index + 2;
        let content = Self::named_cid_character_content(
            page,
            self.magnification,
            source_code,
            at_size,
            resource_number,
        )?;

        let page = self.current_page_mut()?;
        if existing_index.is_none() {
            page.fonts.push(page_font);
        }
        page.content.extend_from_slice(&content);
        Self::include_interval(
            &mut page.min_horizontal,
            &mut page.max_horizontal,
            page.position.horizontal,
            next_horizontal,
        );
        Self::include_interval(
            &mut page.min_vertical,
            &mut page.max_vertical,
            page.position.vertical,
            page.position.vertical,
        );
        page.position.horizontal = next_horizontal;
        Ok(())
    }

    fn set_rule(&mut self, height: Scaled, width: Scaled) -> Result<(), Self::Error> {
        let magnification = self.magnification;
        let page = self.current_page_mut()?;
        let next_horizontal = Self::checked_move(page.position.horizontal, width)?;
        let top = page
            .position
            .vertical
            .checked_sub(height)
            .ok_or(PdfBackendError::PositionOverflow)?;
        Self::append_rule(page, magnification, height, width)?;
        Self::include_interval(
            &mut page.min_horizontal,
            &mut page.max_horizontal,
            page.position.horizontal,
            next_horizontal,
        );
        Self::include_interval(
            &mut page.min_vertical,
            &mut page.max_vertical,
            top,
            page.position.vertical,
        );
        page.position.horizontal = next_horizontal;
        Ok(())
    }

    fn put_rule(&mut self, height: Scaled, width: Scaled) -> Result<(), Self::Error> {
        let magnification = self.magnification;
        let page = self.current_page_mut()?;
        let horizontal_end = Self::checked_move(page.position.horizontal, width)?;
        let top = page
            .position
            .vertical
            .checked_sub(height)
            .ok_or(PdfBackendError::PositionOverflow)?;
        Self::append_rule(page, magnification, height, width)?;
        Self::include_interval(
            &mut page.min_horizontal,
            &mut page.max_horizontal,
            page.position.horizontal,
            horizontal_end,
        );
        Self::include_interval(
            &mut page.min_vertical,
            &mut page.max_vertical,
            top,
            page.position.vertical,
        );
        Ok(())
    }

    fn write_special(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        if self.page.is_none() {
            return Err(PdfBackendError::NoOpenPage);
        }
        let Some(paper_size) = parse_pdf_special(bytes)? else {
            return Ok(());
        };
        if self.document.page_count() != 0 && self.paper_size != Some(paper_size) {
            return Err(PdfBackendError::PaperSizeChangedAfterFirstPage {
                original: self.paper_size,
                requested: paper_size,
            });
        }
        // 第一page内では最後のpapersize specialを採用する。閉じた後は同一値だけ許す。
        self.paper_size = Some(paper_size);
        self.page
            .as_mut()
            .expect("page presence checked above")
            .paper_size = Some(paper_size);
        Ok(())
    }

    fn page_count(&self) -> usize {
        self.document.page_count()
    }

    fn finish(self) -> Result<usize, Self::Error> {
        let (_, byte_count) = self.finish_with_target()?;
        Ok(byte_count)
    }
}

#[derive(Debug)]
pub(crate) enum PdfBackendError {
    Document(PdfDocumentError),
    FontResource(FontResourceError),
    NamedCidProfile(NamedCidProfileError),
    Special(PdfSpecialError),
    InvalidMagnification(Scaled),
    InvalidPageSize {
        width: PdfCoordinate,
        height: PdfCoordinate,
    },
    PaperSizeChangedAfterFirstPage {
        original: Option<PdfPaperSize>,
        requested: PdfPaperSize,
    },
    PageAlreadyOpen,
    NoOpenPage,
    PageStillOpen,
    PositionOverflow,
    PositionStackUnderflow,
    UnbalancedPushes(usize),
    FontAlreadyDefined(u32),
    Type1FontAlreadyAttached(u32),
    InvalidLogicalFontName {
        font_number: u32,
        name: Vec<u8>,
    },
    UndefinedFont(u32),
    NoCurrentFont,
    MissingNamedCidProfile {
        font_number: u32,
        jfm_name: String,
    },
    FontKindMismatch {
        font_number: u32,
        operation: PdfGlyphOperation,
        actual: OutputFontKind,
    },
    NonBmpCharacter {
        font_number: u32,
        character: u32,
    },
    InvalidBmpScalar {
        font_number: u32,
        character: u32,
    },
    CharacterOutsideEmbeddedFont {
        font_number: u32,
        character: u8,
        first_char: u8,
        last_char: u8,
    },
    CharacterNotPreparedForEmbeddedFont {
        font_number: u32,
        character: u8,
    },
    InvalidFontSize {
        font_number: u32,
        at_size: Scaled,
    },
    OutputTooLarge(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfGlyphOperation {
    AttachType1,
    ByteCharacter,
    WideCharacter,
}

impl fmt::Display for PdfBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::FontResource(error) => error.fmt(formatter),
            Self::NamedCidProfile(error) => error.fmt(formatter),
            Self::Special(error) => error.fmt(formatter),
            Self::InvalidMagnification(magnification) => {
                write!(formatter, "invalid PDF magnification {magnification}")
            }
            Self::InvalidPageSize { width, height } => {
                write!(formatter, "invalid PDF page size {width} by {height}")
            }
            Self::PaperSizeChangedAfterFirstPage {
                original,
                requested,
            } => match original {
                Some(original) => write!(
                    formatter,
                    "PDF papersize changed after the first page from {} by {} to {} by {}",
                    original.width, original.height, requested.width, requested.height
                ),
                None => write!(
                    formatter,
                    "PDF papersize was first specified after the first page as {} by {}",
                    requested.width, requested.height
                ),
            },
            Self::PageAlreadyOpen => formatter.write_str("a PDF page is already open"),
            Self::NoOpenPage => formatter.write_str("no PDF page is open"),
            Self::PageStillOpen => formatter.write_str("cannot finish PDF with an open page"),
            Self::PositionOverflow => formatter.write_str("PDF shipout position overflow"),
            Self::PositionStackUnderflow => {
                formatter.write_str("PDF shipout position stack underflow")
            }
            Self::UnbalancedPushes(depth) => {
                write!(formatter, "PDF page ended with {depth} unmatched pushes")
            }
            Self::FontAlreadyDefined(font) => {
                write!(formatter, "PDF font number {font} was defined twice")
            }
            Self::Type1FontAlreadyAttached(font) => {
                write!(formatter, "PDF font number {font} already has a Type 1 object")
            }
            Self::InvalidLogicalFontName { font_number, name } => write!(
                formatter,
                "logical name bytes {name:?} for PDF font number {font_number} are not UTF-8"
            ),
            Self::UndefinedFont(font) => write!(formatter, "undefined PDF font number {font}"),
            Self::NoCurrentFont => formatter.write_str("no current PDF font"),
            Self::MissingNamedCidProfile {
                font_number,
                jfm_name,
            } => write!(
                formatter,
                "Japanese PDF font {font_number} (`{jfm_name}`) requires an explicit named CID profile"
            ),
            Self::FontKindMismatch {
                font_number,
                operation,
                actual,
            } => write!(
                formatter,
                "PDF {operation:?} cannot use {actual:?} font number {font_number}"
            ),
            Self::NonBmpCharacter {
                font_number,
                character,
            } => write!(
                formatter,
                "Japanese PDF font {font_number} cannot encode non-BMP character U+{character:06X} with UniJIS-UCS2-H"
            ),
            Self::InvalidBmpScalar {
                font_number,
                character,
            } => write!(
                formatter,
                "Japanese PDF font {font_number} cannot encode surrogate U+{character:04X}"
            ),
            Self::CharacterOutsideEmbeddedFont {
                font_number,
                character,
                first_char,
                last_char,
            } => write!(
                formatter,
                "character {character} is outside embedded PDF font {font_number} range {first_char}..={last_char}"
            ),
            Self::CharacterNotPreparedForEmbeddedFont {
                font_number,
                character,
            } => write!(
                formatter,
                "character {character} was not prepared for embedded PDF font {font_number}"
            ),
            Self::InvalidFontSize {
                font_number,
                at_size,
            } => write!(
                formatter,
                "invalid PDF font size {at_size} for font number {font_number}"
            ),
            Self::OutputTooLarge(byte_count) => write!(
                formatter,
                "PDF byte count {byte_count} does not fit in usize"
            ),
        }
    }
}

impl std::error::Error for PdfBackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::FontResource(error) => Some(error),
            Self::NamedCidProfile(error) => Some(error),
            Self::Special(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PdfDocumentError> for PdfBackendError {
    fn from(error: PdfDocumentError) -> Self {
        Self::Document(error)
    }
}

impl From<FontResourceError> for PdfBackendError {
    fn from(error: FontResourceError) -> Self {
        Self::FontResource(error)
    }
}

impl From<NamedCidProfileError> for PdfBackendError {
    fn from(error: NamedCidProfileError) -> Self {
        Self::NamedCidProfile(error)
    }
}

impl From<PdfSpecialError> for PdfBackendError {
    fn from(error: PdfSpecialError) -> Self {
        Self::Special(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{PdfBackend, PdfBackendError, Type1FontIdentity};
    use crate::file_search::{
        CommandExecutor, CommandOutput, KpsewhichResolver, LogicalFileName, ResolverOptions,
    };
    use crate::font_resources::afm::{AfmDescriptor, AfmFont, AfmGlyphMetric, AfmNumber};
    use crate::font_resources::loader::{
        FontResourceError, FontResourceLoader, LoadedType1Font, Type1ResourceLoader,
    };
    use crate::font_resources::map::EmbedPolicy;
    use crate::font_resources::named_cid::{
        NamedCidFontProfile, NamedCidFontProfileLoader, NamedCidProfileError,
    };
    use crate::font_resources::type1::Type1FontProgram;
    use crate::output::output_backend::{OutputFontDefinition, OutputFontKind, ShipoutBackend};
    use crate::pdf_document::PdfDocumentError;
    use crate::pdf_font::{
        prepare_type1_font, MissingStemVPolicy, PdfFontError, PdfType1FontRequest,
    };
    use crate::scaled::Scaled;

    use std::cell::Cell;
    use std::collections::{BTreeMap, VecDeque};
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct SyntheticExecutor {
        resolved_paths: VecDeque<PathBuf>,
    }

    impl CommandExecutor for SyntheticExecutor {
        fn execute(
            &mut self,
            _program: &OsStr,
            _arguments: &[OsString],
        ) -> io::Result<CommandOutput> {
            let path = self.resolved_paths.pop_front().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "合成Type 1 resolver応答が足りない",
                )
            })?;
            let mut stdout = path
                .to_str()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "合成temp pathがUTF-8でない")
                })?
                .as_bytes()
                .to_vec();
            stdout.extend_from_slice(b"\r\n");
            Ok(CommandOutput {
                code: Some(0),
                stdout,
                stderr: Vec::new(),
            })
        }
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "rtex-pdf-backend-type1-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.path.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let is_own_directory = self.path.starts_with(std::env::temp_dir())
                && self.path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .starts_with("rtex-pdf-backend-type1-")
                });
            if is_own_directory {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }

    type SyntheticLoader = FontResourceLoader<KpsewhichResolver<SyntheticExecutor>>;

    struct CountingType1Loader<L> {
        inner: L,
        load_count: Rc<Cell<usize>>,
    }

    impl<L: Type1ResourceLoader> Type1ResourceLoader for CountingType1Loader<L> {
        fn load(&mut self, tfm_name: &str) -> Result<LoadedType1Font, FontResourceError> {
            self.load_count.set(self.load_count.get() + 1);
            self.inner.load(tfm_name)
        }
    }

    struct SyntheticNamedCidLoader {
        profile: NamedCidFontProfile,
    }

    impl NamedCidFontProfileLoader for SyntheticNamedCidLoader {
        fn load(&mut self, _jfm_name: &str) -> Result<NamedCidFontProfile, NamedCidProfileError> {
            Ok(self.profile.clone())
        }
    }

    fn named_cid_profile(jfm_name: &str) -> NamedCidFontProfile {
        NamedCidFontProfile::parse(
            format!(
                "PraTeX-Named-CID-Profile 1\n\
                 JfmName {jfm_name}\n\
                 BaseFont HeiseiMin-W3\n\
                 Flags 6\n\
                 FontBBox -123 -257 1001 910\n\
                 ItalicAngle 0\n\
                 Ascent 880\n\
                 Descent -120\n\
                 CapHeight 700\n\
                 StemV 80\n\
                 DefaultWidth 1000\n\
                 EndProfile\n"
            )
            .as_bytes(),
        )
        .unwrap()
    }

    fn synthetic_pfb() -> Vec<u8> {
        fn segment(kind: u8, payload: &[u8]) -> Vec<u8> {
            let mut bytes = vec![0x80, kind];
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(payload);
            bytes
        }

        let mut key = 55_665u16;
        let encrypted: Vec<u8> = b"rand"
            .iter()
            .copied()
            .chain(
                b"/Private 1 dict dup begin\n/StdVW [80] ND\n/Subrs 0 array\n"
                    .iter()
                    .copied(),
            )
            .map(|plain| {
                let cipher = plain ^ (key >> 8) as u8;
                key = key
                    .wrapping_add(u16::from(cipher))
                    .wrapping_mul(52_845)
                    .wrapping_add(22_719);
                cipher
            })
            .collect();
        let mut bytes = segment(1, b"%!PS synthetic\n");
        bytes.extend_from_slice(&segment(2, &encrypted));
        bytes.extend_from_slice(&segment(1, b"cleartomark\n"));
        bytes.extend_from_slice(&[0x80, 3]);
        bytes
    }

    fn synthetic_afm_bytes() -> &'static [u8] {
        b"StartFontMetrics 4.1\n\
FontName BackendSynthetic\n\
EncodingScheme FontSpecific\n\
FontBBox -40 -250 1000 750\n\
ItalicAngle 0\n\
IsFixedPitch false\n\
CapHeight 680\n\
XHeight 430\n\
Ascender 700\n\
Descender -200\n\
StartCharMetrics 1\n\
C 65 ; WX 750 ; N A ;\n\
EndCharMetrics\n\
EndFontMetrics\n"
    }

    fn synthetic_loader(
        embedding_marker: &str,
        flags: Option<u32>,
    ) -> (TestDirectory, SyntheticLoader, String) {
        static NEXT_FONT_ID: AtomicUsize = AtomicUsize::new(0);
        let directory = TestDirectory::new();
        let id = NEXT_FONT_ID.fetch_add(1, Ordering::Relaxed);
        let tfm_name = format!("rtex-pdf-backend-tfm-{id}");
        let font_name = format!("rtex-pdf-backend-font-{id}.pfb");
        let pfb_path = directory.write("physical-font.data", &synthetic_pfb());
        let afm_path = directory.write("physical-metrics.data", synthetic_afm_bytes());
        let flags = flags.map(|flags| format!(" {flags}")).unwrap_or_default();
        let map_path = directory.write(
            "physical-map.data",
            format!("{tfm_name} BackendSynthetic{flags} {embedding_marker}{font_name}\n")
                .as_bytes(),
        );
        let resolver = KpsewhichResolver::new(
            ResolverOptions::default().with_kpsewhich_program("synthetic-kpsewhich"),
            SyntheticExecutor {
                resolved_paths: VecDeque::from([pfb_path, afm_path]),
            },
        );
        let loader =
            FontResourceLoader::with_map(resolver, LogicalFileName::new(map_path.into_os_string()))
                .unwrap();
        (directory, loader, tfm_name)
    }

    fn font_definition(font_number: u32, at_size: Scaled) -> OutputFontDefinition<'static> {
        OutputFontDefinition {
            kind: OutputFontKind::Byte,
            font_number,
            checksum: 0,
            at_size,
            design_size: at_size,
            area: b"",
            name: b"synthetic",
            first_char: 0,
            last_char: 127,
            existing_codes: &[b'A'],
        }
    }

    fn japanese_font_definition(
        font_number: u32,
        at_size: Scaled,
        name: &'static [u8],
    ) -> OutputFontDefinition<'static> {
        OutputFontDefinition {
            kind: OutputFontKind::Japanese,
            font_number,
            checksum: 0,
            at_size,
            design_size: at_size,
            area: b"",
            name,
            first_char: 0,
            last_char: 0,
            existing_codes: &[],
        }
    }

    fn 含む(bytes: &[u8], expected: &[u8]) -> bool {
        bytes
            .windows(expected.len())
            .any(|window| window == expected)
    }

    fn 出現数(bytes: &[u8], expected: &[u8]) -> usize {
        bytes
            .windows(expected.len())
            .filter(|window| *window == expected)
            .count()
    }

    fn afm_number(integer: i64) -> AfmNumber {
        AfmNumber::checked_from_integer(integer).unwrap()
    }

    fn synthetic_type1_afm() -> AfmFont {
        let a = AfmGlyphMetric {
            code: Some(b'A'),
            name: Some("A".to_owned()),
            width_x: afm_number(750),
        };
        let b = AfmGlyphMetric {
            code: Some(b'B'),
            name: Some("B".to_owned()),
            width_x: afm_number(700),
        };
        let c = AfmGlyphMetric {
            code: Some(b'C'),
            name: Some("C".to_owned()),
            width_x: afm_number(725),
        };
        AfmFont {
            descriptor: AfmDescriptor {
                font_name: "BackendSynthetic".to_owned(),
                encoding_scheme: Some("FontSpecific".to_owned()),
                font_bbox: [
                    afm_number(-40),
                    afm_number(-250),
                    afm_number(1000),
                    afm_number(750),
                ],
                italic_angle: AfmNumber::ZERO,
                is_fixed_pitch: false,
                cap_height: afm_number(680),
                x_height: Some(afm_number(430)),
                ascender: afm_number(700),
                descender: afm_number(-200),
                std_vw: Some(afm_number(80)),
                std_hw: None,
            },
            metrics_by_name: BTreeMap::from([
                ("A".to_owned(), a.clone()),
                ("B".to_owned(), b.clone()),
                ("C".to_owned(), c.clone()),
            ]),
            metrics_by_code: BTreeMap::from([(b'A', a), (b'B', b), (b'C', c)]),
        }
    }

    #[test]
    fn 一inch余白を含む複数pageを閉じる() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend.start_page(&[0; 10], 65536, 65536).unwrap();
        backend.end_page().unwrap();
        backend.start_page(&[0; 10], 131072, 196608).unwrap();
        backend.end_page().unwrap();
        assert_eq!(backend.page_count(), 2);
        let (pdf, byte_count) = backend.finish_with_target().unwrap();

        assert_eq!(byte_count, pdf.len());
        assert!(含む(&pdf, b"/MediaBox [0 0 144.996264 144.996264]"));
        assert!(含む(&pdf, b"/MediaBox [0 0 146.988792 145.992528]"));
        assert!(含む(&pdf, b"/Count 2"));
    }

    #[test]
    fn 一inch余白はmagで拡大しない() {
        let mut backend = PdfBackend::new(Vec::new(), 1200).unwrap();
        backend.start_page(&[0; 10], 65536, 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"/MediaBox [0 0 145.195517 145.195517]"));
    }

    #[test]
    fn courierで文字ごとの絶対位置とfont_sizeを書く() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend
            .start_page(&[0; 10], 20 * 65536, 20 * 65536)
            .unwrap();
        backend.define_font(font_definition(3, 10 * 65536)).unwrap();
        backend.set_font(3).unwrap();
        backend.move_right(65536).unwrap();
        backend.move_down(2 * 65536).unwrap();
        backend.set_char(b'(', 65536).unwrap();
        backend.set_char(0x80, 65536).unwrap();
        backend.set_char(b'A', 65536).unwrap();
        backend.define_font(font_definition(7, 20 * 65536)).unwrap();
        backend.set_font(7).unwrap();
        backend.set_char(b'\\', 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"/BaseFont /Courier"));
        assert!(含む(&pdf, b"/Font <<\n/F1 3 0 R\n>>"));
        assert!(含む(&pdf, b"/F1 9.96264 Tf"));
        assert!(含む(&pdf, b"1 0 0 1 72.996264 89.932752 Tm\n(\\() Tj"));
        assert!(含む(&pdf, b"1 0 0 1 74.988792 89.932752 Tm\n(A) Tj"));
        assert!(含む(&pdf, b"/F1 19.92528 Tf"));
        assert!(含む(&pdf, b"(\\\\) Tj"));
        assert_eq!(
            pdf.windows(b") Tj".len())
                .filter(|window| *window == b") Tj")
                .count(),
            3
        );
    }

    #[test]
    fn named_cidでwide文字をjfm幅の絶対位置へ書く() {
        let loader = SyntheticNamedCidLoader {
            profile: named_cid_profile("min10"),
        };
        let mut backend = PdfBackend::with_named_cid_loader(Vec::new(), 1000, loader).unwrap();
        backend
            .define_font(japanese_font_definition(257, 10 * 65536, b"min10"))
            .unwrap();
        backend
            .start_page(&[0; 10], 20 * 65536, 20 * 65536)
            .unwrap();
        backend.set_font(257).unwrap();
        backend.set_wide_char(0x3042, 5 * 65536).unwrap();
        backend.set_wide_char(0x65e5, 7 * 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();
        let text = String::from_utf8_lossy(&pdf);

        for required in [
            "/Subtype /Type0",
            "/Subtype /CIDFontType0",
            "/Encoding /UniJIS-UCS2-H",
            "/CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 4 >>",
            "/ToUnicode 7 0 R",
            "/CMapName /PraTeX-UniJIS-UCS2-ToUnicode",
            "/Font <<\n/F1 3 0 R\n/F2 6 0 R\n>>",
            "/F2 9.96264 Tf\n1 0 0 1 72 91.92528 Tm\n<3042> Tj",
            "/F2 9.96264 Tf\n1 0 0 1 76.98132 91.92528 Tm\n<65E5> Tj",
        ] {
            assert!(text.contains(required), "missing {required}");
        }
        assert_eq!(text.matches("/Subtype /Type0").count(), 1);
        assert_eq!(text.matches("/DescendantFonts [").count(), 1);
        assert!(!text.contains("/W ["));
        assert!(!text.contains("/FontFile"));
    }

    #[test]
    fn 和文profile欠損とjfm名不一致をfont定義時に拒む() {
        let mut without_profile = PdfBackend::new(Vec::new(), 1000).unwrap();
        assert!(matches!(
            without_profile.define_font(japanese_font_definition(
                257,
                10 * 65536,
                b"min10"
            )),
            Err(PdfBackendError::MissingNamedCidProfile {
                font_number: 257,
                ref jfm_name,
            }) if jfm_name == "min10"
        ));
        assert!(!without_profile.fonts.contains_key(&257));

        let loader = SyntheticNamedCidLoader {
            profile: named_cid_profile("goth10"),
        };
        let mut mismatched = PdfBackend::with_named_cid_loader(Vec::new(), 1000, loader).unwrap();
        assert!(matches!(
            mismatched.define_font(japanese_font_definition(257, 10 * 65536, b"min10")),
            Err(PdfBackendError::NamedCidProfile(
                NamedCidProfileError::JfmNameMismatch {
                    ref profile_name,
                    ref requested_name,
                    ..
                }
            )) if profile_name == "goth10" && requested_name == "min10"
        ));
        assert!(!mismatched.fonts.contains_key(&257));
    }

    #[test]
    fn wide文字の範囲とfont種別errorはpageを部分更新しない() {
        let loader = SyntheticNamedCidLoader {
            profile: named_cid_profile("min10"),
        };
        let mut backend = PdfBackend::with_named_cid_loader(Vec::new(), 1000, loader).unwrap();
        backend
            .define_font(japanese_font_definition(257, 10 * 65536, b"min10"))
            .unwrap();
        backend.define_font(font_definition(3, 10 * 65536)).unwrap();
        backend
            .start_page(&[0; 10], 20 * 65536, 20 * 65536)
            .unwrap();

        backend.set_font(257).unwrap();
        let before = backend.page.as_ref().unwrap();
        let before_position = before.position.horizontal;
        let before_content = before.content.clone();
        let before_fonts = before.fonts.clone();
        assert!(matches!(
            backend.set_wide_char(0x1_0000, 5 * 65536),
            Err(PdfBackendError::NonBmpCharacter {
                font_number: 257,
                character: 0x1_0000,
            })
        ));
        assert!(matches!(
            backend.set_char(b'A', 5 * 65536),
            Err(PdfBackendError::FontKindMismatch {
                font_number: 257,
                operation: super::PdfGlyphOperation::ByteCharacter,
                actual: OutputFontKind::Japanese,
            })
        ));
        let after = backend.page.as_ref().unwrap();
        assert_eq!(after.position.horizontal, before_position);
        assert_eq!(after.content, before_content);
        assert_eq!(after.fonts, before_fonts);

        backend.set_font(3).unwrap();
        let before_position = backend.page.as_ref().unwrap().position.horizontal;
        assert!(matches!(
            backend.set_wide_char(0x3042, 5 * 65536),
            Err(PdfBackendError::FontKindMismatch {
                font_number: 3,
                operation: super::PdfGlyphOperation::WideCharacter,
                actual: OutputFontKind::Byte,
            })
        ));
        let after = backend.page.as_ref().unwrap();
        assert_eq!(after.position.horizontal, before_position);
        assert_eq!(after.content, before_content);
        assert_eq!(after.fonts, before_fonts);
    }

    #[test]
    fn type1_handleをpage_resourceと文字命令へ同じ番号で結ぶ() {
        let program = Type1FontProgram {
            bytes: b"ab\0cd".to_vec(),
            length1: 2,
            length2: 1,
            length3: 2,
        };
        let afm = synthetic_type1_afm();
        let prepared = prepare_type1_font(PdfType1FontRequest {
            program: &program,
            afm: &afm,
            encoding: None,
            embedding: EmbedPolicy::Full,
            descriptor_flags: 6,
            missing_stem_v: MissingStemVPolicy::Reject,
            used_codes: &[b'A'],
        })
        .unwrap();

        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend
            .define_font(OutputFontDefinition {
                name: b"synthetic",
                first_char: b'A',
                last_char: b'A',
                ..font_definition(3, 10 * 65536)
            })
            .unwrap();
        backend.attach_type1_font(3, prepared).unwrap();
        backend
            .start_page(&[0; 10], 20 * 65536, 20 * 65536)
            .unwrap();
        backend.set_font(3).unwrap();
        backend.set_char(b'A', 5 * 65536).unwrap();
        assert!(matches!(
            backend.set_char(b'B', 5 * 65536),
            Err(PdfBackendError::CharacterOutsideEmbeddedFont {
                font_number: 3,
                character: b'B',
                ..
            })
        ));
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"/BaseFont /BackendSynthetic"));
        assert!(含む(&pdf, b"/Font <<\n/F1 3 0 R\n/F2"));
        assert!(含む(&pdf, b"/F2 9.96264 Tf"));
        assert!(含む(&pdf, b"<41> Tj"));
    }

    #[test]
    fn type1の疎なcode集合では未準備の中間文字を拒む() {
        let program = Type1FontProgram {
            bytes: b"ab\0cd".to_vec(),
            length1: 2,
            length2: 1,
            length3: 2,
        };
        let afm = synthetic_type1_afm();
        let prepared = prepare_type1_font(PdfType1FontRequest {
            program: &program,
            afm: &afm,
            encoding: None,
            embedding: EmbedPolicy::Full,
            descriptor_flags: 6,
            missing_stem_v: MissingStemVPolicy::Reject,
            used_codes: &[b'A', b'C'],
        })
        .unwrap();

        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend
            .define_font(OutputFontDefinition {
                name: b"synthetic",
                first_char: b'A',
                last_char: b'C',
                ..font_definition(3, 10 * 65536)
            })
            .unwrap();
        backend.attach_type1_font(3, prepared).unwrap();
        backend
            .start_page(&[0; 10], 20 * 65536, 20 * 65536)
            .unwrap();
        backend.set_font(3).unwrap();
        backend.set_char(b'A', 5 * 65536).unwrap();
        assert!(matches!(
            backend.set_char(b'B', 5 * 65536),
            Err(PdfBackendError::CharacterNotPreparedForEmbeddedFont {
                font_number: 3,
                character: b'B',
            })
        ));
        backend.set_char(b'C', 5 * 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"<41> Tj"));
        assert!(!含む(&pdf, b"<42> Tj"));
        assert!(含む(&pdf, b"<43> Tj"));
    }

    #[test]
    fn full_mapからtype1を定義しpageへ埋め込む() {
        let (_directory, loader, tfm_name) = synthetic_loader("<<", Some(6));
        let mut backend = PdfBackend::with_type1_loader(Vec::new(), 1000, loader).unwrap();
        backend
            .start_page(&[0; 10], 20 * 65536, 20 * 65536)
            .unwrap();
        backend
            .define_font(OutputFontDefinition {
                name: tfm_name.as_bytes(),
                first_char: b'A',
                last_char: b'A',
                existing_codes: &[b'A'],
                ..font_definition(3, 10 * 65536)
            })
            .unwrap();
        backend.set_font(3).unwrap();
        backend.set_char(b'A', 5 * 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"/BaseFont /BackendSynthetic"));
        assert!(含む(&pdf, b"/FontFile "));
        assert!(含む(&pdf, b"/StemV 80"));
        assert!(含む(&pdf, b"/Length1 "));
        assert!(含む(&pdf, b"%!PS synthetic\n"));
        assert!(含む(&pdf, b"/F2 "));
        assert!(含む(&pdf, b"<41> Tj"));
    }

    #[test]
    fn 同じtype1論理fontの異なるsizeは埋込みobjectを共有する() {
        let (_directory, loader, tfm_name) = synthetic_loader("<<", Some(6));
        let load_count = Rc::new(Cell::new(0));
        let loader = CountingType1Loader {
            inner: loader,
            load_count: Rc::clone(&load_count),
        };
        let mut backend = PdfBackend::with_type1_loader(Vec::new(), 1000, loader).unwrap();
        backend
            .start_page(&[0; 10], 20 * 65536, 20 * 65536)
            .unwrap();

        backend
            .define_font(OutputFontDefinition {
                name: tfm_name.as_bytes(),
                first_char: b'A',
                last_char: b'A',
                existing_codes: &[b'A'],
                ..font_definition(3, 10 * 65536)
            })
            .unwrap();
        backend
            .define_font(OutputFontDefinition {
                name: tfm_name.as_bytes(),
                design_size: 10 * 65536,
                first_char: b'A',
                last_char: b'A',
                existing_codes: &[b'A'],
                ..font_definition(7, 20 * 65536)
            })
            .unwrap();

        backend.set_font(3).unwrap();
        backend.set_char(b'A', 5 * 65536).unwrap();
        backend.set_font(7).unwrap();
        backend.set_char(b'A', 10 * 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert_eq!(load_count.get(), 1);
        assert_eq!(出現数(&pdf, b"/BaseFont /BackendSynthetic"), 1);
        assert_eq!(出現数(&pdf, b"/FontFile "), 1);
        assert_eq!(出現数(&pdf, b"%!PS synthetic\n"), 1);
        assert!(含む(&pdf, b"/F2 9.96264 Tf"));
        assert!(含む(&pdf, b"/F2 19.92528 Tf"));
        assert!(!含む(&pdf, b"/F3 "));
    }

    #[test]
    fn type1共有identityはat_sizeだけを除外する() {
        let ten_point = OutputFontDefinition {
            name: b"roman",
            design_size: 10 * 65536,
            ..font_definition(3, 10 * 65536)
        };
        let twenty_point = OutputFontDefinition {
            name: b"roman",
            design_size: 10 * 65536,
            ..font_definition(7, 20 * 65536)
        };
        let other_logical_font = OutputFontDefinition {
            name: b"sans",
            design_size: 10 * 65536,
            ..font_definition(9, 20 * 65536)
        };
        let different_checksum = OutputFontDefinition {
            checksum: 1,
            ..twenty_point
        };
        let different_design_size = OutputFontDefinition {
            design_size: 11 * 65536,
            ..twenty_point
        };
        let different_codes = OutputFontDefinition {
            existing_codes: &[b'A', b'B'],
            ..twenty_point
        };

        assert_eq!(
            Type1FontIdentity::new("roman", ten_point),
            Type1FontIdentity::new("roman", twenty_point)
        );
        assert_ne!(
            Type1FontIdentity::new("roman", ten_point),
            Type1FontIdentity::new("sans", other_logical_font)
        );
        assert_ne!(
            Type1FontIdentity::new("roman", ten_point),
            Type1FontIdentity::new("roman", different_checksum)
        );
        assert_ne!(
            Type1FontIdentity::new("roman", ten_point),
            Type1FontIdentity::new("roman", different_design_size)
        );
        assert_ne!(
            Type1FontIdentity::new("roman", ten_point),
            Type1FontIdentity::new("roman", different_codes)
        );
    }

    #[test]
    fn subset_mapをfull埋め込みへ昇格しない() {
        let (_directory, loader, tfm_name) = synthetic_loader("<", Some(6));
        let mut backend = PdfBackend::with_type1_loader(Vec::new(), 1000, loader).unwrap();
        assert!(matches!(
            backend.define_font(OutputFontDefinition {
                name: tfm_name.as_bytes(),
                first_char: b'A',
                last_char: b'A',
                existing_codes: &[b'A'],
                ..font_definition(3, 10 * 65536)
            }),
            Err(PdfBackendError::Document(PdfDocumentError::Font(
                PdfFontError::SubsetEmbeddingUnsupported
            )))
        ));
    }

    #[test]
    fn mapのflags省略はpdftex既定のsymbolicを使う() {
        let (_directory, loader, tfm_name) = synthetic_loader("<<", None);
        let mut backend = PdfBackend::with_type1_loader(Vec::new(), 1000, loader).unwrap();
        backend
            .define_font(OutputFontDefinition {
                name: tfm_name.as_bytes(),
                first_char: b'A',
                last_char: b'A',
                existing_codes: &[b'A'],
                ..font_definition(3, 10 * 65536)
            })
            .unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"/Flags 4"));
    }

    #[test]
    fn 非utf8論理名を明示的に拒む() {
        let (_directory, loader, _) = synthetic_loader("<<", Some(6));
        let mut backend = PdfBackend::with_type1_loader(Vec::new(), 1000, loader).unwrap();
        assert!(matches!(
            backend.define_font(OutputFontDefinition {
                name: &[0xff],
                first_char: b'A',
                last_char: b'A',
                existing_codes: &[b'A'],
                ..font_definition(3, 10 * 65536)
            }),
            Err(PdfBackendError::InvalidLogicalFontName {
                font_number: 3,
                name,
            }) if name == [0xff]
        ));
    }

    #[test]
    fn push_popとruleを座標へ写しspecialは捨てる() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend
            .start_page(&[0; 10], 10 * 65536, 10 * 65536)
            .unwrap();
        backend.move_right(65536).unwrap();
        backend.move_down(2 * 65536).unwrap();
        backend.push().unwrap();
        backend.move_right(65536).unwrap();
        backend.move_down(65536).unwrap();
        backend.put_rule(65536, 2 * 65536).unwrap();
        backend.put_rule(65536, 2 * 65536).unwrap();
        backend.pop().unwrap();
        backend.set_rule(2 * 65536, 3 * 65536).unwrap();
        backend.put_rule(65536, 65536).unwrap();
        backend
            .write_special(b"RAW-SPECIAL-MUST-NOT-APPEAR")
            .unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"73.992528 78.973848 1.992528 0.996264 re f"));
        assert!(含む(&pdf, b"72.996264 79.970112 2.988792 1.992528 re f"));
        assert!(含む(&pdf, b"75.985056 79.970112 0.996264 0.996264 re f"));
        assert_eq!(
            pdf.windows(b"73.992528 78.973848 1.992528 0.996264 re f".len())
                .filter(|window| { *window == b"73.992528 78.973848 1.992528 0.996264 re f" })
                .count(),
            2
        );
        assert!(!含む(&pdf, b"RAW-SPECIAL-MUST-NOT-APPEAR"));
    }

    #[test]
    fn 第一pageの最後のpapersizeをmag非依存で後続pageへ継承する() {
        let mut backend = PdfBackend::new(Vec::new(), 1200).unwrap();
        backend
            .start_page(&[0; 10], 10 * 65536, 10 * 65536)
            .unwrap();
        backend.put_rule(65536, 65536).unwrap();
        backend.write_special(b"papersize=1in,2in").unwrap();
        backend.write_special(b"papersize=2in,3in").unwrap();
        backend.end_page().unwrap();
        backend
            .start_page(&[0; 10], 20 * 65536, 20 * 65536)
            .unwrap();
        backend.write_special(b"papersize=2in,3in").unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert_eq!(
            pdf.windows(b"/MediaBox [0 0 144 216]".len())
                .filter(|window| *window == b"/MediaBox [0 0 144 216]")
                .count(),
            2,
            "papersizeはTeXのmagを掛けず全pageへ継承する"
        );
        assert!(
            含む(&pdf, b"q\n1 0 0 1 0 60.044832 cm\n"),
            "第一page途中のspecialは既出content全体を新しい上端へ合わせる"
        );
    }

    #[test]
    fn 壊れたpapersizeと第二pageからの変更を黙殺しない() {
        let mut malformed = PdfBackend::new(Vec::new(), 1000).unwrap();
        malformed.start_page(&[0; 10], 0, 0).unwrap();
        assert!(matches!(
            malformed.write_special(b"papersize=10pt"),
            Err(PdfBackendError::Special(_))
        ));

        let mut late = PdfBackend::new(Vec::new(), 1000).unwrap();
        late.start_page(&[0; 10], 0, 0).unwrap();
        late.write_special(b"papersize=2in,3in").unwrap();
        late.end_page().unwrap();
        late.start_page(&[0; 10], 0, 0).unwrap();
        assert!(matches!(
            late.write_special(b"papersize=3in,4in"),
            Err(PdfBackendError::PaperSizeChangedAfterFirstPage {
                original: Some(_),
                requested: _,
            })
        ));
    }

    #[test]
    fn 不正な状態と座標overflowを報せる() {
        assert!(matches!(
            PdfBackend::new(Vec::new(), 0),
            Err(PdfBackendError::InvalidMagnification(0))
        ));
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        assert!(matches!(backend.pop(), Err(PdfBackendError::NoOpenPage)));
        backend.start_page(&[0; 10], 0, 0).unwrap();
        assert!(matches!(
            backend.pop(),
            Err(PdfBackendError::PositionStackUnderflow)
        ));
        assert!(matches!(
            backend.set_char(b'A', 65536),
            Err(PdfBackendError::NoCurrentFont)
        ));
        assert!(matches!(
            backend.start_page(&[0; 10], 0, 0),
            Err(PdfBackendError::PageAlreadyOpen)
        ));
        assert!(matches!(
            backend.set_font(9),
            Err(PdfBackendError::UndefinedFont(9))
        ));
        backend.define_font(font_definition(9, 65536)).unwrap();
        assert!(matches!(
            backend.define_font(font_definition(9, 65536)),
            Err(PdfBackendError::FontAlreadyDefined(9))
        ));
        backend.move_right(i32::MAX).unwrap();
        assert!(matches!(
            backend.move_right(1),
            Err(PdfBackendError::PositionOverflow)
        ));
        backend.push().unwrap();
        assert!(matches!(
            backend.end_page(),
            Err(PdfBackendError::UnbalancedPushes(1))
        ));
        backend.pop().unwrap();
        backend.end_page().unwrap();
    }

    #[test]
    fn 開いたpageを残してfinishしない() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend.start_page(&[0; 10], 0, 0).unwrap();
        assert!(matches!(
            backend.finish_with_target(),
            Err(PdfBackendError::PageStillOpen)
        ));
    }

    #[test]
    fn 負の内容寸法でも正のmedia_boxを作る() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend.start_page(&[0; 10], -65536, -131072).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();
        assert!(含む(&pdf, b"/MediaBox [0 0 144 144]"));
    }

    #[test]
    fn 負幅のboxからはみ出すruleまでmedia_boxへ収める() {
        let mut backend = PdfBackend::new(Vec::new(), 1000).unwrap();
        backend.start_page(&[0; 10], 10 * 65536, -65536).unwrap();
        backend.move_down(10 * 65536).unwrap();
        backend.put_rule(10 * 65536, 200 * 65536).unwrap();
        backend.end_page().unwrap();
        let (pdf, _) = backend.finish_with_target().unwrap();

        assert!(含む(&pdf, b"/MediaBox [0 0 343.252802 153.96264]"));
        assert!(含む(&pdf, b"72 72 199.252802 9.96264 re f"));
    }
}

//! 横組用JFMを、欧文8-bit TFMと別domainのfontとして保持する。

use crate::file_search::FileKind;
use crate::fonts::{logical_font_name_and_area, scale_fix_word, SizeIndicator};
use crate::format::{Dumpable, FormatError};
use crate::input::Scanner;
use crate::jfm::{Jfm, JfmAdjustment, JfmClassId, JfmDirection, JfmFixWord};
use crate::nodes::{DimensionOrder, HigherOrderDimension};
use crate::scaled::{xn_over_d, Scaled};
use crate::script_spacing::planner::{
    CompiledJfmPairSpacingTable, JfmMetricId, JfmPairSpacing, JfmPairSpacingRule, PlannerJfmClassId,
};
use crate::script_spacing::FixedGlue;

use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const TEX_FONT_AREA: &str = if cfg!(feature = "trip") {
    "./"
} else {
    "fonts/"
};

/// JFMの`lf < 2^15`と先頭7 wordを含むfile全体の上限。
const MAX_JFM_FILE_BYTES: u64 = ((1_u64 << 15) + 7) * 4;

/// 欧文`FontIndex`と数値domainを共有しない和文font handle。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct JapaneseFontIndex(u8);

impl JapaneseFontIndex {
    pub(crate) fn from_position(position: usize) -> Option<Self> {
        u8::try_from(position).ok().map(Self)
    }

    pub(crate) const fn position(self) -> usize {
        self.0 as usize
    }

    pub(crate) const fn dvi_font_number(self) -> u32 {
        // 8-bit fontは0..=254を使う。和文fontは追加の欧文font数に左右されない
        // 固定namespaceに置き、page間でDVI font numberが変わらないようにする。
        256 + self.0 as u32
    }
}

impl Dumpable for JapaneseFontIndex {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self(u8::undump(lines)?))
    }
}

/// JFM内の24-bit raw codeが何を意味するかをfont定義側で固定する。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JapaneseFontEncoding {
    /// PraTeXのCJK tokenのUnicode scalarをそのままJFM raw codeにする。
    Unicode,
}

impl Dumpable for JapaneseFontEncoding {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "Unicode")
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "Unicode" => Ok(Self::Unicode),
            _ => Err(FormatError::ParseError),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JapaneseGlyphMetrics {
    pub(crate) class: JfmClassId,
    pub(crate) width: Scaled,
    pub(crate) height: Scaled,
    pub(crate) depth: Scaled,
    pub(crate) italic: Scaled,
}

/// parse済みJFMと、指定sizeへ一度だけscaleしたmetric。
#[derive(Clone, Debug)]
pub(crate) struct JapaneseFontInfo {
    pub(crate) check: u32,
    pub(crate) size: Scaled,
    pub(crate) design_size: Scaled,
    pub(crate) name: Vec<u8>,
    pub(crate) area: Vec<u8>,
    pub(crate) encoding: JapaneseFontEncoding,
    raw_jfm: Vec<u8>,
    jfm: Jfm,
    widths: Vec<Scaled>,
    heights: Vec<Scaled>,
    depths: Vec<Scaled>,
    italics: Vec<Scaled>,
    metric_id: JfmMetricId,
    pair_spacing: CompiledJfmPairSpacingTable,
    zw: Scaled,
    zh: Scaled,
}

impl JapaneseFontInfo {
    pub(crate) fn from_resolved_file(
        logical_path: &Path,
        physical_path: &Path,
        size: SizeIndicator,
    ) -> Result<Self, JapaneseFontError> {
        let bytes = read_bounded_jfm(physical_path)?;
        let (name, area) =
            logical_font_name_and_area(logical_path).ok_or(JapaneseFontError::FileNotFound)?;
        Self::from_bytes(bytes, size, name, area, JapaneseFontEncoding::Unicode)
    }

    pub(crate) fn from_file(
        logical_path: &Path,
        size: SizeIndicator,
    ) -> Result<Self, JapaneseFontError> {
        let mut physical_path = match logical_path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => logical_path.to_path_buf(),
            _ => Path::new(TEX_FONT_AREA).join(logical_path),
        };
        physical_path.set_extension("tfm");
        Self::from_resolved_file(logical_path, &physical_path, size)
    }

    fn from_bytes(
        raw_jfm: Vec<u8>,
        size_indicator: SizeIndicator,
        name: Vec<u8>,
        area: Vec<u8>,
        encoding: JapaneseFontEncoding,
    ) -> Result<Self, JapaneseFontError> {
        if raw_jfm.len() as u64 > MAX_JFM_FILE_BYTES {
            return Err(JapaneseFontError::BadFormat);
        }
        let jfm = Jfm::parse(&raw_jfm).map_err(|_| JapaneseFontError::BadFormat)?;
        if jfm.direction != JfmDirection::Horizontal {
            return Err(JapaneseFontError::VerticalUnsupported);
        }
        let size = match size_indicator {
            SizeIndicator::AtSize(size) if size > 0 => size,
            SizeIndicator::Factor(factor) => xn_over_d(jfm.design_size, factor as i32, 1000)
                .map_err(|_| JapaneseFontError::BadFormat)?,
            SizeIndicator::AtSize(_) => return Err(JapaneseFontError::BadFormat),
        };
        let scale_table = |table: &[JfmFixWord]| -> Result<Vec<Scaled>, JapaneseFontError> {
            table
                .iter()
                .map(|word| scale_fix_word(word.raw(), size).ok_or(JapaneseFontError::BadFormat))
                .collect()
        };
        let widths = scale_table(&jfm.widths)?;
        let heights = scale_table(&jfm.heights)?;
        let depths = scale_table(&jfm.depths)?;
        let italics = scale_table(&jfm.italics)?;
        let pair_spacing = compile_pair_spacing(&jfm, size)?;
        let class_zero = jfm.class_of_raw_code(0);
        let zero_info = jfm.char_info(class_zero);
        let zw = widths[zero_info.width_index];
        let zh = heights[zero_info.height_index]
            .checked_add(depths[zero_info.depth_index])
            .ok_or(JapaneseFontError::BadFormat)?;
        Ok(Self {
            check: jfm.check,
            size,
            design_size: jfm.design_size,
            name,
            area,
            encoding,
            raw_jfm,
            jfm,
            widths,
            heights,
            depths,
            italics,
            metric_id: JfmMetricId::new(0),
            pair_spacing,
            zw,
            zh,
        })
    }

    pub(crate) fn metrics_for_unicode(&self, code_point: u32) -> JapaneseGlyphMetrics {
        debug_assert_eq!(self.encoding, JapaneseFontEncoding::Unicode);
        let class = self.jfm.class_of_raw_code(code_point);
        let info = self.jfm.char_info(class);
        JapaneseGlyphMetrics {
            class,
            width: self.widths[info.width_index],
            height: self.heights[info.height_index],
            depth: self.depths[info.depth_index],
            italic: self.italics[info.italic_index],
        }
    }

    pub(crate) const fn zw(&self) -> Scaled {
        self.zw
    }

    pub(crate) const fn zh(&self) -> Scaled {
        self.zh
    }

    pub(crate) fn bind_index(&mut self, index: JapaneseFontIndex) {
        let metric_id = JfmMetricId::new(index.position() as u32);
        self.metric_id = metric_id;
        self.pair_spacing.rebind_metric(metric_id);
    }

    pub(crate) const fn metric_id(&self) -> JfmMetricId {
        self.metric_id
    }

    pub(crate) const fn pair_spacing(&self) -> &CompiledJfmPairSpacingTable {
        &self.pair_spacing
    }

    pub(crate) fn same_identity(&self, logical_path: &Path, requested_size: Scaled) -> bool {
        let mut existing = PathBuf::from(crate::os_string_from_bytes(self.area.clone()));
        existing.push(crate::os_string_from_bytes(self.name.clone()));
        existing == logical_path && self.size == requested_size
    }
}

fn compile_pair_spacing(
    jfm: &Jfm,
    size: Scaled,
) -> Result<CompiledJfmPairSpacingTable, JapaneseFontError> {
    let class_count = jfm.class_count();
    let mut rules = Vec::new();
    for left in 0..class_count {
        for right in 0..class_count {
            let left_class = JfmClassId::from_number(left as u8);
            let right_class = JfmClassId::from_number(right as u8);
            let Some(adjustment) = jfm.adjustment_by_class(left_class, right_class) else {
                continue;
            };
            let spacing = match adjustment {
                JfmAdjustment::Glue(glue) => JfmPairSpacing::Glue(FixedGlue::from_parts(
                    scale_fix_word(glue.width.raw(), size).ok_or(JapaneseFontError::BadFormat)?,
                    HigherOrderDimension {
                        order: DimensionOrder::Normal,
                        value: scale_fix_word(glue.stretch.raw(), size)
                            .ok_or(JapaneseFontError::BadFormat)?,
                    },
                    HigherOrderDimension {
                        order: DimensionOrder::Normal,
                        value: scale_fix_word(glue.shrink.raw(), size)
                            .ok_or(JapaneseFontError::BadFormat)?,
                    },
                )),
                JfmAdjustment::Kern(kern) => JfmPairSpacing::Kern(
                    scale_fix_word(kern.raw(), size).ok_or(JapaneseFontError::BadFormat)?,
                ),
            };
            rules.push(JfmPairSpacingRule::new(
                PlannerJfmClassId::new(left as u8),
                PlannerJfmClassId::new(right as u8),
                spacing,
            ));
        }
    }
    CompiledJfmPairSpacingTable::compile(JfmMetricId::new(0), class_count, &rules)
        .map_err(|_| JapaneseFontError::BadFormat)
}

impl Dumpable for JapaneseFontInfo {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.raw_jfm.dump(target)?;
        self.size.dump(target)?;
        self.name.dump(target)?;
        self.area.dump(target)?;
        self.encoding.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let raw_jfm = Vec::<u8>::undump(lines)?;
        if raw_jfm.len() as u64 > MAX_JFM_FILE_BYTES {
            return Err(FormatError::ParseError);
        }
        let size = Scaled::undump(lines)?;
        let name = Vec::<u8>::undump(lines)?;
        let area = Vec::<u8>::undump(lines)?;
        let encoding = JapaneseFontEncoding::undump(lines)?;
        Self::from_bytes(raw_jfm, SizeIndicator::AtSize(size), name, area, encoding)
            .map_err(|_| FormatError::ParseError)
    }
}

pub(crate) fn load_japanese_font_info(
    logical_path: &Path,
    size: SizeIndicator,
    scanner: &mut Scanner,
) -> Result<JapaneseFontInfo, JapaneseFontError> {
    let mut direct_path = logical_path.to_path_buf();
    direct_path.set_extension("tfm");
    match JapaneseFontInfo::from_resolved_file(logical_path, &direct_path, size) {
        Ok(font) => return Ok(font),
        Err(
            error @ (JapaneseFontError::BadFormat
            | JapaneseFontError::VerticalUnsupported
            | JapaneseFontError::TooManyFonts),
        ) => return Err(error),
        Err(JapaneseFontError::FileNotFound) => {}
    }
    if let Ok(Some(physical_path)) = scanner.resolve_file_path(FileKind::Tfm, &direct_path) {
        match JapaneseFontInfo::from_resolved_file(logical_path, &physical_path, size) {
            Ok(font) => return Ok(font),
            Err(
                error @ (JapaneseFontError::BadFormat
                | JapaneseFontError::VerticalUnsupported
                | JapaneseFontError::TooManyFonts),
            ) => return Err(error),
            Err(JapaneseFontError::FileNotFound) => {}
        }
    }
    JapaneseFontInfo::from_file(logical_path, size)
}

fn read_bounded_jfm(path: &Path) -> Result<Vec<u8>, JapaneseFontError> {
    let mut file = File::open(path).map_err(|_| JapaneseFontError::FileNotFound)?;
    if file
        .metadata()
        .map_err(|_| JapaneseFontError::BadFormat)?
        .len()
        > MAX_JFM_FILE_BYTES
    {
        return Err(JapaneseFontError::BadFormat);
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_JFM_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| JapaneseFontError::BadFormat)?;
    if bytes.len() as u64 > MAX_JFM_FILE_BYTES {
        return Err(JapaneseFontError::BadFormat);
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JapaneseFontError {
    FileNotFound,
    BadFormat,
    VerticalUnsupported,
    TooManyFonts,
}

impl fmt::Display for JapaneseFontError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound => formatter.write_str("JFM file was not found"),
            Self::BadFormat => formatter.write_str("JFM file has an invalid format"),
            Self::VerticalUnsupported => {
                formatter.write_str("vertical JFM is not supported by this horizontal slice")
            }
            Self::TooManyFonts => formatter.write_str("too many Japanese fonts are loaded"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{JapaneseFontEncoding, JapaneseFontError, JapaneseFontIndex, JapaneseFontInfo};
    use crate::fonts::SizeIndicator;
    use crate::format::Dumpable;

    const ZERO: i32 = 0;
    const QUARTER: i32 = 0x0004_0000;
    const HALF: i32 = 0x0008_0000;
    const ONE: i32 = 0x0010_0000;

    fn char_type(character_code: u32, character_type: u8) -> [u8; 4] {
        [
            ((character_code >> 8) & 0xff) as u8,
            (character_code & 0xff) as u8,
            ((character_code >> 16) & 0xff) as u8,
            character_type,
        ]
    }

    fn synthetic_jfm(direction: u16) -> Vec<u8> {
        let char_types = [
            char_type(0, 0),
            char_type(0x003001, 1),
            char_type(0x01f600, 2),
        ];
        let char_infos = [[1, 0x11, 0, 0], [2, 0x10, 0, 0], [1, 0x11, 0, 0]];
        let widths = [ZERO, ONE, HALF];
        let heights = [ZERO, HALF];
        let depths = [ZERO, QUARTER];
        let italics = [ZERO];
        let params = [ZERO; 9];

        let nt = char_types.len();
        let lh = 2;
        let bc = 0;
        let ec = char_infos.len() - 1;
        let lf = 7
            + nt
            + lh
            + (ec - bc + 1)
            + widths.len()
            + heights.len()
            + depths.len()
            + italics.len()
            + params.len();
        let mut bytes = Vec::with_capacity(lf * 4);
        for value in [
            direction,
            nt as u16,
            lf as u16,
            lh as u16,
            bc as u16,
            ec as u16,
            widths.len() as u16,
            heights.len() as u16,
            depths.len() as u16,
            italics.len() as u16,
            0,
            0,
            0,
            params.len() as u16,
        ] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        bytes.extend_from_slice(&0x1234_5678_u32.to_be_bytes());
        bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());
        for table in [&char_types[..], &char_infos[..]] {
            for word in table {
                bytes.extend_from_slice(word);
            }
        }
        for table in [
            &widths[..],
            &heights[..],
            &depths[..],
            &italics[..],
            &params[..],
        ] {
            for value in table {
                bytes.extend_from_slice(&value.to_be_bytes());
            }
        }
        bytes
    }

    fn 横組font() -> JapaneseFontInfo {
        JapaneseFontInfo::from_bytes(
            synthetic_jfm(11),
            SizeIndicator::AtSize(10 * 65_536),
            b"synthetic".to_vec(),
            Vec::new(),
            JapaneseFontEncoding::Unicode,
        )
        .unwrap()
    }

    #[test]
    fn 横組jfmを指定寸法へ一度だけscaleする() {
        let font = 横組font();
        let punctuation = font.metrics_for_unicode(0x3001);
        assert_eq!(punctuation.class.number(), 1);
        assert_eq!(punctuation.width, 5 * 65_536);
        assert_eq!(punctuation.height, 5 * 65_536);
        assert_eq!(punctuation.depth, 0);
        assert_eq!(font.zw(), 10 * 65_536);
        assert_eq!(font.zh(), 7 * 65_536 + 32_768);
    }

    #[test]
    fn 和文fontは生jfmと指定寸法をformatで往復する() {
        let font = 横組font();
        let mut dump = Vec::new();
        font.dump(&mut dump).unwrap();
        let text = String::from_utf8(dump).unwrap();
        let restored = JapaneseFontInfo::undump(&mut text.lines()).unwrap();

        assert_eq!(restored.check, font.check);
        assert_eq!(restored.size, font.size);
        assert_eq!(restored.zw(), font.zw());
        assert_eq!(
            restored.metrics_for_unicode(0x1f600),
            font.metrics_for_unicode(0x1f600)
        );
    }

    #[test]
    fn 縦組jfmを横組fontとして選ばない() {
        let error = JapaneseFontInfo::from_bytes(
            synthetic_jfm(9),
            SizeIndicator::AtSize(10 * 65_536),
            b"vertical".to_vec(),
            Vec::new(),
            JapaneseFontEncoding::Unicode,
        )
        .unwrap_err();
        assert_eq!(error, JapaneseFontError::VerticalUnsupported);
    }

    #[test]
    fn dviの和文font番号は欧文font追加数に依存しない() {
        let index = JapaneseFontIndex::from_position(7).unwrap();
        assert_eq!(index.position(), 7);
        assert_eq!(index.dvi_font_number(), 263);
    }
}

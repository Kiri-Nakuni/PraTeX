use crate::format::{Dumpable, FormatError};

use std::io::Write;

const DUMP_HEADER: &str = "LanguageRegion/PraTeX-1";

/// PraTeX の組版規則が参照する言語・地域。
///
/// TeX の `\language`（ハイフネーション表の番号）とは独立している。
/// 明示値は書式ファイルと将来の ABI で共有する公開番号である。
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageRegion {
    Und = 0,
    Ja = 1,
    ZhHans = 2,
    ZhHant = 3,
    Ko = 4,
    Vi = 5,
}

impl LanguageRegion {
    pub const MIN_CODE: i32 = Self::Und as i32;
    pub const MAX_CODE: i32 = Self::Vi as i32;

    pub const fn code(self) -> u8 {
        self as u8
    }
}

impl Default for LanguageRegion {
    fn default() -> Self {
        Self::Und
    }
}

impl TryFrom<i32> for LanguageRegion {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Und),
            1 => Ok(Self::Ja),
            2 => Ok(Self::ZhHans),
            3 => Ok(Self::ZhHant),
            4 => Ok(Self::Ko),
            5 => Ok(Self::Vi),
            _ => Err(()),
        }
    }
}

impl Dumpable for LanguageRegion {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{DUMP_HEADER}")?;
        self.code().dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        if lines.next().ok_or(FormatError::IncompleteFile)? != DUMP_HEADER {
            return Err(FormatError::ParseError);
        }
        Self::try_from(i32::from(u8::undump(lines)?)).map_err(|_| FormatError::ParseError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 公開番号と内部幅を固定する() {
        assert_eq!(LanguageRegion::Und.code(), 0);
        assert_eq!(LanguageRegion::Ja.code(), 1);
        assert_eq!(LanguageRegion::ZhHans.code(), 2);
        assert_eq!(LanguageRegion::ZhHant.code(), 3);
        assert_eq!(LanguageRegion::Ko.code(), 4);
        assert_eq!(LanguageRegion::Vi.code(), 5);
        assert_eq!(std::mem::size_of::<LanguageRegion>(), 1);
    }

    #[test]
    fn 書式の範囲外を拒む() {
        for code in ["6", "255"] {
            let text = format!("{DUMP_HEADER}\n{code}\n");
            assert!(matches!(
                LanguageRegion::undump(&mut text.lines()),
                Err(FormatError::ParseError)
            ));
        }
    }

    #[test]
    fn 書式の全公開番号を往復する() {
        for expected in [
            LanguageRegion::Und,
            LanguageRegion::Ja,
            LanguageRegion::ZhHans,
            LanguageRegion::ZhHant,
            LanguageRegion::Ko,
            LanguageRegion::Vi,
        ] {
            let mut dumped = Vec::new();
            expected.dump(&mut dumped).unwrap();
            let text = String::from_utf8(dumped).unwrap();
            assert_eq!(LanguageRegion::undump(&mut text.lines()).unwrap(), expected);
        }
    }
}

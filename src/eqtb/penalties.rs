use crate::format::{Dumpable, FormatError};

use std::io::Write;

pub type PenaltyArray = Vec<i32>;

/// e-TeX の段落行間 penalty 配列を識別する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PenaltyArrayVariable {
    InterLine,
    Club,
    Widow,
    DisplayWidow,
}

impl PenaltyArrayVariable {
    pub const ALL: [Self; 4] = [Self::InterLine, Self::Club, Self::Widow, Self::DisplayWidow];

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::InterLine => 0,
            Self::Club => 1,
            Self::Widow => 2,
            Self::DisplayWidow => 3,
        }
    }

    pub const fn primitive_name(self) -> &'static [u8] {
        match self {
            Self::InterLine => b"interlinepenalties",
            Self::Club => b"clubpenalties",
            Self::Widow => b"widowpenalties",
            Self::DisplayWidow => b"displaywidowpenalties",
        }
    }
}

impl Dumpable for PenaltyArrayVariable {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        let name = match self {
            Self::InterLine => "InterLine",
            Self::Club => "Club",
            Self::Widow => "Widow",
            Self::DisplayWidow => "DisplayWidow",
        };
        writeln!(target, "{name}")
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "InterLine" => Ok(Self::InterLine),
            "Club" => Ok(Self::Club),
            "Widow" => Ok(Self::Widow),
            "DisplayWidow" => Ok(Self::DisplayWidow),
            _ => Err(FormatError::ParseError),
        }
    }
}

pub struct PenaltyArrayParameters {
    values: [PenaltyArray; 4],
}

impl PenaltyArrayParameters {
    pub fn new() -> Self {
        Self {
            values: std::array::from_fn(|_| PenaltyArray::new()),
        }
    }

    pub fn get(&self, variable: PenaltyArrayVariable) -> &PenaltyArray {
        &self.values[variable.index()]
    }

    pub fn set(&mut self, variable: PenaltyArrayVariable, value: PenaltyArray) -> PenaltyArray {
        std::mem::replace(&mut self.values[variable.index()], value)
    }

    /// 公開された内部整数照会を行う。正の添字が長さを越えた場合は末尾を反復する。
    pub fn query(&self, variable: PenaltyArrayVariable, index: i32) -> i32 {
        let values = self.get(variable);
        if index < 0 || values.is_empty() {
            0
        } else if index == 0 {
            values.len() as i32
        } else {
            values[(index as usize - 1).min(values.len() - 1)]
        }
    }

    /// 組版側から1始まりの位置を引く。reset状態は従来の単一parameterへ戻すため`None`。
    pub fn value_at(&self, variable: PenaltyArrayVariable, index: usize) -> Option<i32> {
        let values = self.get(variable);
        if values.is_empty() {
            None
        } else {
            let zero_based = index.checked_sub(1)?;
            Some(values[zero_based.min(values.len() - 1)])
        }
    }
}

impl Dumpable for PenaltyArrayParameters {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.values.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self {
            values: <[PenaltyArray; 4]>::undump(lines)?,
        })
    }
}

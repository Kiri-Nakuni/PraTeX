use crate::format::{Dumpable, FormatError};

use std::io::Write;

pub(crate) const MAX_ONE_BYTE_CODE: u32 = 0xFF;
pub const MAX_LATIN_UCS_CODE: u32 = 0x2E7F;
/// e-upTeX accepts this one-past value on the right-hand side of `\lccode`
/// and `\uccode`. It is a sentinel, not a tokenizable latin_ucs code point.
pub const MAX_LATIN_UCS_CASE_CODE: u32 = MAX_LATIN_UCS_CODE + 1;
pub(crate) const LATIN_UCS_TABLE_LEN: usize = MAX_LATIN_UCS_CODE as usize + 1 - 256;

/// Mask for indicating a math symbol of class 7 (Variable).
pub const VAR_CODE: u16 = 0x7000;

/// See 247.
pub struct CodeParameters {
    lc_codes: [i32; 256],
    uc_codes: [i32; 256],
    sf_codes: [i32; 256],
    math_codes: [i32; 256],
    del_codes: [i32; 256],
    /// 256..=U+2E7F。低位 256 個は上の TeX82 表を共有する。
    latin_ucs_lc_codes: Box<[i32]>,
    latin_ucs_uc_codes: Box<[i32]>,
    latin_ucs_sf_codes: Box<[i32]>,
}

impl CodeParameters {
    /// See 232. and 240.
    pub fn new() -> Self {
        let mut params = Self {
            lc_codes: [0; 256],
            uc_codes: [0; 256],
            sf_codes: [1000; 256],
            math_codes: [0; 256],
            del_codes: [-1; 256],
            latin_ucs_lc_codes: vec![0; LATIN_UCS_TABLE_LEN].into_boxed_slice(),
            latin_ucs_uc_codes: vec![0; LATIN_UCS_TABLE_LEN].into_boxed_slice(),
            latin_ucs_sf_codes: vec![1000; LATIN_UCS_TABLE_LEN].into_boxed_slice(),
        };

        for k in 0..256 {
            params.math_codes[k] = k as i32;
        }
        for k in b'0'..=b'9' {
            params.math_codes[k as usize] = k as i32 + VAR_CODE as i32;
        }

        for k in b'A'..=b'Z' {
            params.math_codes[k as usize] = k as i32 + VAR_CODE as i32 + 0x100;
            params.math_codes[(k + b'a' - b'A') as usize] =
                (k + b'a' - b'A') as i32 + VAR_CODE as i32 + 0x100;
            params.lc_codes[k as usize] = (k + b'a' - b'A') as i32;
            params.lc_codes[(k + b'a' - b'A') as usize] = (k + b'a' - b'A') as i32;
            params.uc_codes[k as usize] = k as i32;
            params.uc_codes[(k + b'a' - b'A') as usize] = k as i32;
            params.sf_codes[k as usize] = 999;
        }

        params.del_codes[b'.' as usize] = 0;
        params
    }

    fn index(&self, index: CodeVariable) -> &i32 {
        assert!(index.is_valid(), "code table index is out of range");
        match index {
            CodeVariable::LcCode(n) if n >= 256 => &self.latin_ucs_lc_codes[n - 256],
            CodeVariable::UcCode(n) if n >= 256 => &self.latin_ucs_uc_codes[n - 256],
            CodeVariable::SfCode(n) if n >= 256 => &self.latin_ucs_sf_codes[n - 256],
            CodeVariable::LcCode(n) => &self.lc_codes[n],
            CodeVariable::UcCode(n) => &self.uc_codes[n],
            CodeVariable::SfCode(n) => &self.sf_codes[n],
            CodeVariable::MathCode(n) => &self.math_codes[n],
            CodeVariable::DelCode(n) => &self.del_codes[n],
        }
    }

    fn index_mut(&mut self, index: CodeVariable) -> &mut i32 {
        assert!(index.is_valid(), "code table index is out of range");
        match index {
            CodeVariable::LcCode(n) if n >= 256 => &mut self.latin_ucs_lc_codes[n - 256],
            CodeVariable::UcCode(n) if n >= 256 => &mut self.latin_ucs_uc_codes[n - 256],
            CodeVariable::SfCode(n) if n >= 256 => &mut self.latin_ucs_sf_codes[n - 256],
            CodeVariable::LcCode(n) => &mut self.lc_codes[n],
            CodeVariable::UcCode(n) => &mut self.uc_codes[n],
            CodeVariable::SfCode(n) => &mut self.sf_codes[n],
            CodeVariable::MathCode(n) => &mut self.math_codes[n],
            CodeVariable::DelCode(n) => &mut self.del_codes[n],
        }
    }

    pub fn get(&self, code_var: CodeVariable) -> &i32 {
        self.index(code_var)
    }

    pub fn set(&mut self, code_var: CodeVariable, new_value: i32) -> i32 {
        assert!(
            code_var.accepts_value(new_value),
            "code table value is out of range"
        );
        let prev_value = *self.index_mut(code_var);
        *self.index_mut(code_var) = new_value;
        prev_value
    }
}

/// Specifies a code variable in the Eqtb.
/// Note that we include the delimiter codes here as well
/// that are stored separately from the others in TeX82
/// See 230. and 236.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodeVariable {
    LcCode(usize),
    UcCode(usize),
    SfCode(usize),
    MathCode(usize),
    DelCode(usize),
}

impl CodeVariable {
    pub(crate) fn is_valid(self) -> bool {
        match self {
            Self::LcCode(n) | Self::UcCode(n) | Self::SfCode(n) => {
                n <= MAX_LATIN_UCS_CODE as usize
            }
            Self::MathCode(n) | Self::DelCode(n) => n <= u8::MAX as usize,
        }
    }

    fn accepts_value(self, value: i32) -> bool {
        match self {
            Self::LcCode(_) | Self::UcCode(_) => {
                (0..=MAX_LATIN_UCS_CASE_CODE as i32).contains(&value)
            }
            Self::SfCode(_) => (0..=0o77777).contains(&value),
            // The legacy tables retain their existing range handling. In
            // particular, the default delimiter code is -1.
            Self::MathCode(_) | Self::DelCode(_) => true,
        }
    }

    /// The name of the code variable without preceding escape character.
    /// See 235. and 242.
    pub fn to_string(self) -> Vec<u8> {
        match self {
            Self::LcCode(n) => format!("lccode{}", n).as_bytes().to_vec(),
            Self::UcCode(n) => format!("uccode{}", n).as_bytes().to_vec(),
            Self::SfCode(n) => format!("sfcode{}", n).as_bytes().to_vec(),
            Self::MathCode(n) => format!("mathcode{}", n).as_bytes().to_vec(),
            Self::DelCode(n) => format!("delcode{}", n).as_bytes().to_vec(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeType {
    LcCode,
    UcCode,
    SfCode,
    MathCode,
    DelCode,
}

impl CodeType {
    pub fn to_variable(self, n: usize) -> CodeVariable {
        let variable = match self {
            Self::LcCode => CodeVariable::LcCode(n),
            Self::UcCode => CodeVariable::UcCode(n),
            Self::SfCode => CodeVariable::SfCode(n),
            Self::MathCode => CodeVariable::MathCode(n),
            Self::DelCode => CodeVariable::DelCode(n),
        };
        assert!(variable.is_valid(), "code table index is out of range");
        variable
    }

    /// The type of the code without preceding escape character.
    /// See 235. and 242.
    pub fn as_str(&self) -> &[u8] {
        match self {
            Self::LcCode => b"lccode",
            Self::UcCode => b"uccode",
            Self::SfCode => b"sfcode",
            Self::MathCode => b"mathcode",
            Self::DelCode => b"delcode",
        }
    }
}

impl Dumpable for CodeVariable {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        if !self.is_valid() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "code table index is out of range",
            ));
        }
        match self {
            Self::LcCode(n) => {
                writeln!(target, "LcCode")?;
                n.dump(target)?;
            }
            Self::UcCode(n) => {
                writeln!(target, "UcCode")?;
                n.dump(target)?;
            }
            Self::SfCode(n) => {
                writeln!(target, "SfCode")?;
                n.dump(target)?;
            }
            Self::MathCode(n) => {
                writeln!(target, "MathCode")?;
                n.dump(target)?;
            }
            Self::DelCode(n) => {
                writeln!(target, "DelCode")?;
                n.dump(target)?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        let variable = match variant {
            "LcCode" => {
                let n = usize::undump(lines)?;
                Self::LcCode(n)
            }
            "UcCode" => {
                let n = usize::undump(lines)?;
                Self::UcCode(n)
            }
            "SfCode" => {
                let n = usize::undump(lines)?;
                Self::SfCode(n)
            }
            "MathCode" => {
                let n = usize::undump(lines)?;
                Self::MathCode(n)
            }
            "DelCode" => {
                let n = usize::undump(lines)?;
                Self::DelCode(n)
            }
            _ => return Err(FormatError::ParseError),
        };
        if variable.is_valid() {
            Ok(variable)
        } else {
            Err(FormatError::ParseError)
        }
    }
}

impl Dumpable for CodeParameters {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.lc_codes.dump(target)?;
        self.uc_codes.dump(target)?;
        self.sf_codes.dump(target)?;
        self.math_codes.dump(target)?;
        self.del_codes.dump(target)?;
        LATIN_UCS_TABLE_LEN.dump(target)?;
        for table in [
            &self.latin_ucs_lc_codes,
            &self.latin_ucs_uc_codes,
            &self.latin_ucs_sf_codes,
        ] {
            for &value in table.iter() {
                value.dump(target)?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let lc_codes: [i32; 256] = Dumpable::undump(lines)?;
        let uc_codes: [i32; 256] = Dumpable::undump(lines)?;
        let sf_codes: [i32; 256] = Dumpable::undump(lines)?;
        let math_codes = Dumpable::undump(lines)?;
        let del_codes = Dumpable::undump(lines)?;
        if lc_codes
            .iter()
            .any(|&value| !CodeVariable::LcCode(0).accepts_value(value))
            || uc_codes
                .iter()
                .any(|&value| !CodeVariable::UcCode(0).accepts_value(value))
            || sf_codes
                .iter()
                .any(|&value| !CodeVariable::SfCode(0).accepts_value(value))
        {
            return Err(FormatError::ParseError);
        }
        if usize::undump(lines)? != LATIN_UCS_TABLE_LEN {
            return Err(FormatError::ParseError);
        }
        let mut read_extended = || -> Result<Box<[i32]>, FormatError> {
            let mut values = vec![0; LATIN_UCS_TABLE_LEN];
            for value in &mut values {
                *value = i32::undump(lines)?;
            }
            Ok(values.into_boxed_slice())
        };
        let latin_ucs_lc_codes = read_extended()?;
        let latin_ucs_uc_codes = read_extended()?;
        let latin_ucs_sf_codes = read_extended()?;
        if latin_ucs_lc_codes
            .iter()
            .any(|&value| !CodeVariable::LcCode(256).accepts_value(value))
            || latin_ucs_uc_codes
                .iter()
                .any(|&value| !CodeVariable::UcCode(256).accepts_value(value))
            || latin_ucs_sf_codes
                .iter()
                .any(|&value| !CodeVariable::SfCode(256).accepts_value(value))
        {
            return Err(FormatError::ParseError);
        }
        Ok(Self {
            lc_codes,
            uc_codes,
            sf_codes,
            math_codes,
            del_codes,
            latin_ucs_lc_codes,
            latin_ucs_uc_codes,
            latin_ucs_sf_codes,
        })
    }
}

#[cfg(test)]
mod latin_ucs_tests {
    use super::*;

    fn 壊した表を読む(line: usize, value: i32) -> Result<CodeParameters, FormatError> {
        let mut bytes = Vec::new();
        CodeParameters::new().dump(&mut bytes).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let mut lines: Vec<_> = text.lines().map(str::to_owned).collect();
        lines[line] = value.to_string();
        CodeParameters::undump(&mut lines.join("\n").lines())
    }

    #[test]
    fn case表はu二e八十sentinelだけを上端として受理する() {
        assert!(CodeVariable::LcCode(0).accepts_value(MAX_LATIN_UCS_CASE_CODE as i32));
        assert!(!CodeVariable::LcCode(0).accepts_value(-1));
        assert!(!CodeVariable::UcCode(0).accepts_value(MAX_LATIN_UCS_CASE_CODE as i32 + 1));
    }

    #[test]
    fn format中の壊れたunicode欧文code値を拒否する() {
        // Five 256-entry legacy tables and the extended-table length precede
        // the first extended lccode value.
        let first_extended_lc = 5 * 256 + 1;
        let first_extended_uc = first_extended_lc + LATIN_UCS_TABLE_LEN;
        let first_extended_sf = first_extended_uc + LATIN_UCS_TABLE_LEN;

        assert!(matches!(
            壊した表を読む(first_extended_lc, -1),
            Err(FormatError::ParseError)
        ));
        assert!(matches!(
            壊した表を読む(
                first_extended_uc,
                MAX_LATIN_UCS_CASE_CODE as i32 + 1
            ),
            Err(FormatError::ParseError)
        ));
        assert!(matches!(
            壊した表を読む(first_extended_sf, 0o100000),
            Err(FormatError::ParseError)
        ));
    }
}

impl Dumpable for CodeType {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::LcCode => {
                writeln!(target, "LcCode")?;
            }
            Self::UcCode => {
                writeln!(target, "UcCode")?;
            }
            Self::SfCode => {
                writeln!(target, "SfCode")?;
            }
            Self::MathCode => {
                writeln!(target, "MathCode")?;
            }
            Self::DelCode => {
                writeln!(target, "DelCode")?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "LcCode" => Ok(Self::LcCode),
            "UcCode" => Ok(Self::UcCode),
            "SfCode" => Ok(Self::SfCode),
            "MathCode" => Ok(Self::MathCode),
            "DelCode" => Ok(Self::DelCode),
            _ => Err(FormatError::ParseError),
        }
    }
}

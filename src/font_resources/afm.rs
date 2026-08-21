//! Adobe Font Metrics (AFM) のうち、Type 1 PDF 埋め込みに必要な部分を読む。
//!
//! Adobe Technical Note #5004, AFM File Format Specification 4.1 の
//! 3, 7, 8 節を仕様として実装している。未知のキーは、仕様の拡張規則に従って無視する。

use std::collections::BTreeMap;
use std::fmt;

/// AFM の `number` を 10^-6 単位で保持する固定小数。
///
/// AFM は整数または小数を許すが、ここでは浮動小数を介さずに読み取る。小数部が
/// 6 桁を超える場合は、余分な桁がすべて 0 のときだけ受け入れる。
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct AfmNumber(i64);

impl AfmNumber {
    pub(crate) const SCALE: i64 = 1_000_000;
    pub(crate) const ZERO: Self = Self(0);

    pub(crate) const fn from_scaled(scaled: i64) -> Self {
        Self(scaled)
    }

    pub(crate) const fn scaled(self) -> i64 {
        self.0
    }

    pub(crate) fn checked_from_integer(value: i64) -> Option<Self> {
        value.checked_mul(Self::SCALE).map(Self)
    }
}

impl fmt::Display for AfmNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = i128::from(self.0);
        let negative = value < 0;
        let magnitude = if negative { -value } else { value };
        let whole = magnitude / i128::from(Self::SCALE);
        let fraction = magnitude % i128::from(Self::SCALE);

        if negative {
            formatter.write_str("-")?;
        }
        write!(formatter, "{whole}")?;
        if fraction != 0 {
            let digits = format!("{fraction:06}");
            write!(formatter, ".{}", digits.trim_end_matches('0'))?;
        }
        Ok(())
    }
}

/// PDF の FontDescriptor を構成するための AFM 大域値。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AfmDescriptor {
    pub(crate) font_name: String,
    pub(crate) encoding_scheme: Option<String>,
    pub(crate) font_bbox: [AfmNumber; 4],
    pub(crate) italic_angle: AfmNumber,
    pub(crate) is_fixed_pitch: bool,
    pub(crate) cap_height: AfmNumber,
    pub(crate) x_height: Option<AfmNumber>,
    pub(crate) ascender: AfmNumber,
    pub(crate) descender: AfmNumber,
    /// Older public AFM files (including AMSFonts Computer Modern) omit stem
    /// widths even though a PDF FontDescriptor ultimately needs StemV.
    pub(crate) std_vw: Option<AfmNumber>,
    pub(crate) std_hw: Option<AfmNumber>,
}

/// 一文字分の横書きメトリクス。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AfmGlyphMetric {
    /// 単一バイト符号を持つ場合だけ値を持つ。`C -1` および 256 以上は `None`。
    pub(crate) code: Option<u8>,
    pub(crate) name: Option<String>,
    pub(crate) width_x: AfmNumber,
}

/// Type 1 PDF 出力で参照する AFM 情報。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AfmFont {
    pub(crate) descriptor: AfmDescriptor,
    pub(crate) metrics_by_name: BTreeMap<String, AfmGlyphMetric>,
    pub(crate) metrics_by_code: BTreeMap<u8, AfmGlyphMetric>,
}

impl AfmFont {
    pub(crate) fn parse(input: &[u8]) -> Result<Self, AfmParseError> {
        parse_afm(input)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AfmParseError {
    InvalidUtf8 { valid_up_to: usize },
    MissingStartFontMetrics,
    MissingEndFontMetrics,
    UnexpectedDataAfterEnd { line: usize },
    InvalidValue { line: usize, key: String },
    NumberOverflow { line: usize, key: String },
    NumberTooPrecise { line: usize, key: String },
    DuplicateDescriptorField { line: usize, key: &'static str },
    MissingDescriptorField(&'static str),
    DuplicateCharMetricsSection { line: usize },
    UnexpectedEndCharMetrics { line: usize },
    UnterminatedCharMetrics,
    CharMetricsCountMismatch { declared: usize, actual: usize },
    DuplicateCharacterField { line: usize, key: &'static str },
    MissingCharacterField { line: usize, key: &'static str },
    DuplicateGlyphName { line: usize, name: String },
    DuplicateCharacterCode { line: usize, code: u8 },
}

impl fmt::Display for AfmParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 { valid_up_to } => {
                write!(formatter, "AFM is not UTF-8 at byte {valid_up_to}")
            }
            Self::MissingStartFontMetrics => {
                formatter.write_str("AFM does not begin with StartFontMetrics")
            }
            Self::MissingEndFontMetrics => {
                formatter.write_str("AFM does not end with EndFontMetrics")
            }
            Self::UnexpectedDataAfterEnd { line } => {
                write!(
                    formatter,
                    "AFM has data after EndFontMetrics on line {line}"
                )
            }
            Self::InvalidValue { line, key } => {
                write!(formatter, "invalid {key} value on AFM line {line}")
            }
            Self::NumberOverflow { line, key } => {
                write!(formatter, "{key} overflows on AFM line {line}")
            }
            Self::NumberTooPrecise { line, key } => {
                write!(
                    formatter,
                    "{key} is more precise than 10^-6 on AFM line {line}"
                )
            }
            Self::DuplicateDescriptorField { line, key } => {
                write!(formatter, "duplicate {key} on AFM line {line}")
            }
            Self::MissingDescriptorField(key) => {
                write!(formatter, "AFM is missing descriptor field {key}")
            }
            Self::DuplicateCharMetricsSection { line } => {
                write!(formatter, "duplicate StartCharMetrics on AFM line {line}")
            }
            Self::UnexpectedEndCharMetrics { line } => {
                write!(formatter, "unexpected EndCharMetrics on AFM line {line}")
            }
            Self::UnterminatedCharMetrics => {
                formatter.write_str("AFM character metrics section is not terminated")
            }
            Self::CharMetricsCountMismatch { declared, actual } => write!(
                formatter,
                "AFM declares {declared} character metrics but contains {actual}"
            ),
            Self::DuplicateCharacterField { line, key } => {
                write!(
                    formatter,
                    "duplicate character field {key} on AFM line {line}"
                )
            }
            Self::MissingCharacterField { line, key } => {
                write!(
                    formatter,
                    "missing character field {key} on AFM line {line}"
                )
            }
            Self::DuplicateGlyphName { line, name } => {
                write!(formatter, "duplicate glyph name {name} on AFM line {line}")
            }
            Self::DuplicateCharacterCode { line, code } => {
                write!(
                    formatter,
                    "duplicate character code {code} on AFM line {line}"
                )
            }
        }
    }
}

impl std::error::Error for AfmParseError {}

#[derive(Default)]
struct DescriptorBuilder {
    font_name: Option<String>,
    encoding_scheme: Option<String>,
    font_bbox: Option<[AfmNumber; 4]>,
    italic_angle: Option<AfmNumber>,
    is_fixed_pitch: Option<bool>,
    cap_height: Option<AfmNumber>,
    x_height: Option<AfmNumber>,
    ascender: Option<AfmNumber>,
    descender: Option<AfmNumber>,
    std_vw: Option<AfmNumber>,
    std_hw: Option<AfmNumber>,
}

impl DescriptorBuilder {
    fn finish(self) -> Result<AfmDescriptor, AfmParseError> {
        Ok(AfmDescriptor {
            font_name: required(self.font_name, "FontName")?,
            encoding_scheme: self.encoding_scheme,
            font_bbox: required(self.font_bbox, "FontBBox")?,
            italic_angle: required(self.italic_angle, "ItalicAngle")?,
            is_fixed_pitch: required(self.is_fixed_pitch, "IsFixedPitch")?,
            cap_height: required(self.cap_height, "CapHeight")?,
            x_height: self.x_height,
            ascender: required(self.ascender, "Ascender")?,
            descender: required(self.descender, "Descender")?,
            std_vw: self.std_vw,
            std_hw: self.std_hw,
        })
    }
}

fn required<T>(value: Option<T>, key: &'static str) -> Result<T, AfmParseError> {
    value.ok_or(AfmParseError::MissingDescriptorField(key))
}

/// AFM を読み、Type 1 PDF 埋め込みに必要な記述子と文字幅を返す。
pub(crate) fn parse_afm(input: &[u8]) -> Result<AfmFont, AfmParseError> {
    let text = std::str::from_utf8(input).map_err(|error| AfmParseError::InvalidUtf8 {
        valid_up_to: error.valid_up_to(),
    })?;
    let mut lines = text.split('\n').enumerate();
    let (_, first_line) = lines.next().ok_or(AfmParseError::MissingStartFontMetrics)?;
    parse_start_line(trim_line(first_line))?;

    let mut descriptor = DescriptorBuilder::default();
    let mut metrics_by_name: BTreeMap<String, AfmGlyphMetric> = BTreeMap::new();
    let mut metrics_by_code: BTreeMap<u8, AfmGlyphMetric> = BTreeMap::new();
    let mut char_metrics_seen = false;
    let mut char_metrics_declared = 0usize;
    let mut char_metrics_actual = 0usize;
    let mut in_char_metrics = false;
    let mut ended = false;

    for (index, raw_line) in lines {
        let line_number = index + 1;
        let line = trim_line(raw_line);
        if line.is_empty() {
            continue;
        }
        if ended {
            return Err(AfmParseError::UnexpectedDataAfterEnd { line: line_number });
        }

        if in_char_metrics {
            if line == "EndCharMetrics" {
                if char_metrics_actual != char_metrics_declared {
                    return Err(AfmParseError::CharMetricsCountMismatch {
                        declared: char_metrics_declared,
                        actual: char_metrics_actual,
                    });
                }
                in_char_metrics = false;
                continue;
            }
            if first_key(line) == "Comment" {
                continue;
            }
            if first_key(line) == "EndFontMetrics" {
                return Err(AfmParseError::UnterminatedCharMetrics);
            }

            char_metrics_actual = char_metrics_actual.checked_add(1).ok_or_else(|| {
                AfmParseError::NumberOverflow {
                    line: line_number,
                    key: "StartCharMetrics".to_owned(),
                }
            })?;
            let metric = parse_character_metric(line, line_number)?;
            if let Some(name) = &metric.name {
                // AFM 4.1 permits the same glyph to appear at more than one
                // encoded position.  Keep the first name lookup when its
                // advance is identical, but never hide conflicting metrics.
                if let Some(existing) = metrics_by_name.get(name) {
                    if existing.width_x != metric.width_x {
                        return Err(AfmParseError::DuplicateGlyphName {
                            line: line_number,
                            name: name.clone(),
                        });
                    }
                }
            }
            if let Some(code) = metric.code {
                if metrics_by_code.contains_key(&code) {
                    return Err(AfmParseError::DuplicateCharacterCode {
                        line: line_number,
                        code,
                    });
                }
            }
            if let Some(name) = &metric.name {
                if !metrics_by_name.contains_key(name) {
                    metrics_by_name.insert(name.clone(), metric.clone());
                }
            }
            if let Some(code) = metric.code {
                metrics_by_code.insert(code, metric);
            }
            continue;
        }

        let (key, value) = split_key_value(line);
        match key {
            "EndFontMetrics" => {
                require_no_value(value, line_number, key)?;
                ended = true;
            }
            "StartCharMetrics" => {
                if char_metrics_seen {
                    return Err(AfmParseError::DuplicateCharMetricsSection { line: line_number });
                }
                char_metrics_declared = parse_usize(value, line_number, key)?;
                char_metrics_actual = 0;
                char_metrics_seen = true;
                in_char_metrics = true;
            }
            "EndCharMetrics" => {
                return Err(AfmParseError::UnexpectedEndCharMetrics { line: line_number });
            }
            "FontName" => set_string(&mut descriptor.font_name, value, line_number, "FontName")?,
            "EncodingScheme" => set_string(
                &mut descriptor.encoding_scheme,
                value,
                line_number,
                "EncodingScheme",
            )?,
            "FontBBox" => set_bbox(&mut descriptor.font_bbox, value, line_number, "FontBBox")?,
            "ItalicAngle" => set_number(
                &mut descriptor.italic_angle,
                value,
                line_number,
                "ItalicAngle",
            )?,
            "IsFixedPitch" => set_boolean(
                &mut descriptor.is_fixed_pitch,
                value,
                line_number,
                "IsFixedPitch",
            )?,
            "CapHeight" => set_number(&mut descriptor.cap_height, value, line_number, "CapHeight")?,
            "XHeight" => set_number(&mut descriptor.x_height, value, line_number, "XHeight")?,
            "Ascender" => set_number(&mut descriptor.ascender, value, line_number, "Ascender")?,
            "Descender" => set_number(&mut descriptor.descender, value, line_number, "Descender")?,
            "StdVW" => set_number(&mut descriptor.std_vw, value, line_number, "StdVW")?,
            "StdHW" => set_number(&mut descriptor.std_hw, value, line_number, "StdHW")?,
            // AFM は未認識のキーを無視できる拡張形式である。
            _ => {}
        }
    }

    if in_char_metrics {
        return Err(AfmParseError::UnterminatedCharMetrics);
    }
    if !ended {
        return Err(AfmParseError::MissingEndFontMetrics);
    }

    Ok(AfmFont {
        descriptor: descriptor.finish()?,
        metrics_by_name,
        metrics_by_code,
    })
}

fn trim_line(line: &str) -> &str {
    line.trim_matches(|character: char| character.is_ascii_whitespace())
}

fn first_key(line: &str) -> &str {
    split_key_value(line).0
}

fn split_key_value(line: &str) -> (&str, &str) {
    match line.find(|character: char| character.is_ascii_whitespace()) {
        Some(index) => (&line[..index], trim_line(&line[index..])),
        None => (line, ""),
    }
}

fn parse_start_line(line: &str) -> Result<(), AfmParseError> {
    let (key, version) = split_key_value(line);
    if key != "StartFontMetrics" {
        return Err(AfmParseError::MissingStartFontMetrics);
    }
    parse_number(version, 1, "StartFontMetrics").map(|_| ())
}

fn require_no_value(value: &str, line: usize, key: &str) -> Result<(), AfmParseError> {
    if value.is_empty() {
        Ok(())
    } else {
        Err(AfmParseError::InvalidValue {
            line,
            key: key.to_owned(),
        })
    }
}

fn single_token<'a>(value: &'a str, line: usize, key: &str) -> Result<&'a str, AfmParseError> {
    let mut tokens = value.split_ascii_whitespace();
    let token = tokens.next().ok_or_else(|| AfmParseError::InvalidValue {
        line,
        key: key.to_owned(),
    })?;
    if tokens.next().is_some() {
        return Err(AfmParseError::InvalidValue {
            line,
            key: key.to_owned(),
        });
    }
    Ok(token)
}

fn set_string(
    slot: &mut Option<String>,
    value: &str,
    line: usize,
    key: &'static str,
) -> Result<(), AfmParseError> {
    if slot.is_some() {
        return Err(AfmParseError::DuplicateDescriptorField { line, key });
    }
    // FontName is an AFM name, not an arbitrary line.  Keeping it to one token
    // also prevents later PDF-name serialization from inheriting whitespace.
    *slot = Some(single_token(value, line, key)?.to_owned());
    Ok(())
}

fn set_number(
    slot: &mut Option<AfmNumber>,
    value: &str,
    line: usize,
    key: &'static str,
) -> Result<(), AfmParseError> {
    if slot.is_some() {
        return Err(AfmParseError::DuplicateDescriptorField { line, key });
    }
    *slot = Some(parse_number(value, line, key)?);
    Ok(())
}

fn set_boolean(
    slot: &mut Option<bool>,
    value: &str,
    line: usize,
    key: &'static str,
) -> Result<(), AfmParseError> {
    if slot.is_some() {
        return Err(AfmParseError::DuplicateDescriptorField { line, key });
    }
    let token = single_token(value, line, key)?;
    *slot = Some(match token {
        "true" => true,
        "false" => false,
        _ => {
            return Err(AfmParseError::InvalidValue {
                line,
                key: key.to_owned(),
            })
        }
    });
    Ok(())
}

fn set_bbox(
    slot: &mut Option<[AfmNumber; 4]>,
    value: &str,
    line: usize,
    key: &'static str,
) -> Result<(), AfmParseError> {
    if slot.is_some() {
        return Err(AfmParseError::DuplicateDescriptorField { line, key });
    }
    let mut tokens = value.split_ascii_whitespace();
    let mut numbers = [AfmNumber::ZERO; 4];
    for number in &mut numbers {
        let token = tokens.next().ok_or_else(|| AfmParseError::InvalidValue {
            line,
            key: key.to_owned(),
        })?;
        *number = parse_number_token(token, line, key)?;
    }
    if tokens.next().is_some() {
        return Err(AfmParseError::InvalidValue {
            line,
            key: key.to_owned(),
        });
    }
    *slot = Some(numbers);
    Ok(())
}

fn parse_character_metric(line: &str, line_number: usize) -> Result<AfmGlyphMetric, AfmParseError> {
    let mut source_code = None;
    let mut width_x = None;
    let mut name = None;

    for field in line
        .split(';')
        .map(trim_line)
        .filter(|field| !field.is_empty())
    {
        let (key, value) = split_key_value(field);
        match key {
            "C" => {
                if source_code.is_some() {
                    return Err(AfmParseError::DuplicateCharacterField {
                        line: line_number,
                        key: "C/CH",
                    });
                }
                let code = parse_i64(value, line_number, key)?;
                if code < -1 {
                    return Err(AfmParseError::InvalidValue {
                        line: line_number,
                        key: key.to_owned(),
                    });
                }
                source_code = Some(u8::try_from(code).ok());
            }
            "CH" => {
                if source_code.is_some() {
                    return Err(AfmParseError::DuplicateCharacterField {
                        line: line_number,
                        key: "C/CH",
                    });
                }
                source_code = Some(parse_hex_code(value, line_number, key)?);
            }
            "WX" | "W0X" => {
                if width_x.is_some() {
                    return Err(AfmParseError::DuplicateCharacterField {
                        line: line_number,
                        key: "WX/W0X",
                    });
                }
                width_x = Some(parse_number(value, line_number, key)?);
            }
            "N" => {
                if name.is_some() {
                    return Err(AfmParseError::DuplicateCharacterField {
                        line: line_number,
                        key: "N",
                    });
                }
                name = Some(single_token(value, line_number, key)?.to_owned());
            }
            _ => {}
        }
    }

    let code = source_code.ok_or(AfmParseError::MissingCharacterField {
        line: line_number,
        key: "C/CH",
    })?;
    let width_x = width_x.ok_or(AfmParseError::MissingCharacterField {
        line: line_number,
        key: "WX/W0X",
    })?;
    Ok(AfmGlyphMetric {
        code,
        name,
        width_x,
    })
}

fn parse_hex_code(value: &str, line: usize, key: &str) -> Result<Option<u8>, AfmParseError> {
    let token = single_token(value, line, key)?;
    let digits = token
        .strip_prefix('<')
        .and_then(|rest| rest.strip_suffix('>'))
        .filter(|digits| !digits.is_empty())
        .ok_or_else(|| AfmParseError::InvalidValue {
            line,
            key: key.to_owned(),
        })?;
    let mut code = 0u64;
    for digit in digits.bytes() {
        let value = match digit {
            b'0'..=b'9' => u64::from(digit - b'0'),
            b'a'..=b'f' => u64::from(digit - b'a' + 10),
            b'A'..=b'F' => u64::from(digit - b'A' + 10),
            _ => {
                return Err(AfmParseError::InvalidValue {
                    line,
                    key: key.to_owned(),
                })
            }
        };
        code = code
            .checked_mul(16)
            .and_then(|number| number.checked_add(value))
            .ok_or_else(|| AfmParseError::NumberOverflow {
                line,
                key: key.to_owned(),
            })?;
    }
    Ok(u8::try_from(code).ok())
}

fn parse_number(value: &str, line: usize, key: &str) -> Result<AfmNumber, AfmParseError> {
    let token = single_token(value, line, key)?;
    parse_number_token(token, line, key)
}

fn parse_number_token(token: &str, line: usize, key: &str) -> Result<AfmNumber, AfmParseError> {
    parse_fixed_decimal(token).map_err(|error| match error {
        NumberError::Invalid => AfmParseError::InvalidValue {
            line,
            key: key.to_owned(),
        },
        NumberError::Overflow => AfmParseError::NumberOverflow {
            line,
            key: key.to_owned(),
        },
        NumberError::TooPrecise => AfmParseError::NumberTooPrecise {
            line,
            key: key.to_owned(),
        },
    })
}

fn parse_usize(value: &str, line: usize, key: &str) -> Result<usize, AfmParseError> {
    let token = single_token(value, line, key)?;
    let number = parse_unsigned_integer(token).map_err(|error| match error {
        NumberError::Invalid | NumberError::TooPrecise => AfmParseError::InvalidValue {
            line,
            key: key.to_owned(),
        },
        NumberError::Overflow => AfmParseError::NumberOverflow {
            line,
            key: key.to_owned(),
        },
    })?;
    usize::try_from(number).map_err(|_| AfmParseError::NumberOverflow {
        line,
        key: key.to_owned(),
    })
}

fn parse_i64(value: &str, line: usize, key: &str) -> Result<i64, AfmParseError> {
    let token = single_token(value, line, key)?;
    let (negative, digits) = strip_sign(token).map_err(|_| AfmParseError::InvalidValue {
        line,
        key: key.to_owned(),
    })?;
    let magnitude = parse_unsigned_integer(digits).map_err(|error| match error {
        NumberError::Invalid | NumberError::TooPrecise => AfmParseError::InvalidValue {
            line,
            key: key.to_owned(),
        },
        NumberError::Overflow => AfmParseError::NumberOverflow {
            line,
            key: key.to_owned(),
        },
    })?;
    let signed = if negative {
        -i128::from(magnitude)
    } else {
        i128::from(magnitude)
    };
    i64::try_from(signed).map_err(|_| AfmParseError::NumberOverflow {
        line,
        key: key.to_owned(),
    })
}

fn strip_sign(token: &str) -> Result<(bool, &str), NumberError> {
    if let Some(digits) = token.strip_prefix('-') {
        if digits.is_empty() {
            Err(NumberError::Invalid)
        } else {
            Ok((true, digits))
        }
    } else if let Some(digits) = token.strip_prefix('+') {
        if digits.is_empty() {
            Err(NumberError::Invalid)
        } else {
            Ok((false, digits))
        }
    } else {
        Ok((false, token))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumberError {
    Invalid,
    Overflow,
    TooPrecise,
}

fn parse_unsigned_integer(token: &str) -> Result<u64, NumberError> {
    if token.is_empty() {
        return Err(NumberError::Invalid);
    }
    let mut value = 0u64;
    for digit in token.bytes() {
        if !digit.is_ascii_digit() {
            return Err(NumberError::Invalid);
        }
        value = value
            .checked_mul(10)
            .and_then(|number| number.checked_add(u64::from(digit - b'0')))
            .ok_or(NumberError::Overflow)?;
    }
    Ok(value)
}

fn parse_fixed_decimal(token: &str) -> Result<AfmNumber, NumberError> {
    let (negative, unsigned) = if let Some(rest) = token.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = token.strip_prefix('+') {
        (false, rest)
    } else {
        (false, token)
    };
    if unsigned.is_empty() {
        return Err(NumberError::Invalid);
    }

    let mut parts = unsigned.split('.');
    let integer_digits = parts.next().unwrap_or_default();
    let fractional_digits = parts.next();
    if parts.next().is_some()
        || (integer_digits.is_empty() && fractional_digits.is_none_or(str::is_empty))
    {
        return Err(NumberError::Invalid);
    }

    let integer = if integer_digits.is_empty() {
        0
    } else {
        parse_unsigned_integer(integer_digits)?
    };
    let mut fraction = 0u64;
    let mut fraction_length = 0usize;
    if let Some(digits) = fractional_digits {
        for digit in digits.bytes() {
            if !digit.is_ascii_digit() {
                return Err(NumberError::Invalid);
            }
            if fraction_length < 6 {
                fraction = fraction
                    .checked_mul(10)
                    .and_then(|number| number.checked_add(u64::from(digit - b'0')))
                    .ok_or(NumberError::Overflow)?;
                fraction_length += 1;
            } else if digit != b'0' {
                return Err(NumberError::TooPrecise);
            }
        }
    }
    while fraction_length < 6 {
        fraction = fraction.checked_mul(10).ok_or(NumberError::Overflow)?;
        fraction_length += 1;
    }

    let scaled = i128::from(integer)
        .checked_mul(i128::from(AfmNumber::SCALE))
        .and_then(|number| number.checked_add(i128::from(fraction)))
        .ok_or(NumberError::Overflow)?;
    let signed = if negative { -scaled } else { scaled };
    let scaled = i64::try_from(signed).map_err(|_| NumberError::Overflow)?;
    Ok(AfmNumber::from_scaled(scaled))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor_lines() -> &'static str {
        "FontName Synthetic-Regular\n\
         FontBBox -10.5 -200 1000.25 900\n\
         ItalicAngle -12.5\n\
         IsFixedPitch false\n\
         CapHeight 700\n\
         XHeight 450.125\n\
         Ascender 750\n\
         Descender -250\n\
         StdVW 80.25\n\
         StdHW 70\n"
    }

    fn complete_afm(character_lines: &str, count: usize) -> String {
        format!(
            "StartFontMetrics 4.1\n{}StartCharMetrics {count}\n{character_lines}EndCharMetrics\nEndFontMetrics\n",
            descriptor_lines()
        )
    }

    #[test]
    fn 改行形式と未知キーに依存せず必要な値を読む() {
        let source = complete_afm(
            "C 65 ; WX 722.125 ; N A ; B 0 0 700 700 ;\n\
             C -1 ; W0X 333.5 ; N unencoded ;\n",
            2,
        )
        .replace("\n", "\r\n")
        .replace("FontBBox", "UnknownHeader ignored\r\nFontBBox");

        let font = parse_afm(source.as_bytes()).unwrap();
        assert_eq!(font.descriptor.font_name, "Synthetic-Regular");
        assert_eq!(font.descriptor.encoding_scheme, None);
        assert_eq!(font.descriptor.font_bbox[0].scaled(), -10_500_000);
        assert_eq!(font.descriptor.italic_angle.scaled(), -12_500_000);
        assert!(!font.descriptor.is_fixed_pitch);
        assert_eq!(font.descriptor.x_height.unwrap().scaled(), 450_125_000);
        assert_eq!(font.descriptor.std_hw.unwrap().scaled(), 70_000_000);
        assert_eq!(font.metrics_by_code[&65].width_x.scaled(), 722_125_000);
        assert_eq!(
            font.metrics_by_name["unencoded"].width_x.scaled(),
            333_500_000
        );
    }

    #[test]
    fn 符号なし文字だけをコード表へ入れる() {
        let source = complete_afm(
            "C -1 ; WX 500 ; N by_name ;\n\
             C 256 ; WX 600 ; N outside_byte ;\n\
             CH <42> ; WX 700 ; N B ;\n",
            3,
        );

        let font = parse_afm(source.as_bytes()).unwrap();
        assert_eq!(font.metrics_by_name.len(), 3);
        assert_eq!(font.metrics_by_code.len(), 1);
        assert_eq!(font.metrics_by_code[&0x42].name.as_deref(), Some("B"));
        assert_eq!(font.metrics_by_name["by_name"].code, None);
        assert_eq!(font.metrics_by_name["outside_byte"].code, None);
    }

    #[test]
    fn 固定小数を表示しても精度を失わない() {
        let values = [
            ("0", 0, "0"),
            (".5", 500_000, "0.5"),
            ("+12.340000", 12_340_000, "12.34"),
            ("-0.000001", -1, "-0.000001"),
        ];
        for (source, scaled, rendered) in values {
            let number = parse_fixed_decimal(source).unwrap();
            assert_eq!(number.scaled(), scaled);
            assert_eq!(number.to_string(), rendered);
        }
    }

    #[test]
    fn 必須記述子の欠損を名前で報告する() {
        let source = format!(
            "StartFontMetrics 4.1\n{}EndFontMetrics\n",
            descriptor_lines().replace("CapHeight 700\n", "")
        );
        assert_eq!(
            parse_afm(source.as_bytes()),
            Err(AfmParseError::MissingDescriptorField("CapHeight"))
        );
    }

    #[test]
    fn 古いafmで省略されたstem幅を未指定として保つ() {
        let source = format!(
            "StartFontMetrics 4.1\nEncodingScheme FontSpecific\n{}EndFontMetrics\n",
            descriptor_lines()
                .replace("StdVW 80.25\n", "")
                .replace("StdHW 70\n", "")
        );
        let font = parse_afm(source.as_bytes()).unwrap();
        assert_eq!(
            font.descriptor.encoding_scheme.as_deref(),
            Some("FontSpecific")
        );
        assert_eq!(font.descriptor.std_vw, None);
        assert_eq!(font.descriptor.std_hw, None);
    }

    #[test]
    fn 記述子の重複を黙って上書きしない() {
        let source = format!(
            "StartFontMetrics 4.1\n{}FontName Other\nEndFontMetrics\n",
            descriptor_lines()
        );
        assert!(matches!(
            parse_afm(source.as_bytes()),
            Err(AfmParseError::DuplicateDescriptorField {
                key: "FontName",
                ..
            })
        ));
    }

    #[test]
    fn font名を一つのafm名に限定する() {
        let source = format!(
            "StartFontMetrics 4.1\n{}EndFontMetrics\n",
            descriptor_lines().replace("FontName Synthetic-Regular", "FontName two names")
        );
        assert!(matches!(
            parse_afm(source.as_bytes()),
            Err(AfmParseError::InvalidValue { key, .. }) if key == "FontName"
        ));
    }

    #[test]
    fn グリフ名と文字コードの重複を別々に拒む() {
        let duplicate_name =
            complete_afm("C 65 ; WX 500 ; N same ;\nC 66 ; WX 600 ; N same ;\n", 2);
        assert!(matches!(
            parse_afm(duplicate_name.as_bytes()),
            Err(AfmParseError::DuplicateGlyphName { .. })
        ));

        let duplicate_code =
            complete_afm("C 65 ; WX 500 ; N first ;\nC 65 ; WX 600 ; N second ;\n", 2);
        assert!(matches!(
            parse_afm(duplicate_code.as_bytes()),
            Err(AfmParseError::DuplicateCharacterCode { code: 65, .. })
        ));
    }

    #[test]
    fn 同じglyphを同じ幅で複数codeへ割り当てられる() {
        let source = complete_afm(
            "C 32 ; WX 277 ; N suppress ;\nC 128 ; WX 277 ; N suppress ;\n",
            2,
        );
        let font = parse_afm(source.as_bytes()).unwrap();
        assert_eq!(font.metrics_by_name.len(), 1);
        assert_eq!(font.metrics_by_code.len(), 2);
        assert_eq!(font.metrics_by_name["suppress"].code, Some(32));
    }

    #[test]
    fn 数値の桁あふれと過剰精度を区別する() {
        let overflow = complete_afm("C 65 ; WX 999999999999999999999 ; N A ;\n", 1);
        assert!(matches!(
            parse_afm(overflow.as_bytes()),
            Err(AfmParseError::NumberOverflow { key, .. }) if key == "WX"
        ));

        let too_precise = complete_afm("C 65 ; WX 1.0000001 ; N A ;\n", 1);
        assert!(matches!(
            parse_afm(too_precise.as_bytes()),
            Err(AfmParseError::NumberTooPrecise { key, .. }) if key == "WX"
        ));
    }

    #[test]
    fn 不正なutf8をpanicせず拒む() {
        assert_eq!(
            parse_afm(b"StartFontMetrics 4.1\nFontName \xff\n"),
            Err(AfmParseError::InvalidUtf8 { valid_up_to: 30 })
        );
    }

    #[test]
    fn 文字メトリクスの宣言数を検査する() {
        let source = complete_afm("C 65 ; WX 500 ; N A ;\n", 2);
        assert_eq!(
            parse_afm(source.as_bytes()),
            Err(AfmParseError::CharMetricsCountMismatch {
                declared: 2,
                actual: 1
            })
        );
    }

    #[test]
    fn 文字メトリクスの必須値を検査する() {
        let missing_code = complete_afm("WX 500 ; N A ;\n", 1);
        assert!(matches!(
            parse_afm(missing_code.as_bytes()),
            Err(AfmParseError::MissingCharacterField { key: "C/CH", .. })
        ));

        let missing_width = complete_afm("C 65 ; N A ;\n", 1);
        assert!(matches!(
            parse_afm(missing_width.as_bytes()),
            Err(AfmParseError::MissingCharacterField { key: "WX/W0X", .. })
        ));
    }
}

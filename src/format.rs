pub(crate) mod binary;
mod dump_command;
mod dump_noads;
mod dump_nodes;

use crate::dimension::Dimension;
use crate::eqtb::{ControlSequence, Eqtb, FontIndex, IntegerVariable};
use crate::error::dump_in_group_error;
use crate::hyphenation::Hyphenator;
use crate::input::InputStack;
use crate::logger::{job_output_path, InteractionMode, Logger};
use crate::nodes::GlueRatio;
use crate::print::Printer;
use crate::{open_in, open_out, os_string_from_bytes};

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::File;
use std::hash::Hash;
use std::io::{BufWriter, Read, Write};
use std::path::PathBuf;
use std::str::FromStr;

pub const FORMAT_EXTENSION: &str = "fmt";
const FORMAT_DEFAULT_AREA: &str = "TeXformats";
const FORMAT_DEFAULT_PLAIN: &str = "plain";
pub type LoadedFormat = Box<(Logger, Hyphenator, Box<Eqtb>)>;

/// See 1332.
pub fn load_hyphenator_and_eqtb_from_specified_format_file(
    format_name: OsString,
) -> Result<LoadedFormat, ()> {
    let Some(fmt_file) = open_fmt_file(Some(format_name)) else {
        return Err(());
    };
    let Ok(undumped_objects) = load_fmt_file(fmt_file) else {
        return Err(());
    };
    Ok(undumped_objects)
}

/// See 1332.
pub fn load_hyphenator_and_eqtb_from_default_format_file() -> Result<LoadedFormat, ()> {
    // Now attempt to load the format file or abort.
    let Some(fmt_file) = open_fmt_file(None) else {
        return Err(());
    };
    let Ok(undumped_objects) = load_fmt_file(fmt_file) else {
        return Err(());
    };
    Ok(undumped_objects)
}

/// Attempts to open a format file.
///
/// If the first character of the line is a `&` character, all non-space
/// characters up to the next space character or the end of the line are
/// considered the file name for the format file.
/// In case that the given format file cannot be found or in case that the
/// first character is not a `&`, the PLAIN format file will be opened.
/// If that fails as well, None is returned.
/// See 524.
fn open_fmt_file(format_name: Option<OsString>) -> Option<File> {
    if let Some(file_name) = format_name {
        // Try to find format file from current working directory.
        let mut path = PathBuf::from(&file_name);
        path.set_extension(FORMAT_EXTENSION);
        if let Ok(file) = open_in(&path) {
            return Some(file);
        }
        // Try to find format file in the default format directory.
        let mut path: PathBuf = PathBuf::from(FORMAT_DEFAULT_AREA);
        path.push(file_name);
        path.set_extension(FORMAT_EXTENSION);
        if let Ok(file) = open_in(&path) {
            return Some(file);
        }
        println!("Sorry, I can't find that format; will try PLAIN.");
    }
    // Look for default PLAIN format file
    let mut path: PathBuf = [FORMAT_DEFAULT_AREA, FORMAT_DEFAULT_PLAIN].iter().collect();
    path.set_extension(FORMAT_EXTENSION);
    match open_in(&path) {
        Ok(file) => Some(file),
        Err(_) => {
            println!("I can't find the PLAIN format file!");
            None
        }
    }
}

/// NOTE Does currently not check that all undumped value are in a valid range.
/// See 1303.
fn load_fmt_file(mut fmt_file: File) -> Result<LoadedFormat, FormatError> {
    let mut bytes = Vec::new();
    Read::by_ref(&mut fmt_file)
        .take(binary::MAX_FORMAT_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| FormatError::IncompleteFile)?;
    if bytes.len() > binary::MAX_FORMAT_FILE_BYTES {
        return Err(FormatError::AllocationFailed);
    }
    if binary::has_magic(&bytes) {
        return load_binary_fmt_file(&bytes);
    }
    let format_string = String::from_utf8(bytes).map_err(|_| {
        println!("Format file is not valid UTF-8");
        FormatError::NoUtf8
    })?;
    let mut lines = CountedLines::from_str(&format_string);
    let eqtb = undump_table_of_equivalents(&mut lines)?;
    let hyphenator = undump_hyphenation_tables(&mut lines)?;
    let logger = undump_a_couple_more(&mut lines, &eqtb)?;
    Ok(Box::new((logger, hyphenator, eqtb)))
}

fn load_binary_fmt_file(bytes: &[u8]) -> Result<LoadedFormat, FormatError> {
    let sections = binary::parse_format(bytes)?;

    let eqtb_text = std::str::from_utf8(sections.eqtb_legacy_text).map_err(|_| {
        println!("Format Eqtb section is not valid UTF-8");
        FormatError::NoUtf8
    })?;
    let mut eqtb_lines = CountedLines::from_str(eqtb_text);
    let eqtb = undump_table_of_equivalents(&mut eqtb_lines)?;
    if eqtb_lines.next().is_some() {
        return Err(FormatError::ParseError);
    }

    let hyphenator = Hyphenator::undump_runtime_binary(sections.hyphen_runtime)?;

    let metadata_text = std::str::from_utf8(sections.run_metadata_legacy_text).map_err(|_| {
        println!("Format metadata section is not valid UTF-8");
        FormatError::NoUtf8
    })?;
    let mut metadata_lines = CountedLines::from_str(metadata_text);
    let logger = undump_a_couple_more(&mut metadata_lines, &eqtb)?;
    if metadata_lines.next().is_some() {
        return Err(FormatError::ParseError);
    }
    Ok(Box::new((logger, hyphenator, eqtb)))
}

/// See 1314.
fn undump_table_of_equivalents(lines: &mut CountedLines) -> Result<Box<Eqtb>, FormatError> {
    match Box::<Eqtb>::undump(lines) {
        Ok(eqtb) => Ok(eqtb),
        Err(format_error) => {
            println!(
                "Format error on line {} while parsing Eqtb",
                lines.line_number()
            );
            Err(format_error)
        }
    }
}

/// See 1325.
fn undump_hyphenation_tables<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
) -> Result<Hyphenator, FormatError> {
    match Hyphenator::undump(lines) {
        Ok(hyphenator) => Ok(hyphenator),
        Err(format_error) => {
            println!("Format error while parsing hyphenation tables");
            Err(format_error)
        }
    }
}

/// See 1327.
fn undump_a_couple_more<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    eqtb: &Eqtb,
) -> Result<Logger, FormatError> {
    let interaction = match InteractionMode::undump(lines) {
        Ok(interaction) => interaction,
        Err(_) => {
            println!("Format error while parsing interaction");
            return Err(FormatError::ParseError);
        }
    };
    let format_ident = match String::undump(lines) {
        Ok(format_ident) => format_ident,
        Err(_) => {
            println!("Format error while parsing format_ident");
            return Err(FormatError::ParseError);
        }
    };

    let mut logger = Logger::new(format_ident, interaction);

    // We update the Logger's copies of escapechar and newlinechar here.
    logger.escape_char = eqtb.get_current_escape_character();
    logger.newline_char = eqtb.get_current_newline_character();

    match i32::undump(lines) {
        Ok(69069) => Ok(logger),
        _ => {
            println!("Format error: final constant incorrect");
            Err(FormatError::WrongConstant)
        }
    }
}

/// See 1302. and 1304.
pub fn store_fmt_file(
    hyphenator: &Hyphenator,
    input_stack: &InputStack,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    if !eqtb.save_stack.is_empty() {
        dump_in_group_error(input_stack, eqtb, logger);
    }
    let format_file = create_format_ident_and_open_file(input_stack, eqtb, logger);
    let mut format_file = BufWriter::new(format_file);
    if let Some(log_file) = &mut logger.log_file {
        log_file.flush().expect("Error writing to log file");
    }

    if binary_runtime_format_requested() {
        let mut eqtb_text = Vec::new();
        dump_table_of_equivalents(&mut eqtb_text, eqtb, logger).unwrap();
        dump_font_information(eqtb, logger).unwrap();
        print_hyphenation_summary(hyphenator, logger);
        let hyphen_runtime = hyphenator.dump_runtime_binary().unwrap();
        let mut run_metadata_text = Vec::new();
        dump_a_couple_more(&mut run_metadata_text, logger).unwrap();
        binary::write_format(
            &mut format_file,
            &eqtb_text,
            &hyphen_runtime,
            &run_metadata_text,
        )
        .unwrap();
    } else {
        dump_table_of_equivalents(&mut format_file, eqtb, logger).unwrap();
        dump_font_information(eqtb, logger).unwrap();
        dump_hyphenation_tables(&mut format_file, hyphenator, logger).unwrap();
        dump_a_couple_more(&mut format_file, logger).unwrap();
    }

    eqtb.integers.set(IntegerVariable::TracingStats, 0);
}

fn binary_runtime_format_requested() -> bool {
    std::env::var("PRATEX_FMT_CODEC").as_deref() != Ok("legacy-text")
}

/// See 1328.
fn create_format_ident_and_open_file(
    input_stack: &InputStack,
    eqtb: &Eqtb,
    logger: &mut Logger,
) -> File {
    let mut format_ident = " (preloaded format=".to_string();
    format_ident.push_str(&logger.job_name.as_ref().unwrap().to_string_lossy());
    let date = format!(
        " {}.{}.{})",
        eqtb.integer(IntegerVariable::Year),
        eqtb.integer(IntegerVariable::Month),
        eqtb.integer(IntegerVariable::Day),
    );
    format_ident.push_str(&date);
    logger.terminal_logging = logger.interaction != InteractionMode::Batch;
    let job_name = logger.job_name.as_ref().unwrap();
    let mut path = job_output_path(job_name, FORMAT_EXTENSION);

    let file = loop {
        match open_out(&path) {
            Ok(file) => break file,
            Err(_) => {
                path = logger.prompt_format_file_name(&path, input_stack, eqtb);
            }
        }
    };
    logger.print_nl_str("Beginning to dump on file ");
    logger.slow_print_str(path.as_os_str().as_encoded_bytes());
    logger.print_nl_str("");
    logger.slow_print_str(format_ident.as_bytes());
    logger.format_ident = format_ident;
    file
}

/// See 1313.
fn dump_table_of_equivalents(
    format_file: &mut impl Write,
    eqtb: &Eqtb,
    logger: &mut Logger,
) -> Result<(), std::io::Error> {
    logger.print_ln();
    logger.print_int(eqtb.control_sequences.cs_count as i32);
    logger.print_str(" multiletter control sequences");
    eqtb.dump(format_file)
}

/// See 1320.
fn dump_font_information(eqtb: &Eqtb, logger: &mut Logger) -> Result<(), std::io::Error> {
    for (font_index, font) in eqtb.fonts.iter().enumerate() {
        logger.print_nl_str("\\font");
        logger.print_esc_str(
            eqtb.control_sequences
                .text(ControlSequence::FontId(font_index as FontIndex)),
        );
        logger.print_char(b'=');
        let mut file_name = font.area.clone();
        file_name.append(&mut font.name.clone());
        let path = PathBuf::from(os_string_from_bytes(file_name));
        logger.print_file_name(&path);
        if font.size != font.dsize {
            logger.print_str(" at ");
            logger.print_scaled(font.size);
            logger.print_str("pt");
        }
    }
    // Note that fonts get dumped as part of Eqtb.
    Ok(())
}

/// See 1324.
fn dump_hyphenation_tables(
    format_file: &mut impl Write,
    hyphenator: &Hyphenator,
    logger: &mut Logger,
) -> Result<(), std::io::Error> {
    print_hyphenation_summary(hyphenator, logger);
    hyphenator.dump(format_file)
}

fn print_hyphenation_summary(hyphenator: &Hyphenator, logger: &mut Logger) {
    let mut total_exceptions = 0;
    for exceptions in &hyphenator.exceptions {
        total_exceptions += exceptions.len();
    }
    logger.print_ln();
    logger.print_int(total_exceptions as i32);
    logger.print_str(" hyphenation exception");
    if total_exceptions != 1 {
        logger.print_char(b's');
    }
}

/// See 1326.
fn dump_a_couple_more(format_file: &mut impl Write, logger: &Logger) -> Result<(), std::io::Error> {
    logger.interaction.dump(format_file)?;
    logger.format_ident.dump(format_file)?;
    69069.dump(format_file)?;
    Ok(())
}

/// An error while parsing a format file.
#[derive(Debug)]
pub enum FormatError {
    NoUtf8,
    IncompleteFile,
    ParseError,
    WrongConstant,
    WrongChecksum,
    UnsupportedVersion,
    AllocationFailed,
}

/// A wrapper around `Lines` that counts how many lines have been consumed.
pub struct CountedLines<'a> {
    rest: &'a str,
    finished: bool,
    count: usize,
}

impl<'a> CountedLines<'a> {
    pub fn from_str(text: &'a str) -> Self {
        Self {
            rest: text,
            finished: text.is_empty(),
            count: 0,
        }
    }

    pub fn line_number(&self) -> usize {
        self.count
    }
}

impl<'a> Iterator for CountedLines<'a> {
    type Item = &'a str;

    /// NOTE: `str::lines` と同じ区切り方をするが、探索器の一般機構を通さない。
    /// fmt は一つの値につき一行という形なので、`latex.fmt` では約 395 万回
    /// 呼ばれる。一行が数 byte しかないため、汎用の部分文字列探索は設定費だけで
    /// 走査そのものより高くつく。
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        self.count += 1;
        let bytes = self.rest.as_bytes();
        let mut end = 0;
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        let line = &self.rest[..end];
        if end == bytes.len() {
            self.rest = "";
            self.finished = true;
        } else {
            self.rest = &self.rest[end + 1..];
            // 末尾が改行で終わる場合、その後ろに空行を作らない。
            self.finished = self.rest.is_empty();
        }
        Some(line.strip_suffix('\r').unwrap_or(line))
    }
}

/// 一行を十進の符号なし整数として読む。
///
/// NOTE: `str::parse` は基数、符号、桁溢れを汎用の経路で扱う。fmt は一つの値に
/// つき一行という形なので、`latex.fmt` ではこの解析が約 395 万回走る。十進に
/// 限れば必要な検査だけで足りる。`u64` で読んでから目的の型へ写すので、範囲外は
/// `str::parse` と同じく誤りになる。先頭の `+` も `str::parse` と同じく許す。
fn undump_unsigned<'a, T>(lines: &mut impl Iterator<Item = &'a str>) -> Result<T, FormatError>
where
    T: TryFrom<u64>,
{
    let text = lines.next().ok_or(FormatError::IncompleteFile)?;
    let digits = text.as_bytes().strip_prefix(b"+").unwrap_or(text.as_bytes());
    T::try_from(decimal_digits(digits)?).map_err(|_| FormatError::ParseError)
}

/// 一行を十進の符号つき整数として読む。
fn undump_signed<'a, T>(lines: &mut impl Iterator<Item = &'a str>) -> Result<T, FormatError>
where
    T: TryFrom<i64>,
{
    let text = lines.next().ok_or(FormatError::IncompleteFile)?;
    let bytes = text.as_bytes();
    let (negative, digits) = match bytes.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, bytes),
    };
    // i32::MIN の絶対値も u64 には収まるので、符号は最後に付ける。
    let magnitude = decimal_digits(digits)?;
    let value = if negative {
        i64::try_from(magnitude)
            .map(|v| -v)
            .map_err(|_| FormatError::ParseError)?
    } else {
        i64::try_from(magnitude).map_err(|_| FormatError::ParseError)?
    };
    T::try_from(value).map_err(|_| FormatError::ParseError)
}

/// 十進の桁だけからなる byte 列を `u64` にする。空、桁以外、桁溢れは誤り。
fn decimal_digits(digits: &[u8]) -> Result<u64, FormatError> {
    if digits.is_empty() {
        return Err(FormatError::ParseError);
    }
    let mut value: u64 = 0;
    for &byte in digits {
        let digit = byte.wrapping_sub(b'0');
        if digit > 9 {
            return Err(FormatError::ParseError);
        }
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(digit as u64))
            .ok_or(FormatError::ParseError)?;
    }
    Ok(value)
}

fn parse_next<'a, T: FromStr>(lines: &mut impl Iterator<Item = &'a str>) -> Result<T, FormatError> {
    lines
        .next()
        .ok_or(FormatError::IncompleteFile)?
        .parse()
        .map_err(|_| FormatError::ParseError)
}

// A fmt file is untrusted input. Reserving its declared collection length without a bound lets a
// short, truncated file force an arbitrarily large allocation before any element is validated.
// A small bounded reservation still removes the repeated growth of ordinary token lists while
// keeping the eager allocation independent of an attacker-controlled count.
const MAX_INITIAL_UNDUMP_ELEMENTS: usize = 4 * 1024;
const MAX_INITIAL_UNDUMP_PAYLOAD_BYTES: usize = 64 * 1024;

fn bounded_initial_capacity<T>(declared_len: usize) -> usize {
    let element_size = std::mem::size_of::<T>().max(1);
    declared_len
        .min(MAX_INITIAL_UNDUMP_ELEMENTS)
        .min(MAX_INITIAL_UNDUMP_PAYLOAD_BYTES / element_size)
}

pub trait Dumpable {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error>;
    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError>
    where
        Self: Sized;
}

impl Dumpable for u8 {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        undump_unsigned(lines)
    }
}

impl Dumpable for u16 {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        undump_unsigned(lines)
    }
}

impl Dumpable for u32 {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        undump_unsigned(lines)
    }
}

impl Dumpable for usize {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        undump_unsigned(lines)
    }
}

impl Dumpable for bool {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        parse_next(lines)
    }
}

impl Dumpable for String {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        parse_next(lines)
    }
}

impl<T: Dumpable + Default, const N: usize> Dumpable for [T; N] {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        for t in self {
            t.dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let mut a = std::array::from_fn(|_| T::default());
        for i in 0..N {
            a[i] = T::undump(lines)?;
        }
        Ok(a)
    }
}

impl<T, U> Dumpable for (T, U)
where
    T: Dumpable,
    U: Dumpable,
{
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)?;
        self.1.dump(target)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let t0 = T::undump(lines)?;
        let t1 = U::undump(lines)?;
        Ok((t0, t1))
    }
}

impl<T, U, V> Dumpable for (T, U, V)
where
    T: Dumpable,
    U: Dumpable,
    V: Dumpable,
{
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)?;
        self.1.dump(target)?;
        self.2.dump(target)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let t0 = T::undump(lines)?;
        let t1 = U::undump(lines)?;
        let t2 = V::undump(lines)?;
        Ok((t0, t1, t2))
    }
}

impl<T: Dumpable> Dumpable for Option<T> {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            None => writeln!(target, "None")?,
            Some(x) => {
                writeln!(target, "Some")?;
                x.dump(target)?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "None" => Ok(None),
            "Some" => {
                let x = T::undump(lines)?;
                Ok(Some(x))
            }
            _ => Err(FormatError::ParseError),
        }
    }
}

impl<T: Dumpable> Dumpable for Box<T> {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        (**self).dump(target)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let x = T::undump(lines)?;
        Ok(Box::new(x))
    }
}

impl<T: Dumpable> Dumpable for Vec<T> {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self.len())?;
        for x in self {
            x.dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let n = parse_next(lines)?;
        let mut vec = Vec::new();
        vec.try_reserve(bounded_initial_capacity::<T>(n))
            .map_err(|_| FormatError::AllocationFailed)?;
        for _ in 0..n {
            let x = T::undump(lines)?;
            vec.push(x);
        }
        Ok(vec)
    }
}

impl<T: Dumpable + Eq + Hash, U: Dumpable, S: std::hash::BuildHasher + Default> Dumpable
    for HashMap<T, U, S>
{
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self.len())?;
        for (key, val) in self {
            key.dump(target)?;
            val.dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let n = parse_next(lines)?;
        let mut map = HashMap::default();
        map.try_reserve(bounded_initial_capacity::<(T, U)>(n))
            .map_err(|_| FormatError::AllocationFailed)?;
        for _ in 0..n {
            let key = T::undump(lines)?;
            let val = U::undump(lines)?;
            map.insert(key, val);
        }
        Ok(map)
    }
}

impl<T: Dumpable> Dumpable for std::rc::Rc<T> {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        (**self).dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let t = T::undump(lines)?;
        Ok(std::rc::Rc::new(t))
    }
}

impl Dumpable for Dimension {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        undump_signed(lines)
    }
}

impl Dumpable for GlueRatio {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{}", self)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        parse_next(lines)
    }
}

#[cfg(test)]
mod bounded_reservation_tests {
    use super::{
        bounded_initial_capacity, Dumpable, FormatError, MAX_INITIAL_UNDUMP_PAYLOAD_BYTES,
    };
    use std::collections::HashMap;

    #[test]
    fn 巨大な宣言長だけのvecは巨大確保より先に不完全fmtとして止まる() {
        let source = format!("{}\n", usize::MAX);
        assert!(matches!(
            Vec::<u8>::undump(&mut source.lines()),
            Err(FormatError::IncompleteFile)
        ));
    }

    #[test]
    fn 巨大な宣言長だけのmapは巨大確保より先に不完全fmtとして止まる() {
        let source = format!("{}\n", usize::MAX);
        assert!(matches!(
            HashMap::<u8, u8>::undump(&mut source.lines()),
            Err(FormatError::IncompleteFile)
        ));
    }

    #[test]
    fn fmt予約の要素payload見積りは宣言長にかかわらず上限内に収まる() {
        assert_eq!(bounded_initial_capacity::<u8>(usize::MAX), 4 * 1024);
        assert!(
            bounded_initial_capacity::<[u8; 1024]>(usize::MAX) * 1024
                <= MAX_INITIAL_UNDUMP_PAYLOAD_BYTES
        );
    }
}

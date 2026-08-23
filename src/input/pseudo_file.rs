use crate::print::{cannot_be_printed, to_hex_char, Printer};

/// `\scantokens` 一回が所有できる印字byte数。
///
/// 疑似入力だけでprocess memoryを無制限に増やさず、通常のpackage生成コードには十分な
/// 余裕を持たせる。値はCLIやformatで暗黙に変えず、runを通じて一定にする。
pub(super) const MAX_SCANTOKENS_BYTES_PER_SOURCE: usize = 16 * 1024 * 1024;
/// 一つの疑似入力が持てる論理行数。
pub(super) const MAX_SCANTOKENS_LINES_PER_SOURCE: usize = 1_000_000;
/// 入れ子の疑似入力が同時に所有できる概算byte数。
pub(super) const MAX_VIRTUAL_INPUT_BYTES_LIVE: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PseudoTextLimit {
    pub resource: &'static str,
    pub limit: usize,
}

/// 文字byteと論理改行を混同しない `\scantokens` の所有buffer。
#[derive(Debug)]
pub(crate) struct PseudoText {
    bytes: Vec<u8>,
    line_ends: Vec<usize>,
    next_line: usize,
}

impl PseudoText {
    pub fn line_count(&self) -> usize {
        self.line_ends.len()
    }

    pub fn charge(&self) -> usize {
        self.bytes.len().saturating_add(
            self.line_ends
                .len()
                .saturating_mul(std::mem::size_of::<usize>()),
        )
    }

    /// 次の論理行だけを通常の `LineLexer` が所有する形へ写す。
    /// `\endlinechar` は疑似source生成時でなく、各行を読む時点で加える。
    pub fn next_line(&mut self, end_line_char: Option<u8>) -> Result<Option<Vec<u8>>, ()> {
        let Some(&end) = self.line_ends.get(self.next_line) else {
            return Ok(None);
        };
        let start = if self.next_line == 0 {
            0
        } else {
            self.line_ends[self.next_line - 1]
        };
        let extra = usize::from(end_line_char.is_some());
        let mut line = Vec::new();
        line.try_reserve(end.saturating_sub(start).saturating_add(extra))
            .map_err(|_| ())?;
        line.extend_from_slice(&self.bytes[start..end]);
        if let Some(c) = end_line_char {
            line.push(c);
        }
        self.next_line += 1;
        Ok(Some(line))
    }
}

/// Token表示を一度だけ行い、実fileを経由せず型付き疑似入力へ変換する。
pub(super) struct PseudoFilePrinter {
    bytes: Vec<u8>,
    line_ends: Vec<usize>,
    current_line_start: usize,
    current_line_had_input: bool,
    newline_char: Option<u8>,
    escape_char: Option<u8>,
    tally: usize,
    limit_error: Option<PseudoTextLimit>,
}

impl PseudoFilePrinter {
    pub fn new(newline_char: Option<u8>, escape_char: Option<u8>) -> Self {
        Self {
            bytes: Vec::new(),
            line_ends: Vec::new(),
            current_line_start: 0,
            current_line_had_input: false,
            newline_char,
            escape_char,
            tally: 0,
            limit_error: None,
        }
    }

    pub fn finish(mut self) -> Result<PseudoText, PseudoTextLimit> {
        if self.current_line_had_input {
            self.finish_line();
        }
        if let Some(error) = self.limit_error {
            return Err(error);
        }
        Ok(PseudoText {
            bytes: self.bytes,
            line_ends: self.line_ends,
            next_line: 0,
        })
    }

    fn set_limit_error(&mut self, resource: &'static str, limit: usize) {
        if self.limit_error.is_none() {
            self.limit_error = Some(PseudoTextLimit { resource, limit });
        }
    }

    fn push_raw(&mut self, c: u8) {
        if self.limit_error.is_some() {
            return;
        }
        if self.bytes.len() == MAX_SCANTOKENS_BYTES_PER_SOURCE || self.bytes.try_reserve(1).is_err()
        {
            self.set_limit_error("scantokens buffer size", MAX_SCANTOKENS_BYTES_PER_SOURCE);
            return;
        }
        self.bytes.push(c);
        self.current_line_had_input = true;
        self.tally = self.tally.saturating_add(1);
    }

    fn push_raw_bytes(&mut self, bytes: &[u8]) {
        for &c in bytes {
            self.push_raw(c);
            if self.limit_error.is_some() {
                return;
            }
        }
    }

    fn finish_line(&mut self) {
        if self.limit_error.is_some() {
            return;
        }
        while self.bytes.len() > self.current_line_start && self.bytes.last() == Some(&b' ') {
            self.bytes.pop();
        }
        if self.line_ends.len() == MAX_SCANTOKENS_LINES_PER_SOURCE
            || self.line_ends.try_reserve(1).is_err()
        {
            self.set_limit_error("scantokens line count", MAX_SCANTOKENS_LINES_PER_SOURCE);
            return;
        }
        self.line_ends.push(self.bytes.len());
        self.current_line_start = self.bytes.len();
        self.current_line_had_input = false;
    }

    fn print_raw_or_newline(&mut self, code_point: u32, bytes: &[u8]) {
        if self
            .newline_char
            .is_some_and(|newline| u32::from(newline) == code_point)
        {
            self.finish_line();
        } else {
            self.push_raw_bytes(bytes);
        }
    }
}

impl Printer for PseudoFilePrinter {
    fn print_ln(&mut self) {
        self.finish_line();
    }

    fn print_char(&mut self, c: u8) {
        self.print_raw_or_newline(u32::from(c), std::slice::from_ref(&c));
    }

    fn print_uptex_char(&mut self, code_point: u32, bytes: &[u8]) {
        self.print_raw_or_newline(code_point, bytes);
    }

    fn print(&mut self, c: u8) {
        if self.newline_char == Some(c) {
            self.finish_line();
        } else if cannot_be_printed(c) {
            self.push_raw(b'^');
            self.push_raw(b'^');
            if c < 64 {
                self.push_raw(c + 64);
            } else if c < 128 {
                self.push_raw(c - 64);
            } else {
                self.push_raw(to_hex_char(c / 16));
                self.push_raw(to_hex_char(c % 16));
            }
        } else {
            self.push_raw(c);
        }
    }

    fn print_str(&mut self, s: &str) {
        for &c in s.as_bytes() {
            self.print_char(c);
        }
    }

    fn current_escape_character(&self) -> Option<u8> {
        self.escape_char
    }

    fn get_tally(&self) -> usize {
        self.tally
    }

    fn reset_tally(&mut self) {
        self.tally = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 空入力と空白だけの入力を区別する() {
        let empty = PseudoFilePrinter::new(None, Some(b'\\')).finish().unwrap();
        assert_eq!(empty.line_count(), 0);

        let mut spaces = PseudoFilePrinter::new(None, Some(b'\\'));
        spaces.print_char(b' ');
        let mut spaces = spaces.finish().unwrap();
        assert_eq!(spaces.line_count(), 1);
        assert_eq!(spaces.next_line(None).unwrap(), Some(Vec::new()));
    }

    #[test]
    fn 連続改行と末尾改行は余分な空行を作らない() {
        let mut printer = PseudoFilePrinter::new(Some(b'|'), Some(b'\\'));
        for &c in b"a||b|" {
            printer.print_char(c);
        }
        let mut text = printer.finish().unwrap();
        assert_eq!(text.line_count(), 3);
        assert_eq!(text.next_line(None).unwrap(), Some(b"a".to_vec()));
        assert_eq!(text.next_line(None).unwrap(), Some(Vec::new()));
        assert_eq!(text.next_line(None).unwrap(), Some(b"b".to_vec()));
        assert_eq!(text.next_line(None).unwrap(), None);
    }
}

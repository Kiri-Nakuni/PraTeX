use super::extended_registers::{
    ExtendedRegisterStorage, DENSE_REGISTER_COUNT, MAX_EXTENDED_REGISTER_INDEX,
};
use super::levels::Level;
use super::{undump_register_index, RegisterIndex};

use crate::format::{Dumpable, FormatError};
#[cfg(test)]
use crate::input::line_lexer::{LexError, LineLexer};
use crate::print::{cannot_be_printed, to_hex_char};
use crate::token::Token;

use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

const RAW_STRING_REGISTERS_DUMP_HEADER: &str = "RawStringRegisters/v1";
const RAW_STRING_LEVELS_DUMP_HEADER: &str = "RawStringLevels/v1";

/// 一つの生文字列registerが所有できるbyte数。
///
/// 任意byteを保存できることと無制限にmemoryを確保することは別の契約である。
/// `\scantokens` の一入力源と同じ上限に揃え、fmtの破損長をallocation前に拒む。
pub(crate) const MAX_RAW_STRING_BYTES: usize = 16 * 1024 * 1024;
/// 一つのrun/fmtが全生文字列slotへ論理的に保存できるbyte数。
///
/// register copyは`Rc`を共有するがfmtはslotごとの値を保存するため、共有前の実memory量でなく
/// slot長の合計を数える。破損fmtがdense 256個で数GiBを確保する前に逐次拒否できる。
pub(crate) const MAX_RAW_STRING_STORAGE_BYTES: usize = 64 * 1024 * 1024;

pub(crate) type RcRawString = Rc<Vec<u8>>;

/// 生文字列registerの固定slotを表す。token listのslotとは別domainである。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawStringVariable(RegisterIndex);

impl RawStringVariable {
    pub const fn new(register: RegisterIndex) -> Self {
        Self(register)
    }

    pub const fn register(self) -> RegisterIndex {
        self.0
    }

    pub(crate) fn to_string(self) -> Vec<u8> {
        format!("rawstring{}", self.0).into_bytes()
    }
}

impl Dumpable for RawStringVariable {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self(undump_register_index(lines)?))
    }
}

/// `Rc`を値のidentityとして持つ。cloneはbyte列を複製しない。
#[derive(Debug, Clone, PartialEq, Eq)]
struct RawStringValue(RcRawString);

impl RawStringValue {
    fn empty() -> Self {
        Self(Rc::new(Vec::new()))
    }

    fn undump_with_budget<'a>(
        lines: &mut impl Iterator<Item = &'a str>,
        total_bytes: &mut usize,
    ) -> Result<Self, FormatError> {
        let len = usize::undump(lines)?;
        if len > MAX_RAW_STRING_BYTES {
            return Err(FormatError::ParseError);
        }
        let next_total = total_bytes
            .checked_add(len)
            .ok_or(FormatError::ParseError)?;
        if next_total > MAX_RAW_STRING_STORAGE_BYTES {
            return Err(FormatError::ParseError);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| FormatError::ParseError)?;
        for _ in 0..len {
            bytes.push(u8::undump(lines)?);
        }
        *total_bytes = next_total;
        Ok(Self(Rc::new(bytes)))
    }
}

impl Dumpable for RawStringValue {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.len().dump(target)?;
        for byte in self.0.iter() {
            byte.dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let mut total_bytes = 0;
        Self::undump_with_budget(lines, &mut total_bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RawStringStorageError {
    ValueTooLarge,
    StorageTooLarge,
}

/// e-TeX拡張registerと同じ低位dense・高位sparse配置を使う生文字列storage。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawStringRegisters {
    values: ExtendedRegisterStorage<RawStringValue>,
    total_bytes: usize,
}

impl RawStringRegisters {
    pub(crate) fn new() -> Self {
        Self {
            values: ExtendedRegisterStorage::new(RawStringValue::empty()),
            total_bytes: 0,
        }
    }

    pub(crate) fn get(&self, variable: RawStringVariable) -> &RcRawString {
        &self.values.get(variable.register()).0
    }

    #[cfg(test)]
    pub(crate) fn can_set(
        &self,
        variable: RawStringVariable,
        value: &[u8],
    ) -> Result<(), RawStringStorageError> {
        if value.len() > MAX_RAW_STRING_BYTES {
            return Err(RawStringStorageError::ValueTooLarge);
        }
        let previous_len = self.get(variable).len();
        let next_total = self
            .total_bytes
            .checked_sub(previous_len)
            .and_then(|total| total.checked_add(value.len()))
            .ok_or(RawStringStorageError::StorageTooLarge)?;
        if next_total > MAX_RAW_STRING_STORAGE_BYTES {
            Err(RawStringStorageError::StorageTooLarge)
        } else {
            Ok(())
        }
    }

    /// 現在値だけでなく、他slotと対象slotの将来restoreに必要な余白も含めて検査する。
    ///
    /// `other_restore_bytes`は他slotそれぞれの
    /// `max(restorable_len, current_len) - current_len` の和、
    /// `target_restore_len`は対象slotで将来restoreされ得る値の最大長である。
    pub(crate) fn can_set_with_restore_budget(
        &self,
        variable: RawStringVariable,
        value: &[u8],
        other_restore_bytes: usize,
        target_restore_len: usize,
    ) -> Result<(), RawStringStorageError> {
        if value.len() > MAX_RAW_STRING_BYTES || target_restore_len > MAX_RAW_STRING_BYTES {
            return Err(RawStringStorageError::ValueTooLarge);
        }
        let previous_len = self.get(variable).len();
        let next_current_total = self
            .total_bytes
            .checked_sub(previous_len)
            .and_then(|total| total.checked_add(value.len()))
            .ok_or(RawStringStorageError::StorageTooLarge)?;
        let target_restore_bytes = target_restore_len.saturating_sub(value.len());
        let envelope = next_current_total
            .checked_add(other_restore_bytes)
            .and_then(|total| total.checked_add(target_restore_bytes))
            .ok_or(RawStringStorageError::StorageTooLarge)?;
        if envelope > MAX_RAW_STRING_STORAGE_BYTES {
            Err(RawStringStorageError::StorageTooLarge)
        } else {
            Ok(())
        }
    }

    /// 値を設定し、設定前の`Rc`を返す。register間代入はcallerがcloneするだけでよい。
    #[cfg(test)]
    pub(crate) fn set(
        &mut self,
        variable: RawStringVariable,
        value: RcRawString,
    ) -> Result<RcRawString, RawStringStorageError> {
        self.can_set(variable, &value)?;
        let previous_len = self.get(variable).len();
        let next_total = self
            .total_bytes
            .checked_sub(previous_len)
            .and_then(|total| total.checked_add(value.len()))
            .expect("can_set checked raw string storage arithmetic");
        let previous = self
            .values
            .set_compact(variable.register(), RawStringValue(value))
            .0;
        self.total_bytes = next_total;
        Ok(previous)
    }

    /// `Eqtb`がactive/future envelopeを先に検査したdefinitionまたはrestoreを適用する。
    ///
    /// restore時に容量errorを返す道を残すと、正当に保存した値を群終了時に捨てるかpanicする
    /// ことになる。したがってcapacityの決定はdefinition前の一箇所に置き、ここは値交換だけを
    /// 行う。`saturating_*`はこの関数自身をpanic不能にし、envelope invariantが正常なら通常の
    /// 加減算と同じ結果になる。
    pub(crate) fn replace_reserved(
        &mut self,
        variable: RawStringVariable,
        value: RcRawString,
    ) -> RcRawString {
        debug_assert!(value.len() <= MAX_RAW_STRING_BYTES);
        let previous_len = self.get(variable).len();
        let next_total = self
            .total_bytes
            .saturating_sub(previous_len)
            .saturating_add(value.len());
        debug_assert!(next_total <= MAX_RAW_STRING_STORAGE_BYTES);
        let previous = self
            .values
            .set_compact(variable.register(), RawStringValue(value))
            .0;
        self.total_bytes = next_total;
        previous
    }
}

impl Default for RawStringRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl Dumpable for RawStringRegisters {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{RAW_STRING_REGISTERS_DUMP_HEADER}")?;
        let (default, dense, sparse) = self.values.parts();
        debug_assert!(default.0.is_empty());
        debug_assert_eq!(
            self.total_bytes,
            dense.iter().map(|value| value.0.len()).sum::<usize>()
                + sparse.values().map(|value| value.0.len()).sum::<usize>()
        );
        default.dump(target)?;
        dense.len().dump(target)?;
        for value in dense {
            value.dump(target)?;
        }
        sparse.len().dump(target)?;
        let mut indices: Vec<_> = sparse.keys().copied().collect();
        indices.sort_unstable();
        for index in indices {
            index.dump(target)?;
            sparse[&index].dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        if lines.next().ok_or(FormatError::IncompleteFile)? != RAW_STRING_REGISTERS_DUMP_HEADER {
            return Err(FormatError::ParseError);
        }
        let mut total_bytes = 0;
        let default = RawStringValue::undump_with_budget(lines, &mut total_bytes)?;
        if !default.0.is_empty() {
            return Err(FormatError::ParseError);
        }
        let dense_len = usize::undump(lines)?;
        if dense_len != DENSE_REGISTER_COUNT {
            return Err(FormatError::ParseError);
        }
        let mut dense = Vec::new();
        dense
            .try_reserve_exact(DENSE_REGISTER_COUNT)
            .map_err(|_| FormatError::ParseError)?;
        for _ in 0..DENSE_REGISTER_COUNT {
            dense.push(RawStringValue::undump_with_budget(lines, &mut total_bytes)?);
        }
        let sparse_len = usize::undump(lines)?;
        if sparse_len > MAX_EXTENDED_REGISTER_INDEX as usize + 1 - DENSE_REGISTER_COUNT {
            return Err(FormatError::ParseError);
        }
        let mut sparse = HashMap::new();
        sparse
            .try_reserve(sparse_len)
            .map_err(|_| FormatError::ParseError)?;
        for _ in 0..sparse_len {
            let index = u16::undump(lines)?;
            if (index as usize) < DENSE_REGISTER_COUNT || index > MAX_EXTENDED_REGISTER_INDEX {
                return Err(FormatError::ParseError);
            }
            let value = RawStringValue::undump_with_budget(lines, &mut total_bytes)?;
            if value.0.is_empty() || sparse.insert(index, value).is_some() {
                return Err(FormatError::ParseError);
            }
        }
        Ok(Self {
            values: ExtendedRegisterStorage::from_validated_parts(default, dense, sparse)?,
            total_bytes,
        })
    }
}

/// 生文字列slotのgroup level。値storageと同じ番号配置を使うがfmt sectionは分ける。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RawStringLevels {
    levels: ExtendedRegisterStorage<super::levels::Level>,
}

impl RawStringLevels {
    pub(super) fn new() -> Self {
        Self {
            levels: ExtendedRegisterStorage::new(0),
        }
    }

    pub(super) fn get(&self, variable: RawStringVariable) -> super::levels::Level {
        *self.levels.get(variable.register())
    }

    pub(super) fn set(
        &mut self,
        variable: RawStringVariable,
        level: super::levels::Level,
    ) -> super::levels::Level {
        assert!(level <= super::MAX_GROUPING_DEPTH);
        self.levels.set_compact(variable.register(), level)
    }
}

impl Default for RawStringLevels {
    fn default() -> Self {
        Self::new()
    }
}

impl Dumpable for RawStringLevels {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        writeln!(target, "{RAW_STRING_LEVELS_DUMP_HEADER}")?;
        let (default, dense, sparse) = self.levels.parts();
        debug_assert_eq!(*default, 0);
        default.dump(target)?;
        dense.len().dump(target)?;
        for level in dense {
            level.dump(target)?;
        }
        sparse.len().dump(target)?;
        let mut indices: Vec<_> = sparse.keys().copied().collect();
        indices.sort_unstable();
        for index in indices {
            index.dump(target)?;
            sparse[&index].dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        if lines.next().ok_or(FormatError::IncompleteFile)? != RAW_STRING_LEVELS_DUMP_HEADER {
            return Err(FormatError::ParseError);
        }
        let default = Level::undump(lines)?;
        if default != 0 {
            return Err(FormatError::ParseError);
        }
        if usize::undump(lines)? != DENSE_REGISTER_COUNT {
            return Err(FormatError::ParseError);
        }
        let mut dense = Vec::new();
        dense
            .try_reserve_exact(DENSE_REGISTER_COUNT)
            .map_err(|_| FormatError::ParseError)?;
        for _ in 0..DENSE_REGISTER_COUNT {
            let level = Level::undump(lines)?;
            if level > super::MAX_GROUPING_DEPTH {
                return Err(FormatError::ParseError);
            }
            dense.push(level);
        }
        let sparse_len = usize::undump(lines)?;
        if sparse_len > MAX_EXTENDED_REGISTER_INDEX as usize + 1 - DENSE_REGISTER_COUNT {
            return Err(FormatError::ParseError);
        }
        let mut sparse = HashMap::new();
        sparse
            .try_reserve(sparse_len)
            .map_err(|_| FormatError::ParseError)?;
        for _ in 0..sparse_len {
            let index = u16::undump(lines)?;
            let level = Level::undump(lines)?;
            if (index as usize) < DENSE_REGISTER_COUNT
                || index > MAX_EXTENDED_REGISTER_INDEX
                || level == 0
                || level > super::MAX_GROUPING_DEPTH
                || sparse.insert(index, level).is_some()
            {
                return Err(FormatError::ParseError);
            }
        }
        let levels = ExtendedRegisterStorage::from_validated_parts(default, dense, sparse)?;
        Ok(Self { levels })
    }
}

/// `\therawstring`用。空白、改行、NUL、UTF-8の構成byteも一切解釈しない。
pub(crate) fn raw_bytes_as_other_tokens(bytes: &[u8]) -> Vec<Token> {
    bytes.iter().copied().map(Token::OtherChar).collect()
}

/// `\the\rawstring<n>`用に、値全体を現在の分類で一度に字句化する。
///
/// rawから作ったtokenを実行しながら後半を読むpseudo-fileにはしない。LFとCRLFで
/// logical lineを分け、`boundary_character`は保存済み境界がある行だけへ足す。最終行へは
/// 暗黙に足さない。LF境界を現在の`endlinechar`へ写すか直接eventにするかは仕様決定待ちなので、
/// production callerは決定済みpolicyの値を明示する。callerは字句errorを通常入力と同じ診断へ
/// 結び、hash枯渇ならfatal overflowにする。
#[cfg(test)]
pub(crate) fn raw_bytes_as_snapshot_tokens(
    bytes: &[u8],
    boundary_character: Option<u8>,
    eqtb: &mut super::Eqtb,
    mut report_lex_error: impl FnMut(LexError, &mut super::Eqtb),
) -> Result<Vec<Token>, ()> {
    let mut tokens = Vec::new();
    let mut start = 0;
    loop {
        let end = bytes[start..]
            .iter()
            .position(|&byte| byte == b'\n')
            .map(|offset| start + offset)
            .unwrap_or(bytes.len());
        let mut logical_end = end;
        if logical_end > start && bytes[logical_end - 1] == b'\r' {
            logical_end -= 1;
        }
        let mut line = bytes[start..logical_end].to_vec();
        if end < bytes.len() {
            if let Some(character) = boundary_character {
                line.push(character);
            }
        }
        let mut lexer = LineLexer::new(line);
        loop {
            match lexer.scan_next_token_with_classifier(eqtb) {
                Ok(Some(token)) => tokens.push(token.to_token(true, eqtb)?),
                Ok(None) => break,
                Err(error) => report_lex_error(error, eqtb),
            }
        }
        if end == bytes.len() {
            break;
        }
        start = end + 1;
    }
    Ok(tokens)
}

/// `\showthe`の一つの表示atomを順に渡す。
///
/// canonical UTF-8だけを一文字単位で渡し、不正列は先頭一byteずつ`^^hh`へする。
/// callbackは一atomを途中で折り返してはならない。
pub(crate) fn for_each_raw_diagnostic_atom(mut bytes: &[u8], mut emit: impl FnMut(&[u8])) {
    while let Some((&first, rest)) = bytes.split_first() {
        if first.is_ascii() {
            if cannot_be_printed(first) {
                let escaped = if first < 64 {
                    [b'^', b'^', first + 64, 0]
                } else {
                    [b'^', b'^', first - 64, 0]
                };
                emit(&escaped[..3]);
            } else {
                emit(std::slice::from_ref(&first));
            }
            bytes = rest;
            continue;
        }

        let width = match first {
            0xC2..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF4 => 4,
            _ => 0,
        };
        if width != 0 && bytes.len() >= width {
            let candidate = &bytes[..width];
            if std::str::from_utf8(candidate)
                .ok()
                .is_some_and(|text| text.chars().count() == 1)
            {
                emit(candidate);
                bytes = &bytes[width..];
                continue;
            }
        }

        let escaped = [b'^', b'^', to_hex_char(first / 16), to_hex_char(first % 16)];
        emit(&escaped);
        bytes = rest;
    }
}

/// `\showthe`専用。raw内容を字句器・入力stack・制御綴表へ渡さない。
pub(crate) fn print_raw_diagnostic(bytes: &[u8], logger: &mut crate::logger::Logger) {
    for_each_raw_diagnostic_atom(bytes, |atom| logger.print_raw_diagnostic_atom(atom));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_dump(sparse: &[(u16, &[u8])]) -> Vec<u8> {
        let mut dump = Vec::new();
        writeln!(dump, "{RAW_STRING_REGISTERS_DUMP_HEADER}").unwrap();
        RawStringValue::empty().dump(&mut dump).unwrap();
        256_usize.dump(&mut dump).unwrap();
        for _ in 0..256 {
            RawStringValue::empty().dump(&mut dump).unwrap();
        }
        sparse.len().dump(&mut dump).unwrap();
        for (index, value) in sparse {
            index.dump(&mut dump).unwrap();
            RawStringValue(Rc::new(value.to_vec()))
                .dump(&mut dump)
                .unwrap();
        }
        dump
    }

    #[test]
    fn 値copyはrcを共有し後のslot代入から独立する() {
        let mut registers = RawStringRegisters::new();
        let source = RawStringVariable::new(7);
        let copy = RawStringVariable::new(8);
        let first = Rc::new(b"first".to_vec());
        registers.set(source, first.clone()).unwrap();
        registers.set(copy, registers.get(source).clone()).unwrap();

        assert!(Rc::ptr_eq(registers.get(source), registers.get(copy)));

        registers.set(source, Rc::new(b"second".to_vec())).unwrap();
        assert_eq!(registers.get(source).as_slice(), b"second");
        assert_eq!(registers.get(copy).as_slice(), b"first");
        assert!(!Rc::ptr_eq(registers.get(source), registers.get(copy)));
    }

    #[test]
    fn fmt往復でnul改行不正utf八と高位registerを保つ() {
        let mut before = RawStringRegisters::new();
        let low = RawStringVariable::new(0);
        let high = RawStringVariable::new(32_767);
        before
            .set(low, Rc::new(vec![0, b'\n', b'\r', 0xE3, 0x81]))
            .unwrap();
        before
            .set(high, Rc::new(vec![0xFF, b' ', b'\\', b'%']))
            .unwrap();

        let mut dumped = Vec::new();
        before.dump(&mut dumped).unwrap();
        let text = String::from_utf8(dumped).unwrap();
        let mut lines = text.lines();
        let after = RawStringRegisters::undump(&mut lines).unwrap();

        assert_eq!(after, before);
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn therawstringは空白とnulとutf八byteも全てotherにする() {
        let bytes = [b' ', 0, b'\n', 0xE3, 0x81, 0x82];
        assert_eq!(
            raw_bytes_as_other_tokens(&bytes),
            bytes.into_iter().map(Token::OtherChar).collect::<Vec<_>>()
        );
    }

    #[test]
    fn theは参照時のcatcodeとkcatcodeで字句化する() {
        use crate::eqtb::{CatCode, KCatCode};
        use crate::token::{CjkCategory, CjkToken};

        let mut eqtb = super::super::Eqtb::new();
        eqtb.cat_code_define(b'@', CatCode::Letter, true);
        let first =
            raw_bytes_as_snapshot_tokens("\\a@ あ".as_bytes(), None, &mut eqtb, |_, _| {}).unwrap();
        assert!(matches!(first[0], Token::CSToken { .. }));
        assert_eq!(
            first[1],
            Token::CjkChar(CjkToken::new(0x3042, CjkCategory::Kana).unwrap())
        );

        eqtb.cat_code_define(b'@', CatCode::OtherChar, true);
        eqtb.kcat_code_define(0x3042, KCatCode::Hangul, true);
        let second =
            raw_bytes_as_snapshot_tokens("\\a@ あ".as_bytes(), None, &mut eqtb, |_, _| {}).unwrap();
        assert!(second.len() > first.len());
        assert_eq!(second[1], Token::OtherChar(b'@'));
        assert_eq!(
            second.last(),
            Some(&Token::CjkChar(
                CjkToken::new(0x3042, CjkCategory::Hangul).unwrap()
            ))
        );
    }

    #[test]
    fn toksへ写した時点でcatcodeを固定する() {
        use crate::eqtb::CatCode;

        let mut eqtb = super::super::Eqtb::new();
        eqtb.cat_code_define(b'@', CatCode::Letter, true);
        let frozen = raw_bytes_as_snapshot_tokens(b"@", None, &mut eqtb, |_, _| {}).unwrap();
        eqtb.cat_code_define(b'@', CatCode::OtherChar, true);

        assert_eq!(frozen, vec![Token::Letter(b'@')]);
        assert_eq!(
            raw_bytes_as_snapshot_tokens(b"@", None, &mut eqtb, |_, _| {}).unwrap(),
            vec![Token::OtherChar(b'@')]
        );
    }

    #[test]
    fn showtheは改行nulと不正utf八をatomicに逃がす() {
        let bytes = [
            0, b'\n', b'\r', 0x7F, 0xC2, 0x80, 0x80, 0xC0, 0xAF, 0xED, 0xA0, 0x80, 0xE3, 0x81,
            0x82, 0xE3, 0x81, b'A',
        ];
        let mut atoms = Vec::new();
        for_each_raw_diagnostic_atom(&bytes, |atom| atoms.push(atom.to_vec()));
        assert_eq!(
            atoms,
            vec![
                b"^^@".to_vec(),
                b"^^J".to_vec(),
                b"^^M".to_vec(),
                b"^^?".to_vec(),
                vec![0xC2, 0x80],
                b"^^80".to_vec(),
                b"^^c0".to_vec(),
                b"^^af".to_vec(),
                b"^^ed".to_vec(),
                b"^^a0".to_vec(),
                b"^^80".to_vec(),
                "あ".as_bytes().to_vec(),
                b"^^e3".to_vec(),
                b"^^81".to_vec(),
                b"A".to_vec(),
            ]
        );
    }

    #[test]
    fn fmtは巨大長truncated範囲外byteを拒否する() {
        for malformed in [
            format!("{}\n", MAX_RAW_STRING_BYTES + 1),
            "2\n65\n".to_string(),
            "1\n256\n".to_string(),
        ] {
            assert!(matches!(
                RawStringValue::undump(&mut malformed.lines()),
                Err(FormatError::IncompleteFile | FormatError::ParseError)
            ));
        }
    }

    #[test]
    fn fmtは全storage予算をallocation前の逐次加算で拒否する() {
        let mut at_limit = MAX_RAW_STRING_STORAGE_BYTES;
        assert!(matches!(
            RawStringValue::undump_with_budget(&mut "1\n65\n".lines(), &mut at_limit),
            Err(FormatError::ParseError)
        ));

        let mut overflow = usize::MAX;
        assert!(matches!(
            RawStringValue::undump_with_budget(&mut "1\n65\n".lines(), &mut overflow),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn fmtは範囲外slotとduplicate_sparseを拒否する() {
        let mut out_of_range = minimal_dump(&[]);
        let sparse_count_offset = out_of_range.len() - "0\n".len();
        out_of_range.truncate(sparse_count_offset);
        out_of_range.extend_from_slice(b"1\n32768\n0\n");
        let text = String::from_utf8(out_of_range).unwrap();
        assert!(matches!(
            RawStringRegisters::undump(&mut text.lines()),
            Err(FormatError::ParseError)
        ));

        let duplicate = minimal_dump(&[(256, b"a"), (256, b"b")]);
        let text = String::from_utf8(duplicate).unwrap();
        assert!(matches!(
            RawStringRegisters::undump(&mut text.lines()),
            Err(FormatError::ParseError)
        ));

        let empty_sparse = minimal_dump(&[(256, b"")]);
        let text = String::from_utf8(empty_sparse).unwrap();
        assert!(matches!(
            RawStringRegisters::undump(&mut text.lines()),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn fmtは非空の既定値を拒否する() {
        let valid = minimal_dump(&[]);
        let header_len = format!("{RAW_STRING_REGISTERS_DUMP_HEADER}\n").len();
        let mut malformed = valid[..header_len].to_vec();
        malformed.extend_from_slice(b"1\n65\n");
        malformed.extend_from_slice(&valid[header_len + "0\n".len()..]);
        let text = String::from_utf8(malformed).unwrap();
        assert!(matches!(
            RawStringRegisters::undump(&mut text.lines()),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 全storage上限はrc共有でもslot長の合計で守る() {
        let mut registers = RawStringRegisters::new();
        let shared = Rc::new(vec![b'x'; MAX_RAW_STRING_BYTES]);
        for register in 0..4 {
            registers
                .set(RawStringVariable::new(register), shared.clone())
                .unwrap();
        }
        assert_eq!(registers.total_bytes, MAX_RAW_STRING_STORAGE_BYTES);
        assert_eq!(
            registers.set(RawStringVariable::new(4), shared),
            Err(RawStringStorageError::StorageTooLarge)
        );
    }

    #[test]
    fn active_future_envelopeは対象最大値と他slot予約を合算する() {
        let registers = RawStringRegisters::new();
        let target = RawStringVariable::new(0);
        let other_restore_bytes = MAX_RAW_STRING_STORAGE_BYTES - MAX_RAW_STRING_BYTES;
        assert_eq!(
            registers.can_set_with_restore_budget(
                target,
                &[],
                other_restore_bytes,
                MAX_RAW_STRING_BYTES,
            ),
            Ok(())
        );
        assert_eq!(
            registers.can_set_with_restore_budget(
                target,
                &[],
                other_restore_bytes + 1,
                MAX_RAW_STRING_BYTES,
            ),
            Err(RawStringStorageError::StorageTooLarge)
        );
        // global定義後を表すtarget restore 0なら、そのslotの予約分は解消される。
        assert_eq!(
            registers.can_set_with_restore_budget(target, &[], MAX_RAW_STRING_STORAGE_BYTES, 0,),
            Ok(())
        );
    }

    #[test]
    fn 高位slotへ空値を戻すと疎entryを残さない() {
        let mut registers = RawStringRegisters::new();
        let variable = RawStringVariable::new(32_767);
        registers.set(variable, Rc::new(b"used".to_vec())).unwrap();
        assert_eq!(registers.values.parts().2.len(), 1);
        registers.set(variable, Rc::new(Vec::new())).unwrap();
        assert!(registers.values.parts().2.is_empty());
    }

    #[test]
    fn raw_levelは高位零をcompactにしfmtを往復する() {
        let mut before = RawStringLevels::new();
        let low = RawStringVariable::new(7);
        let high = RawStringVariable::new(32_767);
        assert_eq!(before.set(low, 2), 0);
        assert_eq!(before.set(high, 3), 0);
        assert_eq!(before.set(high, 0), 3);
        assert!(before.levels.parts().2.is_empty());

        let mut dumped = Vec::new();
        before.dump(&mut dumped).unwrap();
        let text = String::from_utf8(dumped).unwrap();
        let mut lines = text.lines();
        let after = RawStringLevels::undump(&mut lines).unwrap();
        assert_eq!(after, before);
        assert_eq!(after.get(low), 2);
        assert_eq!(after.get(high), 0);
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn raw_level_fmtは非零defaultと零の疎entryを拒否する() {
        let mut nonzero_default = Vec::new();
        writeln!(nonzero_default, "{RAW_STRING_LEVELS_DUMP_HEADER}").unwrap();
        let levels = ExtendedRegisterStorage::new(1_usize);
        levels.dump(&mut nonzero_default).unwrap();
        let text = String::from_utf8(nonzero_default).unwrap();
        assert!(matches!(
            RawStringLevels::undump(&mut text.lines()),
            Err(FormatError::ParseError)
        ));

        let mut zero_sparse = Vec::new();
        writeln!(zero_sparse, "{RAW_STRING_LEVELS_DUMP_HEADER}").unwrap();
        let mut levels = ExtendedRegisterStorage::new(0_usize);
        levels.set(256, 0);
        levels.dump(&mut zero_sparse).unwrap();
        let text = String::from_utf8(zero_sparse).unwrap();
        assert!(matches!(
            RawStringLevels::undump(&mut text.lines()),
            Err(FormatError::ParseError)
        ));

        let huge_dense = format!("{RAW_STRING_LEVELS_DUMP_HEADER}\n0\n{}\n", usize::MAX);
        assert!(matches!(
            RawStringLevels::undump(&mut huge_dense.lines()),
            Err(FormatError::ParseError)
        ));

        let dense = vec!["0"; DENSE_REGISTER_COUNT].join("\n");
        let huge_sparse = format!(
            "{RAW_STRING_LEVELS_DUMP_HEADER}\n0\n{DENSE_REGISTER_COUNT}\n{dense}\n{}\n",
            usize::MAX
        );
        assert!(matches!(
            RawStringLevels::undump(&mut huge_sparse.lines()),
            Err(FormatError::ParseError)
        ));
    }
}

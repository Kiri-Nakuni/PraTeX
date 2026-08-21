use crate::eqtb::{CatCode, ControlSequence, ControlSequenceNameUnit, Eqtb, KCatCode};
use crate::token::{CjkCategory, CjkToken, Token, decode_uptex_input_code_point};

/// Scans an input line and produces tokens.
///
/// The produced tokens are quite primitive and have not yet resolved
/// the command words, command symbols and active characters.
///
/// See 303.
#[derive(Debug)]
pub struct LineLexer {
    /// Indicates whether we are ignoring spaces or are at the beginning of a
    /// new line.
    state: LineLexerState,
    /// The index of the current character.
    /// Note: this is called `loc` in the TeX82 code.
    pos: usize,
    /// The current line of input.
    /// Note: In TeX82 this is stored in the `buffer` array. Because
    /// we know the size of this line, we also no longer need the TeX82
    /// variables `start` and `limit` that were used as indices into `buffer`.
    line: Vec<u8>,
}

/// Determines how whitespace is dealt with.
/// See 303.
#[derive(Debug, Clone, Copy)]
enum LineLexerState {
    /// The default value
    Midline,
    /// In this state we are ignoring spaces
    SkipBlanks,
    /// The state when we just started a new line
    NewLine,
}

impl LineLexer {
    #[inline]
    pub const fn new(line: Vec<u8>) -> Self {
        Self {
            state: LineLexerState::NewLine,
            pos: 0,
            line,
        }
    }

    #[inline]
    pub const fn new_with_pos(line: Vec<u8>, pos: usize) -> Self {
        Self {
            state: LineLexerState::NewLine,
            pos,
            line,
        }
    }

    #[inline]
    pub const fn new_midline(line: Vec<u8>) -> Self {
        Self {
            state: LineLexerState::Midline,
            pos: 0,
            line,
        }
    }

    /// Returns true if the whole line has been lexed.
    pub fn is_finished(&self) -> bool {
        self.pos >= self.line.len()
    }

    pub fn line_len(&self) -> usize {
        self.line.len()
    }

    /// Try to get the next token from the current line and change state if necessary.
    /// Returns a [`LexerToken`] if there is one, `None` if the line has been depleted, or
    /// an `Err` if there has been an invalid char.
    /// See 344., 345., 347., 348., 349., 350., 351., and 353.
    pub fn scan_next_token(
        &mut self,
        cat_code: &impl Fn(u8) -> CatCode,
        kcat_code: &impl Fn(u32) -> KCatCode,
    ) -> Result<Option<LexerToken<'_>>, LexError> {
        loop {
            // Get the next unexpanded character or return empty-handed.
            let input = match self.next_unexpanded_input(cat_code, kcat_code) {
                None => return Ok(None),
                Some(val) => val,
            };
            let (chr, cat, next_pos) = match input {
                UnexpandedInput::Byte(chr, cat, next_pos) => (chr, cat, next_pos),
                UnexpandedInput::Cjk(token, next_pos) => {
                    self.pos = next_pos;
                    self.state = LineLexerState::Midline;
                    return Ok(Some(LexerToken::CjkChar(token)));
                }
            };
            self.pos = next_pos;

            // Create a token from the character, potentially consuming more in the process,
            // and change state if necessary.
            use CatCode::*;
            use LineLexerState::{Midline, NewLine, SkipBlanks};
            let token = match (self.state, cat) {
                (_, Letter) => {
                    self.state = Midline;
                    LexerToken::Letter(chr)
                }
                // A space while not skipping spaces.
                (Midline, Spacer) => {
                    self.state = LineLexerState::SkipBlanks;
                    // All Spacers are treated as if they were space characters.
                    LexerToken::Spacer
                }
                // If we ignore the current character.
                (_, Ignore) | (SkipBlanks, Spacer) | (NewLine, Spacer) => continue,

                (_, OtherChar) => {
                    self.state = Midline;
                    LexerToken::OtherChar(chr)
                }
                // An active char.
                (_, ActiveChar) => {
                    // Don't ignore following spaces.
                    self.state = LineLexerState::Midline;
                    LexerToken::ActiveChar(chr)
                }
                // A control sequence.
                (_, Escape) => self.scan_control_sequence(cat_code, kcat_code),

                // **名前空間の印。** ここから名前空間つきの制御綴が始まる
                (_, Namespace) => self.scan_namespaced(cat_code, kcat_code)?,

                // An end-of-line character while not skipping spaces.
                (Midline, CarRet) => {
                    // Skip rest of line.
                    self.pos = self.line.len();
                    // Treat end of line like a single space
                    LexerToken::Spacer
                }
                // An end-of-line character while skipping spaces or the start of a comment.
                (SkipBlanks, CarRet) | (_, Comment) => {
                    self.pos = self.line.len();
                    return Ok(None);
                }
                // An end-of-line character while still at the beginning of line.
                (NewLine, CarRet) => {
                    // Finish line.
                    self.pos = self.line.len();
                    LexerToken::Par
                }

                // All other cases
                (_, LeftBrace) => {
                    self.state = Midline;
                    LexerToken::LeftBrace(chr)
                }
                (_, RightBrace) => {
                    self.state = Midline;
                    LexerToken::RightBrace(chr)
                }
                (_, MathShift) => {
                    self.state = Midline;
                    LexerToken::MathShift(chr)
                }
                (_, TabMark) => {
                    self.state = Midline;
                    LexerToken::TabMark(chr)
                }
                (_, MacParam) => {
                    self.state = Midline;
                    LexerToken::MacParam(chr)
                }
                (_, SupMark) => {
                    self.state = Midline;
                    LexerToken::SuperMark(chr)
                }
                (_, SubMark) => {
                    self.state = Midline;
                    LexerToken::SubMark(chr)
                }
                // An invalid char.
                (_, InvalidChar) => {
                    return Err(LexError::InvalidChar);
                }
            };
            return Ok(Some(token));
        }
    }

    /// Returns the next input unit without charging the ASCII path for a
    /// `\kcatcode` lookup.
    ///
    /// Literal non-ASCII bytes are decoded before tokenization.  `^^` still
    /// goes through the byte routine below, so an expanded byte is never
    /// mistaken for the first byte of a UTF-8 sequence.
    #[inline(always)]
    fn next_unexpanded_input(
        &mut self,
        cat_code: &impl Fn(u8) -> CatCode,
        kcat_code: &impl Fn(u32) -> KCatCode,
    ) -> Option<UnexpandedInput> {
        if let Some((token, next_pos)) = self.literal_cjk_at_current_position(kcat_code) {
            return Some(UnexpandedInput::Cjk(token, next_pos));
        }
        self.next_unexpanded_character(cat_code)
            .map(|(chr, cat, next_pos)| UnexpandedInput::Byte(chr, cat, next_pos))
    }

    /// The replacement variant used while a control-sequence name is being
    /// scanned.  Keeping the byte implementation intact preserves borrowed
    /// ASCII names and TeX82's diagnostic-line replacement for `^^` notation.
    #[inline(always)]
    fn next_unexpanded_input_with_replacement(
        &mut self,
        cat_code: &impl Fn(u8) -> CatCode,
        kcat_code: &impl Fn(u32) -> KCatCode,
    ) -> Option<UnexpandedInput> {
        if let Some((token, next_pos)) = self.literal_cjk_at_current_position(kcat_code) {
            return Some(UnexpandedInput::Cjk(token, next_pos));
        }
        self.next_unexpanded_character_with_replacement(cat_code)
            .map(|(chr, cat, next_pos)| UnexpandedInput::Byte(chr, cat, next_pos))
    }

    #[inline(always)]
    fn literal_cjk_at_current_position(
        &self,
        kcat_code: &impl Fn(u32) -> KCatCode,
    ) -> Option<(CjkToken, usize)> {
        if self.line.get(self.pos).copied()?.is_ascii() {
            return None;
        }
        let (code_point, len) = decode_uptex_input_code_point(&self.line[self.pos..])?;
        let category = match kcat_code(code_point) {
            KCatCode::Kanji => CjkCategory::Kanji,
            KCatCode::Kana => CjkCategory::Kana,
            KCatCode::OtherKChar => CjkCategory::OtherKChar,
            KCatCode::Hangul => CjkCategory::Hangul,
            KCatCode::Modifier => CjkCategory::Modifier,
            // Stage 4c will turn `LatinUcs` into a single Unicode European
            // token.  Until then both non-CJK routes deliberately retain the
            // original bytes and their ordinary 8-bit catcodes.
            KCatCode::LatinUcs | KCatCode::NotCjk => return None,
        };
        let token = CjkToken::new(code_point, category)
            .expect("the decoder only returns code points accepted by CjkToken");
        Some((token, self.pos + len))
    }

    /// Returns the next unexpanded character, its catcode, and the position just after it.
    /// Or None if the line has ended.
    /// See 352 and 355.
    #[inline(always)]
    fn next_unexpanded_character(
        &mut self,
        cat_code: &impl Fn(u8) -> CatCode,
    ) -> Option<(u8, CatCode, usize)> {
        if self.pos >= self.line.len() {
            return None;
        }
        let mut chr = self.line[self.pos];
        let mut pos = self.pos + 1;
        loop {
            let cat = cat_code(chr);
            // If the next character is a "sup_mark" and is followed by the same character and
            // then an ASCII character, we have an expanded character.
            if cat == CatCode::SupMark
                && pos + 1 < self.line.len()
                && self.line[pos] == chr
                && self.line[pos + 1].is_ascii()
            {
                let c = self.line[pos + 1];
                pos += 2;
                // Could this be a hex code?
                // Do we have a second character?
                // Is that one also a hex character?
                // Then we have a hex expanded character (like ^^0d).
                if is_hex(c) && pos < self.line.len() && is_hex(self.line[pos]) {
                    let cc = self.line[pos];
                    pos += 1;
                    chr = double_hex_to_byte(c, cc);
                // We have a traditionally expanded character (like ^^M).
                } else {
                    chr = if c < 64 { c + 64 } else { c - 64 };
                }
            } else {
                return Some((chr, cat, pos));
            }
        }
    }

    /// Returns the next unexpanded character, its catcode, and the position just after it.
    /// Or None if the line has ended.
    /// In addition it changes the current line by substituting the escape sequence with the
    /// unexpanded character.
    /// NOTE This version is needed both to allow us to use a reference for
    /// `LexerToken::CommandWord` and to have the exact same output as TeX82 when the current line
    /// is printed in diagnostic messages.
    /// See 352 and 355.
    #[inline(always)]
    fn next_unexpanded_character_with_replacement(
        &mut self,
        cat_code: &impl Fn(u8) -> CatCode,
    ) -> Option<(u8, CatCode, usize)> {
        if self.pos >= self.line.len() {
            return None;
        }
        let mut chr = self.line[self.pos];
        loop {
            let mut pos = self.pos + 1;
            let cat = cat_code(chr);
            // If the next character is a "sup_mark" and is followed by the same character and
            // then an ASCII character, we have an expanded character.
            if cat == CatCode::SupMark
                && pos + 1 < self.line.len()
                && self.line[pos] == chr
                && self.line[pos + 1].is_ascii()
            {
                let c = self.line[pos + 1];
                pos += 2;
                // Could this be a hex code?
                // Do we have a second character?
                // Is that one also a hex character?
                // Then we have a hex expanded character (like ^^0d).
                if is_hex(c) && pos < self.line.len() && is_hex(self.line[pos]) {
                    let cc = self.line[pos];
                    chr = double_hex_to_byte(c, cc);
                    self.line.drain(self.pos..self.pos + 3);
                    self.line[self.pos] = chr;
                // We have a traditionally expanded character (like ^^M).
                } else {
                    chr = if c < 64 { c + 64 } else { c - 64 };
                    self.line.drain(self.pos..self.pos + 2);
                    self.line[self.pos] = chr;
                }
            } else {
                return Some((chr, cat, pos));
            }
        }
    }

    /// Creates a control sequence token from the input.
    /// See 354 and 356.
    fn scan_control_sequence(
        &mut self,
        cat_code: &impl Fn(u8) -> CatCode,
        kcat_code: &impl Fn(u32) -> KCatCode,
    ) -> LexerToken<'_> {
        match self.scan_cs_name(cat_code, kcat_code) {
            CsName::Empty => LexerToken::CommandWord(&[]),
            CsName::Symbol(c) => LexerToken::CommandSymbol(c),
            CsName::Word(start, end) => LexerToken::CommandWord(&self.line[start..end]),
            CsName::Wide(name) => LexerToken::WideCommand(name),
        }
    }

    /// 制御綴の名前を走査し、**範囲で返す。**
    ///
    /// `\hoge` の側と `*ns\hoge` の側で**同じものを使う**——
    /// `^^` 置換の扱いが揃うことが**構造的に**保証される。
    fn scan_cs_name(
        &mut self,
        cat_code: &impl Fn(u8) -> CatCode,
        kcat_code: &impl Fn(u32) -> KCatCode,
    ) -> CsName {
        match self.next_unexpanded_input_with_replacement(cat_code, kcat_code) {
            // If there are no more characters.
            None => CsName::Empty,
            Some(UnexpandedInput::Byte(c, cat, next_pos)) => {
                let start = self.pos;
                self.pos = next_pos;
                match cat {
                    // This could be a single-letter or multi-letter control sequence.
                    CatCode::Letter => {
                        self.state = LineLexerState::SkipBlanks;
                        loop {
                            match self.next_unexpanded_input_with_replacement(cat_code, kcat_code) {
                                Some(UnexpandedInput::Byte(_, CatCode::Letter, next_pos)) => {
                                    self.pos = next_pos;
                                }
                                Some(UnexpandedInput::Cjk(token, next_pos))
                                    if cjk_can_continue_word(token.category()) =>
                                {
                                    let mut name = self.line[start..self.pos]
                                        .iter()
                                        .copied()
                                        .map(ControlSequenceNameUnit::Byte)
                                        .collect::<Vec<_>>();
                                    name.push(ControlSequenceNameUnit::Unicode(token.code_point()));
                                    self.pos = next_pos;
                                    self.scan_wide_word_tail(&mut name, cat_code, kcat_code);
                                    return CsName::Wide(name);
                                }
                                _ => break,
                            }
                        }
                        let end = self.pos;
                        if end - start > 1 {
                            CsName::Word(start, end)
                        } else {
                            CsName::Symbol(self.line[start])
                        }
                    }
                    // We want to ignore spaces following a command like "\ "
                    CatCode::Spacer => {
                        self.state = LineLexerState::SkipBlanks;
                        CsName::Symbol(c)
                    }
                    // For other non-letter control symbols, we don't want to ignore following
                    // spaces.
                    _ => {
                        self.state = LineLexerState::Midline;
                        CsName::Symbol(c)
                    }
                }
            }
            Some(UnexpandedInput::Cjk(token, next_pos)) => {
                self.pos = next_pos;
                let category = token.category();
                let mut name = vec![ControlSequenceNameUnit::Unicode(token.code_point())];
                match category {
                    CjkCategory::OtherKChar => {
                        self.state = LineLexerState::Midline;
                    }
                    CjkCategory::Modifier => {
                        let added = self.scan_wide_word_tail(&mut name, cat_code, kcat_code);
                        self.state = if added == 0 {
                            LineLexerState::Midline
                        } else {
                            LineLexerState::SkipBlanks
                        };
                    }
                    CjkCategory::Kanji | CjkCategory::Kana | CjkCategory::Hangul => {
                        self.scan_wide_word_tail(&mut name, cat_code, kcat_code);
                        self.state = LineLexerState::SkipBlanks;
                    }
                }
                CsName::Wide(name)
            }
        }
    }

    /// Continue a control word after it has been promoted to a typed, owned
    /// name.  European byte letters and the four word-like CJK categories can
    /// be freely mixed; kcatcode 18 terminates the name.
    fn scan_wide_word_tail(
        &mut self,
        name: &mut Vec<ControlSequenceNameUnit>,
        cat_code: &impl Fn(u8) -> CatCode,
        kcat_code: &impl Fn(u32) -> KCatCode,
    ) -> usize {
        let initial_len = name.len();
        loop {
            match self.next_unexpanded_input_with_replacement(cat_code, kcat_code) {
                Some(UnexpandedInput::Byte(c, CatCode::Letter, next_pos)) => {
                    name.push(ControlSequenceNameUnit::Byte(c));
                    self.pos = next_pos;
                }
                Some(UnexpandedInput::Cjk(token, next_pos))
                    if cjk_can_continue_word(token.category()) =>
                {
                    name.push(ControlSequenceNameUnit::Unicode(token.code_point()));
                    self.pos = next_pos;
                }
                _ => break,
            }
        }
        name.len() - initial_len
    }

    /// 名前空間つきの制御綴を走査する。
    ///
    /// 名前空間の印（catcode 16）を読んだ直後に呼ばれる。
    ///
    /// # 受理の判定
    ///
    /// **catcode を三つに分けるだけ**である。
    ///
    /// | catcode | |
    /// |---|---|
    /// | 0（escape）/ 13（active） | **終端。** ここから先が対象の制御綴 |
    /// | 15（invalid） | 誤り |
    /// | 5 / 9 / 10（行末・無視・空白）と行の終わり | **runaway** |
    /// | それ以外 | **名前に取り込む** |
    ///
    /// 名前空間の印そのものは「それ以外」に入るので取り込まれる——
    /// `*a*b\hoge` は `a*b` の `hoge` である。**階層ではない。**
    fn scan_namespaced(
        &mut self,
        cat_code: &impl Fn(u8) -> CatCode,
        kcat_code: &impl Fn(u32) -> KCatCode,
    ) -> Result<LexerToken<'_>, LexError> {
        let ns_start = self.pos;
        let ns_end;
        let term_cat;
        let term_chr;
        loop {
            let Some((c, cat, next_pos)) =
                self.next_unexpanded_character_with_replacement(cat_code)
            else {
                return Err(LexError::RunawayNamespace);
            };
            match cat {
                CatCode::Escape | CatCode::ActiveChar => {
                    ns_end = self.pos;
                    self.pos = next_pos;
                    term_cat = cat;
                    term_chr = c;
                    break;
                }
                CatCode::InvalidChar => return Err(LexError::InvalidChar),
                // **名前の途中で行が終わることを許さない。**
                // 空白も同じ——名前空間の名前は一続きでなければならない
                CatCode::Spacer | CatCode::CarRet | CatCode::Ignore => {
                    return Err(LexError::RunawayNamespace);
                }
                _ => self.pos = next_pos,
            }
        }
        if let CatCode::ActiveChar = term_cat {
            // 活性文字はそれ自身が対象である。**`escapechar` を挟まない**
            self.state = LineLexerState::Midline;
            return Ok(LexerToken::NamespacedActive(
                &self.line[ns_start..ns_end],
                term_chr,
            ));
        }
        Ok(match self.scan_cs_name(cat_code, kcat_code) {
            // **空は global へ落ちる。** 統一規則（決定事項）
            CsName::Empty => LexerToken::CommandWord(&[]),
            CsName::Symbol(c) => {
                let (ns, _) = split_two(&self.line, ns_start, ns_end, 0, 0);
                LexerToken::NamespacedSymbol(ns, c)
            }
            CsName::Word(s, e) => {
                // 二つの借りを**同時に**取る。どちらも `self.line` からの写しでない借り
                let (ns, name) = split_two(&self.line, ns_start, ns_end, s, e);
                LexerToken::NamespacedWord(ns, name)
            }
            CsName::Wide(name) => LexerToken::NamespacedWide(&self.line[ns_start..ns_end], name),
        })
    }

    /// See 318.
    pub fn get_read_and_unread_parts_of_line(&self, end_line_char: i32) -> (&[u8], &[u8]) {
        let mut line = &self.line[..];
        // If an endlinechar has been added, ignore it.
        if let Some(&c) = self.line.last() {
            if c as i32 == end_line_char {
                line = &line[..line.len() - 1];
            }
        }
        if self.pos >= line.len() {
            (line, &[] as &[u8])
        } else {
            (&line[..self.pos], &line[self.pos..])
        }
    }
}

#[inline]
fn cjk_can_continue_word(category: CjkCategory) -> bool {
    !matches!(category, CjkCategory::OtherKChar)
}

/// Returns true if the given character is a hex digit.
fn is_hex(c: u8) -> bool {
    c.is_ascii_digit() || (c.is_ascii_hexdigit() && c.is_ascii_lowercase())
}

/// Creates a number from a single hex digit.
fn hex_to_number(c: u8) -> u8 {
    if c <= b'9' { c - b'0' } else { c - b'a' + 10 }
}

/// Creates a number from two hex digits.
fn double_hex_to_byte(c: u8, cc: u8) -> u8 {
    let c = hex_to_number(c);
    let cc = hex_to_number(cc);
    16 * c + cc
}

#[derive(Debug, Clone, Copy)]
enum UnexpandedInput {
    Byte(u8, CatCode, usize),
    Cjk(CjkToken, usize),
}

/// A token as returned from [`LineLexer`].
#[derive(Debug)]
/// 制御綴の名前の走査結果。
///
/// byteだけなら範囲を返して借用lookupを保ち、Unicodeが現れた時だけ
/// typedな所有名へ昇格する。
enum CsName {
    Empty,
    Symbol(u8),
    Word(usize, usize),
    Wide(Vec<ControlSequenceNameUnit>),
}

/// 二つの範囲を同時に借りる。
fn split_two(line: &[u8], a0: usize, a1: usize, b0: usize, b1: usize) -> (&[u8], &[u8]) {
    (&line[a0..a1], &line[b0..b1])
}

/// 字句層の誤り。
///
/// **今まで一本しか無かった。** 名前空間の runaway を運べるように分けた。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexError {
    InvalidChar,
    /// 名前空間の名前が閉じないまま行が終わった／空白が来た
    RunawayNamespace,
}

pub enum LexerToken<'a> {
    LeftBrace(u8),
    RightBrace(u8),
    MathShift(u8),
    TabMark(u8),
    MacParam(u8),
    SuperMark(u8),
    SubMark(u8),
    /// A spacer is always treated like a space internally.
    Spacer,
    Letter(u8),
    OtherChar(u8),
    CjkChar(CjkToken),
    ActiveChar(u8),
    CommandSymbol(u8),
    CommandWord(&'a [u8]),
    /// A control-sequence name containing at least one decoded Unicode unit.
    WideCommand(Vec<ControlSequenceNameUnit>),
    /// End-of-paragraph token
    Par,
    /// `*ns\hoge` — （名前空間名, 制御綴名）
    NamespacedWord(&'a [u8], &'a [u8]),
    /// `*ns\!` — （名前空間名, 一文字の制御綴）
    NamespacedSymbol(&'a [u8], u8),
    /// A namespaced target name containing at least one Unicode unit.
    NamespacedWide(&'a [u8], Vec<ControlSequenceNameUnit>),
    /// `*ns~` — （名前空間名, 活性文字）
    NamespacedActive(&'a [u8], u8),
}

impl<'a> LexerToken<'a> {
    pub fn to_token(self, allow_new_cs: bool, eqtb: &mut Eqtb) -> Result<Token, ()> {
        use LexerToken::*;
        let token = match self {
            LeftBrace(c) => Token::LeftBrace(c),
            RightBrace(c) => Token::RightBrace(c),
            MathShift(c) => Token::MathShift(c),
            TabMark(c) => Token::TabMark(c),
            MacParam(c) => Token::MacParam(c),
            SuperMark(c) => Token::SuperMark(c),
            SubMark(c) => Token::SubMark(c),
            Spacer => Token::Spacer(b' '),
            Letter(c) => Token::Letter(c),
            OtherChar(c) => Token::OtherChar(c),
            CjkChar(token) => Token::CjkChar(token),
            // **一文字と活性文字も探索に参加する**（`\usingnamespace`）。
            // 使っている名前空間が無ければ `Active(c)` / `Single(c)` そのものになる
            ActiveChar(c) => Token::CSToken {
                cs: eqtb.lookup_active(c),
            },
            CommandSymbol(c) => Token::CSToken {
                cs: eqtb.lookup_symbol(c),
            },
            CommandWord([]) => Token::CSToken {
                cs: ControlSequence::NullCs,
            },
            CommandWord(name) => Token::CSToken {
                cs: if allow_new_cs {
                    eqtb.lookup_or_create(name)?
                } else {
                    match eqtb.lookup(name) {
                        Some(cs) => cs,
                        None => ControlSequence::Undefined,
                    }
                },
            },
            WideCommand(name) => Token::CSToken {
                cs: if allow_new_cs {
                    eqtb.lookup_or_create_wide(&name)?
                } else {
                    eqtb.lookup_wide(&name)
                        .unwrap_or(ControlSequence::Undefined)
                },
            },
            // **名前空間つき。** 名前空間の名前を番号に直してから引く
            NamespacedWord(ns, name) => Token::CSToken {
                cs: lookup_namespaced(ns, name, None, allow_new_cs, eqtb)?,
            },
            NamespacedSymbol(ns, c) => Token::CSToken {
                cs: lookup_namespaced(ns, &[c], None, allow_new_cs, eqtb)?,
            },
            NamespacedWide(ns, name) => Token::CSToken {
                cs: lookup_namespaced_wide(ns, &name, allow_new_cs, eqtb)?,
            },
            // 活性文字は**その文字自身が名前である。** 種別は store が覚える
            NamespacedActive(ns, c) => Token::CSToken {
                cs: lookup_namespaced(ns, &[c], Some(c), allow_new_cs, eqtb)?,
            },
            Par => eqtb.par_token,
        };
        Ok(token)
    }
}

/// Unicode単位を含む名前空間つき制御綴を引く（無ければ作る）。
fn lookup_namespaced_wide(
    ns: &[u8],
    name: &[ControlSequenceNameUnit],
    allow_new_cs: bool,
    eqtb: &mut Eqtb,
) -> Result<ControlSequence, ()> {
    if ns.is_empty() {
        return if allow_new_cs {
            eqtb.lookup_or_create_wide(name)
        } else {
            Ok(eqtb.lookup_wide(name).unwrap_or(ControlSequence::Undefined))
        };
    }
    let id = eqtb.control_sequences.intern_namespace(ns);
    if allow_new_cs {
        eqtb.lookup_or_create_ns_wide(Some(id), name)
    } else {
        Ok(eqtb
            .lookup_ns_wide(Some(id), name)
            .unwrap_or(ControlSequence::Undefined))
    }
}

/// 名前空間つきの制御綴を引く（無ければ作る）。
///
/// **名前空間の名前は、引くだけでも番号にする。** 番号の表は
/// 定義とは無関係に伸びるので、`\ifx` の比較が安定する。
fn lookup_namespaced(
    ns: &[u8],
    name: &[u8],
    active: Option<u8>,
    allow_new_cs: bool,
    eqtb: &mut Eqtb,
) -> Result<ControlSequence, ()> {
    // **空の名前空間名は global そのものである**（仕様どおり）
    if ns.is_empty() {
        return if allow_new_cs {
            eqtb.lookup_or_create(name)
        } else {
            Ok(eqtb.lookup(name).unwrap_or(ControlSequence::Undefined))
        };
    }
    let id = eqtb.control_sequences.intern_namespace(ns);
    if allow_new_cs {
        eqtb.lookup_or_create_ns(Some(id), name, active)
    } else {
        Ok(eqtb
            .lookup_ns_kind(Some(id), active.is_some(), name)
            .unwrap_or(ControlSequence::Undefined))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const HIRAGANA_A: &[u8] = &[0xE3, 0x81, 0x82];

    fn ordinary_cat_code(byte: u8) -> CatCode {
        match byte {
            b'\\' => CatCode::Escape,
            b'*' => CatCode::Namespace,
            b' ' => CatCode::Spacer,
            b'^' => CatCode::SupMark,
            b'A'..=b'Z' | b'a'..=b'z' => CatCode::Letter,
            _ => CatCode::OtherChar,
        }
    }

    fn cjk_line(prefix: &[u8], suffix: &[u8]) -> Vec<u8> {
        let mut line = Vec::with_capacity(prefix.len() + HIRAGANA_A.len() + suffix.len());
        line.extend_from_slice(prefix);
        line.extend_from_slice(HIRAGANA_A);
        line.extend_from_slice(suffix);
        line
    }

    fn assert_cjk_token(token: LexerToken<'_>, code_point: u32, category: CjkCategory) {
        let LexerToken::CjkChar(token) = token else {
            panic!("CJK token expected");
        };
        assert_eq!(token.code_point(), code_point);
        assert_eq!(token.category(), category);
    }

    #[test]
    fn 公開処理系の非正規utf8境界を復号する() {
        for (bytes, expected) in [
            (&[0xC2, 0x80][..], (0x80, 2)),
            (&[0xDF, 0xBF][..], (0x7FF, 2)),
            (&[0xE0, 0x80, 0x81][..], (1, 3)),
            (&[0xE0, 0x81, 0xBF][..], (0x7F, 3)),
            (&[0xED, 0xA0, 0x80][..], (0xD800, 3)),
            (&[0xED, 0xBF, 0xBF][..], (0xDFFF, 3)),
            (&[0xF0, 0x80, 0x80, 0x81][..], (1, 4)),
            (&[0xF0, 0x80, 0x81, 0xBF][..], (0x7F, 4)),
            (&[0xF4, 0x8F, 0xBF, 0xBE][..], (0x10_FFFE, 4)),
        ] {
            assert_eq!(decode_uptex_input_code_point(bytes), Some(expected));
        }

        for bytes in [
            &[0x80][..],
            &[0xC0, 0xAF][..],
            &[0xC2][..],
            &[0xE3, 0x81][..],
            &[0xE3, 0x28, 0xA1][..],
            &[0xE0, 0x80, 0x80][..],
            &[0xF0, 0x80, 0x80, 0x80][..],
            &[0xF4, 0x8F, 0xBF, 0xBF][..],
            &[0xF4, 0x90, 0x80, 0x80][..],
            &[0xF5, 0x80, 0x80, 0x80][..],
        ] {
            assert_eq!(decode_uptex_input_code_point(bytes), None, "{bytes:02X?}");
        }
    }

    #[test]
    fn 不正列は先頭一byteだけ戻して次のleadへ再同期する() {
        let cat_code = |_| CatCode::OtherChar;
        let kcat_code = |_| KCatCode::OtherKChar;
        for (line, raw_prefix) in [
            (&[0xE3, 0xC2, 0xA2][..], &[0xE3][..]),
            (&[0xE3, 0x81, 0xC2, 0xA2][..], &[0xE3, 0x81][..]),
        ] {
            let mut lexer = LineLexer::new(line.to_vec());
            for expected in raw_prefix {
                assert!(matches!(
                    lexer.scan_next_token(&cat_code, &kcat_code),
                    Ok(Some(LexerToken::OtherChar(actual))) if actual == *expected
                ));
            }
            let token = lexer
                .scan_next_token(&cat_code, &kcat_code)
                .unwrap()
                .unwrap();
            assert_cjk_token(token, 0xA2, CjkCategory::OtherKChar);
        }
    }

    #[test]
    fn asciiと二重上付き記法はkcat表を引かない() {
        let calls = Cell::new(0);
        let kcat_code = |_| {
            calls.set(calls.get() + 1);
            KCatCode::OtherKChar
        };
        let mut lexer = LineLexer::new(b"A^^42".to_vec());

        assert!(matches!(
            lexer.scan_next_token(&ordinary_cat_code, &kcat_code),
            Ok(Some(LexerToken::Letter(b'A')))
        ));
        assert!(matches!(
            lexer.scan_next_token(&ordinary_cat_code, &kcat_code),
            Ok(Some(LexerToken::Letter(b'B')))
        ));
        assert!(
            lexer
                .scan_next_token(&ordinary_cat_code, &kcat_code)
                .unwrap()
                .is_none()
        );
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn kcat十五と未実装の十四は元のbyte列を通す() {
        for kcat in [KCatCode::NotCjk, KCatCode::LatinUcs] {
            let mut lexer = LineLexer::new(HIRAGANA_A.to_vec());
            for expected in HIRAGANA_A {
                assert!(matches!(
                    lexer.scan_next_token(&|_| CatCode::OtherChar, &|_| kcat),
                    Ok(Some(LexerToken::OtherChar(actual))) if actual == *expected
                ));
            }
        }
    }

    #[test]
    fn kcat十六から二十は一個のcjk_tokenになる() {
        for (kcat, category) in [
            (KCatCode::Kanji, CjkCategory::Kanji),
            (KCatCode::Kana, CjkCategory::Kana),
            (KCatCode::OtherKChar, CjkCategory::OtherKChar),
            (KCatCode::Hangul, CjkCategory::Hangul),
            (KCatCode::Modifier, CjkCategory::Modifier),
        ] {
            let mut lexer = LineLexer::new(HIRAGANA_A.to_vec());
            let token = lexer
                .scan_next_token(&ordinary_cat_code, &|_| kcat)
                .unwrap()
                .unwrap();
            assert_cjk_token(token, 0x3042, category);
            assert!(
                lexer
                    .scan_next_token(&ordinary_cat_code, &|_| kcat)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn cjk制御綴の分類ごとに直後空白を処理する() {
        for kcat in [KCatCode::Kanji, KCatCode::Kana, KCatCode::Hangul] {
            let mut lexer = LineLexer::new(cjk_line(b"\\", b" A"));
            let first = lexer
                .scan_next_token(&ordinary_cat_code, &|_| kcat)
                .unwrap()
                .unwrap();
            assert!(matches!(first, LexerToken::WideCommand(_)));
            assert!(matches!(
                lexer.scan_next_token(&ordinary_cat_code, &|_| kcat),
                Ok(Some(LexerToken::Letter(b'A')))
            ));
        }

        for kcat in [KCatCode::OtherKChar, KCatCode::Modifier] {
            let mut lexer = LineLexer::new(cjk_line(b"\\", b" A"));
            let first = lexer
                .scan_next_token(&ordinary_cat_code, &|_| kcat)
                .unwrap()
                .unwrap();
            assert!(matches!(first, LexerToken::WideCommand(_)));
            assert!(matches!(
                lexer.scan_next_token(&ordinary_cat_code, &|_| kcat),
                Ok(Some(LexerToken::Spacer))
            ));
        }
    }

    #[test]
    fn modifierは制御綴が二単位になれば空白を飛ばす() {
        let mut lexer = LineLexer::new(cjk_line(b"\\", b"x A"));
        let first = lexer
            .scan_next_token(&ordinary_cat_code, &|_| KCatCode::Modifier)
            .unwrap()
            .unwrap();
        let LexerToken::WideCommand(name) = first else {
            panic!("wide command expected");
        };
        assert_eq!(
            name,
            vec![
                ControlSequenceNameUnit::Unicode(0x3042),
                ControlSequenceNameUnit::Byte(b'x')
            ]
        );
        assert!(matches!(
            lexer.scan_next_token(&ordinary_cat_code, &|_| KCatCode::Modifier),
            Ok(Some(LexerToken::Letter(b'A')))
        ));
    }

    #[test]
    fn asciiとcjkのword単位を双方向に混在できる() {
        for kcat in [
            KCatCode::Kanji,
            KCatCode::Kana,
            KCatCode::Hangul,
            KCatCode::Modifier,
        ] {
            let mut ascii_first = LineLexer::new(cjk_line(b"\\x", b" A"));
            let first = ascii_first
                .scan_next_token(&ordinary_cat_code, &|_| kcat)
                .unwrap()
                .unwrap();
            let LexerToken::WideCommand(name) = first else {
                panic!("wide command expected");
            };
            assert_eq!(
                name,
                vec![
                    ControlSequenceNameUnit::Byte(b'x'),
                    ControlSequenceNameUnit::Unicode(0x3042)
                ]
            );
            assert!(matches!(
                ascii_first.scan_next_token(&ordinary_cat_code, &|_| kcat),
                Ok(Some(LexerToken::Letter(b'A')))
            ));
        }

        let mut cjk_first = LineLexer::new(cjk_line(b"\\", b"x"));
        cjk_first.line.extend_from_slice(HIRAGANA_A);
        cjk_first.line.extend_from_slice(b" A");
        let first = cjk_first
            .scan_next_token(&ordinary_cat_code, &|_| KCatCode::Kanji)
            .unwrap()
            .unwrap();
        let LexerToken::WideCommand(name) = first else {
            panic!("wide command expected");
        };
        assert_eq!(
            name,
            vec![
                ControlSequenceNameUnit::Unicode(0x3042),
                ControlSequenceNameUnit::Byte(b'x'),
                ControlSequenceNameUnit::Unicode(0x3042)
            ]
        );
        assert!(matches!(
            cjk_first.scan_next_token(&ordinary_cat_code, &|_| KCatCode::Kanji),
            Ok(Some(LexerToken::Letter(b'A')))
        ));
    }

    #[test]
    fn kcat十六と二十はどちら向きでも一つの制御語になる() {
        const COMBINING_DAKUTEN: &[u8] = &[0xE3, 0x82, 0x99];
        for reverse in [false, true] {
            let mut line = vec![b'\\'];
            if reverse {
                line.extend_from_slice(COMBINING_DAKUTEN);
                line.extend_from_slice(HIRAGANA_A);
            } else {
                line.extend_from_slice(HIRAGANA_A);
                line.extend_from_slice(COMBINING_DAKUTEN);
            }
            line.extend_from_slice(b" A");
            let mut lexer = LineLexer::new(line);
            let kcat_code = |code_point| {
                if code_point == 0x3099 {
                    KCatCode::Modifier
                } else {
                    KCatCode::Kanji
                }
            };
            let first = lexer
                .scan_next_token(&ordinary_cat_code, &kcat_code)
                .unwrap()
                .unwrap();
            let LexerToken::WideCommand(name) = first else {
                panic!("wide command expected");
            };
            let expected = if reverse {
                vec![
                    ControlSequenceNameUnit::Unicode(0x3099),
                    ControlSequenceNameUnit::Unicode(0x3042),
                ]
            } else {
                vec![
                    ControlSequenceNameUnit::Unicode(0x3042),
                    ControlSequenceNameUnit::Unicode(0x3099),
                ]
            };
            assert_eq!(name, expected);
            assert!(matches!(
                lexer.scan_next_token(&ordinary_cat_code, &kcat_code),
                Ok(Some(LexerToken::Letter(b'A')))
            ));
        }
    }

    #[test]
    fn kcat十八はascii制御語を終わらせ通常文字として残る() {
        let mut lexer = LineLexer::new(cjk_line(b"\\x", b" A"));
        assert!(matches!(
            lexer.scan_next_token(&ordinary_cat_code, &|_| KCatCode::OtherKChar),
            Ok(Some(LexerToken::CommandSymbol(b'x')))
        ));
        let second = lexer
            .scan_next_token(&ordinary_cat_code, &|_| KCatCode::OtherKChar)
            .unwrap()
            .unwrap();
        assert_cjk_token(second, 0x3042, CjkCategory::OtherKChar);
        assert!(matches!(
            lexer.scan_next_token(&ordinary_cat_code, &|_| KCatCode::OtherKChar),
            Ok(Some(LexerToken::Spacer))
        ));
    }

    #[test]
    fn wide名はbyte名と分離しkcat変更後も同じidentityを使う() {
        let mut eqtb = Eqtb::new();
        let mut wide16 = LineLexer::new(cjk_line(b"\\", b""));
        let wide16 = wide16
            .scan_next_token(&ordinary_cat_code, &|_| KCatCode::Kanji)
            .unwrap()
            .unwrap()
            .to_token(true, &mut eqtb)
            .unwrap();
        let mut wide17 = LineLexer::new(cjk_line(b"\\", b""));
        let wide17 = wide17
            .scan_next_token(&ordinary_cat_code, &|_| KCatCode::Kana)
            .unwrap()
            .unwrap()
            .to_token(true, &mut eqtb)
            .unwrap();
        assert_eq!(wide16, wide17);

        let byte_cat = |byte| {
            if byte == b'\\' {
                CatCode::Escape
            } else {
                CatCode::Letter
            }
        };
        let mut byte = LineLexer::new(cjk_line(b"\\", b""));
        let byte = byte
            .scan_next_token(&byte_cat, &|_| KCatCode::NotCjk)
            .unwrap()
            .unwrap()
            .to_token(true, &mut eqtb)
            .unwrap();
        assert_ne!(wide16, byte);
    }

    #[test]
    fn 名前空間つきwide制御綴をtyped表へ接続する() {
        let mut eqtb = Eqtb::new();
        let line = cjk_line(b"*ns\\", b"");
        let mut first = LineLexer::new(line.clone());
        let first = first
            .scan_next_token(&ordinary_cat_code, &|_| KCatCode::Kanji)
            .unwrap()
            .unwrap()
            .to_token(true, &mut eqtb)
            .unwrap();
        let mut second = LineLexer::new(line);
        let second = second
            .scan_next_token(&ordinary_cat_code, &|_| KCatCode::Kana)
            .unwrap()
            .unwrap()
            .to_token(false, &mut eqtb)
            .unwrap();
        assert_eq!(first, second);
    }
}

use crate::eqtb::ControlSequence;
use crate::macros::{Macro, MacroToken};
use crate::token::Token;

use std::ops::Range;
use std::rc::Rc;

const MAX_MACRO_PARAMETERS: usize = 9;

/// One macro invocation's parameters in one shared token buffer.
///
/// TeX permits at most nine parameters. Storing ranges into a single buffer
/// avoids an independent `Vec` and `Rc` allocation for every argument while
/// still letting an expanded parameter outlive the reader that introduced it.
#[derive(Debug)]
pub struct MacroArguments {
    tokens: Vec<Token>,
    ends: [usize; MAX_MACRO_PARAMETERS],
    len: usize,
    has_references: bool,
}

impl MacroArguments {
    pub fn new(has_references: bool) -> Self {
        Self {
            tokens: Vec::new(),
            ends: [0; MAX_MACRO_PARAMETERS],
            len: 0,
            has_references,
        }
    }

    /// Remember one parameter's normalized range in the scanner-owned buffer.
    pub fn record_scanned(&mut self, start: usize, end: usize) -> Range<usize> {
        assert!(self.len < MAX_MACRO_PARAMETERS);
        let expected_start = if self.len == 0 {
            0
        } else {
            self.ends[self.len - 1]
        };
        assert_eq!(start, expected_start);
        assert!(end >= start);
        self.ends[self.len] = end;
        self.len += 1;
        start..end
    }

    /// Transfer the successfully scanned invocation buffer without copying.
    pub fn finish_scanning(&mut self, scanner_argument: &mut Vec<Token>) {
        assert!(self.tokens.is_empty());
        self.tokens = std::mem::take(scanner_argument);
    }

    pub fn has_references(&self) -> bool {
        self.has_references
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn parameter_bounds(&self, number: usize) -> Range<usize> {
        assert!(number < self.len);
        let start = if number == 0 {
            0
        } else {
            self.ends[number - 1]
        };
        start..self.ends[number]
    }

    pub fn parameter(&self, number: usize) -> &[Token] {
        &self.tokens[self.parameter_bounds(number)]
    }
}

impl Default for MacroArguments {
    fn default() -> Self {
        Self::new(false)
    }
}

/// See 307.
#[derive(Debug)]
/// Reads from a macro call.
pub struct MacroReader {
    pub cs: ControlSequence,
    pub macro_def: Rc<Macro>,
    pub parameters: Option<Rc<MacroArguments>>,
    pub pos: usize,
}

impl MacroReader {
    /// Get the next token from the macro.
    /// See 357.
    pub fn get_next_token(&mut self) -> Option<MacroToken> {
        match self.macro_def.replacement_text.get(self.pos) {
            Some(macro_token) => {
                self.pos += 1;
                Some(*macro_token)
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MacroArguments;
    use crate::token::Token;

    #[test]
    fn 複数のmacro引数を一つのbufferから範囲で読む() {
        let mut arguments = MacroArguments::new(true);
        assert_eq!(arguments.record_scanned(0, 2), 0..2);
        assert_eq!(arguments.record_scanned(2, 3), 2..3);

        let mut tokens = vec![
            Token::Letter(b'a'),
            Token::Letter(b'b'),
            Token::Letter(b'c'),
        ];
        arguments.finish_scanning(&mut tokens);

        assert!(tokens.is_empty());
        assert_eq!(
            arguments.parameter(0),
            &[Token::Letter(b'a'), Token::Letter(b'b')]
        );
        assert_eq!(arguments.parameter(1), &[Token::Letter(b'c')]);
    }

    #[test]
    fn 空のmacro引数も独立した範囲として保持する() {
        let mut arguments = MacroArguments::new(true);
        assert_eq!(arguments.record_scanned(0, 0), 0..0);
        assert_eq!(arguments.record_scanned(0, 1), 0..1);
        let mut tokens = vec![Token::Letter(b'x')];
        arguments.finish_scanning(&mut tokens);

        assert!(arguments.parameter(0).is_empty());
        assert_eq!(arguments.parameter(1), &[Token::Letter(b'x')]);
    }
}

use super::macro_reader::MacroArguments;
use crate::eqtb::{Eqtb, TokenListVariable};
use crate::logger::Logger;
use crate::print::Printer;
use crate::token::Token;
use crate::token_lists::{token_show, RcTokenList};

use std::rc::Rc;

/// A token list as input source.
/// See 307.
#[derive(Debug)]
pub struct TokenListReader {
    storage: TokenListStorage,
    pos: usize,
}

#[derive(Debug)]
enum TokenListStorage {
    Shared(RcTokenList),
    Inline(Token),
    MacroParameter {
        arguments: Rc<MacroArguments>,
        number: u8,
    },
}

impl TokenListReader {
    pub fn from_shared(token_list: RcTokenList) -> Self {
        Self {
            storage: TokenListStorage::Shared(token_list),
            pos: 0,
        }
    }

    pub fn from_token(token: Token) -> Self {
        Self {
            storage: TokenListStorage::Inline(token),
            pos: 0,
        }
    }

    pub fn from_macro_parameter(arguments: Rc<MacroArguments>, number: usize) -> Self {
        arguments.parameter_bounds(number);
        Self {
            storage: TokenListStorage::MacroParameter {
                arguments,
                number: number
                    .try_into()
                    .expect("TeX macros have at most nine parameters"),
            },
            pos: 0,
        }
    }

    /// Get the next token from the token list.
    /// See 357.
    pub fn get_next_token(&mut self) -> Option<Token> {
        match self.as_slice().get(self.pos).copied() {
            Some(token) => {
                self.pos += 1;
                Some(token)
            }
            None => None,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.pos >= self.as_slice().len()
    }

    pub fn deplete(&mut self) {
        self.pos = self.as_slice().len();
    }

    pub fn as_slice(&self) -> &[Token] {
        match &self.storage {
            TokenListStorage::Shared(token_list) => token_list.as_slice(),
            TokenListStorage::Inline(token) => std::slice::from_ref(token),
            TokenListStorage::MacroParameter { arguments, number } => {
                arguments.parameter(usize::from(*number))
            }
        }
    }

    pub fn position(&self) -> usize {
        self.pos
    }
}

#[cfg(test)]
mod tests {
    use super::{MacroArguments, TokenListReader, TokenListStorage};
    use crate::token::Token;

    use std::rc::Rc;

    #[test]
    fn 一字だけはインラインで通常列はrcを共有する() {
        let token = Token::OtherChar(b'x');
        let mut inline = TokenListReader::from_token(token);
        assert!(matches!(
            &inline.storage,
            TokenListStorage::Inline(stored) if *stored == token
        ));
        assert_eq!(inline.get_next_token(), Some(token));
        assert!(inline.is_finished());
        assert_eq!(inline.get_next_token(), None);

        let shared = Rc::new(vec![Token::Letter(b'a'), Token::Letter(b'b')]);
        let reader = TokenListReader::from_shared(shared.clone());
        let TokenListStorage::Shared(stored) = &reader.storage else {
            panic!("通常のトークン列が共有されていない");
        };
        assert!(Rc::ptr_eq(stored, &shared));
        assert_eq!(reader.as_slice(), shared.as_slice());
    }

    #[test]
    fn 共有macro引数の空範囲と読取位置はsourceごとに独立する() {
        let mut arguments = MacroArguments::new(true);
        arguments.record_scanned(0, 0);
        arguments.record_scanned(0, 2);
        let mut tokens = vec![Token::Letter(b'a'), Token::Letter(b'b')];
        arguments.finish_scanning(&mut tokens);
        let arguments = Rc::new(arguments);

        let mut empty = TokenListReader::from_macro_parameter(arguments.clone(), 0);
        assert!(empty.is_finished());
        assert_eq!(empty.get_next_token(), None);

        let mut first = TokenListReader::from_macro_parameter(arguments.clone(), 1);
        let mut second = TokenListReader::from_macro_parameter(arguments, 1);
        assert_eq!(first.get_next_token(), Some(Token::Letter(b'a')));
        assert_eq!(second.get_next_token(), Some(Token::Letter(b'a')));
        first.deplete();
        assert!(first.is_finished());
        assert_eq!(second.get_next_token(), Some(Token::Letter(b'b')));
        assert!(second.is_finished());
    }
}

/// Indicates where a [`TokenSource`] came from.
#[derive(Debug, PartialEq)]
pub enum TokenSourceType {
    /// A parameter.
    Parameter,
    /// The <uj> part of an alignment.
    UTemplate,
    /// The <vj> part of an alignment. Store the command that ended the corresponding cell.
    VTemplate {
        ending_command: AlignCommand,
    },
    /// Something that we want to read again.
    BackedUp,
    /// Something that was automatically inserted, likely to fix an error.
    Inserted,

    OutputText,
    EveryParText,
    EveryMathText,
    EveryDisplayText,
    EveryHboxText,
    EveryVboxText,
    EveryJobText,
    EveryCrText,
    EveryEofText,
    MarkText,
    WriteText,
}

impl TokenSourceType {
    /// Prints a representation of named token lists such as \output, \mark, \everycr.
    /// See 323.
    pub fn print_named_token_list_trace(
        &self,
        token_list: &[Token],
        eqtb: &Eqtb,
        logger: &mut Logger,
    ) {
        use TokenSourceType::*;
        let s = match self {
            // These do not print a trace.
            Parameter | BackedUp | Inserted | UTemplate | VTemplate { .. } => return,
            // All following token lists print a trace here.
            MarkText => b"mark".to_vec(),
            WriteText => b"write".to_vec(),
            OutputText => TokenListVariable::OutputRoutine.to_string(),
            EveryParText => TokenListVariable::EveryPar.to_string(),
            EveryMathText => TokenListVariable::EveryMath.to_string(),
            EveryDisplayText => TokenListVariable::EveryDisplay.to_string(),
            EveryHboxText => TokenListVariable::EveryHbox.to_string(),
            EveryVboxText => TokenListVariable::EveryVbox.to_string(),
            EveryJobText => TokenListVariable::EveryJob.to_string(),
            EveryCrText => TokenListVariable::EveryCr.to_string(),
            EveryEofText => TokenListVariable::EveryEof.to_string(),
        };
        logger.begin_diagnostic(eqtb.tracing_online());
        logger.print_nl_str("");
        logger.print_esc_str(&s);
        logger.print_str("->");
        token_show(token_list, logger, eqtb);
        logger.end_diagnostic(false);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignCommand {
    Tab,
    Span,
    CarRet,
}

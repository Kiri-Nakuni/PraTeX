use super::Scanner;
use crate::command::ConvertCommand;
use crate::eqtb::{CatCode, Eqtb};
use crate::file_search::{CommandExecutor, CommandOutput, KpsewhichResolver, ResolverOptions};
use crate::logger::{InteractionMode, Logger};
use crate::token::Token;
use crate::token_lists::conv_toks;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FakeExecutor {
    arguments: Rc<RefCell<Vec<Vec<OsString>>>>,
    responses: VecDeque<io::Result<CommandOutput>>,
}

impl CommandExecutor for FakeExecutor {
    fn execute(&mut self, _program: &OsStr, arguments: &[OsString]) -> io::Result<CommandOutput> {
        self.arguments.borrow_mut().push(arguments.to_vec());
        self.responses
            .pop_front()
            .expect("合成したkpsewhich応答が足りない")
    }
}

fn unique_name(label: &str) -> String {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    format!(
        "rtex-pdffilesize-{label}-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn temporary_directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(unique_name(label));
    fs::create_dir(&path).unwrap();
    path
}

fn success(path: &Path) -> io::Result<CommandOutput> {
    let mut stdout = path.as_os_str().as_encoded_bytes().to_vec();
    stdout.extend_from_slice(b"\r\n");
    Ok(CommandOutput {
        code: Some(0),
        stdout,
        stderr: Vec::new(),
    })
}

fn scanner_with_response(
    argument: &[u8],
    response: io::Result<CommandOutput>,
) -> (Scanner, Rc<RefCell<Vec<Vec<OsString>>>>) {
    let mut first_line = Vec::with_capacity(argument.len() + 3);
    first_line.push(b'{');
    first_line.extend_from_slice(argument);
    first_line.extend_from_slice(b"}X");

    let arguments = Rc::new(RefCell::new(Vec::new()));
    let executor = FakeExecutor {
        arguments: Rc::clone(&arguments),
        responses: [response].into_iter().collect(),
    };
    let resolver = KpsewhichResolver::new(ResolverOptions::default(), executor);
    (
        Scanner::new_with_file_resolver(first_line, 0, Box::new(resolver)),
        arguments,
    )
}

fn expand_file_size(scanner: &mut Scanner) -> Vec<Token> {
    let mut eqtb = Eqtb::new();
    eqtb.cat_code_define(b'{', CatCode::LeftBrace, true);
    eqtb.cat_code_define(b'}', CatCode::RightBrace, true);
    let mut logger = Logger::new(String::new(), InteractionMode::Batch);
    logger.terminal_logging = false;

    conv_toks(ConvertCommand::PdfFileSize, scanner, &mut eqtb, &mut logger);

    let mut expansion = Vec::new();
    loop {
        let token = scanner.get_token(&mut eqtb, &mut logger);
        if token == Token::Letter(b'X') {
            return expansion;
        }
        expansion.push(token);
    }
}

#[test]
fn pdffilesizeはcwdにない論理名をresolverで解いて大きさを返す() {
    let directory = temporary_directory("resolved");
    let physical_path = directory.join("物理データ.bin");
    fs::write(&physical_path, b"1234567").unwrap();
    let logical_name = unique_name("logical");
    let (mut scanner, arguments) =
        scanner_with_response(logical_name.as_bytes(), success(&physical_path));

    assert_eq!(expand_file_size(&mut scanner), vec![Token::OtherChar(b'7')]);
    let arguments = arguments.borrow();
    assert_eq!(arguments.len(), 1);
    assert!(arguments[0]
        .iter()
        .any(|argument| argument.as_os_str() == OsStr::new("--format=tex")));
    assert_eq!(arguments[0].last(), Some(&OsString::from(&logical_name)));

    fs::remove_file(physical_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn pdffilesizeはbrace内の空白を論理名の一部としてresolverへ渡す() {
    let directory = temporary_directory("space");
    let physical_path = directory.join("空白を含む物理データ.bin");
    fs::write(&physical_path, b"12345678901").unwrap();
    let logical_name = format!("{} package.ltx", unique_name("logical-space"));
    let (mut scanner, arguments) =
        scanner_with_response(logical_name.as_bytes(), success(&physical_path));

    assert_eq!(
        expand_file_size(&mut scanner),
        vec![Token::OtherChar(b'1'), Token::OtherChar(b'1')]
    );
    assert_eq!(
        arguments.borrow()[0].last(),
        Some(&OsString::from(&logical_name))
    );

    fs::remove_file(physical_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn pdffilesizeはresolverが不在を答えたとき空に展開する() {
    let missing = Ok(CommandOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: Vec::new(),
    });
    let logical_name = unique_name("missing");
    let (mut scanner, arguments) = scanner_with_response(logical_name.as_bytes(), missing);

    assert!(expand_file_size(&mut scanner).is_empty());
    assert_eq!(arguments.borrow().len(), 1);
}

#[test]
fn pdffilesizeはresolverを起動できなくてもpanicせず空に展開する() {
    let failure = Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "合成したresolver起動失敗",
    ));
    let logical_name = unique_name("resolver-error");
    let (mut scanner, arguments) = scanner_with_response(logical_name.as_bytes(), failure);

    assert!(expand_file_size(&mut scanner).is_empty());
    assert_eq!(arguments.borrow().len(), 1);
}

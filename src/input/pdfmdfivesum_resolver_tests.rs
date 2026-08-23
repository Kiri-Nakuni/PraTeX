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
        "rtex-pdfmdfivesum-{label}-{}-{}",
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

fn missing() -> io::Result<CommandOutput> {
    Ok(CommandOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn scanner_with_responses(
    input: &[u8],
    responses: VecDeque<io::Result<CommandOutput>>,
) -> (Scanner, Rc<RefCell<Vec<Vec<OsString>>>>) {
    let mut first_line = input.to_vec();
    first_line.push(b'X');

    let arguments = Rc::new(RefCell::new(Vec::new()));
    let executor = FakeExecutor {
        arguments: Rc::clone(&arguments),
        responses,
    };
    let resolver = KpsewhichResolver::new(ResolverOptions::default(), executor);
    (
        Scanner::new_with_file_resolver(first_line, 0, Box::new(resolver)),
        arguments,
    )
}

fn expand_md_five_sum(scanner: &mut Scanner) -> Vec<Token> {
    let mut eqtb = Eqtb::new();
    eqtb.cat_code_define(b'{', CatCode::LeftBrace, true);
    eqtb.cat_code_define(b'}', CatCode::RightBrace, true);
    let mut logger = Logger::new(String::new(), InteractionMode::Batch);
    logger.terminal_logging = false;

    conv_toks(
        ConvertCommand::PdfMdFiveSum,
        scanner,
        &mut eqtb,
        &mut logger,
    );

    let mut expansion = Vec::new();
    loop {
        let token = scanner.get_token(&mut eqtb, &mut logger);
        if token == Token::Letter(b'X') {
            return expansion;
        }
        expansion.push(token);
    }
}

fn expected_hash(input: &[u8]) -> Vec<Token> {
    crate::md5::md5(input)
        .into_iter()
        .flat_map(|byte| format!("{byte:02X}").into_bytes())
        .map(Token::OtherChar)
        .collect()
}

#[test]
fn pdfmdfivesumの既存文字列形式はresolverを呼ばない() {
    let (mut scanner, arguments) = scanner_with_responses(b"{abc}", VecDeque::new());

    assert_eq!(expand_md_five_sum(&mut scanner), expected_hash(b"abc"));
    assert!(arguments.borrow().is_empty());
}

#[test]
fn pdfmdfivesum_fileは引用符を除いたutf8論理名をresolverで解き全byteを読む() {
    let directory = temporary_directory("quoted-utf8");
    let physical_path = directory.join("物理 byte列.tex");
    let contents = b"\0\xFF\r\nPraTeX\n\0";
    fs::write(&physical_path, contents).unwrap();
    let logical_name = format!("{} 日本語 空白.tex", unique_name("logical"));
    let input = format!("FiLe{{\"{logical_name}\"}}");
    let (mut scanner, arguments) = scanner_with_responses(
        input.as_bytes(),
        [success(&physical_path)].into_iter().collect(),
    );

    assert_eq!(expand_md_five_sum(&mut scanner), expected_hash(contents));
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
fn pdfmdfivesum_fileは直接相対binary_pathなら外部resolverを呼ばない() {
    let logical_name = unique_name("direct-relative") + ".bin";
    let contents = b"\0\xFFdirect binary\r\n\0";
    fs::write(&logical_name, contents).unwrap();
    let input = format!("file{{{logical_name}}}");
    let (mut scanner, arguments) = scanner_with_responses(input.as_bytes(), VecDeque::new());

    assert_eq!(expand_md_five_sum(&mut scanner), expected_hash(contents));
    assert!(arguments.borrow().is_empty());

    fs::remove_file(logical_name).unwrap();
}

#[test]
fn pdfmdfivesum_fileはresolverが不在を答えると空に展開する() {
    let logical_name = unique_name("missing");
    let input = format!("file{{{logical_name}}}");
    let (mut scanner, arguments) =
        scanner_with_responses(input.as_bytes(), [missing()].into_iter().collect());

    assert!(expand_md_five_sum(&mut scanner).is_empty());
    assert_eq!(arguments.borrow().len(), 1);
}

#[test]
fn pdfmdfivesum_fileは解決後に読めない対象でも空に展開する() {
    let directory = temporary_directory("unreadable");
    let logical_name = unique_name("logical-unreadable");
    let input = format!("file{{{logical_name}}}");
    let (mut scanner, arguments) = scanner_with_responses(
        input.as_bytes(),
        [success(&directory)].into_iter().collect(),
    );

    assert!(expand_md_five_sum(&mut scanner).is_empty());
    assert_eq!(arguments.borrow().len(), 1);

    fs::remove_dir(directory).unwrap();
}

#[test]
fn pdfmdfivesum_fileはresolver自体の失敗でも空に展開する() {
    let failure = Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "合成したresolver起動失敗",
    ));
    let logical_name = unique_name("resolver-error");
    let input = format!("file{{{logical_name}}}");
    let (mut scanner, arguments) =
        scanner_with_responses(input.as_bytes(), [failure].into_iter().collect());

    assert!(expand_md_five_sum(&mut scanner).is_empty());
    assert_eq!(arguments.borrow().len(), 1);
}

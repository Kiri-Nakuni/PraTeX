use super::Scanner;
use crate::command::prefixable::load_font_info;
use crate::eqtb::Eqtb;
use crate::file_search::{CommandExecutor, CommandOutput, KpsewhichResolver, ResolverOptions};
use crate::fonts::SizeIndicator;
use crate::logger::{InteractionMode, Logger};
use crate::mode_independent::open_read_file;
use crate::os_str_to_bytes;
use crate::token::Token;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FakeExecutor {
    calls: Rc<Cell<usize>>,
    responses: VecDeque<io::Result<CommandOutput>>,
}

impl CommandExecutor for FakeExecutor {
    fn execute(&mut self, _program: &OsStr, _arguments: &[OsString]) -> io::Result<CommandOutput> {
        self.calls.set(self.calls.get() + 1);
        self.responses
            .pop_front()
            .expect("合成したkpsewhich応答が足りない")
    }
}

struct CapturingExecutor {
    arguments: Rc<RefCell<Vec<Vec<OsString>>>>,
    responses: VecDeque<io::Result<CommandOutput>>,
}

impl CommandExecutor for CapturingExecutor {
    fn execute(&mut self, _program: &OsStr, arguments: &[OsString]) -> io::Result<CommandOutput> {
        self.arguments.borrow_mut().push(arguments.to_vec());
        self.responses
            .pop_front()
            .expect("合成したkpsewhich応答が足りない")
    }
}

fn scanner_with_responses(
    responses: impl IntoIterator<Item = io::Result<CommandOutput>>,
) -> (Scanner, Rc<Cell<usize>>) {
    scanner_with_input_and_responses(Vec::new(), responses)
}

fn scanner_with_input_and_responses(
    first_line: Vec<u8>,
    responses: impl IntoIterator<Item = io::Result<CommandOutput>>,
) -> (Scanner, Rc<Cell<usize>>) {
    let calls = Rc::new(Cell::new(0));
    let executor = FakeExecutor {
        calls: Rc::clone(&calls),
        responses: responses.into_iter().collect(),
    };
    let resolver = KpsewhichResolver::new(ResolverOptions::default(), executor);
    (
        Scanner::new_with_file_resolver(first_line, 0, Box::new(resolver)),
        calls,
    )
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

fn unique_name(label: &str) -> String {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    format!(
        "rtex-{label}-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn temporary_directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(unique_name(label));
    fs::create_dir(&path).unwrap();
    path
}

#[test]
fn resolverのunicode物理pathからinputを開き結果をcacheする() {
    let directory = temporary_directory("input-unicode");
    let physical_path = directory.join("物理入力.tex");
    fs::write(&physical_path, b"resolved input").unwrap();
    let logical_path = PathBuf::from(format!("{}.tex", unique_name("logical-input")));
    let (mut scanner, calls) = scanner_with_responses([success(&physical_path)]);

    for _ in 0..2 {
        let (opened_path, mut file) = scanner.open_input_file(&logical_path).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(opened_path, physical_path);
        assert_eq!(contents, b"resolved input");
    }
    assert_eq!(calls.get(), 1);

    fs::remove_file(physical_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn kpsewhichを起動できなくても従来のtexinputを読む() {
    let area = Path::new(super::TEX_AREA);
    let area_existed = area.exists();
    if !area_existed {
        fs::create_dir(area).unwrap();
    }
    let logical_path = PathBuf::from(format!("{}.tex", unique_name("legacy-input")));
    let fallback_path = area.join(&logical_path);
    fs::write(&fallback_path, b"legacy input").unwrap();
    let mut first_line = b"0=".to_vec();
    first_line.extend_from_slice(logical_path.as_os_str().as_encoded_bytes());
    first_line.push(b' ');
    let (mut scanner, calls) = scanner_with_input_and_responses(
        first_line,
        [Err(io::Error::new(
            io::ErrorKind::NotFound,
            "native kpsewhichなし",
        ))],
    );
    let mut eqtb = Eqtb::new();
    let mut logger = Logger::new(String::new(), InteractionMode::Batch);
    logger.terminal_logging = false;

    open_read_file(&mut scanner, &mut eqtb, &mut logger);

    let mut contents = Vec::new();
    scanner.read_file[0]
        .as_mut()
        .expect("kpsewhichがなくてもTeXinputを開く")
        .read_to_end(&mut contents)
        .unwrap();
    assert_eq!(contents, b"legacy input");
    assert_eq!(calls.get(), 1);

    fs::remove_file(fallback_path).unwrap();
    if !area_existed {
        fs::remove_dir(area).unwrap();
    }
}

#[test]
fn openinはresolver上の引用符つきfileをiffileexistsへ見せる() {
    let directory = temporary_directory("openin-unicode");
    let physical_path = directory.join("存在確認の物理入力.ltx");
    fs::write(&physical_path, b"openin data").unwrap();
    let first_line = b"0=\"logical package.ltx\" ".to_vec();
    let (mut scanner, calls) =
        scanner_with_input_and_responses(first_line, [success(&physical_path)]);
    let mut eqtb = Eqtb::new();
    let mut logger = Logger::new(String::new(), InteractionMode::Batch);
    logger.terminal_logging = false;

    open_read_file(&mut scanner, &mut eqtb, &mut logger);

    let mut contents = Vec::new();
    scanner.read_file[0]
        .as_mut()
        .expect("resolver上のfileなら\\ifeofを偽にする")
        .read_to_end(&mut contents)
        .unwrap();
    assert_eq!(contents, b"openin data");
    assert_eq!(calls.get(), 1);

    fs::remove_file(physical_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn openinは非utf8の論理名と物理名をbyteのまま渡す() {
    use std::os::unix::ffi::OsStringExt;

    let directory = temporary_directory("openin-non-utf8");
    let physical_name =
        OsString::from_vec(vec![b'p', b'h', b'y', b's', 0xfe, b'.', b'l', b't', b'x']);
    let physical_path = directory.join(physical_name);
    fs::write(&physical_path, b"raw openin data").unwrap();
    let logical_name = vec![b'l', b'o', b'g', b'i', b'c', 0xff, b'.', b'l', b't', b'x'];
    let mut first_line = b"0=".to_vec();
    first_line.extend_from_slice(&logical_name);
    first_line.push(b' ');
    let (mut scanner, calls) =
        scanner_with_input_and_responses(first_line, [success(&physical_path)]);
    let mut eqtb = Eqtb::new();
    let mut logger = Logger::new(String::new(), InteractionMode::Batch);
    logger.terminal_logging = false;

    open_read_file(&mut scanner, &mut eqtb, &mut logger);

    assert!(scanner.read_file[0].is_some());
    assert_eq!(calls.get(), 1);

    fs::remove_file(physical_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn vaakinputは拡張子補完後の論理名をresolverへ渡し物理pathの生byteを読む() {
    let directory = temporary_directory("vaakinput-unicode");
    let physical_path = directory.join("物理のVaakソース.vaak");
    fs::write(&physical_path, b"% keep this line ending\n40 + 2\n").unwrap();
    let logical_stem = unique_name("logical-vaak-source");
    let mut first_line = logical_stem.as_bytes().to_vec();
    first_line.push(b' ');
    let arguments = Rc::new(RefCell::new(Vec::new()));
    let executor = CapturingExecutor {
        arguments: Rc::clone(&arguments),
        responses: [success(&physical_path)].into_iter().collect(),
    };
    let resolver = KpsewhichResolver::new(ResolverOptions::default(), executor);
    let mut scanner = Scanner::new_with_file_resolver(first_line, 0, Box::new(resolver));
    let mut eqtb = Eqtb::new();
    let mut logger = Logger::new(String::new(), InteractionMode::Batch);
    logger.terminal_logging = false;

    crate::vaak::vaak_input(&mut scanner, &mut eqtb, &mut logger);

    assert_eq!(
        scanner.get_token(&mut eqtb, &mut logger),
        Token::OtherChar(b'4')
    );
    assert_eq!(
        scanner.get_token(&mut eqtb, &mut logger),
        Token::OtherChar(b'2')
    );
    let arguments = arguments.borrow();
    assert_eq!(arguments.len(), 1);
    assert!(arguments[0]
        .iter()
        .any(|argument| argument.as_os_str() == OsStr::new("--format=other text files")));
    let expected_logical_name = OsString::from(format!("{logical_stem}.vaak"));
    assert_eq!(arguments[0].last(), Some(&expected_logical_name));

    fs::remove_file(physical_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn 解決したtfmの物理pathを論理font名とareaへ漏らさない() {
    let directory = temporary_directory("tfm-unicode");
    let physical_path = directory.join("物理書体.tfm");
    fs::write(&physical_path, minimal_tfm()).unwrap();
    let logical_path = Path::new("logical-area").join("cmr10");
    let (mut scanner, calls) = scanner_with_responses([success(&physical_path)]);

    let Ok(font) = load_font_info(
        &logical_path,
        SizeIndicator::Factor(1000),
        b'-' as i32,
        -1,
        &mut scanner,
    ) else {
        panic!("合成TFMを読めなかった");
    };

    assert_eq!(font.name, b"cmr10");
    assert_eq!(
        font.area,
        os_str_to_bytes(Path::new("logical-area").as_os_str())
    );
    assert_ne!(font.area, os_str_to_bytes(directory.as_os_str()));
    assert_eq!(calls.get(), 1);

    fs::remove_file(physical_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

fn minimal_tfm() -> Vec<u8> {
    // Six size words, a two-word header, four zero dimension words, and seven parameters.
    let mut bytes = Vec::new();
    for value in [19_u16, 2, 1, 0, 1, 1, 1, 1, 0, 0, 0, 7] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    // Design size 10pt in TFM's fixed-point representation.
    bytes.extend_from_slice(&[0x00, 0xa0, 0x00, 0x00]);
    bytes.extend_from_slice(&[0; 4 * 4]);
    bytes.extend_from_slice(&[0; 7 * 4]);
    bytes
}

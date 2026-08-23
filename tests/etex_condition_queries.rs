//! e-TeXの条件状態照会と8-bit font文字存在判定。
//!
//! 公式e-TeX manual 3.4の公開契約から独立に固定する。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn 試験directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    module_path!().hash(&mut hash);
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-etex-condition-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ))
}

fn 実行(directory: &Path, source: &str) -> (Output, String) {
    std::fs::create_dir_all(directory).unwrap();
    for file in ["t.tex", "t.log", "t.dvi"] {
        let _ = std::fs::remove_file(directory.join(file));
    }
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(directory)
        .output()
        .unwrap();
    let log = std::fs::read_to_string(directory.join("t.log"))
        .unwrap()
        .replace('\r', "")
        .replace('\n', "");
    (output, log)
}

fn 成功(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn currentiftypeはunless条件だけを負数で返す() {
    let directory = 試験directory("unless符号");
    let (output, log) = 実行(
        &directory,
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\message{[outside=\\the\\currentiftype]}
\\iftrue
  \\message{[outer=\\the\\currentiftype]}
  \\unless\\iffalse
    \\message{[inner=\\the\\currentiftype]}
  \\fi
  \\message{[restored=\\the\\currentiftype]}
\\fi
\\unless\\iftrue
  \\message{[BAD]}
\\else
  \\message{[unless-else=\\the\\currentiftype]}
\\fi
\\message{[after=\\the\\currentiftype]}
\\end
",
    );
    成功(&output, "currentiftype");
    assert!(log.contains("[outside=0]"), "{log}");
    assert!(log.contains("[outer=15]"), "{log}");
    assert!(log.contains("[inner=-16]"), "{log}");
    assert!(log.contains("[restored=15]"), "{log}");
    assert!(log.contains("[unless-else=-15]"), "{log}");
    assert!(log.contains("[after=0]"), "{log}");
    assert!(!log.contains("[BAD]"), "{log}");
}

fn code零だけを持つtfm() -> Vec<u8> {
    let mut bytes = Vec::new();
    // 6 size words、2 header words、code 0のchar-info、width 2個、parameter 7個。
    for value in [21_u16, 2, 0, 0, 2, 1, 1, 1, 0, 0, 0, 7] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    bytes.extend_from_slice(&0_i32.to_be_bytes());
    bytes.extend_from_slice(&0x0008_0000_i32.to_be_bytes());
    bytes.extend_from_slice(&0_i32.to_be_bytes());
    bytes.extend_from_slice(&0_i32.to_be_bytes());
    bytes.extend_from_slice(&0_i32.to_be_bytes());
    bytes.extend_from_slice(&[0; 7 * 4]);
    assert_eq!(bytes.len(), 21 * 4);
    bytes
}

#[test]
fn iffontcharは範囲外文字番号を診断してcode零へ回復する() {
    let directory = 試験directory("iffontchar範囲回復");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("zero.tfm"), code零だけを持つtfm()).unwrap();
    let (output, log) = 実行(
        &directory,
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\font\\f=zero
\\message{[zero=\\iffontchar\\f0 Y\\else N\\fi]}
\\message{[missing=\\iffontchar\\f1 Y\\else N\\fi]}
\\message{[negative=\\iffontchar\\f-1 Y\\else N\\fi/after]}
\\message{[large=\\iffontchar\\f256 Y\\else N\\fi/after]}
\\end
",
    );
    成功(&output, "iffontchar範囲回復");
    assert!(log.contains("[zero=Y]"), "{log}");
    assert!(log.contains("[missing=N]"), "{log}");
    assert!(log.contains("[negative=Y/after]"), "{log}");
    assert!(log.contains("[large=Y/after]"), "{log}");
    assert_eq!(log.matches("! Bad character code").count(), 2, "{log}");
}

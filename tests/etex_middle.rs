//! e-TeXの可変delimiter列 `\middle`。
//!
//! 公式e-TeX manual 3.9/5.4の公開契約から独立に固定する。TFMは公開file formatから
//! この試験内で合成し、上流engineのsourceや試験は利用しない。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn 試験directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    module_path!().hash(&mut hash);
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-etex-middle-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ))
}

fn 可変delimiter用tfm() -> Vec<u8> {
    let mut bytes = Vec::new();
    // 6 size words、2 header words、code 1..=2、width 2、height 3、depth/italic各1、
    // math symbol fontに必要な22 parameters。code 1は高さ5pt、code 2は15pt。
    for value in [39_u16, 2, 1, 2, 2, 3, 1, 1, 0, 0, 0, 22] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());
    bytes.extend_from_slice(&[1, 0x10, 0, 0]);
    bytes.extend_from_slice(&[1, 0x20, 0, 0]);

    for value in [0_i32, 0x0002_0000] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    for value in [0_i32, 0x0008_0000, 0x0018_0000] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_i32.to_be_bytes());
    bytes.extend_from_slice(&0_i32.to_be_bytes());

    for index in 0..22 {
        // fontdimen6 (quad)だけ10pt相当、残りは零でよい。
        let value = if index == 5 { 0x0010_0000_i32 } else { 0 };
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    assert_eq!(bytes.len(), 39 * 4);
    bytes
}

fn 準備(name: &str) -> PathBuf {
    let directory = 試験directory(name);
    std::fs::create_dir_all(&directory).unwrap();
    for file in [
        "middlemetric.tfm",
        "t.tex",
        "t.log",
        "t.dvi",
        "mk.tex",
        "mk.log",
        "mk.fmt",
        "use.tex",
        "use.log",
        "use.dvi",
    ] {
        let _ = std::fs::remove_file(directory.join(file));
    }
    std::fs::write(directory.join("middlemetric.tfm"), 可変delimiter用tfm()).unwrap();
    directory
}

fn 実行(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap()
}

fn 成功(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn 結合log(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap()
        .replace('\r', "")
        .replace('\n', "")
}

const MATH_FONT_SETUP: &str = "\\font\\f=middlemetric
\\textfont0=\\f
\\textfont2=\\f \\scriptfont2=\\f \\scriptscriptfont2=\\f
\\textfont3=\\f \\scriptfont3=\\f \\scriptscriptfont3=\\f
\\delcode40=4098 \\delcode41=4098 \\delcode124=4098
";

#[test]
fn 複数segmentを群で分け全delimiterを最大寸法へ揃える() {
    let directory = 準備("segmentと共通寸法");
    std::fs::write(
        directory.join("t.tex"),
        format!(
            "\\catcode123=1
\\catcode125=2
\\catcode36=3
\\batchmode
\\showboxbreadth=100 \\showboxdepth=100
{MATH_FONT_SETUP}\\count0=7
\\setbox0=\\hbox{{$\\left(
  \\count0=11 \\mathord{{}}
  \\middle|
  \\message{{[second=\\the\\count0/\\the\\currentgrouptype]}}
  \\count0=13 \\vcenter{{\\hrule height20pt width0pt}}
  \\middle\\delimiter4098
  \\message{{[third=\\the\\count0/\\the\\currentgrouptype]}}
  \\mathord{{}}\\showlists
\\right)$}}
\\message{{[outside=\\the\\count0]}}
\\showbox0
\\end
"
        ),
    )
    .unwrap();
    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "middle segment");
    let log = 結合log(&directory.join("t.log"));
    assert!(log.contains("[second=7/16]"), "{log}");
    assert!(log.contains("[third=7/16]"), "{log}");
    assert!(log.contains("[outside=7]"), "{log}");
    assert!(log.contains("\\left\"1002"), "{log}");
    assert_eq!(log.matches("\\middle\"1002").count(), 2, "{log}");
    assert!(log.matches("\\f ^^B").count() >= 3, "{log}");
}

#[test]
fn middleの左右はrightとleftのspacingを使う() {
    let directory = 準備("spacing");
    std::fs::write(
        directory.join("t.tex"),
        format!(
            "\\catcode123=1
\\catcode125=2
\\catcode36=3
\\batchmode
{MATH_FONT_SETUP}\\nulldelimiterspace=0pt
\\setbox0=\\hbox{{$\\left.\\mathrel{{}}\\middle.\\mathrel{{}}\\right.$}}
\\setbox2=\\hbox{{$\\left.\\mathrel{{}}\\mathclose{{}}\\mathopen{{}}\\mathrel{{}}\\right.$}}
\\ifdim\\wd0=\\wd2 \\message{{[spacing=equal]}}\\else
  \\message{{[spacing=BAD/\\the\\wd0/\\the\\wd2]}}\\fi
\\end
"
        ),
    )
    .unwrap();
    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "middle spacing");
    let log = 結合log(&directory.join("t.log"));
    assert!(log.contains("[spacing=equal]"), "{log}");
    assert!(!log.contains("BAD"), "{log}");
}

#[test]
fn 新しいsegmentは元のstyleから始める() {
    let directory = 準備("segment style");
    std::fs::write(
        directory.join("t.tex"),
        format!(
            "\\catcode123=1
\\catcode125=2
\\catcode36=3
\\batchmode
{MATH_FONT_SETUP}\\font\\fs=middlemetric at 5pt
\\scriptfont0=\\fs \\scriptscriptfont0=\\fs
\\showboxbreadth=100 \\showboxdepth=100
\\setbox0=\\hbox{{$\\left.\\scriptstyle\\mathchar1\\middle.\\mathchar1\\right.$}}
\\showbox0
\\end
"
        ),
    )
    .unwrap();
    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "middle style reset");
    let log = 結合log(&directory.join("t.log"));
    assert_eq!(log.matches("\\fs ^^A").count(), 1, "{log}");
    assert_eq!(log.matches("\\f ^^A").count(), 1, "{log}");
}

#[test]
fn delimiter欠落と対応しないmiddleから入力を失わず回復する() {
    let directory = 準備("error回復");
    std::fs::write(
        directory.join("t.tex"),
        format!(
            "\\catcode123=1
\\catcode125=2
\\catcode36=3
\\batchmode
{MATH_FONT_SETUP}\\def\\kept{{\\message{{[kept]}}}}
\\setbox0=\\hbox{{$\\left.\\middle\\hbox{{}}\\kept\\right.$}}
\\setbox2=\\hbox{{$\\middle.\\message{{[after-extra]}}\\mathord{{}}$}}
\\message{{[done]}}
\\end
"
        ),
    )
    .unwrap();
    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "middle error recovery");
    let log = 結合log(&directory.join("t.log"));
    assert!(log.contains("! Missing delimiter (. inserted)"), "{log}");
    assert!(log.contains("! Extra \\middle"), "{log}");
    assert!(log.contains("[kept]"), "{log}");
    assert!(log.contains("[after-extra]"), "{log}");
    assert!(log.contains("[done]"), "{log}");
}

#[test]
fn middle命令はformatと表示を往復する() {
    let directory = 準備("format");
    std::fs::write(
        directory.join("mk.tex"),
        format!(
            "\\catcode123=1
\\catcode125=2
\\catcode36=3
\\batchmode
{MATH_FONT_SETUP}\\let\\savedmiddle=\\middle
\\dump
"
        ),
    )
    .unwrap();
    let output = 実行(&directory, &["mk.tex"]);
    成功(&output, "middle fmt生成");
    assert!(directory.join("mk.fmt").is_file());

    std::fs::write(
        directory.join("use.tex"),
        "\\message{[primitive=\\meaning\\middle/alias=\\meaning\\savedmiddle]}
\\setbox0=\\hbox{$\\left.\\mathord{}\\savedmiddle\\delimiter4098\\showlists\\mathord{}\\right.$}
\\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["&mk", "use.tex"]);
    成功(&output, "middle fmt読戻し");
    let log = 結合log(&directory.join("use.log"));
    assert!(log.contains("[primitive=\\middle/alias=\\middle]"), "{log}");
    assert!(log.contains("\\middle\"1002"), "{log}");
}

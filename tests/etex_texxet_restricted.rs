//! e-TeX TeX--XeTの最初のrestricted hbox slice。
//!
//! 公開e-TeX manual 4.1の、既定off・engine内明示反転・通常DVIへ出す契約から
//! 自作rule fixtureを作る。上流engineのsourceや試験は利用しない。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn 試験directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    module_path!().hash(&mut hash);
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-etex-texxet-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ))
}

fn inline_math用tfm() -> Vec<u8> {
    let mut bytes = Vec::new();
    // 公開TFM file formatから作るcode 1..=2の10pt metric。math symbolに必要な
    // 22 parametersを持ち、fontdimen6 (quad)以外はこのfixtureでは零でよい。
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
        bytes.extend_from_slice(&(if index == 5 { 0x0010_0000_i32 } else { 0 }).to_be_bytes());
    }
    assert_eq!(bytes.len(), 39 * 4);
    bytes
}

fn 準備(name: &str) -> PathBuf {
    let directory = 試験directory(name);
    std::fs::create_dir_all(&directory).unwrap();
    for file in [
        "t.tex",
        "t.log",
        "t.dvi",
        "t.pdf",
        "mathmetric.tfm",
        "mk.tex",
        "mk.log",
        "mk.fmt",
        "use.tex",
        "use.log",
        "use.dvi",
    ] {
        let _ = std::fs::remove_file(directory.join(file));
    }
    std::fs::write(directory.join("mathmetric.tfm"), inline_math用tfm()).unwrap();
    directory
}

fn 実行(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pratex"))
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

fn 連結log(directory: &Path, name: &str) -> String {
    std::fs::read_to_string(directory.join(name))
        .unwrap()
        .replace('\r', "")
        .replace('\n', "")
}

fn 四byte符号付き(bytes: &[u8], position: &mut usize) -> i32 {
    let value = i32::from_be_bytes(bytes[*position..*position + 4].try_into().unwrap());
    *position += 4;
    value
}

fn 可変長符号付き(bytes: &[u8], position: &mut usize, length: usize) -> i32 {
    let mut value = if bytes[*position] & 0x80 == 0 {
        0_i32
    } else {
        -1_i32
    };
    for _ in 0..length {
        value = (value << 8) | i32::from(bytes[*position]);
        *position += 1;
    }
    value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageOperation {
    Rule { horizontal: i32, width: i32 },
    ByteChar(u8),
}

fn 最初のpageの操作(bytes: &[u8]) -> Vec<PageOperation> {
    let bop = bytes
        .iter()
        .position(|&byte| byte == 139)
        .expect("BOPがない");
    let mut position = bop + 45;
    let mut horizontal = 0_i32;
    let mut w = 0_i32;
    let mut x = 0_i32;
    let mut stack = Vec::new();
    let mut operations = Vec::new();
    loop {
        let opcode = bytes[position];
        position += 1;
        match opcode {
            0..=127 => operations.push(PageOperation::ByteChar(opcode)),
            128..=131 => {
                let length = usize::from(opcode - 127);
                let mut character = 0_u32;
                for _ in 0..length {
                    character = (character << 8) | u32::from(bytes[position]);
                    position += 1;
                }
                if let Ok(character) = u8::try_from(character) {
                    operations.push(PageOperation::ByteChar(character));
                }
            }
            132 => {
                let _height = 四byte符号付き(bytes, &mut position);
                let width = 四byte符号付き(bytes, &mut position);
                operations.push(PageOperation::Rule { horizontal, width });
                horizontal += width;
            }
            137 => {
                position += 8;
            }
            140 => break,
            141 => stack.push((horizontal, w, x)),
            142 => (horizontal, w, x) = stack.pop().expect("DVI pushに対するpopがない"),
            143..=146 => {
                horizontal +=
                    可変長符号付き(bytes, &mut position, usize::from(opcode - 142));
            }
            147 => horizontal += w,
            148..=151 => {
                w = 可変長符号付き(bytes, &mut position, usize::from(opcode - 147));
                horizontal += w;
            }
            152 => horizontal += x,
            153..=156 => {
                x = 可変長符号付き(bytes, &mut position, usize::from(opcode - 152));
                horizontal += x;
            }
            157..=160 => position += usize::from(opcode - 156),
            162..=165 => position += usize::from(opcode - 161),
            161 | 166 => {}
            167..=170 => position += usize::from(opcode - 166),
            171..=234 => {}
            235..=238 => position += usize::from(opcode - 234),
            239..=242 => {
                let length_bytes = usize::from(opcode - 238);
                let mut length = 0_usize;
                for _ in 0..length_bytes {
                    length = (length << 8) | usize::from(bytes[position]);
                    position += 1;
                }
                position += length;
            }
            243..=246 => {
                position += usize::from(opcode - 242) + 12;
                let area_length = usize::from(bytes[position]);
                let name_length = usize::from(bytes[position + 1]);
                position += 2 + area_length + name_length;
            }
            other => panic!("fixture pageの未対応DVI opcode {other}"),
        }
    }
    operations
}

fn 最初のpageのset_rule幅(bytes: &[u8]) -> Vec<i32> {
    最初のpageの操作(bytes)
        .into_iter()
        .filter_map(|operation| match operation {
            PageOperation::Rule { width, .. } => Some(width),
            PageOperation::ByteChar(_) => None,
        })
        .collect()
}

#[test]
fn beginr区間を反転して通常dviへ書く() {
    let directory = 準備("beginR DVI");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\TeXXeTstate=1
\\setbox0=\\hbox{\\beginR
  \\vrule width1pt height1pt
  \\kern2pt
  \\vrule width3pt height1pt
\\endR}
\\shipout\\box0
\\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "restricted beginR");
    let dvi = std::fs::read(directory.join("t.dvi")).unwrap();
    let rules: Vec<(i32, i32)> = 最初のpageの操作(&dvi)
        .into_iter()
        .filter_map(|operation| match operation {
            PageOperation::Rule { horizontal, width } => Some((horizontal, width)),
            PageOperation::ByteChar(_) => None,
        })
        .collect();
    let widths: Vec<i32> = rules.iter().map(|&(_, width)| width).collect();
    assert_eq!(widths, vec![3 * 65_536, 65_536]);
    assert_eq!(rules[1].0 - rules[0].0, 5 * 65_536);
}

#[test]
fn beginr区間をbackend共通で直接pdfへも反転する() {
    let directory = 準備("beginR PDF");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\TeXXeTstate=1
\\setbox0=\\hbox{\\beginR
  \\vrule width1pt height1pt
  \\kern2pt
  \\vrule width3pt height1pt
\\endR}
\\shipout\\box0
\\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["--output-format=pdf", "t.tex"]);
    成功(&output, "restricted beginR PDF");
    let pdf_bytes = std::fs::read(directory.join("t.pdf")).unwrap();
    let pdf = String::from_utf8_lossy(&pdf_bytes);
    let rules: Vec<&str> = pdf.lines().filter(|line| line.ends_with(" re f")).collect();
    assert_eq!(rules.len(), 2, "{rules:?}");
    assert_eq!(rules[0].split_whitespace().next(), Some("72"));
    assert_eq!(rules[1].split_whitespace().next(), Some("76.98132"));
    assert_eq!(rules[0].split_whitespace().nth(2), Some("2.988792"));
    assert_eq!(rules[1].split_whitespace().nth(2), Some("0.996264"));
}

#[test]
fn rtl区間のinline_mathを常に左から右のatomic区間とする() {
    let directory = 準備("RTL inline math");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\catcode36=3
\\batchmode
\\font\\f=mathmetric
\\textfont0=\\f
\\textfont2=\\f \\scriptfont2=\\f \\scriptscriptfont2=\\f
\\textfont3=\\f \\scriptfont3=\\f \\scriptscriptfont3=\\f
\\TeXXeTstate=1
\\setbox0=\\hbox{\\beginR
  \\vrule width1pt height1pt
  $\\mathchar1\\mathchar2$
  \\vrule width3pt height1pt
\\endR}
\\shipout\\box0
\\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "RTL inline math");
    let operations = 最初のpageの操作(&std::fs::read(directory.join("t.dvi")).unwrap());
    let semantic: Vec<(&str, i32)> = operations
        .into_iter()
        .map(|operation| match operation {
            PageOperation::Rule { width, .. } => ("rule", width),
            PageOperation::ByteChar(character) => ("char", i32::from(character)),
        })
        .collect();
    assert_eq!(
        semantic,
        vec![
            ("rule", 3 * 65_536),
            ("char", 1),
            ("char", 2),
            ("rule", 65_536),
        ]
    );
}

#[test]
fn 無効stateの方向primitiveを通常nodeへ読み替えない() {
    let directory = 準備("disabled state");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\setbox0=\\hbox{\\beginR\\endR}
\\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    // TeX型のbatchmodeは回復可能errorを診断して入力を続行する。
    // process statusではなく、両方のprimitiveをnode化せず診断した事実を固定する。
    成功(&output, "無効stateの回復可能診断");
    let log = 連結log(&directory, "t.log");
    assert!(log.contains("Improper \\beginR"), "{log}");
    assert!(log.contains("Improper \\endR"), "{log}");
}

#[test]
fn discとalignmentのrestricted_modeを明示hboxと混同しない() {
    let directory = 準備("restricted provenance");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\TeXXeTstate=1
\\setbox0=\\hbox{\\discretionary{\\beginR\\vrule width1pt\\endR}{}{}}
\\setbox1=\\hbox{\\halign{#\\cr\\beginR\\vrule width2pt\\endR\\cr}}
\\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "disc/alignmentの回復可能診断");
    let log = 連結log(&directory, "t.log");
    assert!(
        log.matches("You can't use `\\beginR' in restricted horizontal mode")
            .count()
            >= 2,
        "{log}"
    );
    assert!(
        log.matches("You can't use `\\endR' in restricted horizontal mode")
            .count()
            >= 2,
        "{log}"
    );
}

#[test]
fn rtl区間のdiscretionaryを部分的に誤反転しない() {
    let directory = 準備("RTL discretionary containment");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\TeXXeTstate=1
\\setbox0=\\hbox{\\beginR
  \\vrule width1pt height1pt
  \\discretionary{}{}{\\vrule width2pt height1pt \\vrule width3pt height1pt}
  \\vrule width4pt height1pt
\\endR}
\\shipout\\box0
\\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "RTL discretionaryの限定回復");
    let log = 連結log(&directory, "t.log");
    assert!(
        log.contains("discretionary inside a right-to-left region; direction ignored"),
        "{log}"
    );
    let widths = 最初のpageのset_rule幅(&std::fs::read(directory.join("t.dvi")).unwrap());
    assert_eq!(widths, vec![65_536, 2 * 65_536, 3 * 65_536, 4 * 65_536]);
}

#[test]
fn 方向node入りhboxをunboxして未対応listへ漏らさない() {
    let directory = 準備("sealed unbox");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\TeXXeTstate=1
\\setbox0=\\hbox{\\beginR
  \\vrule width1pt height1pt
  \\vrule width3pt height1pt
\\endR}
\\setbox1=\\hbox{\\unhbox0}
\\shipout\\box0
\\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "direction hboxのunbox拒否");
    let log = 連結log(&directory, "t.log");
    assert!(log.contains("Text-direction box can't be unboxed"), "{log}");
    let widths = 最初のpageのset_rule幅(&std::fs::read(directory.join("t.dvi")).unwrap());
    assert_eq!(widths, vec![3 * 65_536, 65_536]);
}

#[test]
fn fmt内の方向nodeはstateが零へ戻っても型を保つ() {
    let directory = 準備("fmt direction node");
    std::fs::write(
        directory.join("mk.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\TeXXeTstate=1
\\setbox0=\\hbox{\\beginR
  \\vrule width1pt height1pt
  \\vrule width3pt height1pt
\\endR}
\\dump
",
    )
    .unwrap();
    let output = 実行(&directory, &["mk.tex"]);
    成功(&output, "direction node fmt生成");

    std::fs::write(
        directory.join("use.tex"),
        "\\batchmode
\\message{[state=\\the\\TeXXeTstate]}
\\shipout\\box0
\\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["&mk", "use.tex"]);
    成功(&output, "direction node fmt読込");
    let log = std::fs::read_to_string(directory.join("use.log")).unwrap();
    assert!(log.replace(['\r', '\n'], "").contains("[state=0]"));
    let widths = 最初のpageのset_rule幅(&std::fs::read(directory.join("use.dvi")).unwrap());
    assert_eq!(widths, vec![3 * 65_536, 65_536]);
}

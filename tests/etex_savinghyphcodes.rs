//! e-TeX `\savinghyphcodes` の言語別hyphenation code保存。
//!
//! 仕様は公式 e-TeX manual 3.10 と、公式 e-upTeX 2026に対する自作最小入力の
//! black-box観測だけから固定する。Latin-UCSはPraTeX固有拡張として別gateで確かめる。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn 合成tfm() -> Vec<u8> {
    let mut bytes = Vec::new();
    // 6 size words, 2 header words, code 0..=122, 3 widths, 2 heights,
    // one depth and italic entry, no lig/kern/extensible data, 7 parameters.
    for value in [145_u16, 2, 0, 122, 3, 2, 1, 1, 0, 0, 0, 7] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());

    for character in 0..=122 {
        let info = match character {
            45 => [2, 0x10, 0, 0],
            65..=68 | 97..=100 => [1, 0x10, 0, 0],
            _ => [0, 0, 0, 0],
        };
        bytes.extend_from_slice(&info);
    }
    for value in [0_i32, 0x0008_0000, 0x0001_999A] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    for value in [0_i32, 0x000A_0000] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_i32.to_be_bytes());
    bytes.extend_from_slice(&0_i32.to_be_bytes());
    bytes.extend_from_slice(&[0; 7 * 4]);
    assert_eq!(bytes.len(), 145 * 4);
    bytes
}

fn 試験directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    module_path!().hash(&mut hash);
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-etex-savinghyphcodes-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ))
}

fn 準備(name: &str) -> PathBuf {
    let directory = 試験directory(name);
    std::fs::create_dir_all(&directory).unwrap();
    for file in [
        "metric.tfm",
        "t.tex",
        "t.log",
        "mk.tex",
        "mk.log",
        "mk.fmt",
        "use.tex",
        "use.log",
    ] {
        let _ = std::fs::remove_file(directory.join(file));
    }
    std::fs::write(directory.join("metric.tfm"), 合成tfm()).unwrap();
    directory
}

fn 実行(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(arguments)
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

fn 保存後にlccodeを消す文(saving: i32) -> String {
    format!(
        "\\catcode123=1\n\
         \\catcode125=2\n\
         \\batchmode\n\
         \\lccode65=97 \\lccode66=98 \\lccode97=97 \\lccode98=98\n\
         \\savinghyphcodes={saving}\n\
         \\patterns{{a1b}}\n\
         \\lccode65=0 \\lccode66=0 \\lccode97=0 \\lccode98=0\n\
         \\uchyph=1 \\defaulthyphenchar=45 \\font\\f=metric\n\
         \\showboxbreadth=100 \\showboxdepth=10\n\
         \\setbox0=\\vbox{{\\f \\hsize=6pt \\pretolerance=-1 \\tolerance=10000 \\noindent\\hskip0pt AB\\par}}\n\
         \\showbox0 \\message{{[AFTER]}}\n\
         \\end\n"
    )
}

#[test]
fn 正値はpattern時のlccodeを圧縮後の通常hyphenationへ固定する() {
    let directory = 準備("正値");
    std::fs::write(directory.join("t.tex"), 保存後にlccodeを消す文(1)).unwrap();
    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "正値snapshot");
    let log = 結合log(&directory.join("t.log"));
    assert!(log.contains("\\discretionary"), "{log}");
    assert!(log.contains("[AFTER]"), "{log}");
}

#[test]
fn 零以下はsnapshotを作らず圧縮後も現在のlccodeを使う() {
    for saving in [0, -1] {
        let directory = 準備(&format!("非正値-{saving}"));
        std::fs::write(directory.join("t.tex"), 保存後にlccodeを消す文(saving)).unwrap();
        let output = 実行(&directory, &["t.tex"]);
        成功(&output, "非正値control");
        let log = 結合log(&directory.join("t.log"));
        assert!(!log.contains("\\discretionary"), "saving={saving}: {log}");
        assert!(log.contains("[AFTER]"), "saving={saving}: {log}");
    }
}

#[test]
fn 同じlanguageの正値patternsはsnapshotを更新し非正値patternsは以前の値を残す() {
    for (second, expected_break) in [(1, false), (0, true), (-1, true)] {
        let directory = 準備(&format!("再snapshot-{second}"));
        let source = format!(
            "\\catcode123=1\n\
             \\catcode125=2\n\
             \\batchmode\n\
             \\savinghyphcodes=1\n\
             \\lccode65=97 \\lccode66=98 \\lccode97=97 \\lccode98=98\n\
             \\patterns{{a1b}}\n\
             \\lccode65=120 \\lccode99=99 \\lccode100=100 \\lccode120=120\n\
             \\savinghyphcodes={second} \\patterns{{c1d}}\n\
             \\uchyph=1 \\defaulthyphenchar=45 \\font\\f=metric\n\
             \\showboxbreadth=100 \\showboxdepth=10\n\
             \\setbox0=\\vbox{{\\f \\hsize=6pt \\pretolerance=-1 \\tolerance=10000 \\noindent\\hskip0pt AB\\par}}\n\
             \\showbox0 \\end\n"
        );
        std::fs::write(directory.join("t.tex"), source).unwrap();
        let output = 実行(&directory, &["t.tex"]);
        成功(&output, "同一language再snapshot");
        let log = 結合log(&directory.join("t.log"));
        assert_eq!(
            log.contains("\\discretionary"),
            expected_break,
            "second={second}: {log}"
        );
    }
}

#[test]
fn 複数languageはそれぞれのsnapshotで同じ入力文字を正規化する() {
    let directory = 準備("複数language");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\savinghyphcodes=1
\\language=1
\\lccode65=97 \\lccode66=98 \\lccode97=97 \\lccode98=98
\\patterns{a1b}
\\language=2
\\lccode65=99 \\lccode66=100 \\lccode99=99 \\lccode100=100
\\patterns{c1d}
\\lccode65=0 \\lccode66=0
\\uchyph=1 \\defaulthyphenchar=45 \\font\\f=metric
\\showboxbreadth=100 \\showboxdepth=10
\\language=1 \\setbox1=\\vbox{\\f \\hsize=6pt \\pretolerance=-1 \\tolerance=10000 \\noindent\\hskip0pt AB\\par}
\\showbox1
\\language=2 \\setbox2=\\vbox{\\f \\hsize=6pt \\pretolerance=-1 \\tolerance=10000 \\noindent\\hskip0pt AB\\par}
\\showbox2
\\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "複数language");
    let log = 結合log(&directory.join("t.log"));
    assert_eq!(log.matches("\\discretionary").count(), 2, "{log}");
}

#[test]
fn 圧縮後のhyphenation例外も保存codeで読み通常単語に適用する() {
    let directory = 準備("例外");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\savinghyphcodes=1
\\lccode65=97 \\lccode66=98 \\lccode97=97 \\lccode98=98
\\patterns{c1d}
\\defaulthyphenchar=45 \\font\\f=metric
\\setbox0=\\vbox{\\f \\hsize=6pt \\pretolerance=-1 \\tolerance=10000 \\noindent\\hskip0pt cd\\par}
\\lccode65=0 \\lccode66=0 \\lccode97=0 \\lccode98=0
\\hyphenation{A-B}
\\uchyph=1 \\showboxbreadth=100 \\showboxdepth=10
\\setbox1=\\vbox{\\f \\hsize=6pt \\pretolerance=-1 \\tolerance=10000 \\noindent\\hskip0pt AB\\par}
\\showbox1 \\message{[EXCEPTION-AFTER]}
\\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "保存code例外");
    let log = 結合log(&directory.join("t.log"));
    assert!(!log.contains("Not a letter"), "{log}");
    assert!(log.contains("\\discretionary"), "{log}");
    assert!(log.contains("[EXCEPTION-AFTER]"), "{log}");
}

#[test]
fn 保存codeとlatin_ucs拡張は圧縮済みfmtを往復する() {
    let directory = 準備("fmt");
    std::fs::write(
        directory.join("mk.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\kcatcode256=14 \\catcode256=11
\\savinghyphcodes=1
\\lccode65=97 \\lccode66=98 \\lccode97=97 \\lccode98=98 \\lccode256=256
\\patterns{a1b Ā1a}
\\lccode65=0 \\lccode66=0 \\lccode97=0 \\lccode98=0 \\lccode256=0
\\dump
",
    )
    .unwrap();
    let output = 実行(&directory, &["-ini", "mk.tex"]);
    成功(&output, "snapshot fmt生成");
    assert!(directory.join("mk.fmt").is_file());

    std::fs::write(
        directory.join("use.tex"),
        "\\batchmode
\\hyphenation{Ā-a}
\\uchyph=1 \\defaulthyphenchar=45 \\font\\f=metric
\\showboxbreadth=100 \\showboxdepth=10
\\setbox0=\\vbox{\\f \\hsize=6pt \\pretolerance=-1 \\tolerance=10000 \\noindent\\hskip0pt AB\\par}
\\showbox0 \\message{[FMT-AFTER]}
\\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["&mk", "use.tex"]);
    成功(&output, "snapshot fmt読戻し");
    let log = 結合log(&directory.join("use.log"));
    assert!(!log.contains("Not a letter"), "{log}");
    assert!(log.contains("\\discretionary"), "{log}");
    assert!(log.contains("[FMT-AFTER]"), "{log}");
}

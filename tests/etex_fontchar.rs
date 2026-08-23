//! e-TeX の 8-bit TFM 文字寸法問い合わせ。
//!
//! 仕様は公式 e-TeX manual 3.4 と、公式 pdfTeX のe-TeX extended modeに対する自作入力の
//! black-box 観測だけから固定する。TFM は公開file formatからこの試験内で合成する。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn 合成tfm() -> Vec<u8> {
    let mut bytes = Vec::new();
    // 6 size words, 2 header words, 0..=66 のchar-info、各2個の寸法table、7 parameters。
    for value in [90_u16, 2, 0, 66, 2, 2, 2, 2, 0, 0, 0, 7] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());

    // code 0 と A は四成分ともindex 1。B は他のindexが非零でもwidth index 0なので欠落字。
    for character in 0..=66 {
        let info = match character {
            0 | 65 => [1, 0x11, 0x04, 0],
            66 => [0, 0x11, 0x04, 0],
            _ => [0, 0, 0, 0],
        };
        bytes.extend_from_slice(&info);
    }

    for values in [
        [0_i32, 0x0008_0000], // width: 5pt at 10pt
        [0_i32, 0x0004_0000], // height: 2.5pt
        [0_i32, 0x0002_0000], // depth: 1.25pt
        [0_i32, 0x0001_0000], // italic correction: 0.625pt
    ] {
        for value in values {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
    bytes.extend_from_slice(&[0; 7 * 4]);
    assert_eq!(bytes.len(), 90 * 4);
    bytes
}

fn 試験directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    module_path!().hash(&mut hash);
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-etex-fontchar-{}-{:016x}",
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

#[test]
fn 四成分を内部寸法として走査し欠落字を零にする() {
    let directory = 準備("四成分");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\font\\f=metric
\\textfont0=\\f
\\def\\fontid{\\f}
\\def\\charcode{65}
\\count0=65
\\f
\\dimen0=\\fontcharwd\\f65
\\dimen2=\\dimexpr\\fontcharht\\f65+\\fontchardp\\f65\\relax
\\edef\\raw{\\fontcharwd\\f65}
\\edef\\metrics{\\the\\fontcharwd\\f65/\\the\\fontcharht\\f65/\\the\\fontchardp\\f65/\\the\\fontcharic\\f65}
\\message{[values=\\metrics/d0=\\the\\dimen0/sum=\\the\\dimen2/sp=\\number\\fontcharwd\\f65]}
\\message{[fontspec=\\the\\fontcharwd\\fontid65/\\the\\fontcharwd\\font65/\\the\\fontcharwd\\textfont0 65]}
\\message{[charspec=\\the\\fontcharwd\\f\\charcode/\\the\\fontcharwd\\f`A/\\the\\fontcharwd\\f\\count0]}
\\message{[raw=\\meaning\\raw/expanded=\\meaning\\metrics]}
\\message{[missing=\\the\\fontcharwd\\f66/\\the\\fontcharht\\f66/\\the\\fontchardp\\f66/\\the\\fontcharic\\f66]}
\\message{[outside=\\the\\fontcharwd\\f64/\\the\\fontcharwd\\f255/null=\\the\\fontcharwd\\nullfont65]}
\\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "fontchar実行");
    let log = 結合log(&directory.join("t.log"));
    assert!(
        log.contains("[values=5.0pt/2.5pt/1.25pt/0.625pt/d0=5.0pt/sum=3.75pt/sp=327680]"),
        "{log}"
    );
    assert!(log.contains("[fontspec=5.0pt/5.0pt/5.0pt]"), "{log}");
    assert!(log.contains("[charspec=5.0pt/5.0pt/5.0pt]"), "{log}");
    assert!(log.contains("[raw=macro:->\\fontcharwd "), "{log}");
    assert!(
        log.contains("/expanded=macro:->5.0pt/2.5pt/1.25pt/0.625pt]"),
        "{log}"
    );
    assert!(log.contains("[missing=0.0pt/0.0pt/0.0pt/0.0pt]"), "{log}");
    assert!(log.contains("[outside=0.0pt/0.0pt/null=0.0pt]"), "{log}");
}

#[test]
fn 範囲外文字番号は既存の八bit診断で零へ回復する() {
    let directory = 準備("範囲外");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\font\\f=metric
\\message{[negative=\\the\\fontcharwd\\f-1/after]}
\\message{[large=\\the\\fontcharwd\\f256/after]}
\\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "範囲外回復");
    let log = 結合log(&directory.join("t.log"));
    assert_eq!(log.matches("! Bad character code").count(), 2, "{log}");
    assert!(log.contains("[negative=5.0pt/after]"), "{log}");
    assert!(log.contains("[large=5.0pt/after]"), "{log}");
}

#[test]
fn fontchar命令とfontmetricをformatで往復する() {
    let directory = 準備("format");
    std::fs::write(
        directory.join("mk.tex"),
        "\\catcode123=1
\\catcode125=2
\\batchmode
\\font\\f=metric
\\let\\savedwd=\\fontcharwd
\\let\\savedht=\\fontcharht
\\let\\saveddp=\\fontchardp
\\let\\savedic=\\fontcharic
\\dump
",
    )
    .unwrap();
    let output = 実行(&directory, &["mk.tex"]);
    成功(&output, "fmt生成");
    assert!(directory.join("mk.fmt").is_file());

    std::fs::write(
        directory.join("use.tex"),
        "\\message{[\\the\\savedwd\\f65/\\the\\savedht\\f65/\\the\\saveddp\\f65/\\the\\savedic\\f65]}
\\message{[\\meaning\\savedwd/\\meaning\\savedht/\\meaning\\saveddp/\\meaning\\savedic]}
\\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["&mk", "use.tex"]);
    成功(&output, "fmt読戻し");
    let log = 結合log(&directory.join("use.log"));
    assert!(log.contains("[5.0pt/2.5pt/1.25pt/0.625pt]"), "{log}");
    assert!(
        log.contains("[\\fontcharwd/\\fontcharht/\\fontchardp/\\fontcharic]"),
        "{log}"
    );
}

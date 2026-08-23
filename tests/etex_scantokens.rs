//! e-TeX `\scantokens` のtyped疑似入力回帰。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    let directory = std::env::temp_dir().join(format!(
        "etex-scantokens-{}-{:x}",
        std::process::id(),
        hasher.finish()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("t.tex");
    let mut file = std::fs::File::create(&source).unwrap();
    write!(
        file,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n{body}\n\\end\n"
    )
    .unwrap();
    drop(file);

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "PraTeX失敗:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(directory.join("t.log"))
        .unwrap()
        .replace('\n', "")
}

#[test]
fn scantokensは現在のcatcodeで再字句化する() {
    let log = run_tex(
        "catcode",
        "\\catcode`\\@=12
         \\def\\payload{@}
         \\catcode`\\@=13
         \\def@{\\message{[OK]}}
         \\expandafter\\scantokens\\expandafter{\\payload}
         \\message{[AFTER]}",
    );
    assert!(log.contains("[OK] [AFTER]"), "{log}");
}

#[test]
fn 疑似入力は暗黙groupを作らず逐次字句化する() {
    // KOMA-Scriptはこの形で、先頭のendgroupが戻したcatcode 6を同じ行の#へ適用する。
    let log = run_tex(
        "groupなし",
        "\\catcode`\\#=6
         \\begingroup
         \\catcode`\\#=12
         \\scantokens{\\endgroup\\def\\take#1{[#1]}}
         \\message{\\take{OK}}",
    );
    assert!(log.contains("[OK]"), "{log}");
    assert!(!log.contains("Illegal parameter"), "{log}");
}

#[test]
fn everyeofは疑似入力の自然eofで一度だけ入る() {
    let log = run_tex(
        "everyeof",
        "\\everyeof{\\message{[EOF:\\the\\inputlineno]}}
         \\scantokens{\\message{[BODY]}}
         \\message{[AFTER]}",
    );
    assert!(log.contains("[BODY] [EOF:2] [AFTER]"), "{log}");
    assert_eq!(log.matches("[EOF:").count(), 1, "{log}");
}

#[test]
fn endinputでは疑似入力のeveryeofを入れない() {
    let log = run_tex(
        "endinput",
        "\\catcode`\\|=12
         \\newlinechar=`\\|
         \\everyeof{\\message{[EOF]}}
         \\scantokens{\\message{[BODY]}\\endinput|\\message{[BAD]}}
         \\message{[AFTER]}",
    );
    assert!(log.contains("[BODY] [AFTER]"), "{log}");
    assert!(!log.contains("[EOF]"), "{log}");
    assert!(!log.contains("[BAD]"), "{log}");
}

#[test]
fn edef内のscantokensは外側のdef_refを失わない() {
    let log = run_tex(
        "edef",
        "\\everyeof{\\noexpand}
         \\edef\\result{before\\scantokens{middle}after}
         \\message{[\\meaning\\result]}",
    );
    assert!(log.contains("[macro:->beforemiddle after]"), "{log}");
}

#[test]
fn 範囲外newlinecharでもunicode文字を改行にしない() {
    let log = run_tex(
        "unicode",
        "\\newlinechar=-1
         \\kcatcode\"3042=16
         \\scantokens{\\message{[あ]}}",
    );
    assert!(log.contains("[あ]"), "{log}");
}

#[test]
fn tracingscantokensは開始時の値で閉じ括弧まで出す() {
    let log = run_tex(
        "trace-snapshot",
        "\\tracingscantokens=1
         \\scantokens{\\tracingscantokens=0 \\message{[BODY]}}
         \\message{[AFTER]}",
    );
    assert!(log.contains("(  [BODY]) [AFTER]"), "{log}");
}

#[test]
fn formatを読み直してもscantokensの意味を保つ() {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "scantokens-format".hash(&mut hasher);
    let directory = std::env::temp_dir().join(format!(
        "etex-scantokens-fmt-{}-{:x}",
        std::process::id(),
        hasher.finish()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("mk.tex"),
        "\\catcode`\\{=1\n\\catcode`\\}=2\n\\batchmode\n\\dump\n",
    )
    .unwrap();
    let build = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("mk.tex")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        build.status.success() && directory.join("mk.fmt").is_file(),
        "format生成失敗:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    std::fs::write(
        directory.join("use.tex"),
        "\\batchmode\n\\message{[\\meaning\\scantokens]}\n\\scantokens{\\message{[FMT-SCAN]}}\n\\end\n",
    )
    .unwrap();
    let load = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("&mk")
        .arg("use.tex")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        load.status.success(),
        "format読込み失敗:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&load.stdout),
        String::from_utf8_lossy(&load.stderr)
    );
    let log = std::fs::read_to_string(directory.join("use.log"))
        .unwrap()
        .replace('\n', "");
    assert!(log.contains("[\\scantokens]"), "{log}");
    assert!(log.contains("[FMT-SCAN]"), "{log}");
}

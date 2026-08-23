//! PraTeX の静粛 mode は、TeX 文書が明示した端末出力まで隠さない。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn test_directory(name: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};

    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-quiet-mode-{}-{:x}",
        std::process::id(),
        hash.finish()
    ))
}

fn run_tex(binary: &str, name: &str, options: &[&str], body: &str) -> (PathBuf, Output) {
    let directory = test_directory(name);
    std::fs::create_dir_all(&directory).unwrap();
    for extension in ["dvi", "log", "pdf"] {
        let _ = std::fs::remove_file(directory.join(format!("t.{extension}")));
    }
    std::fs::write(
        directory.join("t.tex"),
        format!("\\catcode123=1\n\\catcode125=2\n{body}\n\\end\n"),
    )
    .unwrap();

    let output = Command::new(binary)
        .args(options)
        .arg("t.tex")
        .current_dir(&directory)
        .output()
        .unwrap();
    (directory, output)
}

fn contains(bytes: &[u8], expected: &[u8]) -> bool {
    bytes
        .windows(expected.len())
        .any(|window| window == expected)
}

fn assert_success(output: &Output, directory: &Path) {
    assert!(
        output.status.success(),
        "TeX実行失敗 ({directory:?}):\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

const ONE_RULE_PAGE: &str = "\\count0=1\n\\shipout\\hbox{\\vrule width1pt height1pt}";

#[test]
fn quietは自動出力だけを端末から消す() {
    let body = format!(
        "\\message{{<<MESSAGE-PRESERVED>>}}\n\\immediate\\write16{{<<WRITE16-PRESERVED>>}}\n{ONE_RULE_PAGE}"
    );
    let (normal_directory, normal) = run_tex(env!("CARGO_BIN_EXE_pratex"), "通常出力", &[], &body);
    assert_success(&normal, &normal_directory);
    for expected in [
        b"This is PraTeX".as_slice(),
        b"(t.tex".as_slice(),
        b"[1]".as_slice(),
        b"Output written on t.dvi".as_slice(),
        b"Transcript written on t.log".as_slice(),
        b"<<MESSAGE-PRESERVED>>".as_slice(),
        b"<<WRITE16-PRESERVED>>".as_slice(),
    ] {
        assert!(
            contains(&normal.stdout, expected),
            "通常出力に {:?} がない:\n{}",
            String::from_utf8_lossy(expected),
            String::from_utf8_lossy(&normal.stdout),
        );
    }

    let (quiet_directory, quiet) = run_tex(
        env!("CARGO_BIN_EXE_pratex"),
        "quiet出力",
        &["--quiet"],
        &body,
    );
    assert_success(&quiet, &quiet_directory);
    for preserved in [
        b"<<MESSAGE-PRESERVED>>".as_slice(),
        b"<<WRITE16-PRESERVED>>".as_slice(),
    ] {
        assert!(
            contains(&quiet.stdout, preserved),
            "quietが明示出力 {:?} まで消した:\n{}",
            String::from_utf8_lossy(preserved),
            String::from_utf8_lossy(&quiet.stdout),
        );
    }
    for automatic in [
        b"This is PraTeX".as_slice(),
        b"(t.tex".as_slice(),
        b"[1]".as_slice(),
        b"Output written on t.dvi".as_slice(),
        b"Transcript written on t.log".as_slice(),
    ] {
        assert!(
            !contains(&quiet.stdout, automatic),
            "quietに自動出力 {:?} が残った:\n{}",
            String::from_utf8_lossy(automatic),
            String::from_utf8_lossy(&quiet.stdout),
        );
    }

    // `--quiet` は端末 policy だけであり、transcript の情報量は減らさない。
    let quiet_log = std::fs::read(quiet_directory.join("t.log")).unwrap();
    for expected in [
        b"This is PraTeX".as_slice(),
        b"[1]".as_slice(),
        b"Output written on t.dvi".as_slice(),
        b"<<MESSAGE-PRESERVED>>".as_slice(),
        b"<<WRITE16-PRESERVED>>".as_slice(),
    ] {
        assert!(
            contains(&quiet_log, expected),
            "quietのlogに {:?} がない:\n{}",
            String::from_utf8_lossy(expected),
            String::from_utf8_lossy(&quiet_log),
        );
    }
}

#[test]
fn quietでもtracingoutputの明示診断を端末へ出す() {
    let body = format!("\\tracingoutput=1\n{ONE_RULE_PAGE}");
    let (directory, output) = run_tex(
        env!("CARGO_BIN_EXE_pratex"),
        "quiet tracingoutput",
        &["--quiet"],
        &body,
    );
    assert_success(&output, &directory);
    assert!(
        contains(&output.stdout, b"Completed box being shipped out"),
        "tracingoutputの診断が端末にない:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
    assert!(
        contains(&output.stdout, b"[1]"),
        "tracingoutputが要求したpage番号が端末にない:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
    assert!(!contains(&output.stdout, b"Output written on t.dvi"));
}

#[test]
fn quietでもtexのerrorを端末へ出す() {
    let body = format!("\\nonstopmode\n\\undefinedquietcontrolsequence\n{ONE_RULE_PAGE}");
    let (_directory, output) = run_tex(
        env!("CARGO_BIN_EXE_pratex"),
        "quiet error",
        &["--quiet"],
        &body,
    );
    // 回復可能なTeX errorのprocess statusは互換性のため将来変わり得るので固定しない。
    assert!(
        contains(&output.stdout, b"! Undefined control sequence."),
        "quietがTeX errorを端末から消した:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
    assert!(
        contains(&output.stdout, b"undefinedquietcontrolsequence"),
        "error文脈に原因の制御綴がない:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
    assert!(!contains(&output.stderr, b"panicked at"));
}

#[test]
fn 互換rtexバイナリもpratexを名乗る() {
    let (directory, output) = run_tex(env!("CARGO_BIN_EXE_rtex"), "rtex互換名", &[], ONE_RULE_PAGE);
    assert_success(&output, &directory);
    assert!(
        contains(&output.stdout, b"This is PraTeX, Version 0.1.0-dev"),
        "互換binaryのbannerがPraTeXでない:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
    assert!(!contains(&output.stdout, b"This is rtex"));
}

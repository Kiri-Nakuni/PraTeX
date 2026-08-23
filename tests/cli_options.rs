//! Web2C互換CLIがparserだけでなく、同じPraTeX engine runへ実際に効くことを確認する。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn test_directory(name: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};

    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-cli-options-{}-{:x}",
        std::process::id(),
        hash.finish()
    ))
}

fn prepare_directory(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let directory = test_directory(name);
    std::fs::create_dir_all(&directory).unwrap();
    for (file_name, contents) in files {
        std::fs::write(directory.join(file_name), contents).unwrap();
    }
    directory
}

fn run(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pratex"))
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "PraTeX実行失敗:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn helpとversionはengineを起動せず成功する() {
    let directory = prepare_directory("help version", &[]);

    let help = run(&directory, &["--help"]);
    assert_success(&help);
    assert!(contains(&help.stdout, b"Usage: pratex"));
    for option in [
        b"-fmt=NAME".as_slice(),
        b"-interaction=MODE".as_slice(),
        b"-halt-on-error".as_slice(),
        b"--                           pass all following".as_slice(),
    ] {
        assert!(contains(&help.stdout, option));
    }
    assert!(!contains(&help.stdout, b"(INITEX)"));
    assert!(help.stderr.is_empty());

    let version = run(&directory, &["--version"]);
    assert_success(&version);
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "This is PraTeX, Version 0.1.0-dev"
    );
    assert!(version.stderr.is_empty());
}

#[test]
fn 未知optionは失敗し二重dash後のdash名はtexへ渡す() {
    let directory = prepare_directory(
        "unknown and boundary",
        &[(
            "-leading.tex",
            concat!(
                "\\catcode123=1\n\\catcode125=2\n",
                "\\message{<<DASH-NAME-READ>>}\\end\n"
            ),
        )],
    );

    let unknown = run(&directory, &["--definitely-unknown"]);
    assert!(!unknown.status.success());
    assert!(contains(&unknown.stderr, b"unknown option"));
    assert!(contains(&unknown.stderr, b"use `--` before TeX input"));
    assert!(!contains(&unknown.stdout, b"(INITEX)"));

    let boundary = run(&directory, &["--", "-leading.tex"]);
    assert_success(&boundary);
    assert!(contains(&boundary.stdout, b"<<DASH-NAME-READ>>"));
}

#[test]
fn interactionの四modeがfmtでなくrunを上書きする() {
    let directory = prepare_directory(
        "interaction modes",
        &[(
            "mode.tex",
            concat!(
                "\\catcode123=1\n\\catcode125=2\n",
                "\\message{<<INTERACTION=\\the\\interactionmode>>}\\end\n"
            ),
        )],
    );

    for (name, value) in [
        ("batchmode", b'0'),
        ("nonstopmode", b'1'),
        ("scrollmode", b'2'),
        ("errorstopmode", b'3'),
    ] {
        let log = directory.join("mode.log");
        let _ = std::fs::remove_file(&log);
        let joined = format!("-interaction={name}");
        let output = if name == "nonstopmode" {
            run(&directory, &["-interaction", name, "mode.tex"])
        } else {
            run(&directory, &[&joined, "mode.tex"])
        };
        assert_success(&output);
        let transcript = std::fs::read(&log).unwrap();
        let expected = [
            b'<', b'<', b'I', b'N', b'T', b'E', b'R', b'A', b'C', b'T', b'I', b'O', b'N', b'=',
            value,
        ];
        assert!(
            contains(&transcript, &expected),
            "{name}がlogへ反映されない:\n{}",
            String::from_utf8_lossy(&transcript)
        );
    }
}

#[test]
fn iniで作ったfmtをfmt_optionから読み先頭ampersandを優先する() {
    let directory = prepare_directory(
        "ini and fmt",
        &[
            (
                "alpha.tex",
                "\\catcode123=1\n\\catcode125=2\n\\def\\fromfmt{ALPHA}\\dump\n",
            ),
            (
                "beta.tex",
                "\\catcode123=1\n\\catcode125=2\n\\def\\fromfmt{BETA}\\dump\n",
            ),
            (
                "use.tex",
                "\\message{<<FMT=\\fromfmt>>}\\message{<<MODE=\\the\\interactionmode>>}\\end\n",
            ),
        ],
    );

    for source in ["alpha.tex", "beta.tex"] {
        let output = run(&directory, &["-ini", source]);
        assert_success(&output);
        assert!(directory.join(source.replace(".tex", ".fmt")).is_file());
    }

    let selected = run(&directory, &["-fmt=alpha", "use.tex"]);
    assert_success(&selected);
    assert!(contains(&selected.stdout, b"<<FMT=ALPHA>>"));

    let overridden = run(
        &directory,
        &["-fmt=alpha", "-interaction=batchmode", "use.tex"],
    );
    assert_success(&overridden);
    let transcript = std::fs::read(directory.join("use.log")).unwrap();
    assert!(contains(&transcript, b"<<MODE=0>>"));

    let ampersand = run(&directory, &["-fmt", "alpha", "&beta", "use.tex"]);
    assert_success(&ampersand);
    assert!(contains(&ampersand.stdout, b"<<FMT=BETA>>"));
    assert!(!contains(&ampersand.stdout, b"<<FMT=ALPHA>>"));
}

#[test]
fn halt_on_errorは最初の回復可能errorで失敗終了する() {
    let directory = prepare_directory(
        "halt on error",
        &[(
            "error.tex",
            concat!(
                "\\catcode123=1\n\\catcode125=2\n",
                "\\message{<<BEFORE-ERROR>>}\n",
                "\\undefinedclicontrolsequence\n",
                "\\message{<<AFTER-ERROR>>}\n",
                "\\end\n",
            ),
        )],
    );

    let recovering = run(&directory, &["-interaction=nonstopmode", "error.tex"]);
    assert_success(&recovering);
    assert!(contains(&recovering.stdout, b"<<AFTER-ERROR>>"));

    let halted = run(
        &directory,
        &["-interaction=nonstopmode", "-halt-on-error", "error.tex"],
    );
    assert!(!halted.status.success());
    assert!(contains(&halted.stdout, b"! Undefined control sequence."));
    assert!(contains(&halted.stdout, b"<<BEFORE-ERROR>>"));
    assert!(!contains(&halted.stdout, b"<<AFTER-ERROR>>"));
    assert!(!contains(&halted.stderr, b"panicked at"));
}

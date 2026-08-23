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

fn dvi_comment(path: &Path) -> Vec<u8> {
    let dvi = std::fs::read(path).unwrap();
    assert_eq!(dvi.first(), Some(&247), "DVI preambleがない");
    assert_eq!(dvi.get(1), Some(&2), "DVI id byteがTeX形式でない");
    let length = *dvi.get(14).expect("DVI comment lengthがない") as usize;
    dvi.get(15..15 + length)
        .expect("DVI commentが途中で切れている")
        .to_vec()
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
        b"-jobname=STRING".as_slice(),
        b"-output-comment=STRING".as_slice(),
        b"-no-shell-escape".as_slice(),
        b"-no-mktex=TYPE".as_slice(),
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

#[test]
fn jobnameはtex値とlogと全出力のbasenameを一貫して変える() {
    let directory = prepare_directory(
        "job name identity",
        &[
            (
                "source.tex",
                concat!(
                    "\\catcode123=1 \\catcode125=2\n",
                    "\\message{<<JOB=\\jobname>>}\n",
                    "\\shipout\\hbox{\\vrule width 1pt height 1pt}\n",
                    "\\end\n",
                ),
            ),
            ("dump.tex", "\\catcode123=1 \\catcode125=2 \\dump\n"),
        ],
    );
    std::fs::create_dir_all(directory.join("nested")).unwrap();

    let dvi = run(
        &directory,
        &["--jobname", "nested/dot.name", "source.tex"],
    );
    assert_success(&dvi);
    assert!(contains(&dvi.stdout, b"<<JOB=nested/dot.name>>"));
    assert!(directory.join("nested/dot.name.log").is_file());
    assert!(directory.join("nested/dot.name.dvi").is_file());
    assert!(!directory.join("nested/dot.dvi").exists());
    assert!(!directory.join("source.dvi").exists());

    let pdf = run(
        &directory,
        &[
            "-output-format=pdf",
            "-jobname=nested/pdf.name",
            "source.tex",
        ],
    );
    assert_success(&pdf);
    assert!(contains(&pdf.stdout, b"<<JOB=nested/pdf.name>>"));
    assert!(directory.join("nested/pdf.name.log").is_file());
    assert!(directory.join("nested/pdf.name.pdf").is_file());

    let format = run(
        &directory,
        &["-ini", "--jobname=nested/fmt.name", "dump.tex"],
    );
    assert_success(&format);
    assert!(directory.join("nested/fmt.name.log").is_file());
    assert!(directory.join("nested/fmt.name.fmt").is_file());
}

#[test]
fn 空jobnameは空のtex値とdot始まりの出力名になる() {
    let directory = prepare_directory(
        "empty job name",
        &[(
            "source.tex",
            concat!(
                "\\catcode123=1 \\catcode125=2\n",
                "\\message{<<JOB=\\jobname>>}\n",
                "\\shipout\\hbox{\\vrule width 1pt height 1pt}\n",
                "\\end\n",
            ),
        )],
    );

    let output = run(&directory, &["-jobname=", "source.tex"]);
    assert_success(&output);
    assert!(contains(&output.stdout, b"<<JOB=>>"));
    assert!(directory.join(".log").is_file());
    assert!(directory.join(".dvi").is_file());
}

#[test]
fn output_commentはdviだけへ指定byte列をそのまま置く() {
    let directory = prepare_directory(
        "output comment",
        &[(
            "source.tex",
            concat!(
                "\\catcode123=1 \\catcode125=2\n",
                "\\shipout\\hbox{\\vrule width 1pt height 1pt}\\end\n",
            ),
        )],
    );

    let specified = run(
        &directory,
        &[
            "-jobname=specified",
            "--output-comment",
            "CLI-COMMENT",
            "source.tex",
        ],
    );
    assert_success(&specified);
    assert_eq!(dvi_comment(&directory.join("specified.dvi")), b"CLI-COMMENT");

    let empty = run(
        &directory,
        &["-jobname=empty", "-output-comment=", "source.tex"],
    );
    assert_success(&empty);
    assert_eq!(dvi_comment(&directory.join("empty.dvi")), b"");

    let maximum = "A".repeat(255);
    let maximum_option = format!("-output-comment={maximum}");
    let maximum_output = run(
        &directory,
        &["-jobname=maximum", &maximum_option, "source.tex"],
    );
    assert_success(&maximum_output);
    assert_eq!(dvi_comment(&directory.join("maximum.dvi")), maximum.as_bytes());

    let pdf = run(
        &directory,
        &[
            "-output-format=pdf",
            "-jobname=pdf-comment",
            "-output-comment=PDF-IGNORED",
            "source.tex",
        ],
    );
    assert_success(&pdf);
    let pdf_bytes = std::fs::read(directory.join("pdf-comment.pdf")).unwrap();
    assert!(!contains(&pdf_bytes, b"PDF-IGNORED"));
}

#[test]
fn 外部生成を無効にする指定は実行状態も無効のままにする() {
    let directory = prepare_directory(
        "disabled external execution",
        &[(
            "policy.tex",
            concat!(
                "\\catcode123=1 \\catcode125=2\n",
                "\\message{<<SHELL=\\the\\pdfshellescape>>}\\end\n",
            ),
        )],
    );

    let output = run(
        &directory,
        &[
            "--no-shell-escape",
            "-no-mktex",
            "tex",
            "--no-mktex=tfm",
            "policy.tex",
        ],
    );
    assert_success(&output);
    assert!(contains(&output.stdout, b"<<SHELL=0>>"));
}

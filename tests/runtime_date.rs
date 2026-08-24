use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LEAP_DAY_EPOCH: &str = "1709210096";

fn test_directory(name: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};

    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-runtime-date-{}-{:x}",
        std::process::id(),
        hash.finish()
    ))
}

fn run_tex(name: &str, epoch: &str, body: &str) -> (PathBuf, Output) {
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
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(["--quiet", "--", "t.tex"])
        .env("SOURCE_DATE_EPOCH", epoch)
        .current_dir(&directory)
        .output()
        .unwrap();
    (directory, output)
}

fn assert_success(output: &Output, directory: &Path) {
    assert!(
        output.status.success(),
        "TeX実行失敗 ({directory:?}):\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn dvi_comment(bytes: &[u8]) -> &[u8] {
    assert_eq!(bytes.first(), Some(&247), "DVI preambleがない");
    let length = bytes[14] as usize;
    &bytes[15..15 + length]
}

#[test]
fn 固定epochをtex_log_pdf_dviで一つだけ共有する() {
    let body = r#"
\message{[DATE=\the\year-\the\month-\the\day;TIME=\the\time;PDF=\pdfcreationdate]}
\year=1999 \month=1 \day=2 \time=3
\message{[MUTATED-PDF=\pdfcreationdate]}
\shipout\hbox{\vrule width1pt height1pt}
"#;
    let (directory, output) = run_tex("全consumer", LEAP_DAY_EPOCH, body);
    assert_success(&output, &directory);

    let log = std::fs::read_to_string(directory.join("t.log")).unwrap();
    assert!(log.contains("29 FEB 2024 12:34"), "transcript時刻: {log}");
    assert!(
        log.contains("[DATE=2024-2-29;TIME=754;PDF=D:20240229123456+00'00']"),
        "TeX/PDF時刻: {log}"
    );
    assert!(
        log.contains("[MUTATED-PDF=D:20240229123456+00'00']"),
        "途中のregister代入がrun snapshotを変えた: {log}"
    );

    let dvi = std::fs::read(directory.join("t.dvi")).unwrap();
    assert_eq!(dvi_comment(&dvi), b" PraTeX output 1999.01.02:0003");
}

#[test]
fn fmt識別日はdump時のtex_parameter代入を反映する() {
    let body = r#"\year=1999 \month=1 \day=2 \time=3
\dump"#;
    let (directory, output) = run_tex("fmt識別日", LEAP_DAY_EPOCH, body);
    assert_success(&output, &directory);
    let format = std::fs::read(directory.join("t.fmt")).unwrap();
    assert!(format.starts_with(b"PRATEXF\0"));
    std::fs::write(directory.join("use.tex"), "\\end\n").unwrap();
    let loaded = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(["&t", "use.tex"])
        .current_dir(&directory)
        .output()
        .unwrap();
    assert_success(&loaded, &directory);
    let log = std::fs::read_to_string(directory.join("use.log")).unwrap();
    assert!(log.contains(" (preloaded format=t 1999.1.2)"), "{log}");
}

#[test]
fn 不正な固定epochをutc現在時刻へfallbackしない() {
    let (directory, output) = run_tex("不正epoch", "not-a-timestamp", "");
    assert!(!output.status.success(), "不正値を受理した: {directory:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SOURCE_DATE_EPOCH"), "{stderr}");
    assert!(stderr.contains("integral Unix timestamp"), "{stderr}");
    assert!(!directory.join("t.log").exists());
}

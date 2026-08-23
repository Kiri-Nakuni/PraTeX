//! pdfTeX互換の`\pdfmdfivesum`文字列/file形式。
//!
//! 上流source/testは使わず、公開manualと公式binaryのblack-box観測から作った入力だけを置く。

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temporary_directory() -> std::path::PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "pratex-pdfmdfivesum-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn file形式は展開したkeywordと引用符つきutf8名を読み外側の展開を壊さない() {
    let directory = temporary_directory();
    let contents = b"\0\xFF\r\nPraTeX file MD5\n\0";
    std::fs::write(directory.join("引用 空白.tex"), contents).unwrap();
    std::fs::write(
        directory.join("t.tex"),
        r#"\catcode`\{=1
\catcode`\}=2
\catcode`\#=6
\batchmode
\def\FILEKEY{FiLe}
\def\FILENAME{"引用 空白.tex"}
\def\IDENTITY#1{#1}
\message{[string=\pdfmdfivesum{abc}]}
\message{[before/\pdfmdfivesum \FILEKEY {\IDENTITY{\FILENAME}}/after]}
\message{[missing=\pdfmdfivesum file{存在しない.tex}]}
\end
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&directory)
        .output()
        .unwrap();
    let log = std::fs::read_to_string(directory.join("t.log"))
        .unwrap()
        .replace(['\r', '\n'], "");

    assert!(
        output.status.success(),
        "{}\n{log}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        log.contains("[string=900150983CD24FB0D6963F7D28E17F72]"),
        "{log}"
    );
    // crate自身のMD5をoracleにせず、独立したRFC 1321実装で求めた既知値を固定する。
    assert!(
        log.contains("[before/AF90067940AE3F46B88AEDAA9DA079F4/after]"),
        "{log}"
    );
    assert!(log.contains("[missing=]"), "{log}");
    assert!(!log.contains("!"), "{log}");
}

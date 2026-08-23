//! 生文字列registerの公開command境界。
//!
//! literal/file producerと`\the`の行境界規則は未決定なので、このprocess試験は
//! register copy・alias・group/globaldefs・`\therawstring`の確定部分だけを通す。
//! 全slot空の構文smokeであり、非空`Rc`の移動意味は`command::prefixable`のunit試験が担う。

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn 一時directory() -> std::path::PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "pratex-raw-string-registers-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn aliasとregister_copyとgroup_globaldefsを公開primitiveで処理する() {
    let directory = 一時directory();
    std::fs::write(
        directory.join("raw.tex"),
        r#"\catcode`\{=1
\catcode`\}=2
\catcode`\#=6
\batchmode
\rawstringdef\a=7
\let\b=\a
\ifx\a\b \message{[alias=yes]}\else\message{[alias=no]}\fi
\toksdef\t=7
\ifx\a\t \message{[domain=bad]}\else\message{[domain=separate]}\fi
\rawstring7=\rawstring8
{\rawstring7=\rawstring9}
{\globaldefs=1 \rawstring7=\rawstring10}
\message{[exact=\therawstring\a]}
\show\a
\showthe\a
\end
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("raw.tex")
        .current_dir(&directory)
        .output()
        .unwrap();
    let log = std::fs::read_to_string(directory.join("raw.log"))
        .unwrap()
        .replace(['\r', '\n'], "");

    assert!(
        output.status.success(),
        "{}\n{log}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(log.contains("[alias=yes]"), "{log}");
    assert!(log.contains("[domain=separate]"), "{log}");
    assert!(log.contains("[exact=]"), "{log}");
    assert!(log.contains("\\a=\\rawstring7"), "{log}");
    assert!(!log.contains("!"), "{log}");
}

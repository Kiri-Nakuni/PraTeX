//! 寸法の先頭tokenを整数走査へ直接渡す経路。

use std::io::Write;
use std::process::Command;

fn run_tex(body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("dimension-first-token-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut file = std::fs::File::create(&src).unwrap();
    write!(
        file,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\catcode`\\#=6\n\\batchmode\n{body}\n\\end\n"
    )
    .unwrap();
    drop(file);

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(["-ini", "-halt-on-error", "t.tex"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "寸法入力を処理できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(dir.join("t.log")).unwrap()
}

#[test]
fn 展開済み先頭を小数と各進数の寸法へ渡す() {
    let log = run_tex(
        "\\def\\n{12}\n\
         \\dimen0=.5pt \\dimen1=,5pt \\dimen2=--12pt\n\
         \\dimen3='20pt \\dimen4=\"10pt \\dimen5=`Apt \\dimen6=\\n.5pt\n\
         \\message{[\\the\\dimen0/\\the\\dimen1/\\the\\dimen2/\\the\\dimen3/\
         \\the\\dimen4/\\the\\dimen5/\\the\\dimen6]}",
    );

    assert!(
        log.contains("[0.5pt/0.5pt/12.0pt/16.0pt/16.0pt/65.0pt/12.5pt]"),
        "{log}"
    );
}

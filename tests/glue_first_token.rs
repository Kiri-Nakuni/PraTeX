//! 糊の先頭tokenを寸法走査へ直接渡す経路。

use std::io::Write;
use std::process::Command;

fn run_tex(body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("glue-first-token-{}", std::process::id()));
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
        "糊入力を処理できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(dir.join("t.log"))
        .unwrap()
        .replace('\n', "")
}

#[test]
fn 明示糊の符号は幅だけに掛けて内部糊は全成分を反転する() {
    let log = run_tex(
        "\\def\\n{3}\n\
         \\skip0=-1.5pt plus 2fil minus 3fill\n\
         \\skip1=--2pt plus -4fill minus -5fil\n\
         \\skip2=-\\n.25pt plus 6pt minus 7pt\n\
         \\muskip0=-1.5mu plus 2fil minus 3mu\n\
         \\skip3=1pt plus 2fil minus 3fill \\skip4=-\\skip3\n\
         \\message{[A=\\the\\skip0][B=\\the\\skip1][C=\\the\\skip2]\
         [M=\\the\\muskip0][O=\\the\\skip3][I=\\the\\skip4]}",
    );

    assert!(
        log.contains("[A=-1.5pt plus 2.0fil minus 3.0fill]"),
        "{log}"
    );
    assert!(
        log.contains("[B=2.0pt plus -4.0fill minus -5.0fil]"),
        "{log}"
    );
    assert!(log.contains("[C=-3.25pt plus 6.0pt minus 7.0pt]"), "{log}");
    assert!(log.contains("[M=-1.5mu plus 2.0fil minus 3.0mu]"), "{log}");
    assert!(log.contains("[O=1.0pt plus 2.0fil minus 3.0fill]"), "{log}");
    assert!(
        log.contains("[I=-1.0pt plus -2.0fil minus -3.0fill]"),
        "{log}"
    );
}

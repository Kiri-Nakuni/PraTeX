//! pTeX互換の漢字間隔parameter。
//!
//! 実際の自動挿入規則とは分離し、ここでは通常glueとしての契約を固定する。

use std::hash::{Hash, Hasher};
use std::process::{Command, Output};

fn case_dir(name: &str) -> std::path::PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    let dir = std::env::temp_dir().join(format!(
        "pratex-kanjiskip-{}-{:x}",
        std::process::id(),
        hash.finish()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn join_log(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap().replace('\n', "")
}

fn run_tex(name: &str, body: &str) -> String {
    let dir = case_dir(name);
    let log_path = dir.join("t.log");
    let _ = std::fs::remove_file(&log_path);
    std::fs::write(
        dir.join("t.tex"),
        format!("\\catcode123=1\n\\catcode125=2\n\\batchmode\n{body}\n\\end\n"),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        output.status.success() && log_path.exists(),
        "PraTeXを実行できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    join_log(&log_path)
}

fn make_format(name: &str, body: &str) -> (std::path::PathBuf, Output) {
    let dir = case_dir(name);
    let fmt_path = dir.join("mk.fmt");
    let _ = std::fs::remove_file(&fmt_path);
    let _ = std::fs::remove_file(dir.join("mk.log"));
    std::fs::write(
        dir.join("mk.tex"),
        format!("\\catcode123=1\n\\catcode125=2\n\\batchmode\n{body}\n\\dump\n"),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("mk.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    (dir, output)
}

#[test]
fn 漢字間隔と和欧文間隔はinitexでは零である() {
    let log = run_tex(
        "初期値と表示",
        "\\let\\K=\\kanjiskip \\let\\X=\\xkanjiskip\n\
         \\message{[value=\\the\\K/\\the\\X]}\n\
         \\message{[meaning=\\meaning\\K/\\meaning\\X]}\n\
         \\ifx\\K\\kanjiskip\\message{[k=same]}\\else\\message{[k=different]}\\fi\n\
         \\ifx\\X\\xkanjiskip\\message{[x=same]}\\else\\message{[x=different]}\\fi\n\
         \\show\\kanjiskip \\showthe\\xkanjiskip",
    );
    assert!(log.contains("[value=0.0pt/0.0pt]"), "{log}");
    assert!(log.contains("[meaning=\\kanjiskip/\\xkanjiskip]"), "{log}");
    assert!(log.contains("[k=same]"), "{log}");
    assert!(log.contains("[x=same]"), "{log}");
    assert!(log.contains("> \\kanjiskip=\\kanjiskip."), "{log}");
    assert!(log.contains("> 0.0pt."), "{log}");
}

#[test]
fn 漢字間隔は局所大域と算術代入に従う() {
    let log = run_tex(
        "保存と算術",
        "\\kanjiskip=1pt plus 2pt minus 3pt\n\
         \\xkanjiskip=4pt\n\
         {\\advance\\kanjiskip by 4pt plus 5pt minus 6pt\n\
          \\multiply\\kanjiskip by 3 \\divide\\kanjiskip by 2\n\
          \\message{[local=\\the\\kanjiskip]}}\n\
         \\message{[restored=\\the\\kanjiskip]}\n\
         {\\globaldefs=1 \\xkanjiskip=8pt}\n\
         \\message{[forced-global=\\the\\xkanjiskip]}\n\
         {\\globaldefs=-1 \\global\\xkanjiskip=9pt\n\
          \\message{[forced-local=\\the\\xkanjiskip]}}\n\
         \\message{[forced-restored=\\the\\xkanjiskip]}",
    );
    assert!(
        log.contains("[local=7.5pt plus 10.5pt minus 13.5pt]"),
        "{log}"
    );
    assert!(
        log.contains("[restored=1.0pt plus 2.0pt minus 3.0pt]"),
        "{log}"
    );
    assert!(log.contains("[forced-global=8.0pt]"), "{log}");
    assert!(log.contains("[forced-local=9.0pt]"), "{log}");
    assert!(log.contains("[forced-restored=8.0pt]"), "{log}");
}

#[test]
fn 漢字間隔と和欧文間隔をfmtで往復する() {
    let (dir, output) = make_format(
        "fmt往復",
        "\\kanjiskip=11pt plus 12fil minus 13fill\n\
         \\xkanjiskip=14pt plus 15filll minus 16fil\n\
         \\let\\savedkanji=\\kanjiskip \\let\\savedxkanji=\\xkanjiskip",
    );
    let fmt_path = dir.join("mk.fmt");
    assert!(
        output.status.success() && fmt_path.exists(),
        "fmtを生成できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let use_log = dir.join("use.log");
    let _ = std::fs::remove_file(&use_log);
    std::fs::write(
        dir.join("use.tex"),
        "\\message{[k=\\the\\kanjiskip/\\the\\savedkanji/\\meaning\\savedkanji]}\n\
         \\message{[x=\\the\\xkanjiskip/\\the\\savedxkanji/\\meaning\\savedxkanji]}\n\
         \\end\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("&mk")
        .arg("use.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        output.status.success() && use_log.exists(),
        "fmtを読み戻せなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = join_log(&use_log);
    assert!(
        log.contains(
            "[k=11.0pt plus 12.0fil minus 13.0fill/11.0pt plus 12.0fil minus 13.0fill/\\kanjiskip]"
        ),
        "{log}"
    );
    assert!(
        log.contains(
            "[x=14.0pt plus 15.0filll minus 16.0fil/14.0pt plus 15.0filll minus 16.0fil/\\xkanjiskip]"
        ),
        "{log}"
    );
}

//! e-TeX `\showifs`の条件stack表示とformat境界。
//!
//! 公開e-TeX manual 3.3の診断契約と、公式binaryへの自作black-box観測だけを
//! 固定する。他engineのsourceや上流testには依存しない。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn 準備(name: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    module_path!().hash(&mut hasher);
    name.hash(&mut hasher);
    let directory = std::env::temp_dir().join(format!(
        "etex-showifs-{}-{:016x}",
        std::process::id(),
        hasher.finish()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for file in [
        "t.tex", "t.log", "t.dvi", "mk.tex", "mk.log", "mk.fmt", "use.tex", "use.log",
    ] {
        let _ = std::fs::remove_file(directory.join(file));
    }
    directory
}

fn 実行(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap()
}

fn 成功(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn show診断後まで完走(output: &Output, context: &str) {
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{context}:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn 結合log(path: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(path).unwrap()).replace(['\r', '\n'], "")
}

fn plain実行(name: &str, body: &str) -> String {
    let directory = 準備(name);
    std::fs::write(
        directory.join("t.tex"),
        format!("\\catcode123=1\n\\catcode125=2\n\\catcode35=6\n\\batchmode\n{body}\n\\end\n"),
    )
    .unwrap();
    let output = 実行(&directory, &["t.tex"]);
    show診断後まで完走(&output, name);
    結合log(&directory.join("t.log"))
}

#[test]
fn showifsは空stackと内側から外側の条件を表示する() {
    let log = plain実行(
        "空stackと入れ子",
        "\\showifs
         \\unless\\iffalse
           \\iffalse
           \\else
             \\showifs
           \\fi
         \\fi",
    );
    assert!(log.contains("### no active conditionals"), "{log}");
    let inner = log
        .find("### level 2: \\iffalse\\else entered on line 7")
        .expect("内側条件を表示する");
    let outer = log
        .find("### level 1: \\unless\\iffalse entered on line 6")
        .expect("外側条件を表示する");
    assert!(inner < outer, "内側から外側へ表示する: {log}");
}

#[test]
fn showifsはifcaseとunlessの枝を表示して条件状態を変えない() {
    let log = plain実行(
        "枝状態",
        "\\ifcase1 ZERO\\or
           \\message{[case-before=\\the\\currentiflevel/\\the\\currentiftype/\\the\\currentifbranch]}
           \\showifs
           \\message{[case-after=\\the\\currentiflevel/\\the\\currentiftype/\\the\\currentifbranch]}
         \\else BAD\\fi
         \\unless\\iftrue BAD\\else
           \\message{[unless-before=\\the\\currentiflevel/\\the\\currentiftype/\\the\\currentifbranch]}
           \\showifs
           \\message{[unless-after=\\the\\currentiflevel/\\the\\currentiftype/\\the\\currentifbranch]}
         \\fi",
    );
    assert!(
        log.contains("### level 1: \\ifcase entered on line 5"),
        "{log}"
    );
    assert!(
        log.contains("### level 1: \\unless\\iftrue\\else entered on line 10"),
        "{log}"
    );
    assert!(log.contains("[case-before=1/17/1]"), "{log}");
    assert!(log.contains("[case-after=1/17/1]"), "{log}");
    assert!(log.contains("[unless-before=1/-15/-1]"), "{log}");
    assert!(log.contains("[unless-after=1/-15/-1]"), "{log}");
    assert!(!log.contains("BAD"), "{log}");
}

#[test]
fn showifsは全modeでnodeを残さない() {
    let log = plain実行(
        "全mode",
        "\\iftrue
           \\showifs
           \\setbox0=\\hbox{\\showifs}
           \\setbox1=\\hbox{$\\showifs$}
           \\message{[widths=\\the\\wd0/\\the\\wd1][done]}
         \\fi",
    );
    assert_eq!(log.matches("### level 1: \\iftrue").count(), 3, "{log}");
    assert!(log.contains("[widths=0.0pt/0.0pt][done]"), "{log}");
}

#[test]
fn showifsは百一回でも通常errorの上限へ数えない() {
    let mut body = String::from("\\iftrue\n");
    for _ in 0..101 {
        body.push_str("\\showifs\n");
    }
    body.push_str("\\message{[done]}\\fi");
    let log = plain実行("百一回", &body);
    assert_eq!(log.matches("### level 1: \\iftrue").count(), 101, "{log}");
    assert!(log.contains("[done]"), "{log}");
    assert!(!log.contains("100 errors"), "{log}");
}

#[test]
fn showifsのprimitiveとaliasはformatを往復する() {
    let directory = 準備("format往復");
    std::fs::write(
        directory.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\\let\\ifsshow=\\showifs\n\\dump\n",
    )
    .unwrap();
    let build = 実行(&directory, &["mk.tex"]);
    成功(&build, "showifs format生成");
    assert!(directory.join("mk.fmt").is_file());

    std::fs::write(
        directory.join("use.tex"),
        "\\batchmode
         \\message{[primitive=\\meaning\\showifs/alias=\\meaning\\ifsshow]}
         \\iftrue\\ifsshow\\fi
         \\end\n",
    )
    .unwrap();
    let load = 実行(&directory, &["&mk", "use.tex"]);
    show診断後まで完走(&load, "showifs format読戻し");
    let log = 結合log(&directory.join("use.log"));
    assert!(
        log.contains("[primitive=\\showifs/alias=\\showifs]"),
        "{log}"
    );
    assert!(log.contains("### level 1: \\iftrue"), "{log}");
}

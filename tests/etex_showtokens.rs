//! e-TeX `\showtokens` の非展開走査・表示・format境界。
//!
//! 公開e-TeX manualのgeneral text契約と、公式binaryのblack-box観測だけを
//! 固定する。他engineのsourceや上流testには依存しない。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn 準備(name: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    let directory = std::env::temp_dir().join(format!(
        "etex-showtokens-{}-{:x}",
        std::process::id(),
        hasher.finish()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for file in ["t.log", "t.dvi", "mk.log", "mk.fmt", "use.log", "use.dvi"] {
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
fn showtokensは本文を展開せず入れ子とparameter_tokenを表示する() {
    let log = plain実行(
        "本文を非展開表示",
        "\\count0=0
         \\def\\boom{\\global\\count0=9 EXPANDED}
         \\showtokens{{A}\\boom #1}
         \\message{[count=\\the\\count0]}",
    );
    assert!(log.contains("> {A}\\boom ##1."), "{log}");
    assert!(log.contains("[count=0]"), "{log}");
}

#[test]
fn showtokensは入口の展開だけを通し本文はそのまま保つ() {
    let log = plain実行(
        "入口だけ展開",
        "\\def\\value{VALUE}
         \\showtokens{\\value}
         \\showtokens\\expandafter{\\value}",
    );
    assert!(log.contains("> \\value ."), "{log}");
    assert!(log.contains("> VALUE."), "{log}");
}

#[test]
fn showtokensは空白と制御綴の既存表示規則を共有する() {
    let log = plain実行(
        "空白と制御綴",
        "\\showtokens{}
         \\showtokens{{}}
         \\showtokens{ }
         \\showtokens{ A }
         \\showtokens{A   B}
         \\showtokens{\\relax A}
         \\showtokens{\\?A}",
    );
    assert!(log.contains("> ."), "{log}");
    assert!(log.contains("> {}."), "{log}");
    assert!(log.contains(">  ."), "{log}");
    assert!(log.contains(">  A ."), "{log}");
    assert!(log.contains("> A B."), "{log}");
    assert!(log.contains("> \\relax A."), "{log}");
    assert!(log.contains("> \\?A."), "{log}");
}

#[test]
fn showtokensはparameter文字を現在のcatcodeに従って表示する() {
    let log = plain実行(
        "parameter文字のcatcode",
        "\\catcode35=12
         \\showtokens{#1}
         \\catcode35=6
         \\showtokens{#1}",
    );
    assert!(log.contains("> #1."), "{log}");
    assert!(log.contains("> ##1."), "{log}");
}

#[test]
fn showtokensは全modeでnodeを残さず後続処理を続ける() {
    let log = plain実行(
        "全mode",
        "\\showtokens{VERTICAL}
         \\setbox0=\\hbox{\\showtokens{HORIZONTAL}}
         \\setbox1=\\hbox{$\\showtokens{MATH}$}
         \\message{[widths=\\the\\wd0/\\the\\wd1][done]}",
    );
    assert!(log.contains("> VERTICAL."), "{log}");
    assert!(log.contains("> HORIZONTAL."), "{log}");
    assert!(log.contains("> MATH."), "{log}");
    assert!(log.contains("[widths=0.0pt/0.0pt][done]"), "{log}");
}

#[test]
fn showtokensは百一回でも通常errorの上限へ数えない() {
    for (name, interaction) in [("batch", "\\batchmode"), ("nonstop", "\\nonstopmode")] {
        let mut body = format!("{interaction}\n");
        for _ in 0..101 {
            body.push_str("\\showtokens{}\n");
        }
        body.push_str("\\message{[done]}");
        let log = plain実行(&format!("百一回-{name}"), &body);
        assert_eq!(log.matches("> .").count(), 101, "{name}: {log}");
        assert!(log.contains("[done]"), "{name}: {log}");
        assert!(!log.contains("100 errors"), "{name}: {log}");
    }
}

#[test]
fn showtokensのaliasと和文tokenは既存表示経路を使う() {
    let log = plain実行(
        "aliasと和文",
        "\\kcatcode\"3042=16 \\kcatcode\"548C=16
         \\let\\tokenshow=\\showtokens
         \\message{[primitive=\\meaning\\showtokens/alias=\\meaning\\tokenshow]}
         \\tokenshow{あ#1}
         \\tokenshow{\\和}",
    );
    assert!(
        log.contains("[primitive=\\showtokens/alias=\\showtokens]"),
        "{log}"
    );
    assert!(log.contains("> あ##1."), "{log}");
    assert!(log.contains("> \\和 ."), "{log}");
}

#[test]
fn showtokensのprimitiveとaliasはformatを往復する() {
    let directory = 準備("format往復");
    std::fs::write(
        directory.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\\let\\tokenshow=\\showtokens\n\\dump\n",
    )
    .unwrap();
    let build = 実行(&directory, &["mk.tex"]);
    成功(&build, "showtokens format生成");
    assert!(directory.join("mk.fmt").is_file());

    std::fs::write(
        directory.join("use.tex"),
        "\\batchmode
         \\message{[primitive=\\meaning\\showtokens/alias=\\meaning\\tokenshow]}
         \\tokenshow{FMT \\undefined}
         \\end\n",
    )
    .unwrap();
    let load = 実行(&directory, &["&mk", "use.tex"]);
    show診断後まで完走(&load, "showtokens format読戻し");
    let log = 結合log(&directory.join("use.log"));
    assert!(
        log.contains("[primitive=\\showtokens/alias=\\showtokens]"),
        "{log}"
    );
    assert!(log.contains("> FMT \\undefined ."), "{log}");
}

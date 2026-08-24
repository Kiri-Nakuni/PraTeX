//! e-TeXのvertical discard保存。
//!
//! 公式e-TeX manual 3.11と5.2の公開契約から独立に、page builderと`\vsplit`が
//! 捨てたglue・kern・penaltyの所有権、消去時点、fmt上のprimitive identityを固定する。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn 試験directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    module_path!().hash(&mut hash);
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-etex-vdiscards-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ))
}

fn 準備(name: &str) -> PathBuf {
    let directory = 試験directory(name);
    std::fs::create_dir_all(&directory).unwrap();
    for file in [
        "t.tex", "t.log", "t.dvi", "mk.tex", "mk.log", "mk.fmt", "use.tex", "use.log",
        "use.dvi",
    ] {
        let _ = std::fs::remove_file(directory.join(file));
    }
    directory
}

fn 実行(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap()
}

fn 成功(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn 結合log(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap()
        .replace('\r', "")
        .replace('\n', "")
}

#[test]
fn vsplitの残りから捨てたnodeを順序どおり一度だけ取り出す() {
    let directory = 準備("split保存と取出し");
    std::fs::write(
        directory.join("t.tex"),
        r"\catcode123=1
\catcode125=2
\batchmode
\showboxbreadth=100 \showboxdepth=100
\savingvdiscards=1
\setbox0=\vbox{
  \hrule height1pt width1pt
  \penalty-10000 \vskip2pt \kern3pt \penalty123
  \hrule height4pt width1pt
}
\setbox1=\vsplit0 to1pt
\setbox2=\vbox{\splitdiscards}
\message{[saved=\the\ht2/\the\dp2]}
\showbox2
\setbox3=\vbox{\splitdiscards}
\message{[taken=\the\ht3/\the\dp3]}
\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "split discard保存");
    let log = 結合log(&directory.join("t.log"));
    assert!(log.contains("[saved=5.0pt/0.0pt]"), "{log}");
    assert!(log.contains("[taken=0.0pt/0.0pt]"), "{log}");

    let shown = log.split("> \\box2=").nth(1).expect("box2の表示");
    let penalty_break = shown.find("\\penalty -10000").expect("break penalty");
    let glue = shown.find("\\glue 2.0").expect("discarded glue");
    let kern = shown.find("\\kern 3.0").expect("discarded kern");
    let penalty = shown.find("\\penalty 123").expect("discarded penalty");
    assert!(penalty_break < glue && glue < kern && kern < penalty, "{shown}");
}

#[test]
fn 新しいvsplitは前回の未回収listを開始時に空へ戻す() {
    let directory = 準備("split開始時消去");
    std::fs::write(
        directory.join("t.tex"),
        r"\catcode123=1
\catcode125=2
\batchmode
\savingvdiscards=1
\setbox0=\vbox{\hrule height1pt\penalty-10000\vskip7pt\hrule height1pt}
\setbox1=\vsplit0 to1pt
\setbox4=\vbox{}
\setbox5=\vsplit4 to0pt
\setbox6=\vbox{\splitdiscards}
\message{[cleared=\the\ht6/\the\dp6]}
\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "split discard開始時消去");
    let log = 結合log(&directory.join("t.log"));
    assert!(log.contains("[cleared=0.0pt/0.0pt]"), "{log}");
}

#[test]
fn page先頭のdiscardableだけをoutput中に回収し終端で世代を切る() {
    let directory = 準備("page保存と世代");
    std::fs::write(
        directory.join("t.tex"),
        r"\catcode123=1
\catcode125=2
\batchmode
\showboxbreadth=100 \showboxdepth=100
\savingvdiscards=1
\output={
  \global\setbox10=\vbox{\pagediscards}
  \global\setbox11=\box255
}
\vskip2pt \kern3pt \penalty123
\hrule height1pt width1pt
\penalty-10000
\message{[during=\the\ht10/\the\dp10]}
\showbox10
\setbox12=\vbox{\pagediscards}
\message{[next=\the\ht12/\the\dp12]}
\showbox12
\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "page discard保存");
    let log = 結合log(&directory.join("t.log"));
    assert!(log.contains("[during=5.0pt/0.0pt]"), "{log}");
    assert!(log.contains("[next=0.0pt/0.0pt]"), "{log}");

    let during = log.split("> \\box10=").nth(1).expect("box10の表示");
    let glue = during.find("\\glue 2.0").expect("page glue");
    let kern = during.find("\\kern 3.0").expect("page kern");
    let penalty = during.find("\\penalty 123").expect("page penalty");
    assert!(glue < kern && kern < penalty, "{during}");

    let next = log.split("> \\box12=").nth(1).expect("box12の表示");
    assert!(next.contains("\\penalty 10000"), "{next}");
    assert!(!next.contains("\\glue 2.0"), "{next}");
    assert!(!next.contains("\\kern 3.0"), "{next}");
}

#[test]
fn 非正値では捨てたnodeをspecial_listへ保存しない() {
    let directory = 準備("非正値");
    std::fs::write(
        directory.join("t.tex"),
        r"\catcode123=1
\catcode125=2
\batchmode
\savingvdiscards=0
\setbox0=\vbox{\hrule height1pt\penalty-10000\vskip9pt\hrule height1pt}
\setbox1=\vsplit0 to1pt
\setbox2=\vbox{\splitdiscards}
\message{[off=\the\ht2/\the\dp2]}
\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "非正値のdiscard保存");
    let log = 結合log(&directory.join("t.log"));
    assert!(log.contains("[off=0.0pt/0.0pt]"), "{log}");
}

#[test]
fn 二つのprimitive_identityはfmtを往復しspecial_listはrunを越えない() {
    let directory = 準備("fmt往復");
    std::fs::write(
        directory.join("mk.tex"),
        r"\catcode123=1
\catcode125=2
\batchmode
\let\pagealias=\pagediscards
\let\splitalias=\splitdiscards
\dump
",
    )
    .unwrap();
    let output = 実行(&directory, &["mk.tex"]);
    成功(&output, "discard primitive fmt生成");

    std::fs::write(
        directory.join("use.tex"),
        r"\ifx\pagealias\pagediscards\message{[page=ok]}\fi
\ifx\splitalias\splitdiscards\message{[split=ok]}\fi
\setbox0=\vbox{\pagealias\splitalias}
\message{[runlocal=\the\ht0/\the\dp0]}
\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["&mk", "use.tex"]);
    成功(&output, "discard primitive fmt読戻し");
    let log = 結合log(&directory.join("use.log"));
    for expected in ["[page=ok]", "[split=ok]", "[runlocal=0.0pt/0.0pt]"] {
        assert!(log.contains(expected), "missing {expected}: {log}");
    }
}

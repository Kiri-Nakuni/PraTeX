//! e-TeXの段落行間penalty配列。
//!
//! 公式e-TeX manual 3.8の公開契約から独立に、代入・照会・fmt・実際の
//! post-line-break挿入をprocess境界で固定する。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn 試験directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    module_path!().hash(&mut hash);
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-etex-penalty-arrays-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ))
}

fn 準備(name: &str) -> PathBuf {
    let directory = 試験directory(name);
    std::fs::create_dir_all(&directory).unwrap();
    for file in [
        "t.tex",
        "t.log",
        "t.dvi",
        "mk.tex",
        "mk.log",
        "mk.fmt",
        "use.tex",
        "use.log",
        "use.dvi",
        "penaltymetric.tfm",
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
fn 配列は照会と局所大域代入とresetを一つの状態として扱う() {
    let directory = 準備("代入と照会");
    std::fs::write(
        directory.join("t.tex"),
        r"\catcode123=1
\catcode125=2
\batchmode
\interlinepenalties=3 11 22 33
\clubpenalties=2 44 55
\widowpenalties=1 66
\displaywidowpenalties=2 77 88
\message{[i=\the\interlinepenalties-1/\the\interlinepenalties0/\the\interlinepenalties1/\the\interlinepenalties99]}
\message{[c=\the\clubpenalties0/\the\clubpenalties1/\the\clubpenalties9]}
\message{[w=\the\widowpenalties0/\the\widowpenalties8]}
\message{[d=\the\displaywidowpenalties0/\the\displaywidowpenalties1/\the\displaywidowpenalties8]}
{\clubpenalties=1 99 \message{[local=\the\clubpenalties0/\the\clubpenalties8]}}
\message{[restored=\the\clubpenalties0/\the\clubpenalties8]}
{\global\widowpenalties=2 101 102}
\message{[global=\the\widowpenalties0/\the\widowpenalties9]}
{\globaldefs=1 \displaywidowpenalties=1 103}
{\globaldefs=-1 \global\displaywidowpenalties=1 104 \message{[forcedlocal=\the\displaywidowpenalties1]}}
\message{[globaldefs=\the\displaywidowpenalties0/\the\displaywidowpenalties1]}
\clubpenalties=0
\widowpenalties=-7 \count0=123
\message{[reset=\the\clubpenalties0/\the\widowpenalties0/count=\the\count0]}
\interlinepenalties=1 9
\clubpenalties=1 8
\indent\par
\message{[par-reset=\the\interlinepenalties0/club-stays=\the\clubpenalties0]}
\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "penalty配列の代入と照会");
    let log = 結合log(&directory.join("t.log"));
    for expected in [
        "[i=0/3/11/33]",
        "[c=2/44/55]",
        "[w=1/66]",
        "[d=2/77/88]",
        "[local=1/99]",
        "[restored=2/55]",
        "[global=2/102]",
        "[forcedlocal=104]",
        "[globaldefs=1/103]",
        "[reset=0/0/count=123]",
        "[par-reset=0/club-stays=1]",
    ] {
        assert!(log.contains(expected), "missing {expected}: {log}");
    }
}

#[test]
fn 四配列とaliasはformatを往復する() {
    let directory = 準備("fmt往復");
    std::fs::write(
        directory.join("mk.tex"),
        r"\catcode123=1
\catcode125=2
\batchmode
\interlinepenalties=2 10 11
\clubpenalties=1 20
\widowpenalties=3 30 31 32
\displaywidowpenalties=2 40 41
\let\savedclub=\clubpenalties
\dump
",
    )
    .unwrap();
    let output = 実行(&directory, &["mk.tex"]);
    成功(&output, "penalty配列fmt生成");
    assert!(directory.join("mk.fmt").is_file());

    std::fs::write(
        directory.join("use.tex"),
        r"\message{[fmt=\the\interlinepenalties0/\the\interlinepenalties9/\the\clubpenalties1/\the\widowpenalties2/\the\displaywidowpenalties9]}
\message{[alias=\the\savedclub7]}
\end
",
    )
    .unwrap();
    let output = 実行(&directory, &["&mk", "use.tex"]);
    成功(&output, "penalty配列fmt読戻し");
    let log = 結合log(&directory.join("use.log"));
    assert!(log.contains("[fmt=2/11/20/31/41]"), "{log}");
    assert!(log.contains("[alias=20]"), "{log}");
}

#[test]
fn 通常段落の各行へ配列penaltyを合算する() {
    let directory = 準備("通常段落への挿入");
    std::fs::write(
        directory.join("t.tex"),
        r"\catcode123=1
\catcode125=2
\batchmode
\showboxbreadth=100 \showboxdepth=100
\hsize=10pt \parindent=0pt
\interlinepenalty=0 \clubpenalty=0 \widowpenalty=0 \brokenpenalty=0
\clubpenalties=2 10 20
\widowpenalties=2 100 200
\setbox0=\vbox{
  \interlinepenalties=2 1 2
  \indent\hbox{}\penalty-10000
  \hbox{}\penalty-10000
  \hbox{}\penalty-10000
  \hbox{}\par
}
\showbox0
\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "通常段落へのpenalty配列挿入");
    let log = 結合log(&directory.join("t.log"));
    let shown_box = log.split("> \\box0=").nth(1).unwrap();
    for penalty in [211, 222, 122] {
        let needle = format!("\\penalty {penalty}.");
        assert_eq!(shown_box.matches(&needle).count(), 1, "{needle}: {log}");
    }
}

fn 数式用tfm() -> Vec<u8> {
    let mut bytes = Vec::new();
    // 2 header、code 0のみ、width/height/depth/italic各1、math symbol用22 parameters。
    for value in [35_u16, 2, 0, 0, 1, 1, 1, 1, 0, 0, 0, 22] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    for _ in 0..4 {
        bytes.extend_from_slice(&0_i32.to_be_bytes());
    }
    for index in 0..22 {
        let value = if index == 5 { 0x0010_0000_i32 } else { 0 };
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    assert_eq!(bytes.len(), 35 * 4);
    bytes
}

#[test]
fn display直前の部分段落へdisplaywidow配列を使う() {
    let directory = 準備("display直前への挿入");
    std::fs::write(directory.join("penaltymetric.tfm"), 数式用tfm()).unwrap();
    std::fs::write(
        directory.join("t.tex"),
        r"\catcode123=1
\catcode125=2
\catcode36=3
\batchmode
\font\f=penaltymetric
\textfont2=\f \scriptfont2=\f \scriptscriptfont2=\f
\textfont3=\f \scriptfont3=\f \scriptscriptfont3=\f
\showboxbreadth=100 \showboxdepth=100
\hsize=10pt \parindent=0pt
\interlinepenalty=0 \clubpenalty=0 \widowpenalty=0 \displaywidowpenalty=0
\predisplaypenalty=0 \postdisplaypenalty=0 \brokenpenalty=0
\displaywidowpenalties=2 1000 2000
\setbox0=\vbox{
  \indent\hbox{}\penalty-10000
  \hbox{}\penalty-10000
  \hbox{} $$\mathord{}$$
  \par
}
\showbox0
\end
",
    )
    .unwrap();

    let output = 実行(&directory, &["t.tex"]);
    成功(&output, "display直前へのpenalty配列挿入");
    let log = 結合log(&directory.join("t.log"));
    let shown_box = log.split("> \\box0=").nth(1).unwrap();
    assert_eq!(shown_box.matches("\\penalty 2000.").count(), 1, "{log}");
    assert_eq!(shown_box.matches("\\penalty 1000.").count(), 1, "{log}");
}

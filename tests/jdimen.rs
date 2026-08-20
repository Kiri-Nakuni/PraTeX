//! 和文の寸法単位（pTeX 由来）。
//!
//! `Q`（級）・`H`（歯）は **0.25 mm ちょうど**。
//! `zw`（全角幅）・`zh`（全角高）は**和文フォントに尋ねる単位**——
//! いまは和文フォントが無いので `em` で代用している。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("jdimen-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(f, "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n{body}\n\\end\n").unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex")).arg(&src).current_dir(&dir).output().unwrap();
    std::fs::read_to_string(dir.join("t.log")).unwrap()
}

#[test]
fn 級と歯は同じ長さ() {
    let log = run_tex("同じ", "\\dimen0=1Q \\dimen1=1H \\message{[\\the\\dimen0/\\the\\dimen1]}");
    assert!(log.contains("[0.7113pt/0.7113pt]"), "{log}");
}

#[test]
fn 四級はちょうど一ミリ() {
    // **0.25 mm ちょうど**であることの確かめ。丸め込みまで一致する
    let log = run_tex(
        "一ミリ",
        "\\dimen0=4Q \\dimen1=1mm \\dimen2=4H \\message{[\\the\\dimen0/\\the\\dimen1/\\the\\dimen2]}",
    );
    assert!(log.contains("[2.84526pt/2.84526pt/2.84526pt]"), "{log}");
}

#[test]
fn 小数も通る() {
    let log = run_tex("小数", "\\dimen0=12.3Q \\message{[\\the\\dimen0]}");
    assert!(log.contains("[8.74922pt]"), "{log}");
}

#[test]
fn 負も通る() {
    let log = run_tex("負", "\\dimen0=-2Q \\message{[\\the\\dimen0]}");
    assert!(log.contains("[-1.42262pt]"), "{log}");
}

#[test]
fn 全角幅は和文フォントに尋ねる() {
    // 和文フォントが無いので `em` と同じ。**フォントが入れば、ここだけ変わる**
    let log = run_tex("全角", "\\dimen0=1zw \\dimen1=1zh \\dimen2=1em \\message{[\\the\\dimen0/\\the\\dimen1/\\the\\dimen2]}");
    assert!(log.contains("[0.0pt/0.0pt/0.0pt]"), "{log}");
}

#[test]
fn 伸縮にも書ける() {
    let log = run_tex("伸縮", "\\skip0=1Q plus 2Q minus 1H \\message{[\\the\\skip0]}");
    assert!(log.contains("0.7113pt plus 1.42262pt minus 0.7113pt"), "{log}");
}

#[test]
fn 知らない単位の案内に出る() {
    let log = run_tex("案内", "\\dimen0=1zz ");
    assert!(log.contains("Q, H"), "{log}");
}

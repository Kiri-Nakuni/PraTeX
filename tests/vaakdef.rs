//! `\vaakdef` の受け入れ試験。
//!
//! **rtex を実際に走らせて確かめる。** `\directvaak` と同じ結果になること、
//! 同じ本体なら `\ifx` が等しいと言うこと、書き戻しが効くこと。

use std::io::Write;
use std::process::Command;

/// rtex を走らせて記録を返す。
fn run_tex(name: &str, body: &str) -> String {
    // **試験は並びで走る。** 名前ごとに場所を分けないと同じ記録を奪い合う
    let dir = std::env::temp_dir().join(format!("vaakdef-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    // INITEX には `{` `}` の分類符号が無い
    write!(f, "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n{body}\n\\end\n").unwrap();
    drop(f);
    let exe = env!("CARGO_BIN_EXE_rtex");
    Command::new(exe).arg("t.tex").current_dir(&dir).output().unwrap();
    std::fs::read_to_string(dir.join("t.log")).unwrap()
}

#[test]
fn 定義して呼ぶと直接書くのと同じ() {
    let log = run_tex(
        "定義して呼ぶと直接書くのと同じ",
        "\\count5=10 \\count6=3\n\
         \\vaakdef\\bump{ count[5] * 2 + count[6] }\n\
         \\count0=\\bump\n\
         \\count1=\\directvaak{ count[5] * 2 + count[6] }\n\
         \\message{[\\the\\count0/\\the\\count1]}",
    );
    assert!(log.contains("[23/23]"), "{log}");
}

#[test]
fn 本体が同じなら等しい() {
    let log = run_tex(
        "本体が同じなら等しい",
        "\\vaakdef\\a{ 1 + 1 }\n\\vaakdef\\b{ 1 + 1 }\n\\vaakdef\\c{ 1 + 2 }\n\
         \\message{[\\ifx\\a\\b Y\\else N\\fi\\ifx\\a\\c Y\\else N\\fi]}",
    );
    assert!(log.contains("[YN]"), "{log}");
}

#[test]
fn レジスタに書き戻る() {
    let log = run_tex(
        "レジスタに書き戻る",
        "\\vaakdef\\wr{ count[7] := 99; }\n\\wr\n\\message{[\\the\\count7]}",
    );
    assert!(log.contains("[99]"), "{log}");
}

#[test]
fn 群を出れば書き込みは戻る() {
    // **保存スタックを通している**ので `\global` でなければ群で戻る
    let log = run_tex(
        "群を出れば書き込みは戻る",
        "\\vaakdef\\wr{ count[7] := 5; }\n\
         {\\wr\\message{[in=\\the\\count7]}}\\message{[out=\\the\\count7]}",
    );
    assert!(log.contains("[in=5]"), "{log}");
    assert!(log.contains("[out=0]"), "{log}");
}

#[test]
fn 中身が空なら零() {
    let log = run_tex(
"中身が空なら零",
"\\vaakdef\\e{}\n\\count0=\\e\n\\message{[\\the\\count0]}");
    assert!(log.contains("[0]"), "{log}");
}

#[test]
fn 誤った本体は定義の時点で言う() {
    let log = run_tex(
"誤った本体は定義の時点で言う",
"\\vaakdef\\bad{ 1 + }\n\\message{[ok]}");
    assert!(log.contains("Vaak interpreter error"), "{log}");
    assert!(log.contains("at definition"), "{log}");
    // **定義に失敗しても続く。** 名前は定義され、呼べば 0 を出す
    assert!(log.contains("[ok]"), "{log}");
}

#[test]
fn 見せ方は本体を見せる() {
    let log = run_tex(
"見せ方は本体を見せる",
"\\vaakdef\\f{ 1 + 1 }\n\\show\\f");
    assert!(log.contains("vaak:-> 1 + 1"), "{log}");
}

#[test]
fn 負の値も展開できる() {
    let log = run_tex(
"負の値も展開できる",
"\\vaakdef\\n{ 0 - 7 }\n\\count0=\\n\n\\message{[\\the\\count0]}");
    assert!(log.contains("[-7]"), "{log}");
}

//! 名前空間（字句層まで）。
//!
//! **catcode 16 の文字が名前空間の印である。** `*foo\hello` は
//! 名前空間 `foo` の `hello`——global の `\hello` とは別物である。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("ns-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(
        f,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n\\catcode`\\*=16\n{body}\n\\end\n"
    )
    .unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex")).arg(&src).current_dir(&dir).output().unwrap();
    std::fs::read_to_string(dir.join("t.log")).unwrap()
}

#[test]
fn 名前空間が違えば別物() {
    let log = run_tex(
        "別物",
        "\\def*foo\\hello{FOO}\\def\\hello{GLOBAL}\\def*bar\\hello{BAR}\n\
         \\message{[*foo\\hello][\\hello][*bar\\hello]}",
    );
    assert!(log.contains("[FOO][GLOBAL][BAR]"), "{log}");
}

#[test]
fn ifxは意味を比べる() {
    // **`\ifx` は意味を比べる。同一性ではない。**
    // 別の制御綴でも、中身が同じなら等しいと言う——TeX82 のとおりである
    let log = run_tex(
        "ifx同じ",
        "\\def*foo\\aa{X}\\def\\aa{X}\n\\message{[\\ifx*foo\\aa\\aa Y\\else N\\fi]}",
    );
    assert!(log.contains("[Y]"), "{log}");

    // 中身が違えば違う——**別の入れ物である**ことはこちらで分かる
    let log = run_tex(
        "ifx違う",
        "\\def*foo\\aa{X}\\def\\aa{Y}\n\\message{[\\ifx*foo\\aa\\aa Y\\else N\\fi]}",
    );
    assert!(log.contains("[N]"), "{log}");
}

#[test]
fn 同じ名前空間なら同じもの() {
    let log = run_tex(
        "同じ",
        "\\def*foo\\a{X}\\let*foo\\b=*foo\\a\n\\message{[*foo\\b][\\ifx*foo\\a*foo\\b Y\\else N\\fi]}",
    );
    assert!(log.contains("[X][Y]"), "{log}");
}

#[test]
fn 一文字の制御綴も名前空間に入る() {
    let log = run_tex(
        "一文字",
        "\\def*foo\\!{NS}\\def\\!{GLOBAL}\n\\message{[*foo\\!][\\!]}",
    );
    assert!(log.contains("[NS][GLOBAL]"), "{log}");
}

#[test]
fn 群を出れば戻る() {
    // **同じ番号空間に載せた効き目。** save stack がそのまま働く
    let log = run_tex(
        "群",
        "\\def*foo\\a{OUT}\n{\\def*foo\\a{IN}\\message{[in=*foo\\a]}}\\message{[out=*foo\\a]}",
    );
    assert!(log.contains("[in=IN]"), "{log}");
    assert!(log.contains("[out=OUT]"), "{log}");
}

#[test]
fn globalは群を越える() {
    let log = run_tex(
        "global",
        "\\def*foo\\a{OUT}\n{\\global\\def*foo\\a{IN}}\\message{[out=*foo\\a]}",
    );
    assert!(log.contains("[out=IN]"), "{log}");
}

#[test]
fn 名前空間の名前に印を含められる() {
    // **階層ではない。** `*a*b\hoge` は `a*b` の `hoge`
    let log = run_tex(
        "入れ子でない",
        "\\def*a*b\\h{AB}\\def*a\\h{A}\n\\message{[*a*b\\h][*a\\h]}",
    );
    assert!(log.contains("[AB][A]"), "{log}");
}

#[test]
fn 名前が閉じなければ暴走を報せる() {
    let log = run_tex("runaway", "\\def*foo bar{X}\n\\message{[done]}");
    assert!(log.contains("Runaway namespace name"), "{log}");
    // **読み飛ばして続く**
    assert!(log.contains("[done]"), "{log}");
}

#[test]
fn 印を置かなければ何も変わらない() {
    // catcode 16 の文字が無ければ、字句化は TeX82 のままである
    let dir = std::env::temp_dir().join(format!("ns-plain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(
        f,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n\\def\\a{{X}}\n\\message{{[\\a][*]}}\n\\end\n"
    )
    .unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex")).arg(&src).current_dir(&dir).output().unwrap();
    let log = std::fs::read_to_string(dir.join("t.log")).unwrap();
    assert!(log.contains("[X][*]"), "{log}");
}

// ===== Phase 3：`\namespace` と `\csname` =====

#[test]
fn csname経由でも同じ制御綴になる() {
    let log = run_tex(
        "csname",
        "\\def*foo\\bar{FOOBAR}\n\
         \\message{[\\namespace foo\\csname bar\\endcsname]}",
    );
    assert!(log.contains("[FOOBAR]"), "{log}");
}

#[test]
fn グローバルに作られない() {
    // **`\endcsname` を終端にする案を退けた理由そのもの。**
    // 登録は `\endcsname` に達した一箇所で起きるので、そこへ名前空間を渡すしかない
    let log = run_tex(
        "作られない",
        "\\def*foo\\bar{X}\n\
         \\message{[\\namespace foo\\csname bar\\endcsname]}\n\
         \\message{[global=\\ifx\\bar\\undefined Y\\else N\\fi]}",
    );
    assert!(log.contains("[global=Y]"), "{log}");
}

#[test]
fn csnameで作ってdefできる() {
    let log = run_tex(
        "作る",
        "\\expandafter\\def\\namespace zoo\\csname qux\\endcsname{ZOOQUX}\n\
         \\message{[*zoo\\qux]}",
    );
    assert!(log.contains("[ZOOQUX]"), "{log}");
}

#[test]
fn 名前空間名は展開される() {
    let log = run_tex(
        "展開",
        "\\def*foo\\bar{FOOBAR}\\def\\ns{foo}\n\
         \\message{[\\namespace \\ns\\csname bar\\endcsname]}",
    );
    assert!(log.contains("[FOOBAR]"), "{log}");
}

#[test]
fn 空の名前空間名はグローバルそのもの() {
    let log = run_tex(
        "空",
        "\\def\\bar{GLOBAL}\n\\message{[\\namespace\\csname bar\\endcsname]}",
    );
    assert!(log.contains("[GLOBAL]"), "{log}");
}

#[test]
fn 入れ子は誤り() {
    let log = run_tex(
        "入れ子",
        "\\message{[\\namespace foo\\namespace bar\\csname x\\endcsname]}\\message{[done]}",
    );
    assert!(log.contains("Nested"), "{log}");
    assert!(log.contains("[done]"), "{log}");
}

#[test]
fn csname以外が来れば誤り() {
    let log = run_tex("csname無し", "\\namespace foo\\relax \\message{[done]}");
    assert!(log.contains("Missing"), "{log}");
    assert!(log.contains("[done]"), "{log}");
}

//! e-TeX の糊成分問い合わせ。
//!
//! 仕様は公式 e-TeX manual 3.5, 5.1 の公開記述だけから確かめる。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    let dir =
        std::env::temp_dir().join(format!("etex-glue-{}-{:x}", std::process::id(), h.finish()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut file = std::fs::File::create(&src).unwrap();
    write!(
        file,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n{body}\n\\end\n"
    )
    .unwrap();
    drop(file);

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "TeXを実行できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    join_log(&dir.join("t.log"))
}

/// 記録の79桁折り返しを除いてから照合する。
fn join_log(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap().replace('\n', "")
}

#[test]
fn 伸び縮みの値と次数を別々に答える() {
    let log = run_tex(
        "値と次数",
        "\\skip0=3pt plus 4fil minus 5fill\n\
         \\message{[\\the\\gluestretch\\skip0/\\the\\glueshrink\\skip0/\
         \\the\\gluestretchorder\\skip0/\\the\\glueshrinkorder\\skip0]}",
    );
    assert!(log.contains("[4.0pt/5.0pt/1/2]"), "{log}");
}

#[test]
fn 四種類の次数を明示対応で返す() {
    let log = run_tex(
        "全次数",
        "\\skip0=0pt plus 1pt minus 1fil\n\
         \\skip1=0pt plus 1fill minus 1filll\n\
         \\message{[\\the\\gluestretchorder\\skip0/\\the\\glueshrinkorder\\skip0/\
         \\the\\gluestretchorder\\skip1/\\the\\glueshrinkorder\\skip1]}",
    );
    assert!(log.contains("[0/1/2/3]"), "{log}");
}

#[test]
fn 零の係数でも指定された無限次数を保つ() {
    let log = run_tex(
        "零の無限次数",
        "\\skip0=0pt plus 0fil minus 0filll
         \\message{[\\the\\gluestretch\\skip0/\\the\\glueshrink\\skip0/\
         \\the\\gluestretchorder\\skip0/\\the\\glueshrinkorder\\skip0]}",
    );
    assert!(log.contains("[0.0pt/0.0pt/1/3]"), "{log}");
}

#[test]
fn 負の糊成分も符号と次数を保つ() {
    let log = run_tex(
        "負の成分",
        "\\skip0=0pt plus -1.5filll minus -2fil\n\
         \\message{[\\the\\gluestretch\\skip0/\\the\\glueshrink\\skip0/\
         \\the\\gluestretchorder\\skip0/\\the\\glueshrinkorder\\skip0]}",
    );
    assert!(log.contains("[-1.5pt/-2.0pt/3/1]"), "{log}");
}

#[test]
fn 糊成分は内部整数と内部寸法として式に入る() {
    let log = run_tex(
        "内部量",
        "\\skip0=1pt plus 4fil minus 5fill\n\
         \\dimen0=\\gluestretch\\skip0\n\
         \\count0=\\glueshrinkorder\\skip0\n\
         \\dimen1=\\dimexpr\\glueshrink\\skip0*2\\relax\n\
         \\message{[\\the\\dimen0/\\the\\count0/\\the\\dimen1/\
         \\ifnum\\gluestretchorder\\skip0=1 Y\\else N\\fi]}",
    );
    assert!(log.contains("[4.0pt/2/10.0pt/Y]"), "{log}");
}

#[test]
fn glueexprを引数にできる() {
    let log = run_tex(
        "glueexpr引数",
        "\\message{[\\the\\gluestretch\\glueexpr
         1pt plus 2fil + 3pt plus 4fill\\relax/\
         \\the\\gluestretchorder\\glueexpr
         1pt plus 2fil + 3pt plus 4fill\\relax]}",
    );
    assert!(log.contains("[4.0pt/2]"), "{log}");
}

#[test]
fn 命令自身は展開せずtheだけが明示糊の係数を展開する() {
    let log = run_tex(
        "明示糊と展開性",
        "\\edef\\raw{A\\gluestretch 0pt plus 2fill B}
         \\edef\\value{A\\the\\gluestretch 0pt plus 2fill B}
         \\message{[raw=\\meaning\\raw][value=\\meaning\\value]}",
    );
    assert!(
        log.contains("[raw=macro:->A\\gluestretch 0pt plus 2fill B]"),
        "命令自身が残らなかった: {log}"
    );
    assert!(
        log.contains("[value=macro:->A2.0ptB]"),
        "\\theが明示糊の係数を展開しなかった: {log}"
    );
}

#[test]
fn muglueは単位不一致を報せてから係数と次数を回復する() {
    let log = run_tex(
        "muglueの回復",
        "\\muskip0=1mu plus 2.5fill minus -3mu
         \\message{[\\the\\gluestretch\\muskip0/\\the\\glueshrink\\muskip0/\
         \\the\\gluestretchorder\\muskip0/\\the\\glueshrinkorder\\muskip0]}",
    );
    assert!(log.contains("Incompatible glue units"), "{log}");
    assert!(log.contains("[2.5pt/-3.0pt/2/0]"), "{log}");
}

#[test]
fn meaningは糊の引数を読まない() {
    let log = run_tex(
        "meaning",
        "\\message{[\\meaning\\gluestretch/A][\\meaning\\glueshrink/B]\
         [\\meaning\\gluestretchorder/C][\\meaning\\glueshrinkorder/D]}",
    );
    assert!(
        log.contains(
            "[\\gluestretch/A][\\glueshrink/B][\\gluestretchorder/C][\\glueshrinkorder/D]"
        ),
        "{log}"
    );
}

#[test]
fn 糊成分問い合わせは書き換えを拒む() {
    let log = run_tex("書換え拒否", "\\advance\\gluestretch\\end");
    assert!(
        log.contains("You can't use `\\gluestretch' after \\advance"),
        "{log}"
    );
}

#[test]
fn 糊成分命令はfmtを往復する() {
    let dir = std::env::temp_dir().join(format!("etex-glue-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fmt = dir.join("mk.fmt");
    let _ = std::fs::remove_file(&fmt);
    let use_log = dir.join("use.log");
    let _ = std::fs::remove_file(&use_log);

    std::fs::write(
        dir.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\skip32767=1pt plus 2fil minus 3filll\n\
         \\let\\savedstretch=\\gluestretch\n\
         \\let\\savedshrink=\\glueshrink\n\
         \\let\\savedstretchorder=\\gluestretchorder\n\
         \\let\\savedshrinkorder=\\glueshrinkorder\n\
         \\dump\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("mk.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        output.status.success() && fmt.exists(),
        "fmtを生成できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::write(
        dir.join("use.tex"),
        "\\message{[\\the\\savedstretch\\skip32767/\\the\\savedshrink\\skip32767/\
         \\the\\savedstretchorder\\skip32767/\\the\\savedshrinkorder\\skip32767/\
         \\meaning\\savedstretch/\\meaning\\savedshrink/\
         \\meaning\\savedstretchorder/\\meaning\\savedshrinkorder]}\n\
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
            "[2.0pt/3.0pt/1/3/\\gluestretch/\\glueshrink/\\gluestretchorder/\\glueshrinkorder]"
        ),
        "{log}"
    );
}

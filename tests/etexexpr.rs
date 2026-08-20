//! e-TeX の式（`\numexpr` `\dimexpr` `\glueexpr` `\muexpr`）。
//!
//! **`\multiply` と `\divide` を並べるのとの違いは、中間結果である。**
//! `\numexpr 7*8/3\relax` は 56/3 を丸めて 19。**23 ではない。**

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("etexexpr-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(f, "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n{body}\n\\end\n").unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex")).arg(&src).current_dir(&dir).output().unwrap();
    std::fs::read_to_string(dir.join("t.log")).unwrap()
}

fn n(name: &str, expr: &str) -> String {
    let log = run_tex(name, &format!("\\count0=\\numexpr {expr}\\relax \\message{{[\\the\\count0]}}"));
    let i = log.find('[').unwrap_or(0);
    let j = log[i..].find(']').map(|j| i + j + 1).unwrap_or(log.len());
    log[i..j].to_string()
}

#[test]
fn 足し引き() {
    assert_eq!(n("足", "1+2"), "[3]");
    assert_eq!(n("引", "10-3-2"), "[5]");
}

#[test]
fn 掛け割りが先() {
    assert_eq!(n("優先", "2+3*4"), "[14]");
    assert_eq!(n("優先2", "2*3+4"), "[10]");
}

#[test]
fn 中間結果が三十二ビットに落ちない() {
    // **これが `\numexpr` の存在理由である。**
    // `\multiply\divide` を並べると 7*8=56 → 56/3=18（切り捨て）だが、
    // 式なら 56/3 を**四捨五入して 19**
    assert_eq!(n("中間", "7*8/3"), "[19]");
    // 32 ビットを溢れる中間結果
    assert_eq!(n("溢れ", "2000000000*3/6"), "[1000000000]");
}

#[test]
fn 丸めは四捨五入() {
    assert_eq!(n("丸め上", "5/2"), "[3]");
    assert_eq!(n("丸め下", "4/3"), "[1]");
    // **半分は絶対値の大きい方へ**
    assert_eq!(n("負の丸め", "-5/2"), "[-3]");
}

#[test]
fn 括弧() {
    assert_eq!(n("括弧", "(2+3)*4"), "[20]");
    assert_eq!(n("入れ子", "((1+2)*(3+4))"), "[21]");
}

#[test]
fn 内部量を混ぜられる() {
    let log = run_tex(
        "内部",
        "\\count1=7 \\count0=\\numexpr \\count1*6\\relax \\message{[\\the\\count0]}",
    );
    assert!(log.contains("[42]"), "{log}");
}

#[test]
fn 寸法の式() {
    let log = run_tex(
        "寸法",
        "\\dimen0=\\dimexpr 1pt+2pt\\relax \\dimen1=\\dimexpr 10pt/4\\relax \
         \\message{[\\the\\dimen0/\\the\\dimen1]}",
    );
    assert!(log.contains("[3.0pt/2.5pt]"), "{log}");
}

#[test]
fn 寸法にも和文の単位が使える() {
    let log = run_tex("和文", "\\dimen0=\\dimexpr 4Q*2\\relax \\message{[\\the\\dimen0]}");
    assert!(log.contains("[5.69052pt]"), "{log}");
}

#[test]
fn 糊の式は伸縮も足す() {
    let log = run_tex(
        "糊",
        "\\skip0=\\glueexpr 1pt plus 2pt + 3pt plus 4pt\\relax \\message{[\\the\\skip0]}",
    );
    assert!(log.contains("[4.0pt plus 6.0pt]"), "{log}");
}

#[test]
fn 大きい次数が勝つ() {
    // **TeX の糊の規則そのもの**
    let log = run_tex(
        "次数",
        "\\skip0=\\glueexpr 1pt plus 2fil + 1pt plus 5pt\\relax \\message{[\\the\\skip0]}",
    );
    assert!(log.contains("plus 2.0fil"), "{log}");
}

#[test]
fn relax無しでも終われる() {
    let log = run_tex("relax無し", "\\count0=\\numexpr 6*7 \\message{[\\the\\count0]}");
    assert!(log.contains("[42]"), "{log}");
}

#[test]
fn 式の中に式を書ける() {
    assert_eq!(n("入れ子式", "\\numexpr 2+3\\relax*4"), "[20]");
}

//! e-TeX の拡張。
//!
//! **LaTeX2e は e-TeX を要求する。** ここはその第一歩である。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    let dir = std::env::temp_dir().join(format!("etex-{}-{:x}", std::process::id(), h.finish()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(f, "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n{body}\n\\end\n").unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex")).arg(&src).current_dir(&dir).output().unwrap();
    std::fs::read_to_string(dir.join("t.log")).unwrap()
}

#[test]
fn 版を答える() {
    let log = run_tex("版", "\\message{[\\the\\eTeXversion]}");
    assert!(log.contains("[2]"), "{log}");
}

#[test]
fn protectedはedefで展開されない() {
    // **これが `\protected` の全部である**
    let log = run_tex(
        "protected",
        "\\def\\plain{PLAIN}\\protected\\def\\prot{PROT}\n\
         \\edef\\a{\\plain}\\edef\\b{\\prot}\n\
         \\message{[\\meaning\\a][\\meaning\\b]}",
    );
    assert!(log.contains("[macro:->PLAIN][macro:->\\prot ]"), "{log}");
}

#[test]
fn protectedでも普通に使えば展開される() {
    // **`\message` は展開する文脈である**ので、そこでは展開されない。
    // 普通に呼べば展開される
    let log = run_tex(
        "protected使用",
        "\\protected\\def\\p{\\message{[OK]}}\n\\p",
    );
    assert!(log.contains("[OK]"), "{log}");
}

#[test]
fn protectedはmessageでも展開されない() {
    // **本物の e-TeX と一致する**（`A\p A` と出る）
    let log = run_tex("protectedとmessage", "\\protected\\def\\p{P}\\message{A\\p A}");
    assert!(log.contains("A\\p A"), "{log}");
}

#[test]
fn protectedは見せ方に出る() {
    let log = run_tex("protected表示", "\\protected\\def\\p{X}\\message{[\\meaning\\p]}");
    assert!(log.contains("[\\protected macro:->X]"), "{log}");
}

#[test]
fn ifdefinedは作らない() {
    // **`\csname` と違って表に穴を開けない**
    let log = run_tex(
        "ifdefined",
        "\\message{[\\ifdefined\\nosuch D\\else U\\fi]}\
         \\message{[\\ifdefined\\message D\\else U\\fi]}",
    );
    // `\message` は続けて出すと空白を挟む
    assert!(log.contains("[U] [D]"), "{log}");
}

#[test]
fn ifcsnameも作らない() {
    let log = run_tex(
        "ifcsname",
        "\\def\\yes{1}\n\
         \\message{[\\ifcsname yes\\endcsname Y\\else N\\fi]}\
         \\message{[\\ifcsname nope\\endcsname Y\\else N\\fi]}\n\
         \\message{[after ifcsname=\\ifdefined\\nope D\\else U\\fi]}\n\
         \\expandafter\\relax\\csname other\\endcsname\n\
         \\message{[after csname=\\ifdefined\\other D\\else U\\fi]}",
    );
    assert!(log.contains("[Y] [N]"), "{log}");
    // **`\ifcsname` は作らない。`\csname` は `\relax` にして作る**
    assert!(log.contains("[after ifcsname=U]"), "{log}");
    assert!(log.contains("[after csname=D]"), "{log}");
}

#[test]
fn unlessが反転する() {
    let log = run_tex(
        "unless",
        "\\message{[\\unless\\iftrue T\\else F\\fi][\\unless\\iffalse T\\else F\\fi]}",
    );
    assert!(log.contains("[F][T]"), "{log}");
}

#[test]
fn 群の深さと種類を答える() {
    let log = run_tex(
        "群",
        "\\message{[\\the\\currentgrouplevel/\\the\\currentgrouptype]}\
         {\\message{[\\the\\currentgrouplevel/\\the\\currentgrouptype]}}",
    );
    assert!(log.contains("[0/0]"), "{log}");
    assert!(log.contains("[1/1]"), "{log}");
}

#[test]
fn 条件の深さと種類を答える() {
    let log = run_tex(
        "条件",
        "\\message{[\\the\\currentiflevel]}\
         \\iftrue\\message{[\\the\\currentiflevel/\\the\\currentiftype/\\the\\currentifbranch]}\\fi",
    );
    assert!(log.contains("[0]"), "{log}");
    // `\iftrue` は 15 番目（0 起点で 14）なので 15
    assert!(log.contains("[1/15/1]"), "{log}");
}

#[test]
fn 追跡の整数を持つ() {
    let log = run_tex(
        "追跡",
        "\\tracingassigns=1 \\tracinggroups=2 \\tracingifs=3 \
         \\message{[\\the\\tracingassigns\\the\\tracinggroups\\the\\tracingifs]}",
    );
    assert!(log.contains("[123]"), "{log}");
}

#[test]
fn numexprは式である() {
    let log = run_tex("numexpr", "\\count0=\\numexpr 7*8/3\\relax \\message{[\\the\\count0]}");
    assert!(log.contains("[19]"), "{log}");
}

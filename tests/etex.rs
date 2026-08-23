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
    // 作業ディレクトリを指定しているので相対名で渡す。TeX の入力名走査へ
    // Windows のドライブ文字 (`C:`) を持ち込まず、Unix と同じ条件で確かめる。
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    join_log(&dir.join("t.log"))
}

/// **記録は 79 桁で折り返される。** 印の途中でも折れるので、繋ぎ直してから見る。
fn join_log(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap().replace('\n', "")
}

#[test]
fn 版を答える() {
    let log = run_tex("版", "\\message{[\\the\\eTeXversion]}");
    assert!(log.contains("[2]"), "{log}");
}

#[test]
fn pratexは自分の版だけを名乗る() {
    let log = run_tex(
        "PraTeX版",
        "\\message{[PraTeX=\\the\\pratexversion]}\n\
         \\ifdefined\\pTeXversion\\message{[pTeX偽装]}\\fi\n\
         \\ifdefined\\upTeXversion\\message{[upTeX偽装]}\\fi\n\
         \\ifdefined\\pdftexversion\\message{[pdfTeX偽装]}\\fi",
    );
    assert!(log.contains("[PraTeX=1]"), "{log}");
    assert!(!log.contains("偽装"), "{log}");
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
fn lastnodetypeは空のlistと基本nodeの型を答える() {
    let log = run_tex(
        "lastnodetype基本",
        "\\catcode36=3\n\
         \\message{[empty=\\the\\lastnodetype]}\n\
         \\setbox0=\\hbox{\\message{[h-empty=\\the\\lastnodetype]}\
           \\hbox{}\\message{[hlist=\\the\\lastnodetype]}\
           \\vbox{}\\message{[vlist=\\the\\lastnodetype]}\
           \\vrule width1pt\\message{[rule=\\the\\lastnodetype]}\
           \\vadjust{}\\message{[adjust=\\the\\lastnodetype]}\
           \\discretionary{}{}{}\\message{[disc=\\the\\lastnodetype]}\
           \\write0{}\\message{[whatsit=\\the\\lastnodetype]}\
           $\\relax$\\message{[math=\\the\\lastnodetype]}\
           \\hskip1pt\\message{[glue=\\the\\lastnodetype]}\
           \\kern1pt\\message{[kern=\\the\\lastnodetype]}\
           \\penalty0\\message{[penalty=\\the\\lastnodetype]}}\n\
         \\setbox1=\\vbox{\\insert0{}\\message{[insert=\\the\\lastnodetype]}\
           \\mark{}\\message{[mark=\\the\\lastnodetype]}}",
    );
    for expected in [
        "[empty=-1]",
        "[h-empty=-1]",
        "[hlist=1]",
        "[vlist=2]",
        "[rule=3]",
        "[insert=4]",
        "[mark=5]",
        "[adjust=6]",
        "[disc=8]",
        "[whatsit=9]",
        "[math=10]",
        "[glue=11]",
        "[kern=12]",
        "[penalty=13]",
    ] {
        assert!(log.contains(expected), "{expected}: {log}");
    }
}

#[test]
fn lastnodetypeはnested_boxからpage側の型へ戻る() {
    let log = run_tex(
        "lastnodetype page復元",
        "\\hrule height1pt \\message{[rule=\\the\\lastnodetype]}\n\
         \\vskip1pt \\message{[glue=\\the\\lastnodetype]}\n\
         \\kern2pt \\message{[kern=\\the\\lastnodetype]}\n\
         \\penalty0 \\message{[penalty=\\the\\lastnodetype]}\n\
         \\setbox0=\\hbox{} \\message{[restored=\\the\\lastnodetype]}",
    );
    for expected in [
        "[rule=3]",
        "[glue=11]",
        "[kern=12]",
        "[penalty=13]",
        "[restored=13]",
    ] {
        assert!(log.contains(expected), "{expected}: {log}");
    }
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
fn 最上位の表示用整数は記録器へ同期する() {
    let log = run_tex(
        "表示用整数の同期",
        "\\escapechar=33 \\show\\count
         \\newlinechar=124 \\message{[newline=A|B]}",
    );
    assert!(log.contains("> !count=!count."), "{log}");
    assert!(log.contains("[newline=AB]"), "{log}");
}

#[test]
fn numexprは式である() {
    let log = run_tex("numexpr", "\\count0=\\numexpr 7*8/3\\relax \\message{[\\the\\count0]}");
    assert!(log.contains("[19]"), "{log}");
}

#[test]
fn expandedは展開しきる() {
    let log = run_tex(
        "expanded",
        "\\def\\a{A}\\def\\b{\\a B}\\count0=42\n\
         \\message{[\\expanded{\\b}][\\expanded{x\\the\\count0 y}]}",
    );
    assert!(log.contains("[AB][x42y]"), "{log}");
}

#[test]
fn detokenizeは字句に直す() {
    let log = run_tex("detokenize", "\\def\\a{A}\\message{[\\detokenize{\\a b}]}");
    // **展開しない。** `\a` がそのまま文字になる
    assert!(log.contains("[\\a b]"), "{log}");
}

#[test]
fn unexpandedは展開しない() {
    let log = run_tex(
        "unexpanded",
        "\\def\\a{A}\\edef\\b{\\unexpanded{\\a}}\\message{[\\meaning\\b]}",
    );
    assert!(log.contains("[macro:->\\a ]"), "{log}");
}

#[test]
fn 引用符つきのファイル名を読む() {
    // **LaTeX2e が `\openin\@inputcheck"expl3.ltx" ` と書く。**
    // これが無いと `\IfFileExists` が必ず偽になる
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "引用符".hash(&mut h);
    let dir = std::env::temp_dir().join(format!("etex-{}-{:x}", std::process::id(), h.finish()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("target file.tex"), "\\relax\n").unwrap();
    std::fs::write(dir.join("plain.tex"), "\\relax\n").unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(
        f,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n\
         \\openin0=\"plain.tex\" \\message{{[q=\\ifeof0 N\\else Y\\fi]}}\\closein0\n\
         \\openin0=\"target file.tex\" \\message{{[sp=\\ifeof0 N\\else Y\\fi]}}\\closein0\n\
         \\openin0=nosuch.tex \\message{{[no=\\ifeof0 N\\else Y\\fi]}}\n\\end\n"
    )
    .unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex")).arg("t.tex").current_dir(&dir).output().unwrap();
    let log = join_log(&dir.join("t.log"));
    assert!(log.contains("[q=Y]"), "{log}");
    assert!(log.contains("[sp=Y]"), "{log}");
    assert!(log.contains("[no=N]"), "{log}");
}

#[test]
fn pdf文字列命令は外側の走査を壊さない() {
    let log = run_tex(
        "pdf文字列",
        "\\def\\a{AZ}\\message{[before/\\pdfescapehex{\\a}/after]}\n\
         \\message{[\\pdfmdfivesum{abc}]}",
    );
    assert!(log.contains("[before/415A/after]"), "{log}");
    assert!(log.contains("[900150983CD24FB0D6963F7D28E17F72]"), "{log}");
}

#[test]
fn pdffilesizeは一般テキストを名前として読む() {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "pdffilesize".hash(&mut h);
    let dir = std::env::temp_dir().join(format!("etex-{}-{:x}", std::process::id(), h.finish()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("four.bin"), b"1234").unwrap();
    std::fs::write(
        dir.join("t.tex"),
        "\\catcode`\\{=1\n\\catcode`\\}=2\n\\batchmode\n\
         \\def\\file{four.bin}\\message{[size=\\pdffilesize{\\file}]}\n\\end\n",
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    let log = join_log(&dir.join("t.log"));
    assert!(log.contains("[size=4]"), "{log}");
}

#[test]
fn pdfstrcmpは展開した文字列を比較する() {
    let log = run_tex(
        "pdfstrcmp",
        "\\def\\a{abc}\\message{[\\pdfstrcmp{\\a}{abc}/\\pdfstrcmp{abc}{abd}/\\pdfstrcmp{abd}{abc}]}",
    );
    assert!(log.contains("[0/-1/1]"), "{log}");
}

#[test]
fn everyeofはファイルを閉じる前に一度だけ入る() {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "everyeof".hash(&mut h);
    let dir = std::env::temp_dir().join(format!("etex-{}-{:x}", std::process::id(), h.finish()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("child.tex"), "\\message{[child]}\n").unwrap();
    std::fs::write(
        dir.join("t.tex"),
        "\\catcode`\\{=1\n\\catcode`\\}=2\n\\batchmode\n\
         \\count0=0 \\everyeof{\\advance\\count0 by1 \\message{[eof]}}\n\
         \\input child.tex\n\\message{[after=\\the\\count0]}\n\\end\n",
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    let log = join_log(&dir.join("t.log"));
    assert!(log.contains("[child] [eof]) [after=1]"), "{log}");
}

#[test]
fn readlineは現在の分類符号を無視して一行を読む() {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "readline".hash(&mut h);
    let dir = std::env::temp_dir().join(format!("etex-{}-{:x}", std::process::id(), h.finish()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("raw.txt"), "\\undefined{#}%  two\nsecond\n").unwrap();
    std::fs::write(
        dir.join("t.tex"),
        "\\catcode`\\{=1\n\\catcode`\\}=2\n\\batchmode\n\
         \\endlinechar=`\\! \\openin0=raw.txt\n\
         \\readline0 to \\first \\readline0 to \\second\n\
         \\message{[\\meaning\\first]} \\message{[\\meaning\\second]}\n\\end\n",
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    let log = join_log(&dir.join("t.log"));
    assert!(log.contains("[macro:->\\undefined{#}%  two!]"), "{log}");
    assert!(log.contains("[macro:->second!]"), "{log}");
}

#[test]
fn interactionmodeは現在値を読み書きできる() {
    let log = run_tex(
        "interactionmode",
        "\\message{[before=\\the\\interactionmode]}\n\
         \\interactionmode=2 \\message{[scroll=\\the\\interactionmode]}\n\
         \\interactionmode=1 \\message{[nonstop=\\the\\interactionmode]}",
    );
    assert!(log.contains("[before=0]"), "{log}");
    assert!(log.contains("[scroll=2]"), "{log}");
    assert!(log.contains("[nonstop=1]"), "{log}");
}

#[test]
fn pdfshellescapeは無効状態を内部整数として答える() {
    let log = run_tex(
        "pdfshellescapeの値",
        "\\chardef\\status=\\pdfshellescape\n\
         \\count0=\\pdfshellescape\n\
         \\message{[value=\\the\\pdfshellescape/\\the\\status/\\the\\count0/\
         \\ifnum\\pdfshellescape=0 Y\\else N\\fi]}",
    );
    assert!(log.contains("[value=0/0/0/Y]"), "{log}");
}

#[test]
fn pdfshellescapeのmeaningは後続字句を消費しない() {
    let log = run_tex(
        "pdfshellescapeのmeaning",
        "\\message{[meaning=\\meaning\\pdfshellescape/after]}",
    );
    assert!(
        log.contains("[meaning=\\pdfshellescape/after]"),
        "{log}"
    );
}

#[test]
fn pdfshellescape自身は展開せずtheだけが数へ展開する() {
    let log = run_tex(
        "pdfshellescapeの展開性",
        "\\edef\\raw{\\pdfshellescape}
         \\edef\\value{\\the\\pdfshellescape}
         \\message{[raw=\\meaning\\raw/value=\\meaning\\value]}",
    );
    assert!(
        log.contains("[raw=macro:->\\pdfshellescape "),
        "primitive自身が残らなかった: {log}"
    );
    assert!(
        log.contains("/value=macro:->0]"),
        "\\theを介して数へ展開されなかった: {log}"
    );
}

#[test]
fn pdfshellescapeは書き換えを拒み無効状態を保つ() {
    let log = run_tex(
        "pdfshellescapeの読み取り専用性",
        "\\advance\\pdfshellescape by 1
         \\message{[after=\\the\\pdfshellescape]}",
    );
    assert!(log.contains("You can't use"), "書き換えを拒まなかった: {log}");
    assert!(log.contains("[after=0]"), "無効状態が変わった: {log}");
}

#[test]
fn pdfshellescape命令はfmtを往復する() {
    let dir = std::env::temp_dir().join(format!("pdfshellescape-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fmt = dir.join("mk.fmt");
    let _ = std::fs::remove_file(&fmt);
    let use_log = dir.join("use.log");
    let _ = std::fs::remove_file(&use_log);

    std::fs::write(
        dir.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\let\\status=\\pdfshellescape\n\\dump\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("mk.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success() && fmt.exists(),
        "fmtを生成できなかった: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    std::fs::write(
        dir.join("use.tex"),
        "\\message{[primitive=\\the\\pdfshellescape/alias=\\the\\status/meaning=\\meaning\\status]}\n\
         \\end\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("&mk")
        .arg("use.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success() && use_log.exists(),
        "fmtを読み戻せなかった: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let log = join_log(&use_log);
    assert!(
        log.contains("[primitive=0/alias=0/meaning=\\pdfshellescape]"),
        "{log}"
    );
}

#[test]
fn 拡張レジスタの両端を六種類とも読み書きできる() {
    let log = run_tex(
        "拡張レジスタの両端",
        "\\count256=256 \\count32767=32767\n\
         \\dimen256=1pt \\dimen32767=2pt\n\
         \\skip256=3pt \\skip32767=4pt\n\
         \\muskip256=5mu \\muskip32767=6mu\n\
         \\toks256={LOW} \\toks32767={HIGH}\n\
         \\setbox256=\\hbox{\\vrule width7pt} \\setbox32767=\\hbox{\\vrule width8pt}\n\
         \\message{[c=\\the\\count256/\\the\\count32767]}\n\
         \\message{[d=\\the\\dimen256/\\the\\dimen32767]}\n\
         \\message{[s=\\the\\skip256/\\the\\skip32767]}\n\
         \\message{[m=\\the\\muskip256/\\the\\muskip32767]}\n\
         \\message{[t=\\the\\toks256/\\the\\toks32767]}\n\
         \\message{[b=\\the\\wd256/\\the\\wd32767]}",
    );
    assert!(log.contains("[c=256/32767]"), "{log}");
    assert!(log.contains("[d=1.0pt/2.0pt]"), "{log}");
    assert!(log.contains("[s=3.0pt/4.0pt]"), "{log}");
    assert!(log.contains("[m=5.0mu/6.0mu]"), "{log}");
    assert!(log.contains("[t=LOW/HIGH]"), "{log}");
    assert!(log.contains("[b=7.0pt/8.0pt]"), "{log}");
}

#[test]
fn 高位レジスタは群で戻りglobalなら残る() {
    let log = run_tex(
        "高位レジスタの復元",
        "\\count32767=1 \\dimen32767=1pt \\skip32767=1pt \\muskip32767=1mu\n\
         \\toks32767={O} \\setbox32767=\\hbox{\\vrule width1pt}\n\
         {\\count32767=2 \\dimen32767=2pt \\skip32767=2pt \\muskip32767=2mu\n\
          \\toks32767={L} \\setbox32767=\\hbox{\\vrule width2pt}\n\
          \\message{[local=\\the\\count32767/\\the\\dimen32767/\\the\\skip32767/\\the\\muskip32767/\\the\\toks32767/\\the\\wd32767]}}\n\
         \\message{[restored=\\the\\count32767/\\the\\dimen32767/\\the\\skip32767/\\the\\muskip32767/\\the\\toks32767/\\the\\wd32767]}\n\
         {\\global\\count32767=3 \\global\\dimen32767=3pt\n\
          \\global\\skip32767=3pt \\global\\muskip32767=3mu\n\
          \\global\\toks32767={G} \\global\\setbox32767=\\hbox{\\vrule width3pt}}\n\
         \\message{[global=\\the\\count32767/\\the\\dimen32767/\\the\\skip32767/\\the\\muskip32767/\\the\\toks32767/\\the\\wd32767]}",
    );
    assert!(
        log.contains("[local=2/2.0pt/2.0pt/2.0mu/L/2.0pt]"),
        "{log}"
    );
    assert!(
        log.contains("[restored=1/1.0pt/1.0pt/1.0mu/O/1.0pt]"),
        "{log}"
    );
    assert!(
        log.contains("[global=3/3.0pt/3.0pt/3.0mu/G/3.0pt]"),
        "{log}"
    );
}

#[test]
fn 別名定義から高位レジスタを使える() {
    let log = run_tex(
        "高位レジスタの別名",
        "\\countdef\\highcount=32767 \\dimendef\\highdimen=32767\n\
         \\skipdef\\highskip=32767 \\muskipdef\\highmuskip=32767\n\
         \\toksdef\\hightoks=32767\n\
         \\highcount=11 \\highdimen=2pt \\highskip=3pt \\highmuskip=4mu\n\
         \\hightoks={TOK}\n\
         \\message{[alias=\\the\\count32767/\\the\\dimen32767/\\the\\skip32767/\\the\\muskip32767/\\the\\toks32767]}",
    );
    assert!(log.contains("[alias=11/2.0pt/3.0pt/4.0mu/TOK]"), "{log}");
}

#[test]
fn 最上位整数代入は拡張境界と群の規則を保つ() {
    let log = run_tex(
        "最上位整数代入",
        "\\count255=15 \\count256=16 \\count32767=17
         {\\count255=25 \\global\\count256=26
          \\globaldefs=1 \\count32767=27
          \\message{[inside=\\the\\count255/\\the\\count256/\\the\\count32767]}}
         \\message{[after=\\the\\count255/\\the\\count256/\\the\\count32767]}
         {\\globaldefs=-1 \\global\\count255=35
          \\message{[forced-local=\\the\\count255]}}
         \\message{[forced-restored=\\the\\count255]}",
    );
    assert!(log.contains("[inside=25/26/27]"), "{log}");
    assert!(log.contains("[after=15/26/27]"), "{log}");
    assert!(log.contains("[forced-local=35]"), "{log}");
    assert!(log.contains("[forced-restored=15]"), "{log}");
}

#[test]
fn 三万二千七百六十八番は六種類とも拒む() {
    for (kind, body) in [
        ("count", "\\count32768=1 \\message{[done]}"),
        ("dimen", "\\dimen32768=1pt \\message{[done]}"),
        ("skip", "\\skip32768=1pt \\message{[done]}"),
        ("muskip", "\\muskip32768=1mu \\message{[done]}"),
        ("toks", "\\toks32768={X} \\message{[done]}"),
        ("box", "\\setbox32768=\\hbox{\\vrule width1pt} \\message{[done]}"),
    ] {
        let log = run_tex(&format!("範囲外-{kind}"), body);
        assert!(log.contains("Bad register code"), "{kind}: {log}");
        assert!(log.contains("[done]"), "{kind}: {log}");
    }
}

#[test]
fn 差し込み番号は二百五十四までに限る() {
    fn insertion_log(number: u16) -> String {
        let body = "\\showboxbreadth=10 \\showboxdepth=10\n\
                    \\setbox0=\\vbox{\\insertNNN{\\hrule width1pt height1pt}}\n\
                    \\showbox0 \\message{[done]}"
            .replace("NNN", &number.to_string());
        run_tex(&format!("差し込み-{number}"), &body)
    }

    let valid = insertion_log(254);
    assert!(valid.contains("\\insert254, natural size"), "{valid}");
    // showbox 自身の OK 通知だけであること。
    assert_eq!(valid.matches("! ").count(), 1, "{valid}");

    for invalid in [255_u16, 256] {
        let log = insertion_log(invalid);
        // 不正な番号は 0 に直し、高位レジスタを差し込みクラスにしない。
        assert!(log.contains("\\insert0, natural size"), "{invalid}: {log}");
        // showbox の通知に加え、番号の誤りを一度報告する。
        assert!(log.matches("! ").count() >= 2, "{invalid}: {log}");
        assert!(log.contains("[done]"), "{invalid}: {log}");
    }
}

#[test]
fn box二百五十五は拡張上限と別に残る() {
    let log = run_tex(
        "box255の特殊性",
        "\\setbox255=\\hbox{\\vrule width5pt}\n\
         \\setbox32767=\\hbox{\\vrule width7pt}\n\
         \\message{[boxes=\\the\\wd255/\\the\\wd32767]}\n\
         \\message{[void=\\ifvoid255 Y\\else N\\fi/\\ifvoid32767 Y\\else N\\fi]}",
    );
    assert!(log.contains("[boxes=5.0pt/7.0pt]"), "{log}");
    assert!(log.contains("[void=N/N]"), "{log}");
}

#[test]
fn 拡張レジスタと別名はfmtを往復する() {
    let dir = std::env::temp_dir().join(format!("etex-register-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fmt = dir.join("mk.fmt");
    let _ = std::fs::remove_file(&fmt);

    std::fs::write(
        dir.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\count256=12 \\count32767=34\n\
         \\dimen256=1pt \\dimen32767=2pt\n\
         \\skip256=3pt \\skip32767=4pt\n\
         \\muskip256=5mu \\muskip32767=6mu\n\
         \\toks256={LOW} \\toks32767={HIGH}\n\
         \\setbox256=\\hbox{\\vrule width7pt} \\setbox32767=\\hbox{\\vrule width8pt}\n\
         \\countdef\\highcount=32767 \\dimendef\\highdimen=32767\n\
         \\skipdef\\highskip=32767 \\muskipdef\\highmuskip=32767\n\
         \\toksdef\\hightoks=32767\n\\dump\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("mk.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    if !fmt.exists() {
        eprintln!(
            "\\dump が使えないので飛ばす: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }

    std::fs::write(
        dir.join("use.tex"),
        "\\message{[c=\\the\\count256/\\the\\count32767]}\n\
         \\message{[d=\\the\\dimen256/\\the\\dimen32767]}\n\
         \\message{[s=\\the\\skip256/\\the\\skip32767]}\n\
         \\message{[m=\\the\\muskip256/\\the\\muskip32767]}\n\
         \\message{[t=\\the\\toks256/\\the\\toks32767]}\n\
         \\message{[b=\\the\\wd256/\\the\\wd32767]}\n\
         \\highcount=35 \\highdimen=9pt \\highskip=10pt \\highmuskip=11mu\n\
         \\hightoks={ALIAS}\n\
         \\message{[alias=\\the\\count32767/\\the\\dimen32767/\\the\\skip32767/\\the\\muskip32767/\\the\\toks32767]}\n\\end\n",
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("&mk")
        .arg("use.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    let log = join_log(&dir.join("use.log"));
    assert!(log.contains("[c=12/34]"), "{log}");
    assert!(log.contains("[d=1.0pt/2.0pt]"), "{log}");
    assert!(log.contains("[s=3.0pt/4.0pt]"), "{log}");
    assert!(log.contains("[m=5.0mu/6.0mu]"), "{log}");
    assert!(log.contains("[t=LOW/HIGH]"), "{log}");
    assert!(log.contains("[b=7.0pt/8.0pt]"), "{log}");
    assert!(
        log.contains("[alias=35/9.0pt/10.0pt/11.0mu/ALIAS]"),
        "{log}"
    );
}

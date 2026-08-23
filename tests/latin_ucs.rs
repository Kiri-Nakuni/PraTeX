//! upTeX `latin_ucs` のUnicode欧文一文字token。
//!
//! 原実装や上流試験は参照せず、公開仕様と公式e-upTeXの黒箱結果だけを固定する。

use std::hash::{Hash, Hasher};
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    let dir = std::env::temp_dir().join(format!(
        "latin-ucs-{}-{:x}",
        std::process::id(),
        hash.finish()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("t.tex"),
        format!(
            "\\catcode123=1\n\\catcode125=2\n\\catcode35=6\n\\batchmode\n{body}\n\\end\n"
        ),
    )
    .unwrap();
    let log_path = dir.join("t.log");
    let _ = std::fs::remove_file(&log_path);
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        output.status.success() && log_path.exists(),
        "PraTeXを実行できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&std::fs::read(log_path).unwrap()).replace('\n', "")
}

#[test]
fn unicode欧文を含むハイフネーションパターンを一文字として登録する() {
    let log = run_tex(
        "unicode欧文pattern",
        "\\kcatcode223=14
         \\catcode223=11
         \\lccode223=223
         \\kcatcode256=14
         \\catcode256=11
         \\lccode256=256
         \\patterns{.buß3 .Ā1}
         \\message{[patterns-after]}",
    );
    assert!(log.contains("[patterns-after]"), "{log}");
    assert!(!log.contains("Nonletter"), "{log}");
    assert!(!log.contains("Bad \\patterns"), "{log}");
}

#[test]
fn unicode欧文braceでpatternと例外の明示囲みを閉じる() {
    let log = run_tex(
        "unicode欧文brace hyphen data",
        "\\kcatcode256=14
         \\kcatcode257=14
         \\catcode256=1
         \\catcode257=2
         \\patternsĀ.a1ā
         \\hyphenationĀa-bā
         \\message{[hyphen-data-after]}",
    );
    assert!(log.contains("[hyphen-data-after]"), "{log}");
    assert!(!log.contains("Runaway"), "{log}");
    assert!(!log.contains("Bad \\patterns"), "{log}");
    assert!(!log.contains("Improper \\hyphenation"), "{log}");
}

#[test]
fn u二e八十case_sentinelをhyphen_trieへ格納しない() {
    let log = run_tex(
        "case sentinel pattern",
        "\\kcatcode223=14
         \\catcode223=11
         \\lccode223=11904
         \\patterns{.ß1}
         \\message{[sentinel-after]}",
    );
    assert!(log.contains("Nonletter"), "{log}");
    assert!(log.contains("[sentinel-after]"), "{log}");
}

#[test]
fn detokenizeしたunicode欧文はcatcode十二になる() {
    let log = run_tex(
        "detokenize-catcode十二",
        "\\kcatcode223=14
         \\catcode223=11
         \\lccode223=223
         \\def\\saved{\\detokenize{ß}}
         \\message{[other=\\ifcat\\saved ?T\\else F\\fi]}
         \\message{[letter=\\ifcat\\saved ßT\\else F\\fi]}",
    );
    assert!(log.contains("[other=T]"), "{log}");
    assert!(log.contains("[letter=F]"), "{log}");
}

#[test]
fn stringしたunicode欧文はcatcode十二になる() {
    let log = run_tex(
        "string catcode twelve",
        "\\kcatcode256=14
         \\catcode256=1
         \\edef\\saved{\\string Ā}
         \\message{[other=\\ifcat\\saved ?T\\else F\\fi]}
         \\message{[left=\\ifcat\\saved ĀT\\else F\\fi]}",
    );
    assert!(log.contains("[other=T]"), "{log}");
    assert!(log.contains("[left=F]"), "{log}");
}

#[test]
fn unicode活性文字と同じ符号位置の制御記号は別定義になる() {
    let log = run_tex(
        "activeとsymbol",
        "\\kcatcode223=14
         \\catcode223=13
         \\defß{ACTIVE-OUTER}
         {\\defß{ACTIVE-INNER}\\message{[inner=ß]}}
         \\message{[active=ß]}
         \\catcode223=12
         \\def\\ß{SYMBOL}
         \\message{[symbol=\\ß][csname=\\csname ß\\endcsname][ifcsname=\\ifcsname ß\\endcsname T\\else F\\fi]}
         \\catcode223=13
         \\message{[active-again=ß]}",
    );
    assert!(log.contains("[inner=ACTIVE-INNER]"), "{log}");
    assert!(log.contains("[active=ACTIVE-OUTER]"), "{log}");
    assert!(
        log.contains("[symbol=SYMBOL][csname=SYMBOL][ifcsname=T]"),
        "{log}"
    );
    assert!(log.contains("[active-again=ACTIVE-OUTER]"), "{log}");
}

#[test]
fn unicode欧文のgroupとmacro引数のbrace判定を分離する() {
    let log = run_tex(
        "groupとmacro引数",
        "\\kcatcode256=14
         \\kcatcode257=14
         \\catcode256=1
         \\catcode257=2
         \\count0=1 Ā\\count0=2 ā
         \\message{[group=\\the\\count0]}
         \\def\\G#1{<#1>}
         \\message{[arg=\\GĀZ]}",
    );
    assert!(log.contains("[group=1]"), "{log}");
    assert!(log.contains("[arg=<Ā>Z]"), "{log}");
}

#[test]
fn unicode欧文braceはgeneral_textとmacro置換のraw_tokenとして残る() {
    let log = run_tex(
        "raw brace token",
        "\\kcatcode256=14
         \\kcatcode257=14
         \\catcode256=1
         \\catcode257=2
         \\message{[general=ĀXāY]}
         \\def\\R{ĀXāY}
         \\message{[replacement=\\R]}",
    );
    assert!(log.contains("[general=ĀXāY]"), "{log}");
    assert!(log.contains("[replacement=ĀXāY]"), "{log}");
}

#[test]
fn unicode欧文のmacro_parameterを置換に使える() {
    let log = run_tex(
        "unicode macro parameter",
        "\\kcatcode256=14
         \\catcode256=6
         \\def\\FĀ1{[Ā1]}
         \\message{[result=\\F{OK}]}",
    );
    assert!(log.contains("[result=[OK]]"), "{log}");
}

#[test]
fn case変換はbyteとunicode欧文を跨いで元のcatcodeを保つ() {
    let log = run_tex(
        "case cross lane",
        "\\kcatcode256=14
         \\catcode256=11
         \\lccode256=97
         \\uccode97=256
         \\def\\doLower#1{\\message{[lower-code=\\number`#1][lower-cat=\\ifcat#1aT\\else F\\fi]}}
         \\def\\doUpper#1{\\message{[upper-code=\\number`#1][upper-cat=\\ifcat#1ĀT\\else F\\fi]}}
         \\lowercase{\\doLower Ā}
         \\uppercase{\\doUpper a}
         \\catcode256=12
         \\def\\doOther#1{\\message{[other-cat=\\ifcat#1?T\\else F\\fi]}}
         \\lowercase{\\doOther Ā}",
    );
    assert!(log.contains("[lower-code=97][lower-cat=T]"), "{log}");
    assert!(log.contains("[upper-code=256][upper-cat=T]"), "{log}");
    assert!(log.contains("[other-cat=T]"), "{log}");
}

#[test]
fn case変換のu零零df恒等写像はutf八tokenを保つ() {
    let log = run_tex(
        "case U+00DF identity",
        "\\kcatcode223=14
         \\catcode223=11
         \\lccode223=223
         \\def\\probe#1{\\message{[code=\\number`#1][meaning=\\meaning#1][out=#1]}}
         \\lowercase{\\probe ß}",
    );
    assert!(
        log.contains("[code=223][meaning=the letter ß][out=ß]"),
        "{log}"
    );
}

#[test]
fn latin_ucsのcatcode十五はinvalidでなく和文tokenへfallbackする() {
    let log = run_tex(
        "catcode十五fallback",
        "\\kcatcode223=14
         \\catcode223=15
         \\message{[meaning=\\meaning ß]}",
    );
    assert!(log.contains("[meaning=kanji character ß]"), "{log}");
    assert!(!log.contains("Invalid character"), "{log}");
}

#[test]
fn unicode欧文braceは必須左braceにはなるがbalanced_textの構造にはしない() {
    let log = run_tex(
        "explicit braceとbalanced text",
        "\\kcatcode256=14
         \\kcatcode257=14
         \\catcode256=1
         \\catcode257=2
         \\toks0=ĀAāB}
         \\message{[toks=\\the\\toks0]}",
    );
    assert!(log.contains("[toks=AāB]"), "{log}");
    assert!(!log.contains("Missing { inserted"), "{log}");
}

#[test]
fn unicode欧文の特殊catcodeを型付きcommandとして公開する() {
    let log = run_tex(
        "特殊catcode meaning",
        "\\kcatcode256=14
         \\catcode256=3 \\message{[math=\\meaning Ā]}
         \\catcode256=4 \\message{[tab=\\meaning Ā]}
         \\catcode256=6 \\message{[param=\\meaning Ā]}
         \\catcode256=7 \\message{[sup=\\meaning Ā]}
         \\catcode256=8 \\message{[sub=\\meaning Ā]}",
    );
    assert!(log.contains("[math=math shift character Ā]"), "{log}");
    assert!(log.contains("[tab=alignment tab character Ā]"), "{log}");
    assert!(log.contains("[param=macro parameter character Ā]"), "{log}");
    assert!(log.contains("[sup=superscript character Ā]"), "{log}");
    assert!(log.contains("[sub=subscript character Ā]"), "{log}");
}

#[test]
fn unicode欧文のalignment_tabで二列のpreambleを作れる() {
    let log = run_tex(
        "unicode alignment tab",
        "\\kcatcode256=14
         \\catcode256=4
         \\setbox0=\\vbox{\\halign{#Ā#\\cr\\cr}}
         \\message{[alignment-after]}",
    );
    assert!(log.contains("[alignment-after]"), "{log}");
    assert!(!log.contains("Missing # inserted"), "{log}");
    assert!(!log.contains("Extra alignment tab"), "{log}");
}

#[test]
fn unicode欧文braceのalignment_deltaをmacro区切り走査でも相殺する() {
    let log = run_tex(
        "unicode alignment delta",
        "\\kcatcode256=14
         \\kcatcode257=14
         \\catcode256=1
         \\catcode257=2
         \\def\\eat#1Z{}
         \\setbox0=\\vbox{\\halign{#&#\\cr \\eat Ā&āZ A&B\\cr}}
         \\message{[align-brace-after]}",
    );
    assert!(log.contains("[align-brace-after]"), "{log}");
    assert!(!log.contains("Extra alignment tab"), "{log}");
    assert!(!log.contains("Forbidden control sequence"), "{log}");
}

#[test]
fn unicode欧文のifは符号位置をifcatはcatcodeを比較する() {
    let log = run_tex(
        "unicode if and ifcat",
        "\\kcatcode223=14
         \\catcode223=11 \\def\\A{ß}
         \\catcode223=12 \\def\\B{ß}
         \\message{[if=\\if\\A\\B T\\else F\\fi]}
         \\message{[ifcat=\\ifcat\\A\\B T\\else F\\fi]}
         \\kcatcode256=14 \\kcatcode257=14
         \\catcode256=3 \\catcode257=3
         \\message{[same-cat=\\ifcat ĀāT\\else F\\fi]}",
    );
    assert!(log.contains("[if=T]"), "{log}");
    assert!(log.contains("[ifcat=F]"), "{log}");
    assert!(log.contains("[same-cat=T]"), "{log}");
}

//! upTeXのUTF-8一文字tokenと型付き制御綴名。
//!
//! 原実装や上流試験は参照せず、公開仕様と公式e-upTeXの黒箱結果だけを固定する。

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    run_tex_in_dir(name, body).1
}

fn run_tex_in_dir(name: &str, body: &str) -> (PathBuf, String) {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    let dir = std::env::temp_dir().join(format!(
        "cjk-token-{}-{:x}",
        std::process::id(),
        hash.finish()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("t.tex"),
        format!("\\catcode123=1\n\\catcode125=2\n\\batchmode\n{body}\n\\end\n"),
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
        "rtexを実行できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = String::from_utf8_lossy(&std::fs::read(log_path).unwrap()).replace('\n', "");
    (dir, log)
}

#[test]
fn 和文tokenは符号位置と入力時categoryを別々に保持する() {
    let log = run_tex(
        "category保持",
        "\\kcatcode\"3042=16
         \\def\\a{あ}
         \\kcatcode\"3042=17
         \\def\\b{あ}
         \\message{[if=\\if\\a\\b T\\else F\\fi]}
         \\message{[ifcat=\\ifcat\\a\\b T\\else F\\fi]}
         \\message{[ifx=\\ifx\\a\\b T\\else F\\fi]}
         \\message{[meaning=\\meaning あ]}",
    );
    assert!(log.contains("[if=T]"), "{log}");
    assert!(log.contains("[ifcat=F]"), "{log}");
    assert!(log.contains("[ifx=F]"), "{log}");
    assert!(log.contains("[meaning=kanji character あ]"), "{log}");
}

#[test]
fn wide名と同じ見た目のutf八byte名を混同しない() {
    let log = run_tex(
        "制御綴identity",
        "\\kcatcode\"3042=16
         \\def\\あ{WIDE}
         \\kcatcode\"3042=17
         \\message{[category-shared=\\あ]}
         \\kcatcode\"3042=15
         \\expandafter\\def\\csname あ\\endcsname{BYTES}
         \\message{[bytes=\\csname あ\\endcsname]}
         \\kcatcode\"3042=16
         \\message{[wide=\\あ][wide-csname=\\csname あ\\endcsname]}",
    );
    assert!(log.contains("[category-shared=WIDE]"), "{log}");
    assert!(log.contains("[bytes=BYTES]"), "{log}");
    assert!(log.contains("[wide=WIDE][wide-csname=WIDE]"), "{log}");
}

#[test]
fn 和文categoryごとの制御語と制御記号の空白規則を保つ() {
    let log = run_tex(
        "制御綴走査",
        "\\kcatcode\"3042=16
         \\def\\あA{MIX}
         \\message{[mixed=\\あA Z]}
         \\kcatcode\"3042=18
         \\def\\あ{SYMBOL}
         \\message{[symbol=\\あ Z]}
         \\kcatcode\"3042=20
         \\def\\あ{ONE}
         \\def\\あA{MANY}
         \\message{[modifier-one=\\あ Z][modifier-many=\\あA Z]}",
    );
    assert!(log.contains("[mixed=MIXZ]"), "{log}");
    assert!(log.contains("[symbol=SYMBOL Z]"), "{log}");
    assert!(
        log.contains("[modifier-one=ONE Z][modifier-many=MANYZ]"),
        "{log}"
    );
}

#[test]
fn wide制御綴の表示空白は現在のkcatcodeと論理長で決まる() {
    let log = run_tex(
        "制御綴表示",
        "\\kcatcode\"3042=16
         \\kcatcode\"3044=16
         \\def\\あ{W}
         \\def\\あい{M}
         \\def\\one{\\あ}
         \\def\\two{\\あい}
         \\show\\あ
         \\message{[one16=\\meaning\\one][two16=\\meaning\\two]}
         \\kcatcode\"3042=18
         \\message{[one18=\\meaning\\one][two18=\\meaning\\two]}
         \\kcatcode\"3042=15
         \\message{[one15=\\meaning\\one]}",
    );
    assert!(log.contains("[one16=macro:->\\あ ]"), "{log}");
    assert!(log.contains("[two16=macro:->\\あい ]"), "{log}");
    assert!(log.contains("[one18=macro:->\\あ]"), "{log}");
    assert!(log.contains("[two18=macro:->\\あい ]"), "{log}");
    assert!(log.contains("[one15=macro:->\\あ ]"), "{log}");
    assert!(log.contains("> \\あ=macro:"), "{log}");
}

#[test]
fn 一文字wide制御綴をalphabetic定数として符号位置へ戻す() {
    let log = run_tex(
        "和文alphabetic定数",
        "\\kcatcode\"3042=16
         \\message{[code=\\number`\\あ]}",
    );
    assert!(log.contains("[code=12354]"), "{log}");
}

#[test]
fn stringとdetokenizeは実行時のkcatcodeで再分類する() {
    let log = run_tex(
        "再分類",
        "\\kcatcode\"3042=16
         \\def\\saved{あ}
         \\def\\detok{\\detokenize{あ}}
         \\kcatcode\"3042=17
         \\message{[string17=\\ifcat\\expandafter\\string\\saved あT\\else F\\fi]}
         \\message{[detok17=\\ifcat\\detok あT\\else F\\fi]}
         \\kcatcode\"3042=18
         \\def\\other{あ}
         \\kcatcode\"3042=15
         \\message{[string15=\\ifcat\\expandafter\\string\\saved\\other T\\else F\\fi]}",
    );
    assert!(log.contains("[string17=T]"), "{log}");
    assert!(log.contains("[detok17=T]"), "{log}");
    assert!(log.contains("[string15=T]"), "{log}");
}

#[test]
fn 和文字のutf八継続byteをnewlinecharと取り違えない() {
    let log = run_tex("和文newlinechar", "\\newlinechar=129 \\message{[あ]}");
    assert!(log.contains("[あ]"), "{log}");
}

#[test]
fn writeは和文字を二重符号化せずutf八で保存する() {
    let (dir, log) = run_tex_in_dir(
        "和文write",
        "\\immediate\\openout0=out.txt
         \\immediate\\write0{[あ]}
         \\immediate\\closeout0",
    );
    assert!(!log.contains("! "), "{log}");
    let written = std::fs::read(dir.join("out.txt")).unwrap();
    assert!(
        written
            .windows("[あ]".len())
            .any(|part| part == "[あ]".as_bytes()),
        "{}",
        String::from_utf8_lossy(&written)
    );
}

#[test]
fn 直接入力のエラー文脈は有効utf八を一文字のまま表示する() {
    let log = run_tex(
        "和文error文脈",
        "\\kcatcode\"3042=16
         \\count0=あ",
    );
    assert!(log.contains("あ"), "{log}");
    assert!(!log.contains("^^e3^^81^^82"), "{log}");
}

#[test]
fn cjk_tokenとtyped制御綴名はfmtを往復する() {
    let dir = std::env::temp_dir().join(format!("cjk-token-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fmt = dir.join("mk.fmt");
    let _ = std::fs::remove_file(&fmt);
    let _ = std::fs::remove_file(dir.join("mk.log"));
    let use_log = dir.join("use.log");
    let _ = std::fs::remove_file(&use_log);

    std::fs::write(
        dir.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\kcatcode\"3042=16\n\
         \\def\\あ{WIDE}\n\
         \\def\\saved{あ}\n\
         \\kcatcode\"3042=15\n\
         \\expandafter\\def\\csname あ\\endcsname{BYTES}\n\
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
        "CJK fmtを生成できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::write(
        dir.join("use.tex"),
        "\\message{[bytes=\\csname あ\\endcsname]}\n\
         \\kcatcode\"3042=17\n\
         \\message{[wide=\\あ]}\n\
         \\message{[saved-if=\\if\\saved あT\\else F\\fi]}\n\
         \\message{[saved-ifcat=\\ifcat\\saved あT\\else F\\fi]}\n\
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
        "CJK fmtを読み戻せなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = String::from_utf8_lossy(&std::fs::read(use_log).unwrap()).replace('\n', "");
    assert!(log.contains("[bytes=BYTES]"), "{log}");
    assert!(log.contains("[wide=WIDE]"), "{log}");
    assert!(log.contains("[saved-if=T]"), "{log}");
    assert!(log.contains("[saved-ifcat=F]"), "{log}");
}

//! e-TeX の mark class。
//!
//! 公開 e-TeX マニュアル 3.6 にある、0--32767 のクラス、class 0 と
//! TeX82 の mark の同一性、ページ組版と `\vsplit` のクラス別記録を確かめる。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    let dir = std::env::temp_dir().join(format!(
        "etex-marks-{}-{:x}",
        std::process::id(),
        h.finish()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(
        f,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n{body}\n\\end\n"
    )
    .unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    join_log(&dir.join("t.log"))
}

/// TeX の記録は mark text の途中でも 79 桁で折り返される。
fn join_log(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap().replace('\n', "")
}

#[test]
fn 零番クラスは従来のmarkと同じである() {
    let log = run_tex(
        "零番と従来mark",
        "\\setbox0=\\vbox{\\mark{OLD-FIRST}\\marks0{NEW-BOT}\\hbox{}}\n\
         \\setbox1=\\vsplit0 to 100pt\n\
         \\message{[old=\\splitfirstmark/\\splitbotmark]}\n\
         \\message{[zero=\\splitfirstmarks0/\\splitbotmarks0]}",
    );
    assert!(log.contains("[old=OLD-FIRST/NEW-BOT]"), "{log}");
    assert!(log.contains("[zero=OLD-FIRST/NEW-BOT]"), "{log}");
}

#[test]
fn 離れた三クラスは互いに混ざらない() {
    let log = run_tex(
        "三クラスの独立",
        "\\setbox0=\\vbox{\n\
           \\marks1{ONE-FIRST}\\marks256{MID-FIRST}\\marks32767{HIGH-FIRST}\n\
           \\marks32767{HIGH-BOT}\\marks1{ONE-BOT}\\marks256{MID-BOT}\\hbox{}}\n\
         \\setbox1=\\vsplit0 to 100pt\n\
         \\message{[first=\\splitfirstmarks1/\\splitfirstmarks256/\\splitfirstmarks32767]}\n\
         \\message{[bot=\\splitbotmarks1/\\splitbotmarks256/\\splitbotmarks32767]}",
    );
    assert!(
        log.contains("[first=ONE-FIRST/MID-FIRST/HIGH-FIRST]"),
        "{log}"
    );
    assert!(log.contains("[bot=ONE-BOT/MID-BOT/HIGH-BOT]"), "{log}");
}

#[test]
fn ページを越えるとbotが次のtopになる() {
    let log = run_tex(
        "ページ間のmark",
        "\\count0=0 \\vsize=100pt\n\
         \\output={\\global\\advance\\count0 by 1\n\
           \\message{[page\\the\\count0:old=\\topmark/\\firstmark/\\botmark;zero=\\topmarks0/\\firstmarks0/\\botmarks0;one=\\topmarks1/\\firstmarks1/\\botmarks1;mid=\\topmarks256/\\firstmarks256/\\botmarks256;high=\\topmarks32767/\\firstmarks32767/\\botmarks32767]}\n\
           \\shipout\\box255}\n\
         \\mark{Z1A}\\marks0{Z1B}\\marks1{O1A}\\marks1{O1B}\n\
         \\marks256{M1A}\\marks256{M1B}\\marks32767{H1A}\\marks32767{H1B}\n\
         \\hbox{}\\penalty-10000\n\
         \\marks0{Z2A}\\marks0{Z2B}\\marks1{O2A}\\marks1{O2B}\n\
         \\marks256{M2A}\\marks256{M2B}\\marks32767{H2A}\\marks32767{H2B}\n\
         \\hbox{}\\penalty-10000",
    );
    assert!(
        log.contains("[page1:old=/Z1A/Z1B;zero=/Z1A/Z1B;one=/O1A/O1B;mid=/M1A/M1B;high=/H1A/H1B]"),
        "{log}"
    );
    assert!(
        log.contains(
            "[page2:old=Z1B/Z2A/Z2B;zero=Z1B/Z2A/Z2B;one=O1B/O2A/O2B;mid=M1B/M2A/M2B;high=H1B/H2A/H2B]"
        ),
        "{log}"
    );
}

#[test]
fn markのない次ページは前ページのbotを三つの値へ引き継ぐ() {
    let log = run_tex(
        "markのない次ページ",
        "\\count0=0 \\vsize=100pt
         \\output={\\global\\advance\\count0 by 1
           \\message{[page\\the\\count0:one=\\topmarks1/\\firstmarks1/\\botmarks1;high=\\topmarks32767/\\firstmarks32767/\\botmarks32767]}
           \\shipout\\box255}
         \\marks1{ONE}\\marks32767{HIGH}\\hbox{}\\penalty-10000
         \\hbox{}\\penalty-10000",
    );
    assert!(log.contains("[page1:one=/ONE/ONE;high=/HIGH/HIGH]"), "{log}");
    assert!(
        log.contains("[page2:one=ONE/ONE/ONE;high=HIGH/HIGH/HIGH]"),
        "{log}"
    );
}

#[test]
fn 連続したvsplitはクラス別の先頭と末尾を更新する() {
    let log = run_tex(
        "連続vsplit",
        "\\setbox0=\\vbox{\n\
           \\marks1{O1A}\\marks256{M1A}\\marks32767{H1A}\n\
           \\marks1{O1B}\\marks256{M1B}\\marks32767{H1B}\n\
           \\hbox{\\vrule height 1pt}\\penalty-10000\n\
           \\marks1{O2A}\\marks256{M2A}\\marks32767{H2A}\n\
           \\marks1{O2B}\\marks256{M2B}\\marks32767{H2B}\n\
           \\hbox{\\vrule height 1pt}}\n\
         \\setbox1=\\vsplit0 to 1pt\n\
         \\message{[split1-first=\\splitfirstmarks1/\\splitfirstmarks256/\\splitfirstmarks32767]}\n\
         \\message{[split1-bot=\\splitbotmarks1/\\splitbotmarks256/\\splitbotmarks32767]}\n\
         \\setbox2=\\vsplit0 to 100pt\n\
         \\message{[split2-first=\\splitfirstmarks1/\\splitfirstmarks256/\\splitfirstmarks32767]}\n\
         \\message{[split2-bot=\\splitbotmarks1/\\splitbotmarks256/\\splitbotmarks32767]}",
    );
    assert!(log.contains("[split1-first=O1A/M1A/H1A]"), "{log}");
    assert!(log.contains("[split1-bot=O1B/M1B/H1B]"), "{log}");
    assert!(log.contains("[split2-first=O2A/M2A/H2A]"), "{log}");
    assert!(log.contains("[split2-bot=O2B/M2B/H2B]"), "{log}");
}

#[test]
fn voidとhboxへのvsplitも全クラスのsplit_markを消す() {
    let log = run_tex(
        "失敗vsplitの初期化",
        "\\setbox0=\\vbox{\\marks1{ONE}\\marks256{MID}\\marks32767{HIGH}\\hbox{}}
         \\setbox2=\\vsplit0 to 100pt
         \\message{[before=\\splitfirstmarks1/\\splitfirstmarks256/\\splitfirstmarks32767]}
         \\setbox2=\\vsplit1 to 1pt
         \\message{[void=\\splitfirstmarks1/\\splitfirstmarks256/\\splitfirstmarks32767]}
         \\setbox0=\\vbox{\\marks1{TWO}\\marks256{MIDDLE}\\marks32767{UPPER}\\hbox{}}
         \\setbox2=\\vsplit0 to 100pt
         \\setbox1=\\hbox{not a vbox}
         \\setbox2=\\vsplit1 to 1pt
         \\message{[hbox=\\splitfirstmarks1/\\splitfirstmarks256/\\splitfirstmarks32767]}",
    );
    assert!(log.contains("[before=ONE/MID/HIGH]"), "{log}");
    assert!(log.contains("[void=//]"), "{log}");
    assert!(log.contains("[hbox=//]"), "{log}");
}

#[test]
fn mark本文はprotected命令を走査時に展開しない() {
    let log = run_tex(
        "mark本文のprotected",
        "\\protected\\def\\p{EXPANDED}\n\
         \\setbox0=\\vbox{\\marks1{A\\p B}\\hbox{}}\n\
         \\setbox1=\\vsplit0 to 100pt\n\
         \\edef\\captured{\\splitfirstmarks1}\n\
         \\message{[captured=\\meaning\\captured]}",
    );
    assert!(log.contains("[captured=macro:->A\\p B]"), "{log}");
}

#[test]
fn meaningは複数形mark命令の後続番号を読まない() {
    let log = run_tex(
        "mark命令のmeaning",
        "\\message{[marks=\\meaning\\marks 1]}\n\
         \\message{[top=\\meaning\\topmarks 1]}\n\
         \\message{[first=\\meaning\\firstmarks 1]}\n\
         \\message{[bot=\\meaning\\botmarks 1]}\n\
         \\message{[splitfirst=\\meaning\\splitfirstmarks 1]}\n\
         \\message{[splitbot=\\meaning\\splitbotmarks 1]}",
    );
    for expected in [
        "[marks=\\marks1]",
        "[top=\\topmarks1]",
        "[first=\\firstmarks1]",
        "[bot=\\botmarks1]",
        "[splitfirst=\\splitfirstmarks1]",
        "[splitbot=\\splitbotmarks1]",
    ] {
        assert!(log.contains(expected), "{expected}: {log}");
    }
}

#[test]
fn 三万二千七百六十八番のmarkクラスはすべて拒む() {
    for (primitive, body) in [
        ("marks", "\\marks32768{X}\\message{[done]}"),
        ("topmarks", "\\message{[\\topmarks32768]}\\message{[done]}"),
        (
            "firstmarks",
            "\\message{[\\firstmarks32768]}\\message{[done]}",
        ),
        ("botmarks", "\\message{[\\botmarks32768]}\\message{[done]}"),
        (
            "splitfirstmarks",
            "\\message{[\\splitfirstmarks32768]}\\message{[done]}",
        ),
        (
            "splitbotmarks",
            "\\message{[\\splitbotmarks32768]}\\message{[done]}",
        ),
    ] {
        let log = run_tex(&format!("範囲外-{primitive}"), body);
        assert!(
            log.contains("Bad register code"),
            "{primitive} が 32768 を拒まなかった: {log}"
        );
        assert!(log.contains("[done]"), "{primitive}: {log}");
    }
}

#[test]
fn mark節点のクラスはfmtを往復する() {
    let dir = std::env::temp_dir().join(format!("etex-marks-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fmt = dir.join("mk.fmt");
    let _ = std::fs::remove_file(&fmt);

    std::fs::write(
        dir.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\setbox0=\\vbox{\\marks1{ONE}\\marks256{MID}\\marks32767{HIGH}\\hbox{}}\n\
         \\dump\n",
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
        "\\batchmode\n\
         \\setbox1=\\vsplit0 to 100pt\n\
         \\message{[fmt=\\splitfirstmarks1/\\splitfirstmarks256/\\splitfirstmarks32767]}\n\
         \\end\n",
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("&mk")
        .arg("use.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    let log = join_log(&dir.join("use.log"));
    assert!(log.contains("[fmt=ONE/MID/HIGH]"), "{log}");
}

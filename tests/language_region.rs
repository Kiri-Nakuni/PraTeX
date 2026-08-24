//! PraTeX 固有の組版言語・地域状態。

use std::hash::{Hash, Hasher};
use std::process::{Command, Output};

fn case_dir(name: &str) -> std::path::PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    let dir = std::env::temp_dir().join(format!(
        "pratex-region-{}-{:x}",
        std::process::id(),
        hash.finish()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_tex(name: &str, body: &str) -> String {
    let dir = case_dir(name);
    let log_path = dir.join("t.log");
    let _ = std::fs::remove_file(&log_path);
    std::fs::write(
        dir.join("t.tex"),
        format!("\\catcode123=1\n\\catcode125=2\n\\batchmode\n{body}\n\\end\n"),
    )
    .unwrap();
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
    join_log(&log_path)
}

fn join_log(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap().replace('\n', "")
}

fn make_format(name: &str, body: &str) -> (std::path::PathBuf, Output) {
    make_format_with_codec(name, body, None)
}

fn make_format_with_codec(
    name: &str,
    body: &str,
    codec: Option<&str>,
) -> (std::path::PathBuf, Output) {
    let dir = case_dir(name);
    let fmt_path = dir.join("mk.fmt");
    let _ = std::fs::remove_file(&fmt_path);
    let _ = std::fs::remove_file(dir.join("mk.log"));
    std::fs::write(
        dir.join("mk.tex"),
        format!("\\catcode123=1\n\\catcode125=2\n\\batchmode\n{body}\n\\dump\n"),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_rtex"));
    command.arg("mk.tex").current_dir(&dir);
    if let Some(codec) = codec {
        command.env("PRATEX_FMT_CODEC", codec);
    }
    let output = command.output().unwrap();
    (dir, output)
}

#[test]
fn 既定値と六つの公開値を読める() {
    let log = run_tex(
        "公開値",
        "\\message{[default=\\the\\pratexregion]}\n\
         \\pratexregion=0 \\message{[und=\\the\\pratexregion]}\n\
         \\pratexregion=1 \\message{[ja=\\the\\pratexregion]}\n\
         \\pratexregion=2 \\message{[zh-hans=\\the\\pratexregion]}\n\
         \\pratexregion=3 \\message{[zh-hant=\\the\\pratexregion]}\n\
         \\pratexregion=4 \\message{[ko=\\the\\pratexregion]}\n\
         \\pratexregion=5 \\message{[vi=\\the\\pratexregion]}",
    );
    for expected in [
        "[default=0]",
        "[und=0]",
        "[ja=1]",
        "[zh-hans=2]",
        "[zh-hant=3]",
        "[ko=4]",
        "[vi=5]",
    ] {
        assert!(log.contains(expected), "{expected}: {log}");
    }
}

#[test]
fn 局所大域とglobaldefsを保存スタックで扱う() {
    let log = run_tex(
        "保存スタック",
        "\\pratexregion=1\n\
         {\\pratexregion=2 \\message{[local=\\the\\pratexregion]}}\n\
         \\message{[restored=\\the\\pratexregion]}\n\
         {\\global\\pratexregion=3}\n\
         \\message{[global=\\the\\pratexregion]}\n\
         {\\globaldefs=1 \\pratexregion=4}\n\
         \\message{[forced-global=\\the\\pratexregion]}\n\
         {\\globaldefs=-1 \\global\\pratexregion=5\n\
          \\message{[forced-local=\\the\\pratexregion]}}\n\
         \\message{[forced-restored=\\the\\pratexregion]}",
    );
    for expected in [
        "[local=2]",
        "[restored=1]",
        "[global=3]",
        "[forced-global=4]",
        "[forced-local=5]",
        "[forced-restored=4]",
    ] {
        assert!(log.contains(expected), "{expected}: {log}");
    }
}

#[test]
fn theとshowtheとmeaningとletが同じ型を参照する() {
    let log = run_tex(
        "内部量と別名",
        "\\pratexregion=3\n\
         \\let\\regioncopy=\\pratexregion\n\
         \\message{[the=\\the\\regioncopy/meaning=\\meaning\\regioncopy]}\n\
         \\ifx\\regioncopy\\count0\\message{[typed=same]}\\else\\message{[typed=distinct]}\\fi\n\
         \\showthe\\regioncopy\n\
         \\regioncopy=5\n\
         \\message{[shared=\\the\\pratexregion/\\the\\regioncopy]}",
    );
    assert!(log.contains("[the=3/meaning=\\pratexregion]"), "{log}");
    assert!(log.contains("[typed=distinct]"), "{log}");
    assert!(log.contains("> 3."), "{log}");
    assert!(log.contains("[shared=5/5]"), "{log}");
}

#[test]
fn 範囲外代入は現在値を保つ() {
    let log = run_tex(
        "範囲外",
        "\\pratexregion=2\n\
         \\pratexregion=6 \\message{[after-high=\\the\\pratexregion]}\n\
         \\pratexregion=-1 \\message{[after-low=\\the\\pratexregion]}",
    );
    assert_eq!(log.matches("Invalid PraTeX region").count(), 2, "{log}");
    assert!(log.contains("[after-high=2]"), "{log}");
    assert!(log.contains("[after-low=2]"), "{log}");
}

#[test]
fn language番号と組版地域は独立している() {
    let log = run_tex(
        "languageとの独立",
        "\\language=42 \\pratexregion=2\n\
         \\message{[first=\\the\\language/\\the\\pratexregion]}\n\
         \\language=7\n\
         \\message{[language-only=\\the\\language/\\the\\pratexregion]}\n\
         \\pratexregion=5\n\
         \\message{[region-only=\\the\\language/\\the\\pratexregion]}",
    );
    assert!(log.contains("[first=42/2]"), "{log}");
    assert!(log.contains("[language-only=7/2]"), "{log}");
    assert!(log.contains("[region-only=7/5]"), "{log}");
}

#[test]
fn 算術命令は組版地域を書き換えない() {
    let log = run_tex(
        "算術拒否",
        "\\pratexregion=4\n\
         \\let\\regioncopy=\\pratexregion\n\
         \\advance\\pratexregion\\relax\n\
         \\message{[after-advance=\\the\\pratexregion]}\n\
         \\multiply\\pratexregion\\relax\n\
         \\message{[after-multiply=\\the\\pratexregion]}\n\
         \\divide\\pratexregion\\relax\n\
         \\message{[after-divide=\\the\\pratexregion]}\n\
         \\advance\\regioncopy\\relax\n\
         \\message{[after-alias=\\the\\pratexregion]}",
    );
    assert_eq!(log.matches("You can't use").count(), 4, "{log}");
    assert!(log.contains("[after-advance=4]"), "{log}");
    assert!(log.contains("[after-multiply=4]"), "{log}");
    assert!(log.contains("[after-divide=4]"), "{log}");
    assert!(log.contains("[after-alias=4]"), "{log}");
}

#[test]
fn 組版地域と別名はfmtを往復する() {
    let (dir, output) = make_format(
        "fmt往復",
        "\\language=91\n\\pratexregion=3\n\\let\\savedregion=\\pratexregion",
    );
    let fmt_path = dir.join("mk.fmt");
    assert!(
        output.status.success() && fmt_path.exists(),
        "fmtを生成できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let use_log = dir.join("use.log");
    let _ = std::fs::remove_file(&use_log);
    std::fs::write(
        dir.join("use.tex"),
        "\\message{[fmt=\\the\\pratexregion/\\the\\savedregion/\\the\\language/\\meaning\\savedregion]}\n\\end\n",
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
    assert!(log.contains("[fmt=3/3/91/\\pratexregion]"), "{log}");
}

#[test]
fn 壊れたfmtの範囲外地域を拒む() {
    let (dir, output) =
        make_format_with_codec("fmt範囲外", "\\pratexregion=3", Some("legacy-text"));
    let fmt_path = dir.join("mk.fmt");
    assert!(
        output.status.success() && fmt_path.exists(),
        "fmtを生成できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let format = std::fs::read_to_string(&fmt_path).unwrap();
    let valid = "LanguageRegion/PraTeX-1\n3\n";
    assert_eq!(format.matches(valid).count(), 1, "{format}");
    let corrupted = format.replacen(valid, "LanguageRegion/PraTeX-1\n6\n", 1);
    std::fs::write(&fmt_path, corrupted).unwrap();
    std::fs::write(dir.join("use.tex"), "\\end\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("&mk")
        .arg("use.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!output.status.success(), "壊れたfmtを受理した");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Format error"),
        "診断が無い: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

//! upTeX の `\kcatcode` 表と代入。
//!
//! UTF-8 lexer はまだ入れないため、文字そのものではなく数値指定だけで試す。

use std::hash::{Hash, Hasher};
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    let dir = std::env::temp_dir().join(format!(
        "kcatcode-{}-{:x}",
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
    join_log(&log_path)
}

fn join_log(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap().replace('\n', "")
}

#[test]
fn 公開表の日本語と例外集合を数値で引ける() {
    let log = run_tex(
        "既定値",
        "\\message{[basic=\\the\\kcatcode0/\\the\\kcatcode127]}\n\
         \\message{[latin=\\the\\kcatcode\"AA/\\the\\kcatcode\"A1]}\n\
         \\message{[ja=\\the\\kcatcode\"3042/\\the\\kcatcode\"30FC/\\the\\kcatcode\"4E00]}\n\
         \\message{[ko=\\the\\kcatcode\"AC00]}\n\
         \\message{[modifier=\\the\\kcatcode\"3099/\\the\\kcatcode\"E0100]}\n\
         \\message{[wide=\\the\\kcatcode\"FF21/\\the\\kcatcode\"FF66]}\n\
         \\message{[range=\\the\\kcatcode0/\\the\\kcatcode\"10FFFF]}",
    );
    assert!(log.contains("[basic=15/15]"), "{log}");
    assert!(log.contains("[latin=15/18]"), "{log}");
    assert!(log.contains("[ja=17/17/16]"), "{log}");
    assert!(log.contains("[ko=19]"), "{log}");
    assert!(log.contains("[modifier=20/20]"), "{log}");
    assert!(log.contains("[wide=17/17]"), "{log}");
    assert!(log.contains("[range=15/18]"), "{log}");
}

#[test]
fn 局所代入と大域代入はブロック単位で復元する() {
    let log = run_tex(
        "保存スタック",
        "\\message{[before=\\the\\kcatcode\"3042/\\the\\kcatcode\"3098/\\the\\kcatcode\"3099]}\n\
         {\\kcatcode\"3042=20\n\
          \\message{[outer=\\the\\kcatcode\"3042/\\the\\kcatcode\"3098/\\the\\kcatcode\"3099]}\n\
          {\\kcatcode\"3098=15\n\
           \\message{[inner=\\the\\kcatcode\"3042/\\the\\kcatcode\"3098]}}\n\
          \\message{[restored-inner=\\the\\kcatcode\"3042/\\the\\kcatcode\"3098]}}\n\
         \\message{[after=\\the\\kcatcode\"3042/\\the\\kcatcode\"3098]}\n\
         {\\global\\kcatcode\"3042=16}\n\
         \\message{[global=\\the\\kcatcode\"3042/\\the\\kcatcode\"3098]}\n\
         {\\kcatcode\"3042=15 \\global\\kcatcode\"3098=20}\n\
         \\message{[local-global=\\the\\kcatcode\"3042/\\the\\kcatcode\"3098]}\n\
         {\\global\\kcatcode\"3042=15 \\kcatcode\"3098=19}\n\
         \\message{[global-local=\\the\\kcatcode\"3042/\\the\\kcatcode\"3098]}\n\
         {\\globaldefs=1 \\kcatcode\"3042=19}\n\
         \\message{[globaldefs-positive=\\the\\kcatcode\"3042]}\n\
         {\\globaldefs=-1 \\global\\kcatcode\"3042=20}\n\
         \\message{[globaldefs-negative=\\the\\kcatcode\"3042]}",
    );
    assert!(log.contains("[before=17/17/20]"), "{log}");
    assert!(log.contains("[outer=20/20/20]"), "{log}");
    assert!(log.contains("[inner=15/15]"), "{log}");
    assert!(log.contains("[restored-inner=20/20]"), "{log}");
    assert!(log.contains("[after=17/17]"), "{log}");
    assert!(log.contains("[global=16/16]"), "{log}");
    assert!(log.contains("[local-global=20/20]"), "{log}");
    assert!(log.contains("[global-local=15/15]"), "{log}");
    assert!(log.contains("[globaldefs-positive=19]"), "{log}");
    assert!(log.contains("[globaldefs-negative=19]"), "{log}");
}

#[test]
fn 非連続例外と拡張漢字ブロックは別々の保存単位を持つ() {
    let log = run_tex(
        "例外と拡張漢字",
        "{\\kcatcode\"AA=14\n\
          \\message{[latin-set=\\the\\kcatcode\"AA/\\the\\kcatcode\"BA/\\the\\kcatcode\"C0/\\the\\kcatcode\"FF/\\the\\kcatcode\"A9/\\the\\kcatcode\"D7]}}\n\
         {\\kcatcode\"2CEB0=17\n\
          \\message{[f=\\the\\kcatcode\"2CEB0/\\the\\kcatcode\"2EBEF/\\the\\kcatcode\"2EBF0/\\the\\kcatcode\"2EE60]}\n\
          \\kcatcode\"2EE60=19\n\
          \\message{[i=\\the\\kcatcode\"2EBF0/\\the\\kcatcode\"2EE60/\\the\\kcatcode\"2F7FF/\\the\\kcatcode\"2CEB0]}}",
    );
    assert!(log.contains("[latin-set=14/14/14/14/18/18]"), "{log}");
    assert!(log.contains("[f=17/17/16/16]"), "{log}");
    assert!(log.contains("[i=19/19/19/17]"), "{log}");
}

#[test]
fn 未割当区間は直前の開始境界と保存単位を共有する() {
    let log = run_tex(
        "未割当区間",
        "{\\kcatcode\"10200=15\n\
          \\message{[gap-a=\\the\\kcatcode\"101D0/\\the\\kcatcode\"101FF/\\the\\kcatcode\"10200/\\the\\kcatcode\"1027F/\\the\\kcatcode\"10280]}}\n\
         {\\kcatcode\"12550=19\n\
          \\message{[gap-b=\\the\\kcatcode\"12480/\\the\\kcatcode\"12550/\\the\\kcatcode\"12F8F/\\the\\kcatcode\"12F90]}}\n\
         {\\kcatcode\"108B0=20\n\
          \\message{[gap-c=\\the\\kcatcode\"10880/\\the\\kcatcode\"108B0/\\the\\kcatcode\"108DF/\\the\\kcatcode\"108E0]}}",
    );
    assert!(log.contains("[gap-a=15/15/15/15/18]"), "{log}");
    assert!(log.contains("[gap-b=19/19/19/18]"), "{log}");
    assert!(log.contains("[gap-c=20/20/20/18]"), "{log}");
}

#[test]
fn unicode末尾の擬似境界は別々に代入できる() {
    let log = run_tex(
        "末尾擬似境界",
        "{\\kcatcode\"323B0=15\n\
          \\message{[j=\\the\\kcatcode\"323B0/\\the\\kcatcode\"3347F/\\the\\kcatcode\"33480]}}\n\
         {\\kcatcode\"33480=17\n\
          \\message{[tail-kanji=\\the\\kcatcode\"3347F/\\the\\kcatcode\"33480/\\the\\kcatcode\"3FFFF/\\the\\kcatcode\"40000]}}\n\
         {\\kcatcode\"40000=15\n\
          \\message{[plane=\\the\\kcatcode\"40000/\\the\\kcatcode\"4FFFF/\\the\\kcatcode\"50000]}}\n\
         {\\kcatcode\"E0100=15\n\
          \\message{[vs=\\the\\kcatcode\"E0100/\\the\\kcatcode\"E01EF/\\the\\kcatcode\"E01F0]}}\n\
         {\\kcatcode\"E01F0=19\n\
          \\message{[tail-gap=\\the\\kcatcode\"E01F0/\\the\\kcatcode\"EFFFF/\\the\\kcatcode\"F0000]}}",
    );
    assert!(log.contains("[j=15/15/16]"), "{log}");
    assert!(log.contains("[tail-kanji=16/17/17/18]"), "{log}");
    assert!(log.contains("[plane=15/15/18]"), "{log}");
    assert!(log.contains("[vs=15/15/18]"), "{log}");
    assert!(log.contains("[tail-gap=19/19/18]"), "{log}");
}

#[test]
fn サロゲート整数域は三ブロックとして受理する() {
    let log = run_tex(
        "サロゲート",
        "\\message{[defaults=\\the\\kcatcode\"D7FF/\\the\\kcatcode\"D800/\\the\\kcatcode\"DFFF/\\the\\kcatcode\"E000]}\n\
         {\\kcatcode\"D800=15\n\
          \\message{[high=\\the\\kcatcode\"D800/\\the\\kcatcode\"DB7F/\\the\\kcatcode\"DB80]}\n\
          \\kcatcode\"DB80=19\n\
          \\message{[private=\\the\\kcatcode\"DB80/\\the\\kcatcode\"DBFF/\\the\\kcatcode\"DC00]}\n\
          \\kcatcode\"DC00=20\n\
          \\message{[low=\\the\\kcatcode\"DC00/\\the\\kcatcode\"DFFF/\\the\\kcatcode\"E000]}}",
    );
    assert!(log.contains("[defaults=19/18/18/18]"), "{log}");
    assert!(log.contains("[high=15/15/18]"), "{log}");
    assert!(log.contains("[private=19/19/18]"), "{log}");
    assert!(log.contains("[low=20/20/18]"), "{log}");
}

#[test]
fn 範囲外と不正カテゴリーを診断して処理を続ける() {
    let log = run_tex(
        "診断",
        "\\kcatcode-1=14\n\
         \\kcatcode\"110000=15\n\
         \\kcatcode\"3042=13\n\
         \\message{[bad-low=\\the\\kcatcode\"3042]}\n\
         \\kcatcode\"3042=21\n\
         \\message{[bad-high=\\the\\kcatcode\"3042]}\n\
         \\kcatcode\"10200=14\n\
         \\message{[latin-too-high=\\the\\kcatcode\"10200]}\n\
         \\message{[continued]}",
    );
    assert_eq!(log.matches("Bad Unicode code point").count(), 2, "{log}");
    assert!(
        log.contains("Invalid code (13), should be in the range 15..20"),
        "{log}"
    );
    assert!(
        log.contains("Invalid code (21), should be in the range 15..20"),
        "{log}"
    );
    assert!(
        log.contains("Invalid code (14), should be in the range 15..20"),
        "{log}"
    );
    assert!(log.contains("[bad-low=16]"), "{log}");
    assert!(log.contains("[bad-high=16]"), "{log}");
    assert!(log.contains("[latin-too-high=16]"), "{log}");
    assert!(log.contains("[continued]"), "{log}");
}

#[test]
fn 和文カテゴリーと命令はfmtを往復する() {
    let dir = std::env::temp_dir().join(format!("kcatcode-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fmt = dir.join("mk.fmt");
    let _ = std::fs::remove_file(&fmt);
    let _ = std::fs::remove_file(dir.join("mk.log"));
    let use_log = dir.join("use.log");
    let _ = std::fs::remove_file(&use_log);

    std::fs::write(
        dir.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\kcatcode\"2E00=14\n\
         \\kcatcode\"3042=20\n\
         \\kcatcode\"AA=15\n\
         \\kcatcode\"2CEB0=17\n\
         \\kcatcode\"2EBF0=19\n\
         \\kcatcode\"D800=20\n\
         \\let\\savedkcatcode=\\kcatcode\n\
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
        "\\message{[values=\\the\\savedkcatcode\"2E7F/\\the\\savedkcatcode\"3042/\\the\\savedkcatcode\"BA/\
         \\the\\savedkcatcode\"2CEB0/\\the\\savedkcatcode\"2EE60/\
         \\the\\savedkcatcode\"DB7F]}\n\\end\n",
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
    assert!(log.contains("[values=14/20/15/17/19/20]"), "{log}");
}

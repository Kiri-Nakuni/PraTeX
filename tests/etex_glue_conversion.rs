//! e-TeX の通常糊・数式糊変換（`\mutoglue` `\gluetomu`）。
//!
//! 仕様は公式 e-TeX manual 3.5 の公開記述と e-upTeX の黒箱挙動だけから確かめる。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    let dir = std::env::temp_dir().join(format!(
        "etex-glue-conversion-{}-{:x}",
        std::process::id(),
        h.finish()
    ));
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
fn theは数値を保って目標の糊単位で展開する() {
    let log = run_tex(
        "the",
        "\\message{[A=\\the\\mutoglue 1mu plus 2fil minus 3filll]
         [B=\\the\\gluetomu 4pt plus 5fil minus 6fill]}",
    );
    assert!(
        log.contains("[A=1.0pt plus 2.0fil minus 3.0filll] [B=4.0mu plus 5.0fil minus 6.0fill]"),
        "{log}"
    );
}

#[test]
fn 変換結果は代入と糊式の内部量として使える() {
    let log = run_tex(
        "代入と式",
        "\\muskip0=1.25mu plus 2fil minus 3fill
         \\skip0=\\mutoglue\\muskip0
         \\muskip1=\\gluetomu\\skip0
         \\skip1=\\glueexpr \\mutoglue 2mu plus 3fil + 4pt plus 5fil\\relax
         \\muskip2=\\muexpr \\gluetomu 6pt plus 7fill + 8mu plus 9fill\\relax
         \\message{[\\the\\skip0/\\the\\muskip1]
          [\\the\\skip1/\\the\\muskip2]}",
    );
    assert!(!log.contains("Incompatible glue units"), "{log}");
    assert!(
        log.contains("[1.25pt plus 2.0fil minus 3.0fill/1.25mu plus 2.0fil minus 3.0fill]"),
        "{log}"
    );
    assert!(
        log.contains("[6.0pt plus 8.0fil/14.0mu plus 16.0fill]"),
        "{log}"
    );
}

#[test]
fn meaningは引数を読まず変換命令自身を示す() {
    let log = run_tex(
        "meaning",
        "\\message{[\\meaning\\mutoglue/A][\\meaning\\gluetomu/B]}",
    );
    assert!(log.contains("[\\mutoglue/A][\\gluetomu/B]"), "{log}");
}

#[test]
fn 逆の糊型も診断後に一対一換算して読み進める() {
    let log = run_tex(
        "型不一致",
        "\\skip0=7pt plus 2fil minus 3filll
         \\muskip0=11mu plus 5fill minus 6mu
         \\message{[A=\\the\\mutoglue\\skip0]
          [B=\\the\\gluetomu\\muskip0][C=ok]}",
    );
    assert_eq!(log.matches("Incompatible glue units").count(), 2, "{log}");
    assert!(
        log.contains(
            "[A=7.0pt plus 2.0fil minus 3.0filll] [B=11.0mu plus 5.0fill minus 6.0mu][C=ok]"
        ),
        "{log}"
    );
}

#[test]
fn 糊型変換命令はfmtを往復する() {
    let dir = std::env::temp_dir().join(format!("etex-glue-conversion-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fmt = dir.join("mk.fmt");
    let _ = std::fs::remove_file(&fmt);
    let use_log = dir.join("use.log");
    let _ = std::fs::remove_file(&use_log);

    std::fs::write(
        dir.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\let\\savedmutoglue=\\mutoglue\n\
         \\let\\savedgluetomu=\\gluetomu\n\
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
        "\\muskip0=1mu plus 2fil\n\
         \\skip0=3pt minus 4fill\n\
         \\message{[\\the\\savedmutoglue\\muskip0/\
          \\the\\savedgluetomu\\skip0/\\meaning\\savedmutoglue/\
          \\meaning\\savedgluetomu]}\n\
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
        log.contains("[1.0pt plus 2.0fil/3.0mu minus 4.0fill/\\mutoglue/\\gluetomu]"),
        "{log}"
    );
}

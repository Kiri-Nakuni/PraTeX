//! e-TeXの段落形状照会。
//!
//! 意味は公式e-TeX manual 3.4の公開記述から固定する。実装sourceや上流testは移植しない。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    let dir = std::env::temp_dir().join(format!(
        "etex-parshape-{}-{:x}",
        std::process::id(),
        hasher.finish()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("t.tex");
    let mut file = std::fs::File::create(&source).unwrap();
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
    std::fs::read_to_string(dir.join("t.log"))
        .unwrap()
        .replace('\n', "")
}

#[test]
fn 行番号から字下げと行長を別々に答える() {
    let log = run_tex(
        "字下げと行長",
        "\\parshape3 1pt 2pt 3pt 4pt 5pt 6pt\n\
         \\message{[\\the\\parshapeindent1/\\the\\parshapelength1/\
         \\the\\parshapeindent2/\\the\\parshapelength2/\
         \\the\\parshapeindent3/\\the\\parshapelength3]}",
    );
    assert!(
        log.contains("[1.0pt/2.0pt/3.0pt/4.0pt/5.0pt/6.0pt]"),
        "{log}"
    );
}

#[test]
fn 組番号の奇数を字下げ偶数を行長として答える() {
    let log = run_tex(
        "組番号",
        "\\parshape3 1pt 2pt 3pt 4pt 5pt 6pt\n\
         \\message{[\\the\\parshapedimen1/\\the\\parshapedimen2/\
         \\the\\parshapedimen3/\\the\\parshapedimen4/\
         \\the\\parshapedimen5/\\the\\parshapedimen6]}",
    );
    assert!(
        log.contains("[1.0pt/2.0pt/3.0pt/4.0pt/5.0pt/6.0pt]"),
        "{log}"
    );
}

#[test]
fn 指定行を越えた照会は最後の組を繰り返す() {
    let log = run_tex(
        "末尾反復",
        "\\parshape2 1pt 2pt 3pt 4pt\n\
         \\message{[\\the\\parshapeindent2147483647/\\the\\parshapelength2147483647/\
         \\the\\parshapedimen2147483647/\\the\\parshapedimen2147483646]}",
    );
    assert!(log.contains("[3.0pt/4.0pt/3.0pt/4.0pt]"), "{log}");
}

#[test]
fn 非正の番号と空の段落形状は零寸法を返す() {
    let log = run_tex(
        "零と負数",
        "\\parshape1 1pt 2pt\n\
         \\message{[\\the\\parshapeindent0/\\the\\parshapelength-1/\
         \\the\\parshapedimen0]}\n\
         \\parshape0\n\
         \\message{[\\the\\parshapeindent1/\\the\\parshapelength1/\
         \\the\\parshapedimen1]}",
    );
    assert!(
        log.contains("[0.0pt/0.0pt/0.0pt] [0.0pt/0.0pt/0.0pt]"),
        "{log}"
    );
}

#[test]
fn 段落形状照会は内部寸法として式へ入る() {
    let log = run_tex(
        "内部寸法",
        "\\parshape2 1pt 2pt 3pt 4pt\n\
         \\dimen0=\\dimexpr\\parshapeindent2+\\parshapelength1\\relax\n\
         \\dimen2=\\parshapeindent\\numexpr1+1\\relax\n\
         \\ifdim\\parshapedimen4=4pt\n\
           \\message{[\\the\\dimen0/\\the\\dimen2/YES]}\n\
         \\else\\message{[NO]}\\fi",
    );
    assert!(log.contains("[5.0pt/3.0pt/YES]"), "{log}");
}

#[test]
fn numberは寸法をsp整数として答える() {
    let log = run_tex(
        "numberのsp値",
        "\\parshape1 1pt 4pt\n\
         \\message{[\\number\\parshapeindent1/\\number\\parshapelength1/\
         \\number\\parshapedimen2]}",
    );
    assert!(log.contains("[65536/262144/262144]"), "{log}");
}

#[test]
fn 命令自身は展開せずtheだけが寸法へ展開する() {
    let log = run_tex(
        "展開性",
        "\\parshape1 1pt 2pt
         \\edef\\raw{A\\parshapeindent1B}
         \\edef\\value{A\\the\\parshapeindent1B}
         \\message{[\\meaning\\raw][\\meaning\\value]}",
    );
    assert!(
        log.contains("[macro:->A\\parshapeindent 1B][macro:->A1.0ptB]"),
        "{log}"
    );
}

#[test]
fn 段落形状照会はformatを往復する() {
    let dir = std::env::temp_dir().join(format!("etex-parshape-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let _ = std::fs::remove_file(dir.join("mk.fmt"));
    let _ = std::fs::remove_file(dir.join("use.log"));
    std::fs::write(
        dir.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\let\\savedindent=\\parshapeindent\n\
         \\let\\savedlength=\\parshapelength\n\
         \\let\\saveddimen=\\parshapedimen\n\\dump\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("mk.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(output.status.success() && dir.join("mk.fmt").exists());

    std::fs::write(
        dir.join("use.tex"),
        "\\parshape1 7pt 8pt\n\
         \\message{[\\the\\savedindent1/\\the\\savedlength1/\\the\\saveddimen2/\
         \\meaning\\savedindent/\\meaning\\savedlength/\\meaning\\saveddimen]}\n\\end\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("&mk")
        .arg("use.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(output.status.success() && dir.join("use.log").exists());
    let log = std::fs::read_to_string(dir.join("use.log"))
        .unwrap()
        .replace('\n', "");
    assert!(
        log.contains("[7.0pt/8.0pt/8.0pt/\\parshapeindent/\\parshapelength/\\parshapedimen]"),
        "{log}"
    );
}

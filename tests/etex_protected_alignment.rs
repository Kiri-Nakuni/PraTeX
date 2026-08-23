//! e-TeX の保護macroとalignment先読みの境界。
//!
//! `\omit`と`\noalign`を探す特殊な先読みでは通常macroだけを展開し、
//! 保護macroは行・欄の通常入力として残す。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    let dir = std::env::temp_dir().join(format!(
        "etex-protected-alignment-{}-{:x}",
        std::process::id(),
        hasher.finish()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("test.tex");
    let mut file = std::fs::File::create(&source).unwrap();
    write!(
        file,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\catcode35=6\n\\batchmode\n{body}\n\\end\n"
    )
    .unwrap();
    drop(file);

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("test.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "PraTeX failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(dir.join("test.log"))
        .unwrap()
        .replace('\n', "")
}

#[test]
fn 通常macroのomitは欄先読みで展開する() {
    let log = run_tex(
        "通常omit",
        "\\count0=0
         \\def\\ordinaryomit{\\omit}
         \\setbox0=\\vbox{\\halign{\\global\\advance\\count0 by1 #\\cr
           \\ordinaryomit X\\cr}}
         \\message{[template=\\the\\count0]}",
    );

    assert!(log.contains("[template=0]"), "{log}");
    assert!(!log.contains("Misplaced \\omit"), "{log}");
}

#[test]
fn protected_macroのomitは欄先読みで展開しない() {
    let log = run_tex(
        "保護omit",
        "\\count0=0
         \\protected\\def\\protectedomit{\\omit}
         \\setbox0=\\vbox{\\halign{\\global\\advance\\count0 by1 #\\cr
           \\protectedomit X\\cr}}
         \\message{[template=\\the\\count0]}",
    );

    assert!(log.contains("[template=1]"), "{log}");
    assert!(log.contains("Misplaced \\omit"), "{log}");
}

#[test]
fn 通常macroのnoalignは行先読みで展開する() {
    let log = run_tex(
        "通常noalign",
        "\\count0=0
         \\def\\ordinarynoalign{\\noalign}
         \\setbox0=\\vbox{\\halign{#\\cr
           X\\cr
           \\ordinarynoalign{\\global\\advance\\count0 by1}Y\\cr}}
         \\message{[noalign=\\the\\count0]}",
    );

    assert!(log.contains("[noalign=1]"), "{log}");
    assert!(!log.contains("Misplaced \\noalign"), "{log}");
}

#[test]
fn protected_macroのnoalignは行先読みで展開しない() {
    let log = run_tex(
        "保護noalign",
        "\\protected\\def\\protectednoalign{\\noalign}
         \\setbox0=\\vbox{\\halign{#\\cr
           X\\cr
           \\protectednoalign{}Y\\cr}}
         \\message{[finished]}",
    );

    assert!(log.contains("Misplaced \\noalign"), "{log}");
    assert!(log.contains("[finished]"), "{log}");
}

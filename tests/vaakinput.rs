//! `\vaakinput 名前.vaak` — **ファイルを読んで走らせる。**
//!
//! `\directvaak{…}` は一般テキストを取る（`\directlua` と同じ契約）ので
//! **字句器を通る。** その結果 Vaak のソースとしては壊れる——
//! 改行が空白に潰れ、`%` が TeX の注釈になり、名前空間の印が発火する。
//!
//! **`\vaakinput` は字句器を通さない。**

use std::io::Write;
use std::process::Command;

struct Case {
    dir: std::path::PathBuf,
}

impl Case {
    fn new(name: &str) -> Self {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut h);
        let dir = std::env::temp_dir().join(format!("vi-{}-{:x}", std::process::id(), h.finish()));
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn vaak(&self, name: &str, src: &str) -> &Self {
        std::fs::write(self.dir.join(name), src).unwrap();
        self
    }

    fn run(&self, body: &str) -> String {
        let src = self.dir.join("t.tex");
        let mut f = std::fs::File::create(&src).unwrap();
        write!(f, "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n{body}\n\\end\n").unwrap();
        drop(f);
        Command::new(env!("CARGO_BIN_EXE_rtex"))
            .arg("t.tex")
            .current_dir(&self.dir)
            .output()
            .unwrap();
        std::fs::read_to_string(self.dir.join("t.log")).unwrap()
    }
}

#[test]
fn ファイルを読んで走らせる() {
    let c = Case::new("基本");
    c.vaak("p.vaak", "1 + 2 * 3\n");
    let log = c.run("\\count0=\\vaakinput p.vaak \\message{[\\the\\count0]}");
    assert!(log.contains("[7]"), "{log}");
}

#[test]
fn 拡張子を足す() {
    let c = Case::new("拡張子");
    c.vaak("q.vaak", "41 + 1\n");
    let log = c.run("\\count0=\\vaakinput q \\message{[\\the\\count0]}");
    assert!(log.contains("[42]"), "{log}");
}

#[test]
fn 行注釈が効く() {
    // **`\directvaak` ではここが壊れる。** 改行が空白に潰れるので
    // `%` が残り全部を食う
    let c = Case::new("行注釈");
    c.vaak("p.vaak", "% これは注釈\n1 + 1  % これも\n");
    let log = c.run("\\count0=\\vaakinput p.vaak \\message{[\\the\\count0]}");
    assert!(log.contains("[2]"), "{log}");
}

#[test]
fn 塊注釈が効く() {
    let c = Case::new("塊注釈");
    c.vaak("p.vaak", "%{ 何行に\n   わたっても }%\n20 + 22\n");
    let log = c.run("\\count0=\\vaakinput p.vaak \\message{[\\the\\count0]}");
    assert!(log.contains("[42]"), "{log}");
}

#[test]
fn 改行が残る() {
    let c = Case::new("改行");
    c.vaak(
        "p.vaak",
        "var s := 0;\nnfor (i, 1, 10) {\n    s += i;   % 一行ずつ\n};\ns\n",
    );
    let log = c.run("\\count0=\\vaakinput p.vaak \\message{[\\the\\count0]}");
    assert!(log.contains("[55]"), "{log}");
}

#[test]
fn 名前空間の印が発火しない() {
    // **`\directvaak` なら `3 * 4` が Runaway になる**
    let c = Case::new("名前空間");
    c.vaak("p.vaak", "3 * 4\n");
    let log = c.run("\\catcode`\\*=16 \\count0=\\vaakinput p.vaak \\message{[\\the\\count0]}");
    assert!(!log.contains("Runaway"), "{log}");
    assert!(log.contains("[12]"), "{log}");
}

#[test]
fn レジスタを触れる() {
    let c = Case::new("レジスタ");
    c.vaak("p.vaak", "count[7] := count[5] * 2;\ncount[5]\n");
    let log = c.run(
        "\\count5=21 \\count0=\\vaakinput p.vaak \\message{[\\the\\count0/\\the\\count7]}",
    );
    assert!(log.contains("[21/42]"), "{log}");
}

#[test]
fn 無いファイルは報せて続く() {
    let c = Case::new("無い");
    let log = c.run("\\count0=\\vaakinput nosuch.vaak \\message{[\\the\\count0][done]}");
    assert!(log.contains("Vaak interpreter error"), "{log}");
    assert!(log.contains("[0][done]"), "{log}");
}

#[test]
fn 塊注釈はdirectvaakでは壊れる() {
    // **これが `\vaakinput` の存在理由である。**
    //
    // `%{` は TeX の注釈として行を食い、中身の行は**コードとして字句化され**、
    // 閉じの `}%` の `}` が `\directvaak` の群を**早く閉じる。**
    //
    // 行注釈（`%` から行末）だけは TeX と偶然一致するので通ってしまう——
    // **偶然に頼れない。**
    let c = Case::new("塊注釈の対比");
    let src = "%{ 塊\n   注釈 }%\n1 + 1\n";
    c.vaak("p.vaak", src);

    let good = c.run("\\count0=\\vaakinput p.vaak \\message{[good=\\the\\count0]}");
    assert!(good.contains("[good=2]"), "{good}");

    let bad = c.run(&format!("\\count0=99 \\count0=\\directvaak{{{src}}}\\message{{[bad]}}"));
    assert!(bad.contains("Vaak interpreter error") || bad.contains("Too many"), "{bad}");
}

#[test]
fn 閉じ括弧の直前の注釈もdirectvaakでは壊れる() {
    let c = Case::new("末尾注釈の対比");
    let src = "1 + 1  %";
    c.vaak("p.vaak", src);

    let good = c.run("\\count0=\\vaakinput p.vaak \\message{[good=\\the\\count0]}");
    assert!(good.contains("[good=2]"), "{good}");

    let bad = c.run(&format!("\\count0=\\directvaak{{{src}}}\\message{{[bad]}}"));
    // `%` が閉じ括弧を食うので runaway
    assert!(bad.contains("Runaway"), "{bad}");
}

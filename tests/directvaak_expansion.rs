//! `\directvaak` が外側の展開走査を壊さないことを、rtex 本体で確かめる。

use std::hash::{Hash, Hasher};
use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    let directory = std::env::temp_dir().join(format!(
        "directvaak-expansion-{}-{:x}",
        std::process::id(),
        hash.finish()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let log_path = directory.join("t.log");
    let _ = std::fs::remove_file(&log_path);

    let mut source = std::fs::File::create(directory.join("t.tex")).unwrap();
    write!(
        source,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n{body}\n\\end\n"
    )
    .unwrap();
    drop(source);

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rtex failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    std::fs::read_to_string(log_path).unwrap()
}

#[test]
fn messageの外側で溜めた字句を失わない() {
    let log = run_tex("message", "\\message{[before\\directvaak{ 20 + 22 }after]}");
    assert!(log.contains("[before42after]"), "{log}");
}

#[test]
fn edefの前後と後続の字句を失わない() {
    let log = run_tex(
        "edef",
        "\\edef\\kept{L\\directvaak{ 6 * 7 }R}\n\\message{[\\kept][tail]}",
    );
    assert!(log.contains("[L42R][tail]"), "{log}");
}

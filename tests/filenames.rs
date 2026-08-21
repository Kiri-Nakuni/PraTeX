//! OSのファイル名をUTF-8 `String`と決めつけないprocess境界。

#[cfg(unix)]
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

fn test_directory(name: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};

    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "rtex-filenames-{}-{:x}",
        std::process::id(),
        hash.finish()
    ))
}

fn contains(bytes: &[u8], expected: &[u8]) -> bool {
    bytes
        .windows(expected.len())
        .any(|window| window == expected)
}

#[test]
fn unicodeのinputとopenoutをos文字列のまま扱う() {
    let directory = test_directory("unicode");
    std::fs::create_dir_all(&directory).unwrap();
    for name in ["日本語.log", "日本語.dvi", "出力.dat"] {
        let _ = std::fs::remove_file(directory.join(name));
    }
    std::fs::write(directory.join("補助.tex"), "\\message{[INPUT-OK]}\n").unwrap();
    std::fs::write(
        directory.join("日本語.tex"),
        "\\catcode123=1\n\\catcode125=2\n\
         \\input 補助.tex \
         \\setbox0=\\vbox{\\openout0=出力.dat \\write0{OPENOUT-OK}\\closeout0}\n\
         \\shipout\\box0\n\\end\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("日本語.tex")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let log = std::fs::read(directory.join("日本語.log")).unwrap();
    assert!(contains(&log, b"[INPUT-OK]"));
    let written = std::fs::read(directory.join("出力.dat")).unwrap();
    assert!(contains(&written, b"OPENOUT-OK"));
}

#[cfg(unix)]
#[test]
fn unixの非utf8_cli名を欠落もpanicもさせない() {
    use std::os::unix::ffi::OsStringExt;

    let directory = test_directory("non-utf8");
    std::fs::create_dir_all(&directory).unwrap();
    let tex_name = OsString::from_vec(vec![b'n', 0xff, b'.', b't', b'e', b'x']);
    let log_name = OsString::from_vec(vec![b'n', 0xff, b'.', b'l', b'o', b'g']);
    let dvi_name = OsString::from_vec(vec![b'n', 0xff, b'.', b'd', b'v', b'i']);
    let _ = std::fs::remove_file(directory.join(&log_name));
    let _ = std::fs::remove_file(directory.join(dvi_name));
    std::fs::write(
        directory.join(&tex_name),
        b"\\catcode123=1\n\\catcode125=2\n\\batchmode\n\\message{[RAW-NAME-OK]}\\end\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg(&tex_name)
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(output.status.success());
    let log = std::fs::read(directory.join(log_name)).unwrap();
    assert!(contains(&log, b"[RAW-NAME-OK]"));
}

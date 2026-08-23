//! e-TeX の TeX--XeT state と format の境界。

use std::process::Command;

#[test]
fn texxetstateはfmt読込時に零へ戻り他の整数は残る() {
    let dir = std::env::temp_dir().join(format!("etex-texxet-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fmt = dir.join("mk.fmt");
    let use_log = dir.join("use.log");
    for path in [&fmt, &dir.join("mk.log"), &use_log] {
        let _ = std::fs::remove_file(path);
    }

    std::fs::write(
        dir.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\TeXXeTstate=7\n\
         \\tolerance=4321\n\
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
        "\\message{[texxet=\\the\\TeXXeTstate/tolerance=\\the\\tolerance]}\n\
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

    let log = std::fs::read_to_string(use_log).unwrap().replace('\n', "");
    assert!(log.contains("[texxet=0/tolerance=4321]"), "{log}");
}

//! 未展開tokenだけを読む経路の意味境界。

use std::io::Write;
use std::process::Command;

#[test]
fn マクロ引数の制御綴を展開せずnoexpandも保持する() {
    let directory = std::env::temp_dir().join(format!("token-only-input-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let input = directory.join("token-only.tex");
    let mut file = std::fs::File::create(&input).unwrap();
    write!(
        file,
        "\\catcode`\\{{=1\n\
         \\catcode`\\}}=2\n\
         \\catcode`\\#=6\n\
         \\batchmode\n\
         \\count0=0\n\
         \\def\\a{{\\advance\\count0 by1}}\n\
         \\def\\drop#1{{}}\n\
         \\drop{{\\a\\a}}\n\
         \\edef\\kept{{\\noexpand\\a}}\n\
         \\def\\expected{{\\a}}\n\
         \\ifx\\kept\\expected \\def\\same{{Y}}\\else \\def\\same{{N}}\\fi\n\
         \\message{{[count=\\the\\count0][noexpand=\\same]}}\n\
         \\end\n"
    )
    .unwrap();
    drop(file);

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(["-ini", "-halt-on-error", "token-only.tex"])
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "未展開token入力を処理できなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let log = std::fs::read_to_string(directory.join("token-only.log")).unwrap();
    assert!(log.contains("[count=0][noexpand=Y]"), "{log}");
}

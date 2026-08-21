//! standalone PDF出力のprocess境界。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn test_directory(name: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};

    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "rtex-pdf-output-{}-{:x}",
        std::process::id(),
        hash.finish()
    ))
}

fn run_tex(name: &str, options: &[&str], body: &str) -> (PathBuf, Output) {
    let directory = test_directory(name);
    std::fs::create_dir_all(&directory).unwrap();
    for extension in ["aux", "dvi", "log", "pdf"] {
        let _ = std::fs::remove_file(directory.join(format!("t.{extension}")));
    }
    std::fs::write(
        directory.join("t.tex"),
        format!("\\catcode123=1\n\\catcode125=2\n\\batchmode\n{body}\n\\end\n"),
    )
    .unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_rtex"));
    command.args(options).arg("t.tex").current_dir(&directory);
    let output = command.output().unwrap();
    (directory, output)
}

fn contains(bytes: &[u8], expected: &[u8]) -> bool {
    bytes
        .windows(expected.len())
        .any(|window| window == expected)
}

fn count(bytes: &[u8], expected: &[u8]) -> usize {
    bytes
        .windows(expected.len())
        .filter(|window| *window == expected)
        .count()
}

fn assert_success(output: &Output, directory: &Path) {
    assert!(
        output.status.success(),
        "TeX実行失敗 ({directory:?}):\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn synthetic_pfb() -> Vec<u8> {
    fn segment(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0x80, kind];
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    let mut bytes = segment(1, b"%!PS synthetic CLI font\n");
    bytes.extend_from_slice(&segment(2, &[1, 2, 3, 4]));
    bytes.extend_from_slice(&segment(1, b"cleartomark\n"));
    bytes.extend_from_slice(&[0x80, 3]);
    bytes
}

fn synthetic_afm() -> &'static [u8] {
    b"StartFontMetrics 4.1\n\
FontName CliSynthetic\n\
EncodingScheme FontSpecific\n\
FontBBox -40 -250 1000 750\n\
ItalicAngle 0\n\
IsFixedPitch false\n\
CapHeight 680\n\
XHeight 430\n\
Ascender 700\n\
Descender -200\n\
StdVW 80\n\
StartCharMetrics 1\n\
C 65 ; WX 1000 ; N A ;\n\
EndCharMetrics\n\
EndFontMetrics\n"
}

fn synthetic_single_a_tfm() -> Vec<u8> {
    let mut bytes = Vec::new();
    // 6 size words, 2 header words, 1 char-info, 2 widths, 3 zero dimensions, 7 params.
    for value in [21_u16, 2, 65, 65, 2, 1, 1, 1, 0, 0, 0, 7] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    // Design size 10pt in TFM's 12.20 fixed-point representation.
    bytes.extend_from_slice(&[0x00, 0xa0, 0x00, 0x00]);
    // A exists and selects width table entry 1.
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&[0, 16, 0, 0]);
    bytes.extend_from_slice(&[0; 3 * 4]);
    bytes.extend_from_slice(&[0; 7 * 4]);
    assert_eq!(bytes.len(), 21 * 4);
    bytes
}

fn write_synthetic_font(directory: &Path) {
    std::fs::write(directory.join("synthetic.tfm"), synthetic_single_a_tfm()).unwrap();
    std::fs::write(directory.join("synthetic.pfb"), synthetic_pfb()).unwrap();
    std::fs::write(directory.join("synthetic.afm"), synthetic_afm()).unwrap();
}

fn assert_not_panicked(output: &Output) {
    let mut diagnostic = output.stdout.clone();
    diagnostic.extend_from_slice(&output.stderr);
    assert!(!contains(&diagnostic, b"panicked at"));
    assert!(!contains(&diagnostic, b"thread 'main' panicked"));
}

/// TeXの記録は79桁で折れるため、資材名や診断を照合する前に改行を除く。
fn joined_log(path: &Path) -> Vec<u8> {
    std::fs::read(path)
        .unwrap()
        .into_iter()
        .filter(|byte| !matches!(byte, b'\r' | b'\n'))
        .collect()
}

#[test]
fn pdfを直接二page書きruleだけをcontentへ入れる() {
    let (directory, output) = run_tex(
        "二ページ",
        &["--output-format=pdf"],
        "\\setbox0=\\hbox{\\special{RAW-SPECIAL-MUST-NOT-APPEAR}%\n\
         \\vrule width20pt height10pt depth0pt}\n\
         \\shipout\\box0\n\
         \\setbox0=\\hbox{\\vrule width1pt height2pt depth0pt}\n\
         \\shipout\\box0",
    );
    assert_success(&output, &directory);

    let pdf_path = directory.join("t.pdf");
    assert!(pdf_path.is_file());
    assert!(!directory.join("t.dvi").exists());
    let pdf = std::fs::read(&pdf_path).unwrap();
    assert_eq!(
        pdf.len() as u64,
        std::fs::metadata(&pdf_path).unwrap().len()
    );
    assert!(pdf.starts_with(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n"));
    assert!(contains(&pdf, b"/Type /Catalog"));
    assert!(contains(&pdf, b"/Count 2"));
    assert!(contains(&pdf, b"/BaseFont /Courier"));
    assert_eq!(count(&pdf, b" re f\n"), 2);
    assert!(!contains(&pdf, b"RAW-SPECIAL-MUST-NOT-APPEAR"));
    let log = joined_log(&directory.join("t.log"));
    assert!(contains(&log, b"Output written on t.pdf"));
}

#[test]
fn 明示したfull_mapだけがtype1をstandalone_pdfへ埋め込む() {
    let directory = test_directory("CLI Type1埋込み");
    std::fs::create_dir_all(&directory).unwrap();
    for extension in ["dvi", "log", "pdf"] {
        let _ = std::fs::remove_file(directory.join(format!("t.{extension}")));
    }
    write_synthetic_font(&directory);
    let map_path = directory.join("埋込み.map");
    std::fs::write(&map_path, b"synthetic CliSynthetic 6 <<synthetic.pfb\n").unwrap();
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\font\\embedded=synthetic\n\
         \\setbox0=\\hbox{\\embedded A}\n\
         \\shipout\\box0\n\\end\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("--output-format=pdf")
        .arg("--pdf-font-map")
        .arg(&map_path)
        .arg("t.tex")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert_success(&output, &directory);

    let pdf = std::fs::read(directory.join("t.pdf")).unwrap();
    assert!(contains(&pdf, b"/BaseFont /CliSynthetic"));
    assert!(contains(&pdf, b"/FontFile "));
    assert!(contains(&pdf, b"/Length1 "));
    assert!(contains(&pdf, b"%!PS synthetic CLI font\n"));
    assert!(contains(&pdf, b"/F2 "));
    assert!(contains(&pdf, b"<41> Tj"));
    assert!(!directory.join("t.dvi").exists());
}

#[test]
fn 存在しないfont_mapはpanicせず原因を残して終了する() {
    let (directory, output) = run_tex(
        "missing font map",
        &[
            "--output-format=pdf",
            "--pdf-font-map=rtex-definitely-missing-font-map.map",
        ],
        "\\shipout\\hbox{\\vrule width1pt height1pt}",
    );

    assert!(!output.status.success());
    assert_not_panicked(&output);
    let log = joined_log(&directory.join("t.log"));
    assert!(contains(&log, b"PDF font map initialization failed"));
    assert!(contains(&log, b"rtex-definitely-missing-font-map.map"));
}

#[test]
fn 壊れたfont_mapはpanicせずparse位置を残して終了する() {
    let directory = test_directory("broken font map");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("broken.map"), b"<<not-a-tfm-name\n").unwrap();
    let (directory, output) = run_tex(
        "broken font map",
        &["--output-format=pdf", "--pdf-font-map=broken.map"],
        "\\shipout\\hbox{\\vrule width1pt height1pt}",
    );

    assert!(!output.status.success());
    assert_not_panicked(&output);
    let log = joined_log(&directory.join("t.log"));
    assert!(contains(&log, b"PDF font map initialization failed"));
    assert!(contains(&log, b"line 1"));
}

#[test]
fn 欠けたtype1資材はshipoutからpanicせず原因を残して終了する() {
    let directory = test_directory("missing Type1 resource");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("synthetic.tfm"), synthetic_single_a_tfm()).unwrap();
    std::fs::write(
        directory.join("missing-resource.map"),
        b"synthetic CliSynthetic 6 <<missing-resource.pfb\n",
    )
    .unwrap();
    let (directory, output) = run_tex(
        "missing Type1 resource",
        &["--output-format=pdf", "--pdf-font-map=missing-resource.map"],
        "\\font\\embedded=synthetic\n\
         \\setbox0=\\hbox{\\embedded A}\n\
         \\shipout\\box0",
    );

    assert!(!output.status.success());
    assert_not_panicked(&output);
    let log = joined_log(&directory.join("t.log"));
    assert!(contains(&log, b"PDF output failed"));
    assert!(contains(&log, b"missing-resource.pfb"));
}

#[test]
fn 既定のdvi出力を変えない() {
    let (directory, output) = run_tex(
        "既定DVI",
        &[],
        "\\setbox0=\\hbox{\\vrule width2pt height3pt depth0pt}\n\\shipout\\box0",
    );
    assert_success(&output, &directory);
    let dvi = std::fs::read(directory.join("t.dvi")).unwrap();
    assert_eq!(dvi.first(), Some(&247));
    assert!(!directory.join("t.pdf").exists());
    let log = std::fs::read(directory.join("t.log")).unwrap();
    assert!(contains(&log, b"Output written on t.dvi"));
}

#[test]
fn pageが無ければpdfを作らない() {
    let (directory, output) = run_tex("ページ無し", &["-output-format=pdf"], "");
    assert_success(&output, &directory);
    assert!(!directory.join("t.pdf").exists());
    let log = std::fs::read(directory.join("t.log")).unwrap();
    assert!(contains(&log, b"No pages of output."));
}

#[test]
fn 不明な出力形式をtex入力より前に拒む() {
    let (directory, output) = run_tex("不明形式", &["--output-format=xps"], "");
    assert!(!output.status.success());
    assert!(contains(&output.stderr, b"unknown output format `xps`"));
    assert!(!directory.join("t.log").exists());
    assert!(!directory.join("t.pdf").exists());
    assert!(!directory.join("t.dvi").exists());
}

#[test]
fn 負幅のshipoutでも見えるruleをmedia_boxから切らない() {
    let (directory, output) = run_tex(
        "負幅",
        &["--output-format=pdf"],
        "\\setbox0=\\hbox to -1pt{\\vrule width200pt height10pt\\hss}\\shipout\\box0",
    );
    assert_success(&output, &directory);
    let pdf = std::fs::read(directory.join("t.pdf")).unwrap();

    assert!(contains(&pdf, b"/MediaBox [0 0 343.252802 153.96264]"));
    assert!(contains(&pdf, b"72 72 199.252802 9.96264 re f"));
}

#[test]
fn command_line入力でも不正なmagをlogへ残す() {
    let directory = test_directory("mag診断");
    std::fs::create_dir_all(&directory).unwrap();
    for extension in ["dvi", "log", "pdf"] {
        let _ = std::fs::remove_file(directory.join(format!("texput.{extension}")));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("\\batchmode\\mag=0\\shipout\\hbox{}\\end")
        .current_dir(&directory)
        .output()
        .unwrap();
    // rtexは現状、回復できたTeX errorをprocess statusへ反映しない。ここではtranscriptだけを固定する。
    assert!(output.status.success());
    let log = std::fs::read(directory.join("texput.log")).unwrap();
    assert!(contains(
        &log,
        b"Illegal magnification has been changed to 1000"
    ));
}

#[test]
fn crlfで答えた出力先に復帰文字を混ぜない() {
    let directory = test_directory("CRLF出力prompt");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("prompt.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\shipout\\hbox{\\vrule width1pt height1pt}\\end\n",
    )
    .unwrap();
    // 既定名をdirectoryで塞ぎ、別名を尋ねる経路へ入れる。
    std::fs::create_dir_all(directory.join("prompt.pdf")).unwrap();
    let actual = directory.join("actual.pdf");
    let _ = std::fs::remove_file(&actual);

    let mut child = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(["--output-format=pdf", "prompt.tex"])
        .current_dir(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"actual.pdf\r\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_success(&output, &directory);

    assert!(actual.is_file());
    let log = std::fs::read(directory.join("prompt.log")).unwrap();
    assert!(contains(&log, b"Output written on actual.pdf"));
    assert!(!contains(&log, b"actual.pdf^^M"));
}

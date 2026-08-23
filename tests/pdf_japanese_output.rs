//! 合成JFMのwide glyphを、明示named CID profileからPDFへ通すprocess試験。
//!
//! JFMは公開file formatだけから組み立て、字形fileは同梱しない。生成PDFの実表示は
//! `HeiseiMin-W3`と`UniJIS-UCS2-H`を解決できるviewer環境に依存する。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ZERO: i32 = 0;
const QUARTER: i32 = 0x0004_0000;
const HALF: i32 = 0x0008_0000;
const ONE: i32 = 0x0010_0000;

fn char_type(character_code: u32, character_type: u8) -> [u8; 4] {
    [
        ((character_code >> 8) & 0xff) as u8,
        (character_code & 0xff) as u8,
        ((character_code >> 16) & 0xff) as u8,
        character_type,
    ]
}

fn synthetic_jfm() -> Vec<u8> {
    let char_types = [char_type(0, 0), char_type(0x003042, 1)];
    let char_infos = [[1, 0x11, 0, 0], [2, 0x10, 0, 0]];
    let widths = [ZERO, ONE, HALF];
    let heights = [ZERO, HALF];
    let depths = [ZERO, QUARTER];
    let italics = [ZERO];
    let params = [ZERO; 9];

    let nt = char_types.len();
    let lh = 2;
    let bc = 0;
    let ec = char_infos.len() - 1;
    let lf = 7
        + nt
        + lh
        + (ec - bc + 1)
        + widths.len()
        + heights.len()
        + depths.len()
        + italics.len()
        + params.len();
    let mut bytes = Vec::with_capacity(lf * 4);
    for value in [
        11,
        nt as u16,
        lf as u16,
        lh as u16,
        bc as u16,
        ec as u16,
        widths.len() as u16,
        heights.len() as u16,
        depths.len() as u16,
        italics.len() as u16,
        0,
        0,
        0,
        params.len() as u16,
    ] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0x1234_5678_u32.to_be_bytes());
    bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());
    for table in [&char_types[..], &char_infos[..]] {
        for word in table {
            bytes.extend_from_slice(word);
        }
    }
    for table in [
        &widths[..],
        &heights[..],
        &depths[..],
        &italics[..],
        &params[..],
    ] {
        for value in table {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
    bytes
}

fn prepare_directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    let directory = std::env::temp_dir().join(format!(
        "pratex-pdf-japanese-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for name in [
        "t.tex",
        "t.log",
        "t.pdf",
        "synthetic.tfm",
        "upjisr-h.tfm",
        "min10.cidprofile",
    ] {
        let _ = std::fs::remove_file(directory.join(name));
    }
    std::fs::write(directory.join("synthetic.tfm"), synthetic_jfm()).unwrap();
    std::fs::write(directory.join("upjisr-h.tfm"), synthetic_jfm()).unwrap();
    std::fs::write(
        directory.join("min10.cidprofile"),
        b"PraTeX-Named-CID-Profile 1\n\
JfmName synthetic\n\
BaseFont HeiseiMin-W3\n\
Flags 6\n\
FontBBox -123 -257 1001 910\n\
ItalicAngle 0\n\
Ascent 880\n\
Descent -120\n\
CapHeight 700\n\
StemV 80\n\
DefaultWidth 1000\n\
EndProfile\n",
    )
    .unwrap();
    directory
}

fn write_input(directory: &Path, jfm_name: &str) {
    std::fs::write(
        directory.join("t.tex"),
        format!(
            "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
             \\pratexjfont\\J={jfm_name} at 10pt\n\\J\n\
             \\kcatcode\"3042=16\n\
             \\setbox0=\\hbox{{ああ}}\n\\shipout\\box0\n\\end\n"
        ),
    )
    .unwrap();
}

fn run_rtex(directory: &Path, with_profile: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rtex"));
    command.arg("--output-format=pdf");
    if with_profile {
        command
            .arg("--pdf-japanese-cid-profile")
            .arg(directory.join("min10.cidprofile"));
    }
    command
        .arg("t.tex")
        .current_dir(directory)
        .output()
        .unwrap()
}

fn occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

#[test]
fn 合成jfmの二文字をnamed_cid_pdfへ一続きで出す() {
    let directory = prepare_directory("wide-success");
    write_input(&directory, "synthetic");
    let output = run_rtex(&directory, true);
    assert!(
        output.status.success(),
        "和文PDFを出せなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let pdf = std::fs::read(directory.join("t.pdf")).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(pdf.starts_with(b"%PDF-1.4\n"));
    for required in [
        "/Subtype /Type0",
        "/Subtype /CIDFontType0",
        "/BaseFont /HeiseiMin-W3-UniJIS-UCS2-H",
        "/Encoding /UniJIS-UCS2-H",
        "/CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 4 >>",
        "/Font <<\n/F1 3 0 R\n/F2 6 0 R\n>>",
        "1 0 0 1 72 ",
        "1 0 0 1 76.98132 ",
        "<3042> Tj",
    ] {
        assert!(text.contains(required), "PDFに `{required}` がない");
    }
    assert_eq!(occurrences(&pdf, b"/Subtype /Type0"), 1);
    assert_eq!(occurrences(&pdf, b"/DescendantFonts ["), 1);
    assert_eq!(occurrences(&pdf, b"<3042> Tj"), 2);
    assert!(!text.contains("/W ["));
    assert!(!text.contains("/FontFile"));
}

#[test]
fn 既定upjisr_hは追加指定なしで和文pdfを出す() {
    let directory = prepare_directory("built-in-upjisr-h");
    write_input(&directory, "upjisr-h");
    let output = run_rtex(&directory, false);
    assert!(
        output.status.success(),
        "既定profileで和文PDFを出せなかった: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pdf = std::fs::read(directory.join("t.pdf")).unwrap();
    assert!(pdf
        .windows(b"/BaseFont /HeiseiMin-W3-UniJIS-UCS2-H".len())
        .any(|window| window == b"/BaseFont /HeiseiMin-W3-UniJIS-UCS2-H"));
    assert_eq!(occurrences(&pdf, b"<3042> Tj"), 2);
}

#[test]
fn 内蔵profileのないjfmはtofuへ落とさず原因を先に診断する() {
    let directory = prepare_directory("missing-profile");
    write_input(&directory, "synthetic");
    let output = run_rtex(&directory, false);
    assert!(!output.status.success());
    let transcript = format!(
        "{}{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        std::fs::read_to_string(directory.join("t.log")).unwrap_or_default()
    )
    .replace(['\r', '\n'], "");
    assert!(
        transcript.contains("JFM `synthetic` has no built-in named CID profile"),
        "{transcript}"
    );
    assert!(transcript.contains("! PDF output failed:"), "{transcript}");
    assert!(!transcript.contains("! Emergency stop"), "{transcript}");
}

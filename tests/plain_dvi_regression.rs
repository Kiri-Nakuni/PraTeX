//! `origin/main`のTeX82経路と、plain欧文DVIのpage bodyをbyte単位で比較する。
//!
//! 配布fontやTeX Liveへ依存しないよう、公開TFM形式だけから合成した等幅fontを使う。
//! DVI preambleのengine名・実行時刻とpostambleのfile pointerは比較せず、最後の
//! BOPからEOPまでを固定するため、glyph opcodeとsp座標の退行は隠れない。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FIRST_CHARACTER: u16 = 46;
const LAST_CHARACTER: u16 = 90;

// `origin/main` f174f44で同じ合成TFMと入力から得た値。
const MAIN_PAGE_BODY: &[u8] = &[
    139, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 141, 243, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0, 10,
    0, 0, 0, 9, 115, 121, 110, 116, 104, 101, 116, 105, 99, 171, 84, 72, 69, 142, 141, 81, 85, 73,
    67, 75, 142, 141, 66, 82, 79, 87, 78, 142, 141, 70, 79, 88, 142, 141, 74, 85, 77, 80, 83, 142,
    141, 79, 86, 69, 82, 142, 141, 84, 72, 69, 142, 141, 76, 65, 90, 89, 142, 141, 68, 79, 71, 46,
    142, 141, 84, 72, 69, 142, 141, 81, 85, 73, 67, 75, 145, 2, 56, 156, 66, 82, 79, 87, 78, 142,
    141, 70, 79, 88, 145, 2, 56, 156, 74, 85, 77, 80, 83, 46, 142, 140,
];

fn synthetic_uppercase_tfm() -> Vec<u8> {
    let character_count = LAST_CHARACTER - FIRST_CHARACTER + 1;
    let header_words = 2_u16;
    let width_words = 2_u16;
    let height_words = 1_u16;
    let depth_words = 1_u16;
    let italic_words = 1_u16;
    let parameter_words = 7_u16;
    let file_words = 6
        + header_words
        + character_count
        + width_words
        + height_words
        + depth_words
        + italic_words
        + parameter_words;

    let mut bytes = Vec::with_capacity(usize::from(file_words) * 4);
    for value in [
        file_words,
        header_words,
        FIRST_CHARACTER,
        LAST_CHARACTER,
        width_words,
        height_words,
        depth_words,
        italic_words,
        0,
        0,
        0,
        parameter_words,
    ] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());

    // 全文字を幅table 1（0.5em）へ結ぶ。
    for _ in 0..character_count {
        bytes.extend_from_slice(&[1, 0, 0, 0]);
    }
    for value in [0_i32, 0x0008_0000] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_i32.to_be_bytes()); // height
    bytes.extend_from_slice(&0_i32.to_be_bytes()); // depth
    bytes.extend_from_slice(&0_i32.to_be_bytes()); // italic

    // slant, space, stretch, shrink, x-height, quad, extra-space。
    for value in [
        0_i32,
        0x0005_5555,
        0x0002_AAAA,
        0x0001_C71C,
        0x0007_0000,
        0x0010_0000,
        0x0001_0000,
    ] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    assert_eq!(bytes.len(), usize::from(file_words) * 4);
    bytes
}

fn test_directory() -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    module_path!().hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-plain-dvi-regression-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ))
}

fn prepare_fixture() -> PathBuf {
    let directory = test_directory();
    std::fs::create_dir_all(&directory).unwrap();
    for name in ["plain.tex", "plain.log", "plain.dvi", "synthetic.tfm"] {
        let _ = std::fs::remove_file(directory.join(name));
    }
    std::fs::write(directory.join("synthetic.tfm"), synthetic_uppercase_tfm()).unwrap();
    std::fs::write(
        directory.join("plain.tex"),
        b"\\catcode`\\{=1 \\catcode`\\}=2\n\
          \\font\\f=synthetic \\f \\hsize=36pt \\parindent=0pt \\tolerance=10000\n\
          \\shipout\\vbox{THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG. THE QUICK BROWN FOX JUMPS.\\par}\n\
          \\end\n",
    )
    .unwrap();
    directory
}

fn run_current(directory: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(["--quiet", "--", "plain.tex"])
        .current_dir(directory)
        .output()
        .unwrap()
}

fn final_page_body(dvi: &[u8]) -> &[u8] {
    let post_post = dvi
        .iter()
        .rposition(|&byte| byte == 249)
        .expect("DVI post_postがない");
    assert!(post_post + 5 < dvi.len(), "DVI post_postが途中で切れている");
    let post = u32::from_be_bytes(dvi[post_post + 1..post_post + 5].try_into().unwrap()) as usize;
    assert_eq!(
        dvi.get(post),
        Some(&248),
        "DVI post pointerがPOSTを指さない"
    );
    assert!(post >= 5, "DVI POSTが短すぎる");
    let bop = u32::from_be_bytes(dvi[post + 1..post + 5].try_into().unwrap()) as usize;
    assert_eq!(
        dvi.get(bop),
        Some(&139),
        "DVI final BOP pointerがBOPを指さない"
    );
    assert_eq!(dvi.get(post - 1), Some(&140), "最後のpageがEOPで閉じない");
    &dvi[bop..post]
}

#[test]
fn plain欧文のbopからeopまでorigin_mainとbyte一致する() {
    let directory = prepare_fixture();
    let output = run_current(&directory);
    assert!(
        output.status.success(),
        "PraTeX実行失敗:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let dvi = std::fs::read(directory.join("plain.dvi")).unwrap();
    let page = final_page_body(&dvi);
    assert_eq!(
        page, MAIN_PAGE_BODY,
        "origin/main page bodyとの不一致。fixture={directory:?}; actual={page:?}"
    );
}

//! `pratex-japanese` がNFSSのsizeを横組JFMへ渡す境界の回帰。
//!
//! LaTeX本体はblack boxのlive gateで別途確認する。この試験は公開JFM形式から作った
//! metricと最小のLaTeX hook面だけを使い、package側のcache・群・DVI font定義を固定する。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ZERO: i32 = 0;
const QUARTER: i32 = 0x0004_0000;
const HALF: i32 = 0x0008_0000;
const ONE: i32 = 0x0010_0000;

fn synthetic_jfm() -> Vec<u8> {
    let char_types = [[0, 0, 0, 0], [0x30, 0x42, 0, 1]];
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

fn prepare_directory() -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "pratex-japanese-nfss-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for name in [
        "t.tex",
        "t.log",
        "t.dvi",
        "upjisr-h.tfm",
        "pratex-japanese.sty",
    ] {
        let _ = std::fs::remove_file(directory.join(name));
    }
    std::fs::write(directory.join("upjisr-h.tfm"), synthetic_jfm()).unwrap();
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tex/latex/pratex/pratex-japanese.sty"),
        directory.join("pratex-japanese.sty"),
    )
    .unwrap();
    directory
}

fn run_rtex(directory: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(directory)
        .output()
        .unwrap()
}

fn read_u32(bytes: &[u8], start: usize) -> u32 {
    u32::from_be_bytes(bytes[start..start + 4].try_into().unwrap())
}

fn japanese_font_scales(dvi: &[u8]) -> Vec<u32> {
    let id = dvi
        .iter()
        .rposition(|byte| *byte != 223)
        .expect("DVI末尾にID byteがあること");
    let post_post = id.checked_sub(5).expect("post_post recordがあること");
    assert_eq!(dvi[post_post], 249, "DVI末尾がpost_postであること");
    let post = read_u32(dvi, post_post + 1) as usize;
    assert_eq!(dvi[post], 248, "postamble pointerがpostを指すこと");

    let mut position = post + 29;
    let mut scales = Vec::new();
    while position < post_post {
        let opcode = dvi[position];
        position += 1;
        match opcode {
            138 => {}
            243..=246 => {
                position += usize::from(opcode - 242);
                position += 4; // checksum
                let scale = read_u32(dvi, position);
                position += 8; // scale and design size
                let area = usize::from(dvi[position]);
                let name = usize::from(dvi[position + 1]);
                position += 2;
                let font_name = &dvi[position + area..position + area + name];
                position += area + name;
                if font_name == b"upjisr-h" {
                    scales.push(scale);
                }
            }
            _ => panic!("postamble中の未対応opcode {opcode}"),
        }
    }
    scales.sort_unstable();
    scales
}

#[test]
fn 同じnfss寸法はspのcache_keyを共有する() {
    let package = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tex/latex/pratex/pratex-japanese.sty"),
    )
    .unwrap();
    assert!(package.contains(r"\number\dimexpr\f@size pt\relax"));
    assert!(package.contains(r"\ifcsname pratex@jfont@\pratex@jfontsizekey\endcsname"));
    assert!(package.contains(r"\expandafter\global\expandafter\pratexjfont"));
    assert!(package.contains(r"\AddToHook{cmd/selectfont/after}"));
}

#[test]
fn nfssの大小と群復元をjfm寸法とdviへ渡す() {
    let directory = prepare_directory();
    std::fs::write(
        directory.join("t.tex"),
        r#"\catcode123=1
\catcode125=2
\catcode35=6
\catcode`\@=11
\def\NeedsTeXFormat#1[#2]{}
\def\ProvidesPackage#1[#2]{}
\def\PackageError#1#2#3{\errmessage{#1: #2}}
\long\def\newcommand#1#2{\long\def#1{#2}}
\long\def\AtBeginDocument#1{\gdef\pratexdocumenthook{#1}}
\long\def\AddToHook#1#2{\gdef\pratexselectfonthook{#2}}
\def\f@size{10}
\input pratex-japanese.sty
\pratexdocumenthook
\kcatcode"3042=16
\global\setbox0=\hbox{あ}
{\def\f@size{9}\pratexselectfonthook
 \global\setbox1=\hbox{あ}
 \def\f@size{9.0}\pratexselectfonthook
 \global\setbox2=\hbox{あ}}
\global\setbox3=\hbox{あ}
{\def\f@size{14.4}\pratexselectfonthook
 \global\setbox4=\hbox{あ}}
\message{[JFM-NFSS=\the\wd0/\the\wd1/\the\wd2/\the\wd3/\the\wd4]}
\shipout\vbox{\box0\box1\box2\box3\box4}
\end
"#,
    )
    .unwrap();

    let output = run_rtex(&directory);
    assert!(
        output.status.success(),
        "NFSS/JFM試験を実行できない: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = std::fs::read_to_string(directory.join("t.log"))
        .unwrap()
        .replace('\n', "");
    assert!(
        log.contains("[JFM-NFSS=5.0pt/4.5pt/4.5pt/5.0pt/7.2pt]"),
        "NFSS sizeまたは群復元がJFMへ届かない: {log}"
    );
    assert_eq!(
        japanese_font_scales(&std::fs::read(directory.join("t.dvi")).unwrap()),
        [589_824, 655_360, 943_718],
        "9と9.0は同じJFMを使い、10ptへ群復元し、14.4ptを一度だけ定義する"
    );
}

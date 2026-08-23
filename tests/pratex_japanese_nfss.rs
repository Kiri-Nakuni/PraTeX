//! `pratex-japanese` の和文NFSS属性・JFM cache・従属欧文を固定する回帰。
//!
//! LaTeX本体はblack boxのlive gateで別途確認する。この試験は公開JFM形式から作った
//! metricと最小のLaTeX hook面だけを使い、package固有の宣言・群・DVI font定義を測る。

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

fn prepare_directory(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "pratex-japanese-nfss-{}-{label}",
        std::process::id(),
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for name in [
        "t.tex",
        "t.log",
        "t.dvi",
        "upjisr-h.tfm",
        "upjisg-h.tfm",
        "pratex-japanese.sty",
    ] {
        let _ = std::fs::remove_file(directory.join(name));
    }
    let jfm = synthetic_jfm();
    std::fs::write(directory.join("upjisr-h.tfm"), &jfm).unwrap();
    std::fs::write(directory.join("upjisg-h.tfm"), &jfm).unwrap();
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tex/latex/pratex/pratex-japanese.sty"),
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

fn japanese_font_definitions(dvi: &[u8]) -> Vec<(String, u32)> {
    let id = dvi
        .iter()
        .rposition(|byte| *byte != 223)
        .expect("DVI末尾にID byteがあること");
    let post_post = id.checked_sub(5).expect("post_post recordがあること");
    assert_eq!(dvi[post_post], 249, "DVI末尾がpost_postであること");
    let post = read_u32(dvi, post_post + 1) as usize;
    assert_eq!(dvi[post], 248, "postamble pointerがpostを指すこと");

    let mut position = post + 29;
    let mut definitions = Vec::new();
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
                if font_name.starts_with(b"upjis") {
                    definitions.push((String::from_utf8(font_name.to_vec()).unwrap(), scale));
                }
            }
            _ => panic!("postamble中の未対応opcode {opcode}"),
        }
    }
    definitions.sort_unstable();
    definitions
}

fn joined_log(directory: &Path) -> String {
    std::fs::read_to_string(directory.join("t.log"))
        .unwrap()
        .replace('\n', "")
}

#[test]
fn 和文nfssと従属欧文はpratex固有の宣言面を持つ() {
    let package = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tex/latex/pratex/pratex-japanese.sty"),
    )
    .unwrap();
    for required in [
        r"\DeclarePraTeXJapaneseFontShape",
        r"\pratexjfontencoding",
        r"\pratexjfontfamily",
        r"\pratexjfontseries",
        r"\pratexjfontshape",
        r"\DeclarePraTeXRelationFont",
        r"\SetPraTeXRelationFont",
        r"\UsePraTeXRelationFont",
        r"\AddToHook{cmd/selectfont/before}",
        r"\AddToHook{selectfont}",
        r"\global\def\pratex@relationpending{0}",
        r"pratex@relation@exact@",
        r"pratex@relation@wild@",
        r"\number\dimexpr\f@size pt\relax",
    ] {
        assert!(package.contains(required), "宣言面が欠けている: {required}");
    }
    for setter in [
        r"\def\pratexjfontencoding#1{\edef\pratex@jencoding{#1}}",
        r"\def\pratexjfontfamily#1{\edef\pratex@jfamily{#1}}",
        r"\def\pratexjfontseries#1{\edef\pratex@jseries{#1}}",
        r"\def\pratexjfontshape#1{\edef\pratex@jshape{#1}}",
    ] {
        assert!(
            package.contains(setter),
            "和文setterが要求時点を固定しない: {setter}"
        );
    }
    assert!(
        !package.contains(r"\AddToHook{cmd/selectfont/after}"),
        "JFM同期をgeneric command hookへ戻してはならない"
    );
    for forbidden in [
        r"\DeclareRelationFont",
        r"\SetRelationFont",
        r"\userelfont",
        r"\pdftexversion",
        r"\luatexversion",
        r"\XeTeXversion",
        r"\pTeXversion",
        r"\upTeXversion",
    ] {
        assert!(
            !package.contains(forbidden),
            "未契約の互換名または他engine identityを含む: {forbidden}"
        );
    }
}

#[test]
fn 和文属性の群復元とmetric別size_cacheをdviへ渡す() {
    let directory = prepare_directory("attributes");
    std::fs::write(
        directory.join("t.tex"),
        r#"\catcode123=1
\catcode125=2
\catcode35=6
\catcode`\@=11
\def\NeedsTeXFormat#1[#2]{}
\def\ProvidesPackage#1[#2]{}
\def\PackageError#1#2#3{\errmessage{#1: #2}}
\def\@empty{}
\def\space{ }
\long\def\AtBeginDocument#1{\gdef\pratexdocumenthook{#1}}
\long\def\AddToHook#1#2{%
  \expandafter\gdef\csname pratex@testhook:#1\endcsname{#2}}
\def\fontencoding#1{\edef\f@encoding{#1}}
\def\fontfamily#1{\edef\f@family{#1}}
\def\fontseries#1{\edef\f@series{#1}}
\def\fontshape#1{\edef\f@shape{#1}}
\def\fontseriesforce#1{\edef\f@series{#1}}
\def\fontshapeforce#1{\edef\f@shape{#1}}
\def\selectfont{%
  \csname pratex@testhook:cmd/selectfont/before\endcsname
  \csname pratex@testhook:selectfont\endcsname}
\def\f@size{10}
\def\f@encoding{OT1}\def\f@family{cmr}\def\f@series{m}\def\f@shape{n}
\input pratex-japanese.sty
\DeclarePraTeXJapaneseFontShape{PJY1}{gt}{bx}{it}{upjisg-h}
\pratexdocumenthook
\kcatcode"3042=16
\global\setbox0=\hbox{あ}
{\def\f@size{9}%
 \def\requestedencoding{PJY1}\def\requestedfamily{gt}%
 \def\requestedseries{bx}\def\requestedshape{it}%
 \pratexjfontencoding{\requestedencoding}%
 \pratexjfontfamily{\requestedfamily}%
 \pratexjfontseries{\requestedseries}%
 \pratexjfontshape{\requestedshape}%
 \def\requestedencoding{BROKEN}\def\requestedfamily{BROKEN}%
 \def\requestedseries{BROKEN}\def\requestedshape{BROKEN}%
 \selectfont
 \global\setbox1=\hbox{あ}
 \def\f@size{9.0}\selectfont
 \global\setbox2=\hbox{あ}%
 \message{[JATTR-SNAPSHOT=\pratex@jencoding/\pratex@jfamily/%
   \pratex@jseries/\pratex@jshape]}}
\global\setbox3=\hbox{あ}
\message{[JATTR-OUT=\pratex@jfamily/\pratex@jseries/\pratex@jshape]}
{\def\f@size{14.4}\selectfont
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
        "和文NFSS/JFM試験を実行できない: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = joined_log(&directory);
    assert!(
        log.contains("[JATTR-SNAPSHOT=PJY1/gt/bx/it]"),
        "和文属性がmacroの再定義に追随してしまう: {log}"
    );
    assert!(
        log.contains("[JATTR-OUT=mc/m/n]"),
        "和文属性が群終了時に戻らない: {log}"
    );
    assert!(
        log.contains("[JFM-NFSS=5.0pt/4.5pt/4.5pt/5.0pt/7.2pt]"),
        "NFSS sizeまたは群復元がJFMへ届かない: {log}"
    );
    assert_eq!(
        japanese_font_definitions(&std::fs::read(directory.join("t.dvi")).unwrap()),
        [
            ("upjisg-h".to_owned(), 589_824),
            ("upjisr-h".to_owned(), 655_360),
            ("upjisr-h".to_owned(), 943_718),
        ],
        "metricとspの組をcacheし、9と9.0だけを共有しなければならない"
    );
}

#[test]
fn 従属欧文は一回だけ適用し宣言の大域局所とshape_wildcardを守る() {
    let directory = prepare_directory("relations");
    std::fs::write(
        directory.join("t.tex"),
        r#"\catcode123=1
\catcode125=2
\catcode35=6
\catcode`\@=11
\def\NeedsTeXFormat#1[#2]{}
\def\ProvidesPackage#1[#2]{}
\def\PackageError#1#2#3{\message{[PACKAGE-ERROR=#2]}}
\def\@empty{}
\def\space{ }
\long\def\AtBeginDocument#1{\gdef\pratexdocumenthook{#1}}
\long\def\AddToHook#1#2{%
  \expandafter\gdef\csname pratex@testhook:#1\endcsname{#2}}
\def\fontencoding#1{\edef\f@encoding{#1}}
\def\fontfamily#1{\edef\f@family{#1}}
\def\fontseries#1{\edef\f@series{#1}}
\def\fontshape#1{\edef\f@shape{#1}}
\def\fontseriesforce#1{\edef\f@series{#1}}
\def\fontshapeforce#1{\edef\f@shape{#1}}
\def\selectfont{%
  \csname pratex@testhook:cmd/selectfont/before\endcsname
  \csname pratex@testhook:selectfont\endcsname}
\def\f@size{10}
\def\f@encoding{OT1}\def\f@family{cmr}\def\f@series{m}\def\f@shape{n}
\input pratex-japanese.sty
\DeclarePraTeXRelationFont{PJY1}{mc}{m}{n}{T1}{cmr}{bx}{it}
{\SetPraTeXRelationFont{PJY1}{mc}{m}{n}{TS1}{cmss}{m}{sl}
 \UsePraTeXRelationFont\selectfont
 \message{[REL-LOCAL=\f@encoding/\f@family/\f@series/\f@shape]}}
\fontencoding{OT1}\fontfamily{manual}\fontseries{m}\fontshape{n}
\UsePraTeXRelationFont\selectfont
\message{[REL-GLOBAL=\f@encoding/\f@family/\f@series/\f@shape]}
\fontfamily{manual}\selectfont
\message{[REL-ONESHOT=\f@encoding/\f@family/\f@series/\f@shape]}
\fontencoding{OT1}\fontfamily{cmr}\fontseries{m}\fontshape{n}%
\UsePraTeXRelationFont
{\selectfont
 \message{[REL-NESTED-IN=\f@encoding/\f@family/\f@series/\f@shape]}}
\message{[REL-NESTED-PENDING=\pratex@relationpending]}
\fontfamily{manual}\selectfont
\message{[REL-NESTED-OUT=\f@encoding/\f@family/\f@series/\f@shape]}
\DeclarePraTeXJapaneseFontShape{PJY1}{mc}{w}{it}{upjisr-h}
\DeclarePraTeXJapaneseFontShape{PJY1}{mc}{w}{all}{upjisr-h}
\DeclarePraTeXRelationFont{PJY1}{mc}{w}{}{TU}{lmss}{sb}{n}
\DeclarePraTeXRelationFont{PJY1}{mc}{w}{all}{T1}{cmtt}{bx}{it}
\pratexjfontseries{w}\pratexjfontshape{it}\fontshape{sl}
\UsePraTeXRelationFont\selectfont
\message{[REL-WILDCARD=\f@encoding/\f@family/\f@series/\f@shape]}
\pratexjfontshape{all}\fontencoding{OT1}\fontfamily{cmr}%
\fontseries{m}\fontshape{sl}\UsePraTeXRelationFont\selectfont
\message{[REL-ALL-EXACT=\f@encoding/\f@family/\f@series/\f@shape]}
\pratexjfontshape{it}%
\pratexjfontfamily{missing}\fontencoding{OT1}\fontfamily{manual}%
\fontseries{m}\fontshape{n}\UsePraTeXRelationFont\selectfont
\message{[REL-MISSING=\f@encoding/\f@family/\f@series/\f@shape]}
\end
"#,
    )
    .unwrap();

    let output = run_rtex(&directory);
    assert!(
        output.status.success(),
        "従属欧文試験を実行できない: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = joined_log(&directory);
    for expected in [
        "[REL-LOCAL=TS1/cmss/m/sl]",
        "[REL-GLOBAL=T1/cmr/bx/it]",
        "[REL-ONESHOT=T1/manual/bx/it]",
        "[REL-NESTED-IN=T1/cmr/bx/it]",
        "[REL-NESTED-PENDING=0]",
        "[REL-NESTED-OUT=OT1/manual/m/n]",
        "[REL-WILDCARD=TU/lmss/sb/sl]",
        "[REL-ALL-EXACT=T1/cmtt/bx/it]",
        "[REL-MISSING=OT1/manual/m/n]",
        "[PACKAGE-ERROR=No relation font for PJY1/missing/w/it]",
        "[PACKAGE-ERROR=Japanese font shape PJY1/missing/w/it is not declared]",
    ] {
        assert!(
            log.contains(expected),
            "従属欧文契約が違う: {expected}\n{log}"
        );
    }
}

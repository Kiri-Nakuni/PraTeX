//! 横組JFM/K/X/禁則を、実hlist・line break・DVIへ一回だけ接続する回帰。
//!
//! JFM/TFMは公開file formatだけから合成し、他engineのsourceや上流testを使わない。
//! 2026-08-24には公開pTeX manual 2025-05-10と、TeX Live 2026の公式e-upTeX
//! `3.141592653-p4.1.2-u2.02-251130-2.6`を自作入力だけで照合した。使用した
//! `uptex.windows` archiveのSHA-256は`docs/euptex-port-notes.md`記録済みの
//! `c878983da002f32a24a507680ccf00261a3761089ed324892668ded589bf9c0d`。
//! 直結和和Kはshow listへ出ず、箱寸法・再箱詰め・改行・DVI移動には効く一方、
//! Xはmaterial glueとして表示される、という観測を以下へ固定する。unshifted hboxの
//! edgeに置くKは`MaterialKanjiSkip`として直結glyph間の`VirtualKanjiSkip`から分ける。
//! discretionaryは左側を遮断し、no-break/post-break枝末尾から右側だけを接続するため、
//! このsliceでは空枝がK/Xを接続しない契約だけをproduction回帰にする。

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

/// class1→2は2.5pt glue、class2→1は-2.5pt kernになる10pt横組JFM。
fn spacing_jfm() -> Vec<u8> {
    let char_types = [
        char_type(0, 0),
        char_type(0x003001, 3),
        char_type(0x003002, 3),
        char_type(0x00300c, 4),
        char_type(0x00300d, 5),
        char_type(0x003042, 1),
        char_type(0x003044, 2),
        char_type(0x00ff08, 4),
        char_type(0x00ff09, 5),
    ];
    let char_infos = [
        [1, 0x11, 0, 0],
        [2, 0x11, 1, 0],
        [1, 0x11, 1, 1],
        [2, 0x11, 0, 0],
        [2, 0x11, 0, 0],
        [2, 0x11, 0, 0],
    ];
    let widths = [ZERO, ONE, HALF];
    let heights = [ZERO, HALF];
    let depths = [ZERO, QUARTER];
    let italics = [ZERO];
    let glue_kern_steps = [[128, 2, 0, 0], [128, 1, 128, 0]];
    let kerns = [-QUARTER];
    let glues = [QUARTER, QUARTER / 2, QUARTER / 4];
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
        + glue_kern_steps.len()
        + kerns.len()
        + glues.len()
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
        glue_kern_steps.len() as u16,
        kerns.len() as u16,
        glues.len() as u16,
        params.len() as u16,
    ] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0x5350_4347_u32.to_be_bytes());
    bytes.extend_from_slice(&(10_u32 << 20).to_be_bytes());
    for table in [&char_types[..], &char_infos[..]] {
        for word in table {
            bytes.extend_from_slice(word);
        }
    }
    for table in [&widths[..], &heights[..], &depths[..], &italics[..]] {
        for value in table {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
    for word in &glue_kern_steps {
        bytes.extend_from_slice(word);
    }
    for table in [&kerns[..], &glues[..], &params[..]] {
        for value in table {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
    assert_eq!(bytes.len(), lf * 4);
    bytes
}

fn latin_a_tfm() -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in [21_u16, 2, 65, 65, 2, 1, 1, 1, 0, 0, 0, 7] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&[0x00, 0xa0, 0x00, 0x00]);
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&[0, 16, 0, 0]);
    bytes.extend_from_slice(&[0; 3 * 4]);
    bytes.extend_from_slice(&[0; 7 * 4]);
    assert_eq!(bytes.len(), 21 * 4);
    bytes
}

fn test_directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    std::env::temp_dir().join(format!(
        "pratex-japanese-spacing-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ))
}

fn prepare_directory(name: &str) -> PathBuf {
    let directory = test_directory(name);
    std::fs::create_dir_all(&directory).unwrap();
    for name in [
        "t.tex",
        "t.log",
        "t.dvi",
        "mk.tex",
        "mk.log",
        "mk.fmt",
        "use.tex",
        "use.log",
        "spacing.tfm",
        "latin.tfm",
    ] {
        let _ = std::fs::remove_file(directory.join(name));
    }
    std::fs::write(directory.join("spacing.tfm"), spacing_jfm()).unwrap();
    std::fs::write(directory.join("latin.tfm"), latin_a_tfm()).unwrap();
    directory
}

fn run_rtex(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap()
}

fn assert_success(output: &Output, message: &str) {
    assert!(
        output.status.success(),
        "{message}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn joined_log(directory: &Path, stem: &str) -> String {
    std::fs::read_to_string(directory.join(format!("{stem}.log")))
        .unwrap()
        .replace(['\r', '\n'], "")
}

fn common_prefix() -> &'static str {
    "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
     \\font\\L=latin at 10pt \\L\n\
     \\pratexjfont\\J=spacing at 10pt \\J\n\
     \\kcatcode\"3001=16 \\kcatcode\"3002=16\n\
     \\kcatcode\"300C=16 \\kcatcode\"300D=16\n\
     \\kcatcode\"3042=16 \\kcatcode\"3044=16\n\
     \\kcatcode\"FF08=16 \\kcatcode\"FF09=16\n"
}

#[test]
fn jfmと仮想kと実xはhlistへ一度だけ入り明示nodeを越えない() {
    let directory = prepare_directory("hlistとbarrier");
    let source = format!(
        "{}\\kanjiskip=1pt plus .5pt minus .25pt
         \\xkanjiskip=2pt
         \\setbox0=\\hbox{{あい}}
         \\setbox1=\\hbox{{いあ}}
         \\setbox2=\\hbox{{ああ}}
         \\setbox3=\\hbox{{あA}}
         \\setbox4=\\hbox{{Aあ}}
         \\setbox5=\\hbox{{あ\\kern3pt い}}
         \\setbox6=\\hbox{{あ\\hskip4pt い}}
         \\setbox7=\\hbox{{あ\\special{{barrier}}い}}
         \\setbox8=\\hbox{{あ$ $い}}
         \\setbox9=\\hbox{{あ\\penalty50 あ}}
         \\setbox10=\\hbox{{あ。}}
         \\message{{[boxes=\\the\\wd0/\\the\\wd1/\\the\\wd2/\\the\\wd3/\\the\\wd4/\\the\\wd5/\\the\\wd6/\\the\\wd7/\\the\\wd8/\\the\\wd9/\\the\\wd10]}}
         \\showboxbreadth=100 \\showboxdepth=10
         \\showbox0 \\showbox1 \\showbox2 \\showbox3 \\showbox9 \\showbox10
         \\kanjiskip=3pt \\setbox11=\\hbox{{\\unhcopy2}}
         \\setbox12=\\hbox{{あ\\vrule width1pt height0pt depth0pt い}}
         \\setbox13=\\hbox{{あ\\hbox{{}}い}}
         \\setbox14=\\hbox{{あ\\discretionary{{}}{{}}{{}}い}}
         \\message{{[refinalized=\\the\\wd11]}}
         \\message{{[more-barriers=\\the\\wd12/\\the\\wd13/\\the\\wd14]}}
         \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "横組spacingのhlist試験を実行できなかった");
    let log = joined_log(&directory, "t");
    assert!(
        log.contains(
            "[boxes=17.5pt/12.5pt/11.0pt/17.0pt/17.0pt/18.0pt/19.0pt/15.0pt/15.0pt/11.0pt/11.0pt]"
        ),
        "{log}"
    );
    assert!(log.contains("[refinalized=13.0pt]"), "{log}");
    assert!(
        log.contains("[more-barriers=16.0pt/15.0pt/15.0pt]"),
        "{log}"
    );
    assert!(
        log.contains("\\glue(\\pratexjfm) 2.5 plus 1.25 minus 0.625"),
        "{log}"
    );
    assert!(log.contains("\\kern-2.5 (PraTeX JFM)"), "{log}");
    assert!(!log.contains("\\glue(\\kanjiskip)"), "{log}");
    assert!(log.contains("\\glue(\\xkanjiskip) 2.0"), "{log}");
    assert!(log.contains("\\penalty 50"), "{log}");
    assert!(log.contains("\\penalty 10000"), "{log}");
}

#[test]
fn 上下移動しないhboxの和文edgeだけに実kとxを置く() {
    let directory = prepare_directory("hbox edgeのmaterial KとX");
    let source = format!(
        "{}\\kanjiskip=4pt plus 1pt minus 2pt \\xkanjiskip=3pt
         \\showboxbreadth=100 \\showboxdepth=10
         \\setbox0=\\hbox{{あ\\hbox{{あ}}}}
         \\setbox1=\\hbox{{\\hbox{{あ}}あ}}
         \\setbox2=\\hbox{{A\\hbox{{あ}}}}
         \\setbox3=\\hbox{{\\hbox{{あ}}A}}
         \\setbox4=\\hbox{{あ\\hbox{{A}}}}
         \\setbox5=\\hbox{{\\hbox{{A}}あ}}
         \\setbox6=\\hbox{{あ\\raise1pt\\hbox{{あ}}}}
         \\setbox7=\\hbox{{あ\\raise1pt\\hbox{{A}}}}
         \\setbox8=\\hbox{{あ\\hbox{{\\hbox{{}}あ}}}}
         \\setbox9=\\hbox{{あ\\hbox{{\\kern0pt あ}}}}
         \\message{{[box-edge=\\the\\wd0/\\the\\wd1/\\the\\wd2/\\the\\wd3/\\the\\wd4/\\the\\wd5/\\the\\wd6/\\the\\wd7/\\the\\wd8/\\the\\wd9]}}
         \\showbox0
         \\setbox10=\\hbox{{\\unhcopy0\\setbox20=\\lastbox
           \\message{{[K-lastbox=\\the\\lastskip/\\the\\lastnodetype]}}}}
         \\kanjiskip=6pt \\setbox11=\\hbox{{\\unhcopy0}}
         \\setbox12=\\hbox{{\\unhcopy2\\setbox21=\\lastbox
           \\message{{[X-lastbox=\\the\\lastskip/\\the\\lastnodetype]}}}}
         \\setbox13=\\hbox{{ああ}} \\showbox13
         \\message{{[after-open=\\the\\wd10/\\the\\wd11/\\the\\wd12]}}
         \\kanjiskip=4pt
         \\setbox14=\\hbox{{あ\\hbox{{あ}}\\kern1pt\\vrule width1pt height1pt depth0pt}}
         \\shipout\\box14
         \\hsize=5pt \\parindent=0pt
         \\pretolerance=-1 \\tolerance=10000 あ\\hbox{{あ}}\\par
         \\message{{[box-edge-lines=\\the\\prevgraf]}}
         \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "hbox edgeのK/X試験を実行できなかった");
    let log = joined_log(&directory, "t");
    assert!(
        log.contains(
            "[box-edge=14.0pt/14.0pt/18.0pt/18.0pt/18.0pt/18.0pt/10.0pt/15.0pt/14.0pt/10.0pt]"
        ),
        "{log}"
    );
    assert!(
        log.contains("\\glue(\\kanjiskip) 4.0 plus 1.0 minus 2.0"),
        "{log}"
    );
    assert!(log.contains("[K-lastbox=0.0pt/0]"), "{log}");
    assert!(log.contains("[X-lastbox=0.0pt/0]"), "{log}");
    assert!(log.contains("[after-open=5.0pt/16.0pt/10.0pt]"), "{log}");
    assert!(log.contains("[box-edge-lines=2]"), "{log}");
    let direct = log
        .split("> \\box13=")
        .nth(1)
        .expect("直結glyphのshowbox出力がある");
    assert!(!direct.contains("\\glue(\\kanjiskip)"), "{direct}");
    let (wide, rules) = first_page_events(&std::fs::read(directory.join("t.dvi")).unwrap());
    assert_eq!(wide.len(), 2);
    assert_eq!(wide[1].h, wide[0].h + 9 * 65_536);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].h, wide[0].h + 15 * 65_536);
}

#[test]
fn 空discretionaryは左右のkとxを接続しない() {
    let directory = prepare_directory("空discretionaryのKとX");
    let source = format!(
        "{}\\kanjiskip=4pt \\xkanjiskip=3pt
         \\setbox0=\\hbox{{あ\\discretionary{{}}{{}}{{}}あ}}
         \\setbox1=\\hbox{{あ\\discretionary{{}}{{}}{{}}A}}
         \\setbox2=\\hbox{{A\\discretionary{{}}{{}}{{}}あ}}
         \\message{{[empty-disc=\\the\\wd0/\\the\\wd1/\\the\\wd2]}}
         \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(
        &output,
        "空discretionaryのK/X barrier試験を実行できなかった",
    );
    let log = joined_log(&directory, "t");
    assert!(log.contains("[empty-disc=10.0pt/15.0pt/15.0pt]"), "{log}");
}

#[test]
fn 仮想kは利用者glueから隠れ寸法と再箱詰めには効く() {
    let directory = prepare_directory("仮想Kの可視性");
    let source = format!(
        "{}\\kanjiskip=4pt plus 1pt minus 2pt \\xkanjiskip=3pt
         \\showboxbreadth=100 \\showboxdepth=10
         \\setbox0=\\hbox{{ああ\\message{{[pair-tail=\\the\\lastskip/\\the\\lastnodetype]}}}}
         \\showbox0
         \\kanjiskip=6pt \\setbox1=\\hbox{{\\unhcopy0}}
         \\kanjiskip=4pt \\setbox2=\\hbox{{ああ\\hskip7pt\\unskip
           \\message{{[after-unskip=\\the\\lastskip/\\the\\lastnodetype]}}}}
         \\showbox2
         \\setbox3=\\hbox{{あA}} \\showbox3
         \\message{{[virtual-widths=\\the\\wd0/\\the\\wd1/\\the\\wd2/\\the\\wd3]}}
         \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "仮想Kの利用者可視性試験を実行できなかった");
    let log = joined_log(&directory, "t");
    assert!(log.contains("[pair-tail=0.0pt/0]"), "{log}");
    assert!(log.contains("[after-unskip=0.0pt/0]"), "{log}");
    assert!(
        log.contains("[virtual-widths=14.0pt/16.0pt/14.0pt/18.0pt]"),
        "{log}"
    );
    assert!(!log.contains("\\glue(\\kanjiskip)"), "{log}");
    assert!(log.contains("\\glue(\\xkanjiskip) 3.0"), "{log}");
}

#[test]
fn hbox終端の局所kをunsave前にsnapshotする() {
    let directory = prepare_directory("局所K snapshot");
    let source = format!(
        "{}\\kanjiskip=1pt
         \\setbox0=\\hbox{{\\kanjiskip=4pt ああ}}
         \\setbox1=\\hbox{{ああ}}
         \\message{{[local=\\the\\wd0][restored=\\the\\wd1]}}
         \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "局所Kのsnapshot試験を実行できなかった");
    let log = joined_log(&directory, "t");
    assert!(log.contains("[local=14.0pt][restored=11.0pt]"), "{log}");
}

#[test]
fn alignment_cellも局所kをunsave前にsnapshotする() {
    let directory = prepare_directory("alignment cellの局所K");
    let source = format!(
        "{}\\kanjiskip=1pt
         \\setbox0=\\vbox{{\\halign{{#\\cr \\kanjiskip=4pt ああ\\cr}}}}
         \\setbox1=\\hbox{{ああ}}
         \\message{{[alignment=\\the\\wd0][restored=\\the\\wd1]}}
         \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "alignment cellのK snapshot試験を実行できなかった");
    let log = joined_log(&directory, "t");
    assert!(log.contains("[alignment=14.0pt][restored=11.0pt]"), "{log}");
}

#[test]
fn k境界は改行候補になりbuilt_in最小禁則は鍵括弧の分離を防ぐ() {
    let directory = prepare_directory("改行と最小禁則");
    let source = format!(
        "{}\\setbox0=\\hbox{{「あ」\\kern1pt\\vrule width1pt height1pt depth0pt}}
         \\shipout\\box0
         \\hsize=5pt \\parindent=0pt \\tolerance=10000 \\pretolerance=-1
         \\kanjiskip=1pt
         ああ\\par \\message{{[kanji-lines=\\the\\prevgraf]}}
         あ。\\par \\message{{[kinsoku-lines=\\the\\prevgraf]}}
         「あ\\par \\message{{[open-bracket-lines=\\the\\prevgraf]}}
         あ」\\par \\message{{[close-bracket-lines=\\the\\prevgraf]}}
         \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "Kと禁則のline break試験を実行できなかった");
    let log = joined_log(&directory, "t");
    assert!(log.contains("[kanji-lines=2]"), "{log}");
    assert!(log.contains("[kinsoku-lines=1]"), "{log}");
    assert!(log.contains("[open-bracket-lines=1]"), "{log}");
    assert!(log.contains("[close-bracket-lines=1]"), "{log}");
    let (wide, rules) = first_page_events(&std::fs::read(directory.join("t.dvi")).unwrap());
    assert_eq!(
        wide.iter().map(|event| event.character).collect::<Vec<_>>(),
        [0x300c, 0x3042, 0x300d]
    );
    assert_eq!(wide[1].h, wide[0].h + 5 * 65_536);
    assert_eq!(wide[2].h, wide[0].h + 10 * 65_536);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].h, wide[0].h + 16 * 65_536);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WideEvent {
    character: u32,
    h: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuleEvent {
    h: i32,
    width: i32,
}

fn read_unsigned(bytes: &[u8], position: &mut usize, length: usize) -> u32 {
    let mut value = 0;
    for _ in 0..length {
        value = (value << 8) | u32::from(bytes[*position]);
        *position += 1;
    }
    value
}

fn read_signed(bytes: &[u8], position: &mut usize, length: usize) -> i32 {
    let mut value = if bytes[*position] & 0x80 == 0 { 0 } else { -1 };
    for _ in 0..length {
        value = (value << 8) | i32::from(bytes[*position]);
        *position += 1;
    }
    value
}

fn first_page_events(bytes: &[u8]) -> (Vec<WideEvent>, Vec<RuleEvent>) {
    let bop = bytes.iter().position(|&byte| byte == 139).unwrap();
    let mut position = bop + 45;
    let mut h = 0;
    let mut v = 0;
    let mut w = 0;
    let mut x = 0;
    let mut y = 0;
    let mut z = 0;
    let mut stack = Vec::new();
    let mut wide = Vec::new();
    let mut rules = Vec::new();
    loop {
        let opcode = bytes[position];
        position += 1;
        match opcode {
            129 | 130 => {
                let character = read_unsigned(bytes, &mut position, usize::from(opcode - 127));
                wide.push(WideEvent { character, h });
                h += match character {
                    0x300c | 0x300d | 0x3042 => 5 * 65_536,
                    0x3044 => 10 * 65_536,
                    _ => panic!("fixture外のwide glyph U+{character:04X}"),
                };
            }
            132 => {
                let _height = read_signed(bytes, &mut position, 4);
                let width = read_signed(bytes, &mut position, 4);
                rules.push(RuleEvent { h, width });
                h += width;
            }
            138 => {}
            140 => break,
            141 => stack.push((h, v, w, x, y, z)),
            142 => (h, v, w, x, y, z) = stack.pop().unwrap(),
            143..=146 => h += read_signed(bytes, &mut position, usize::from(opcode - 142)),
            147 => h += w,
            148..=151 => {
                w = read_signed(bytes, &mut position, usize::from(opcode - 147));
                h += w;
            }
            152 => h += x,
            153..=156 => {
                x = read_signed(bytes, &mut position, usize::from(opcode - 152));
                h += x;
            }
            157..=160 => v += read_signed(bytes, &mut position, usize::from(opcode - 156)),
            161 => v += y,
            162..=165 => {
                y = read_signed(bytes, &mut position, usize::from(opcode - 161));
                v += y;
            }
            166 => v += z,
            167..=170 => {
                z = read_signed(bytes, &mut position, usize::from(opcode - 166));
                v += z;
            }
            171..=238 => {
                if opcode >= 235 {
                    position += usize::from(opcode - 234);
                }
            }
            239..=242 => {
                let length = read_unsigned(bytes, &mut position, usize::from(opcode - 238));
                position += length as usize;
            }
            243..=246 => {
                position += usize::from(opcode - 242) + 12;
                let area = usize::from(bytes[position]);
                let name = usize::from(bytes[position + 1]);
                position += 2 + area + name;
            }
            other => panic!("fixture pageの未対応DVI opcode {other}"),
        }
    }
    (wide, rules)
}

#[test]
fn jfm_glueはdvi上の次glyphとrule座標へ一度だけ効く() {
    let directory = prepare_directory("DVI座標");
    let source = format!(
        "{}\\setbox0=\\hbox{{あい\\kern1pt\\vrule width1pt height1pt depth0pt}}
         \\shipout\\box0 \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "JFM glueのDVI試験を実行できなかった");
    let (wide, rules) = first_page_events(&std::fs::read(directory.join("t.dvi")).unwrap());
    assert_eq!(wide.len(), 2);
    assert_eq!(wide[0].character, 0x3042);
    assert_eq!(wide[1].character, 0x3044);
    assert_eq!(wide[1].h, wide[0].h + 7 * 65_536 + 32_768);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].h, wide[0].h + 18 * 65_536 + 32_768);
    assert_eq!(rules[0].width, 65_536);
}

#[test]
fn 仮想kはdviの次glyph座標へ一度だけ効く() {
    let directory = prepare_directory("仮想KのDVI座標");
    let source = format!(
        "{}\\kanjiskip=4pt
         \\setbox0=\\hbox{{ああ}} \\shipout\\box0 \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "仮想KのDVI座標試験を実行できなかった");
    let (wide, rules) = first_page_events(&std::fs::read(directory.join("t.dvi")).unwrap());
    assert_eq!(wide.len(), 2);
    assert_eq!(wide[0].character, 0x3042);
    assert_eq!(wide[1].character, 0x3042);
    assert_eq!(wide[1].h, wide[0].h + 9 * 65_536);
    assert!(rules.is_empty());
}

#[test]
fn fmtから戻したkとjfm対表がproduction_finalizerへ届く() {
    let directory = prepare_directory("fmt spacing");
    let make = format!(
        "{}\\kanjiskip=3pt \\xkanjiskip=2pt
         \\setbox0=\\hbox{{ああ}} \\dump\n",
        common_prefix()
    );
    std::fs::write(directory.join("mk.tex"), make).unwrap();
    let output = run_rtex(&directory, &["mk.tex"]);
    assert_success(&output, "spacing入りfmtを生成できなかった");
    std::fs::write(
        directory.join("use.tex"),
        "\\batchmode\n\\setbox1=\\hbox{ああ}\n\\setbox3=\\hbox{あい}\n\
         \\kanjiskip=4pt \\setbox2=\\hbox{\\unhcopy0}\n\
         \\message{[fmt=\\the\\wd1/\\the\\wd2/\\the\\wd3]}\n\\end\n",
    )
    .unwrap();
    let output = run_rtex(&directory, &["&mk", "use.tex"]);
    assert_success(&output, "spacing入りfmtを読み戻せなかった");
    let log = joined_log(&directory, "use");
    assert!(log.contains("[fmt=13.0pt/14.0pt/17.5pt]"), "{log}");
}

#[test]
fn 自動間隔switchと許可表はhbox終端の現在値を使い群ごとに戻る() {
    let directory = prepare_directory("auto switchと許可表");
    let source = format!(
        "{}\\kanjiskip=1pt \\xkanjiskip=2pt
         \\setbox0=\\hbox{{ああ}} \\setbox1=\\hbox{{あA}}
         \\setbox2=\\hbox{{\\noautospacing ああ}}
         \\setbox3=\\hbox{{\\noautoxspacing あA}}
         \\setbox4=\\hbox{{\\xspcode65=0 あA}}
         \\setbox5=\\hbox{{\\inhibitxspcode\"3042=0 あA}}
         \\message{{[values=\\the\\xspcode65/\\the\\inhibitxspcode\"3042]}}
         \\message{{[controlled=\\the\\wd0/\\the\\wd1/\\the\\wd2/\\the\\wd3/\\the\\wd4/\\the\\wd5]}}
         \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(
        &output,
        "自動間隔switchと許可表のhbox試験を実行できなかった",
    );
    let log = joined_log(&directory, "t");
    assert!(log.contains("[values=3/3]"), "{log}");
    assert!(
        log.contains("[controlled=11.0pt/17.0pt/10.0pt/15.0pt/15.0pt/15.0pt]"),
        "{log}"
    );
}

#[test]
fn xspcodeとinhibitxspcodeとswitchは独立にglobaldefsへ従う() {
    let directory = prepare_directory("spacing stateのsave stack");
    let source = format!(
        "{}\\kanjiskip=1pt \\xkanjiskip=2pt
         \\xspcode65=3 \\xspcode66=3
         \\inhibitxspcode\"3042=3 \\inhibitxspcode\"3044=3
         {{\\xspcode65=0 \\inhibitxspcode\"3042=0
           \\global\\xspcode66=1 \\global\\inhibitxspcode\"3044=1}}
         \\message{{[tables=\\the\\xspcode65/\\the\\xspcode66/\\the\\inhibitxspcode\"3042/\\the\\inhibitxspcode\"3044]}}
         {{\\noautospacing \\global\\noautoxspacing}}
         \\setbox0=\\hbox{{ああ}} \\setbox1=\\hbox{{あA}}
         \\autoxspacing
         {{\\globaldefs=-1 \\global\\xspcode65=0 \\global\\inhibitxspcode\"3042=0
           \\message{{[inside=\\the\\xspcode65/\\the\\inhibitxspcode\"3042]}}}}
         \\message{{[outside=\\the\\xspcode65/\\the\\inhibitxspcode\"3042]}}
         \\message{{[switches=\\the\\wd0/\\the\\wd1]}}
         \\end\n",
        common_prefix()
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "spacing stateのsave stack試験を実行できなかった");
    let log = joined_log(&directory, "t");
    assert!(log.contains("[tables=3/1/3/1]"), "{log}");
    assert!(log.contains("[inside=0/0]"), "{log}");
    assert!(log.contains("[outside=3/3]"), "{log}");
    assert!(log.contains("[switches=11.0pt/15.0pt]"), "{log}");
}

#[test]
fn 自動間隔switchと許可表はfmtからproduction_finalizerへ戻る() {
    let directory = prepare_directory("spacing controls fmt");
    let make = format!(
        "{}\\kanjiskip=1pt \\xkanjiskip=2pt
         \\noautospacing \\noautoxspacing
         \\xspcode65=0 \\inhibitxspcode\"3042=1
         \\dump\n",
        common_prefix()
    );
    std::fs::write(directory.join("mk.tex"), make).unwrap();
    let output = run_rtex(&directory, &["mk.tex"]);
    assert_success(&output, "自動間隔制御入りfmtを生成できなかった");

    std::fs::write(
        directory.join("use.tex"),
        "\\batchmode
         \\setbox0=\\hbox{ああ} \\setbox1=\\hbox{あA}
         \\autospacing \\autoxspacing
         \\setbox2=\\hbox{ああ} \\setbox3=\\hbox{あA}
         \\xspcode65=3 \\setbox4=\\hbox{あA}
         \\message{[fmt-values=\\the\\xspcode65/\\the\\inhibitxspcode\"3042]}
         \\message{[fmt-controls=\\the\\wd0/\\the\\wd1/\\the\\wd2/\\the\\wd3/\\the\\wd4]}
         \\end\n",
    )
    .unwrap();
    let output = run_rtex(&directory, &["&mk", "use.tex"]);
    assert_success(&output, "自動間隔制御入りfmtを読み戻せなかった");
    let log = joined_log(&directory, "use");
    assert!(log.contains("[fmt-values=3/1]"), "{log}");
    assert!(
        log.contains("[fmt-controls=10.0pt/15.0pt/11.0pt/15.0pt/17.0pt]"),
        "{log}"
    );
}

#[test]
fn inhibitxspcodeの局所消去は復元枠を予約しglobal追加に消費させない() {
    let directory = prepare_directory("inhibit restore reservation");
    let mut source = format!(
        "{}\\inhibitxspcode\"3042=0
         {{\\inhibitxspcode\"3042=3\n",
        common_prefix()
    );
    for code_point in 0x4000_u32..0x4400 {
        source.push_str(&format!("\\global\\inhibitxspcode\"{code_point:X}=0\n"));
    }
    source.push_str(
        "}\\message{[reservation=\\the\\inhibitxspcode\"3042/\\the\\inhibitxspcode\"4000/\\the\\inhibitxspcode\"43FF]}
         {\\inhibitxspcode\"3042=3 \\global\\inhibitxspcode\"3042=1}
         \\message{[reserved-target=\\the\\inhibitxspcode\"3042]}\\end\n",
    );
    std::fs::write(directory.join("t.tex"), source).unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "容量超過を回復して復元予約を守れなかった");
    let log = joined_log(&directory, "t");
    assert!(log.contains("Too many inhibitxspcode entries"), "{log}");
    assert!(log.contains("[reservation=0/0/3]"), "{log}");
    assert!(log.contains("[reserved-target=1]"), "{log}");
    assert_eq!(
        log.matches("Too many inhibitxspcode entries").count(),
        1,
        "{log}"
    );
    assert!(!log.contains("panicked at"), "{log}");
}

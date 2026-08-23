//! PraTeX nativeの横組JFMを、和文glyph nodeからDVIまで通す。
//!
//! fixtureは公開JFM file formatだけから組み立て、配布fontや他engineのsourceに依存しない。

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ZERO: i32 = 0;
const QUARTER: i32 = 0x0004_0000;
const HALF: i32 = 0x0008_0000;
const ONE: i32 = 0x0010_0000;
const HORIZONTAL_JFM_ID: u16 = 11;
const VERTICAL_JFM_ID: u16 = 9;

fn char_type(character_code: u32, character_type: u8) -> [u8; 4] {
    [
        ((character_code >> 8) & 0xff) as u8,
        (character_code & 0xff) as u8,
        ((character_code >> 16) & 0xff) as u8,
        character_type,
    ]
}

fn synthetic_jfm(direction: u16) -> Vec<u8> {
    let char_types = [
        char_type(0, 0),
        char_type(0x003042, 1),
        char_type(0x020000, 2),
    ];
    // class 0: 1zw x (.5zh + .25zh), class 1: .5zw x .5zh,
    // class 2: 1zw x (.5zh + .25zh)
    let char_infos = [[1, 0x11, 0, 0], [2, 0x10, 0, 0], [1, 0x11, 0, 0]];
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
        direction,
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

fn test_directory(name: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    let directory = std::env::temp_dir().join(format!(
        "pratex-japanese-glyph-{}-{:016x}",
        std::process::id(),
        hash.finish()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn prepare_directory(name: &str) -> PathBuf {
    let directory = test_directory(name);
    for file_name in [
        "t.tex",
        "t.log",
        "t.dvi",
        "mk.tex",
        "mk.log",
        "mk.fmt",
        "use.tex",
        "use.log",
        "use.dvi",
        "synthetic.tfm",
        "vertical.tfm",
    ] {
        let _ = std::fs::remove_file(directory.join(file_name));
    }
    std::fs::write(
        directory.join("synthetic.tfm"),
        synthetic_jfm(HORIZONTAL_JFM_ID),
    )
    .unwrap();
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WideEvent {
    opcode: u8,
    character: u32,
    font: Option<u32>,
    h: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuleEvent {
    h: i32,
    height: i32,
    width: i32,
}

#[derive(Default)]
struct PageEvents {
    wide: Vec<WideEvent>,
    rules: Vec<RuleEvent>,
    font_definitions: Vec<(u32, Vec<u8>)>,
}

fn read_unsigned(bytes: &[u8], position: &mut usize, length: usize) -> u32 {
    let mut value = 0_u32;
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

/// 最初のpageをDVI公開仕様どおりに解釈する。wide glyphのadvanceだけはfixtureの
/// JFM metricを使い、後続ruleの絶対hとの整合を検査する。
fn parse_first_page(bytes: &[u8]) -> PageEvents {
    let bop = bytes.iter().position(|&byte| byte == 139).unwrap();
    let mut position = bop + 45;
    let mut events = PageEvents::default();
    let mut h = 0_i32;
    let mut v = 0_i32;
    let mut w = 0_i32;
    let mut x = 0_i32;
    let mut y = 0_i32;
    let mut z = 0_i32;
    let mut stack = Vec::new();
    let mut font = None;

    loop {
        let opcode = bytes[position];
        position += 1;
        match opcode {
            0..=127 => panic!("このfixtureのpageに8-bit glyphは現れない"),
            128 => panic!("このfixtureのpageにset1 glyphは現れない"),
            129 | 130 => {
                let length = usize::from(opcode - 127);
                let character = read_unsigned(bytes, &mut position, length);
                events.wide.push(WideEvent {
                    opcode,
                    character,
                    font,
                    h,
                });
                h += match character {
                    0x3042 => 5 * 65_536,
                    0x020000 => 10 * 65_536,
                    _ => panic!("fixtureにない和文符号位置: U+{character:04X}"),
                };
            }
            131 => panic!("Unicode scalarにset4は不要"),
            132 => {
                let height = read_signed(bytes, &mut position, 4);
                let width = read_signed(bytes, &mut position, 4);
                events.rules.push(RuleEvent { h, height, width });
                h += width;
            }
            133..=137 => panic!("このfixtureにput命令は現れない"),
            138 => {}
            139 => panic!("page途中にbopが現れた"),
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
            171..=234 => font = Some(u32::from(opcode - 171)),
            235..=238 => {
                font = Some(read_unsigned(
                    bytes,
                    &mut position,
                    usize::from(opcode - 234),
                ));
            }
            239..=242 => {
                let length_bytes = usize::from(opcode - 238);
                let length = read_unsigned(bytes, &mut position, length_bytes) as usize;
                position += length;
            }
            243..=246 => {
                let number = read_unsigned(bytes, &mut position, usize::from(opcode - 242));
                position += 12;
                let area_length = usize::from(bytes[position]);
                let name_length = usize::from(bytes[position + 1]);
                position += 2 + area_length;
                let name = bytes[position..position + name_length].to_vec();
                position += name_length;
                events.font_definitions.push((number, name));
            }
            _ => panic!("page中の未対応DVI opcode {opcode}"),
        }
    }
    events
}

#[test]
fn 横組jfmの寸法とwide_dvi命令と座標を一続きで保つ() {
    let directory = prepare_directory("横組end-to-end");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\pratexjfont\\J=synthetic at 10pt\n\\J\n\
         \\kcatcode\"3042=16\n\\kcatcode\"20000=16\n\
         \\dimen0=1zw \\dimen1=1zh\n\
         \\setbox0=\\hbox{あ𠀀\\kern1pt\\vrule width1pt height1pt depth0pt}\n\
         \\message{[metric=\\the\\dimen0/\\the\\dimen1][box=\\the\\wd0/\\the\\ht0/\\the\\dp0]}\n\
         \\showboxbreadth=10 \\showboxdepth=10 \\showbox0\n\
         \\shipout\\box0\n\\end\n",
    )
    .unwrap();

    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "横組JFM入力をDVIへ出せなかった");
    let log = std::fs::read_to_string(directory.join("t.log"))
        .unwrap()
        .replace('\n', "");
    assert!(log.contains("[metric=10.0pt/7.5pt]"), "{log}");
    assert!(log.contains("[box=17.0pt/5.0pt/2.5pt]"), "{log}");
    assert!(log.contains("\\pratexwideglyph あ class 1"), "{log}");
    assert!(log.contains("\\pratexwideglyph 𠀀 class 2"), "{log}");

    let dvi = std::fs::read(directory.join("t.dvi")).unwrap();
    let events = parse_first_page(&dvi);
    assert!(
        events
            .font_definitions
            .iter()
            .any(|(number, name)| *number == 256 && name == b"synthetic"),
        "和文font 256の定義がない: {:?}",
        events.font_definitions
    );
    assert_eq!(
        events.wide,
        [
            WideEvent {
                opcode: 129,
                character: 0x3042,
                font: Some(256),
                h: events.wide[0].h,
            },
            WideEvent {
                opcode: 130,
                character: 0x020000,
                font: Some(256),
                h: events.wide[0].h + 5 * 65_536,
            },
        ]
    );
    assert_eq!(events.rules.len(), 1);
    assert_eq!(events.rules[0].h, events.wide[0].h + 16 * 65_536);
    assert_eq!(events.rules[0].height, 65_536);
    assert_eq!(events.rules[0].width, 65_536);
}

#[test]
fn current和文fontは群で復元されzwとzhも追随する() {
    let directory = prepare_directory("群復元");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\jfont\\Jten=synthetic at 10pt\n\
         \\pratexjfont\\Jtwenty=synthetic at 20pt\n\
         \\Jten \\dimen0=1zw \\dimen1=1zh\n\
         \\message{[outer=\\the\\dimen0/\\the\\dimen1]}\n\
         {\\Jtwenty \\dimen0=1zw \\dimen1=1zh\n\
          \\message{[inner=\\the\\dimen0/\\the\\dimen1]}}\n\
         \\dimen0=1zw \\dimen1=1zh\n\
         \\message{[restored=\\the\\dimen0/\\the\\dimen1]}\n\\end\n",
    )
    .unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "current和文fontの群試験を実行できなかった");
    let log = std::fs::read_to_string(directory.join("t.log"))
        .unwrap()
        .replace('\n', "");
    assert!(log.contains("[outer=10.0pt/7.5pt]"), "{log}");
    assert!(log.contains("[inner=20.0pt/15.0pt]"), "{log}");
    assert!(log.contains("[restored=10.0pt/7.5pt]"), "{log}");
}

#[test]
fn 和文fontと選択はfmtを往復してwide_dviを出せる() {
    let directory = prepare_directory("fmt往復");
    std::fs::write(
        directory.join("mk.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\kcatcode\"3042=16\n\\pratexjfont\\J=synthetic at 10pt\n\\J\n\\dump\n",
    )
    .unwrap();
    let output = run_rtex(&directory, &["mk.tex"]);
    assert_success(&output, "和文font入りfmtを生成できなかった");
    assert!(directory.join("mk.fmt").exists());

    std::fs::write(
        directory.join("use.tex"),
        "\\batchmode\n\\dimen0=1zw\n\\message{[zw=\\the\\dimen0]}\n\
         \\setbox0=\\hbox{あ}\n\\shipout\\box0\n\\end\n",
    )
    .unwrap();
    let output = run_rtex(&directory, &["&mk", "use.tex"]);
    assert_success(&output, "和文font入りfmtを読み戻せなかった");
    let log = std::fs::read_to_string(directory.join("use.log"))
        .unwrap()
        .replace('\n', "");
    assert!(log.contains("[zw=10.0pt]"), "{log}");
    let events = parse_first_page(&std::fs::read(directory.join("use.dvi")).unwrap());
    assert_eq!(events.wide.len(), 1);
    assert_eq!(events.wide[0].opcode, 129);
    assert_eq!(events.wide[0].character, 0x3042);
    assert_eq!(events.wide[0].font, Some(256));
}

#[test]
fn 外部vertical_modeの和文は選択済みjfmで段落を開始する() {
    let directory = prepare_directory("外部vertical mode");
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\pratexjfont\\J=synthetic at 10pt\n\\J\n\\kcatcode\"3042=16\n\
         あ\\par\n\\end\n",
    )
    .unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "外部vertical modeから和文段落を開始できなかった");
    let log = std::fs::read_to_string(directory.join("t.log")).unwrap();
    assert!(
        !log.contains("CJK typesetting needs a Japanese font metric"),
        "{log}"
    );
    let events = parse_first_page(&std::fs::read(directory.join("t.dvi")).unwrap());
    assert_eq!(events.wide.len(), 1);
    assert_eq!(events.wide[0].character, 0x3042);
    assert_eq!(events.wide[0].font, Some(256));
}

#[test]
fn 縦組jfmは横組primitiveで黙って選ばれない() {
    let directory = prepare_directory("縦組診断");
    std::fs::write(
        directory.join("vertical.tfm"),
        synthetic_jfm(VERTICAL_JFM_ID),
    )
    .unwrap();
    std::fs::write(
        directory.join("t.tex"),
        "\\catcode123=1\n\\catcode125=2\n\\batchmode\n\
         \\pratexjfont\\J=vertical at 10pt\n\\J\n\\kcatcode\"3042=16\nあ\n\\end\n",
    )
    .unwrap();
    let output = run_rtex(&directory, &["t.tex"]);
    assert_success(&output, "縦組JFMの診断入力を完走できなかった");
    let log = std::fs::read_to_string(directory.join("t.log"))
        .unwrap()
        .replace('\n', "");
    assert!(
        log.contains("vertical JFM is not supported by this horizontal slice"),
        "{log}"
    );
    assert!(
        log.contains("CJK typesetting needs a Japanese font metric"),
        "{log}"
    );
    assert!(!directory.join("t.dvi").exists());
}

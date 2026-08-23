use std::path::PathBuf;
use std::process::Command;

fn 読む(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn classは明示的なpratex_identityだけを要求する() {
    let class = 読む("tex/latex/pratex/prjsarticle.cls");
    assert!(class.contains(r"\ifdefined\pratexversion"));
    assert!(class.contains(r"\ifnum\pratexversion<1"));

    for forbidden in [
        r"\pdftexversion",
        r"\luatexversion",
        r"\XeTeXversion",
        r"\epTeXversion",
        r"\pTeXversion",
        r"\upTeXversion",
        r"\NeedsTeXFormat{pLaTeX2e}",
    ] {
        assert!(
            !class.contains(forbidden),
            "他engineへの偽装・identity依存を含む: {forbidden}"
        );
    }
}

#[test]
fn classは横組articleの公開面とfont_hookを持つ() {
    let class = 読む("tex/latex/pratex/prjsarticle.cls");
    for required in [
        r"\LoadClass{article}",
        r"\renewcommand\maketitle",
        r"\renewcommand\section",
        r"\renewcommand\subsection",
        r"\renewcommand\paragraph",
        r"\pratexsetjapanesefonthook",
        r"\pratexsetlatinfonthook",
        r"\kanjiskip=0zw",
        r"\xkanjiskip=.25zw",
        r"\setlength\parindent{1em}",
        r"\setlength\leftmargini{2em}",
    ] {
        assert!(
            class.contains(required),
            "class契約が欠けている: {required}"
        );
    }
}

#[test]
fn 代表標本は和欧混植と基本listを一つずつ通す() {
    let sample = 読む("docs/examples/prjsarticle-sample.tex");
    for required in [
        r"\documentclass[a4paper,10pt]{prjsarticle}",
        r"\maketitle",
        "日本語とLatin text",
        r"\section{",
        r"\begin{itemize}",
        r"\begin{enumerate}",
        r"\begin{description}",
    ] {
        assert!(
            sample.contains(required),
            "代表標本が欠けている: {required}"
        );
    }
}

#[test]
fn asset_manifestは取得物をhashとlicenseで固定する() {
    let manifest = 読む("tests-support/prjsarticle/assets.json");
    for required in [
        "latex-base.tds.zip",
        "424bcbab851723495397f0542db8722a68917f31d9f28055ebc65baa7ed35336",
        "l3kernel.tds.zip",
        "342e0ac756b418d095a23eb37aa771a4df3d27db396d43c9e911e0ab9e138aca",
        "unicode-data.tds.zip",
        "ef541913356b94a2ed0795e41609b8108db4edf0227080151b865c3a4963c895",
        "cm-tfm.zip",
        "9c0f99fa34c7d801c40f6b5ff60bc28f200e8ef6ffb2fe75e54ca835c67fc04c",
        "latex-fonts.zip",
        "4e73240c4037643a7ef7c353bedd4a10cf0e180d851c54f1e68fda4397f33936",
        "LPPL-1.3c-or-later",
        "Knuth License",
    ] {
        assert!(
            manifest.contains(required),
            "asset lockが欠けている: {required}"
        );
    }

    let runner = 読む("tools/test-prjsarticle.ps1");
    assert!(runner.contains("Invoke-WebRequest"));
    assert!(runner.contains("Assert-Hash"));
    assert!(runner.contains("repository外"));
    assert!(!runner.contains(r"\def\pratexversion"));
    assert!(!runner.contains("Get-Command kpsewhich"));
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DviPosition {
    h: i64,
    v: i64,
    w: i64,
    x: i64,
    y: i64,
    z: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DviRule {
    page: u32,
    offset: usize,
    h: i64,
    v: i64,
    height: i64,
    width: i64,
    put: bool,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct DviMeaning {
    pages: u32,
    rules: Vec<DviRule>,
    first_glyph: Option<(u32, usize)>,
}

struct DviReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> DviReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn byte(&mut self) -> Result<u8, String> {
        let byte = self
            .bytes
            .get(self.pos)
            .copied()
            .ok_or_else(|| format!("DVIがoffset {}で途切れた", self.pos))?;
        self.pos += 1;
        Ok(byte)
    }

    fn skip(&mut self, length: usize) -> Result<(), String> {
        let end = self
            .pos
            .checked_add(length)
            .ok_or_else(|| "DVI record長がoverflowした".to_owned())?;
        if end > self.bytes.len() {
            return Err(format!(
                "DVIがoffset {}で{} bytes不足した",
                self.pos,
                end - self.bytes.len()
            ));
        }
        self.pos = end;
        Ok(())
    }

    fn unsigned(&mut self, length: usize) -> Result<u64, String> {
        if !(1..=4).contains(&length) {
            return Err(format!("未対応のDVI整数長: {length}"));
        }
        let mut value = 0_u64;
        for _ in 0..length {
            value = (value << 8) | u64::from(self.byte()?);
        }
        Ok(value)
    }

    fn signed(&mut self, length: usize) -> Result<i64, String> {
        let value = self.unsigned(length)?;
        let shift = 64 - 8 * length;
        Ok(((value << shift) as i64) >> shift)
    }
}

fn note_glyph(meaning: &mut DviMeaning, page: u32, offset: usize) {
    if meaning.first_glyph.is_none() {
        meaning.first_glyph = Some((page, offset));
    }
}

fn dvi意味を読む(bytes: &[u8]) -> Result<DviMeaning, String> {
    let mut reader = DviReader::new(bytes);
    let mut meaning = DviMeaning::default();
    let mut position = DviPosition::default();
    let mut stack = Vec::new();
    let mut page = 0_u32;

    while reader.pos < bytes.len() {
        let offset = reader.pos;
        let opcode = reader.byte()?;
        match opcode {
            0..=127 => note_glyph(&mut meaning, page, offset),
            128..=131 => {
                reader.skip(usize::from(opcode - 127))?;
                note_glyph(&mut meaning, page, offset);
            }
            132 | 137 => {
                let height = reader.signed(4)?;
                let width = reader.signed(4)?;
                let put = opcode == 137;
                meaning.rules.push(DviRule {
                    page,
                    offset,
                    h: position.h,
                    v: position.v,
                    height,
                    width,
                    put,
                });
                if !put {
                    position.h += width;
                }
            }
            133..=136 => {
                reader.skip(usize::from(opcode - 132))?;
                note_glyph(&mut meaning, page, offset);
            }
            138 => {}
            139 => {
                if !stack.is_empty() {
                    return Err(format!("bop前のDVI stackが空ではない: offset {offset}"));
                }
                reader.skip(44)?;
                position = DviPosition::default();
                page = page
                    .checked_add(1)
                    .ok_or_else(|| "DVI page数がoverflowした".to_owned())?;
                meaning.pages = page;
            }
            140 => {
                if !stack.is_empty() {
                    return Err(format!("eop時のDVI stackが空ではない: offset {offset}"));
                }
            }
            141 => stack.push(position),
            142 => {
                position = stack
                    .pop()
                    .ok_or_else(|| format!("DVI stack underflow: offset {offset}"))?;
            }
            143..=146 => position.h += reader.signed(usize::from(opcode - 142))?,
            147 => position.h += position.w,
            148..=151 => {
                position.w = reader.signed(usize::from(opcode - 147))?;
                position.h += position.w;
            }
            152 => position.h += position.x,
            153..=156 => {
                position.x = reader.signed(usize::from(opcode - 152))?;
                position.h += position.x;
            }
            157..=160 => position.v += reader.signed(usize::from(opcode - 156))?,
            161 => position.v += position.y,
            162..=165 => {
                position.y = reader.signed(usize::from(opcode - 161))?;
                position.v += position.y;
            }
            166 => position.v += position.z,
            167..=170 => {
                position.z = reader.signed(usize::from(opcode - 166))?;
                position.v += position.z;
            }
            171..=234 => {}
            235..=238 => reader.skip(usize::from(opcode - 234))?,
            239..=242 => {
                let length = reader.unsigned(usize::from(opcode - 238))?;
                let length = usize::try_from(length)
                    .map_err(|_| "DVI special長がusizeへ収まらない".to_owned())?;
                reader.skip(length)?;
            }
            243..=246 => {
                reader.skip(usize::from(opcode - 242) + 12)?;
                let area = usize::from(reader.byte()?);
                let name = usize::from(reader.byte()?);
                reader.skip(area + name)?;
            }
            247 => {
                reader.skip(13)?;
                let comment = usize::from(reader.byte()?);
                reader.skip(comment)?;
            }
            248 => reader.skip(28)?,
            249 => {
                reader.skip(5)?;
                while reader.pos < bytes.len() && bytes[reader.pos] == 223 {
                    reader.pos += 1;
                }
                if reader.pos != bytes.len() {
                    return Err(format!(
                        "post_post後に223以外のbyteがある: offset {}",
                        reader.pos
                    ));
                }
            }
            _ => return Err(format!("未定義DVI opcode {opcode}: offset {offset}")),
        }
    }
    Ok(meaning)
}

fn 固有rule<'a>(meaning: &'a DviMeaning, width: i64, height: i64) -> &'a DviRule {
    let matches: Vec<_> = meaning
        .rules
        .iter()
        .filter(|rule| rule.width == width && rule.height == height)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "{width}sp x {height}spのmarker ruleは一個でなければならない: {matches:#?}"
    );
    matches[0]
}

fn maketitleの既知座標を照合する(bytes: &[u8]) {
    let meaning = dvi意味を読む(bytes).expect("DVIを公開opcodeとして復号できること");
    assert_eq!(meaning.pages, 1, "title fixtureは一頁でなければならない");

    let title = 固有rule(&meaning, 17, 23);
    let author = 固有rule(&meaning, 19, 29);
    let date = 固有rule(&meaning, 21, 31);
    let body = 固有rule(&meaning, 31, 37);
    for marker in [title, author, date, body] {
        assert_eq!(marker.page, 1);
        assert!(!marker.put, "markerはadvanceするset_ruleでなければならない");
    }

    if let Some((glyph_page, glyph_offset)) = meaning.first_glyph {
        assert!(
            glyph_page > 1 || glyph_offset > body.offset,
            "TFM幅なしで座標を推定しないため、四markerより前にglyphを置けない"
        );
    }

    // 10ptの40em = 26,214,400sp。奇数spの余りはleading/top側へ一sp多く
    // 配る、というprjsarticle自身の幾何契約を固定する。±1spの許容は置かない。
    assert_eq!(title.h - body.h, 13_107_192, "titleの水平座標");
    assert_eq!(author.h - body.h, 13_107_191, "authorの水平座標");
    assert_eq!(date.h - body.h, 13_107_190, "dateの水平座標");

    assert!(title.v < author.v && author.v < date.v && date.v < body.v);
    assert_eq!(body.v - title.v, 8_110_068, "titleの垂直座標");
    assert_eq!(body.v - author.v, 5_160_945, "authorの垂直座標");
    assert_eq!(body.v - date.v, 3_194_864, "dateの垂直座標");
}

fn signed4(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn bop(bytes: &mut Vec<u8>) {
    bytes.push(139);
    bytes.extend_from_slice(&[0; 44]);
}

fn right4(bytes: &mut Vec<u8>, amount: i32) {
    bytes.push(146);
    signed4(bytes, amount);
}

fn down4(bytes: &mut Vec<u8>, amount: i32) {
    bytes.push(160);
    signed4(bytes, amount);
}

fn set_rule(bytes: &mut Vec<u8>, height: i32, width: i32) {
    bytes.push(132);
    signed4(bytes, height);
    signed4(bytes, width);
}

#[test]
fn dvi_decoderは移動stackとruleのsp座標を保つ() {
    let mut dvi = Vec::new();
    bop(&mut dvi);
    right4(&mut dvi, 100);
    down4(&mut dvi, 200);
    dvi.push(141);
    dvi.extend_from_slice(&[143, 7]);
    set_rule(&mut dvi, 23, 17);
    dvi.push(142);
    dvi.push(137);
    signed4(&mut dvi, 29);
    signed4(&mut dvi, 19);
    dvi.push(140);

    let meaning = dvi意味を読む(&dvi).unwrap();
    assert_eq!(meaning.pages, 1);
    assert_eq!(meaning.first_glyph, None);
    assert_eq!(meaning.rules.len(), 2);
    assert_eq!((meaning.rules[0].h, meaning.rules[0].v), (107, 200));
    assert_eq!((meaning.rules[1].h, meaning.rules[1].v), (100, 200));
    assert!(!meaning.rules[0].put);
    assert!(meaning.rules[1].put);
}

#[test]
fn maketitle_oracleは一spのずれを許容しない() {
    let body_h = 100;
    let title_v = 1_000_000;
    let body_v = title_v + 8_110_068;
    let targets = [
        (body_h + 13_107_192, title_v, 23, 17),
        (body_h + 13_107_191, body_v - 5_160_945, 29, 19),
        (body_h + 13_107_190, body_v - 3_194_864, 31, 21),
        (body_h, body_v, 37, 31),
    ];

    let mut dvi = Vec::new();
    bop(&mut dvi);
    let mut h = 0;
    let mut v = 0;
    for (target_h, target_v, height, width) in targets {
        right4(&mut dvi, target_h - h);
        h = target_h;
        down4(&mut dvi, target_v - v);
        v = target_v;
        set_rule(&mut dvi, height, width);
        h += width;
    }
    dvi.push(140);
    maketitleの既知座標を照合する(&dvi);

    let mut shifted = dvi.clone();
    // titleだけを一sp左へ動かす。次のright4へ一spを戻し、後続markerは変えない。
    shifted[49] -= 1;
    shifted[68] += 1;
    let failed = std::panic::catch_unwind(|| maketitleの既知座標を照合する(&shifted));
    assert!(failed.is_err(), "titleの1spずれを見逃している");
}

#[test]
#[ignore = "PraTeX identityと日本語glyph/JFM枝をmerge後、pinned CTAN cacheで実行する"]
fn 公式ctan資材でprjsarticle_title_dviを生成して照合する() {
    let cache = std::env::var_os("PRATEX_PRJSARTICLE_ASSET_CACHE")
        .expect("PRATEX_PRJSARTICLE_ASSET_CACHEをrepository外のcacheへ設定する");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let work = std::env::temp_dir().join(format!(
        "pratex-prjsarticle-live-{}-{stamp}",
        std::process::id()
    ));
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/test-prjsarticle.ps1");
    let shell = std::env::var_os("PRATEX_PWSH").unwrap_or_else(|| "pwsh".into());
    let engine = std::env::var_os("PRATEX_PRJSARTICLE_ENGINE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_rtex")));

    let mut command = Command::new(shell);
    command
        .arg("-NoProfile")
        .arg("-File")
        .arg(script)
        .arg("-AssetCache")
        .arg(cache)
        .arg("-WorkRoot")
        .arg(&work)
        .arg("-RtexPath")
        .arg(engine);
    if std::env::var_os("PRATEX_PRJSARTICLE_FETCH").as_deref() == Some("1".as_ref()) {
        command.arg("-Fetch");
    }
    let output = command.output().expect("PowerShell runnerを起動できること");
    assert!(
        output.status.success(),
        "prjsarticle runner失敗:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let dvi = std::fs::read(work.join("run/maketitle-oracle.dvi")).unwrap();
    maketitleの既知座標を照合する(&dvi);
}

use std::path::PathBuf;

fn 読む(path: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
}

#[test]
fn 派生classは上流の権利とrevisionを保持する() {
    let class = 読む("tex/latex/pratex/prjlreq.cls");
    for required in [
        "Copyright 2017-2026, Noriyuki Abe.",
        "BSD 2-Clause License",
        "ac06fd0770096be8a118197d670a34da06384e02",
        "not an independent",
        "prjlreq-UPSTREAM-LICENSE.txt",
        r"\ProvidesClass{prjlreq}",
    ] {
        assert!(
            class.contains(required),
            "派生classの来歴が欠けている: {required}"
        );
    }
    assert!(!class.contains("Copyright (C) Codex"));
    assert!(!class.contains(r"\ProvidesClass{jlreq}"));

    let license = 読む("tex/latex/pratex/prjlreq-UPSTREAM-LICENSE.txt");
    for required in [
        "Copyright 2017-2026, Noriyuki Abe.",
        "Redistribution and use in source and binary forms",
        "Redistributions of source code must retain",
        "Redistributions in binary form must reproduce",
        "THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\"",
    ] {
        assert!(
            license.contains(required),
            "BSD noticeが欠けている: {required}"
        );
    }

    let provenance = 読む("docs/prjlreq-provenance.md");
    assert!(provenance.contains("独立実装とは呼ばない"));
    assert!(provenance.contains("排他的権利"));
    assert!(provenance.contains("4f990534dcf3bebce7c399fa3855eaab1d5fba2ca0a5142dd2173c987f494a5c"));
}

#[test]
fn classはpratex固有identityから横組だけをロードする() {
    let class = 読む("tex/latex/pratex/prjlreq.cls");
    for required in [
        r"\ifdefined\pratexversion",
        r"\ifdefined\pratexjfont",
        r"\ifdefined\kanjiskip",
        r"\ifdefined\xkanjiskip",
        r"\LoadClass{article}",
        r"\RequirePackage{pratex-japanese}",
        r"\DeclareOption{tate}",
        r"\DeclareOption{book}",
        r"\DeclareOption{report}",
        "PraTeX vertical direction is not connected yet",
    ] {
        assert!(
            class.contains(required),
            "最小ロード境界が欠けている: {required}"
        );
    }
    assert!(!class.contains(r"\ifnum\pratexversion<1"));
    for forbidden in [
        r"\pdftexversion",
        r"\luatexversion",
        r"\XeTeXversion",
        r"\pTeXversion",
        r"\upTeXversion",
        r"\epTeXversion",
        r"\NeedsTeXFormat{pLaTeX2e}",
    ] {
        assert!(
            !class.contains(forbidden),
            "他engine identityへ依存している: {forbidden}"
        );
    }
}

#[test]
fn 横組fixtureは上流由来の和文間隔と和欧混植を通す() {
    let class = 読む("tex/latex/pratex/prjlreq.cls");
    for required in [
        r"\providecommand*{\jlreqkanjiskip}{0pt plus .25\jlreq@zw minus 0pt}",
        r"\providecommand*{\jlreqxkanjiskip}",
        r"\kanjiskip=\jlreqkanjiskip\relax",
        r"\xkanjiskip=\jlreqxkanjiskip\relax",
        r"\setlength{\parindent}{1\jlreq@zw}",
        r"\emergencystretch=3\jlreq@zh",
    ] {
        assert!(class.contains(required), "横組既定が欠けている: {required}");
    }

    let fixture = 読む("tests/fixtures/prjlreq/horizontal-minimal.tex");
    for required in [
        r"\documentclass[a4paper,10pt]{prjlreq}",
        "PRJLREQ-KANJISKIP:",
        "PRJLREQ-XKANJISKIP:",
        "PRJLREQ-PARINDENT:",
        "PRJLREQ-IDENTITY:",
        "日本語とLatin ABC xyz、数字0123",
    ] {
        assert!(
            fixture.contains(required),
            "横組fixtureが欠けている: {required}"
        );
    }
}

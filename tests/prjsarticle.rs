use std::path::PathBuf;

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
        assert!(class.contains(required), "class契約が欠けている: {required}");
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
        assert!(sample.contains(required), "代表標本が欠けている: {required}");
    }
}

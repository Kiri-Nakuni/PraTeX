# `prjsarticle` — PraTeX native横組class

`tex/latex/pratex/prjsarticle.cls`は、PraTeXの日本語glyph/JFM枝へ接続するための
小規模なLaTeX article classである。A4、約40字相当の行長、一字下げ、広めの行送り、
固定高さのtitle block、見出し、基本list、日本語font hookを持つ。縦組、割注、縦中横は
このclassの初版範囲ではない。

## engine identityとfont境界

classは`\pratexversion >= 1`を必須とする。他engineの`\pdftexversion`、
`\luatexversion`、`\XeTeXversion`、pTeX/upTeX判定用primitiveを定義・参照して
package分岐を偽装しない。初版engine identityとして次をPraTeX coreへ要求する。

- `\pratexversion`: read-only integer。初版の公開契約値は1。
- `\pratexrevision`: expandableなbuild/revision文字列。比較の主手段にはしない。
- 将来の個別能力には、`\pratexfeature{japanese-horizontal-glyph-dvi}`が0または
  契約versionを返す低頻度queryを推奨する。

formatまたはPraTeX adapterは、次のclass固有hookへfont選択命令を登録する。

- `\pratexsetjapanesefonthook{...}`
- `\pratexsetlatinfonthook{...}`
- `\pratexjapanesefonthook` / `\pratexlatinfonthook`

これらはengine identityを作るAPIではない。pTeX互換名をclass側で補うadapterも置かない。

## production出力までのengine依存

2026-08-23の基点`dd1d775`にはclassが利用する`\kanjiskip`、`\xkanjiskip`、`zw`はあるが、
次は未接続である。

1. `CjkToken`からJFM/TFM metric付きwide glyph nodeを作ること。
2. classの日本語font hookから選べるPraTeX nativeなJFM/NFSS adapter。
3. wide glyphをDVI `set2` / `set3` eventへ出すこと。
4. K/X tableを中央spacing finalizerへ接続し、段落へ自動挿入すること。
5. 実用品質には`xspcode`/`inhibitxspcode`、禁則、和文widow処理を加えること。

標準日本語組版をVaak/WASM callbackへ逃がさない。classのfont hookは組版判断を行わず、
engineが所有するfont/glyph境界を選択するだけである。

## `\maketitle`回帰方針

LaTeX本家との完全DVI一致はLaPraTeX format完成まで要求しない。`prjsarticle`は固定高さの
title/author/date rowを持ち、`tests/fixtures/prjsarticle/maketitle-oracle.tex`の固有ruleを
PraTeX自身のknown semantic oracleとして使う。試験はDVIを公開仕様どおり復号し、title ruleの
sp座標、中央揃え、body開始位置、page数を固定する。plain欧文のorigin/main rTeX完全回帰とは
別gateである。

## jsclasses調査と権利

公開挙動の比較対象として、CTAN `jsclasses` 2025-05-10を2026-08-23に取得した。

- URL: `https://mirrors.ctan.org/macros/jptex/latex/jsclasses.zip`
- SHA-256: `b73ec5e8208dfa1dae6f58cab9b033e8e91780aefaaacc879c26e904ce8953f8`
- license: BSD-2-Clause
- copyright: 1995--1999 ASCII MEDIA WORKS、1999--2016 Haruhiko Okumura、
  2016--2025 Japanese TeX Development Community

`prjsarticle.cls`へjsclassesのsource、macro定義、test本文は移植していない。A4横組、
日本語文書向けの行長・行送り・titleという利用者から見える挙動だけを調査し、LaTeXの
公開class interfaceから独立に実装した。jsclasses自身も、classだけでは日本語対応を有効化せず、
対応engine環境が別途必要であるとREADMEで明記している。

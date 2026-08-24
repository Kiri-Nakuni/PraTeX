# 日本語出版互換性fixture matrix

更新: 2026-08-24

## 実行段階

| 段階 | 目的 | 合格条件 |
|---|---|---|
| 0. prerequisite | engine不足とpackage非互換を分ける | e-TeX/TeX--XeT、upTeX互換、PDF backendの対象機能が個別gateに合格 |
| 1. load | class/packageの検出・option処理 | 他engine偽装なし、未対応optionを黙って無視しない、error 0 |
| 2. minimal semantics | 一機能ずつ意味を固定 | DVI recordまたはPDF object、寸法、node、抽出文字を検査 |
| 3. composition | 機能同士の相互作用 | 縦組、ruby、割注、注、float、font切替等を同時利用して意味が変わらない |
| 4. official sample | 実在class/template | 配布sampleを無改変で処理し、official engineの意味と比較 |
| 5. long document | 再実行・page build・性能 | 複数回処理が収束し、100頁級で参照・目次・索引・memoryを検査 |
| 6. delivery | 印刷・公開成果物 | profile別preflight、font、box、色、tag、抽出、validatorに合格 |

## P0共通fixture

### `font-role-matrix`

- 欧文: roman、italic、bold、sans、mono、数式roman/italic/bold
- 和文: 明朝、ゴシック、複数weight、縦横用face
- 文脈: 本文、書名、章見出し、caption、脚注、bookmark
- 検査: 選択TFM/JFM/native face、PDF `/BaseFont`、`/FontDescriptor`、`/Widths`、埋込み/subset、
  ToUnicode、抽出文字、描画

少し前のPraTeX枝で報告された「欧字がすべてタイプライタ体になる」退行の直接gateにする。

### `font-range-routing`

- Latin、Greek、Cyrillic、仮名、漢字、約物、BMP、SIP、第三面、IVS
- exact scalar、範囲、Unicode plane、routing class、縦横、fallback chain
- bold/italic時の代替、missing glyph、同tier重複error、group/global/fmt
- layout font identity、glyph selection、ToUnicodeを検査

### `jlreq-horizontal` / `jlreq-vertical`

- 和欧混植、JLReq約物、禁則、見出し、脚注、数式、表、画像、二段組
- 縦組側は右綴じ、奇偶頁、柱・ノンブル、白頁、縦中横、割注を追加
- pLaTeX、upLaTeX、LuaLaTeXの公式profileを別oracleにし、平均化しない

### `dvi-pdf-backend`

`graphicx + xcolor + hyperref`、p/up側の`pxjahyper`を用い、PNG/PDF/EPS、色、内部link、
日本語bookmark/metadata、page size、font embeddingを検査する。DVI specialと最終PDFは別のoracleにする。

### `novel-b6-twoside`

- B6J、縦組、book、twoside、右綴じ
- 前付、本文、後付、目次、扉、中扉、章見出し、奥付
- 行長、行数、ノド、天・地、見開きbaseline、柱、ノンブル
- 会話文、地の文、二倍ダーシ、三点リーダー、ruby、圏点、縦中横、割注、挿絵

### `academic-class-minimal-*`

最初の公式class群は次とする。

- 情報処理学会: `submit` / `techrep`
- 電子情報通信学会: `paper` / `technicalreport`
- 土木学会: upLaTeX / LuaLaTeX
- jlreq: 横組 / 縦組

各classについて、title、和英abstract、author/affiliation、figure、table、数式、footnote、citationだけの
最小原稿と、公式sample全体を分ける。

## P1 fixture

| fixture | 主な対象 |
|---|---|
| `ruby-matrix` | PXrubricaとnative ruby: mono/group/jukugo/両側、行頭末、分割、進入、圏点、ActualText |
| `warichu-tatechuyoko` | 自動/手動割注、1–3桁縦中横、禁則、見出し・脚注内、column/page境界 |
| `trimmarks-prepress` | Media/Trim/Bleed/CropBox、3 mm bleed、トンボ、右綴じ、page順 |
| `bibliography-ja-mixed` | pBibTeX/upBibTeX＋学会bstとbiblatex+Biberを分離。和欧著者、URL、DOI、aux/bbl往復 |
| `thesis-long` | 表紙、和英要旨、front/main/back matter、目次、図表一覧、付録、索引、100頁相当 |
| `stem-packages` | amsmath、newtx、siunitx、mhchem、TikZ/pgfplots、listings |
| `humanities-packages` | ruby、割注、endnotes、日本語索引、深い脚注、IPA・複数script |

## P2 fixture

- `utbook/tbook + plext`、`ltjtbook + lltjext`の旧縦組API
- `jsbook`、`BXjscls`の横書き一般書
- `multicol`、`nidanfloat`、段抜き図、最終頁balancing
- `graphicx`、`pdfpages`、`pxtatescale`による全面挿絵と縦組float
- `makeidx/imakeidx + mendex/upmendex`の読み付き索引
- 学会・大学別の追加template

## delivery profile

印刷所や公開先の要求を一つへまとめない。少なくとも次をprofile化する。

- PDF versionとPDF/X-1a/X-4
- MediaBox、TrimBox、BleedBox、CropBox、仕上り寸法と塗足し
- 全font埋込み・subset、ToUnicode、license flag
- DeviceCMYK/ICCBased/output intent、RGB混入、特色、overprint、透明
- 右綴じ、page数・順序、viewer preference
- tagged PDF/PDF/UA、reading order、ruby、柱artifact、表、図alt
- EPUB 3.3、縦組、ruby/圏点fallback、navigation、EPUBCheck

生成成功だけを合格にせず、positive/negative fixtureと独立validatorを用意する。

# 日本の学術論文・学位論文で使われるLaTeX

更新: 2026-08-24

## 現在の三系統

日本の実務では次が併存する。

1. 学会投稿で根強い`(u)pLaTeX -> DVI -> dvipdfmx`
2. 学位論文・新しいtemplateで増えている`LuaLaTeX + LuaTeX-ja + fontspec`
3. `jlreq`、`BXjscls`、土木学会class等の複数engine対応

互換性はpackageを無作為にloadするだけでなく、実在するclassと付属sampleを丸ごと処理する
class-first方式で測る。同時に、原因を一つへ絞った自作fixtureを持つ。

## 優先class・template

| 優先 | 実務例 | 主な現行profile | PraTeXで固定する意味 |
|---|---|---|---|
| P0 | [`jlreq`](https://ctan.org/pkg/jlreq) | p/upLaTeXはDVI、LuaLaTeXは直接PDF | 同じ和欧原稿の横縦、見出し、脚注、表、数式、禁則、JFM |
| P0 | [情報処理学会`ipsj.cls`](https://www.ipsj.or.jp/journal/submit/style.html) | pLaTeX系＋dvipdfmx/dvips | JY1/JT1、`jis/jisg`、`\kanjiskip`、graphicx、color、bst、submit/techrep |
| P0 | [電子情報通信学会`ieicej.cls`](https://www.ieice.org/ftp/) | pLaTeX中心、upTeX検出、DVI | amsmath、newtx、graphicx、xcolor、bm、cite、url、paper/technicalreport |
| P0 | [土木学会`jjsce.cls`](https://appmech-jsce.com/jam/%E5%9C%9F%E6%9C%A8%E5%AD%A6%E4%BC%9A%E8%AB%96%E6%96%87%E9%9B%86%E3%81%AElatex%E3%82%AF%E3%83%A9%E3%82%B9%E3%83%95%E3%82%A1%E3%82%A4%E3%83%AB/) | upLaTeX推奨、LuaLaTeXも公式対応 | 同一classでDVI/直接PDF、zref、flushend、hyperref、pxjahyperを比較 |
| P0 | [北海道大学システム情報科学・学位論文](https://www.ist.hokudai.ac.jp/div/ssi/EduThesisTemplate.html) | LuaLaTeX+ltjsbook推奨、Xe+bxjsbook、up+jsbook | 長文、原ノ味多weight、fontspec、newtxmath/unicode-math、前付・本文・後付、文献 |
| P0 | [京都大学言語学研究](https://ceschi.bun.kyoto-u.ac.jp/archive/bun_archive/linguistics/lin-kulr/index.htm) | LuaLaTeX+ltjsarticle | fontspec、和欧混植、記号脚注、通常脚注、和英abstract。欧文font退行検出 |
| P1 | [日本数学会`msjproc`](https://math.or.jp/meeting/texstyle/howto_ja.pdf) | 日本語はp/up/LuaLaTeX | expl3、l3keys2e、amsmath/amsthm、定理、数式番号、MSC、文献 |
| P1 | [人工知能学会大会](https://conf.ai-gakkai.or.jp/jsai2026/author-guideline/) | jarticle+jsaiac、pLaTeX系 | 二段組、旧LaTeX判定、旧`\kanjiskip` alias、学会bst |
| P1 | [日本機械学会template](https://www.jsme.or.jp/publish/transact/for-authors.html) | pLaTeX、DVI | times/mathptmx、graphicx、数式。欧文がmonoになる退行を捕捉 |
| P1 | [東京大学CS学位論文](https://www.i.u-tokyo.ac.jp/edu/course/cs/thesis.shtml) | pLaTeX->dvipdfmx | 和英表題・要旨、front/main matter、目次、図表一覧、複数章、BibTeX |
| P2 | [日本物理学会大会](https://www.gakkai-web.net/gakkai/jps/jps_n/template.html)、[`jpsj`](https://ctan.org/pkg/jpsj) | pLaTeX/EPS、英文誌class | EPS、物理数式、Times系、二段組 |

土木学会classの配布条件は複製・改変・再配布を許さないため、repositoryへvendorしない。試験時に
公式URLから取得し、version、取得日、SHA-256を固定する。他の学会・大学templateも個別licenseを
確認し、更新を同じ名前で上書きしない。

## package群

### 日本語class・font

- [`jsclasses`](https://ctan.org/pkg/jsclasses)、[`BXjscls`](https://github.com/zr-tex8r/BXjscls)
- [`LuaTeX-ja`](https://ctan.org/pkg/luatexja)、`ltjsarticle/ltjsbook`
- [`plautopatch`](https://ctan.org/pkg/plautopatch)
- [`japanese-otf`](https://ctan.org/pkg/japanese-otf)、[`pxchfon`](https://ctan.org/pkg/pxchfon)
- [`pxjahyper`](https://ctan.org/pkg/pxjahyper)、[`jlreq-deluxe`](https://ctan.org/pkg/jlreq-deluxe)
- [`pxrubrica`](https://ctan.org/pkg/pxrubrica)

`japanese-otf`はpTeX向けVF/support fileであり、native OTF shapingではない。`pxchfon`もdvipdfmxの
font map変更なので、PraTeX直接PDF・native OTFとは別profileで扱う。

### 数式・理工系

- [`amsmath`](https://ctan.org/pkg/amsmath)、`amssymb`、`amsthm`、[`mathtools`](https://ctan.org/pkg/mathtools)
- `bm`、[`newtxtext/newtxmath`](https://www.ctan.org/pkg/newtx)
- [`siunitx`](https://ctan.org/pkg/siunitx)、[`mhchem`](https://ctan.org/pkg/mhchem)
- [`unicode-math`](https://ctan.org/pkg/unicode-math)

IEICE・土木学会は`newtx*`を指定する一方、旧templateには`times/mathptmx`が残る。roman、italic、
bold、sans、mono、数式fontを別々に検査する。

### PDF・図・参照

- [`graphicx`](https://ctan.org/pkg/graphicx)、`xcolor`
- [`hyperref`](https://ctan.org/pkg/hyperref)、`bookmark`
- `zref`、`cleveref`、`geometry`、`caption/subcaption`

p/upLaTeXではdriver option、DVI special、dvipdfmxの解釈までが契約である。直接PDFではPDF object、
destination、bookmark、metadata、ToUnicodeを検査する。

### 文献

学会templateではpBibTeX/upBibTeXと指定`.bst`が依然中心である。学位論文では
[`biblatex`+Biber](https://ctan.org/tex-archive/macros/latex/contrib/biblatex)も使われる。和欧著者名、
読み順、URL/DOI、複数回引用、`.aux/.bbl`往復を試験する。学会指定bstを別方式で代替して
「互換」としない。

### 図・program

- [`PGF/TikZ`](https://ctan.org/pkg/pgf)、[`pgfplots`](https://ctan.org/pkg/pgfplots)
- [`listings`](https://ctan.org/pkg/listings)、[`minted`](https://ctan.org/pkg/minted)

TikZはdriver、color、transparency、PDF objectを広く踏む。mintedは外部processとcacheを必要とするため、
core互換gateから分離する。

### 人文系

PXrubrica、jlreq割注、`endnotes`、`imakeidx + mendex/upmendex`、`biblatex + csquotes`を対象にする。
言語学fixtureにはIPA、複数script、例文番号、深い脚注を含める。

## fontspec上位互換に関係する既存面

LuaTeX-jaのfontspec接続には次の利用者向け機能がある。

- `\jfontspec`
- `\setmainjfont`、`\setsansjfont`、`\setmonojfont`
- `\newjfontfamily`、`\newjfontface`
- `\defaultjfontfeatures`、`\addjfontfeatures`
- `AltFont={Range=..., Font=...}`
- `\ltjdeclarealtfont` / `\DeclareAlternateKanjiFont`
- `YokoFeatures`、`TateFeatures`、`TateFont`

PraTeXはこれらを互換目標に含め、exact code point、Unicode範囲・面、routing class、IVS、fallbackを
同じcompiled tableへ拡張する。詳細は[`../../opentype-package-roadmap.md`](../../opentype-package-roadmap.md)。

## preprint

[Jxivのguideline](https://jxiv.jst.go.jp/jxiv_docs/ja/Jxiv_guidelines_ja.html)は専用様式を要求せず、
日本語/英語を問わずtext抽出可能な単一PDF、20 MiB以下を要求する。PraTeX PDF backendのP0 profileに
できる。

[arXivのprocessor一覧](https://info.arxiv.org/help/faq/texlive.html)にはPraTeXがなく、
[TeX sourceから別途生成したPDFの提出](https://info.arxiv.org/help/submit/index.html)も通常の代替には
ならない。arXiv側がprocessorを追加する前にPraTeXを提出engine互換と称さない。

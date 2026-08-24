# 日本の小説・エッセイ・文芸書で使われるLaTeX

更新: 2026-08-24

## 基準系

新規の縦書き小説・文芸書は次の二本を基準にする。

1. `jlreq + upLaTeX + dvipdfmx`
   - pLaTeX系package、DVI driver依存、既存同人誌templateとの互換性を広く測る。
2. `jlreq + LuaLaTeX + LuaTeX-ja`
   - 直接PDF、OpenType、fontspec、現代的な日本語font選択を測る。

旧文書用に`utbook/tbook + plext`と`ltjtbook + lltjext`を別profileとして残す。横書き一般書には
`jsbook`と`BXjscls`を加える。TeX--XeT対応をXeTeX engine互換とみなさない。

## class・engine

| 構成 | 実務上の位置 | 主な互換点 |
|---|---|---|
| [`jlreq`](https://ctan.org/pkg/jlreq) + upLaTeX + dvipdfmx | 縦書き新規文書の基準 | `tate,book,twoside`、版面、扉、柱、ノンブル、割注、縦中横、右綴じ |
| `jlreq` + LuaLaTeX + LuaTeX-ja | native font・直接PDFの基準 | upLaTeXとの改行/禁則/方向比較、Unicode、font fallback、PDF resource |
| `utbook/utarticle + plext` | Unicode化された従来型縦組 | `\tate`、`\rensuji`、方向付きbox/table、傍線・傍点 |
| `ltjtbook/ltjtarticle + lltjext` | pTeX系公開APIのLua側 | plext相当の方向切替、box/table内の縦組 |
| `tbook/tarticle` | 既刊・旧template | 8-bit和文token、既存DVI package |
| [`jsclasses`](https://ctan.org/pkg/jsclasses) / [`BXjscls`](https://ctan.org/pkg/bxjscls) | 横書き一般書・電子書籍 | 横書きbookのpage style、font、PDF |

## 文芸組版機能とpackage

| 優先 | 用途 | class/package | 検査点 |
|---|---|---|---|
| P0 | 版面 | jlreqの`line_length`、`number_of_lines`、`gutter`、`head_space`、`foot_space` | 縦組の行長・行数、ノド、天・地、見開きbaseline |
| P0 | 扉・章見出し | `\NewTobiraHeading`、`\NewBlockHeading`、`\ModifyHeading` | 改頁、右左page開始、白頁、ノンブル抑止 |
| P0 | 柱・ノンブル | jlreq page style | 右綴じ、奇偶頁、前付/本文/後付、first/last mark |
| P0 | 縦中横 | `\tatechuyoko`、旧`\rensuji` | 1–3桁、英字、括弧、行頭末、box内 |
| P0 | 割注 | `\warichu` | 自動/手動分割、禁則、本文複数行、column/page境界 |
| P0 | 方向付きbox/table | `plext` / `lltjext` | parbox、minipage、tabular、傍線・傍点、方向の入れ子 |
| P1 | ルビ・圏点 | [`PXrubrica`](https://ctan.org/pkg/pxrubrica) | mono/group/jukugo/両側、行分割、進入、圏点、縦横 |
| P1 | Lua native ruby | [`luatexja-ruby`](https://tug.ctan.org/macros/luatex/generic/luatexja/doc/luatexja-ruby.pdf) | 熟語分割、複数回run、`.ltjruby`安定性 |
| P1 | p/up OpenType資産 | [`jlreq-deluxe`](https://ctan.org/pkg/jlreq-deluxe)、[`pxchfon`](https://ctan.org/pkg/pxchfon)、[`japanese-otf`](https://ctan.org/pkg/japanese-otf) | JFM/VF置換による版面変化、weight、CID/AJ文字 |
| P1 | Lua OpenType | `fontspec + luatexja-fontspec/preset` | main/sans/mono和文、縦用feature、和欧relation、fallback |
| P1 | 日本語bookmark | `hyperref + pxjahyper`（p/up） | Unicode bookmark、内部link、R2L/TwoPageRight。Luaではpxjahyperを使わない |
| P1 | トンボ・塗足し | [`jlreq-trimmarks`](https://tug.org/docs/latex/jlreq/jlreq-trimmarks-ja.html) | Media/Trim/BleedBox、3 mm bleed、class tombowとの重複 |
| P2 | 画像 | [`graphicx`](https://ctan.org/pkg/graphicx)、[`pdfpages`](https://ctan.org/pkg/pdfpages)、[`pxtatescale`](https://ctan.org/pkg/pxtatescale) | 挿絵、口絵、全面画像、回転、clip、縦組float |
| P2 | 色 | [`xcolor`](https://ctan.org/pkg/xcolor) | gray/CMYK指定と実PDF色空間を分ける |
| P2 | patch | [`plautopatch`](https://ctan.org/pkg/plautopatch) | p/up固有patchの自動load |
| P2 | 旧トンボ | [`gentombow`](https://ctan.org/pkg/gentombow) | 既存入稿文書 |
| P2 | 物理紙面 | [`bxpapersize`](https://ctan.org/pkg/bxpapersize)、[`bxpdfver`](https://ctan.org/pkg/bxpdfver) | layout寸法とPDF寸法、version/圧縮 |
| P2 | 索引 | `makeidx + upmendex` | 読み、Unicode見出し、縦組page番号 |
| P2 | 二段組 | `jlreq[twocolumn]`、`nidanfloat` | 段抜き、最終頁、float、白頁 |

`jlreq`と旧`otf`を単純に重ねるとjlreq用VF/JFMが置換され、字詰め・版面が変わり得る。
`jlreq-deluxe`を優先したprofileを持つ。packageをloadできるだけで合格にしない。

## 公開template・corpus候補

- [fnshr/latex-templatesの縦書き書籍template](https://github.com/fnshr/latex-templates/blob/master/tate-book-template.tex)
  - upLaTeX+dvipdfmx、jlreq、PXrubrica、画像、色、扉、章見出し、hyperref+pxjahyper。
- [Cloud LaTeX「B6サイズ・縦書き小説本」](https://cloudlatex.io/templates/245?locale=ja)
  - LuaLaTeX+jlreqの現代的入口。
- [B6縦書き小説templateの解説](https://adbird.hatenablog.com/entry/2022/06/19/170525)
  - jlreq、PXrubrica、lltjext、graphicx、扉、目次、章見出し、挿絵。
- [A5縦書き二段組の実例](https://adbird.hatenablog.com/entry/2022/05/08/130208)
  - 段間、全面挿絵、pdfpages/afterpage、output routine。
- [pLaTeXによる実刊行同人誌の構成](https://ankokudan.org/d/dl/pdf/pdf-platex.pdf)
  - tarticle、multicol、PXrubrica、graphicx、奥付を含む旧互換corpus。
- [Overleaf jlreq日本語template](https://www.overleaf.com/latex/templates/ri-ben-yu-japanese-jlreq/jjkkyvjjvvgk)
  - LuaLaTeX系の最小smoke。
- [Re:VIEWのjlreq対応資料](https://review-knowledge-ja.readthedocs.io/ja/latest/latex/review3-latex.html)
  - authoring system全体との後段適合試験。

第三者templateはlicenseを確認し、取得限定ならrepositoryへvendorしない。

## 小説固有の校正・authoring

一太郎等が実用化している次を、組版coreとは別にLSP/toolingの対象にする。

- 会話文と地の文の区別、段落字下げ
- 二倍ダーシ、三点リーダー、閉じ括弧、句読点、表記揺れ
- 字数目標、outline、章・scene navigation
- ruby、IVS/異体字、font routingのinspector
- 縦組previewのhit-testとsource spanへの逆対応
- comment、変更提案、accept/reject、版間diff

これらをTeXの用途外とせず、再現可能な規則組版の上に対話層を接続する。

## 入稿PDF

印刷所ごとに条件が違うため、package一個の成功を入稿適合とみなさない。例として、
[しまや出版](https://www.shimaya.net/howto/pdf.html)、
[PICO](https://www.pico-net.com/doujinshi/doujinshi_manual/letterbody/013.html)、
[栄光](https://www.eikou.com/qa/answer.cfm?id=64)、
[プリペラ](https://pripela.com/user_data/document)はfont埋込み、塗足し、PDF/X等について異なる案内を持つ。

最終gateは印刷所profileごとに、PDF version、仕上り寸法、Trim/BleedBox、font埋込み/subset、
色空間、透明、page数・順序、右綴じを検査する。

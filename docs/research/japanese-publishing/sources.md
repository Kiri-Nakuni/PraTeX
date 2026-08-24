# 一次資料索引

確認日: 2026-08-24

URLは調査時点の入口である。実際の互換gateでは取得したfileのversion/revision、取得日時、
SHA-256、licenseを別manifestに固定する。

## 日本語組版・class

| 資料 | 用途 |
|---|---|
| [W3C JLReq](https://www.w3.org/TR/jlreq/?lang=ja) | 日本語横縦組、文字class、禁則、ruby、圏点、縦中横、割注、版面 |
| [jlreq CTAN](https://ctan.org/pkg/jlreq) | jlreq class/package配布、manual、version |
| [jlreq日本語README](https://github.com/abenori/jlreq/blob/master/README-ja.md) | engine、option、class機能 |
| [jlreq-trimmarks](https://tug.org/docs/latex/jlreq/jlreq-trimmarks-ja.html) | トンボ、塗足し、PDF box |
| [PXrubrica CTAN](https://ctan.org/pkg/pxrubrica) | pTeX系ruby・圏点 |
| [PXrubrica manual](https://mirrors.ctan.org/macros/jptex/latex/pxrubrica/pxrubrica.pdf) | mono/group/jukugo ruby、分割、進入 |
| [LuaTeX-ja CTAN](https://ctan.org/pkg/luatexja) | Lua系日本語組版、font、方向 |
| [luatexja-ruby manual](https://tug.ctan.org/macros/luatex/generic/luatexja/doc/luatexja-ruby.pdf) | Lua native ruby |
| [jsclasses](https://ctan.org/pkg/jsclasses) | 日本語article/book class |
| [BXjscls](https://github.com/zr-tex8r/BXjscls) | 複数engine日本語class |
| [uplatex](https://github.com/texjporg/uplatex) | upLaTeX format/class公開資料 |
| [japanese-otf](https://ctan.org/pkg/japanese-otf) | pTeX VF/CID系OTF package |
| [jlreq-deluxe](https://ctan.org/pkg/jlreq-deluxe) | jlreqと多書体の接続 |
| [pxchfon](https://ctan.org/pkg/pxchfon) | dvipdfmx font map変更 |
| [pxjahyper](https://ctan.org/pkg/pxjahyper) | p/upLaTeX日本語PDF文字列 |

## 学会・大学

| 資料 | 用途 |
|---|---|
| [情報処理学会投稿style](https://www.ipsj.or.jp/journal/submit/style.html) | `ipsj.cls` |
| [情報処理学会研究報告](https://www.ipsj.or.jp/kenkyukai/genko.html) | PDF提出要件 |
| [電子情報通信学会TeX配布](https://www.ieice.org/ftp/) | `ieicej.cls` |
| [IEICE執筆要領](https://www.ieice.org/jpn/shiori/iss_3.html) | encoding、class/package制約 |
| [土木学会class](https://appmech-jsce.com/jam/%E5%9C%9F%E6%9C%A8%E5%AD%A6%E4%BC%9A%E8%AB%96%E6%96%87%E9%9B%86%E3%81%AElatex%E3%82%AF%E3%83%A9%E3%82%B9%E3%83%95%E3%82%A1%E3%82%A4%E3%83%AB/) | up/Lua両対応class |
| [北海道大学学位論文](https://www.ist.hokudai.ac.jp/div/ssi/EduThesisTemplate.html) | Lua/up/Xe長文template |
| [京都大学言語学研究](https://ceschi.bun.kyoto-u.ac.jp/archive/bun_archive/linguistics/lin-kulr/index.htm) | LuaLaTeX人文系template |
| [日本数学会TeX style](https://math.or.jp/meeting/texstyle/howto_ja.pdf) | p/up/Lua数理論文 |
| [人工知能学会大会](https://conf.ai-gakkai.or.jp/jsai2026/author-guideline/) | 旧pLaTeX系二段組 |
| [日本機械学会投稿](https://www.jsme.or.jp/publish/transact/for-authors.html) | Times系/DVI template |
| [東京大学CS学位論文](https://www.i.u-tokyo.ac.jp/edu/course/cs/thesis.shtml) | pLaTeX長文class |
| [Jxiv guideline](https://jxiv.jst.go.jp/jxiv_docs/ja/Jxiv_guidelines_ja.html) | text抽出可能な単一PDF |
| [arXiv TeX processor](https://info.arxiv.org/help/faq/texlive.html) | 受理engine一覧 |

## 一般LaTeX package

| 分野 | 資料 |
|---|---|
| font | [fontspec](https://ctan.org/pkg/fontspec)、[newtx](https://www.ctan.org/pkg/newtx)、[unicode-math](https://ctan.org/pkg/unicode-math) |
| 数式・科学 | [amsmath](https://ctan.org/pkg/amsmath)、[mathtools](https://ctan.org/pkg/mathtools)、[siunitx](https://ctan.org/pkg/siunitx)、[mhchem](https://ctan.org/pkg/mhchem) |
| PDF・図 | [graphicx](https://ctan.org/pkg/graphicx)、[xcolor](https://ctan.org/pkg/xcolor)、[hyperref](https://ctan.org/pkg/hyperref)、[PGF/TikZ](https://ctan.org/pkg/pgf) |
| 文献 | [biblatex](https://ctan.org/tex-archive/macros/latex/contrib/biblatex) |
| program | [listings](https://ctan.org/pkg/listings)、[minted](https://ctan.org/pkg/minted) |
| 段・回込み | [multicol](https://ctan.org/pkg/multicol/)、[wrapfig](https://ctan.org/pkg/wrapfig)、[wrapstuff](https://ctan.org/pkg/wrapstuff) |
| 印刷・色 | [pdfx](https://ctan.org/pkg/pdfx)、[colorspace](https://ctan.org/pkg/colorspace)、[spotxcolor](https://ctan.org/pkg/spotxcolor)、[colorprofiles](https://ctan.org/pkg/colorprofiles) |
| EPUB | [tex4ebook](https://ctan.org/pkg/tex4ebook)、[make4ht](https://ctan.org/pkg/make4ht)、[TeX4ht](https://ctan.org/pkg/tex4ht) |

## DTP・word processor

| 資料 | 調査対象 |
|---|---|
| [Adobe CJK composition](https://helpx.adobe.com/jp/indesign/using/composing-cjk-characters.html) | 日本語composer |
| [Adobe mojikumi](https://helpx.adobe.com/indesign/desktop/language-and-proofing/chinese-japanese-and-korean/customize-mojikumi-spacing-sets.html) | 文字組preset |
| [Adobe kinsoku](https://helpx.adobe.com/indesign/desktop/language-and-proofing/chinese-japanese-and-korean/use-kinsoku-settings.html) | 禁則・追込み |
| [Adobe preflight](https://helpx.adobe.com/indesign/desktop/print/preflight/configure-and-use-the-preflight-panel.html) | 常時preflight |
| [Adobe PDF export](https://helpx.adobe.com/indesign/desktop/save-export-and-publish/save-and-export/adobe-pdf-export-options.html) | PDF/X、box、font、色 |
| [Affinity vertical text statement](https://support.serif.com/hc/en-us/articles/10259496602895-Does-Affinity-V2-support-Right-to-Left-or-Vertical-text) | RTL/縦組制約 |
| [Affinity preflight](https://affinity.help/publisher2/English.lproj/pages/Publishing/preflight.html) | 一般DTP preflight |
| [Word Track Changes](https://support.microsoft.com/en-us/word/training/track-changes-in-word) | 変更履歴 |
| [Word ruby](https://learn.microsoft.com/en-us/globalization/fonts-layout/ruby) | 東アジアruby |
| [Word OOXML kinsoku](https://learn.microsoft.com/ja-jp/openspecs/office_standards/ms-oe376/1ed6a072-e2ec-4b71-a42d-20f007bd097d) | 禁則document model |
| [一太郎小説機能](https://www.justsystems.com/jp/products/ichitaro/features/feature03.html) | 小説preset・校正・構造 |
| [一太郎校正/縦組](https://www.justsystems.com/jp/products/ichitaro/features/feature02.html) | authoring workflow |
| [一太郎印刷/PDF](https://www.justsystems.com/jp/products/ichitaro/features/feature05.html) | POD・入稿 |

## 標準・検証

| 資料 | 用途 |
|---|---|
| [JAGAT「印刷用PDFの作り方 2026」](https://www.jagat.or.jp/archives/82309) | PDF/Xとpreflight |
| [LaTeX tagging project](https://latex3.github.io/tagging-project/documentation/) | tagged PDF/PDF/UA |
| [tagging package status](https://latex3.github.io/tagging-project/tagging-status/) | package別tag対応 |
| [EPUB 3.3](https://www.w3.org/TR/epub-33/) | EPUB構造 |
| [EPUB Accessibility 1.1](https://www.w3.org/TR/epub-a11y-11/) | accessibility |
| [EPUBCheck](https://www.w3.org/publishing/epubcheck/) | EPUB validator |

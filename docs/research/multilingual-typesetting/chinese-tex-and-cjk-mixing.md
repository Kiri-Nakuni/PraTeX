# 中国語TeX文化とHan-unificationを隠す和中混植

更新: 2026-08-24

## 結論

`Han`というUnicode scriptや一つのscalarだけでは、日本語、中国本土簡体字、台湾繁体字、香港繁体字の
字形・句読点・禁則・font fallbackを決められない。Unicodeは統合漢字の言語識別をhigher-level
protocolへ委ね、localeに適したfontまたはfont混合を選ぶよう求める。
[Unicode 17 Chapter 18](https://www.unicode.org/versions/Unicode17.0.0/core-spec/chapter-18/)

OpenTypeの`locl`も同じUnicode値からlanguage-system metadataに従って別glyphを選ぶ。
[OpenType `locl`](https://learn.microsoft.com/en-us/typography/opentype/spec/features_ko#tag-locl)、
[OpenType Layout](https://learn.microsoft.com/en-us/typography/opentype/spec/chapter2)

PraTeXは少なくとも次を別domainとして保持する。

- Unicode scalar、grapheme cluster、IVS、外字identity
- line break・自動空白用`ScriptClassId`
- `ja-JP`、`zh-Hans-CN`、`zh-Hant-TW`、`zh-Hant-HK`等の組版`LanguageRegion`
- 横組・縦組のwriting mode
- run localeと独立したparagraph/document主言語profile
- font route、fallback chain、OpenType script/language system、shaping feature
- JFM/Chinese metric class、catcode/kcatcode、TeX hyphenation `language`

同じHan同士でlanguage runが変わっても、通常は空白を挿入しない。font、`locl`、fallback、句読点・
圏点profileだけを必要な位置で切り替える。scalarとPDF `ToUnicode`は変えない。

## 中国本土・台湾・香港・日本を分ける

[W3C CLReq](https://www.w3.org/TR/clreq/)は中国本土・台湾・香港の横組・縦組を比較する包括的な
実装資料である。2026-08-24時点の現行snapshotは
[2026-08-04 Group Note Draft](https://www.w3.org/TR/2026/DNOTE-clreq-20260804/)であり、
Recommendationではないことも記録する。

| region | 代表的な差 | PraTeX profile |
|---|---|---|
| 中国本土 | 横組主体。横組句読点は一般に仮想body左下、縦組は右上。句読点圧縮と行末詰めを使う | `zh-Hans-CN`、本土句読点、GB禁則 |
| 台湾 | 横組・縦組とも実用。句読点は中央配置が基本で、圧縮しない出版物もある | `zh-Hant-TW`、台湾中央句読点、台湾縦組 |
| 香港 | 横組・縦組、中央句読点。HKSCS収録文字と香港字形を別に検査する | `zh-Hant-HK`、香港font/fallback、香港句読点 |
| 日本 | JLReq、和文句読点、圏点位置、和欧間隔が中国語と異なる | `ja-JP`を中国語regionへ統合しない |

中国本土の一次規則には
[GB/T 15834-2011 標点符号用法](https://openstd.samr.gov.cn/bzgk/std/newGbInfo?hcno=22EA6D162E4110E752259661E1A0D0A8)、
[GB/T 9704-2012 公文格式](https://openstd.samr.gov.cn/bzgk/std/newGbInfo?hcno=F3CC9BEF482524C895FDA7A08BB4A70E)
がある。台湾は教育部の
[重訂標點符號手冊](https://language.moe.gov.tw/001/Upload/FILES/SITE_CONTENT/M0001/HAU/haushou.htm)と
[国字標準字体資料](https://language.moe.gov.tw/result.aspx?classify_sn=23&content_sn=30&subclassify_sn=436)
をfixtureの根拠にする。香港の
[HKSCS-2016](https://www.ogcio.gov.hk/en/our_work/business/tech_promotion/ccli/terms/doc/e_hkscs_2016.pdf)
はcoded character setであってglyph標準ではないため、coverageと地域字形を同じ試験にしない。

CLReqからengineへ直結する条件は次である。

- 禁則にはnone/basic/GB/strict等の段階があり、regionだけから一意に選ばない。
- 句読点圧縮はbreak位置を変えるため、line breaking後の見た目補正にしない。
- Han--Latin間隔は約1/4 emが代表値でも、行端や句読点隣接では入れず、justify時の縮伸もある。
- CJK em frameとLatin baseline/ascenderを分け、region font切替でbaselineをjumpさせない。
- 中文paragraph中の日本語span等で圏点・引用符が主言語規約へ従う場合があるため、run localeと
  paragraph/document profileを一値へ潰さない。

## Babel adapter

Babelは`\selectlanguage`、`\foreignlanguage`、language別`\babelfont`、localeごとのspacing/line-break
transformを持つ有力な先例である。

- [Babel manual](https://mirrors.ctan.org/macros/latex/required/babel/base/babel.pdf)
- [Chinese guide](https://latex3.github.io/babel/guides/locale-chinese.html)
- [Japanese guide](https://latex3.github.io/babel/guides/locale-japanese.html)
- [Cantonese guide](https://latex3.github.io/babel/guides/locale-cantonese.html)
- [locale naming](https://latex3.github.io/babel/guides/locale-naming.html)

ただし文字による自動切替は主にscriptが変わるspanの補助で、同じHan scalarから日本語・中国語を
判定できない。[Babel 3.96 `onchar`](https://latex3.github.io/babel/news/whats-new-in-babel-3.96.html)
また、Babelのgeneric spacingも地域差を自動的に解消する普遍規則ではない。台湾・香港の中央句読点を
区別してきた経緯は[Babel 24.9](https://latex3.github.io/babel/news/whats-new-in-babel-24.9.html)と
[Babel 25.6](https://latex3.github.io/babel/news/whats-new-in-babel-25.6.html)を追う。

PraTeX adapterは次を行う。

1. `selectlanguage` / `foreignlanguage`相当をtyped `LanguageRegion` runのpush/popへ写す。
2. `babelfont[locale]`相当をregion-scoped font family/fallback tableへcompileする。
3. locale transformをspacing、punctuation、kinsoku profileへ明示的に写す。
4. `onchar`はLatin--Han等のscript変更に限り、Han内部のlocale推定には使わない。
5. BabelのTeX `language`、PraTeX `LanguageRegion`、paragraph主言語、OpenType tagを別に復元する。

## OpenType language systemとfont fixture

[OpenType language-system tag registry](https://learn.microsoft.com/en-us/typography/opentype/spec/languagetags)
の代表的な対応は次である。

| PraTeX locale | OpenType tag |
|---|---|
| `ja-JP` | `JAN ` |
| `zh-Hans-CN` | `ZHS ` |
| `zh-Hant-TW` | `ZHT ` |
| `zh-Hant-HK` | `ZHH ` |
| `zh-Hant-MO` | `ZHTM` |

OpenType tagの粒度はBCP 47や組版profileと一致しないため、`LanguageRegion`をOpenType tagそのものに
しない。hostがfontの公開language systemを検査して変換する。

Adobeの[`locl-test`](https://github.com/adobe-fonts/locl-test)では、同じU+904D「遍」をJP/KR/CN/TW/HKで
別glyphにする。[`source-locl-test`](https://github.com/adobe-fonts/source-locl-test)はより広いfixture候補で
ある。[Source Han Sans](https://github.com/adobe-fonts/source-han-sans)と
[Source Han Serif](https://github.com/adobe-fonts/source-han-serif)はregion別fontとPan-CJK collectionを持つ。
試験ではrelease、font file、face index、hashを固定し、OS fallbackへ依存しない。

IVSはlocale選択の代用ではない。特定glyph identityをplain textで保持する時だけbase ideographと
variation selectorの列として使う。
[UTS #37](https://www.unicode.org/reports/tr37/)、[IVD Registry](https://www.unicode.org/ivd/)

## 中国語LaTeX ecosystem

### engine/class/packageの基線

| priority | class/package | 主な互換面 |
|---|---|---|
| P0/P1 | [CTeX bundle](https://ctan.org/pkg/ctex) | `ctexart/rep/book/beamer`、heading、字号、日付・数字、fontset、句読点、CJK--Latin glue。固定fontと`fontset=none`を基準にする |
| P0 | [xeCJK](https://ctan.org/pkg/xecjk) | CJK/Latin font、文字block/scalar/range routing、多段fallback、句読点圧縮、interchar glue |
| P0 | [LuaTeX-ja](https://ctan.org/pkg/luatexja) | JFM、縦組、alternate Kanji fontのscalar/range routing先例 |
| P0 | [ChineseJFM](https://ctan.org/pkg/chinese-jfm) | `zh_CN`、`zh_TW`、`ja_JP` JFM、全角/半角/開明/縦組の差 |
| P1/P2 | [LuaTeX-CN](https://ctan.org/pkg/luatex-cn) | 現代中文横縦組、古籍、割注、圏点、拼音、段階的禁則の高度oracle |
| P2 | [CJK bundle](https://ctan.org/pkg/cjk) | pdfLaTeX系legacy 8-bit/subfont文書。OTF-nativeとは別profile |

### 実用文書

- 学位論文: [ThuThesis](https://ctan.org/pkg/thuthesis)、[fduthesis](https://ctan.org/pkg/fduthesis)、
  [NJUThesis](https://ctan.org/pkg/njuthesis)、[SJTUTeX](https://ctan.org/pkg/sjtutex)、
  [USTCThesis](https://github.com/ustctug/ustcthesis)。大学提出規程とcommunity classの権威を分ける。
- 台湾fixture: [NTU community template](https://github.com/Hsins/NTU-Thesis-LaTeX-Template)と
  [NTU Library guide](https://www.lib.ntu.edu.tw/doc/cl/NTUTDR_Guide.pdf)を別々に照合する。
- 公文: [gbt9704](https://ctan.org/pkg/gbt9704)。
- 文献: [GB/T 7714-2025](https://openstd.samr.gov.cn/bzgk/std/newGbInfo?hcno=C6CE52E55AC09B9C79A20AEA77CEDD14)、
  [gbt7714](https://ctan.org/pkg/gbt7714)、[biblatex-gb7714-2015](https://ctan.org/pkg/biblatex-gb7714-2015)。
- 書籍等: [easybook](https://ctan.org/pkg/easybook)、[zhlineskip](https://ctan.org/pkg/zhlineskip)、
  [xpinyin](https://ctan.org/pkg/xpinyin)、[zhnumber](https://ctan.org/pkg/zhnumber)、
  [hanzibox](https://ctan.org/pkg/hanzibox)、[beaulivre](https://ctan.org/pkg/beaulivre)。

## engine不変条件

### routingとcache

候補順位は次を基線にする。

1. region付きscalar/IVS/range/routing-class override
2. region付きfamily role
3. 同region fallback chain
4. 文書既定の同region fallback
5. 異region fallbackは明示opt-inだけ。使用時は診断する

HK primaryが欠くglyphをglobal CN fontへ黙って落とさない。fallback先でも元script、language tag、feature、
writing modeを保って再shapeする。

- route cache: `cluster/scalar sequence + LanguageRegion + font role + writing mode + override/fallback generation`
- shape cache: `font file identity/hash + face index + size + variation + direction + script + OTL language + features + original scalar sequence`

`(font, scalar)`だけのcacheでは、最初にshapeしたJP glyphがCN/TW/HKへ汚染される。

### RustyBuzzとPDF

`LanguageRegion`とOpenType languageはRustyBuzzの有無に関係なくdomain modelへ残す。region別font fileなら
shaperなしでも地域字形を選べるが、Pan-CJK fontの`locl`依存経路はshaping capabilityなしで保証できない。
silent default glyphへ落とさず、能力不足を診断する。縦組では`vert/vrt2`も同じshape planへ渡す。

`locl`やfallbackでGIDが変わってもPDF `ToUnicode`は元scalar列へ戻す。同じU+904DのJP/CN/TW/HK
glyphはすべてU+904D、supplementary HanはUTF-16BE pair、IVSはbase+VSである。subset code、CID、glyph name
からUnicodeを推測しない。viewer依存の非埋込みCID fontは地域字形のcompletion gateに使わない。

## 最小fixture matrix

### P0 engine

1. Adobe `locl-test`のU+904Dをja-JP、zh-Hans-CN、zh-Hant-TW、zh-Hant-HKでshapeする。glyph/outlineは
   region別、PDF抽出は全てU+904D。
2. 同一font/sizeでJP→CN→TW→HK→JPとshapeし、route/shape cache汚染がない。
3. HK primaryからglyphを欠落させ、global CNよりHK fallbackを優先する。異regionだけなら診断する。
4. 同じ句読点列をCN/TW/HK、横縦、狭い行幅で組み、位置、圧縮、禁則、break位置を比較する。
5. 日本語paragraph中の中文spanと逆向きを組み、Han--Han region境界へ自動空白を入れずfont/`locl`だけを
   切り替える。run localeとparagraph主言語の句読点・圏点契約も検査する。
6. `中文OpenType测试`と`日本語OpenType試験`でHan--Latin間隔、句読点隣接、行端、明示spaceとの
   二重glueを検査する。
7. subset、supplementary Han、IVS、`locl`を含むPDFの抽出UTF-8列をsourceと完全比較する。

### P1 package/API

8. Babelの持続/局所span、入れ子復元、region別font、同じHan script内の明示切替。
9. xeCJKのscalar/range block、多段fallback、句読点style。
10. ChineseJFMの`zh_CN` / `zh_TW` / `ja_JP`、alternate Kanji font、横縦組。
11. CTeXを固定fontsetで通し、heading、字号、font role、punctuation、CJK--Latin glueを検査する。

### P2 ecosystem

12. ThuThesis、fduthesis、NJUThesis、SJTUTeXの最小学位論文。
13. `gbt7714` / `biblatex-gb7714-2015`の中文・英文混在文献。
14. `gbt9704`の一頁公文。
15. LuaTeX-CNの現代縦組、古籍、割注・双行注。

package互換の最終判定はTeX--XeTを含むe-TeX、upTeX上位互換、LuaTeX級PDF、OTF経路の完成後に
行う。それ以前にもP0 font/data hash、locale→OpenType tag表、最小入力、期待glyph/spacing/抽出列は固定する。

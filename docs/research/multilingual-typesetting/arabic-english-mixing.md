# Arabic--English混植とPraTeXの将来境界

更新: 2026-08-24

## 結論

TeX--XeTは`\beginL` / `\endL`、`\beginR` / `\endR`による明示LR/RL区間と、改行後の方向nodeを
扱うe-TeX互換層である。Unicode Bidirectional Algorithm、Arabic shaping、font fallbackを代行しない。
PraTeX 1ではTeX--XeTを完全実装するが、Arabic完成をrelease gateへ追加しない。今は将来のUBA、
shaping、line breaking、PDF logical orderを相互に代用せず接続できる型とtestを保つ。

## 責任分界

| 層 | 責任 | 責任外 |
|---|---|---|
| TeX--XeT | 明示LR/RL node、lineごとの反転、DVI/PDF方向出力、LTR mathの隔離と方向復帰 | Unicode bidi class、isolate、paired bracket、Arabic shaping |
| Unicode bidi | logical textからparagraph level、isolating run、数字・中立文字・括弧を含むresolved levelを決め、改行後にlineのvisual orderを決める | shaping、fallback、break選択、justification |
| OpenType shaper | direction/script/language/fontが一様なrunからglyph、advance、offset、clusterを作る | bidi、fallback、line/page breaking |
| TeX paragraph builder | break候補、glue、penaltyから行を選び、paragraph/pageを最適化する | UBAやjoiningを暗黙に推測すること |
| PDF backend | glyph、logical scalar range、ToUnicode、tag、reading orderを出す | 見た目だけからlogical orderを復元すること |

根拠は[e-TeX manual](https://mirrors.ctan.org/systems/doc/etex/etex_man.pdf)、
[UAX #9](https://www.unicode.org/reports/tr9/)、
[HarfBuzz shaping concepts](https://harfbuzz.github.io/shaping-concepts.html)、
[HarfBuzzが扱わない処理](https://harfbuzz.github.io/what-harfbuzz-doesnt-do.html)、
[UAX #14](https://www.unicode.org/reports/tr14/)を使う。

shaping済みRTL runをTeX--XeT finalizerでも反転すると二重反転になる。`ShapedRun`のglyph traversalと
LR finalizerのどちらが一度だけvisual化するかを公開契約にする。

## 現行LaTeXの実用profile

### LuaLaTeX + Babel

BabelのArabic guideはLuaTeX、`bidi=basic`、言語区間別`\babelfont`を中心に説明する。Arabicの
font routeではHarfBuzz rendererを利用できる。ただし
`bidi=basic`をUAX #9完全準拠と仮定せず、Unicode公式conformance dataで別に検査する。

- [Babel Arabic guide](https://latex3.github.io/babel/guides/locale-arabic.html)
- [Babel manual](https://tug.ctan.org/language/babel/base/babel.pdf)
- [Babel 24.14 news](https://latex3.github.io/babel/news/whats-new-in-babel-24.14.html)

### XeLaTeX/LuaLaTeX + polyglossia + fontspec

PolyglossiaはBCP 47 language、地域別counter、言語別fontを提供し、fontspecでArabic script/languageを
選ぶ。方向処理はXeTeXの`bidi`またはLuaTeX側の方向packageと組み合わせる。`bidi`がfootnote、list、
column、table、equation number、header/footerを個別に扱うことは、UBAだけではdocument structureを
完成できないことも示す。

- [polyglossia](https://ctan.org/pkg/polyglossia)
- [polyglossia manual](https://tug.ctan.org/macros/unicodetex/latex/polyglossia/doc/polyglossia.pdf)
- [fontspec](https://ctan.org/pkg/fontspec)
- [bidi manual](https://mirrors.ctan.org/macros/unicodetex/generic/bidi/bidi-doc.pdf)

Legacyまたは用途特化の比較対象には
[ArabTeX](https://ctan.org/pkg/arabtex)、[arabi](https://ctan.org/pkg/arabi)、
[ArabXeTeX](https://ctan.org/pkg/arabxetex)、[arabluatex](https://ctan.org/pkg/arabluatex)、
[arabic-book](https://ctan.org/pkg/arabic-book)、[texnegar](https://ctan.org/pkg/texnegar)がある。
互換性試験ではUnicode新規文書、transliteration、critical edition、kashida等の用途を混ぜない。

## 必要な組版規則

### logical order、数字、句読点

sourceはArabic presentation formやvisual orderでなくsemantic Unicodeのlogical orderで保持する。
Arabic paragraph中のEnglishはLTR run、LTR paragraph中のArabicはRTL runとする。動的に埋め込む
引用、URL、著者名等はisolateの意味を保つ。

数字列はArabic文中でもLTRになり、ASCII/Eastern Arabic-Indic digitとArabic-Indic digitでは
bidi classが異なる。括弧のsource codepointを入れ替えず、paired-bracket ruleとmirroringで
表示glyphだけを選ぶ。根拠は[W3C ALReq](https://www.w3.org/TR/alreq/)、
[UAX #9](https://www.unicode.org/reports/tr9/)、
[BidiBrackets.txt](https://www.unicode.org/Public/17.0.0/ucd/BidiBrackets.txt)を使う。

### shaping、joining、fallback

Arabic shapingはjoining state、ligature、language-specific form、cursive attachment、mark positioningを
含む。OpenTypeの`isol/init/medi/fina`、`rlig`、`locl`、`curs`、`mark`、`mkmk`等をfontの公開tableに
従って使う。ZWNJはjoiningを切り、ZWJはjoiningを促し、可視Tatweel U+0640で代用しない。

fallbackはshaperの責任ではない。joining segmentまたはbase+mark clusterの途中でfontを切ると
接続とanchorを失うため、最小契約はsegment全体を覆う一fontを選び、無ければ明示診断する。

- [OpenType Arabic development](https://learn.microsoft.com/en-us/typography/script-development/arabic)
- [ArabicShaping.txt](https://www.unicode.org/Public/17.0.0/ucd/ArabicShaping.txt)
- [HarfBuzz cluster](https://harfbuzz.github.io/clusters.html)

### 改行とjustification

break候補はUAX #14、language policy、cluster safetyから作り、TeX paragraph builderが組合せを選ぶ。
UBAのline ruleはbreak決定後に一度だけ適用する。Arabic justificationはword space、alternate、
ligature、kashida等の優先度が地域・書体・styleに依存する。固定Tatweelをsourceへ挿入する実装は
採らない。高度なkashidaは通常interword glue完成後のoptional profileとし、候補、上限、再shaping、
badnessをparagraph側で管理する。

## 将来pipeline

1. semantic Unicodeと著者指定bidi/joining controlをlogical orderで保持する。
2. higher-level protocolからparagraph base directionを決める。
3. optional UBA stageがembedding levelとisolating runを解決する。
4. `bidi level × ScriptClassId × LanguageRegion × font/style`でitemizeする。
5. cluster/joining segmentを壊さないfont fallback後、一様runをlogical orderでshapeする。
6. UAX #14、cluster safety、language ruleからbreak候補を作り、TeXが行を選ぶ。
7. 行端でjoining contextが変わるrunはBOT/EOTを付けて再shapeする。
8. UBAのline reorderを一度だけ行う。
9. glyphとlogical scalar range、cluster map、language、tag structureをbackendへ渡す。

`InputCategory`、`ScriptClassId`、Unicode `Bidi_Class`、paragraph direction/level、
`LanguageRegion`、font route、shaper glyph/cluster IDを別domainにする。`Arab` scriptだけから`ar`、`fa`、
`ur`を推測しない。RustyBuzzはdefault-offとし、採用時にversion/fontをpinして`hb-shape`とglyph、
advance、offset、clusterを比較する。

## 最小fixture

### TeX--XeT互換

- `\TeXXeTstate=0`で既存LTR node、DVI、PDFが不変。
- nested `R -> L -> R`、不均衡区間、box/unbox、改行を跨ぐR区間。
- RTL paragraph、LTR display math、RTL再開。
- DVIと直接PDFのvisual order一致と、shaped RTL runの反転が一回だけ。

### UBAとshaping

- Unicode 17.0の[BidiTest.txt](https://www.unicode.org/Public/17.0.0/ucd/BidiTest.txt)と
  [BidiCharacterTest.txt](https://www.unicode.org/Public/17.0.0/ucd/BidiCharacterTest.txt)をpinする。
- Arabic + `English 123`、English + Arabic + `(ABC-12)`、EN/AN、nested/unmatched brackets、
  LRI/RLI/FSI/PDI、同一paragraphの幅違い改行。
- pinしたfontで`سلام`、lam--alef、joining四位置、stacked marks、ZWNJ/ZWJ/Tatweel、`ar`/`fa`
  `locl`を`hb-shape`と比較する。
- joining segmentやbase+markの一部だけfallbackできるcaseは黙ってfontを分断しない。

### structureとPDF

- mixed-direction list、table、footnote、inline/display math、equation number。
- lam--alef glyphのToUnicodeは`U+0644 U+0627`で、presentation formにしない。
- cluster reorder後もcopy/searchはlogical sequenceを返す。
- paragraph/span language、list/table/footnote/mathのtagとlogical reading orderを検査する。
- unmatched isolate/overrideや過剰nestingをboundedに扱い、logの不可視bidi controlを`U+2067`等で
  可視化する。

見た目の成功だけではtagged PDFやPDF/UA相当を意味しない。現行package状況は
[LaTeX tagging status](https://latex3.github.io/tagging-project/tagging-status/)も別gateとして追う。

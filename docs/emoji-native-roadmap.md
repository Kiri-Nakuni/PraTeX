# PraTeX native emoji roadmap

更新: 2026-08-24

状態: **設計のみ。通常OTF対応の後に着手する。**

## 優先順位と「native」の定義

実装順はJFM/TFM基線、PDF直接出力、通常OTF/TrueType loader・metric・shaping・fallback、native絵文字
とする。絵文字のためにOTFや現在の性能調整を中断しない。

PraTeXで「native絵文字対応」と呼ぶには、少なくとも次をすべて満たす。

- 利用者がplain UTF-8の絵文字scalar/sequenceを書くだけで組版でき、絵文字ごとのTeX macroを要しない。
- VS15/VS16、modifier、regional indicator、keycap、tag、ZWJを含むsequenceを一つの壊せない
  text clusterとして保持し、その内部で改行・font分断・自動空白を行わない。
- text/emoji presentation、font route、fallback、shapingをcluster全体で決める。一scalarずつ
  「glyphがある最初のfont」へ送らない。
- monochrome outlineだけでなく、少なくとも一つのportableなcolor-font経路を直接PDFへ描画する。
- 一glyphまたは複数glyphへshapeされても、copy/search/accessible textは元のUnicode scalar列へ戻る。
- 和文・中文・欧文との境界、縦横組、baseline、line breakingをengineの通常nodeとして扱う。

名前から絵文字を引く`\emoji{...}`型packageは後から作ってよいが、それは入力支援でありnative
完成条件の代用ではない。

## 一次仕様とversion pin

[UTS #51 Unicode Emoji](https://www.unicode.org/reports/tr51/)はemoji character、VS15/VS16、modifier、
flag、tag、ZWJ sequenceとRGI集合を定義する。emoji sequenceは一つのgrapheme clusterであり、
対応する一glyphがない時に複数glyphへfallbackしてもcluster内部で分割しない。

cluster境界は[UAX #29](https://www.unicode.org/reports/tr29/)、改行禁止は
[UAX #14](https://www.unicode.org/reports/tr14/)、縦組の既定方向は
[UTR #50](https://www.unicode.org/reports/tr50/)を参照する。実装dataは「latest」を実行時取得せず、
完成済みUnicode/Emoji versionと取得SHA-256をrepositoryのgenerator manifestへ固定する。初期fixtureは
[Unicode 17.0 emoji data](https://www.unicode.org/Public/17.0.0/emoji/)を候補とし、draft dataを既定へ
混ぜない。

## 文字identityとnode境界

現在のwide glyphは概ね一scalarを一nodeへする。絵文字は複数scalarが一つのtext atomになるため、
通常OTF段階で次のbackend非依存境界を先に作る。

```text
TextCluster {
  source_scalar_range,
  scalar_or_ivs_sequence,
  grapheme_boundary,
  script_class,
  language_region,
  writing_mode,
  presentation_preference,
  font_role
}

ShapedCluster {
  text_cluster_identity,
  font_instance,
  glyphs_and_positions,
  cluster_map,
  advance_and_bounds,
  color_presentation
}
```

`presentation_preference`は少なくともtext、emoji、defaultを区別する。VS15/VS16をtokenize時に捨てず、
shaperのglyph IDやfont-local variation indexを文字identityへ逆流させない。emoji routing classを
`InputCategory`、catcode/kcatcode、JFM class、`LanguageRegion`と同じIDにしない。

任意に長いcombining sequenceを固定長bufferだけで拒まない。通常caseはsmall inline storage、長いcaseは
run-local bounded allocationまたはstreaming summaryへ移し、文書全体の入力上限と診断上限を別に持つ。

## segmentation、改行、組版locale

- UAX #29のextended grapheme clusterとUTS #51 RGI dataを同じversionで生成する。
- UAX #14とUTS #51が禁止するZWJ/modifier/flag sequence内部のbreakをline breakerへ入れない。
- 一つのemoji clusterが複数glyphへfallbackしても、TeXのdiscretionaryやhyphenationを内部へ作らない。
- 和文・中文との外側境界はbuilt-in spacing/禁則profileが一度だけ決める。emojiをHanやLatinへ偽装しない。
- `LanguageRegion`は周囲とのspacing、句読点、fontset defaultには使えるが、同じemoji sequenceから
  日本語・中国語を推定する入力にはしない。
- 縦組ではcluster全体にUTR #50のorientationとfontのvertical metricを適用し、baseだけ回転して
  modifierやZWJ要素を置き去りにしない。明示overrideはfont routingとは別にする。

## font routingとshaping

font fallbackの単位はscalarでなくclusterである。候補fontについて単純cmap coverageだけを見ず、
variation selector、GSUB、emoji sequence、必要markを含むshape結果を検証する。route cacheは少なくとも
`(scalar sequence, presentation, font role, LanguageRegion, writing mode, route generation)`をkeyにする。

通常font、text presentation emoji、color emojiのfallback chainを別に宣言できるようにする。VS15が
明示されたclusterをcolor emojiへ、VS16が明示されたclusterをtext outlineへ黙って落とさない。
完全な一glyphが無いsequenceをUTS #51どおり意味の通る複数glyphで表示するfallbackと、font欠落errorを
区別してtracingする。

RustyBuzzはshaping候補であって、font loading、fallback、color glyph painting、subsettingを担当しない。
採用時はdefault-off、version pin、license/unsafe/binary size監査、pinしたfontに対する`hb-shape`との
glyph ID・advance・offset・cluster差分試験を行う。

## color fontとPDF backend

[OpenType 1.9.1](https://learn.microsoft.com/en-us/typography/opentype/spec/otff)はcolor glyph形式として
COLR/CPAL、CBDT/CBLC、`sbix`、SVGを持つ。COLRのpaintingはtext shaping後、presentation前の別処理である。
従ってshaperの結果をそのままPDF text operatorへ書くだけではcolor glyphにならない。

対応順は次を基線にする。

1. monochrome outline emojiを通常OTF経路でshape・subset・embedする。
2. [COLR v0/v1](https://learn.microsoft.com/en-us/typography/opentype/spec/colr)と
   [CPAL](https://learn.microsoft.com/en-us/typography/opentype/spec/cpal)をbounded paint graphへparseし、
   outline、solid、gradient、transform、clip、compositeをPDF vector/shading/resourceへ写す。
3. CBDT/CBLCと[`sbix`](https://learn.microsoft.com/en-us/typography/opentype/spec/sbix)のstrikeを
   明示size/PPI policyで選び、image XObjectへする。lossless sourceを不要に再圧縮しない。
4. OpenType SVGは最後のoptional profileにする。SVG/SVGZを未検証のままPDFへ埋めず、XML、参照、script、
   resource、圧縮展開、画素・path・再帰量を制限したrenderer/translatorだけを許す。

safe Rust parser候補には`ttf-parser`等があるが、採用前に通常OTFと同じdependency監査を行う。parserが
color tableを読めることと、PDFへ同じ見た目を安全に描けることを混同しない。

PDF backendはglyph paintとlogical textを別eventで受ける。一つのemoji glyphへ複数scalarが対応する場合、
`ToUnicode`だけで十分か、marked-contentの`ActualText`も要るかをPDF version/profile別に適合試験し、
抽出列を重複させない。subset code、CID、glyph name、presentation formからUnicodeを推測しない。

## DVIとの契約

通常DVIにはCOLR paint graphやbitmap strikeのportableな意味がない。次を明示的に分ける。

- driverが解決できるTFM/VF/物理fontへ落とせるmonochrome profile
- driverとversionを固定したspecialまたは生成image/VF profile
- color nativeを保証する直接PDF profile

DVIでcolorを表せない時にVS/ZWJを捨てて別文字として出したり、glyphを無言で消したりしない。対応profileが
無ければ診断する。DVIとPDFでline width、advance、break位置が一致するかはpaintとは別に検査する。

## package、Vaak、WASM API

PraTeX-native OpenType packageはfont role、text/emoji presentation、cluster fallback、palette、foreground
colorを宣言し、全tableをhost-owned routing planへcompileする。emoji名やshortcodeのpackageはCLDR等の
version/licenseを別管理し、engine primitiveを絵文字ごとに増やさない。

標準emoji処理はBuiltInで完結し、paragraph hot loopからVaak/WASMを呼ばない。Vaak API/WASM ABIで
出版社固有routeを追加する場合も、bounded scalar-sequence tableを登録時に全検証・compileし、clusterごとの
ABI往復をしない。provider-local ID、font handle、paint cacheはfmtへ保存しない。

## 完了gate

- Unicodeの`emoji-test.txt`、`emoji-sequences.txt`、`emoji-zwj-sequences.txt`と、同versionの
  GraphemeBreakTest/LineBreakTestをmanifest・SHA-256付きで固定する。
- text/emoji presentation pair、skin tone、family/profession ZWJ、regional-indicator flag、keycap、tag flag、
  minimally-qualified/unsupported sequenceをraw UTF-8から試す。
- sequence内部のnode/break/font切替が0で、外側の和文・中文・Latin spacingが一度だけ決まる。
- pinしたmonochrome、COLR v0/v1、bitmap color fontについて`hb-shape`とglyph/cluster/advanceを比較し、
  PDFをrenderしてoutline/layer/color/alpha/gradient/bitmap placementを検査する。
- copy/searchで元scalar列、VS、ZWJ、modifier、regional indicator、tagを契約どおり復元し、glyph IDや
  fallback表示列へ変わらない。
- 横組・縦組、hbox、paragraph、alignment、discretionary、copy/unbox、PDF tag/ActualTextを通す。
- malformed font、循環COLR graph、巨大SVG/bitmap、長大cluster、欠落glyphをboundedに拒否し、panicしない。
- ASCII/通常和文/通常OTFではemoji table lookup、color paint allocation、Vaak/WASM callが0で、既存DVI/PDF
  意味を変えない。

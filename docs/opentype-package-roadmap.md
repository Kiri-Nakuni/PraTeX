# PraTeX-native OpenType package roadmap

更新: 2026-08-24

## 目的

PraTeXのnative OTF/TrueType loader、metric、shaping境界が完成した後、その機能をLaTeXから
使うPraTeX固有packageを作る。到達点は単なる`fontspec`互換ではなく、公開されたfontspecの
主要な利用者向け機能を包含し、和文NFSS/JFM連携と文字単位のfont routingを一級機能として
加えた上位互換面である。`luatexja-fontspec`に相当する和文接続も同じ設計へ含める。

XeTeX、LuaTeX、LuaTeX-jaのversion primitiveを偽装して既存packageを通さない。PraTeX固有の
feature queryと、公開仕様から独立に実装したpackageを使う。既存packageのsourceを移植するか
forkする判断は、licenseと保守境界を別途監査するまで行わない。

## 利用者機能

少なくとも次を対象にする。

- roman、sans、monoと和文familyについて、family、series、shape、size、OpenType feature、
  language、script、variation axisを宣言・選択する。
- default fontに加え、文字class、Unicode scalar一個、scalar範囲、Unicode planeごとのfontを
  簡潔に割り当てる。
- fontがglyphを持たない場合の明示fallback chainを宣言し、どの規則とfontが選ばれたかを
  tracingできる。
- 縦組用face、縦書きfeature、和文JFM、欧文NFSS relationを同じfontsetへ関連付ける。
- 将来はIVS列、外字`AtomRef`、利用者定義文字collectionを同じrouting機構へ追加できる。

具体的なTeX構文はAPI試作まで固定しない。概念上は一つのfontsetへ`Default`、`Class`、
`CodePoint`、`Range`、`Plane`、`Fallback`を宣言し、そのfontsetをNFSS family/shapeから選ぶ。
単一の大きなpackageにする案と、汎用OpenType層＋和文拡張層に分ける案を同じfixtureで比較する。

## 文字classと優先順位

font routing用`FontRoutingClassId`は入力字句分類の`InputCategory`、公開`\catcode` / `\kcatcode`
番号、JFM metric class、TeXの`language`、font内部glyph IDとは別domainにする。組版用
`ScriptClassId`や利用者定義collectionから明示的に写せるが、生整数をcastしない。

JFM classは選択済みfont metricに依存するため、それ自体をfont選択の入力にすると循環する。
「句読点」「仮名」「漢字」等でfontを分ける時は、fontに依存しないrouting classを先に決め、
font選択後にそのfontのJFM classを求める。

規則の候補順位は次を基線にし、API実験でblack-box化する。

1. IVSまたは将来の外部文字identityに対するexact規則
2. Unicode scalar一個のexact規則
3. 明示scalar範囲
4. Unicode plane
5. font routing class
6. fontset defaultとfallback chain

同じtierで重なる規則は宣言順に依存させず、明示priorityがない限り登録時errorにする。範囲の
狭さを暗黙priorityにしない。surrogate、U+10FFFF超、逆順範囲、存在しないplane、循環fallback、
同じfaceの無限再訪を全table検証時に拒む。

## engine境界と性能

TeX packageは宣言を一文字ずつ実行時callbackへ渡さず、fontset全体をengineへ提出する。engineは
検証後、exact scalar table、非重複interval table、plane table、class table、default chainへ
compileし、一世代として原子的に交換する。paragraphのglyph hot loopでは静的dispatchと
run-local cacheだけを使い、trait object、TeX macro展開、Vaak/WASM call、filesystem lookupを
行わない。

font fileの論理名、解決済みpath、face index、variation座標、feature set、direction、sizeを
分離する。同じfaceでもvariationやfeatureが違えばlayout instanceは別identityにし、同一実体の
font bytesとcmapは共有する。provider handle、RustyBuzz face、cache generationはfmtへ保存せず、
fmtには検証可能な宣言だけを保存する。

PraTeX API、Vaak API、WASM ABIから追加する時も同じcandidate compilerを使う。高頻度routingは
host-owned tableへcompileし、per-glyph ABI往復を許さない。trapや不正tableで既存世代を部分更新
しない。

## backendとの接続

layout coreはfont routing結果をbackend非依存のfont identityとglyph runへする。直接PDFは
subset埋込みとToUnicodeへ接続する。通常DVIでは、driverが解決できるTFM/JFM/VFと物理font mappingを
持つfontだけを出せるので、native OTFをDVIへ黙って既存TFM名として偽装しない。生成TFM/VF、
明示special、または直接PDF限定のどれを採るかはDVI consumerとの適合試験で決める。

## 完了gate

- fontspecの代表的な公開利用例をPraTeX固有検出で処理し、未対応optionを黙って無視しない。
- roman/sans/mono、和文明朝/ゴシック、series/shape/size/縦横を同じNFSS文書で切り替える。
- exact code point、範囲、plane、class、fallbackの各routingと重複errorを試験する。
- JFM classとrouting classの循環がなく、K/X/JLReq spacingはfont切替後のmetricで一度だけ決まる。
- 直接PDFで使用glyphだけを埋め込み、ToUnicode、variation、feature、font resource identityを検査する。
- DVI対応を提供する場合は公式driverで同じfont選択を再現し、提供しない組合せは明示errorにする。
- 同じ文書のroutingなし／ありをprofileし、per-glyph allocationと外部processが増えていない。

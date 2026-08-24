# PraTeX-native OpenType package roadmap

更新: 2026-08-24

## 目的

PraTeXのnative OTF/TrueType loader、metric、shaping境界が完成した後、その機能をLaTeXから
使うPraTeX固有packageを作る。到達点は単なる`fontspec`互換ではなく、公開されたfontspecの
主要な利用者向け機能を包含し、和文NFSS/JFM連携と文字単位のfont routingを一級機能として
加えた上位互換面である。`luatexja-fontspec`に相当する和文接続も同じ設計へ含める。

特に和中混植では、Unicode Han unificationのため同じscalarが共有されていても、文脈が日本語、
簡体字中国語、台湾繁体字、香港繁体字等のどれかによって自然な字形が異なる。PraTeXは見た目を
合わせるためにscalarを別文字へ置換しない。明示された言語区間を`LanguageRegion`へ写し、font
routing、OpenType language system / `locl`、fallback、約物・禁則を選びながら、文字identityと
PDF `ToUnicode`には元のscalarまたはIVS列を保つ。Han unificationを感じさせない和中混植を、
追加macroの偶然ではなくpackage/APIのcompletion gateにする。

XeTeX、LuaTeX、LuaTeX-jaのversion primitiveを偽装して既存packageを通さない。PraTeX固有の
feature queryと、公開仕様から独立に実装したpackageを使う。既存packageのsourceを移植するか
forkする判断は、licenseと保守境界を別途監査するまで行わない。

## 利用者機能

少なくとも次を対象にする。

- roman、sans、monoと和文familyについて、family、series、shape、size、OpenType feature、
  language、script、variation axisを宣言・選択する。
- default fontに加え、文字class、Unicode scalar一個、scalar範囲、Unicode planeごとのfontを
  簡潔に割り当てる。
- 文書既定と入れ子の言語区間について、日本語、簡体字中国語、繁体字中国語等のfontset、
  OpenType language、fallback、組版profileを宣言する。同じscalarへregion別routeを持てる。
- fontがglyphを持たない場合の明示fallback chainを宣言し、どの規則とfontが選ばれたかを
  tracingできる。
- 縦組用face、縦書きfeature、和文JFM、欧文NFSS relationを同じfontsetへ関連付ける。
- 将来はIVS列、外字`AtomRef`、利用者定義文字collectionを同じrouting機構へ追加できる。

具体的なTeX構文はAPI試作まで固定しない。概念上は一つのfontsetへ`Default`、`Class`、
`CodePoint`、`Range`、`Plane`、`Region`、`Fallback`を宣言し、そのfontsetをNFSS family/shapeから選ぶ。
単一の大きなpackageにする案と、汎用OpenType層＋和文拡張層に分ける案を同じfixtureで比較する。

## 和中混植と言語区間

Unicode ScriptがHanであることだけでは、日本語・簡体字・繁体字のどの字形を使うか決まらない。
TeXの`\language`もhyphenation table番号であり、この用途へ流用しない。現在のR0
`\pratexregion`は`und`、`ja`、`zh-Hans`、`zh-Hant`、`ko`、`vi`をtypedに保持できるが、まだ
glyph生成へ接続されていない。R7では少なくとも次を連続して処理する。

1. packageまたはAPIが明示言語tagをhostの組版localeへ正規化する。
2. localeをfont routingのcontextとして、region固有fontsetとfallback chainを選ぶ。
3. 選択fontが公開するOpenType script/language tableを検査し、対応するlanguage systemと
   `locl`等のfeatureを含むshape planを作る。
4. shaping結果のglyph IDはregionで変わってよいが、text clusterは元のscalar/IVSへ対応させる。
5. spacing、句読点、禁則、line breakも同じlocaleを読み、font側と別のregionへずれない。
6. PDFではglyph selectionと`ToUnicode`を分け、同じscalarをja/zhで組んでも抽出文字を変えない。

`zh-Hant`だけでは台湾と香港等の字形・組版差を表現し切れない可能性がある。既存fmt/ABI codeへ
場当たり的な整数を足さず、中国語圏の実務調査とBCP 47/OpenTypeの公開仕様を根拠に、built-in
region追加またはversion付きhost registryのどちらで表すかを決める。少なくとも台湾と香港を
同じfallbackへ黙って潰す実装は完成扱いにしない。

Babel互換adapterは、Babelの公開言語切替・入れ子区間をPraTeXの組版localeへ写す入口として
調査・試作する。adapterはBabelのhyphenation `language`、PraTeXの`LanguageRegion`、Unicode
scriptを同一IDへ潰さない。未登録tag、script/region矛盾、font側language system不在時のfallbackを
診断でき、group終了時に元のregionへ戻ることをfixtureで固定する。

## 文字classと優先順位

font routing用`FontRoutingClassId`は入力字句分類の`InputCategory`、公開`\catcode` / `\kcatcode`
番号、JFM metric class、TeXの`language`、font内部glyph IDとは別domainにする。組版用
`ScriptClassId`や利用者定義collectionから明示的に写せるが、生整数をcastしない。

JFM classは選択済みfont metricに依存するため、それ自体をfont選択の入力にすると循環する。
「句読点」「仮名」「漢字」等でfontを分ける時は、fontに依存しないrouting classを先に決め、
font選択後にそのfontのJFM classを求める。

各規則は任意のregion predicateを持てる。region固有規則は同じ文字tierのregion-neutral規則より
優先するが、`ja`と`zh-Hans`のように排他的なpredicate同士を宣言順で競合させない。規則の文字側
候補順位は次を基線にし、API実験でblack-box化する。

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
font bytesとcmapは共有する。shape-plan cacheはregion、解決したOpenType language system、script、
direction、featureをkeyに含め、jaで作った`locl` planをzhへ再利用しない。font bytes cache自体は
regionごとに複製しない。provider handle、RustyBuzz face、cache generationはfmtへ保存せず、
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
- 同じHan scalar列を`ja`、`zh-Hans`、`zh-Hant`と、決定後の台湾・香港profileで組み、期待する
  font face、OpenType language system、glyph ID、約物・禁則が選ばれる。region境界をgroup、hbox、
  paragraph、alignment、discretionary、縦横組、copy/unboxで失わない。
- 同じscalarをregion別glyphへshapeした直接PDFから、元の同じUnicode scalarを抽出できる。
  IVSはbase+variation selectorを保持し、font glyph IDや互換漢字への置換を`ToUnicode`へ漏らさない。
- Babel adapterの文書既定、入れ子言語区間、`foreignlanguage`相当、未登録tag、font側language
  system不在を試し、TeX `\language`と`LanguageRegion`が独立して復元される。
- JFM classとrouting classの循環がなく、K/X/JLReq spacingはfont切替後のmetricで一度だけ決まる。
- 直接PDFで使用glyphだけを埋め込み、ToUnicode、variation、feature、font resource identityを検査する。
- DVI対応を提供する場合は公式driverで同じfont選択を再現し、提供しない組合せは明示errorにする。
- 同じ文書のroutingなし／ありをprofileし、per-glyph allocationと外部processが増えていない。

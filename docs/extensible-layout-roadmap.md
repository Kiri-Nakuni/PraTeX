# 拡張可能なscript境界組版とCJKV region

更新: 2026-08-24

R0のtyped `LanguageRegion`、横組JFM/wide glyph/BuiltIn finalizer、汎用class tableのvalidationと
direct-glyph用run-local dispatcherまでは実装済みである。RegionNodeを含むR3全伝播、R4のJLReq全体、
公開Vaak registration、WASM runtime/batch、font fallbackは未実装である。

## 目的

PraTeXの`\kanjiskip` / `\xkanjiskip`相当を「日本語と欧文の間だけ」に固定せず、
Han--Latin、Hangul--Latin、その他のscript境界を同じ機構で扱う。pTeX互換とJLReqの標準規則は
engine coreに一級実装し、Vaak/WASMを既定日本語組版の依存先にしない。利用者・出版社固有の
単純または高頻度な差替えだけVaakから宣言表として登録し、実験的で複雑だが低頻度の規則だけを
version付きWASM ABIへbatchで問い合わせる。callbackは、そのPraTeX実行中にVaak codeが能力を
明示要求した場合だけ有効にし、LuaTeX型の常設・全callback有効化は行わない。

同時に、中国語・日本語・韓国語・ベトナム語の組版差をengine levelで表す。ここで必要なのは
Unicode scriptでもTeXのhyphenation番号でもなく、句読点、禁則、font fallback、OpenTypeの
language systemなどを選ぶ**layout locale**である。本書では利用者の呼び方に合わせて
`LanguageRegion`と呼ぶが、地理だけを表す値ではない。

## 混ぜてはいけない五つのdomainと二つの入力view

| domain | 例 | 決めるもの | 決めないもの |
|---|---|---|---|
| canonical `InputCategory` | TeX `CatCode`、wide、raw-byte route | byte/UTF-8入力をどのtokenにするか | Unicode script、組版locale |
| layout `ScriptClassId` | Latin、Han、Hiragana、Katakana、Hangul、Common、Inherited | glyph間のscript境界 | 日本語か中国語か |
| `LanguageRegion` | und、ja、zh-Hans、zh-Hant、ko、vi | 組版locale、句読点規則、fallback/shaping context | TeX hyphenation table |
| `TexLanguage` | 0..255 | `\patterns`、`\hyphenation`、hyphen minima | CJKVの地域差 |
| text/glyph identity | Unicode scalar/IVS、namespaceつき外部文字、造字 | 文字の同一性とfont mapping | catcode、script境界、region |

Han一字だけからja/zh-Hans/zh-Hantを推定できない。ベトナム語の通常scriptはLatinである。
したがって、どのdomainからも別domainを暗黙推定しない。有効な組合せには
`(Han, zh-Hant, TeX language 0)`、`(Latin, vi, TeX language 37)`がある。

和中混植では同じHan scalarが日本語・中国語の区間で別の地域字形を要求し得る。glyph ID、
OpenType language/`locl`、fallback、句読点・禁則はregionにより変えてよいが、scalar/IVSの
identityとPDF `ToUnicode`は元の列を保つ。`zh-Hant`だけで台湾・香港等を黙って同一視せず、
調査後にbuilt-in値またはversion付きregistryを追加する。Babel等のadapterは明示言語区間を
`LanguageRegion`へ写すが、TeX `language`やcatcodeを同じIDへ潰さない。

`\catcode`とupTeX互換`\kcatcode`は別domainではなく、上の`InputCategory`へ入る二つの
数値viewである。たとえば双方の公開値14/16は別の意味なので、それぞれのcodecで意味へ
写してから統合する。保存表は互換性とASCII fast pathのため当面別の粒度を保つ。

UnicodeのScript propertyはprimary associationであり、Common、Inherited、Unknownも持つ。
Script_Extensionsは複数scriptとの関係を表すが、layout localeの代用にはならない。PraTeXの
初期built-in script表はversionを記録し、更新時にfixtureと性能を再測定する。

異体字、外字、造字の文字identityはこれらの分類値へ混ぜない。UTF-8とIVSを
第一表現にし、超漢字/TRONと嘘字方式をimport境界として参考にする設計は
[文字・異体字・造字の内部表現](glyph-identity-roadmap.md)に分ける。

## `LanguageRegion`の最初の契約

fmtと将来ABIで固定するbuilt-in値を次とする。

| code | 意味 | BCP 47相当の表示名 |
|---:|---|---|
| 0 | 未指定 | `und` |
| 1 | 日本語組版 | `ja` |
| 2 | 簡体字中国語組版 | `zh-Hans` |
| 3 | 繁体字中国語組版 | `zh-Hant` |
| 4 | 韓国語組版 | `ko` |
| 5 | ベトナム語組版 | `vi` |

最初の独自primitiveは`\pratexregion=<0..5>`とする。typedなEqtb parameterとして
local/global/globaldefs、save stack、fmt、`\the`、`\showthe`、`\let`、`\meaning`を通す。
enum値なので`\advance`、`\multiply`、`\divide`の対象にはしない。範囲外の代入は診断し、
現在値を変更しない。

既存の`\language`、`\setlanguage`、`\lefthyphenmin`、`\righthyphenmin`、`\patterns`、
`\hyphenation`の意味は変えない。pTeX/upTeX互換profileは初期化時にregion 1を明示できるが、
engineがmodeや入力文字から自動設定しない。TeX82、pdfTeX互換、TRIPの既定は0である。

将来任意のBCP 47 tagを受ける場合も、fmtへproviderのrun-local IDを保存しない。built-in codeと
version付きのhost registry IDは別domainにし、fmt読込み後のprovider registryは空から始める。

## regionを文字列へ伝える位置

regionをtokenize時の`CjkToken`へ焼き込まない。macroに保存したtokenは実行時のgroupと
代入を反映すべきだからである。glyph nodeを作る時点でambient regionを読む。

初期案はzero-widthの`RegionNode`をhorizontal listへ入れることである。

- `HorizontalMode`は最後にemitしたregionを小さな値で保持する。
- regionが変わった後、次のtext atom直前だけmarkerを入れる。
- default `und`ではmarkerを一つも増やさない。
- groupの`unsave`からlistへ直接触らず、次の文字でlazyに同期する。
- DVI/PDF出力では描画せず、spacing finalizerと将来のfont/shaping contextだけが読む。
- box register内のmarkerはfmtを往復する。

`\unhbox` / `\unhcopy`、alignment cell/span、discretionaryはlistをbulk appendするため、
外側の「最後にemitしたregion」cacheを古くし得る。append後はcacheをdirtyにし、次の文字前に
`und`への復帰も含む明示markerを入れるか、末尾contextを型付きsummaryとして受け渡す。
全listを毎回走査してcacheを直す実装にはしない。

restricted hboxにもfont/JFM/spacingのregionが必要である。初期段階ではboxを外側の文字と
透明に連結せず、script境界のbarrierとして扱う。将来必要なら`ListNode`へfirst/last
`BoundaryAtom` summaryを持たせる。paragraphではspacing finalization後にline breakと
hyphenationを行い、RegionNodeはhyphenatorの現在言語を変更しない。

## script境界spacingの中央化

現在のPraTeXはJFM、Unicode glyph node、`\kanjiskip`、`\xkanjiskip`をまだ実装していない。
wide文字を診断して捨てる段階でglueだけを先に差し込んでも、実用上の意味も正しい性能測定も
得られない。JFMとwide glyph nodeを先に作り、その後に一つの
`finalize_horizontal_list`へ境界処理を集約する。

このfinalizerはgeneric `hpack`の中へ隠さない。少なくとも次の意味上のlist終端から呼ぶ。

- paragraphを`end_graf`からline breakerへ渡す前
- restricted hboxをpackする前
- horizontal alignment cell/spanを閉じる前
- display前段落をline breakへ渡す前

入力はglyphそのものではなくhost-ownedの小さなrecordとする。

```text
BoundaryAtom {
  code_point,
  script_class_id,
  language_region_code,
  tex_language_number,
  font_handle,
  metric_class,
  flags
}
```

境界ごとにleft/rightの値を保ち、例えばHan(ja)--Latin、Han(zh-Hant)--Latin、
Hangul(ko)--Latinを別規則にできる。JFMの`JfmId` / char typeはfont固有の別domainであり、
`ScriptClassId`へ混ぜない。ambient regionはdefault font/JFM選択の入力にはできるが、明示した
同一font/JFMのidentityや寸法を後から変えない。

自動挿入glueは通常の明示`\hskip`と区別できるnode subtypeを持たせる。`\lastskip`、
`\showbox`、line breaking、再finalize時の二重挿入について契約を固定してから互換primitiveを
公開する。pTeX互換の`\kanjiskip` / `\xkanjiskip`、`\xspcode`、`\inhibitxspcode`、
auto-spacing switchは公開manualと黒箱観測だけから別段階で接続する。

## VaakとWASMのdispatch

per-boundaryのtrait object callやABI往復をhot loopへ置かない。listを閉じる前に一度だけ
静的dispatcherを選ぶ。

1. `BuiltIn`: engine内のpTeX互換規則と一級JLReq規則。標準日本語はこの経路だけで完結する。
2. `CompiledTable`: 明示有効化されたVaak/WASMが利用者固有のrange・class pair・action表をuploadし、
   hostが検証・compileしたものをsafe Rustで引く。単純または高頻度の差替えはここへ置く。
3. `ExplicitWasm`: 実験的で複雑・低頻度な判断だけ、bounded list/batch単位で一度に問い合わせる。

現在の最初のproduction sliceは、compile完了後のtableだけを`Eqtb`のrun-local ownerへinstallし、
direct glyphを持つ一listでactivation generationとregionが終始同じ時だけ`CompiledTable`を選ぶ。
途中のreplace/revoke/region変更では表を混ぜずlist全体を`BuiltIn`へ戻す。ASCII-onlyかつ未activationの
listは従来gateより先へ入らない。RegionNodeがない間はindirect hbox/disc edgeへ終端時regionを推測せず、
その境界だけBuiltInへ戻す。adjustment tierとline-edge discardもline breaker接続前は適用済みとしない。

Vaak codeの実行だけではcallbackを有効にしない。そのVaak実行がspacing capabilityを明示要求し、
hostが許可した時だけrun-local handleを発行する。scope終了、取消し、PraTeX実行終了で失効し、
fmtへ保存しない。通常実行にglobal callback tableを置かない。

WASM ABIはRust enum、pointer、slice、`Rc`、allocatorを公開しない。V1 recordは固定幅`u32`で、
left/rightそれぞれのcode point、script class、region、TeX language、font/metric handleと、
list kind、direction、flagsを持つ。contextはread-only snapshotであり、finalizer中にEqtbやregionを
再入可能に変更するhost APIを出さない。ABI version、capability、最大batch長、memory、fuelを
handshakeする。trap、fuel切れ、不正ID、不正actionではbatch全体を破棄し、built-in fallbackへ
原子的に戻す。

## OTF/JFMとの接続

regionは`FontInfo`のidentityや`\font`の再利用keyへ暗黙に入れない。glyph使用時に
`FontUseContext { font, script, region }`として、将来のfont fallbackとOTF shaping planへ渡す。
OpenType language-system tagはregionからJAN/ZHS/ZHT/KOR等の候補を選べるが、fontが公開する
script/language tableとfallback規則を検査して決める。TeX language番号から選ばない。

JFM lookupの基本keyは`(jfm_id, code_point)`である。regionはどのdefault JFMを選ぶか、
句読点・禁則policyをどう選ぶかの入力にはできるが、同じ明示JFMのmetricをambient regionだけで
変更しない。この分離によりe-upTeX互換JFMと韓国語・中国語向けpolicyを同じspacing frameworkへ
載せられる。

## 段階

| 段 | 内容 | 完了条件 |
|---|---|---|
| R0 | typed `LanguageRegion` stateと`\pratexregion` | group/global/fmt/表示、`\language`独立、TRIP不変 |
| R1 | `TexLanguage(u8)`で既存hyphenation境界を型付け | 生のi32正規化を一箇所にし、既存意味を変えない |
| R2 | JFM、Unicode glyph node、DVI/PDF wide glyph event | CJK tokenを捨てず、metricと出力へ到達する |
| R3 | `ScriptClassId`とRegionNode、list context伝播 | paragraph/hbox/alignment/discretionary/unhbox/fmtを通す |
| R4 | built-in finalizer、pTeX互換、一級JLReq spacing/禁則 | 一箇所の決定でglue・penaltyを挿入し、再実行が冪等。標準日本語はprovider不要 |
| R5 | 利用者固有のexplicit Vaak table upload | 無効時call 0、scope失効、host側compiled lookup |
| R6 | 実験規則向けversion付きWASM batch ABI | bounded/fuel/fallback、per-boundary ABI call 0 |
| R7 | font fallback、OTF language system、CJKV policy | region別glyph/shaping fixture、明示font identity不変 |

R0は`ac6ad90`で実装・試験済みである。現在は明示compiled tableを有効にしたdirect-glyph listだけ
uniform regionがclass/rule選択へ影響するが、RegionNodeがないため途中変更は全list fallbackとなる。
通常BuiltInのregion別組版やbox/disc伝播ができるR3完了とは数えない。
日本語組版のpTeX相当完了条件と、e-upTeXにないJLReq意味論の優先順は
[pTeX相当からJLReq一級対応へ進むroadmap](japanese-typesetting-roadmap.md)に分けて固定する。

## 試験と性能の採否条件

- `\pratexregion`: 0..5、範囲外、local/global/globaldefs、`\the`、`\showthe`、`\meaning`、
  `\let`、fmt破損、`\language`との独立。
- context伝播: `ja -> {ko -> glyph} -> glyph`、`und`復帰、変更後文字なし、nested hbox、
  discretionary、alignment、inline/display math、box copy/unbox。
- spacing: ASCIIのみ、各CJKV--Latin、Common/Inherited、left/right region不一致、明示glueとの順序、
  `\lastskip`、`\showbox`、line break、再finalize。
- provider: explicit activation、scope失効、無効時call 0、table検証、batch分割、trap/fuel/
  不正actionのatomic fallback。
- font/JFM:同じHan scriptでja/zh-Hans/zh-Hantが別context、同じ明示font/JFM identityは不変。

TeX82/TRIP既定のASCIIではRegionNode 0、Unicode表引き0、Vaak/WASM call 0、string/hash allocation
0をhard boundaryにする。region比較は既存word開始処理と融合した小さな整数比較一回まで。
Node enumのsizeを増やさないことをx86_64で記録し、ASCII大量paragraph/hbox、mixed CJKV、
region頻繁変更のfixtureをrelease LTOで変更前後交互に測る。stdout/log/DVI/PDF hashとTRIPの
意味比較を固定し、有意な退行があればspacing abstractionを差し戻す。

## 権利と一次資料

pTeX/upTeX/e-upTeXの実装sourceや上流試験を移植しない。互換primitiveは公開manualと許可された
black-box観測から独立して書く。Unicode property、BCP 47、OpenTypeとの接続も各公開仕様を
version付きdataとして扱い、PraTeX固有のregistry/ABI codeと混ぜない。

- [Unicode Standard Annex #24: Unicode Script Property](https://www.unicode.org/reports/tr24/)
- [RFC 5646 / BCP 47: Tags for Identifying Languages](https://www.rfc-editor.org/info/rfc5646/)
- [OpenType Layout tag registry](https://learn.microsoft.com/en-us/typography/opentype/spec/ttoreg)
- [e-upTeX移植記録](euptex-port-notes.md)
- [拡張可能な文字分類器](character-classifier-extension.md)
- [UTF-8を保つ文字・異体字・造字の内部表現](glyph-identity-roadmap.md)

# pTeX相当からJLReq一級対応へ進む日本語組版roadmap

更新: 2026-08-25

## 方針

PraTeXでいう「最低限の日本語組版」は、横書きの一例を出すことではない。pTeX相当は最初の
互換milestoneであり、終点は**upTeX以上のJLReq native対応**である。JFM、和文font/node、
自動和文間隔、禁則、横組・縦組、縦中横、割注、DVI/PDF出力までをengine coreで支える。
縦中横・割注のauthor向け名はformat macroに置けるが、方向、分割候補、外側行分割との協調を
macroだけへ押し込まない。

JLReqの標準的な日本語組版も一級機能としてengine内に置く。Vaak/WASMは標準規則を成立させる
依存先にせず、利用者・出版社固有のprofileまたは実験的な低頻度処理を**明示要求した時だけ**
差し替える境界に限る。既定日本語paragraphのcallback回数は0を条件とする。

現在はUTF-8 CJK token、`\kcatcode`、typed `LanguageRegion`、`\kanjiskip` / `\xkanjiskip`
の通常glue parameter面に加え、boundedな横組JFMを`\pratexjfont`（同じ意味の範囲だけ`\jfont`
alias）で定義・選択できる。current和文font、JFM class付き`WideCharNode`、`zw` / `zh`、
DVI `set2` / `set3`まで一続きに接続したため、選択済み横組JFMがあればCJK tokenを捨てずに
横組DVIへ出せる。JFM readerは横11／縦9、24-bit raw文字code、u8 class、skip、再配置、
256超glue/kern indexを検査し、class対programをload時に直接表へcompileする。

JFM pair adjustment、K/X自動空白、BuiltIn最小禁則は横組の実hlist・行分割・DVIへ
接続済みである。[W3C JLReq Table 2](https://www.w3.org/International/jlreq/tables/table_en3.pdf)
はcl-01の後とcl-02の前で分離しないclass間規則を定める。Table 2自体はcode point表ではないため、
所属文字の一次資料は同じ2020-08-11 W3C Working Group Note
[JLReqのAppendix A.1/A.2](https://www.w3.org/TR/jlreq/)とする。

現在の横組和文JFM経路で採用するのは、次の12対である。

- `U+FF08/U+FF09`（`（）`）、`U+3014/U+3015`（`〔〕`）、`U+FF3B/U+FF3D`（`［］`）、
  `U+FF5B/U+FF5D`（`｛｝`）
- `U+3008/U+3009`（`〈〉`）、`U+300A/U+300B`（`《》`）、`U+300C/U+300D`（`「」`）、
  `U+300E/U+300F`（`『』`）
- `U+3010/U+3011`（`【】`）、`U+2985/U+2986`（`⦅⦆`）、`U+3018/U+3019`（`〘〙`）、
  `U+3016/U+3017`（`〖〗`）

Appendix Aは小括弧・角括弧・波括弧をASCII code pointで記載する一方、和文組版では
fullwidth互換形を使う実装慣行を注記している。このためBuiltIn表は`U+0028/U+0029`、
`U+005B/U+005D`、`U+007B/U+007D`をLatin経路のままとし、対応する上記fullwidth形だけを
和文禁則に入れる。横組限定の欧文引用符`U+2018/U+2019`、`U+201C/U+201D`、
guillemet `U+00AB/U+00BB`、縦組限定の`U+301D/U+301F`、Appendixがそのcode pointを
挙げない`U+FF5F/U+FF60`はこのbounded subsetへ拡張しない。

main-loopでのJFM pair・最小禁則の早期挿入と、unshifted hbox／discretionaryの限定境界は
接続済みである。shifted/vbox、未検証command、discの全JFM class・禁則・unbox matrix、
禁則class全体、`\tfont`と縦組は未完了であり、P0全体や「日本語組版対応」の完了ではない。

明示installされた検証済みscript class対tableは、run-local dispatcherがlistごとに一度選び、
世代とregionが安定したdirect glyph境界でfixed/K/X/no-spaceと限定boundary glue/penaltyへ
materializeできる。RegionNode、indirect box/disc、adjustment tier、line-edge discard、公開provider
registration/runtimeは未接続で、標準日本語のBuiltIn経路はcallback 0のままである。

`\kanjiskip` / `\xkanjiskip`の実spacing、JFMとのhybrid、暗黙K、
script-pair拡張のclean-room設計は
[和文間隔core設計](kanjiskip-core-design.md)に分離した。

## 内部domain

catcodeへ組版情報を押し込まない。少なくとも次を別の型として保つ。

| domain | 役割 |
|---|---|
| `InputCategory` | tokenizerのカノン分類（`CatCode`、wide、raw-byte route）。`kcatcode`は別公開番号の互換view |
| `TextIdentity` | 元のUnicode scalar列・IVS・将来の外字参照。正規化やglyph IDで同一性を潰さない |
| `ScriptClassId` | Han、Kana、Hangul、Latin、Common等の組版script |
| `LanguageRegion` | ja、zh-Hans、zh-Hant、ko、vi等のlayout locale |
| `JfmClassId` / `LayoutClass` | font固有JFM classとJLReq文字class。互いにもcatcodeにも混ぜない |
| `GlyphRequest` | font、方向、region、variation selector、feature、最終glyphの要求 |

tokenとnodeへsource provenanceを保持できる余地を作る。これはIVS、PDF `ToUnicode`、診断、
将来のincremental実行とLSPを同じ対応関係から解くためであり、glyph IDを論理文字へ逆推定しない。

## P0: pTeX相当

P0全体を終えて初めて「日本語組版対応」と呼ぶ。途中の横組PDFはcheckpointであって完了ではない。

### P0a: JFMと和文glyph

1. 公開JFM仕様からbounded readerを独立実装し、文字からclass、width、height、depth、italic、
   class対glue/kernを得る。
2. `\jfont`、`\tfont`と横・縦・欧文のcurrent fontを型付きにする。`zw`、`zh`を現在のJFMへ
   接続し、暫定`em`代用を終える。
3. 元のUnicode `TextIdentity`とJFM classを持つwide glyph nodeを追加する。
4. DVIのwide character eventとPDFの和文font eventをbackend共通の意味recordへ渡す。
   DVI byte列とPDF object表現はbackendごとに分ける。

### P0b: 自動間隔と禁則

listを閉じる一箇所の`finalize_horizontal_list`で、次を決める。

- `\kanjiskip`、`\xkanjiskip`
- `\autospacing`、`\noautospacing`、`\autoxspacing`、`\noautoxspacing`
- `\xspcode`、`\inhibitxspcode`
- `\inhibitglue`、`\disinhibitglue`
- `\prebreakpenalty`、`\postbreakpenalty`、和文widow penalty

自動nodeは明示glue/penaltyと区別できるprovenanceを持ち、finalizerを再実行しても二重挿入しない。
ただし`PtexCompat`では、段落末に有効な`\kanjiskip`値が段落全体へ効くことや、暗黙のskipが
`\lastskip`や`\showbox`へ通常の明示glueとして現れないことまでblack-boxで合わせる。

### P0c: pTeX方向と縦組

- typed `WritingMode`、direction node、direction boxを追加する。
- 横／縦JFM、baseline shift、boxのwidth/height/depth、nested yoko/tateを公開意味どおり扱う。
- line breaking、hpack/vpack、alignment、unbox、page builder、DVI/PDF shipoutを方向対応にする。
- pTeX方向とTeX--XeTのLR区間はprimitive意味論を共有しない。内部nodeとbackend座標だけを
  再利用できる構造にする。

### P0d: 互換primitiveと合格条件

公開pTeX/upTeX manualの和文primitiveを棚卸しし、局所・大域代入、group、fmt、`\the`、
`\showthe`、`\meaning`、error回復を該当機能ごとに試験する。内部表現はUTF-8/Unicodeを
第一とし、旧来の内部文字codeは観測可能な互換面だけadapterで再現する。

TeX Live 2026の`euptex`をblack-box oracleにし、同じUTF-8入力について次を正規化比較する。

- `\showbox`、`\showlists`のnode種、glue、kern、penalty
- 行分割位置、boxのwidth/height/depth、baseline shift
- `updvitype`のset/put、font、方向、sp座標
- JFM class全組合せ、和和・和欧・欧和、font切替、group境界、明示glue、auto/inhibit
- 狭い`\hsize`の禁則、nested yoko/tate、alignment、unbox、page境界

PDFのbyte一致は要求しない。node意味、sp座標、描画結果、抽出Unicodeを比較する。

## P1: e-upTeXを越えてJLReq実務に必要なcore意味論

### 1. 優先順位付き行長調整

JLReqは文字class対ごとの自然アキ、詰め・空け限界と、どのアキを先に調整するかを規定する。
通常のTeX glue次数だけでは、約物、和欧文間、欧文間隔などの段階を十分に表せない。

```text
BoundaryRule {
  natural,
  shrink_limit,
  stretch_limit,
  shrink_tier,
  stretch_tier,
  break_rule,
  line_edge_rule,
  reason_id,
}
```

固定個数のtierをengine-nativeな小整数tableとして処理する。`PtexCompat`と`Jlreq`をbuilt-in
profileにし、`\kanjiskip`等は前者への互換adapterとする。`\jidori`や`\akigumi`のmacro面も
同じ一行調整器を使う。

### 2. class pairとsequenceを扱う禁則

pTeX互換の一文字pre/post penaltyと登録制限は`PtexCompat`で再現する。一級JLReq profileでは、
left/right class、writing mode、line contextをkeyにした疎なbreak ruleを持つ。連数字、rubyの
親文字列、厳格／緩和禁則を、一文字の前後どちらか一値へ潰さない。

### 3. 実font、IVS、fallback

「和文fontには全和文文字がある」というpTeX前提を直接PDFへ持ち込まない。font `cmap`で実在を
確認し、元のUnicode列・IVS、JFM class、glyph ID、fallback policyを別domainにする。欠字は
typed nodeと診断にし、PDFの埋込み・`ToUnicode`へ同じ対応を渡す。

### 4. rubyと圏点の構造

author向け記法はLaPraTeX macroに置くが、正しい分割、overhang、衝突、縦組、accessibilityには
base文字範囲とannotationの対応を知るsemantic nodeが必要である。モノルビ、グループルビ、
熟語ルビ、両側ruby、圏点を一つの`AnnotationNode`系で扱う。

### 5. 縦中横とglyph orientation

`\tatechuyoko`の表面はmacroでよい。横方向の固定長`InlineObject`、周囲のJLReq class、
縦用advance、layout extentとink extent、元文字と回転glyphの対応はcoreで扱う。pTeXの
`WritingMode`、TeX--XeTの`InlineDirection`、optional shapingの`ShapingMode`は別型にする。

### 6. 基本版面gridと段末均等化

紙面寸法や見出し指定はLaPraTeXが宣言する。page builder側は自然なline extentsとgrid advanceを
分け、ruby・圏点・大きなinline objectの後でもbaselineをgridへ戻せるようにする。多段末の
均等化はfloat、明示改ページと同時に扱うcore policyとして後段に実装する。

### 7. 割注

engine-levelで扱う。用途名primitive `\warichu`を直接追加せず、内側二laneの行分割、均等化、
本文複数行へのfragment、禁則、外側行分割との協調を表せる用途非依存の抽象にする。内部nodeは
次節の案Bに固定し、author向け表面はLaPraTeX macroとして後から接続する。

## 縦中横・割注の抽象化（案Bを採択）

2026-08-23に利用者が案Bを選択した。案A/Cは再検討時にtrade-offを失わないため記録として残す。

| 案 | 内部表現 | 縦中横 | 複数の本文行に跨る割注 | 難度・主なtrade-off |
|---|---|---|---|---|
| A（不採択） | 属性付き原子的`InlineObject`だけ | 最短で実装可能 | 不可 | 低～中。固定boxとして既存line breakerへ載るが、割注は一外行内に閉じる |
| **B（採択）** | 固定`InlineObject`＋分割可能`InlineSubflow` | Aを第1段にする | 可能 | 高いが段階導入可能。割注候補を外側line breakerへ渡し、一般automatonのABIを早期固定しない |
| C（不採択） | 統一fragment automaton | 一遷移で表現 | 可能 | 最大。ruby等も一般化できる一方、state上限・枝刈り・fmt・ABIを初手から固定する |

案BではTeX入力を後で再実行しない。入力は一度だけ実行して副作用を確定し、immutableな
`OwnedParagraphIR`から`SubflowCandidate`を列挙する。標準候補生成はnativeでcallback 0、Vaakは
検証済みlayout/cost tableを一度だけ登録し、WASMは必要な場合もbounded candidate batchだけを
扱う。raw node pointerやRust enumをABIへ出さない。

案Bの段階は、B0 `WritingMode`/frame＋縦中横、B1 一外行内の二lane、B2 continuationを持つ
複数外行fragment、B3 明示opt-in providerの順とする。begin/end marker構文は便利なfrontendに
できるが、coreでは最終的に同じtyped nodeへcompileする。

## engine、macro、拡張の境界

| 場所 | 対象 |
|---|---|
| engine core | JFM、和文glyph/font、spacing、禁則、優先行調整、方向・縦組、inline object/subflow、font実在、glyph-text対応、annotation意味、grid基盤 |
| LaPraTeX macro | class option、版面宣言、見出し、ruby・圏点・縦中横・割注のauthor surface |
| Vaak table | 利用者・出版社固有のclass/pair差替え。明示capabilityで一度uploadしhost側でcompile |
| WASM batch | 実験的な形態素break、独自辞書、特殊annotation policyなど複雑で低頻度の処理 |

標準日本語profileはVaak/WASMがなくても完全に動作する。拡張providerはfmtへrun-local handleを
保存せず、無効・trap・fuel切れでは検証済みbuilt-in profileへ原子的に戻す。

## 性能条件

- ASCII paragraphではUnicode/JFM表引き、provider call、追加allocationを0にする。
- JFM raw codeからclassへの検索はwide glyph生成時に一回、class pairはload時にcompileした
  `u16`の直接表を一回引くだけにする。
- spacing finalizerはlist終端で一回だけ。line breaker内にtrait objectやABI callを置かない。
- priority調整は固定bucket、annotation/grid用allocationは使用時だけにする。
- 10万字段落、混植、縦組をreleaseで測り、TeX82経路のTRIPとDVI/PDF意味比較を固定する。
- DVI modeの同一入力、同一TeX tree、同等DVIについて、upLaTeXに対するend-to-end wall timeを
  corpus幾何平均・各主要caseとも**1.2倍未満**にする。探索、fmt復元等は内訳も記録するが、
  合否値から隠さない。
- 性能抽象化が有意な退行を生み、意味上必要でもない場合は分離枝で差し戻す。

## 後続段階

1. P0の横・縦pTeX相当を完了する。
2. `jsarticle`基本横組を通し、次に`jlreq`横組、最後にその縦組を通す。
3. P1の優先行調整・class pair禁則・実font/IVSを先行実装する。
4. 採択済みの案BをB0からB3まで段階実装し、LaPraTeXがruby、圏点、縦中横、割注、版面gridをtyped core APIへ接続する。
5. ja/zh-Hans/zh-Hant/ko/viの標準profileをcoreに追加し、同じ境界機構をCJKVへ一般化する。
6. source provenanceと副作用journalを使い、incremental replayと実行結果ベースLSPへ接続する。

## クリーンルーム資料

上流実装sourceや上流の回帰試験は移植しない。公開manual、標準文書、許可された実行結果から
独立fixtureを作る。

- [pTeX guide](https://mirrors.ctan.org/info/ptex-manual/ptex-guide-en.pdf)
- [pTeX manual](https://mirrors.ctan.org/info/ptex-manual/ptex-manual.pdf)
- [JFM仕様](https://mirrors.ctan.org/info/ptex-manual/jfm.pdf)
- [JFM clean-room実装記録](jfm-port-notes.md)
- [W3C 日本語組版処理の要件](https://www.w3.org/TR/jlreq/)
- [W3C Japanese Gap Analysis](https://www.w3.org/TR/jpan-gap/)
- [W3C Simple Ruby](https://www.w3.org/TR/simple-ruby/)
- [jlreq公式README](https://github.com/abenori/jlreq/blob/master/README-ja.md)
- [LuaTeX-ja manual](https://tug.ctan.org/macros/luatex/generic/luatexja/doc/luatexja-ja.pdf)
- [拡張可能なscript境界組版](extensible-layout-roadmap.md)
- [文字・異体字・造字の内部表現](glyph-identity-roadmap.md)

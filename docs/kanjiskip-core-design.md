# `\kanjiskip` / `\xkanjiskip` core設計

更新: 2026-08-23

## 目的と調査境界

PraTeXの日本語組版はengine coreの一級機能として実装する。
`\kanjiskip`と`\xkanjiskip`をLaTeXのpTeX検出だけ通す空の名前として追加せず、
代入・fmt・行分割・箱・JFMまで意味を持つ機構へ段階的に接続する。

この設計はTeX Live 2026の公開`ptex-manual` / `eptexdoc`と、
公式e-upTeXへ自作最小入力だけを与えた黒箱観測から作った。
pTeX/upTeX/e-upTeXのengine sourceと上流testは参照していない。

## 1. primitiveとしての契約

INITEXのengine既定値はどちらも`0pt`である。
preloaded `euptex.fmt`の次の値はformat所有であり、engineへ焼き込まない。

- `\kanjiskip=0pt plus .4pt minus .4pt`
- `\xkanjiskip=2.5pt plus 1pt minus 1pt`

二つとも通常のglue parameterと同じ経路を使う。

- `\the`、`\showthe`、`\show`
- `\let`、`\ifx`
- 局所代入、`\global`、`\globaldefs`
- `\advance`、`\multiply`、`\divide`
- fmt dump/load

黒箱では、`1pt plus 2pt minus 3pt`へ
`4pt plus 5pt minus 6pt`を加え、3倍して2で割ると
`7.5pt plus 10.5pt minus 13.5pt`になった。
`\globaldefs=1`はgroup外へ残り、`\globaldefs=-1`は明示的な
`\global`も局所化した。

自作INITEX fmtでは次を完全に往復した。

- K: `11pt plus 12fil minus 13fill`
- X: `14pt plus 15filll minus 16fil`

表示契約:

- `\showthe\kanjiskip`は`> 10.0pt.`
- `\show\kanjiskip`は`\kanjiskip=\kanjiskip.`
- `\let\K\kanjiskip`後の`\meaning\K`は`\kanjiskip`

### 最小primitive slice

既存のnamed skipへ次を加える。

- `SkipVariable::KanjiSkip`
- `SkipVariable::XKanjiSkip`

主な変更箇所:

- `src/eqtb/skips.rs`: 値、名前、index、dump/undump
- `src/eqtb/levels.rs`: save levelと局所復元
- `src/eqtb/primitives.rs`: `PrefixableCommand::Glue`として登録

走査・算術・内部量は既存の次の経路を共有する。

- `src/command/prefixable.rs`
- `src/command/arithmetic.rs`
- `src/scan_internal.rs`

primitiveを持つだけで公開LaTeX資材はPraTeXをpTeX系と判定する。
2026-08-23のproduction checkpointでは横組hlistへK/Xを接続したが、後述する
BuiltIn最小subsetに限る。pTeX全互換やJLReq完成を示す検出stubにはしない。

## 2. K/X挿入の黒箱契約

### list終端時の最終状態

K、X、自動間隔switch、`xspcode`、`inhibitxspcode`は、
hboxまたは段落を閉じた時点の最終状態でlist全体を再評価する。

- `あ\kanjiskip=7pt い`は既出境界にもK=7ptを使う。
- `あ\xkanjiskip=8pt A`は既出境界にもX=8ptを使う。
- `xspcode` / `inhibitxspcode`も終端直前の表値が既出境界へ効く。

代入時に即glue nodeを確定するだけでは互換にならない。

### 暗黙Kと実nodeのX

通常の和文–和文Kの最終契約は暗黙である。

- 箱の寸法、stretch/shrink、改行候補には効く。
- `\showbox`、`\showlists`、`\lastskip`へ現れない。
- `\noautospacing`でも幅0の仮想境界は改行候補として残る。

現productionでは直結glyph間を`VirtualKanjiSkip`としてmaterializeし、寸法・改行・出力には
使う一方、node introspectionと`\unskip`から隠す。箱境界のKは別variantの
`MaterialKanjiSkip`であり、このvisibilityへ混ぜない。いずれも由来を型で保持し、
再finalize時だけ除去する。

Xは実glue nodeである。

- `\showbox`へ`\glue(\xkanjiskip)`と表示する。
- `\noautoxspacing`でも幅0のnodeを残し、その位置で改行できる。

penaltyを挟む和文–和文は
「和文文字、penalty、明示K glue、和文文字」の順になる。
自動Kを切った場合も幅0のK nodeを残す。

### boxとunbox

上下移動していないhboxは、内側のfirst/last文字を外側境界として使う。

- 和文–和文ならK
- 和文–欧文ならX
- `\raise` / `\lower`したboxは対象外

根拠は[pTeX manual 2025-05-10版](https://tug.ctan.org/info/ptex-manual/ptex-manual.pdf)
の`\kanjiskip`節と、2026-08-24にTeX Live 2026公式e-upTeX
`3.141592653-p4.1.2-u2.02-251130-2.6`へ与えた自作最小入力である。binary archiveは
`uptex.windows` revision 78020、SHA-256は
`c878983da002f32a24a507680ccf00261a3761089ed324892668ded589bf9c0d`。

| 入力（J=10pt和文、A=cmr10、K=4pt、X=3pt） | 幅 | `\showbox` |
|---|---:|---|
| `漢\hbox{字}` / `\hbox{漢}字` | 24pt | 箱外に実`\glue(\kanjiskip) 4pt` |
| `A\hbox{漢}` / `\hbox{漢}A` | 20.50002pt | 箱外に実`\glue(\xkanjiskip) 3pt` |
| `漢\hbox{A}` / `\hbox{A}漢` | 20.50002pt | 同上 |
| `漢\raise1pt\hbox{字}` | 20pt | Kなし |
| `漢\raise1pt\hbox{A}` | 17.50002pt | Xなし |
| `漢\hbox{\hbox{}字}` | 24pt | 先頭の空hboxを越えてmaterial K |
| `漢\hbox{\kern0pt 字}` | 20pt | 先頭kernがedgeを遮る |

production sliceはglyphとunshifted hboxだけを再帰してedge summaryを作る。未観測nodeを
推測で透明にせず、shifted hbox、vbox、明示kern/glue/rule等はedgeを遮る。箱edgeでは
JFM pair・禁則を推測適用せずK/X finalizer actionだけを置く。Kはfmt往復可能でTeXから
可視な`MaterialKanjiSkip`、直結glyph間は不可視な`VirtualKanjiSkip`のままである。

`\unhbox` / `\unhcopy`後は、内側で自動生成したK/Xを
外側listの最終値で再構成する。
内側K=7/X=8、外側K=2/X=5なら、展開後はK=2/X=5になる。
自動nodeと利用者が明示したglueを区別するprovenanceが必要である。

### discretionary

同じ公式binaryへpre/post/no-breakを独立に変えた入力を与えると、discは左右を単純に
接続する透明nodeではなかった。

- `漢\discretionary{}{}{}字`は20ptで、空discを越えるKはない。和欧の場合もXはない。
- no-breakが`中`なら、左の`漢`から`中`へはKを置かず、`中`から右の`字`へだけ
  material Kを置く。右が欧文ならmaterial Xになる。
- 改行時も左のglyphからpre-break先頭へは置かず、post-break末尾から右のglyphへだけ置く。
- pre/post/no-break内の直結和文glyph同士には通常の暗黙Kが効く。
- no-breakとpost-breakのscriptを変えると、右境界に必要なactionも枝ごとに変わる。

したがって単一の外側glueをdisc後へ置く実装は誤りである。productionでは各restricted hlistを
`unsave`前に個別finalizeし、枝内直結Kを`VirtualKanjiSkip`として保持する。利用者が書いたglueや
penaltyは従来のdiscretionary list制約で拒否し、PraTeX由来のJFM/K/X/禁則nodeだけを枝内に許す。

外側finalizerは左glyphからpre/no-break先頭へspacingを置かない。no-breakとpost-breakの末尾を
独立に要約し、次のglyphとのK/Xを各枝末尾へ条件付きnodeとして置く。ここでKは箱edgeと同じ
`MaterialKanjiSkip`であり、枝内直結の仮想Kへ混ぜない。pre-breakには外側右境界を付けず、空枝は
barrierのままである。packerとline breakerは枝内glueのwidth/stretch/shrinkを数え、改行後の枝移植と
DVI/PDF outputが実際に選んだno-break/post-breakだけを出力する。DiscNodeの公開3リストとfmt wireは
増やしていない。

### 方向bit

`xspcode`と`inhibitxspcode`は方向bitの論理積で判定する。

| 値 | 許可方向 |
|---:|---|
| 0 | 両方禁止 |
| 1 | 和文→欧文だけ |
| 2 | 欧文→和文だけ |
| 3 | 両方 |

## 3. JFMとK/Xはhybridにする

K/Xだけならlist-closeでの再評価が中心だが、JFM glue/kernと禁則は
和文main loopで早期に処理しなければならない。

黒箱では次の差があった。

- `）（`: JFM glue 1個
- `）{}（`または`）\relax（`: JFM glue 2個

空groupや展開不能commandはJFM pairの連続性だけを切る。
後段のK判定では両文字は依然として境界候補だが、class 0側JFMが左右どちらかに残る時は
そのJFMが元pair spacingを置換したものとしてKを足さない。両側とも無い時だけlate Kへ戻る。

JFM nodeはlistを閉じる前に`\lastnodesubtype`、`\unskip`、
pLaTeX側の除去macroから観測・除去され得る。
close時に全JFM nodeを消して作り直すと利用者操作を破壊する。

2026-08-23の最小production接続は、まずJFM/K/X/禁則がline break・box寸法・DVI座標へ
一度だけ届くことを固定するため、JFM/禁則もlist-closeでmaterializeする。これはhybrid最終形では
なく、実行中の`\unskip`や`\lastnodesubtype`に対するpTeX意味は未完成である。

2026-08-24には推奨構成のうち、通常glyph境界と確認済みnode-less commandについて
JFM/禁則をmain loopへ移した。closeはそれらを保持してK/Xだけを再評価し、`\unskip`で消えた
JFMを復活させない。shifted/vbox、disc内のnode-less境界、`\lastnodesubtype`を含む未検証commandの
全matrixは残る。

推奨構成:

1. main-control側
   - wide glyph
   - JFM glue/kern
   - pre/post penalty
   - JFM pair continuity
   - `\inhibitglue`
2. list-close側
   - K fallback
   - X
   - auto switch
   - `xspcode` / `inhibitxspcode`
   - box boundary
   - 自動K/Xの再評価

## 4. `\inhibitglue`

`\inhibitglue`はJFM metric由来の空白だけをone-shotで抑止し、K/Xの規則自体は禁止しない。

- JFM pairが実在する`）\inhibitglue（`はvisible JFM glueを置かず、抑止したJFMを暗黙Kへ
  置換もしない。pairが存在しない和和境界では通常の暗黙Kを残す。
- `\relax`、空group、register代入等のnode-less commandではpending状態を維持する。
- penalty、kern、rule、box、glyph等の実node一個の追加でpendingを消費する。
- 内側listから外へ漏らさず、外側pendingを内側listへ継承しない。
- `\disinhibitglue`はnodeを作らずpendingを解除する。

2026-08-25に`HorizontalMode`ごとのtyped pending stateとしてproduction接続した。JFMが
実在したが抑止された境界は`InhibitedByMainLoop` provenanceを右glyphへ持たせ、node-less境界、
fmt、`\unhcopy`を越えてJFMやKへ復活させない。公式e-upTeX 2026の自作probeでは、直接、
`\relax`、空group、整数代入の各境界が15ptから10ptになり、penalty追加または
`\disinhibitglue`では15ptへ戻った。probe入力SHA-256は
`61cac67dc5fbf477893730d36fca5317ea97358525c3634b3148711b6ff2da08`、binary/archive hashは
`docs/jfm-port-notes.md`に固定する。

後続checkpointで横組のdirect hbox/段落のlist先頭・末尾class 0もmain-loop eventへ接続した。
公開manualどおりhbox / `\noindent`の先頭glueを除去、hbox末尾glueの三成分を零化し、
字下げ段落の`\leavevmode\inhibitglue 【`は初期indent boxを論理端に数えず抑止する。
indirect box/disc、`\unhcopy`のedge再評価、shifted/vbox、縦組は未接続である。

## 5. 内部表現

中央決定箇所として`src/japanese_spacing.rs`相当を設け、
JFM/K/X/box境界の規則を利用側へ複製しない。

最低限必要な状態:

- wide glyphの符号位置、script、region、font、JFM class
- JFM pair continuity
- pending inhibit
- list単位の`needs_script_spacing`
- 自動K/X/JFMか利用者nodeかを示すprovenance
- boxのfirst/last境界summary

主な接続点:

- `src/main_control.rs`: 現在のCJK未対応診断を和文main loopへ置換
- `src/horizontal_mode.rs`: continuity、pending、fast flag
- `src/fonts.rs`、`src/jfm.rs`、`src/eqtb/fonts.rs`: JFM font接続
- `src/nodes.rs`、`src/format/dump_nodes.rs`: wide glyphとprovenance
- `src/box_building.rs`: `unsave`前のsnapshot/finalize
- `src/alignment.rs`: cellを`unsave`前にfinalize
- `src/line_breaking.rs`: 暗黙Kのbreak/幅/stretch/shrink
- `src/packaging.rs`: hpack測定とglue set
- `src/output.rs`とPDF backend: wide glyphと暗黙境界の移動
- `src/hyphenation.rs`: K/Xが欧文wordを切る境界
- `src/mode_independent.rs`、`src/eqtb.rs`: unskipとlast-node query

## 6. 性能を落とさない暗黙K

全和文境界へ隠し`GlueNode`を生成する実装はcorrectness checkpointには使える。
しかし純和文でnode数をほぼ倍増させるので最終形にはしない。

推奨最終形:

- wide glyphに`implicit_kanji_after` bit
- hlist単位に最終K spec
- line breaker、packer、outputが仮想glue eventとして扱う
- exceptional Kと全Xだけ実node

ASCII-only listは`needs_script_spacing=false`ならclose-time scanを0回にする。
Unicode script propertyとJFM classはglyph作成時に保持し、finalizerで再検索しない。
outer boxは中身を再走査せずfirst/last summaryを使う。

## 7. script-pair拡張

標準日本語は必ずcoreの`BuiltInPtex`経路に置く。

~~~text
BoundaryAtom {
  code_point,
  script_class,
  language_region,
  font_handle,
  metric_class,
  flags
}
~~~

listごとに一度だけdispatchを選ぶ。

- `BuiltIn`: pTeX/JLReq、日本語標準
- `CompiledTable`: 明示要求されたVaak/WASMがuploadした検証済み表
- `ExplicitWasm`: 低頻度で複雑なbounded batch

標準経路の条件:

- Vaak call 0
- WASM call 0
- trait object call 0
- JFM class-pair表をdirect index
- `xspcode`は`[u8; 256]`
- 空の`inhibitxspcode`表はbranch一つで既定3

Han–Latin、Hangul–Latin等も同じ`ScriptPairRule`へ載せる。
pTeX primitiveは`BuiltInPtex` profileへのadapterであり、
標準日本語paragraphをVaak/WASMへ委譲しない。

## 8. 横組みplanner slice

`src/script_spacing/planner.rs`は、node/main loopへ接続する前の中央決定層である。
PraTeXのengine identityやversionをpTeX/upTeXに偽装せず、組版profileを
`JapaneseSpacingProfile::BuiltInPtex`として明示的に選ぶ。primitive名は、対応する
eqtb/save/fmt/node意味が接続されるまで登録しない。

plannerの入力は元の文字境界、JFM font/metric/class identity、JFM連続性、list終端時の
`PtexSpacingState` snapshotである。出力は固定長`BoundaryActionPlan`であり、次を型で区別する。

- main loop phase: `KinsokuPenalty`、`JfmGlue`、`JfmKern`
- list finalizer phase: `ImplicitKanjiSkip`、`MaterialXKanjiSkip`

禁則は境界で「左文字のpost + 右文字のpre」を一つに合成し、JFM由来空白より先に出す。
同一JFM font/metricのclass対にglue/kernがあればそれをKより優先する。表に規則がない、
連続性が切れた、font/metricが変わった時は暗黙Kへ戻る。ただし`\inhibitglue`が実在する
JFM pairを抑止した境界だけは、そのJFMを暗黙Kへ置換しない。
異なるJFM font間の厳密なpTeX挙動は黒箱課題なので、現sliceでは誤ってclass対を横断しない
保守的fallbackとする。

和欧・欧和は、`xspcode`と`inhibitxspcode`を「和→欧=bit 0、欧→和=bit 1」の
同じ0--3 codecへ正規化し、両側の許可bitの論理積でmaterial Xを決める。
`noautospacing`は幅0の暗黙K境界を、`noautoxspacing`は許可済み境界に幅0のmaterial Xを残す。
plannerは既存の自動actionを入力に取らない純粋関数なので、自動provenanceだけを除いて
同じ元境界を再評価すれば冪等である。JFM/禁則はmain loop phaseだけ、K/Xは最終値を使う
list finalizer phaseだけをconsumerが適用する。

`PtexSpacingState`には`ptex-spacing-state-v1`の版付きfmt codecがあり、auto switch、K/X、
256要素xsp表、sparse inhibit/禁則表を全成分往復する。undumpは0--3、Unicode scalar、
glue寸法、疎表上限、entryの昇順・一意性を検証してからstateを公開する。現時点ではeqtb本体へ
fieldを追加していないので、このcodecが通常fmtへ書かれるとはまだ主張しない。

ASCII-only listは`ScriptSpacingListState::needs_script_spacing=false`のままになり、
`finalize_if_needed`はcallbackを呼ばない。このgateをprofile dispatch、JFM表引き、provider選択、
出力event生成のすべてより前に置く。したがって従来のplain欧文経路にはcallback、table lookup、
追加allocationを入れず、統合時にはorigin/mainとDVI意味列・座標を比較する。

### 2026-08-24 compiled table dispatcher checkpoint

`ScriptSpacingDispatcher`は一runの検証済みtableを所有し、compile成功後だけ世代を進めて公開する。
`Eqtb::dump`はtable・activation generation・provider handleを一切保存せず、undumpは空registryから始める。
各listはdirect glyphで見たgenerationと`LanguageRegion`を小さな値で記録し、close時に一度だけprofileを
選ぶ。list途中のreplace/revoke/region変更は新旧decisionを混ぜず、全体を`BuiltIn`へ戻す。

安定したdirect glyph境界では、Unicode scalarをtable固有`ScriptClassId`へ分類し、dense class pairを
allocation/dynamic dispatch/ABI callなしで引く。`BuiltInFallback`、no-space、K、X、fixed glueと、
現在のTeX nodeで意味を保てるboundary glue/break penaltyを中央finalizerがmaterializeする。
生成nodeは自動script-spacing provenanceだけをfmtへ往復し、provider IDは持たない。再finalize時はその
provenanceだけ除いて元glyphから再評価する。fmt後にregistryが空のままunboxした場合も旧compiled nodeを
検出してBuiltInへ戻し、過去runの表効果を漏らさない。WASM 0.0 candidateも同じcompile済みtableとinstall入口へ入る。

RegionNode未接続のindirect hbox/disc edge、非零adjustment tier、line-edge discardは終端値から推測せず
BuiltInへ戻す。これらと公開Vaak/runtime registrationは次checkpointであり、今回の接続をR3/R5/R6完了とは
数えない。標準pTeX/JLReq経路はこのdispatcherへtableを登録せず、Vaak/WASM callback 0を維持する。

### 2026-08-23基線と2026-08-24 hybridのproduction接続 checkpoint

`src/script_spacing/finalizer.rs`は、横組listに和文が一つでもある時だけ
`JapaneseSpacingPlanner`を一度選び、元の`WideCharNode` / `CharNode` / `LigatureNode`境界を
走査する。font load時にJFM class対をscale済みdense表へcompileし、hot loopはwide nodeが持つ
Unicode、font、metric、classだけを読む。標準経路はVaak/WASM registryへ入らない。

現在の`BuiltIn`禁則は、W3C JLReq 3.1.7 / 3.1.8とAppendix A.1/A.2を代表する
`、。`と横組括弧12対のbounded subsetだけである。code point表はplanner一箇所のvisitorにあり、
consumerへswitchを複製しない。
これは[JLReq 3.1.7](https://www.w3.org/TR/jlreq/#characters-not-starting-a-line)と
[JLReq 3.1.8](https://www.w3.org/TR/jlreq/#characters-not-ending-a-line)の全class実装ではない。

明示penaltyだけは文字境界に透明で、明示glue、kern、math、whatsit、rule等はbarrierである。
discは左側を遮断し、確認済みno-break/post-break末尾だけを右側候補として扱う。`{}`、`\relax`、
`\unskip`、`\message`、semi-simple group、`\showthe`、
整数register代入はnodeを足さずJFM continuityを切る確認済み境界である。listのうち確認済み
unshifted hboxだけはedge summaryを使い、shifted hboxとvboxはbarrierのままにする。
main-loop JFM/禁則のtyped provenanceはfmt/unhcopyを越えて保持し、再finalizeはclose-time K/Xだけを
除いて元境界から再生成する。hbox、段落、alignment cell、display math移行の`unsave` / pop前に
snapshotし、局所K/Xを失わない。

このcheckpointのrelease試験は627 passed、0 failed、7 ignored。合成JFM/TFMのproduction試験
6件でnode幅、局所group、alignment、fmt、再finalize、barrier、line break、DVI座標を固定した。
origin/mainのplain欧文DVI page body 183 bytesはbyte差分0で、公式CTAN TRIPも既知正常hashを維持した。

hybrid接続後はmain-loop oracle、削除保持、fmt/unhcopyを16試験へ広げ、全releaseは
846 passed、0 failed、10 ignoredである。公式e-upTeX binaryとarchiveのSHA-256は
`docs/jfm-port-notes.md`へ固定した。

### 接続済みhookと残件

- auto switch、`[XspCode; 256]`、sparse inhibit表はtyped eqtb、save stack、fmtへ接続済み。
- 通常glyphと確認済みnode-less境界ではmain-loop phaseだけをmaterializeし、list closeは
  自動K/X provenanceだけを除去して`unsave`前の最終snapshotで再適用する。
- `\inhibitglue` / `\disinhibitglue`はhlist-localなone-shot状態として接続済み。JFMの有無を
  planner一箇所で判定し、抑止済みprovenanceをfmt/unboxへ保持する。

1. line breaker、packer、DVI/PDF backendは`ImplicitKanjiSkip`を同じ仮想glue eventとして読む。
   plain欧文DVI differentialを先に固定し、その後に和文event oracleを追加する。
2. shifted/vbox、discの全JFM class・禁則・unbox matrix、未検証commandの条件付きevent、
   全JLReq文字class、縦組へ広げる。

## 9. commit順

1. K/Xを通常glue parameterとして追加し、generic意味とfmtを固定
2. auto switch、query、xsp/inhibit表をtyped eqtb state化
3. 和文font、wide glyph、JFM main loop
4. JFM provenance、禁則、inhibit state
5. hbox、paragraph、alignmentの`unsave`前finalization
6. 実K/X、box境界、unbox再評価
7. line breaker、packer、DVI/PDF output
8. 隠しGlue checkpointから仮想Kへsafe-Rust最適化
9. script-pair built-in tableと明示opt-in provider境界

## 10. 集中試験

- `漢字間隔と和欧文間隔はinitexでは零である`
- `漢字間隔は局所代入から復元される`
- `漢字間隔はglobaldefsと算術代入に従う`
- `漢字間隔と和欧文間隔をfmtで往復する`
- `showtheは糊の全成分を表示する`
- `段落末の漢字間隔を段落全体へ適用する`
- `暗黙の漢字間隔はshowboxとlastskipへ現れない`
- `自動間隔を無効にしても零境界で改行できる`
- `左右の許可bitの論理積で和欧文間隔を決める`
- `空群はjfm連結だけを中断する`
- `inhibitglueはnode追加まで維持される`
- `上下移動した箱を和文境界とみなさない`
- `箱を開いた後は外側の最終間隔値で組み直す`
- `配列cellはunsaveより前の間隔値で閉じる`
- `標準日本語組版はvaakとwasmを呼ばない`
- `純欧文listは和文finalizerを通らない`

## 11. 残る黒箱課題

- rule、kern、box末尾の非glyph node等のJFM/K/X edge matrix
- discretionaryの全JFM class・禁則順序、node-less境界、unbox再評価
- inline math、accent、ligature、language whatsit
- 異なるJFM font間、方向変更、縦組
- `\unskip`後に片側JFMだけが残る組合せ、box/discを含むunbox再評価
- pre/post penaltyとglueの厳密な順序
- `inhibitxspcode`の範囲外回復と上限
- 仮想Kのinfinite shrink回復とpre-display-size
- DVI/PDFでstretch済み暗黙Kの座標

これらも公開manualと自作black-box fixtureで埋める。

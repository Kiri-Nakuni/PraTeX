# `\overloaddef` 多節macro設計

状態: **設計のみ。primitive、command型、fmt表現、試験は未実装。**

## 目的

PraTeX固有の `\overloaddef` は、一つの制御綴へ複数のTeX型parameter textを登録し、
呼出しtoken列に対して最も具体的な一節だけを展開する。通常の `\def`、`\gdef`、`\edef`、
`\xdef` の「新しい定義が以前の意味を置換する」契約は変更しない。

これは値の型や展開結果で選ぶfunction overloadではない。未展開token列へ複数の
macro parameter patternを照合する **多節macro** である。標準TeX macroの通常経路、性能、
`\ifx`を暗黙に変えず、`\overloaddef`で作られた新しいcommand型だけがdispatch費用を負う。

TeXのparameter textではdelimiterを文字列でなくcatcodeを含むtoken列として保存し、
引数を展開せず、braceをbalanceしながら正確なdelimiter列まで読む。この既存規則を保つ。
丸括弧、comma、period、semicolon自体にnestingの意味は与えない。参考仕様は
[TeX by Topic, Chapter 11](https://tug.ctan.org/info/texbytopic/TeXbyTopic.pdf)とする。

## V0の公開構文

V0ではfamily共通のcall terminatorを定義ごとに明示する。

```tex
\overloaddef\a{;}.#1(#2);{一引数節: #1 / #2}
\overloaddef\a{;}.#1(#2,#3);{二引数節: #1 / #2 / #3}
\overloaddef\a{;}.#1(#2).#3(#4);{chain節: #1 / #2 / #3 / #4}
```

構文要素は順に次である。

```text
\overloaddef <定義対象token> {<family terminator>} <parameter text> {<replacement text>}
```

- V0のterminator指定は **catcode 12の一文字tokenをちょうど一個** 含む。
- 各parameter textの末尾にも同じterminator tokenを明記する。terminatorは末尾に一度だけ現れ、
  それより前のbrace level 0位置へ同じtokenを書いてはならない。定義時とundump時に拒否する。
- 同じfamilyへ追加する全節はterminatorとfamily属性を共有する。
- parameter text、`#1`から`#9`までの連番、delimiter、replacement text、`##`は
  通常の `\def` と同じtoken規則を使う。
- 最後のdelimiterからterminatorを暗黙推論する糖衣、control sequence／複数token terminator、
  terminatorの異なる節はV0へ入れない。

呼出しは次のようになる。

```tex
\a.f(x);
\a.f(x,y);
\a.f(x).g(y);
```

terminatorはbrace level 0で予約tokenになる。引数dataとして使う時はbraceで保護する。

```tex
\a.f({x;y});
```

## なぜ競合規則が必要か

上の三節はTeXのdelimiter規則では排他的でない。

- `.f(x,y);` は一引数節にも一致し、その時の `#2` は `x,y` になる。
- `.f(x).g(y);` も一引数節に一致し、その時の `#2` は `x).g(y` になる。
- `.f((x,y));` の内側commaは丸括弧で保護されないため、二引数節にも一致する。
- `.f({x,y});` のcommaはbrace内なので、二引数節のdelimiterにはならない。

最初に一致した節、最後に定義した節、literal数が最大の節を選ぶ規則は採用しない。
それらは定義順や恣意的なtie-breakによって、後から節を追加した時に既存呼出しの意味を
黙って変えるためである。

## call frameと照合

呼出し時は次の順序をカノンにする。

1. family terminatorまでを、展開せず一つのbounded call frameへ収集する。
2. brace内のterminatorはcallを閉じない。brace level 0の最初のterminatorだけが閉じる。
3. 全節を同じframeへ副作用なしに照合する。
4. 各節では通常TeXと同じ左から右のdelimiter走査を行い、backtrackingしない。
5. parameter textがframe全体へ完全一致した節だけを成功候補にする。
6. 候補を選び終わるまでreplacement textを実行せず、argument代入を外へ公開しない。

通常macroで行うargumentの正規化も照合結果へ含める。特に、引数全体が一つのbrace groupだった
場合の外側brace除去を省略せず、delimiter位置だけを似せた別scannerにしない。

terminator、frame token数、節数、pattern総量、brace深さには実装前に明示上限を定める。
終端なしの無制限先読み、入力streamへの投機的なread-back、指数的backtrackingは採用しない。

## specificityの定義

成功候補 `P` が、call frame中でparameterでなくpattern literalとして照合した絶対token位置の
集合を `F(P)` とする。family共通terminator位置は全候補に共通なので比較から除く。

```text
PよりQが具体的  <=>  F(P) は F(Q) の真部分集合
```

真部分集合関係で他候補に支配された候補を除く。選択結果は次の三通りだけである。

- 成功候補が一つなら、その節を選ぶ。
- 非支配候補が一つだけなら、その節を選ぶ。この節は他の全成功候補より具体的である。
- 非支配候補が複数ならambiguity errorにする。

候補が一つもなければno-match errorにする。定義順はdispatch結果へ影響しない。
完全に同じparameter textの再登録は別候補を増やさず、同じ節のreplacementを置換する。

### 例

| call frame | 成功候補 | 結果 |
|---|---|---|
| `.f(x);` | 一引数節 | 一引数節 |
| `.f(x,y);` | 一引数節、二引数節 | comma位置を追加固定する二引数節 |
| `.f(x).g(y);` | 一引数節、chain節 | 中間の `).` と次の `(` を追加固定するchain節 |
| `.f(x,y).g(z);` | 三節すべて | 二引数節とchain節が比較不能なのでambiguity error |

最後の呼出しを許す時は、両方の構造を固定する節を明示する。

```tex
\overloaddef\a{;}.#1(#2,#3).#4(#5);{commaつきchain節}
```

この節は対象call上で二引数節とchain節のliteral位置をともに包含するため、一意に選ばれる。
この規則はcallごとの具体性を定めるものであり、二つのpattern language全体の包含証明ではない。
比較不能をpriorityで黙って解消しない。

異なるparameter textが同じcall上で同じliteral位置集合を持つ場合も、一方を選ばない。例えば
`a#1b#2;` と `#1ab#2;` はcall `ab;` で同じ位置の `a` と `b` をliteralとして固定するが、
signatureは同一でないためduplicate置換ではなくambiguity errorになる。

## familyのTeX意味論

### prefixと展開

- `\long`、`\outer`、`\protected`は節ごとでなくfamily全体の属性にする。
- 各 `\overloaddef` はprefixの有無を通常定義と同様に明示する。prefix省略は既存属性の継承でなく
  falseである。したがって既存の `\long` familyへ節を足す時も `\long\overloaddef` と書き、
  追加定義の属性が既存familyと異なる時はfamilyを変更せずerrorにする。
- `\overloaddef`はreplacementを未展開で保存する。
- familyは通常macroと同じexpandable commandであり、選択後のargument substitutionだけを
  通常の展開列へ戻す。
- `\protected` familyは展開抑止を判断する時点でdispatch自体を行わない。
- `\edef`内のprotected family tokenと `\noexpand`直後のfamily tokenは、その一tokenだけを通常macroと
  同じ規則で残す。後続のcall token列を暗黙にframeとして保護せず、それぞれ通常どおり展開するため、
  実行時の照合結果が変わり得る。call全体を保持したい時は明示的な未展開化を使う。
- `\overloadedef`に相当する展開定義はV0へ入れず、通常の `\edef` の意味も変えない。

`\long`を節ごとにすると、terminatorまで収集中に `\par` を許すかを節選択前に決められない。
`\outer`と`\protected`も節選択より前に意味を持つため、family属性でなければならない。

### 代入、group、`\let`

- 最初の `\overloaddef` は通常の定義代入と同様に対象の以前の意味を置換し、familyを作る。
- 既存familyへの節追加と同一signature置換は、family全体のimmutable snapshotを新しいeqtb値として
  代入する。共有中のfamilyをin-place mutationしない。
- local追加／置換はgroup終了時に以前のfamily全体へ戻る。
- `\global\overloaddef`と `\globaldefs` は通常の定義代入経路を使う。
- group内でlocalに変更したfamilyへ続けてglobal追加する時は、現在見えているlocal family全体を基点に
  新しいglobal snapshotを代入する。group外に隠れた古いfamilyだけへ追加する特例は作らない。
- `\let\b=\a` はその時点のfamily全体をsnapshotとして複製する。その後の `\a`への節追加を
  `\b`へ伝播させない。
- 通常の `\def`、`\edef`、`\let`による別meaningの代入はfamily全体を置換できる。

### `\ifx`、表示、fmt

- overloaded familyと通常macroは、節が一つでも `\ifx` falseとする。
- 二familyの `\ifx` はcommand型、family属性、terminator、signatureからreplacementへの対応を
  登録順に依存せず構造比較する。pointer identityやdispatch cacheを意味へ含めない。
- `\meaning`と `\show`は通常macroを装わず、`overloaded macro:`に続けてterminator、属性、
  節をparameter token列のcanonicalな全順序で表示する。登録順は観測可能な意味にしない。
- fmtはfamily属性、terminatorの完全なtoken identity、節列、parameter/replacement token列を保存する。
- undump時に件数、長さ、parameter番号、terminator、duplicateを再検証する。
- matcher、trie、cache、pointer、run-local IDはfmtへ保存せず再構築する。
- 新command tagを導入するfmt schema/versionを明示的に上げる。新engineは従来fmtを読めるが、
  overloaded familyを含む新fmtを旧engineが通常macroとして誤読しないようversion不一致で拒否させる。

canonical全順序のtoken identity符号化は、文字tokenのcommand/catcode/characterとcontrol sequence
identityを失わず、fmt往復前後で同じ順序になるものとしてO1着手前に固定する。

## error recovery

- terminator欠落、禁止された `\par`、outer token、brace不整合、alignment境界は、通常macro scannerと
  同じscanner status、warning index、回復順序を使う。
- no-matchとambiguityではcall frameを一単位として消費し、どのreplacementも実行しない。
- 部分的なargument、選択途中の状態、family変更を残さない。
- nonstop処理ではterminator直後から継続する。診断にはtarget、call frame、成功候補のparameter textを
  boundedに表示する。
- alignment tabやfile EOFを越えてterminatorを無制限に探さない。
- frame長やbrace深さの上限を越えた時はreplacementを実行せず、別の有限なresynchronization上限内で
  braceを追跡しながら同じterminatorまで捨てる。terminatorへ到達すれば直後から継続し、既存scannerの
  alignment／outer／EOF境界またはresynchronization上限へ達した時はrunawayとしてjobを中止する。
  切り取られたframe後半を通常入力として再実行しない。

## V0の非目標

- 通常の `\def`を多重定義化すること
- 値、型、展開結果、現在modeによるdispatch
- 丸括弧をbraceのようにbalanceする別parser
- 定義順、最終定義、最大literal数によるpriority
- 自動suffix推論、複数token／control sequence terminator
- 節ごとに異なる `\long`、`\outer`、`\protected`
- 10個以上のparameter、可変個parameter番号
- Vaak／WASM callbackによる標準dispatch

丸括弧やcommaを構文木として解釈し、引数個数を厳密に数えるDSLは将来別機能として設計できる。
それはTeXのmacro parameter text互換ではないため、`\overloaddef` V0へ混ぜない。

## 実装段階とcompletion gate

| 段階 | 範囲 | gate |
|---|---|---|
| O0 | 本設計、構文、ambiguity、非目標の固定 | 設計のみとinventoryへ明記 |
| O1 | family型、定義scanner、local/global、duplicate置換、表示 | 通常 `\def`の回帰とgroup／fmt unit |
| O2 | bounded frame、全節match、literal位置包含、原子的error | 上の四call、brace保護、no-match、incomparable候補 |
| O3 | `\long`／outer／protected、`\let`／`\ifx`、`\edef`、alignment | 通常macroと同じ回復順序をfocused process testで固定 |
| O4 | fmt破損matrix、全release、plain DVI、公式TRIP | 既存macro・LaTeX・TRIPを変えないことを確認 |

実装を対応済みと数えるのはO4後である。O0文書だけの状態でprimitive名、fmt command tag、
診断文、数値上限がproductionに存在すると表示しない。

## 最小試験行列

- 一引数、二引数、chain、commaつきchainの一意選択／ambiguity
- 同じliteral位置集合を持つ別signatureのambiguity、途中terminatorを持つ死んだsignatureの定義拒否
- empty delimited argument、単一brace groupの外側brace除去、同じdelimiterの反復、
  brace内comma／terminator、catcode違い
- duplicate signatureのlocal/global置換、group復元、local snapshot後のglobal追加、`\globaldefs`
- `\let` snapshot、通常macroとの `\ifx` false、family同士の構造比較、`\meaning`／`\show`
- family属性の一致／不一致、`\long`の `\par`、outer回復、protected family、
  `\edef`／`\noexpand`で後続call tokenが保護されないこと
- terminator欠落、no-match、複数最大候補、EOF、alignment中の `&`／`\cr`
- frame／brace／resynchronization上限、fmt往復、旧schema、破損した件数／parameter番号／
  terminator／duplicateの拒否
- 通常 `\def`、macro引数scanner、plain DVI、LaTeX smoke、公式TRIPの非回帰

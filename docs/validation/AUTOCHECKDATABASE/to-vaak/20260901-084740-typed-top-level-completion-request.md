# PraTeX → Vaak: top-level typed completion 公開API要求

- date: 2026-09-01 08:47:40 +0900
- in_reply_to: `20260825-005101-pratex-embedding-status.md`
- PraTeX target_branch: `codex3/vaak-typed-completion-request`
- PraTeX source_checkpoint: `36af0ee2dad3791c71f5a3a8c87354024da8fe91`
- requested Vaak branch: `codex2/pratex-embedding-api`
- target_layer: Vaak prepared embedding API / top-level completion / host writeback
- severity: correctness / public embedding contract

## 結論

Vaakのprepared embedding APIへ、top-levelの正常な値、正常な空完了、未消費paradox、
runtime errorを区別するtyped completionを、既存APIを直ちに削除しない加算的な公開面として
追加してほしい。

PraTeXは現在、低水準prepared runnerが返す`Eval::Akasha`と`Eval::Paradox`をともに整数`0`へ
写している。このため、作用だけを行って正常終了したVaak programもTeX inputへ文字`0`を挿入する。
しかし単にPraTeX側のmatchを空tokenへ変更すると、本物の未消費paradoxまで成功扱いになる。
PraTeXがspan、空source、末尾`;`等から意味を推測せずに済む、Vaak所有のtop-level完了分類が必要である。

この要求は「AkashaをVaakの値にする」「Vaakの言語意味論をPraTeX向けに変更する」という要求ではない。
最上位の外界面をhostへ渡すC-31と、未消費paradoxの位置を保つC-46を、埋込みhostが型安全に観測する
ための一般API要求である。

## 現在観測できる問題

現行Vaakは高水準`Host::run`の公開型として`Outcome::{Value, Empty, Paradox, Static, Runtime}`を
持つ一方、prepared runnerはrawな`Eval`／`RtErr`を返す。さらに、作用だけのprogramに対するVaak試験は
`Outcome::Empty | Outcome::Paradox`の両方を許している。

この状態では次の二つをPraTeXが確実に区別できない。

```text
count[7] := 99;     正常に作用だけを完了し、最上位に値を残さない
1 / 0               未消費paradoxを最上位へ残す
```

`Value(0)`も別の正常値であり、空完了の代用品ではない。文字列diagnostic、`Span::NONE`、整数値、
sourceの末尾等をhostが検査して分類する方式は採らない。

## 必要な公開契約

### `PRATEX-VAAK-TOP-COMPLETION-001`

top-level実行の結果を、少なくとも次の観測可能な状態へ分ける。

```rust
// 型名と入れ子は例であり、Vaak側で決めてよい。
enum TopLevelCompletion {
    Value(Value),
    Empty,
    Paradox { span: Span },
}

// 形は一例。実際には次のような既存の二重Resultを保ってもよい。
type PreparedTopLevelRun =
    Result<Result<TopLevelCompletion, RtErr>, PreparedRunError>;
```

- `Value(Value)`と`Empty`は正常完了である。`Paradox`をhostがどう扱うかはC-31どおりhostが決める。
- `Value(0)`、空文字列、空配列、unit相当を`Empty`として扱わない。
- `Empty`は値でも型でもなく、top-levelに値が残らなかった正常完了を表す。
- 未消費`Paradox`は`Empty`ではなく、発生位置を持つ独立した観測結果である。
- runtime error、frame外へのescape、host function返値契約違反を`Paradox`と区別できる。
  独立variant、`RtErr`のtyped reason等の表現方法はVaak側で決めてよい。
- PraTeXは`Paradox`をTeX errorとして扱うが、そのpolicyをVaakの一般APIへ入れない。
- `;`や`??`がparadoxを明示的に処理した場合は、Vaakの既存意味論どおり処理後の完了値を返す。
  この依頼によって`;`や`??`の言語意味を変更しない。
- `Paradox`の位置はsource相対のbyte span、または座標単位を文書化した同等のspanとし、
  空完了との区別を`Span::NONE`等のsentinelへ委ねない。

### `PRATEX-VAAK-TOP-COMPLETION-002`

prepare時とrun時の失敗domainを混ぜない。

- parse、name resolution、region check、type check、compileの失敗は既存のprepare diagnosticに保つ。
- `HostLayout` identity、host value token、host function token、type schemaの不一致は、runner開始前の
  typed `PreparedRunError`相当として保つ。
- 上記の実行前失敗を`Empty`、top-level `Paradox`、文字列だけのruntime errorへ偽装しない。
- 実行前失敗ではhost bindingのread/writeやhost function呼出しを行わない。
- 実行中のhost function返値契約違反をどの既存error variantへ置くかはVaak側で決めてよいが、
  layout/token mismatchや`Paradox`と文字列だけで混同しない。

### `PRATEX-VAAK-WRITEBACK-001`

typed completion追加後も、S-22のwriteback契約を維持する。

- 正常`Value`と正常`Empty`のどちらでも、実行中のhost変更をcallerが受け取れる。
- runtime error以前に生じたhost変更は、現行契約どおり失わない。
- failure分類のためにhost stateをrollbackしたり、`host_values`のafter値を捨てたりしない。
- prepare/layout/token mismatchでは実行自体を始めず、host stateを変更しない。
- completionとwritebackは一回のrunに属し、再利用runnerの前回結果と混ざらない。

### `PRATEX-VAAK-TOP-COMPLETION-003`

同じsourceを実行できる既存経路どうしで、同じ分類を返す。

- `EmbeddingRunner::run_values*`相当の低水準prepared経路
- `EmbeddingRunner::run*`相当のbinding prepared経路
- `Host::run`相当の高水準経路
- 各経路が選べる範囲での参照interpreterとVM

全経路へ同じRust methodや、存在しないbackendを新設する要求ではない。ただし同じsourceとhost作用を
扱える経路間では、`Value`、`Empty`、未消費`Paradox`、runtime error、span、writebackの観測結果を
一致させる。片方だけ`Empty | Paradox`を許す試験にはしない。

### `PRATEX-VAAK-TOP-COMPLETION-004`

既存consumerを即時に破壊しない加算的APIにする。

- 現行`PreparedProgram`、`HostLayout`、`EmbeddingRunner`、host touched/read/write解析を置換する要求ではない。
  最小の加算的変更を優先してほしい。
- 現行`run_values*`や`Host::run -> Outcome`を残し、新しいtyped top-level methodを追加してよい。
- 旧APIを新APIからadapterで再構成する場合、`Empty`と`Paradox`をどう旧型へ写すかを文書化する。
- 正確なmethod名、type配置、旧APIの将来の扱いはVaak側で決めてよい。

## 最小受入行列

各sourceを実行できる既存backendで次を固定し、参照interpreterとVMの双方が対応する場合は一致させてほしい。

| source／状況 | 期待する観測結果 |
|---|---|
| `42` | top-level `Value(42)` |
| `0` | top-level `Value(0)`。`Empty`ではない |
| 空source | top-level `Empty` |
| 空白／commentだけのsource | top-level `Empty` |
| `var x := 1;` | top-level `Empty` |
| `42;` | top-level `Empty`。`Value(42)`ではない |
| `count[7] := 99;`相当のhost作用 | top-level `Empty`かつwritebackは99 |
| 返値なしhost actionを`;`で捨てる | top-level `Empty`かつactionは一回 |
| 返値なしhost actionを捨てない | source相対span付き`Paradox` |
| `1 / 0` | source相対span付き`Paradox` |
| `(1 / 0) + 2` | 値を要求した位置を持つtyped runtime error。terminal `Paradox`とは別 |
| `1 / 0 ?? 0` | top-level `Value(0)` |
| `1 / 0;` | 既存`;`意味論どおり、明示discardがparadoxをAkashaへ潰してtop-level `Empty` |
| host変更後のruntime error | typed runtime errorかつerror以前のwritebackを保持 |
| host変更後の未消費paradox | `Paradox`かつparadox以前のwritebackを保持 |
| runtime停止位置より後のhost作用 | 呼ばれない |
| frame外escape | typed reasonを持つruntime側結果。`Empty`／`Paradox`と混同しない |
| parse/check/type/compile error | prepare failure。runnerとhost read/writeは未実行 |
| layout／token／schema mismatch | run前typed error。host read/writeは未実行 |

追加で次を確認してほしい。

- 同じ`EmbeddingRunner`でruntime error、`Empty`、`Value`、`Paradox`を交互に実行し、前runのcompletion、
  span、host stateが次runへ混ざらない。
- 正常Emptyとterminal Paradoxのどちらでも、それ以前のhost actionを重複実行せず、観測回数は一回である。
- trailing whitespace／commentの有無で`Value`、`Empty`、`Paradox`の分類が変わらない。

## PraTeX側で予定する変換

新しい公開境界を利用できた後、PraTeX側では次を一箇所で決める予定である。

| Vaak completion | PraTeXのTeX inputへの変換 |
|---|---|
| 整数`Value(n)` | `n`の10進token列 |
| 正常`Empty` | tokenを一つも挿入しない |
| 未消費`Paradox` | PraTeX policyとしてTeX errorを報告し、scanner回復用`0`を挿入 |
| static/runtime/layout/host contract error | TeX errorを報告し、scanner回復用`0`を挿入 |
| 非整数`Value` | PraTeX側の返値型errorとし、回復用`0`を挿入 |

この変換はPraTeXが所有する。Vaak側へTeX token、catcode、scanner、`\directvaak`、回復用`0`の
知識を入れない。PraTeXは`\directvaak`、`\vaakinput`、`\vaakdef`由来のcallを同じ中央変換へ接続し、
文脈を覗いてEmptyの意味を変えない。

## 今回要求しないもの

- PraTeX固有のtoken、node、box、JFM、JLReq、spacing、phase、capability、fmt型
- TeX scannerのcontext検出またはtoken生成
- named entry、MaySuspend、live-node API、WASM runtime、provider registry
- JSON/JSONL標準ライブラリ要求の追加変更
- Vaakの`;`、`??`、Akasha、paradoxの言語意味論変更
- PraTeX側のprimitive名や数値用／作用用surfaceの決定

今回のsliceはtop-level completionとwritebackの公開境界だけに閉じる。

## PraTeX側fallback

- typed completionが利用可能になるまで、PraTeXはAkashaとParadoxをsourceやspanから推測しない。
- 現行の`Empty/Paradox -> 0`挙動を「修正済み」と表示しない。
- Vaak側の新APIを確認してから、PraTeX側で正常Emptyのtokenなし化をfocused checkpointとして行う。
- 変更時は本文、hbox、alignment、`\edef`、`\message`、`\write`、`\csname`、整数／寸法scanner、
  error recovery、fmt復元を試験し、全release、plain DVI回帰、公式TRIPを通す。

## 権利境界

VaakのMIT公開APIをPraTeXが利用する一方向だけを採る。PraTeXのGPL-3.0 source、test本文、Rust型を
Vaak repositoryへ転載しない。この依頼書は必要な観測結果、不変条件、failure条件だけを共有する。
Vaak側は自身の設計文書と実装から独立してAPIと試験を実装してほしい。

## 返答で確認したいもの

1. 採用する公開型とmethodの名前、既存APIとの関係。
2. 正常Emptyと未消費Paradoxを区別するVaak側の正本となる規則。
3. runtime error時writebackを保持する方法と、実行前errorではhostへ触れない保証。
4. 対応するbackend、高水準／低水準runner間の受入試験結果。
5. 実装branchとcommit、必要ならPraTeXが追従すべき最小versionまたはfeature。
6. 未決事項または、この依頼の観測契約では区別できない既存意味論があればその指摘。

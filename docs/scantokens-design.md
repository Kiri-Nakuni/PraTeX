# `\scantokens` 疑似入力の設計

更新: 2026-08-23

## 目的と状態

この文書は、e-TeX の `\scantokens` を PraTeX へクリーンルーム実装するための
意味論と所有境界を固定する。2026-08-23にtyped疑似入力、自然EOF、fmt、tracingまで
実装し、KOMA-Scriptが必要とする動的catcode再走査を実物で確認した。下の一覧のうち
raw byte、診断context、資源超過などの未充足試験と`\pausing`の黒箱監査は引き続き残る。

`\scantokens` は単なる `\detokenize` と token の差し戻しではない。未展開の
general text を文字列へ戻した後、その文字列を file と同じ字句経路へ入れる。
したがって category code、`^^` 記法、コメント、制御綴、行末、EOF、診断 context を
既存の入力機構と共有しなければならない。

## クリーンルームの境界

実装仕様に用いる資料は次の二種類だけである。

- 公開された *The ε-TeX manual* §3.7:
  <https://texdoc.org/serve/e-TeX/0>
- 公開された *The e-TeX Short Reference Manual*:
  <https://tug.ctan.org/systems/doc/etex/etex_ref.html>
- WSL 上の e-upTeX 3.141592653-p4.1.2-u2.02-251130-2.6
  (TeX Live 2026) に対する、この監査で新しく作った最小入力の黒箱観測

e-TeX、pdfTeX、pTeX、upTeX、e-upTeX の実装 source と付属 test は参照しない。
LaTeX、expl3、package source は到達度を測る入力にはできるが、意味論の根拠にはしない。
黒箱入力は公開 manual の曖昧な境界を識別するためだけに作り、出力から内部データ構造を
推定しない。

## 公開契約から確定する意味

`\scantokens{<general text>}` は次の順に働く。

1. balanced text を**展開せず**に吸収する。
2. token 列を、現在の `\escapechar` と `\newlinechar` を使って文字表現へ戻す。
3. その文字表現を所有する疑似 file を、現在の入力より先に積む。
4. 疑似 file の各行を通常の入力字句器で読み、読取時点の catcode、kcatcode、
   `\endlinechar` を適用する。
5. 命令自身の展開結果は空だが、疑似 file から得た token は呼出し位置へ流れ込む。

`\scantokens` は expandable である。例えば `\edef` の走査中なら、疑似 file の token は
その `\edef` が続けて吸収する。文字を category 12 の token にして差し戻す実装では、
catcode変更、コメント、制御綴、`^^xy` の再解釈が失われるため不適合である。

## scanner との境界

引数の吸収には既存の `token_lists::nested_scan_toks(scanner, false, ...)` を使う。
`Scanner::scan_toks` は `def_ref` を作り直すので、直接呼ぶと外側の `\edef`、`\message`、
`\write` 等が既に溜めた token を失う。`nested_scan_toks` が次を控えてから戻す一箇所を
通す。

- `def_ref`
- `scanner_status`
- `warning_index`

黒箱では次の入力が `macro:->beforemiddle after` になった。`before` が残ることと、既定の
`\endlinechar` が `middle` の後の空白になることを同時に固定できる。

```tex
\everyeof{\noexpand}
\edef\x{before\scantokens{middle}after}
```

展開 dispatch は疑似 source を積んだ後、そのまま返す。`scanner.ins_list` や
`printed_str_toks` で結果を token list にしてはならない。

## typed な疑似 file buffer

### LF sentinel を使わない理由

文字 byte と論理改行は型で区別する。次の表現を初期案とする。

```text
PseudoText {
    bytes: Vec<u8>,
    line_ends: Vec<usize>,
    next_line: usize,
}
```

専用の `PseudoFilePrinter` は生成開始時の `\newlinechar` を snapshot する。出力する
code point がその値なら byte 列へ入れず、現在行の終端 offset を `line_ends` へ追加する。
それ以外は `bytes` へ入れる。

黒箱では、`\newlinechar=-1` のとき category 12 の raw byte 10 が疑似入力後に
active byte 10 として実行された。raw byte 13 も論理改行の直前で除かれず、active byte 13
として実行された。よって次の実装はどちらも意味を壊す。

- raw LF を論理行の sentinel にする `Cursor<Vec<u8>>`
- CRLF として raw CR を除く、実 file 用 `read_line()` のそのままの再利用

`print_uptex_char(code_point, bytes)` は code point 単位で `\newlinechar` と比較する。
UTF-8 の continuation byte が偶然同じ値でも、和文・Unicode文字の途中を分割しない。
疑似 file を `LineLexer` へ渡した後の文字分類は既存 `CharacterClassifier` だけが担い、
別の catcode/kcatcode 判定を作らない。

### 行の確定

行末の ASCII space は外部入力と同様に除く。ただし、空 payload と空白だけの payload は
区別する必要がある。builder は `current_line_had_input` を持つ。

- 非改行文字を一つ印字したら `current_line_had_input = true`
- newlinechar を印字したら trailing space を除き、空でも一行を記録して false に戻す
- 生成終了時は true の場合だけ最後の行を記録する

この規則による黒箱結果は次のとおりだった。

| payload | 論理行 | 自然EOF時の `\inputlineno` |
|---|---:|---:|
| 空 | 0 | 1 |
| space 一つ | 空行1つ | 2 |
| newlinechar 一つ | 空行1つ | 2 |
| 非空の N 行 | N | N + 1 |

末尾の newlinechar が終端した行は一つ存在するが、その後に余分な空行を足さない。連続する
newlinechar は同じ byte offset を複数回 `line_ends` へ持てるので、それぞれ空行になる。

## `\newlinechar` と `\endlinechar`

両者の時点は異なる。

- `\newlinechar` は疑似 source の**生成時**に固定する。
- `\endlinechar` は各論理行を `LineLexer` へ渡す**読取時**の現在値を使う。
- 0--255 の外にある `\newlinechar` は「論理改行文字なし」である。
- 0--255 の外にある `\endlinechar` は行末へ何も足さない。

三行の疑似 source で、第一行は文字を実行し、第二行で `\endlinechar` を変更し、第三行で
再び文字を実行する黒箱では、第一・第二行に旧値、第三行に新値が付いた。第二行そのものは
字句化前に旧 `\endlinechar` を受け取るためである。

疑似 source の第一行で `\newlinechar` を変更しても、既に作られた残りの行境界は変わらなかった。
一方、そこで新しく呼ぶ入れ子の `\scantokens` は変更後の値を snapshot する。

## 入力 stack と所有

`InputSource::TextSource` の `SourceType` に `PseudoFile` を加える。独立した
`InputSource` variant にすると通常 token ごとの判別を増やすため、source 種別は既存どおり
行が尽きた時だけ見る。

```text
SourceType::PseudoFile {
    reader: PseudoText,
    every_eof_seen: bool,
    trace_opened: bool,
}
```

疑似 source は byte buffer、行終端表、読取位置を単独所有する。EOF、`\endinput`、engine
shutdown のいずれでも一度だけ破棄する。実 file を疑似 source の中から `\input` した場合は、
既存の `line_number_stack` に疑似行番号を控え、子 file 終了後に同じ疑似行へ戻る。

入れ子の黒箱では次の順を確認した。

```text
外側疑似 l.1
  内側疑似 l.1, l.2
  内側自然EOF l.3
外側疑似 l.1へ復帰
外側自然EOF l.2
実入力 l.0へ復帰
```

各自然 EOF で `\everyeof` は一度ずつ実行された。

## 自然 EOF、`\everyeof`、`\endinput`

`\everyeof` は**自然 EOF の先を読もうとした時だけ**実行する。

1. source の `every_eof_seen` を先に立てる。
2. その時点の現在の `\everyeof` token list を、既存 token source として積む。
3. token list が空でも seen を戻さない。
4. token list 終了後に同じ source へ戻る。
5. 二度目の EOF で source を閉じ、必要なら runaway 検査へ進む。

`\everyeof` は source 生成時に snapshot しない。EOFへ達した時点の値を読む。一方、後述する
`\tracingscantokens` は生成開始時の状態を source に保存する。

`\endinput` は現在行を最後の行にする強制終了であり、自然 EOF の先を読む操作ではない。
黒箱では実 file、疑似 file のどちらでも `\everyeof` は実行されなかった。PraTeXの
実file経路は自然EOFと`force_eof`を分離済みである。疑似sourceも同じ終了理由を持たせる。

一行の実 file が自然 EOF へ達した黒箱では、`\everyeof` 内の `\inputlineno` は2だった。
実file経路は自然EOFの読取試行で次の論理行番号へ進める。疑似fileにも同じ規則を置く。

疑似 source から、第一行に `\endinput` を含む実 file を `\input` した黒箱は次の順だった。

```text
疑似 source l.1
実 file l.1を実行して強制終了（everyeofなし）
疑似 source l.1へ復帰
疑似 source自然EOF l.2（everyeof一回）
実入力へ復帰
```

## `\tracingscantokens`

正の `\tracingscantokens` は疑似 source を開く時に `( `、閉じる時に `)` を出す。
判定は呼出開始時の snapshot である。

- 正で開始し、疑似 source 内で0にしても閉じ括弧を出す。
- 0で開始し、疑似 source 内で正にしても括弧を出さない。
- `\everyeof` の token を読み終えた後に閉じ括弧を出す。
- EOF runaway の黒箱では `)` を閉じてから runaway を報告した。
- engine shutdown でも `trace_opened` の source だけを一度閉じる。

現在値を close 時にもう一度調べず、`trace_opened: bool` を source が所有する。これにより
入れ子でも括弧を正しく対応させる。

## error context と行番号

疑似 source 内の通常エラーは、最初に疑似 file の `l.N` と現在行を示し、その後に実 file
または terminal が見つかるまで traceback を続ける。既存 `show_context` の
「`SourceType::File` または `TerminalBase` で止める」という考え方に、`PseudoFile` を停止条件として
加えてはいけない。

現行 `show_context` は表示する全 `TextSource` に単一の `self.line_number` を使う。疑似 source
の下にある実 file まで表示すると、外側実 file に疑似行番号を付けてしまう。
`line_number_stack.iter().rev()` を file-like source の出現順に対応させる。

- 最初に現れる file-like source: 現在の `line_number`
- それより外側の file-like source: `line_number_stack` の新しい側から順に対応
- token source、stream、terminal insert: 行番号 stack を消費しない
- terminal base: `<*>` を表示し、file-like 行番号を表示しない

疑似 source 内から実 file を読んでいる最中のエラーは、その実 file が最初の実 source なので、
通常の file context で停止する。実 file が閉じて疑似 source に戻った後のエラーだけが、疑似行と
外側実 source の両方を表示する。

## 資源上限と OOM

実装は safe Rust だけで行う。OS file、temporary file、`unsafe`、raw pointer は不要である。

疑似 source の追加メモリは次の二つである。

- serialized byte buffer
- logical line end offset table

生成は token 数と出力 byte 数に対して O(n)、各行を `LineLexer` の所有 `Vec<u8>` へ渡す
コピーも全体で O(n) とする。`Vec<Vec<u8>>`、先頭からの `drain`、行ごとの全残量コピーは避ける。

`Vec::push` が process OOM になるまで任せない。最終的なrun-local `InputLimits` に少なくとも
次を置く。

```text
max_scantokens_bytes_per_source
max_scantokens_lines_per_source
max_virtual_input_bytes_live
```

builder は `checked_add` と `try_reserve` を使う。`Printer` trait は `Result` を返さないため、
最初の容量超過または reserve 失敗を sticky error として保存し、それ以降の書込みを止める。
`token_show` から戻った一箇所で `overflow("scantokens buffer size", limit, ...)` へ移す。
source を積む時に live budget を加算し、EOF、強制終了、shutdown の一箇所で減算する。
既存 `INPUT_STACK_SIZE` は入れ子の source 数の上限として引き続き効く。

任意の固定値を意味論 commit に黙って入れない。既定値は他の PraTeX 資源上限と同じ場所で
明示し、CLI、daemon、将来のincremental実行でもrunごとに固定する。元の general text を
収める `Vec<Token>` は既存 scanner の資源であるため、この budget が process 全体の完全な
メモリ上限ではないことも記録する。

2026-08-23の最初のproduction sliceは、専用moduleに明示した16 MiB/source、100万行/source、
64 MiB/liveの固定上限を使う。`checked_add`、`try_reserve`、終了時charge回収は接続済みだが、
共通run-local `InputLimits`とCLI/daemonからの設定は未実装である。固定値を互換仕様とはせず、
構成可能にする時も一runの途中で値を変えない。

## performance 境界

標準入力の token hot pathには次を増やさない。

- callback
- hash lookup
- lock / atomic
- 仮想 trait call
- token ごとの `SourceType` 再判定

`PseudoFile` は既存 `TextSource` の一種とし、追加分岐は行終了時だけに置く。serialization は
`\scantokens` が明示的に呼ばれた時だけ行い、一度印字して一度読む。行終端表は offset の密な
配列にし、次行の探索を先頭から繰り返さない。

## fmt への影響

fmt に必要なのは command の意味だけである。

- `ExpandableCommand::ScanTokens`
- primitive名 `scantokens` の登録
- command の表示
- `format/dump_command.rs` の文字列 variant `ScanTokens`
- expansion dispatch

疑似 source の buffer、行位置、`every_eof_seen`、`trace_opened`、resource charge は run-local
であり、fmtへ保存しない。`\everyeof` と `\tracingscantokens` の値・level・fmt表現は既にある。

既存 fmt の command 表現は文字列 variant なので、新 variant の追加で旧 variant の番号は
ずれない。ただし古い fmt 自身には `\scantokens` の primitive meaning が含まれないため、
LaPraTeX format は新 engine で再生成する。

## 必須回帰試験

試験名は日本語にし、少なくとも次を固定する。

1. `scantokensは現在のcatcodeで再字句化する`
2. `scantokensは引数を展開せず疑似入力を先に読む`
3. `scantokensは二重上付き記法を再解釈する`
4. `newlinecharは生成時に固定しendlinecharは各行で読む`
5. `範囲外のnewlinecharはraw改行文字を行境界にしない`
6. `範囲外のendlinecharは行末tokenを足さない`
7. `raw十とraw十三を論理改行から区別する`
8. `空入力と空白だけの入力を区別する`
9. `連続改行と末尾改行は余分な空行を作らない`
10. `疑似行末の空白だけを除く`
11. `everyeofは自然EOFで一度だけrunawayより前に入る`
12. `endinputではeveryeofを入れない`
13. `edef内のscantokensは外側のdef_refを失わない`
14. `入れ子の疑似入力は行番号とeveryeofを独立させる`
15. `tracingscantokensは開始時の判定で括弧を閉じる`
16. `疑似入力から実fileを読み同じ疑似行へ戻る`
17. `疑似行の診断は外側実fileの正しい行まで続く`
18. `和文とUnicode文字を読取時の分類表で再分類する`
19. `scantokensの意味はfmtを往復する`
20. `疑似入力のbyte行数live上限を越えるとTeX overflowになる`

process-level試験のログは79桁で折り返されるため、既存 `join_log` で連結して比較する。
raw byte、空行、line iterator は unit test、EOF、context、fmt、入れ子は process-level test に
分ける。

## 実装履歴と残件

安全にreviewできる単位を次の三つに分けた。commit文は作業内容でなく、意味を分ける理由を書く。

1. **完了 (`1d54445`)** `自然EOFだけをフックにする：endinputと行番号を実入力で分離する`

   実 file の `force_eof` と自然 EOF を分け、自然 EOF 行番号、外側 source の context 行番号を
   先に直す。疑似 source が誤った既存意味を共有するのを防ぐ。

2. **完了 (`d90e98f`)** `KOMAが動的分類で再走査するため疑似入力を型付きで積む`

   typed buffer、`PseudoFile`、primitive、nested scan、`\everyeof`、trace、fmt、focused test を
   一緒に入れる。名前だけ存在する半実装の primitive を commit 間に残さない。

3. **進行中** `疑似入力の互換境界を推測に戻さない：観測と資源上限を記録する`

   この設計、clean-room観測、機能一覧、資源上限、既知の全体token-memory境界を更新する。

focused test、plain欧文DVI回帰、全releaseは通過済みである。TRIPはこのcode checkpoint後には
未再実施であり、次の統合gateでnormalized差分を再確認する。`\scantokens` はTeX82にないため、
TRIPの意味差分を増やしてはならない。

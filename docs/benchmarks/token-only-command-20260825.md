# 未展開tokenのcommand非所有化

更新: 2026-08-25

## 結論

macro引数などを未展開のまま読む`Scanner::get_token`は、従来`get_next`が返した`Command`を
直ちに捨てていた。制御綴がmacroなら、このためだけに`Rc<Macro>`のclone/dropが一組発生する。
token専用経路は入力終了、割込み、alignment、outer command回復を従来と同じ順で処理しつつ、
eqtbのcommandを借用して必要な判定だけを行い、所有する`Command`を作らない。

最終形は`InputStack::get_next`も両callerへ強制inlineする。単にtoken専用callerを増やした第二案では、
それまで通常`Scanner::get_next`へ埋め込まれていたinput source dispatchが独立symbolへ戻り、299頁を
2.1%遅くしたためである。最終形はそのcodegen退行を除き、局所と文書の両方で採用条件を満たした。

| fixture | baseline wall中央値 | candidate wall中央値 | paired wall幾何平均比 | task-clock比 | instructions比 | candidate短縮 |
|---|---:|---:|---:|---:|---:|---:|
| 未展開CS micro | 0.249982 s | 0.203967 s | **0.808751** | **0.804746** | **0.875575** | wall/task/instructions 31/31組 |
| 299頁`lipsum` | 1.605009 s | 1.553521 s | **0.963979** | **0.963964** | **0.968622** | wall 15/20、task 15/20、instructions 20/20 |

299頁のcycles比は0.965295、cache-miss比は1.014340だった。cache missは20組中12組だけ短く、
改善とは扱わない。wall約3.6%をupLaTeX比へそのまま
外挿せず、同じengine内の局所勝利と299頁非回帰として採用する。baseline/candidateの299頁DVIとauxは
それぞれbyte一致した。

raw counterは[`token-only-command-20260825.tsv`](token-only-command-20260825.tsv)、build、依存、
入力、fmt、出力、環境は
[`token-only-command-20260825-provenance.tsv`](token-only-command-20260825-provenance.tsv)に固定した。

## 棄却した二段階

最初の`52e13a0`はraw input結果を一つの抽象へまとめ、通常command取得とtoken専用取得で共有した。
microでもinstructions比1.145289、wall比1.033915となり、通常経路を太らせたので棄却した。

`a8678ac`は通常`get_next`を変更前の形へ戻し、token専用loopだけを追加した。microはwall比0.939975へ
改善したが、299頁はwall比1.021160、instructions比1.044585だった。profileではbaselineで
`Scanner::get_next`へ埋め込まれていた`InputStack::get_next`がcandidateでは8.34%の独立symbolになっていた。
`974996e`でこの局所dispatchをalways-inlineに戻し、監査追補`8e0e543`で不正tokenと回復境界を
通常取得へ揃えると、299頁instructions比は0.968622になった。
棄却二案の全counterは
[`token-only-command-rejected-20260825.tsv`](token-only-command-rejected-20260825.tsv)に残した。

## 意味境界

- `DontExpand`は一般alignment/outer検査より前に制御綴tokenとして返す。
- 通常tokenは`alignment_delta`を一度だけ加え、Tab/Span/CarRetがalignmentを閉じる時だけ既存の
  V-template挿入へ渡す。
- scannerが通常状態でない時のouter macroと`EndTemplate`は、所有cloneなしで借用中に判定し、
  既存の回復と空白token返却を保つ。
- line/file/token-list終了、疑似stream終端、字句error、割込みの順序は通常`get_next`と同じである。
- token source内の`Token::Null`とlexer専用Latin-UCS分類は、通常`get_next`と同じ内部不変条件として
  拒否する。
- 通常の展開済みcommand取得は変更前の実装を保ち、旧macro定義を展開中に保持するための
  `Rc<Macro>` cloneは消していない。

process testは未展開引数中のmacro制御綴が実行されず、`\noexpand`を含む`\edef`が従来意味を
保つことを固定する。追加unitはNull/不正Latin-UCS拒否、`DontExpand`早期return、四種のalignment
終端、outer macro回復を固定した。最終focusedはunit 11件とprocess 1件が成功した。

## 全体検証

`cargo test --release --locked --no-fail-fast`は**941 passed、0 failed、11 ignored**で、plain DVI
byte回帰も成功した。公式TRIPはStage 1 / Stage 2 / 固定comment runがすべてexit 0、
`tripos.tex`、PLtoTF→TFtoPL、固定comment DVIが公式fileへbyte一致し、`8terminal.tex`は0 byteだった。
固定comment DVIのSHA-256は公式と同じ
`09802695e330d34acec9192c15debe2de65e34fcbd3f947db9c8924240b1fe0a`である。

同じ最終binaryをwarm-up 3回後15組で三engine比較した結果、中央値はPraTeX 1.607243秒、
upLaTeX 0.998914秒、LuaLaTeX 5.724616秒だった。PraTeX/upLaTeXのpaired wall比は中央値
1.615624、幾何平均1.590993で1.3未満には未達だが、全18 runのPraTeX/upLaTeX DVIは
SHA-256 `196f46c6ea737d524992e1c93db40d1c10fb59884e412a83a6bf594e76e75ebd`へbyte一致した。
rawは[`lipsum-300page-20260825.tsv`](lipsum-300page-20260825.tsv)に固定する。

## 再開時のprofile

最終binaryの299頁を1回だけ採った1,709-sample profileでは、exclusive上位は
`Scanner::get_token` 9.00%、通常`Scanner::get_next` 8.37%、Kpathsea DB構築7.54%、
`macro_expand` 6.35%、fmt文字列行走査3.23%、allocator 3.19%、fmt CRC32 2.98%、
hyphen trie runtime検証2.46%だった。これは候補選定用で採否counterではない。性能調整は利用者判断で
いったん停止する。再開する場合はtoken専用source dispatch、macro parameter reader、通常command clone、
起動時Kpathsea/typed fmtを別々の小A/Bにし、一つの巨大変更へまとめない。

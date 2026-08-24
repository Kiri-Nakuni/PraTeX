# e-TeX 移植記録

## クリーンルームの境界

e-TeX の原実装は参照せず、公開マニュアルとブラックボックス試験から書き直す。

現在の一次資料:

- *The ε-TeX manual*: <https://texdoc.org/serve/e-TeX/0>
- 2026-08-21 閲覧

LaTeX / expl3 は互換性を測る試験入力としてのみ使い、実装の資料にはしない。

## `\showtokens`

公開manual §3.3・3.12・5.1の`<general text>`契約に従い、左braceを探す入口では通常どおりmacroを展開し、
brace内のbalanced textは展開せずtoken列として吸収する。従って`\showtokens{\value}`は
`\value`を表示し、`\showtokens\expandafter{\value}`だけが入口の`\expandafter`で値を先に置く。
外側braceは表示しない。

走査はmain-controlから既存`scan_toks(current_cs, false)`へ入り、表示は既存`token_show`へ渡す。
これにより制御語後の区切り空白、`\escapechar`、折返し、catcode 6のparameter tokenを`##`とする
規則、PraTeXのwide/CJK tokenを別実装にしない。`\let` aliasでは実行時の制御綴をwarning indexへ
渡し、primitive identityは`ShowCommand`のfmt表現で往復する。horizontal modeでは既存`Show(_)`
境界と同じくJFM pair continuityを切り、診断命令自身はnodeを作らない。

公式TeX Live 2026のe-TeXとe-upTeXへ自作最小入力を与え、未展開本文、入口展開、入れ子brace、
parameter token、和文token、和文JFM pairの分断をblack-boxで照合した。原実装sourceと上流testは
参照していない。実装側は[専用process試験](../tests/etex_showtokens.rs)と
[日本語spacing finalizer試験](../tests/japanese_spacing_finalizer.rs)で固定する。

照合資材はCTAN tlnetの2026-08-24時点の配布物である。

- `pdftex.windows` revision 78097（874,164 bytes、archive SHA-256
  `6794c3c173d1c3e9add63ed3d631b07312c208ed7d60dbed7764f588ce09ee6e`）。`etex.exe`は
  pdfTeX 3.141592653-2.6-1.40.29で、SHA-256は
  `4b582d0be712b74ae5090aba2d7338f185082f6446cbee7b26115e8ab6e21184`。
- `uptex.windows` revision 78020（1,431,444 bytes、archive SHA-256
  `c878983da002f32a24a507680ccf00261a3761089ed324892668ded589bf9c0d`）。`euptex.exe`は
  e-upTeX 3.141592653-p4.1.2-u2.02-251130-2.6で、SHA-256は
  `9f35e1fbb5b3a4b71bd1fed7c634b8876bcd965eaa0865c6b6424eb99a301c2e`。
- 取得元は`https://mirrors.ctan.org/systems/texlive/tlnet/archive/<package>.tar.xz`。

公式binaryではbatch/nonstopの101回も通常error上限へ数えず最後まで到達する一方、show診断を
process historyへ残してexit 1になる。PraTeXは全ての非fatal診断後の通常終了statusを現在0へ固定
しており、これは`\showtokens`固有ではない既存CLI差分である。本sliceではerror countとinteractionを
既存show shellに合わせ、終了statusの全体修正を混ぜない。

## `\pagediscards`と`\splitdiscards`

公開manual §3.11と§5.2に従い、正の`\savingvdiscards`でpage builderまたは公開`\vsplit`が
先頭から捨てるglue・kern・penaltyだけを、二つのrun-local special listへ順序どおり保存する。
`\pagediscards`と`\splitdiscards`は`\unvbox`と同様にnodeの所有権を現在のvertical listへ移し、
同じ保存listから二度は読めない。page listはoutput routine終端、split listはvoidまたは
不適合boxを含む各`\vsplit`開始時にも空へ戻す。内部の挿入分割は公開`\vsplit`ではないため
split listを更新しない。special listはrun-localでありfmtへ保存せず、primitive identityだけを保存する。

自作最小入力を公式TeX Live 2026 pdfTeX 3.141592653-2.6-1.40.29へ与えたblack-boxでは、
split側がbreak penalty `-10000`に続いて2 pt glue、3 pt kern、penalty 123を返し、page側も
2 pt glue、3 pt kern、penalty 123を同順で返した。どちらも回収boxの高さは5 ptで、output
routine終端後の`\pagediscards`にはそれらが残らず、空box終端のpenalty 10000だけを観測した。
PraTeXの[process試験](../tests/etex_vdiscards.rs)で同じnode順、寸法、消去時点に加え、
非正値、次の`\vsplit`、fmt往復とrun-local空状態を固定する。原実装sourceと上流testは参照していない。

照合資材は2026-08-24取得のCTAN tlnet `pdftex.windows` revision 78097である。archiveは
`pdftex.windows.tar.xz`（874,164 bytes、SHA-256
`6794c3c173d1c3e9add63ed3d631b07312c208ed7d60dbed7764f588ce09ee6e`）、取得元は
`https://mirrors.ctan.org/systems/texlive/tlnet/archive/pdftex.windows.tar.xz`。
`pdftex.exe`のSHA-256は
`4b582d0be712b74ae5090aba2d7338f185082f6446cbee7b26115e8ab6e21184`である。

## `\everyeof`

公開マニュアル §3.7 の契約:

- `\everyeof={<token list>}` は token-list parameter である。
- 実ファイルまたは `\scantokens` の疑似ファイルでEOFの先を読もうとしたときに入る。
- runaway検査とファイルを閉じる処理より前に読む。

rtex の各ファイル入力源に「すでに挿入したか」を持たせ、EOFで一度だけ既存の
token-list input source を積む。読み終えた後に元のファイル入力へ戻り、二度目のEOFで
通常どおり閉じる。この順なら既存のrunaway検査は `\everyeof` の後になる。

子ファイルを読む統合試験で、本文の後、閉じ括弧がログへ出る前に一度だけ実行される
ことを固定した。実fileと`\scantokens`疑似fileでは自然EOFと`\endinput`によるforce EOFを
分離し、後者では`\everyeof`を挿入しない。自然EOFの読取試行ではsource-local行番号を次の
論理行へ進める。疑似fileはtyped bufferから通常の行字句器へ逐次入り、入れ子・fmtも試験済み。
過去のLaTeX停止点やrelease件数は現在地の判定に使わず、
[e-TeX/TeX--XeT監査](etex-texxet-status.md)を優先する。

## `\readline`

公開マニュアル §3.7 の契約:

- `\readline<number> to <control sequence>` は `\read` と同じ入力源から次の一行を読む。
- 現在の category code は使わない。
- 文字コード32だけを category 10、それ以外（行末文字を含む）を category 12 にする。
- 読んだ token 列を、引数なし macro の置換本文として定義する。

rtex は既存の `\read` と stream・端末・EOF・`\endlinechar` の規則を共有し、字句器へ
通さずに文字 token を直接作る。これにより、入力中のバックスラッシュ、波括弧、`#`、
`%`、連続空白をそのまま保持する。

## `\interactionmode`

公開マニュアル §3.6 の契約どおり、現在の対話モードを内部整数0〜3として読み、同じ
範囲の代入で `batch`、`nonstop`、`scroll`、`errorstop` を切り替える。範囲外は
2-bit number ではないため誤りを報せ、既存の制限値走査と同様に0へ直す。

# e-TeX 移植記録

## クリーンルームの境界

e-TeX の原実装は参照せず、公開マニュアルとブラックボックス試験から書き直す。

現在の一次資料:

- *The ε-TeX manual*: <https://texdoc.org/serve/e-TeX/0>
- 2026-08-21 閲覧

LaTeX / expl3 は互換性を測る試験入力としてのみ使い、実装の資料にはしない。

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

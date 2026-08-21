# pdfTeX 互換層と直接出力

## クリーンルームの境界

pdfTeX の原実装は参照せず、公開マニュアルに書かれた命令の契約と、入力に対する
ブラックボックス試験から書き直す。実装を足すコミットには、根拠にした節と観測を
残す。

現在の一次資料は *The pdfTeX user manual*:

- <https://tug.ctan.org/systems/doc/pdftex/manual/pdftex-a.pdf>
- 2026-08-21 閲覧

LaTeX / expl3 は実装の資料ではなく、互換性を測る試験入力としてだけ使う。CTAN の
TDS archive を一時領域へ展開し、配布物そのものはこのリポジトリへ入れない。

## 二つの範囲を分ける

1. **互換プリミティブ**：文字列、ファイル照会、MD5 など。DVI出力のままでも使える。
2. **PDF直接出力**：object、stream、xref、フォント埋め込み、ページ出力。

いま入っているのは前者だけである。直接出力は、既存のDVI出力を回帰試験で固定して
から backend の境界を抜き出す。

## `\pdfstrcmp`

公式マニュアル §4.15.5 の契約:

- 展開可能命令である。
- 二つの general text を比較する。
- 等しければ `0`、第一引数が前なら `-1`、それ以外は `1` を返す。

rtex では既存の general text 走査を二度使い、展開後のバイト列を辞書順で比較する。
現行CTANの `expl3-code.tex` はこの命令を文字列比較の土台にしている。

## `\pdfshellescape`

公式マニュアル §4.24.2 の契約:

- 読み取り専用の内部整数である。
- 無制限のshell escapeなら `1`、許可表に限るrestricted modeなら `2`、それ以外は `0`。
- `\the`、`\number`、整数代入や `\ifnum` の数値走査に使えるが、命令自身は展開命令ではない。

2026-08-22に公式TeX Live 2026のpdfTeX 1.40.29、e-pTeX p4.1.2-u2.02、e-upTeX
p4.1.2-u2.02を黒箱実行した。三エンジンとも綴りは `\pdfshellescape` だけで、
`-no-shell-escape` / restricted / unrestricted に対して `0` / `2` / `1` を返した。
`\meaning` は `\pdfshellescape`、直接の `\edef` では命令を残し、`\the` を挟むと数へ
展開され、`\advance` では書き込みを拒んだ。XeTeXの `\shellescape` は別の互換面なので
aliasにしない。

rtexはshell実行機能をまだ持たないため、安全側の固定値 `0` だけを返す。プロセス起動や
環境照会は行わない。将来CLIとkpathsea相当の設定を導入するときに、状態の決定箇所を
この内部整数へ接続する。

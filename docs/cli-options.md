# PraTeX CLI 対応表

状態: 2026-08-24 の実装。Web2C 全体との互換を表明するものではない。

一次資料は TUG の [Web2C 2026 manual](https://www.tug.org/texinfohtml/web2c.html) の
「Option conventions」「Common options」「tex invocation」「Determining the memory dump to
use」である。PraTeX 固有 option は同じ parser を通すが、Web2C option とは分けて記す。

## 実装済み

| option | 受理する綴り | PraTeX での意味 |
|---|---|---|
| help | `-help`, `--help` | engine、時計、fmt、入力を開かず、実装済み option だけを stdout へ表示して status 0 で終了する。 |
| version | `-version`, `--version` | release gate 未達を反映する現在の PraTeX banner を stdout へ表示して status 0 で終了する。 |
| interaction | `-interaction=MODE`, `-interaction MODE`（二重 dash も可） | `batchmode=0`, `nonstopmode=1`, `scrollmode=2`, `errorstopmode=3`。fmt から復元した値より後で run-scoped override を一度だけ適用する。4値以外は起動前 error にする。 |
| format | `-fmt=NAME`, `-fmt NAME`（二重 dash も可） | `NAME.fmt` を現在directory、次に `TeXformats/` から直接探す。公開順位どおり、最初の非option引数が `&OTHER` なら `OTHER` が優先する。 |
| initial engine | `-ini`, `--ini` | fmt を読まず primitive 初期状態を作り、`\patterns` と `\dump` を initial run だけで許す。`-ini -fmt=NAME` では初期状態が優先する。 |
| halt | `-halt-on-error`, `--halt-on-error` | 最初の回復可能 TeX error の文脈とhelpを transcript へ残し、その場で status 1 にする。後続tokenは実行しない。 |
| job name | `-jobname=STRING`, `-jobname STRING`（二重 dash も可） | 入力名からの導出前にrun-scopedな `\jobname` を固定し、log、DVI/PDF、fmtへ同じbasenameを使う。空値も受理して`.log`等を作り、既存のdotを拡張子と見なさず`.dvi`等を追記する。path separatorはOSのpathとして働き、親directoryは事前に存在する必要がある。 |
| output comment | `-output-comment=STRING`, `-output-comment STRING`（二重 dash も可） | DVI preambleのcommentを既定の実時刻文字列から指定byte列へ置換する。空値と255 byteまでを受理し、256 byte以上はDVIを開く前にerrorにする。直接PDFではWeb2C互換にoptionを受理するが、PDF metadataへ転用せず無視する。 |
| shell禁止 | `-no-shell-escape`, `--no-shell-escape` | shell実行を持たない現在のrun policy `Disabled`を明示する。`\pdfshellescape`は0を返す。正方向とrestricted modeは受理しない。 |
| mktex禁止 | `-no-mktex=tex|tfm`, `-no-mktex tex|tfm`（二重 dash も可） | PraTeXが`mktextex`/`mktextfm`生成経路を持たない現在の`Disabled`状態をfile typeごとに明示する。`pk`など他typeと正方向`-mktex`は受理しない。 |
| option終端 | `--` | 以後のOS文字列を option と解釈せず、そのまま TeX の最初の入力行へ渡す。dashで始まる入力名にはこの境界が必要である。 |

値を取る option は `=` と空白分離の両方を受理する。実装済み option について一重 dash と
二重 dash は同じ意味だが、現在は省略形を受理しない。同じ option を複数回指定した場合、値を
持つものは後の指定が勝つ。

## PraTeX 固有 option

| option | 状態 |
|---|---|
| `-output-format=dvi|pdf` | 実装済み。空白分離と二重 dash も受理する。 |
| `--pdf-font-map=PATH` | PDF時だけ実装済み。PATHはOS文字列のまま保持する。 |
| `--pdf-japanese-cid-profile=PATH` | PDF時だけ実装済み。既定`upjisr-h`の内蔵profileを上書きする。 |
| `--quiet` | 実装済み。自動進捗だけを端末から隠し、文書の `\message` / `\write16` と transcript は残す。 |

## 意図的に拒否するもの

`--` より前の未知のdash始まりは TeX 入力へ黙って流さず、起動前に status 1 と option名を
返す。したがって、次の Web2C option は未実装のまま「受理だけ」されることはない。

| 未実装領域 | 例 |
|---|---|
| 出力配置・記録 | `-output-directory`, `-recorder` |
| 診断形式 | `-file-line-error`, `-no-file-line-error` |
| fmt 自動選択 | `-parse-first-line`, `-no-parse-first-line`, `-progname` |
| kpathsea設定 | `-kpathsea-debug`, `-cnf-line` |
| 入力変換 | `-8bit`, `-translate-file` |
| shell実行の正方向 | `-shell-escape`, `-shell-restricted` |
| mktex生成の正方向・他type | `-mktex=tex`, `-mktex=tfm`, `-no-mktex=pk` |

PraTeX は別engineへ偽装せず、未実装 option に成功を返さない。

## 現在の差異と既知制限

- 配布時に固定された既定fmtがまだないため、fmt selectorも `-ini` もない起動は、既存の
  source実行互換を保って自動的にinitial engineへ入る。Web2Cの通常の `tex` がprogram名から
  既定fmtを選ぶ挙動は、native kpathsea resolverのprogram-name境界と一緒に今後実装する。
- Web2Cでは `-fmt=NAME` がprogram nameにも影響する。PraTeXは現時点でfmtファイル選択だけに
  使用し、kpathsea探索用program nameは変更しない。特に `-ini -fmt=NAME` はinitial engineを
  選ぶが、探索pathのprogram名切替はまだ行わない。
- fmt読込み自体はnative kpathsea resolverへまだ接続されておらず、現在directoryと
  `TeXformats/`の直接探索だけである。TeX Live treeのfmt探索対応を意味しない。
- `%&fmt` のmain input先頭行解析は `-parse-first-line` とともに未実装である。command lineの
  先頭 `&fmt` とは別機能なので混同しない。
- `-jobname`内のseparatorを含む親directoryをPraTeXは作らない。openに失敗した場合は従来の
  TeX promptへ入り、`-output-directory`による一括配置もまだない。
- `-no-mktex`はPraTeX自身にtex/tfm生成helperが存在しないことの明示である。移行中の
  file resolverは探索だけを行う外部`kpsewhich`へfallbackする場合があり、通常lookupの
  子process 0という最終設計が完成したことを意味しない。

## 公式binaryで固定した境界

公開manualに加え、2026-08-24に公式CTAN配布のTeX Live revision 78097 Windows
`pdftex.exe`（pdfTeX 3.141592653-2.6-1.40.29 / kpathsea 6.4.2）へ自作最小入力を与えた。
`-jobname`は空値、slash/backslash、dotを文字どおり`\jobname`へ返し、log、DVI/PDF、fmtには
拡張子を置換せず追記した。`-output-comment`は空値と255 byteをDVIへそのまま置き、PDF modeでは
指定文字列を出力へ入れなかった。probeは上流source/testを参照せず独立に作成した。

取得物は`https://mirrors.ctan.org/systems/texlive/tlnet/archive/pdftex.windows.tar.xz`
（874,164 bytes、SHA-256
`6794c3c173d1c3e9add63ed3d631b07312c208ed7d60dbed7764f588ce09ee6e`）。
`pdftex.exe`のSHA-256は
`4b582d0be712b74ae5090aba2d7338f185082f6446cbee7b26115e8ab6e21184`である。

## 回帰試験

```powershell
cargo test --locked --lib run_options::tests
cargo test --locked --test cli_options
```

process試験は help/version の早期終了、未知optionと `--`、4 interaction値、`-ini`でのfmt生成、
`-fmt`読込、`&fmt`の優先、fmt読込後のinteraction override、halt時の後続token不実行に加え、
空/path/dotを含むjob nameの`\jobname`・log・DVI/PDF・fmt一貫性、DVI commentの指定/空値/255 byte境界、
PDFでの非転用、shell/mktex無効指定と`\pdfshellescape=0`を確認する。

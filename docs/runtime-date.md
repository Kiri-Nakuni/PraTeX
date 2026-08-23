# PraTeXのrun時刻

更新: 2026-08-23

## 一つのrun、一つの時計

PraTeXはcommand lineを解釈した直後、fmtを読む前に時計を一度だけ取得する。その
`RunDateTime`から次を初期化し、run途中でOS時計や環境変数を読み直さない。

- `\year`、`\month`、`\day`、`\time`
- transcript先頭の日付と分単位の時刻
- expandableな`\pdfcreationdate`の秒・UTC offsetを含むPDF日時文字列

TeXの四つの整数parameterは従来どおり代入可能である。DVI preamble commentとfmtの
識別日は、それぞれ最初のshipout時・dump時のparameter値を読む既存契約を保つ。一方、
`\pdfcreationdate`は実行開始時点を表すため、parameterへの後続代入では変わらない。
LaTeXの既定`\date{\today}`は、文書runで初期化された同じparameterを展開する。

## 再現可能な固定経路

`SOURCE_DATE_EPOCH`があれば、整数のUnix秒として厳密に読み、仕様どおりUTCで暦へ直す。
したがってPDF日時のoffsetは`+00'00'`となる。不正値・範囲外の値を現在時刻へfallback
させず、入力を開く前に起動エラーにする。

PraTeXではこの一変数を指定した時点でTeXの時刻parameterも固定する。pdfTeXが
`SOURCE_DATE_EPOCH`に加えて`FORCE_SOURCE_DATE=1`を要求する構成とは異なるが、PraTeXでは
四つのconsumerを別々のclock policyに分けないための意図した差である。

`SOURCE_DATE_EPOCH`の定義は「1970-01-01 00:00:00 UTCからの、閏秒を除いた整数秒」である。

- 仕様: <https://reproducible-builds.org/specs/source-date-epoch/>
- pdfTeX manual: <https://tug.ctan.org/systems/doc/pdftex/manual/pdftex-a.pdf>

## local timezoneとtarget境界

Windows/Unixの通常runでは、`chrono::Local::now()`を一回呼び、そのinstantに対するDST込みの
fixed UTC offsetをsnapshotへ保存する。WindowsはOS Time Zone API、Unixは`TZ`または
system timezone dataを利用する。PraTeX自身のWASM targetなど、それ以外のtargetでは
local clockをUTCへ黙って読み替えない。host clock ABIができるまでは、固定した
`SOURCE_DATE_EPOCH`がなければtyped起動エラーにする。

PDF日時はoffsetを分までしか表せないため、秒単位の歴史的offsetを与える環境は明示エラーと
する。またPDF日時の年は4桁なので、固定epochが0--9999年を外れる場合も明示エラーとする。

## dependency監査

`chrono`はUTC変換に必要なcoreを全targetへ、`clock` featureだけを
`cfg(any(unix, windows))`へ接続した。default featureは切り、PraTeX側から
`wasm-bindgen`を有効にしない。

| crate | lock | license | このtargetでの用途 |
|---|---:|---|---|
| `chrono` | 0.4.45 / SHA-256 `1aa79e62e7697b8e29b513a68abacf485adcd1fe8284a4316c5ae868e6633327` | MIT OR Apache-2.0 | 暦変換、Windows/Unix local clock |
| `num-traits` | 0.2.19 / SHA-256 `071dfc062690e90b734c0b2273ce72ad0ffa95f0c74596bc250dcfd960262841` | MIT OR Apache-2.0 | `chrono`の数値trait |
| `autocfg` | 1.5.1 / SHA-256 `f2032f911046de80f0a198e0901378627c33f59ea0ac00e363d481118bd70a53` | Apache-2.0 OR MIT | `num-traits` build dependency |
| `windows-link` | 0.2.1 / SHA-256 `f0805222e57f7521d6a62e36fa9163bc891acd422f971defe97d64e70d0a4fe5` | MIT OR Apache-2.0 | Windows API link |

Windowsのactive dependency treeはこの4 crateだけである。WASM/WASI側は`chrono`、
`num-traits`、`autocfg`だけで、`clock` featureは入らない。Unixでは`chrono`が
`iana-time-zone`をtimezone検出に使う。

PraTeXの`src/`には`unsafe`を追加しない。`chrono` 0.4.45自身はWindows API境界の
`src/offset/local/windows.rs`と生成bindingに監査可能な`unsafe`を持つ。このFFIは
target-specific dependencyの内部に閉じ、PraTeXの通常sourceへ移さない。

## 公式LaTeX live gate

`tools/test-prjsarticle.ps1`は既定で`SOURCE_DATE_EPOCH=1709210096`、すなわち
2024-02-29 12:34:56 UTCをformat生成と二つの文書runへ渡す。公式CTAN `latex.ltx`から作った
fmtを使い、`tests/fixtures/prjsarticle/runtime-date-maketitle.tex`が`\date`を指定せずに
`\maketitle`まで完走することを確認する。log oracleは次を同時に要求する。

- TeX parameter: `2024-2-29/754`
- `\pdfcreationdate`: `D:20240229123456+00'00'`
- LaTeX既定date: `February 29, 2024`

2026-08-23のrelease実測はexit 0で、生成したruntime-date DVIのSHA-256は
`64ad9cd7580a1b62af44204d0c760f47effb34fe68907619259bbc934b9f6f00`だった。CTAN archiveの
版・URL・hash・licenseは`tests-support/prjsarticle/assets.json`を使い、生成物はrepository外へ
置いた。

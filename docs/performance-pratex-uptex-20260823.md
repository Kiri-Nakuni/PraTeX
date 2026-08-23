# PraTeX と upTeX の plain format 性能比較（2026-08-23）

## 結論

この測定では、インストール済み `pratex` は全ての ASCII plain-TeX
fixture で TeX Live 2026 の `uptex` より短い wall time だった。wall time の
幾何平均比（PraTeX / upTeX）は **0.203** である。

ただし、これは現 checkout の性能 gate の合格証明ではない。current source tree
`42c28005a67201e2b94c6cb31f4f6e63f3d42a2f` は `vaak::embedding` API が dependency
`c230b0bdbce039b904ad00e02f34968213a57461` に存在しないため、release `pratex` を
build できなかった。測定対象は `/home/suima/.cargo/bin/pratex`（SHA-256
`bcbe0f1f9d680f958f0e51b98d09b16ff551613e9d35c157c161eb6a690655b4`）であり、source
commit は特定できていない。また、現段階で意味比較は page 数・DVI size・先頭の
DVI event 列の照合に留まり、全 event 正規化比較はまだ行っていない。

## 条件

- 日時: 2026-08-23, Asia/Tokyo
- CPU: Intel Core i7-8650U、CPU 0 に `taskset` で固定
- OS: Linux 7.0.0-29-generic x86_64
- comparator: TeX Live 2026 `uptex`（e-upTeX 3.141592653-p4.1.2-u2.02-251130-2.6）
- `perf`: 7.0.12。各 engine を先に 2 回 warm-up し、`perf stat -r 15` で測定
- event: `task-clock,cycles,instructions,context-switches,page-faults`
- format: 同じ TeX Live の `plain.tex` / `hyphen.tex` から、各 engine 自身で作った
  local format。PraTeX は `plain-build.fmt`、upTeX は `uptex-plain.fmt`。
- 入力と生成物、log は `/tmp/pratex-uptex-perf-format-PJ9nlY` のみへ置いた。

fixture は全て外部 macro package を使わない ASCII plain-TeX である。

1. `short`: 一段落、hyphenation・ligature・math・DVI shipout。
2. `expansion`: macro 展開と `\advance` を 100,000 回、最後に一頁を shipout。
3. `multipage`: 同じ狭幅段落を 160 回組み、19 頁を shipout。

## 結果

wall time は `perf stat` の 15 回平均、`±` は同出力の相対標準偏差である。

| fixture | pages | PraTeX wall | upTeX wall | 比 (P/U) | PraTeX task-clock | upTeX task-clock | P/U instructions |
|---|---:|---:|---:|---:|---:|---:|---:|
| short | 1 | 21.705 ms ±7.98% | 209.918 ms ±0.91% | **0.103** | 16.88 ms | 200.28 ms | 0.168 |
| expansion | 1 | 120.411 ms ±4.12% | 266.396 ms ±1.94% | **0.452** | 115.36 ms | 254.01 ms | 1.070 |
| multipage | 19 | 41.463 ms ±1.47% | 230.596 ms ±1.43% | **0.180** | 38.44 ms | 218.75 ms | 0.449 |
| wall 幾何平均 | — | — | — | **0.203** | — | — | — |

`page-faults` は short / expansion / multipage の順に、PraTeX が
883 / 877 / 1,243、upTeX が 8,012 / 8,011 / 8,030 だった。context switch は全標本で
0 だった。

## 出力確認と解釈

各 fixture は両 engine で終了 code 0、同じ page 数になった。DVI size は順に
PraTeX/upTeX = 716/712、392/388、68,624/68,624 bytes だった。DVI byte hash は一致しない。
主な既知差は preamble comment（engine 名・日時）と upTeX の DVI 表示方式である。
`short` の `dvitype` / `updvitype` は page 開始後の font 選択、文字、glue、座標が同じ列で
始まることを確認したが、これだけで full semantic equivalence は主張しない。

この結果は、plain ASCII に限る process 起動・format load・展開・paragraph/page build・DVI
shipout を含む warm comparison としては有用である。一方、project の hard gate
（同一 TeX tree、同等 DVI の end-to-end 1.2 倍未満）に必要な LaTeX、和文、混植、禁則、縦組、
cold run、全 DVI event 正規化比較を含まない。Vaak dependency を整合させて current source の
release binary を再生成後、この corpus を再実行し、次に日本語組版が接続された段階で必要 corpus
を増やす。

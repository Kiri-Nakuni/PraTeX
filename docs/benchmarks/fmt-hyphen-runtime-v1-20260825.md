# HyphenRuntimeV1 fmt A/B

更新: 2026-08-25

## 結論

行辞書案と異なり、型付き`HyphenRuntimeV1`は採用した。旧ASCII fmtのbuild用`PreTrie`約30.2 MB、
約600万行とbuild hashの復元をloaded runから除き、fmtを57,221,231 byteから21,532,633 byteへ縮めた。
同一candidate binary、CPU 0固定、codecだけを変えた交互A/Bの結果は次である。

| fixture | 旧ASCII平均 | runtime v1平均 | paired wall幾何平均比 | 中央値比 |
|---|---:|---:|---:|---:|
| LaTeX空実行、20組 | 535.40 ms | 253.48 ms | 0.4732 | 0.4651 |
| `lipsum` 299頁、15組 | 1,573.61 ms | 1,296.15 ms | 0.8236 | 0.8157 |

| counter平均 | 旧ASCII | runtime v1 | 変化 |
|---|---:|---:|---:|
| 空実行 instructions | 4.198 billion | 1.775 billion | -57.7% |
| 空実行 cache misses | 20.18 million | 6.89 million | -65.8% |
| 299頁 instructions | 11.429 billion | 9.021 billion | -21.1% |
| 299頁 cache misses | 25.58 million | 12.36 million | -51.7% |

299頁DVIは両codecで1,396,468 byte、SHA-256
`e3896449ad99402c21a963819d4abdfe4e6422d270980bf1b146d2b3e1291e55`へbyte一致し、auxも一致した。
空実行rawは[`fmt-hyphen-runtime-v1-noop-20260825.tsv`](fmt-hyphen-runtime-v1-noop-20260825.tsv)、
299頁rawは
[`fmt-hyphen-runtime-v1-document-20260825.tsv`](fmt-hyphen-runtime-v1-document-20260825.tsv)、
binary・fmt・fixture・順序は
[`fmt-hyphen-runtime-v1-20260825-provenance.tsv`](fmt-hyphen-runtime-v1-20260825-provenance.tsv)
に固定した。

## 読める範囲

task-clock平均から空実行を差し引くと、本文側は旧1,024.71 ms、新1,027.58 msでほぼ変わらない。
従ってこれはfmt/startupの改善であり、展開・走査・paragraph buildを速くした値ではない。

同日の公式binary診断値937 msと直接比べると、新299頁task-clock 1,275 msは概算1.36倍である。
測定rotationが異なるため正式な三engine gateではないが、1.3未満にも0.98未満にもまだ届いていない。
fmt/startupは公式側と同程度以下へ入ったので、次はloaded-bodyの約1.60倍差、特にtoken取得、整数走査、
macro reader、input frameのdata layoutを同一DVI A/Bで削る。

## 正しさ

新fmtを既定にした`cargo test --release --locked --no-fail-fast`は932 passed、0 failed、11 ignored。
旧ASCIIは`PRATEX_FMT_CODEC=legacy-text`で生成し、magicなしreaderを引き続きprocess試験した。

TRIP featureの隔離binary SHA-256は
`be5cdc9f3563410131940c3e4389bd8f09dfd43c5f065eaf84665a2ccdd93784`。binary／legacyとも
Stage 1/2 exit 0、`tripos.tex` byte一致、`8terminal.tex`空、PLtoTF→TFtoPL一致だった。
binary `trip.fmt`は511,381 byte、legacyは516,268 byte。固定commentで両方のDVIが公式2920 byteへ
byte一致し、SHA-256は既知正常値
`09802695e330d34acec9192c15debe2de65e34fcbd3f947db9c8924240b1fe0a`だった。
隔離artifactは`/tmp/pratex-trip-hyphen-runtime-v1.oiJd5iKP`に残した。

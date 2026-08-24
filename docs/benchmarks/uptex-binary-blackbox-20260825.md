# 公式upLaTeX binaryのblack-box hot path比較

更新: 2026-08-25

## 境界

公式e-upTeXの実装source、上流test、外部debug informationは参照していない。TeX Live 2026の
strip済み公式binaryへ、自作入力、`perf`、`strace`、公開DVIだけを与えた。binary内の関数名は
公開symbolではなく、`.eh_frame`の関数範囲、逆assemble、call stack、自作probeから推定した
**観測上の対応**であり、upTeX実装のcanonical名として引用しない。

測定はCPU 0へ固定して逐次実行した。競合を検出したDVI backendのwall/cycle標本は破棄した。
299頁fixtureではPraTeX/upLaTeXが1,396,464 byteの同一DVIを生成し、SHA-256は
`d4da0f725720e4c6f51a5c7fb689385a05168f20d68befa59840e2814d729c2f`だった。正式な六順序rotation
測定を置き換える値ではなく、差の場所と改善上限を決める診断値である。

raw counterは[`uptex-binary-blackbox-20260825.tsv`](uptex-binary-blackbox-20260825.tsv)、
実行file、fmt、fixture、元statのhashは
[`uptex-binary-blackbox-20260825-provenance.tsv`](uptex-binary-blackbox-20260825-provenance.tsv)
に固定した。

## 全体分解

| | upLaTeX | PraTeX | PraTeX / upLaTeX |
|---|---:|---:|---:|
| 空実行・fmt/startup | 295.45 ms | 859.62 ms | 2.91 |
| 299頁全体 | 936.48 ms | 2,209.29 ms | 2.36 |
| 299頁から空実行を引いたcounter差 | 641.03 ms | 1,349.67 ms | 2.11 |

fmt/startupだけをupLaTeX相当まで短縮しても、PraTeXは概算1,645.12 ms、upLaTeX全体の1.76倍である。
本文側だけを同等化しても概算1,500.65 ms、1.60倍である。従って0.98倍には両側の構造変更が必要で、
unsafeをfmt復元へ局所導入するだけでは届かない。

299頁から空実行を引いたcounter差では、PraTeXのinstructionsはupLaTeXの2.00倍、cache missは
3.87倍だった。全体のbranch miss率だけでなく、所有権処理とデータ配置を主因候補にする。

## fmt観測

| | on-disk | 復号後またはPraTeX表現 | `read` |
|---|---:|---:|---:|
| 公式`uplatex.fmt` | 3,641,570 byte gzip | 14,995,744 byte | 446回、3.64 MB、10.7 ms |
| PraTeX `latex.fmt` | 57,221,231 byte ASCII | 同左 | 2回、57.2 MB、32.2 ms |

公式側はfmt semantic undump本体のself cyclesが空実行のおよそ2%、gzip展開が約24%、Kpathsea DB構築が
約70%だった。PraTeX profileではhyphen trie undump、汎用Vec復元、再確保、PreTrie再検証、control
sequence復元が独立hotspotとして見える。

最初のA/Bはraw structのunchecked復元でなく、versionとsection長、要素数、checksumを持つbinary fmt、
検証済み長さからの一回確保、`chunks_exact`と`from_le_bytes`によるsafeな一段decode、decode中の検証統合
とする。破損fmt拒否、TRIP、299頁同一DVIを同時gateにする。

## 公式binaryの観測上の対応

| PraTeX側の概念 | 公式binaryで観測した役割 | 299頁profile |
|---|---|---:|
| `main_control` | 11,783 byteの大dispatch。token取得、代入、組版、shipoutを呼ぶ | inclusive 75.28%、self 2.98% |
| 展開token取得loop | `get_next`後、macroと他の展開commandを分配 | inclusive 33.23% |
| `get_next` | input state、catcode、token listを直接走査 | inclusive 24.22%、self 18.47% |
| macro call | 引数走査とreplacement list開始 | inclusive 18.63% |
| begin/end token list | input frame push/popとtoken list/free-list処理 | microで双方が支配的 |
| expandable command dispatch | jump tableで展開primitiveを分配 | inclusive 18.91%、self 1.89% |
| `scan_int` | 符号、radix、内部整数を走査 | inclusive 5.94% |
| `scan_toks` | brace/token走査、token node確保、回復 | inclusive 16.30% |
| fmt loader | magic検査とblock reader呼出し | 299頁7.05%、空実行28.86% |
| Kpathsea DB hash insert | 文字hashとcollision chain処理 | 299頁self 14.30%、空実行self 56.04% |
| DVI hlist出力 | glyph/nodeからDVI record生成 | inclusive 3.45%、self 2.76% |

公式側で独立したpost-load trie validation関数は観測できなかった。loaderへinlineされている可能性は
排除できないが、fmt loader selfが空実行の約2%なので、PraTeXの独立全trie再走査とは形が異なる。

## 自作probe

- macro展開: 約2,560万回。1.322 s。公式側のself shareは`get_next` 28.26%、macro call 16.97%、
  end-token-list 13.34%、begin-token-list 9.56%。
- 整数走査: 384万代入。1.484 s。`scan_int` self 16.56%、`get_next` 35.05%。
- token-list走査: 192万代入。1.202 s。`scan_toks` self 5.68%、旧list破棄5.89%、free-list 4.61%。
- DVI: 4000 shipout、2,304,000 glyph。両engineのDVIは2,580,120 byteでbyte一致し、SHA-256は
  `1936d6743bbc96077c422b8b4702d08f39fd61880458b00ffd1caa7f617b792a`。backendよりloop展開とtoken取得が
  支配的だった。外乱が大きいwall/cyclesは性能比に採用しない。

## safe Rustでの順序

1. version付きbinary fmtとdecode時検証統合。
2. `scan_int`の通常十進経路をinternal quantity・診断経路から分離するA/B。
3. primitive commandをCopy viewにし、macroをrun-lifetime IDで参照してhot loopの`Command` clone/dropを減らす。
4. input frameのhot fieldを小さな連続storageへ分け、診断情報をcold側へ置く。
5. Kpathsea DB初期化の遅延またはfingerprint付きpersistent native indexをA/Bする。
6. DVI batchingはbackendが利用者corpusで顕在化した後に行う。

どの局所値もend-to-end達成へ外挿しない。特にbinary fmtだけで1.3や0.98を達成したとは扱わない。

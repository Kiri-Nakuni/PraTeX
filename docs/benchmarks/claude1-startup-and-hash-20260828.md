# 起動固定費と共通経路の hash 除去（`claude1/perf-integration`）

分岐点: `codex3/roadmap-integration` の `36af0ee`
測定機: Intel Core i7-8650U、Linux、TeX Live 2026
実施日: 2026-08-28

## 測り方

この laptop は発熱で周波数が下がり、他 process の負荷も乗る。したがって
**採否の判定には `perf stat` の命令数**を使う。命令数は周波数にも他 process の
負荷にも依存せず、実測のばらつきは 0.01〜0.04% である。DVI の指紋を同時に見て、
変わったものは速くても採らない。

正式な比は `tools/bench-document-throughput-linux.sh` で取る。`tools/run-throughput.sh`
がこの枝の binary と fmt を渡し、`tools/summarize-throughput.py` が `runs.tsv` を
対 (paired) の比で集計する。対にするのは、発熱や他 process による drift が同じ
round の両 engine へほぼ等しく効き、比を取ると打ち消されるためである。

反復用には `tools/measure-instructions.py` を使う。

## 結果

299 頁 `lipsum-300page`、warm-up 3 回、標本 9 回、CPU 0 固定、六順序回転。

| | 分岐点 `36af0ee` | 本枝 |
|---|---|---|
| 命令数 | 8,739,185,718 | **7,341,737,735**（−15.99%） |
| `latex.fmt` | 21,532,699 byte / 3,949,254 行 | **18,931,934 byte / 2,513,260 行** |
| PraTeX wall 中央値 | — | 1.1786 s |
| upLaTeX wall 中央値 | — | 0.8069 s |
| **paired wall 比 中央値** | **1.615624**（記録値） | **1.4689** |
| paired wall 比 幾何平均 | 1.590993（記録値） | **1.4429** |

DVI は分岐点と byte 一致。ハーネスの PraTeX/upLaTeX DVI 意味比較も通過した。
全 release 試験は 953 passed / 0 failed / 11 ignored。

**roadmap 再開条件の 1.3 未満には未達である。**

## 起動固定費と組版の分離

同じ format で、本文が一行だけの `article` を測って差し引いた。

| | 起動固定費 | 組版（299 頁 − tiny） |
|---|---|---|
| 分岐点 PraTeX | 2,363M 命令 / 407 ms | 6,376M / 857 ms |
| 本枝 PraTeX | **1,439M 命令 / 337 ms** | **5,903M / 767 ms** |
| upLaTeX | 975M 命令 / 235 ms | 3,438M / 529 ms |
| 本枝 / upLaTeX | 1.48 / 1.43 | **1.72 / 1.45** |

起動固定費は命令数で **39.1%**、組版は **7.4%** 減った。効き方が違うので分けて見る。
残る差は、起動が 102 ms、組版が 238 ms である。**組版側の方が大きい**が、
起動側の方が原因がはっきりしている。

起動の内訳（命令数、tiny 文書）は次のとおり。

| | 割合 |
|---|---|
| `load_fmt_file`（eqtb の組み立てを含む） | 10.3% |
| **組み込み kpathsea（C）の `read_line` と `hash_insert_normalized`** | **13.1%** |
| allocator | 10.6% |
| fmt の値読み出し（`undump_signed` `undump_unsigned` `undump_byte_string`） | 10.8% |
| `Trie::language_patterns_are_valid` | 5.7% |
| `Token` と `MacroCall` の読み出し | 5.5% |

**組み込み kpathsea が起動の 13% を占める。** `bundled-kpathsea` は既定 feature で、
TeX Live の C ライブラリを実際に組み込んでいる。`KpathseaFastPath::new` は
`Kpaths::new_in_process_with_program_name` を無条件に呼ぶので、直接 path で
解決できる run でも初期化費を払う。遅延化できれば効くはずだが、file 解決の境界は
`docs/kpathsea-port-notes.md` に細かい不変条件があるので、担当者の判断に委ねる。

## 何が効いたか

| 変更 | 命令数（累計） |
|---|---|
| `panic = "abort"` と `codegen-units = 1` | −3.15% |
| lig/kern 表を hash から線形走査へ | −3.95% |
| 行と段落の節点列に容量を先に確保 | −4.53% |
| `split_adjust` の先行走査、`hlist_out` の文字経路、`round` | −4.87% |
| 制御綴の索引を SipHash から乗算・回転の hash へ | −5.43% |
| **fmt の行分割を専用実装へ** | **−10.83%** |
| **trie 検証の一時表を SipHash から外す** | **−14.22%** |
| **制御綴の名前を一行の十六進へ** | **−15.99%** |

大きかったのは後ろの三つで、いずれも fmt 読み込みである。

**行分割**: `latex.fmt` は 21.5 MB に 3,949,254 行あり、一つの値につき一行という
形である。`str::lines` は部分文字列探索の一般機構を通るため、一行が 5.4 byte しか
ないこの形では設定費が走査そのものより高くつく。改行を直接探す反復子にした。

**trie 検証**: `Trie::language_patterns_are_valid` は fmt を読むたびに言語ごとへ
trie 全体を辿り、節点番号を鍵にした一時表へ入れていた。既定の SipHash を
乗算・回転の hash に替えただけで起動が 16.1% 減った。

**名前の書き方**: 行の頻度を数えると、上位を占めるのは制御綴の名前を構成する
byte だった。`Vec<u8>` の既定の書き出しは一 byte 一行なので、expl3 の長い名前が
hash の鍵と `escaped` で二度、それぞれ一 byte 一行になっていた。一行の十六進へ
変えると fmt の行数が 36.4% 減った。

## 効かなかったもの

| 試したこと | 結果 |
|---|---|
| `get_token` で eqtb 引きを条件で守る | **+1.1% 悪化。** 判定条件（`align_state == 0`、`scanner_status != Normal`）は通常の文では成立しないので引きを省けるはずだが、分岐を足す方が高くついた。match の外へ括り出すと更に悪化した |
| `InputStack::get_next` の `inline(always)` を外す | **+5.8% 悪化。** 既存の判断が正しい |
| 整数解析を `str::parse` から十進専用へ | 差なし。範囲検査が明示されるので残した |
| fmt 読み込み前の buffer 確保 | 差なし。`read_to_end` が既に大きさを見ている |
| `post_line_break` の節点複製を除く | 見送り。分割点に来るのは糊・penalty 等の小さい節点だけで（discretionary は手前で処理済み）、複製は安価だった |

## 残っている差と、次に効くはずのもの

299 頁の命令数 profile（本枝）は次のとおり。

| 領域 | 割合 |
|---|---|
| 入力・展開（`get_token` 16.3%、`get_next` 12.1%、`macro_expand` 8.4%、他） | **43%** |
| allocator（malloc / free / realloc） | 8.8% |
| 組版（行分割、`append_node`、節点破棄） | 15% |
| fmt 読み込みの残り | 8.4% |

`macro_expand` → `scan_parameters` → `scan_a_parameter` →
`contribute_entire_group_to_current_parameter` が単独で 10.7% を占める。中括弧引数の
収集であり、費用の中身は token ごとの `get_token` である。`get_token` は
`get_next` を取り込んだ 6 KB の関数で、296 byte の stack frame を持つ。特定の
命令に偏っておらず、分岐を足す方向は上記のとおり悪化した。

**次に効くはずなのは fmt の binary 化である。** 起動の残り 1,594M 命令のうち、
値の読み出し（`undump_unsigned` 13.2%、`undump_signed` 3.5%）と `load_fmt_file`
11.0% は、一つの値につき一行というテキスト形式そのものに由来する。行の走査と
桁の走査が二度に分かれており、列挙の変種は名前の文字列比較で読んでいる。
`3db344e` が導入した section つき binary 容器はあるが、eqtb 部分は今も
その中のテキストである。

起動を upLaTeX 並みの 235 ms へ近づけられれば、299 頁の wall は
1.160 → 約 1.05 s、比は **約 1.32** になる。組版側で更に 1 割削れれば 1.3 を切る。
ただし `Dumpable` の実装は百箇所を超えるため、着手は一続きの作業として計画する必要がある。

## 未実施

- **公式 TRIP は走らせていない。** 資材は同梱されておらず、CTAN からの取得が要る。
  取得は利用者の許可を得てから行う。
- 日本語 corpus、縦組、`prjlreq` での再測定。

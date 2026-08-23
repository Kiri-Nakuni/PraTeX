# safe Rust 性能測定

## 合格条件: DVI modeでupLaTeXの1.2倍未満

PraTeXの性能合格条件は、単に「Rust実装として十分速い」ことではない。**DVI出力modeの
end-to-end実行時間を、同じ入力、同じTeX tree、同じcold/warm条件で動かすupLaTeXの
1.2倍未満に収める。** 意味が同等でないDVI、機能を省いた短いfixture、探索時間を別processへ
隠した値では合格にしない。日本語機能が増えるほど比較対象へ近づき、条件を満たす価値も難度も
上がるため、主要sliceごとに退行を止める。

合否判定にはprocess起動、native file探索、fmt復元、展開、組版、page build、DVI closeまでを
含むwall timeを使う。診断用にCPU timeと各区間も別列へ記録するが、engine部分だけを切り出して
end-to-endの失敗を打ち消さない。PraTeXだけが通常lookupごとに`kpsewhich`を子process起動したり、
upLaTeX側だけがcache済みだったりする非対称な標本は採用しない。

最低限、次を継続測定する。

- process起動、TeX Live探索、fmt復元、展開、段落整形、JFM class対処理、page build、
  DVI shipoutを分離したmicro/macro benchmark
- ASCII、和文、和欧混植、禁則が多い狭い段落、100頁、横組、縦組の固定corpus
- 同一TeX Live tree・warm/cold条件・CPU affinity・release LTOで交互に走らせたwall/CPU値
- 公開DVI仕様に基づく正規化event列、font identity、sp座標、page数が同等の実行だけを性能標本に採用

必須corpusの幾何平均と各主要caseをともに`PraTeX / upLaTeX < 1.2`へ置く。一つの巨大caseで
小さい文書の退行を隠さない。5%以内の幾何平均と10%以内の各caseは、その後のstretch goalとする。
PDF直接出力はupLaTeX単体と同じ仕事ではないためDVI gateへ混ぜず、upLaTeX + driverの
pipelineと別に測る。

最適化はsafe Rustの範囲だけで行う。`unsafe`を前提にした案はこの性能計画の候補に含めない。

性能変更は、同じrelease設定・同じ合成入力で変更前後を交互に走らせ、出力の一致を
確認してから採用する。測定用入力、実行ファイルの複製、logはリポジトリ外の
`%TEMP%` にだけ置き、版方へ入れない。

## 現在の一頁budget（Linux perf、`82fa3a2`）

TeX Live 2026、同じ一頁入力を15回測った既知の分解は次である。これは機能完成後の
合格値ではなく、native探索に使える現在の時間budgetである。

| | wall |
|---|---:|
| upLaTeX DVI | 229 ms |
| PraTeX、現在の通常探索 | 524 ms |
| PraTeX、資材を手元へ置いた診断経路 | 140 ms |
| 上の524 ms中の外部`kpsewhich` | 約291 ms |
| 上の524 ms中の自前`ls-R`索引等 | 約93 ms |

このcaseのstrict gateは`229 * 1.2 = 274.8 ms`未満であり、現在値は約2.29倍で不合格である。
一方、外部processだけを同じ意味のnative resolverへ置換した概算233 msは約1.02倍なので、
通常lookupの子process 0は単なる高速化候補でなく最初の必須sliceである。ただし140 msは
TeX tree探索を省いた非対称条件なので合格標本にせず、233 msも実装後に同一tree・同一resolver結果・
等価DVIで再測定する。JFM、K/X、縦組等を加えるたびにこの余裕を再計上し、完成間際まで借金を
隠さない。

## 横組JFM glyph sliceの欧文DVI gate（2026-08-23）

`origin/main`のrTeXと横組JFM glyph枝へ、同じ`cmr10.tfm`と次のbyte-only plain入力を与えた。
engine comment長が違うpreambleを除き、最初のBOP (139) からEOP (140) までをbyte列として比較した。

```tex
\catcode`\{=1 \catcode`\}=2
\font\f=cmr10 \f \hsize=200pt \parindent=0pt \tolerance=10000
\shipout\vbox{The quick brown fox jumps over the lazy dog. The quick brown fox jumps.\par}
\end
```

mainはBOP offset 43、glyph枝は45だったが、page bodyは双方183 bytesでbyte差分0だった。
従ってwide node、font selection enum、DVI `set2`/`set3`追加は、このfixtureのbyte glyph opcodeと
sp座標を変えていない。この検査は意味退行gateであり、upLaTeX 1.2倍未満の性能合格を示す値ではない。
横組JFMを含む同等DVI corpusと同一TeX treeが揃った時点でwall timeを別に再測定する。

## 過去のWSL e-upTeX診断値

次の値は、同じPC、同じWSL、同じCPU scheduler上でPraTeXとTeX Live 2026 e-upTeXを
交互に走らせた過去のmicro benchmarkである。upLaTeX format、JFM、page build、DVI出力を
含まないため、現在のhard gateの合否には使わない。hot pathの遅い箇所を見つける診断値として
だけ残す。

`4745f3c`をWSL上でもrelease LTO buildし、INITEX、fmtなし、探索なしで測った。入力は
macro展開と `\advance\count0 by 1` を1000万回行い、終了時に値を検査する。2回warm-up後、
順序を反転しながら各11回測ったwall中央値は次である。

| | PraTeX | e-upTeX | 比 |
|---|---:|---:|---:|
| 空に近いINITEX | 14.361 ms | 151.657 ms | 0.095 |
| 1000万回展開・整数加算 | 1975.460 ms | 1140.525 ms | **1.732** |

起動を概算で控除するとengine部分は約1.98倍であり、1.2倍gateを明確に越えた。このため
`codex/perf-wsl-euptex-safe`を切り、safe Rustのprofile/refactorを先に行う。Windows nativeの
PraTeX/e-upTeX値は環境差の参考にだけ残し、合否へ使わない。

LLVMのinstrumentation profileでは、1000万回入力におよそ次の回数があった。

- `InputStack::get_next` / `Scanner::get_next`: 1.11億 / 1.01億回
- `get_x_token`: 9000万回
- integer参照: 4110万回
- `scan_keyword`: 2000万回
- `RawVec::grow_amortized`: 約1000万回

10M入力だけで学習したLLVM PGOも診断として試した。CPU 0固定の追加測定ではgeneric PGOが
2151.80 msから1479.86 msへ短縮したが、同じ列のe-upTeX 1097.07 msに対してなお1.349倍だった。
狭い入力へのPGOを製品上の解決とはせず、profileが示した確保とdispatchを一件ずつ直す。

## キーワード成功経路の無確保化

TeXの§407に相当する `scan_keyword` は、成功時にも一致済み字句を `Vec`へpushしていた。
1000万回入力では `by` のためだけに約1000万回のgrow/freeが発生する。現行engineの最長語は
6字なので、6字までは局所配列へ置き、失敗して字句を戻す時だけ `Vec`を作る。7字以上も
従来どおり動くheap fallbackを残し、入力上限にはしない。

親 `4745f3c` と `955318e` をWSL rustc 1.97.1、release LTO、CPU 0固定で比較した。100万回版を
4回warm-up後、順序を交互にして各31回測った。

| | 親 | 無確保化 | 短縮 |
|---|---:|---:|---:|
| wall中央値 | 252.708 ms | 240.270 ms | 4.92% |
| child CPU中央値 | 257.403 ms | 243.710 ms | 5.32% |

先頭空白と大文字、部分一致失敗の復元順、7字超の成功と失敗を直接試験した。release全体は
507 passed、0 failed、6 ignored。TRIPは両段exit 0、999 records同士で、preamble comment、
pointer、末尾paddingを除く意味差0だった。PraTeX DVI SHA-256は
`b20af20a1463c6846f0c4c1ce687cd6354ce1a5f65ee401507627570787ae9fe`のままである。
unsafe Rustは使っていない。

## 最上位整数代入の直接化

整数演算の代入は、group外でも毎回 `Definition` と `Variable` へ包み直し、保存levelを調べていた。
最上位では局所・大域代入の意味が同じで、保存すべき外側の値もない。`9bb6023`ではloggerへ同期する
`escapechar` / `newlinechar` を先に処理した後、`cur_level == 0`だけ整数表へ直接書く。group内、
`globaldefs`、高位registerの既存経路は変えない。

独立targetを用い、CPU時間で比較した結果は次である。

| workload | 親 | 直接化 | 短縮 |
|---|---:|---:|---:|
| 100万回、31標本の中央値 | 272.864 ms | 256.476 ms | 6.00% |
| 1000万回、11標本の中央値 | 2447.354 ms | 2257.860 ms | 7.74% |

1000万回の平均でも5.69%短縮した。release全体は507 passed、0 failed、6 ignored。
TRIPは両段exit 0、`tripos.tex`一致、DVI hashと既知の999 records意味差0を維持した。
この時点でも同一WSL e-upTeX比1.2未満には届かなかった。当時はいったん性能専用作業を止めて
e-TeX/pdfTeXと日本語組版の統合へ戻ったが、現在はsafe Rustのまま主要sliceごとに退行を測る。

## 一字の差し戻し

数値や条件の走査は、先読みした一字を `back_input` で頻繁に戻す。従来は一回ごとに
一要素の `Vec<Token>` と `Rc` を作っていた。`TokenListReader` に一字を直接保持する
表現を加え、通常のtoken listは従来どおり同じ `Rc` を共有するようにした。

測定入力は次の130 bytes（末尾LFを含む）で、100万反復中に約200万回の一字差し戻しを
通る。

```tex
\catcode123=1 \catcode125=2 \count0=0\relax \def\x{\advance\count0 by1\relax \ifnum\count0<1000000\relax \expandafter\x\fi}\x\end
```

- fixture SHA-256:
  `891B4D7B8B647F0E05886065C55716E0D195983E2C8F8E0B548E34248D1EE6FC`
- Windows x86_64、rustc 1.98.0、release LTO
- 各実行ファイルを2回warm-up後、順番を交互にして各11回測定
- wall timeとprocess CPU timeの中央値を比較

| | 変更前 | 変更後 | 短縮 |
|---|---:|---:|---:|
| wall中央値 | 768.249 ms | 510.810 ms | 33.51% |
| CPU中央値 | 750.000 ms | 500.000 ms | 33.33% |

全22回で終了値は0、stdout SHA-256は
`C91C3D5D175B00E4D9E00BB5F88A240BFDC339339DC777D6B51419141124E233`、
log SHA-256は
`2F4892969B144313E1A6710D8C5C5DFE18F5B76B3829F174210A489397790609` で一致した。

64-bit buildでは `TokenListReader` は16 bytesから24 bytesになるが、最大variantは別に
あるため `InputSource` 全体は56 bytesのまま変わらない。unsafe Rustは使っていない。
release全343試験を通し、最適化前後のTRIP DVI SHA-256も
`27B79B612B94A1D2815A8747D09B6BA665F2ADFB9F521FCFE7020C6347A29342` で一致した。

## CJK token導入時のASCII退行確認

UTF-8 CJK tokenとtyped制御綴を足した枝でも、上と同じASCII fixtureを使い、直前の
`9d04c08` とrelease LTO buildを交互に各11回測った。両方とも同じVaak checkoutを使い、
2回ずつwarm-upした。

| | `9d04c08` | CJK token枝 | 変化 |
|---|---:|---:|---:|
| wall中央値 | 553.506 ms | 542.120 ms | -2.06% |
| CPU中央値 | 546.875 ms | 531.250 ms | -2.86% |

stdoutとlogのSHA-256は全22回でそれぞれ一種類だけで、変更前後が一致した。小差はcode配置や
測定揺らぎの範囲なので高速化とは数えないが、ASCII fast pathの退行は観測されなかった。
CJK用decoderと `kcatcode` 検索はASCIIでは呼ばず、typed hashと逆引き表もwide制御綴を
初めて作るまで確保しない。測定用source tree、target、logは `%TEMP%` のみに置いた。
同じworktreeでrelease全406試験とTRIP二段を通し、TRIP DVI hashも直前枝と一致した。

## 統一文字分類器のASCII退行確認

`catcode` / `kcatcode` の問い合わせを `CharacterClassifier` traitへ統一した枝を、直接の親
`9af3f19` と比較した。短い計測ではWindowsのprocess CPU timeの15.625ms粒度が相対的に
大きいため、上のfixtureの終了値だけ300万へ増やした。両方を同じrustc 1.98.0、同じVaak
checkout、release LTOでbuildし、1回warm-up後に順番を交互にして各11回測定した。

| | `9af3f19` | 統一分類器枝 | 変化 |
|---|---:|---:|---:|
| wall中央値 | 1642.206 ms | 1636.016 ms | -0.38% |
| CPU中央値 | 1625.000 ms | 1593.750 ms | -1.92% |

stdout SHA-256は全22回で
`25855EADFEEFB5EA17162B1E1E012A6B87758354BB4759C8FE486DFE8B91F5BF`、logは
`E5C427B0A95D409FD86A1C7CA5D4E65583864ACCD45F05AB61F2E0406C621B87` の一種類だけで、
終了値も全て0だった。小差は測定揺らぎとして高速化には数えないが、ASCII退行は観測
されなかった。組込み経路は `Eqtb` 自身へ静的dispatchし、中間object、allocation、
Unicode表引き、拡張class ID生成をASCIIに加えない。`CatCode` は `repr(u8)` である。

## WSL TeX Liveの探索cost

Windows側にnative `kpsewhich`がない実環境で、Ubuntu-24.04上のTeX Live 2026
（Kpathsea 6.4.2）を測った。`kpsewhich`を一件ずつWSLで起動する測定は同じqueryを5回行い、
初回3,892.4994 ms、以後349.3728、321.1636、328.9875、348.1094 msだった。

PraTeXの既定resolverが三つの`ls-R`を発見・UNC越しに索引化し、`cmr10.tfm`を解決して
Windows側から開くend-to-end試験は8.87 sだった。release LTOのlink 3分23秒はこの値に
含めない。索引は以後の同名検索でone-shot processを省けるが、現在の全database先読みは
短い一件だけなら遅い。初回3.89 sと追加query約0.33 sの列と比べると合計16件前後
（初回後さらに約15件）、warm値だけで8.87 sを割ると26--28件が概算の損益境界である。

これは一台のWindows--WSL/UNC構成の値で、native TeX Liveや他のstorageを代表しない。
一回の手測定であり、再現用の環境依存試験は解決経路を索引だけに固定していない。
正しさのため、曖昧な候補や実在する利用者treeでは引き続きone-shot CLIへ戻す。次の性能枝では
同じ解決結果を条件に、lazy/adaptive索引化とWSL内でのbounded読込みを別々に比較する。
詳細は [TeX Live探索の移植記録](kpathsea-port-notes.md) にある。

### `ls-R`索引表現のisolated safe-Rust実験（2026-08-22）

end-to-end変更へ先走らないため、実リポジトリを編集せず、WSL
`/tmp/pratex-lsr-safe-probe-*`の独立prototypeで次の四方式を比較した。

- A: 現行readerの所有`HashMap`意味を再現
- B: `RandomState`を保ち、unique name数とdirectory数を正確に予約
- C: deterministic FNV-1aの所有`HashMap`
- D: 一つのbyte arenaにoffset/lengthを持ち、FNV bucketをcollision-safeなbyte比較で連鎖

環境はWSL2 Ubuntu 24.04、Linux 6.18.33.2、i7-13620H、CPU 2固定、rustc 1.97.1
（LLVM 22.1.6）、`-O -C codegen-units=1`。warm-up 3回後、方式順を回転して24標本を取った。
probe source SHA-256は
`824b6b2220d46315d377552b6a0deda53193e506b75125f25a7302b9ed6f7e87`、binaryは
`71839ced31f7706565482331243cf8bba8452eadd72d3b48b2256004be3f3149`である。
ただしsourceとbinaryはWSL `/tmp/pratex-lsr-safe-probe-20260822`の消去により残っていない。
このhashは当日のartifact同一性の記録であって、hashだけから再現はできない。従って本節は
探索的測定として扱い、性能gateには使わない。採否を決める再測定では、A--D、意味論assert、
合成非UTF-8 fixture、interleaved測定、RSS child modeを持つ`tools/lsr_safe_probe.rs`と、
fixture発見・一時directory・toolchain/hash収集・CPU固定を行うrunnerを先にcommitする。

公開CLI `kpsewhich --all ls-R`で得たfixtureは次の三つ。最大のdist treeは
288,994行、17,298 directory、254,397 accepted entry、231,561 unique basename、
22,836 cross-directory extra candidateだった。

| fixture | byte | SHA-256 |
|---|---:|---|
| config | 80 | `418d569540155c83d3e01fb88cf8ecbf5870deedc3844f86d38df2f9b4d4f5b2` |
| var | 3,330 | `25692224564e8ce593b8bbf8cabd142557b129aa69303d4d2021f4a6433c9e26` |
| dist | 5,674,350 | `17677745673338040a914c26c1935da2c6515d573d3bc7fb3d1b7dbaf4cc0d9e` |

全方式でbasenameをbyte-sortした**全name→candidate directory列**を直接`assert_eq`し、
distのsemantic FNV64 `aa62d954fb168fec`が一致した。4096件の固定hit/miss corpusも結果列を
直接比較し、checksumはhit `11f73eace8743fef`、miss `9eb4e710cf95c4fd`で一致した。
非UTF-8 basename `na\xffme.tex`、重複抑制、hidden entry拒否を含む合成fixtureも一致した。

最大distのbuild時間:

| 方式 | 中央値 | 平均 | p10 / p90 | A比 |
|---|---:|---:|---:|---:|
| A | 49.562 ms | 52.726 ms | 45.849 / 61.076 ms | -- |
| B | 27.112 ms | 27.768 ms | 22.836 / 31.496 ms | -45.3% |
| C | 25.830 ms | 26.871 ms | 21.956 / 30.436 ms | -47.9% |
| D | 24.164 ms | 25.823 ms | 21.556 / 28.646 ms | -51.3% |

最大distのlookup中央値（ns/query）と個別process `/proc` VmHWM:

| 方式 | hit | miss | VmHWM（raw入力込み） |
|---|---:|---:|---:|
| A | 60.287 | 21.101 | 56,832 KiB |
| B | 61.090 | 22.026 | 44,464 KiB |
| C | 53.898 | 31.733 | 44,464 KiB |
| D | 56.285 | 42.479 | 32,168 KiB |

Bはbuildとpeak memoryを大きく改善したが、正確なunique-name/directory件数を測定区間外から
与えたoracle上限である。従って採用結果ではない。実readerが一回の走査で安価に作れる
過大容量hintを設計し、end-to-end resolverで再測定する第一候補とする。

C/DはhitだけならAより速い一方、missが50.4%/101.3%悪化した。さらにunkeyed FNVは、外部から
細工できる`ls-R`に対するhash-flooding DoSを許し、Dのchainはbuild/lookupとも線形へ退化する。
したがって現状は非推奨で、`RandomState`を外さない。Unix prototypeはraw byteを保持したが、
Windows readerがinvalid UTF-8を拒む既存platform policyも表現変更で勝手に変えない。
この絶対値は現行意味を模したprototype内訳であり、PraTeX end-to-end値ではない。

## Linux TeX Liveでの費用分解

Claudeが`codex/euptex-utf8-cjk-token`系の`04d4189`をLinux 7.0、i7-8650U、TeX Live 2026、
release LTOで外部監査した。現在枝そのものやWindows--WSLの数値ではないため絶対値を混ぜないが、
同じ一頁LaTeX入力を12回測って費用を段階的に足した結果は次だった。

| 段階 | PraTeX | 増分 |
|---|---:|---:|
| 書式なし・空入力 | 約1.3 ms | process起動 |
| 16.8 MiBのLaTeX fmtを読む | 約114 ms | fmt復元 約113 ms |
| 一頁を組む | 約141 ms | 組版 約27 ms |
| TeX Live外部探索を使う | 約522 ms | 探索 約381 ms |

同じ条件のpdfTeXは一頁約196 msで、内訳の推定はkpathsea初期化約137 ms、fmtと組版約50 ms。
PraTeXの組版部分は約27 ms対約25 msでほぼ同じであり、少なくともこのfixtureから
「safe Rustの組版意味論が支配的に遅い」とは言えない。現時点の大きな費用は探索とfmt復元である。

監査では用途別`--show-path`等の外部起動が一回約137--144 ms、自前`ls-R`索引が約103 msだった。
したがって優先候補を次とする。

1. 公開`texmf.cnf`の必要な部分集合を独立実装するか、正しさを証明できる場合だけ
   `--show-path`を遅延し、外部kpathsea初期化回数を減らす。
2. fmt 16.8 MiBの内訳を型・表ごとに計測し、既知個数の予約や疎表の表現を個別に比較する。
3. `ls-R`の一行ごとの確保、HashMapの予約不足、短いkeyのhash costをprofileし、変更前後を
   同じ索引結果で比較する。

`texmf.cnf`全体を推測実装して探索順を変える最適化は採らない。曖昧・未対応な式は従来どおり
公開`kpsewhich`へ戻す。1.3 msの探索不要起動、ASCII fast path、TRIP意味一致をhard boundaryにし、
数値は同じcommit、TeX tree、language設定、親processのみの計測で取り直してから採否を決める。

## fmt collectionのbounded予約（2026-08-24）

利用者のLinux測定では、ほぼ同じ長文を三回処理してからDVI driverまで通した列が
upTeX 3.15 sに対してPraTeX 9.14 sだった。この絶対値にはclass、package、TeX tree、
`dvipdfmx`が含まれるため、まずWindows上の隔離CTAN cacheでengine内部を三段に分けた。

- rustc 1.91.0、Windows x86_64、release LTO
- 同じ17,446,628 byteの`latex.fmt`
- formatを読んで直ちに終わるcase、空の`article`、和文を含むLatin 300段落のcase
- 変更前Aと変更後Bを一回ずつwarm-upし、順序を反転しながら各8回
- `PRATEX_PERF_PHASES`を一時的に入れた測定binaryだけでfmt読込みと行分割を計時し、
  計測後にinstrumentationとprobe sourceを版方から除いた

事前の一回測定では、fmt全体468.8 msのうちfile読込み26.9 ms、Eqtb復元441.5 ms、
hyphenation表0.4 ms未満だった。300段落の行分割301回は合計86.4 msであり、少なくとも
このcache済みcaseではEqtb復元が最初のhotspotだった。一般の`Vec`と`HashMap`のfmt復元は
宣言個数を知っているのに空collectionへ逐次pushしていたため、初期capacityを越える
token listやtableではgrowとcopyを繰り返していた。

変更後は最初の予約を4,096要素か要素payload幅換算64 KiB相当の小さい方へ制限し、`try_reserve`を
使う。fmtの宣言長はuntrustedなので、宣言値そのものを`with_capacity`へ渡さない。予約失敗後に
逐次growへ戻すと、同じmemory pressure下でallocationを繰り返すため、typedな
`AllocationFailed`として停止する。`usize::MAX`だけを書いたtruncated Vec/HashMapが巨大確保せず
`IncompleteFile`になる試験と、要素payload幅を含むcapacity hint上限試験を置いた。この64 KiBは
要素payloadの見積り上限であり、`HashMap`のload factor、control byte、allocator metadataを含む
実allocation byte数の上限ではない。

表は中央値 ± 母標準偏差である。

| case | wall A | wall B | 変化 | Eqtb A | Eqtb B | 変化 | peak RSS A | peak RSS B |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| formatだけ | 595.56 ± 51.08 ms | 526.68 ± 56.60 ms | -11.56% | 403.78 ± 39.04 ms | 338.92 ± 51.39 ms | -16.06% | 37.90 ± 0.65 MiB | 35.72 ± 0.69 MiB |
| 空`article` | 807.81 ± 56.24 ms | 684.58 ± 72.81 ms | -15.26% | 422.07 ± 37.39 ms | 342.10 ± 31.65 ms | -18.95% | 38.12 ± 0.87 MiB | 36.65 ± 0.64 MiB |
| 300段落 | 1,412.37 ± 83.09 ms | 1,309.17 ± 82.87 ms | -7.31% | 451.87 ± 23.93 ms | 387.32 ± 53.75 ms | -14.28% | 39.02 ± 0.82 MiB | 36.10 ± 0.98 MiB |

`SOURCE_DATE_EPOCH=1709210096`でA/Bを別々に再実行し、300段落DVIは双方166,940 byte、
SHA-256 `441889a18b75e3aac97c3e7c11e98978a6c6eb3a8399002c6f828acc5edb467c`、logは双方
SHA-256 `54df4ea04fb7133c0514857298561b4123ad35c2aea178b2783d262792df564a`だった。
formatだけと空文書はpageをshipoutしないのでDVI比較対象ではない。

これはOS cacheを明示purgeしていないwarm測定であり、cold filesystem値ではない。また
Windowsの平坦化cacheを使った内部A/Bで、利用者のLinux TeX Live tree、`mainpra.tex`、
`dvipdfmx`を再測定した値ではない。したがって9.14 s全体がこの比率だけ縮むとは扱わず、
Linuxの同一corpusで改めてengine三回とdriver一回を分離する。

A/Bのsource基点はともに`cc65f38`で、Aの実行file SHA-256は
`ea5ecc821fd6416ea75c84dd0657f42d27c12508902c52cf9d8d86a39fded337`、Bは
`56da8321f7fdf7fc9f235e532b537827d4c5b2d6c94bf50b059e5ae2ba31582a`である。48標本のraw値は
[`benchmarks/fmt-bounded-reservation-20260824.csv`](benchmarks/fmt-bounded-reservation-20260824.csv)
へ固定した。Windowsのprocess CPU timerはこの短いcaseに対して15.625 ms刻みと粗く、scheduler
競合も分離できないため採否には使わず、交互実行のwall、process内`Instant`によるfmt区間、
DVI/log一致を使った。

### WSL resolver失敗反復の診断

同じ日に、`hyperref`、`graphicx`、`siunitx`、`pxrubrica`を含む平坦化package probeを
Windows上で走らせた。`graphics.cfg`不足で停止するまで7.59 sかかり、外部processは13回、
合計3.645 sだった。13回すべてが失敗後に繰り返された
`wsl.exe --cd / --exec wslpath -w /`である。backend discoveryの失敗がrun中に記録されず、
別のoptional file lookupごとに同じ発見処理へ戻ることを実測した。

これはLinuxの利用者benchmarkには存在しないWindows固有の列なので、上の190%差の説明には
使わない。将来の最小修正ではfailureをrun-localに保持し、resolver cacheを明示clearした時だけ
再発見する。ただしstale DB、alias、拡張子補完、casefold、非`!!`利用者treeを
「証明済み不在」へ潰さず、通常のnative kpathsea統合とは別checkpointで測る。

## 次の候補

測定済みの次候補は、探索外部processの削減、fmt内訳の計測、`ls-R`索引の確保削減、
入力行bufferの再利用、PDF文字命令の一時`String`除去である。一つの枝へ混ぜず、同じ
出力hashとTRIPを条件に個別採否を決める。

# 寸法走査のfirst-token handoff

更新: 2026-08-25
採用commit: `d5eb585`

## 結論

寸法scannerは空白と符号を読み、最初の展開済みtokenとcommandを既に得ている。
従来のnon-internal経路はそのtokenを`back_input`し、整数scannerが直後に同じtokenを
再取得していた。`UnexpandableCommand` / `Token`の対をprivateな整数経路へ直接渡し、
終端token、空白消費、符号、8/10/16進、alphabetic定数、internal値の決定は従来の一箇所に残した。

| fixture | baseline wall中央値 | candidate wall中央値 | paired wall幾何平均比 | paired task-clock幾何平均比 | paired instructions幾何平均比 |
|---|---:|---:|---:|---:|---:|
| 寸法1,600,000代入 | 1.2095 s | 1.1714 s | **0.965479** | **0.966130** | **0.964690** |
| 299頁`lipsum` | 1.2878 s | 1.2839 s | 0.996439 | 0.996567 | 1.001318 |

寸法microは31組すべてでcandidateが短かった。instructionsも31組すべで0.9645--0.9649倍に収まり、
狙った一回のbackup / source push / token再dispatchを減らした効果と整合する。299頁では
wallは20組中13組、task-clockは11組だけcandidateが短く、約0.35%の差は分散より小さい。
したがってend-to-end短縮値は上限やgate達成へ外挿せず、局所勝利と非回帰だけを採用根拠にする。

両binaryの299頁DVIとaux、microのlogはbyte一致した。先頭`.` / `,`、複数符号、8/16進、
alphabetic定数、macro展開で得た数字はprocess testへ固定した。raw counterは
[`dimension-first-token-20260825.tsv`](dimension-first-token-20260825.tsv)、binary・fmt・入力・環境は
[`dimension-first-token-20260825-provenance.tsv`](dimension-first-token-20260825-provenance.tsv)にある。

## 境界と残件

handoff時に`align_state`は触らない。最初の取得で既にalignment deltaが加算済みで、
従来のbackupと再取得の差引は0だからである。終端tokenを一回戻すinteger側の契約と、
alphabetic定数の次tokenに対する手動相殺も変えない。

今回は寸法の通常non-internal経路だけである。glue入口の符号やe-TeX式から寸法への
first-token handoff、数字の終端tokenをfactor / term / sum間で複数回戻す問題、unit keyword走査は
別candidateにする。中央入力経路の診断context、interrupt確認時点、alignmentはそれぞれ独立に固定する。

## 検証

focusedは新unit 1件、寸法process 1件、`jdimen` 7件、`etexexpr` 12件が成功した。
`cargo test --release --locked --no-fail-fast`は**934 passed、0 failed、11 ignored**。

公式CTAN TRIP archiveとmanifestの10資材をSHA-256再検証し、CPU 0・1 jobの隔離targetで
`trip` featureをbuildした。Stage 1 / 2はともにexit 0、`tripos.tex`は公式とbyte一致、
`8terminal.tex`は0 byte、PLtoTF→TFtoPLのTFMとPLはbyte一致した。固定commentのDVIは
公式2,920 byteと完全一致し、SHA-256は
`09802695e330d34acec9192c15debe2de65e34fcbd3f947db9c8924240b1fe0a`である。TRIP実行fileは
`6d7eef7a7df32a75bae03f14acf94e4428da0b8d413de3261647f8989351565b`、fmtは511,386 byte・
`086098b60d0093c2a07c11a43f2beb122fb1f7bb7f51cc8bf37ea2db56d80cc8`。隔離artifactは
`/tmp/pratex-trip-20260825.iNah6roG/current-dimension-first-token*`に残した。

# 糊走査のfirst-token handoff

更新: 2026-08-25

## 結論

`scan_glue`は空白・符号と最初の展開済みcommand/token対を既に取得している。従来の
non-internal経路はtokenだけを`back_input`し、寸法scannerが直後に再取得していた。
取得済みの対と符号を`inf=false`固定の寸法入口へ渡し、backup、入力frameのpush/pop、
token再dispatchを除いた。

| fixture | baseline wall中央値 | candidate wall中央値 | paired wall幾何平均比 | paired task-clock幾何平均比 | paired instructions幾何平均比 |
|---|---:|---:|---:|---:|---:|
| 糊幅1,600,000代入 | 1.3974 s | 1.3533 s | **0.965541** | **0.965646** | **0.960493** |
| 299頁`lipsum` | 1.4275 s | 1.4192 s | 0.994623 | 0.994808 | 1.000125 |

糊microではwallが31組中30組、task-clockとinstructionsは31組すべてでcandidateが短かった。
wallの残る1組も比1.000019である。299頁ではwallとtask-clockが20組中11組だけ短く、
instructionsは9組だけ短かった。したがって299頁の約0.54%短縮をgate達成へ外挿せず、
局所勝利とend-to-end非回帰だけを採用根拠にする。両binaryの299頁DVIとauxはbyte一致した。

raw counterは[`glue-first-token-20260825.tsv`](glue-first-token-20260825.tsv)、binary、依存commit、
入力、format、意味hash、実行環境は
[`glue-first-token-20260825-provenance.tsv`](glue-first-token-20260825-provenance.tsv)に固定した。

## 意味境界

- 明示糊の先頭符号は幅だけへ掛け、後続の`plus` / `minus`成分へ掛けない。
- 内部Glue/MuGlueの先頭符号は従来どおり`GlueSpec::negate`で全成分へ掛ける。このbranchは
  handoff対象にしていない。
- normal/muの判定は寸法scannerの中央経路へ残す。公開するcrate内入口は`inf=false`固定なので、
  糊幅へ`fil` / `fill` / `filll`を許さない。
- commandとtokenを組で渡し、既に解決した`\noexpand`状態をtokenの再取得で作り直さない。
- 最初の取得で`align_state`は更新済みなので、handoff側では加減しない。

共通core抽出だけの初期形では通常寸法に薄い中間symbolが残ったため、`scan_normal_dimen`と
token取得だけを行う`scan_dimen`をalways-inlineにした。最終binaryでは両中間symbolが消え、
299頁の予備6組でinstructions比が0.999939へ戻った後に、上表の全組を取り直した。

## 再現性

最初のA/B中に共有path dependency `../vaak`へ別担当の未commit変更が現れたため、その測定は
全て破棄した。正式値はVaak `7dc011b`のclean detached worktreeとPraTeX `763e4a7`のdetached
worktreeを同じ一時rootへ置き、同じPraTeX絶対path、同じ`CARGO_TARGET_DIR`でbaselineをbuild、
binaryを保存、candidate patchを当てて再buildした値だけである。

## 検証

focused process testは、明示糊の奇偶符号、macro展開された先頭数値、mu幅、内部糊全体の反転、
元register不変を固定する。局所logは測定分だけ異なるbanner時刻行を除いてbyte一致し、299頁は
双方299頁・1,396,468 byte、DVI SHA-256
`285ef47cd661c712237948afd0085b370c9c1b9492f5ec8acd4a6660d412655d`、aux SHA-256
`db187909886572dd6e4beed4704f83078aecb28738881b6c4a67012f1d536027`へ一致した。

focused 20件に続き、`cargo test --release --locked --no-fail-fast`は
**935 passed、0 failed、11 ignored**だった。公式TRIPはStage 1 / Stage 2ともexit 0、
`tripos.tex`とPLtoTF→TFtoPLは公式fileへbyte一致し、`8terminal.tex`は0 byteだった。
`-output-comment= TeX output 1776.07.04:1200`を与えた対照runのDVIは公式2,920-byte DVIと
byte単位で一致し、SHA-256は双方
`09802695e330d34acec9192c15debe2de65e34fcbd3f947db9c8924240b1fe0a`である。

TRIP feature実行fileのSHA-256は
`f34ab2265e45ad6d4a9d1189ec872b6069045798293fdf4db4bf1c9ab7cd458b`、既定binary fmtは
SHA-256 `2c9bbab7abb91c9b4cac42016a0f5a67b0e5252c45668a328326146ebb3bcf07`だった。
隔離artifactは`/tmp/pratex-trip-20260825.iNah6roG/current-glue-first-token*`と
`actual-glue-first-token`に残した。

# MacroCall引数参照mask案の棄却

更新: 2026-08-25

## 結論

macro呼出しごとに`parameter_text`と`replacement_text`を走査していた参照引数maskを、
`MacroCall`生成時に一度だけ導出してprivate fieldへ保持する案を測った。fmt wireは変えず、
fmt読込み時にも同じconstructorで再導出し、focused testは6件成功した。しかしmacro内部だけを
強く反復する既存fixtureで明確に遅くなったため、candidate sourceは棄却してcommitへ残さない。

| fixture | baseline wall中央値 | candidate wall中央値 | paired wall幾何平均比 | paired task-clock幾何平均比 | paired instructions幾何平均比 |
|---|---:|---:|---:|---:|---:|
| 299頁`lipsum` | 2.7758 s | 2.7548 s | 0.990571 | 0.991236 | 1.015209 |
| 8引数macro 100,000反復 | 0.9166 s | 0.9693 s | **1.057001** | **1.056055** | **1.021428** |

299頁ではcandidateのwallが20組中18組で短かった。一方、狙ったmacro fixtureでは31組中30組で
wallが長く、task-clock中央値は450.96 msから477.42 ms、cyclesのpaired幾何平均比も
1.059820へ悪化した。両fixtureの出力はそれぞれDVI・auxまたはlogでbyte一致した。

この相反は単なるCPU周波数差ではない。instructionsは299頁の全20組で1.015倍前後、macro fixtureの
全31組で1.021倍前後へ増えた。ただし、field追加、constructor経路、release LTO後のcode layoutの
どれが増加を支配したかはこのA/Bだけでは特定していない。299頁だけの見かけ上の約1%改善を採用根拠にせず、
意図したhot pathで約5.7%悪化した事実を優先する。

raw counterは
[`macro-call-derived-mask-rejected-20260825.tsv`](macro-call-derived-mask-rejected-20260825.tsv)、
binary・fmt・入力・環境は
[`macro-call-derived-mask-rejected-20260825-provenance.tsv`](macro-call-derived-mask-rejected-20260825-provenance.tsv)
に固定した。baselineとcandidateは同じcommitを基点に3分以内にbuildし、同じVaak commit、同じfmt、
CPU 0固定、`SOURCE_DATE_EPOCH=1709210096`で奇偶roundの実行順を反転した。

## 次の境界

`MacroCall`全体の表現を増やす案は止める。次は既存表現を保ったまま、数値・寸法・糊scannerが
既に展開した先頭tokenを差し戻して直後に再取得する往復を、privateなfirst-token handoffで一段ずつ
除く。候補ごとにmicro fixtureと299頁を分離し、中央入力経路を変える時は診断context、
`align_state`、interrupt時点、全release、TRIPをgateにする。

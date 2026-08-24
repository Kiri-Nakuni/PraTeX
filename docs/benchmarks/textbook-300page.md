# 300頁級教材fixtureの設計

`tools/fixtures/textbook-300page.tex`は、長文のmacro/token走査、page build、数式、float、
TikZ/PGF、目次、相互参照と補助fileの複数run収束を同じ入力で測るための合成fixtureである。
第三者の教材本文、class sample、図、表、演習は複製していない。

## page数の設計と初回実測

- 24章 × 12個のpage-bounded lessonで本文288頁。
- titleとreader's mapで約2頁。
- 24章、288節、reader's mapからなる約313 entryの目次を7--9頁と見込む。
- 合計見積りは297--299頁。engine、format、font metricによる改行差を含む保守範囲は296--302頁。

2026-08-25にsystem TeX Live 2026と`13d1ab1`のPraTeXで三engineを各四回通し、三者とも
**298頁**になった。見積り範囲内なので、page数を合わせるためのsource調整はしていない。

各lessonは1頁に収まる量に抑え、末尾を`\clearpage`で区切る。将来の初回実測でoverflowが判明した
場合は、まず実page数と発生箇所を記録する。測定後に無言でsource量を調整して過去の結果と比較しない。

## 入力契約

- ASCIIだけの標準LaTeX `book` sourceであり、PraTeX、upLaTeX、LuaLaTeXに同じfileを渡す。
- 比較対象はDVI mode。`xcolor`は既存TikZ smokeと同じ`dvips` driverを明示する。
- runtimeのTikZ/PGF版とpackage file hashを測定ごとに固定する。初回のsystem TeX Live 2026実測は
  PGF/TikZ 3.1.11aであり、既存の隔離smokeが使った3.1.12と同じ版だと仮定しない。
- 数式は`amsmath`の`equation`、`align`、`cases`、matrixと`\eqref`に限定する。
- TikZは既存smokeと同じ基本的な`draw`、色、線、nodeだけを使う。
- LaTeX 2026標準のactive tieは未実装の`\ifincsname`へ入るため、fixture内で
  `\leavevmode\nobreak\ `へ固定し、三engineへ同じ定義を与える。この回避はe-TeX対応済みの主張ではない。
- pTeX固有primitive、engine version偽装、乱数、shell escape、外部画像、外部生成sourceを使わない。
- bibliography、索引、画像変換等の別processを混ぜない。

この契約は三engineでの成功を先取りして保証するものではない。特にPraTeXでのTikZ/PGF実績は
3.1.12の小さなDVI smokeに限られるため、長文fixtureの結果は別のgateとして記録する。

## 2026-08-25の初回診断

最初のsourceは、LaTeX 2026のactive tie `~`が未実装のe-TeX `\ifincsname`へ入ってPraTeXで停止した。
fixtureが必要とする非改行空白だけを`\leavevmode\nobreak\ `へ固定し、同じ定義を三engineへ与えた。
これはPraTeXが`\ifincsname`対応済みという意味ではなく、e-TeX残件を隠して互換判定した値でもない。

修正後はwarm-up三回と測定一回を逐次実行し、PraTeX、upLaTeX、LuaLaTeXがすべて298頁を完走した。
最終`.aux`は三者ともSHA-256
`f7efa4f43ae7031f61f2abf8a5fbbf28086eb712f67cfe822101abc5a310bca6`、`.toc`は
`0224d72072584a908716eb7a188c90d4c8ac5c5b351ecfb166d08e10f0d05f3e`へbyte一致した。
PraTeX/upLaTeXのDVIは各engine内で収束後に安定したが、相互には一致しなかった。

公開DVI opcodeをfont identityで正規化した比較の最初の意味差はcanonical record 61,819である。
最初のmoduleのfootnote内部で、upLaTeXの縦移動11,462,722 spに対しPraTeXは11,461,561 spとなり、
**1,161 sp（約0.0177 pt）**ずれた。続くruleと脚注本文も同量だけずれるため、font番号や整数幅だけの
encoding差ではない。この時のraw DVIはPraTeX 507,472 byte、upLaTeX 508,104 byteで、page数、aux、tocが
同じでも同等組版のhard gateには不合格とする。

診断一回のwallはPraTeX 8.360 s、upLaTeX 4.261 s、LuaLaTeX DVI 4.914 sだった。PraTeX/upLaTeX比は
1.962だが、計測runnerが当時realtime clockを使っており、さらに上記DVI意味差があるため正式な性能標本へ
採用しない。修正後は`perf duration_time`へ移行済みであり、1,161 sp差を解消してから15組を取り直す。

初回runtimeはPGF/TikZ 3.1.11aである。`tikz.sty` SHA-256は
`6e39ff4fdf9f126aff28880a7dd59fccc0e6735409d92ca455cdd2a4f2b4db53`、`tikz.code.tex`は
`5cf22e53ee27e044a06a4aebdd77924101a491915021428a61cd6ff3ff2c8e0e`、`pgf.revision.tex`は
`8c056cfd919cca2dc5e398c776865e1af6a8605ab9d715f452aa68327164b52d`である。隔離した既存smokeの
3.1.12という記録をsystem treeへ流用しない。

## 将来の測定で固定するもの

同一source、同一TeX tree、同じDVI modeを使い、engine本体と後段driverの時間を混ぜない。
各runについてwall time、CPU time、peak memory、exit、error数、page数を記録する。`.aux`と`.toc`の
hashをrunごとに保存し、連続するrunで両方が一致した時点を参照・目次の収束とする。

DVIは生成成功やraw hashだけでなく、font、opcode、sp座標を含む意味比較を先に行う。比較対象間で
意味が一致しないrunの速度を同一workloadの性能値として扱わない。

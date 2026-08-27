# 枝の地図

更新: 2026-08-25

**`main` は触らない。** tyti氏のTeX82実装を保存する歴史的な基点である。

```text
main (f174f44)                         素のTeX82
 └─ vaak                              Vaak bridge
     ├─ jdimen                        Q/H/zw/zh
     ├─ etex-expr                     \numexpr系
     └─ namespace                    名前空間Phase 0--7
         └─ full                     上記を集めた歴史的baseline
             └─ etex-latex           e-TeX/pdfTeX/LaTeX統合の主系列
                 └─ ...              機能単位のcodex/*枝
                    └─ codex/kpse-lsr-index
                        └─ codex/cjkv-region-layout
                            └─ codex/pdf-texlive-type1
                                └─ codex/ptex-jfm-core
                                    └─ codex/perf-wsl-euptex-safe
                                        └─ codex/euptex-integration-resume
                                            └─ ... codex2の機能・性能checkpoint
                                                └─ codex2/perf-resolver-index (f414757)
                                                    └─ codex3/perf-integration (bee8724) 性能停止点
                                                        └─ codex3/roadmap-integration  現在の機能統合枝
```

`full`は長く歴史的baselineで止まっていたが、2026-08-23以後は**検証済み機能の統合先**として
再び進める。作業を`full`上で直接行わず、現在は`codex3/*`で意味と試験を固定してから順次mergeする。
`codex3/perf-integration`は分裂していた機能枝を集約した後、同一DVIを保つ性能作業を行った枝である。
全releaseとTRIP/DVI意味gateは通過し、狭いmacro fixtureはupTeX比1.257になった。最新299頁の正式
end-to-end比はupLaTeX比1.615624（paired中央値）で、性能gate未達のまま利用者判断により
性能調整をいったん停止した。以後の機能checkpointは`codex3/roadmap-integration`へ集め、
引き続き`full`へ送る前に個別gateを通す。

## 現在地

| 枝 | 状態 | 固定した結果 |
|---|---|---|
| `codex/kpse-lsr-index` | push済みbaseline `dc1c554` | release test 455通過、失敗0、4 ignored。bounded `ls-R`索引とWindows--WSL探索境界 |
| `codex/cjkv-region-layout` | 歴史的R0 `ac6ad90` | typed `LanguageRegion`と`\pratexregion`。release test 466通過、失敗0、4 ignored。TRIPのDVI意味比較差0 |
| `codex/pdf-texlive-type1` | push済み `bb7235f` | TeX Live mapの複数resource、flags既定値、PFB Private `StdVW`。全release失敗0、4 ignored、TRIP 999 recordsで意味差0 |
| `codex/ptex-jfm-core` | push済み `4745f3c` | 公開仕様だけによるbounded JFM reader/model。release 503通過、失敗0、6 ignored。配布JFM 96件とTRIPを通過 |
| `codex/perf-wsl-euptex-safe` | 検証済み `9bb6023` | WSL同士の1.2倍gateを固定。keyword成功経路と最上位整数代入をsafe Rustで短縮。release 507通過、TRIP意味差0 |
| `codex/euptex-integration-resume` | 統合基線 `6ce8315` | e-TeX/pdfTeX、LaTeX、日本語組版、resolver、PDF/Type 1を統合した基線 |
| `codex2/perf-resolver-index` | `codex3`基点 `f414757` | Linux既定の組込みKpathsea、run-local resolver、固定CTAN tree測定、性能probeと不採用cacheの根拠を統合 |
| `codex3/perf-integration` | 性能停止点 `bee8724`、code `8e0e543` | vertical discard、run-local compiled spacing dispatcher、最小横組`prjlreq`を統合。型付きfmt、寸法・糊handoff、未展開tokenのcommand非所有化と不正token境界まで採用。全release 941 passed / 0 failed / 11 ignored、公式TRIP維持。三engine再測定はupLaTeX比1.615624で未達、性能調整は一時停止 |
| `codex3/roadmap-integration` | 現在の機能統合枝、code `09e1eca` | 性能停止点からJLReq `\inhibitglue` / `\disinhibitglue`、e-TeX `\showifs`、Type 1 at-size共有を統合。全release 950 passed / 0 failed / 11 ignored。公式TRIP固定comment DVIは公式2,920 byteへ完全一致 |
| `codex3/jlreq-list-edge-class0` | 未統合WIP `4572918`、両remoteへpush済み | 横組direct hbox/段落のclass 0端点。planner 20/20、日本語spacing 23/24。字下げ段落末尾glueの観測1件が未解決で、全release・TRIP未実施。解決まで統合禁止 |
| `claude1/perf-integration` | 未統合、push なし | `codex3/roadmap-integration`から分岐。fmtの行分割とtrie検証のhash、lig/kern表、列の容量、`panic = "abort"`を直した。299頁paired wall比は1.615624から**1.4750**、命令数−14.20%。DVIはbyte一致、release 953 passed / 0 failed / 11 ignored。**公式TRIPは未実施**で、1.3未達 |
| `claude1/tex82-perf` | 未統合、push なし | `main`から分岐した素のTeX82。Knuth TeXとの比を回帰で起動費と分けた。本文一つあたり1.782から**1.535**（命令）、1.681から**1.334**（時間）。DVI命令1,306,596個がKnuth TeXと一致。1.1未達で、到達には節点保持方式の作り替えが要る |

R0は組版localeの状態、group/global/fmt、表示だけであり、まだJFM、文字間隔、禁則、
font選択、DVI/PDF出力を変えない。R1以降は
[拡張可能なscript境界組版](extensible-layout-roadmap.md)で段階を分ける。

## 2026-08-24の分裂枝監査

`codex3/perf-integration`を作る際、remoteの未統合patchをproduction機能、検証補助、測定記録に
分けた。異なる基点の枝をmerge commitごと重ねず、必要な意味commitだけを現在基点へ移した。

| 元の枝 | 採否 | 現在の扱い |
|---|---|---|
| `codex2/etex-discards` | 統合 | 元`9c70a6c` / `b790ee3` / `6eed748`を`700973b` / `06a5b25` / `1a518cd`として移植。focused 6件成功 |
| `codex2/script-spacing-dispatcher` | 統合 | 元`3633878` / `763bbe4`を`58b9589` / `0e55c20`として移植。lib 43件、process 18件成功 |
| `codex2/prjlreq` | 統合 | 元`d8952d7` / `f94adac`を`971073b` / `c493a94`として移植。静的契約試験3件成功。現在枝でのprocess再測定は未実施 |
| `codex2/samply-windows-profile` | 既存と同値 | patchは`f414757`の祖先に既に含まれるため重ねなかった |
| `codex2/kpathsea-linux-gate` | 後続実装＋手動gateを統合 | Linux既定productionは後続bundled Kpathseaへ置換済み。配布側library用のignored testと実TeX tree runnerは現行`system-kpathsea` featureへ移植 |
| `etex-latex` | 既存と同値 | 全patchが現在基点の祖先またはpatch同値だったため重ねなかった |
| `codex3/main`、`claude/for-codex` | 測定・連絡資料 | production差分ではない。binary由来を確定できない値はhard gateにせず、再現可能な条件だけを[性能測定](performance.md)へ残す |
| `suima/perf` | 不採用 | release debug情報の増量と生成flamegraphだけでproduction機能ではない。巨大生成物を版方へ入れない |

## 主な機能系列

| 系列 | 現在の記録 |
|---|---|
| Vaak bridge | [vaak-integration.md](vaak-integration.md) |
| 名前空間 | [NAMESPACE_ROADMAP.md](NAMESPACE_ROADMAP.md) |
| 起動固定費と共通経路のhash | [benchmarks/claude1-startup-and-hash-20260828.md](benchmarks/claude1-startup-and-hash-20260828.md) |
| e-TeX / e-upTeX clean-room互換 | [euptex-port-notes.md](euptex-port-notes.md) |
| PDF直接出力とfont | [pdf-backend-notes.md](pdf-backend-notes.md) |
| TeX Live / kpathsea相当resolver | [kpathsea-port-notes.md](kpathsea-port-notes.md) |
| TeX82外機能と独立実装 | [feature-inventory.md](feature-inventory.md) |
| CJKV regionとscript境界 | [extensible-layout-roadmap.md](extensible-layout-roadmap.md) |
| pTeX相当とJLReq一級日本語組版 | [japanese-typesetting-roadmap.md](japanese-typesetting-roadmap.md) |
| JFM clean-room reader/model | [jfm-port-notes.md](jfm-port-notes.md) |
| e-TeX / TeX--XeT完全性監査 | [etex-texxet-status.md](etex-texxet-status.md) |
| 文字・異体字・造字identity | [glyph-identity-roadmap.md](glyph-identity-roadmap.md) |

## 枝とcommitの規律

- safe Rustの通常作業は目的ごとの`codex3/*`枝で行い、意味を固定する試験と一緒にcommitする。
- 十分に固まったcheckpointは`full`へ順次mergeしてpushする。最低条件はfocused test、全release、
  必要なTRIP/DVI・PDF意味比較、`git diff --check`であり、production未接続の設計だけを完成機能と
  してmergeしない。
- 性能調整を含めPraTeXの通常sourceはsafe Rustだけにする。
- commit messageは日本語で、変更内容の列挙より「なぜその境界が必要か」を書く。
- commit/push前後に`origin/claude/for-codex`を確認し、連絡は`for_CLAUDE.md`へ残す。
- 壊れた名前を含む利用者fileや無関係なworking tree変更は、整理を理由に移動・削除しない。

## 名前空間の印とVaak

名前空間用catcode 16にした文字はTeXの字句化で先に解釈されるため、Vaak本体に現れる
演算子と衝突し得る。例えば`*`は乗算と衝突する。Vaak sourceをTeX token列として読む
現在の経路では、印にはVaakの綴りに現れない文字を選ぶ。

## 権利

rtex/PraTeX側はGPL-3.0であり、基礎実装の権利はtyti氏に帰属する。VaakはMITだが、
PraTeXへ組み込んだ配布物はGPLv3として扱う。GPL側の実装をVaakへ移さない。
pTeX/upTeX/e-upTeXの互換機能は上流sourceを移植せず、公開仕様と許可された
black-box観測から独立して実装する。

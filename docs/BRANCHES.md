# 枝の地図

更新: 2026-08-23

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
                                            └─ codex2/jlreq-script-spacing  現在の作業枝
```

`full`は長く歴史的baselineで止まっていたが、2026-08-23以後は**検証済み機能の統合先**として
再び進める。作業を`full`上で直接行わず、`codex2/*`で意味と試験を固定してから順次mergeする。
現在はe-TeX、PDF、resolver、PraTeX CLI、互換性試験が65 commit先に積まれているため、最初の
更新は小さなcherry-pickを重ねず、全releaseとTRIPを通した統合checkpointへのfast-forwardを使う。

## 現在地

| 枝 | 状態 | 固定した結果 |
|---|---|---|
| `codex/kpse-lsr-index` | push済みbaseline `dc1c554` | release test 455通過、失敗0、4 ignored。bounded `ls-R`索引とWindows--WSL探索境界 |
| `codex/cjkv-region-layout` | 現在の統合枝、R0 `ac6ad90` | typed `LanguageRegion`と`\pratexregion`。release test 466通過、失敗0、4 ignored。TRIPのDVI意味比較差0 |
| `codex/pdf-texlive-type1` | push済み `bb7235f` | TeX Live mapの複数resource、flags既定値、PFB Private `StdVW`。全release失敗0、4 ignored、TRIP 999 recordsで意味差0 |
| `codex/ptex-jfm-core` | push済み `4745f3c` | 公開仕様だけによるbounded JFM reader/model。release 503通過、失敗0、6 ignored。配布JFM 96件とTRIPを通過 |
| `codex/perf-wsl-euptex-safe` | 検証済み `9bb6023` | WSL同士の1.2倍gateを固定。keyword成功経路と最上位整数代入をsafe Rustで短縮。release 507通過、TRIP意味差0 |
| `codex/euptex-integration-resume` | 統合基線 `6ce8315` | e-TeX/pdfTeX、LaTeX、日本語組版、resolver、PDF/Type 1を統合した基線 |
| `codex2/jlreq-script-spacing` | 現在の作業枝 | K/X parameterと、JLReqへ広げられるscript class対spacingの内部境界を実装する |

R0は組版localeの状態、group/global/fmt、表示だけであり、まだJFM、文字間隔、禁則、
font選択、DVI/PDF出力を変えない。R1以降は
[拡張可能なscript境界組版](extensible-layout-roadmap.md)で段階を分ける。

## 主な機能系列

| 系列 | 現在の記録 |
|---|---|
| Vaak bridge | [vaak-integration.md](vaak-integration.md) |
| 名前空間 | [NAMESPACE_ROADMAP.md](NAMESPACE_ROADMAP.md) |
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

- safe Rustの通常作業は目的ごとの`codex2/*`枝で行い、意味を固定する試験と一緒にcommitする。
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

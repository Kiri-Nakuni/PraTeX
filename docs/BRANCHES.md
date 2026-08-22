# 枝の地図

更新: 2026-08-22

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
                         └─ codex/cjkv-region-layout  現在の作業枝
```

`full`を現在の「全部入り」とは呼ばない。以後のe-TeX、PDF、resolver、PraTeX CLI、
互換性試験は`etex-latex`以降で積み上がっている。

## 現在地

| 枝 | 状態 | 固定した結果 |
|---|---|---|
| `codex/kpse-lsr-index` | push済みbaseline `dc1c554` | release test 455通過、失敗0、4 ignored。bounded `ls-R`索引とWindows--WSL探索境界 |
| `codex/cjkv-region-layout` | 現在の統合枝、R0 `ac6ad90` | typed `LanguageRegion`と`\pratexregion`。release test 466通過、失敗0、4 ignored。TRIPのDVI意味比較差0 |

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
| 文字・異体字・造字identity | [glyph-identity-roadmap.md](glyph-identity-roadmap.md) |

## 枝とcommitの規律

- safe Rustの通常作業は目的ごとの`codex/*`枝で行い、意味を固定する試験と一緒にcommitする。
- `unsafe`を試す場合は、safe Rust枝へ混ぜず、名前で判別できる専用枝を先に切る。
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

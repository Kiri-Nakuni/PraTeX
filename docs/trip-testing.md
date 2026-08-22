# TRIP 試験の走らせ方

`tools/run-trip.ps1` は、Knuth の TRIP をリポジトリ外の隔離領域で走らせる。
第三者資材そのものは Git に入れず、実行時に公式 CTAN archive から必要な試験入力と
期待出力だけを取り出す。`tex.web` などの実装ソースは展開しない。

## 権利と出典

- 出典は CTAN の [TeX package](https://ctan.org/pkg/tex) である。
- 同 package のライセンス表示は Knuth License である。
- 実体の取得先は
  `https://mirrors.ctan.org/systems/knuth/dist/tex.zip` である。
- 手順は Donald E. Knuth, *A torture test for TeX*, Appendix A に従う。
  読みやすい PDF は CTAN の
  [tripman.pdf](https://mirrors.ctan.org/info/knuth-pdf/tex/tripman.pdf) にある。
- 取得対象の SHA-256 は `tests-support/trip/assets.json` に固定した。
  上流が変わったときは自動追従せず、出典と差分を人が確認して manifest を更新する。
- TRIP は互換性のブラックボックス試験としてのみ使う。rtex の実装は既存コードと
  公開仕様から書き、TeX／e-(u)pTeX の実装ソースを写さない。

## 一括実行

PowerShell 7 から、リポジトリの根で次を実行する。

```powershell
pwsh -File tools/run-trip.ps1
```

runner は一意な `%TEMP%/rtex-trip-*` を作り、次を順に行う。

1. 公式 archive を取得し、必要な十ファイルの SHA-256 を検証する。
2. 隔離した Cargo target に `cargo build --release --features trip --locked` する。
3. 空行、続いて `\input trip` を端末入力にして INITEX の一段目を走らせる。
4. 先頭・中間・末尾の空白を保った ` &trip  trip ` で二段目を走らせる。
5. log、端末出力、`tripos.tex`、DVI を公式結果と比較する。

最後に表示される作業領域を消さないため、失敗時にも全入力・標準出力・標準エラー・
差分を調べられる。runner 自身は再帰削除をしない。

## 段階ごとの再実行

同じ作業領域を指定すれば、段階を選んで再実行できる。

```powershell
$work = Join-Path $env:TEMP "rtex-trip-investigation"
pwsh -File tools/run-trip.ps1 -WorkRoot $work -Step Fetch,Build,Stage1,Stage2,Compare
pwsh -File tools/run-trip.ps1 -WorkRoot $work -Step Build,Stage1,Stage2,Compare
pwsh -File tools/run-trip.ps1 -WorkRoot $work -Step Compare
```

`WorkRoot` は、第三者資材の誤 commit を避けるためリポジトリ内には置けない。
既存の非空ディレクトリは runner 自身の印がある場合だけ再利用し、無関係な作業物を
上書きしない。
既に取得した公式 archive を使う場合は `-ArchivePath` を指定できる。
既存の rtex を測る場合は `-RtexPath` を指定し、`Build` を省ける。

## 比較結果

主要な成果物は次のとおりである。

| ファイル | 内容 |
|---|---|
| `source-record.json` | archive の出典、取得時刻、archive と各資材の SHA-256 |
| `actual/stage1.json` | 一段目の終了値、入力 byte 列、生成 format の SHA-256 |
| `actual/stage2.json` | 二段目の終了値、入力 byte 列、DVI の SHA-256 |
| `comparison.json` | 各比較の結果と、施した正規化 |
| `normalized/` | 期待値と実測値の最小正規化後テキスト |
| `diff/` | `git diff --no-index` による unified diff |
| `missing-tools.txt` | 実施できなかった TeXware の検査 |

自動正規化は改行、最初の engine／日付 banner、隔離領域の絶対 path だけである。
Knuth が許容している箱の `glue set`、accent kern、容量値、help message、文字列統計、
memory 統計の差は、診断情報を失わないよう自動では消さない。

名前空間拡張がある版では、`\catcode` の許容範囲を示す一箇所の文言差
（`0..15` に対して `0..16`）が既知の候補である。それ以外を推測で許容せず、
`diff/` の実測から一つずつ分類する。

## 外部 TeXware

Appendix A の完全な手順には `PLtoTF`、`TFtoPL`、`DVItype` が要る。

- `pltotf` と `tftopl` が両方 PATH にあれば、`trip.pl → trip.tfm → trip.pl` の往復を行う。
- どちらかが無ければ、SHA-256 を検証した公式 `trip.tfm` で engine 試験を続ける。
- `dvitype` があれば、output level 2、開始 page `*.*.*.*.*.*.*.*.*.*`、
  最大 1000000 page、72.27 dpi、新 magnification 0 で `trip.typ` を作る。
- 無い検査は成功扱いに偽装せず、`missing-tools.txt` と `comparison.json` に残す。

PATH にない実行ファイルは `-PlToTfPath`、`-TfToPlPath`、`-DviTypePath` で明示できる。

## 2026-08-22 の実測基準

`codex/trip-glue-ratio` では、両段とも終了値 0 で完走し、16 page を出力した。
`tripos.tex` は byte 単位で公式結果と一致し、`8terminal.tex` は空である。

公式 DVI は 2920 bytes、rtex は 2924 bytes であり、ファイル全体の hash は一致しない。
公開 DVI 仕様に従って全 record を復号し、次の位置情報だけを比較から外した結果、
公式・rtex とも **999 records**、意味上の差は **0 records** だった。

- preamble の engine comment（公式` TeX output...`の27 bytesに対し、現行
  ` PraTeX output...`は30 bytes）
- comment が3 bytes長いことに伴う BOP / post / post_post のfile pointer
- post_post後の4-byte境界padding（公式4 bytes、PraTeX 5 bytes）

現行枝でも独立decoderで全16 pageのBOP逆参照鎖、postから最終BOP、post_postからpost、
push/popを照合し、両方999 records、意味差0、最大stack深さ17だった。rawのfile sizeは
2920 bytes対2924 bytesのままである。

以前残っていた page 10 の movement `639342177` と page 15 の
`203921756` は、glue ratio を box へ保存するときだけ単精度境界へ揃えることで、
公式 operand `639342208` と `203921760` に一致した。ratio の consumer、glue の累積、
fmt の表現は倍精度のまま変えていない。

log / terminal transcript には、e-TeX拡張レジスタの範囲、追加単位、memory統計を
実装していないこと、入力promptの整形などの診断差が残る。runner はこれらを許容差へ
隠さず `different` として保存するため、DVIの意味一致とlogの未解消差を混同しないこと。

# PraTeX

PraTeXは、tyti氏によるTeX82のRust再実装`rtex`を基礎に、現代的なTeXエンジンを
段階的に作る実験的なプロジェクトです。単体でDVIまたはPDFを出力し、e-upTeXとの
互換性を高めながら、Vaakを明示的に組み込めるエンジンを目標にしています。

まだ実用版でも、e-upTeX・pdfTeX・LuaTeXの代替でもありません。既存の文書をそのまま
処理できるとは限らず、生成物は必ず確認してください。元のrtexのREADMEは
[README_origin.md](README_origin.md) に内容を変えず保存しています。

## 現在できること

- TeX82の中核、formatの生成・読込み、DVI出力
- e-TeXおよびpdfTeXの原始命令の一部
- `-output-format=pdf`による外部DVI driverを必要としないPDF直接出力
- PDFへの暫定的なStandard 14 font出力と、明示したmapによるType 1 font全埋込み。実配布
  `pdftex.map`の複数resource構文、flags既定値、PFB Private `StdVW` fallbackまで読める
- UTF-8入力からのCJK一文字token、`catcode`と`kcatcode`を分離した文字分類基盤
- `\pratexregion=0..5`によるCJKV組版locale状態。group/global/fmt/表示は対応済みだが、
  まだ文字間隔やfont選択には影響しない
- `ls-R`索引と`kpsewhich`の公開CLIを組み合わせたTeX入力、font、mapなどの部分的な探索
- native `kpsewhich`がないWindowsから、resolver内でbackendを混ぜずに既定WSLを使うfallback
- `\directvaak`、`\vaakdef`、`\vaakinput`によるVaakとの実験的な連携

TeX82から増えた機能、部分実装、PraTeX独自機能、既存仕様を独立して書き直した範囲は
[docs/feature-inventory.md](docs/feature-inventory.md) に一覧化しています。
e-TeX／TeX--XeTの完全性監査は
[docs/etex-texxet-status.md](docs/etex-texxet-status.md)、pTeX相当とJLReqの実装順は
[docs/japanese-typesetting-roadmap.md](docs/japanese-typesetting-roadmap.md) にあります。

外部のCTAN/TeX Live 2026資材を一時環境に揃えた実測では、無改変の公式
`latex.ltx`から`latex.fmt`を生成し、最小`article`をDVI/PDFまで処理できました。
DVIは1 page / 392 bytesで、TeX LiveのpdfTeX結果と正規化した`dvitype`命令列の差は0、
直接PDFは1 page / 2169 bytesで構造読込みと描画まで確認しています。これは
一般のclass/package互換性やPDFの字形・抽出互換性を保証するものではありません。
TeX Live 2026の実物`cmr10.pfb`を検証用full-mapで埋め込む試験も1 page / 37,491 bytesで
strict parseとPoppler描画まで通しています。正規mapの`<cmr10.pfb`はsubset指定なので、
subset未実装中は意図的に拒否します。
固定幅の実物`cmtt10.pfb`でも、map flags省略時の`/Flags 4`とPFB由来`/StemV 69`を確認して
おり、AFMからflagsを暗黙に作り直しません。

TRIPではDVIの全999 recordを復号した意味比較が公式結果と一致しています。ただし、
banner、診断、容量、拡張された範囲などのlog差まで解消したという意味ではありません。
比較の条件と残差は [docs/trip-testing.md](docs/trip-testing.md) に記録しています。

## 構築と実行

Rust toolchainを用意し、このリポジトリのルートで実行します。

```console
cargo build --release --bin pratex
cargo run --release --bin pratex -- '&plain' file.tex
```

既定の実行対象も`pratex`です。

```console
cargo run --release -- '&plain' file.tex
```

移行のため、従来名の`rtex` binaryもaliasとして残しています。

```console
cargo run --release --bin rtex -- '&plain' file.tex
```

format、TFM、LaTeX一式、fontは同梱していません。例えば`plain.fmt`を作るには、利用する
`plain.tex`、`hyphen.tex`、TFMをローカルのTeX環境などから用意してください。

### 主な実行option

| option | 意味 |
|---|---|
| `-output-format=dvi` / `--output-format=dvi` | DVIを出力する（既定） |
| `-output-format=pdf` / `--output-format=pdf` | PDFを直接出力する |
| `--pdf-font-map=<map>` | PDF出力でmapを指定し、対応するType 1 fontを埋め込む |
| `--quiet` | banner、page番号、通常の出力要約など、自動的な端末出力を抑える |
| `--` | 以降をPraTeXのoptionとして解釈せず、TeXの入力行へ渡す |

`--quiet`でも、文書が明示した`\message`や`\write16`、エラー、prompt、明示したtracingは
残ります。これはTeX文書の観測可能な出力まで捨てるbatch modeではありません。

PDF backendの現在の範囲と制約は
[docs/pdf-backend-notes.md](docs/pdf-backend-notes.md) を参照してください。
TeX Live探索の対応範囲、WSL境界、性能値は
[docs/kpathsea-port-notes.md](docs/kpathsea-port-notes.md) に記録しています。

## 試験

通常の回帰試験はsafe Rustのrelease buildで走らせます。

```console
cargo test --release
```

PowerShell 7がある環境では、TRIP runnerも実行できます。

```powershell
pwsh -File tools/run-trip.ps1
```

TRIP資材やLaTeX互換性確認用のCTAN資産はrepositoryへvendorしません。必要な試験でだけ
公式配布元から取得し、出典とhashを固定します。`latex.ltx`を含む既存format・class・
packageは実装の資料として写さず、互換性を測る外部入力として扱います。

## 未完成の領域

- e-TeXおよびpdfTeX原始命令の残りと、広範なclass/packageを処理するLaTeX2e互換性
- upTeXの`latin_ucs`、JFM、Unicode文字node、和文glue・禁則・縦組
- OTF／TrueType、CID font、ToUnicode、font subsetを含むPDF font処理
- `texmf.cnf`、全path expression、alias、`mktex*`を含むkpathseaの完全な互換性
- `jsarticle`、`jlreq`、`ltjsarticle`、`hyperref`を実用的に動かすための互換層
- 実行ごとに明示して有効化するVaak callbackと、低頻度で複雑な拡張向けWASM ABI
- 生文字列register、`\therawstring`、TCX legacy input、`^^^^` / `^^^^^^`
- IVS、外字、嘘字/TRON資産、造字を分けて保つ文字identityとfont mapping
- 明示登録で追加できる寸法単位と、script境界spacing/regionのVaak・WASM拡張
- file監視、incremental再実行、不足packageのopt-in取得、実行経路に根拠を持つLSP
- 公式LaTeX sourceを改変せず独立実装するPraTeX用format **LaPraTeX**

長期設計の現在地は次に分けています。これらは「設計がある」ことと
「利用できる」ことを区別してください。

- [script境界組版とCJKV region](docs/extensible-layout-roadmap.md)
- [UTF-8を保つ文字・異体字・造字の内部表現](docs/glyph-identity-roadmap.md)
- [拡張可能な寸法単位](docs/extensible-dimension-units-roadmap.md)
- [監視/incremental実行/package取得/LSP](docs/incremental-tooling-roadmap.md)
- [LaPraTeX](docs/lapratex-roadmap.md)

直接pathは外部探索より常に優先します。索引から探索順を一意に証明できない場合は
one-shot `kpsewhich`へ戻ります。外部programを起動できない場合、TeX入力などは従来の
ローカル探索へ戻り、PDF font資材などは探索errorとして報告します。
Windowsではnative `kpsewhich`の起動fileがない時だけ既定WSLを選び、各resolver instance内で
backendを混ぜません。現在はScannerとPDF資材loaderが別instanceなので、run-globalに一つの
TeX Liveを固定するところまでは未実装です。完全なKpathsea互換も保証しません。
PDF出力も現段階では文字の見た目やtext extractionまでpdfTeX相当ではありません。

## 実装方針と権利

PraTeXはGPL-3.0で配布します。基礎となるrtexの権利はtyti氏に帰属します。VaakはMITで
配布されていますが、PraTeXへ組み込んだ全体はGPLv3として扱います。GPLのrtex側から
Vaakへコードを移すことはしません。

実装は原則としてsafe Rustだけを使います。頻出経路の性能を測り、出力とTRIPの結果を
固定したうえで最適化します。

e-TeX、pTeX、upTeX、pdfTeX由来の拡張は、上流の実装コードを移植せず、公開仕様と
許可された黒箱観測から書き直します。特にpTeX／upTeX系は由来ごとにライセンスが異なる
ため、一括してBSDとみなしません。境界と根拠は
[docs/euptex-port-notes.md](docs/euptex-port-notes.md) および各移植ノートに記録します。


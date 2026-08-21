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
- PDFへの暫定的なStandard 14 font出力と、明示したmapによるType 1 font全埋込み
- UTF-8入力からのCJK一文字token、`catcode`と`kcatcode`を分離した文字分類基盤
- `kpsewhich`の公開CLIを利用したTeX入力、font、mapなどの部分的な探索
- `\directvaak`、`\vaakdef`、`\vaakinput`によるVaakとの実験的な連携

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

- e-TeXおよびpdfTeX原始命令の残りと、現代のLaTeX2eを最後まで処理する互換性
- upTeXの`latin_ucs`、JFM、Unicode文字node、和文glue・禁則・縦組
- OTF／TrueType、CID font、ToUnicode、font subsetを含むPDF font処理
- kpathseaの`ls-R`、設定、探索順を含む完全な互換性
- `jsarticle`、`jlreq`、`ltjsarticle`、`hyperref`を実用的に動かすための互換層
- 実行ごとに明示して有効化するVaak callbackと、低頻度で複雑な拡張向けWASM ABI

`kpsewhich`が利用できない環境でも従来のローカル探索へ戻りますが、TeX Liveと同じ探索を
保証するものではありません。PDF出力も現段階では文字の見た目やtext extractionまで
pdfTeX相当ではありません。

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


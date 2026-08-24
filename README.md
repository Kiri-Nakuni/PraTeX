# PraTeX

PraTeXは、tyti氏によるTeX82のRust再実装`rtex`を基礎に、現代的なTeXエンジンを
段階的に作る実験的なプロジェクトです。単体でDVIまたはPDFを出力し、e-upTeXとの
互換性を高めながら、Vaakを明示的に組み込めるエンジンを目標にしています。
学術論文だけでなく、日本語の小説・エッセイ・文芸書を第一級用途にし、縦組、ルビ、
縦中横、割注、版面、柱・ノンブル、入稿PDFまでをengine・package・toolingの連続した機能として
扱う方針です。
和中混植も主要目標です。同じHan Unicode scalarを日本語・簡体字中国語・繁体字中国語の
言語区間ごとに自然な地域字形へshapeし、Han unificationの影響を見た目へ出さない一方、元の
文字identityとPDF `ToUnicode`は保ちます。現在の`\pratexregion`はそのためのR0状態だけで、
font routing、OpenType language/`locl`、region別fallback、約物・禁則への接続は未実装です。

まだ実用版でも、e-upTeX・pdfTeX・LuaTeXの代替でもありません。既存の文書をそのまま
処理できるとは限らず、生成物は必ず確認してください。元のrtexのREADMEは
[README_origin.md](README_origin.md) に内容を変えず保存しています。

## 現在できること

- TeX82の中核、formatの生成・読込み、DVI出力
- e-TeXおよびpdfTeXの原始命令の一部。e-TeXの`\scantokens`は一時fileを使わない
  typed疑似入力として、動的catcode再走査、`\everyeof`、行番号、fmtまで接続済み
- 他engineの版番号へ偽装しないPraTeX固有の識別子。開発中は
  `\pratexversion=0`、`\pratexrevision=0.1.0-dev`で、完成前に版1を名乗らない
- `-output-format=pdf`による外部DVI driverを必要としないPDF直接出力
- PDFへの暫定的なStandard 14 font出力と、明示したmapによるType 1 font全埋込み。実配布
  `pdftex.map`の複数resource構文、flags既定値、PFB Private `StdVW` fallbackまで読める
- 明示した一つのprofileで、横組JFMのBMP wide glyphを非埋込みType 0/CIDFontType0と
  `UniJIS-UCS2-H`へ出す最小PDF基線。BMP source code用の`/ToUnicode`を持つが、字形は
  埋め込まず、表示はviewer側fontに依存する
- UTF-8入力からのCJK一文字token。`catcode`側をカノンとし、`kcatcode`の公開番号を
  互換viewとして意味へ写す文字分類基盤
- `\kanjiskip` / `\xkanjiskip`の通常glue parameter面、`\autospacing` / `\autoxspacing`の
  switch、`\xspcode`、`\inhibitxspcode`と、横組の和和・和欧・欧和境界へ一度だけ挿入する
  BuiltIn最小finalizer
- boundedな横組JFMを読む`\pratexjfont`（横組定義・選択だけ`\jfont` alias）、current和文font、
  `zw`/`zh`、Unicode/JFM class付きwide node、class対glue/kern、DVI `set2`/`set3`の最小基線
- `\pratexregion=0..5`によるCJKV組版locale状態。group/global/fmt/表示は対応済みだが、
  まだ文字間隔やfont選択には影響しない
- `ls-R`索引と`kpsewhich`の公開CLIを組み合わせたTeX入力、font、mapなどの部分的な探索
- native `kpsewhich`がないWindowsから、resolver内でbackendを混ぜずに既定WSLを使うfallback
- `\directvaak`、`\vaakdef`、`\vaakinput`によるVaakとの実験的な連携。静的な失敗は
  parse/check/type-checkの段階、行・桁、Vaak側の理由をTeXの診断へ残す

plain formatでVaakを使う最小例は
[examples/plain-vaak.tex](examples/plain-vaak.tex) にあります。`\vaakdef`と`\directvaak`、
局所束縛の`let`/`var`、TeX registerをVaakのhost配列へ明示的に別名束縛する例を一つの
文書で試せます。この配布例そのものを回帰試験から読み、説明と実装がずれないようにしています。

TeX82から増えた機能、部分実装、PraTeX独自機能、既存仕様を独立して書き直した範囲は
[docs/feature-inventory.md](docs/feature-inventory.md) に一覧化しています。
e-TeX／TeX--XeTの完全性監査は
[docs/etex-texxet-status.md](docs/etex-texxet-status.md)、pTeX相当とJLReqの実装順は
[docs/japanese-typesetting-roadmap.md](docs/japanese-typesetting-roadmap.md) にあります。
Vaak/WASMを明示登録時だけ有効にする内部境界は
[docs/vaak-embedding-api-design.md](docs/vaak-embedding-api-design.md)、upLaTeXとのDVI性能gateは
[docs/performance.md](docs/performance.md) にあります。どちらも未実装部分を含む設計・測定記録です。
担当やセッションを交代する場合の現在枝、未commit差分、検証手順は
[docs/HANDOFF.md](docs/HANDOFF.md) にまとめています。
正式版1の完了条件と、それ以後にリウヴィル定数の桁へ収束する版規則は
[docs/versioning.md](docs/versioning.md) に固定しています。

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

公式KOMA-Script 3.49.2、Babel 26.9、hyph-utf8の英語patternによる標準英語構成を使った
最小`scrartcl`も、無改変のclassでexit 0、log error 0、1 page / 332 bytesのDVIまで確認して
います。検証用`language.dat`だけは英語と三つのaliasを列挙して生成したものです。さらに
明示的な`pratex-japanese` packageを読み込む和欧混植例は、同じ公式runtimeでerror 0、
1 page / 588 bytesのDVIまで実測しています。
`\scantokens`を持たない旧binaryではclass読込み中に未定義7件から77 errorsへ連鎖していました。

同じPraTeX生成`latex.fmt`での主要package実測は
[docs/package-compatibility.md](docs/package-compatibility.md)に固定しています。2026-08-24時点では
`article`、`scrartcl`、`graphicx`、`xcolor`、`hyperref`、TikZ/PGF、`siunitx`、
代表`prjsarticle`が最小DVIへ到達しています。`pxrubrica`はgeneric fallback smokeだけです。
これは各packageの限定入力であり、全APIや実用文書の互換性を保証しません。

`\scantokens` code checkpoint直前に得たTRIP DVIのhashは、独立decoderで全999 recordの
意味差0を確認した既知正常値と一致しています。ただし、
banner、診断、容量、拡張された範囲などのlog差まで解消したという意味ではありません。
比較の条件と残差は [docs/trip-testing.md](docs/trip-testing.md) に記録しています。

plain formatの欧文DVIは、`origin/main`のrTeXとBOPからEOPまでのpage bodyをbyte単位で
比較しています。現在の固定fixtureは183 bytesで差分0です。LaTeXについてはengine依存部分を
PraTeX用に独立実装している途中なので、同じ完全回帰をまだ主張しません。

## 構築と実行

Rust toolchainと通常のTeX Liveを用意し、`cargo`、`kpsewhich`、`dvipdfmx`へPATHを通します。
PraTeXとVaakは同じ親directoryへ置いてください。`Cargo.toml`は兄弟directoryの`../vaak`を
参照します。このリポジトリのルートで次を実行します。

```console
cargo build --release --locked --bin pratex
cargo run --release --locked --bin pratex -- --quiet -ini -- plain.tex '\dump'
cargo run --release --locked --bin pratex -- --quiet -- '&plain' file.tex
```

二行目がTeX Liveの`plain.tex`を探索し、同じPraTeX binary用の`plain.fmt`を現在directoryへ
生成します。TeX Liveが配布する別engine用の`plain.fmt`をそのまま読み込ませないでください。

既定の実行対象も`pratex`です。

```console
cargo run --release --locked -- '&plain' file.tex
```

移行のため、従来名の`rtex` binaryもaliasとして残しています。

```console
cargo run --release --locked --bin rtex -- '&plain' file.tex
```

TeX Live側のformat、TFM/JFM、標準LaTeX class・package、fontは同梱していません。
PraTeX固有の`prjsarticle.cls`、`pratex-japanese.sty`と実行例はrepositoryにあります。
Linuxの既定buildは、公式TeX Live 2026の固定revisionからKpathsea 6.4.2を静的に組み込み、
通常のTEX/TFM/JFM等をin-processで探索します。初回buildには`git`とnetwork、または同じ
`texk/kpathsea` treeを指す`KPATHSEA_SRC_DIR`が必要です。配布側のKpathsea development libraryを
使う場合は`--no-default-features --features stats,system-kpathsea`を明示できます。
Windows等の未接続targetではTeX Liveの`ls-R`と`kpsewhich`を段階的に利用します。
primitiveを追加・変更したPraTeXで古いformatを
使わず、同じbinaryで作り直してください。現在format探索はlocal優先なので、生成した
`latex.fmt`と文書は同じ作業directoryへ置くのが確実です。

### 日本語横組みを実際に試す

現在の通常経路は、TeX Liveの本文用`upjisr-h.tfm`と見出し用`upjisg-h.tfm`を使うDVI出力です。次のPowerShell例は
PraTeX用`prjsarticle`、`pratex-japanese`、実行例を一時作業directoryへ揃え、現在のbinaryで
`latex.fmt`を生成してから和欧混植文書を処理します。

```powershell
# 必要なTeX Live資材を先に確認する。
kpsewhich latex.ltx
kpsewhich hyphen.cfg
kpsewhich upjisr-h.tfm
kpsewhich upjisr-h.vf
kpsewhich upjisg-h.tfm
kpsewhich upjisg-h.vf
kpsewhich tcrm1000.tfm

$repo = (Resolve-Path .).Path
$pratex = (Resolve-Path target\release\pratex.exe).Path
$demo = Join-Path ([IO.Path]::GetTempPath()) `
  ("pratex-demo-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory $demo | Out-Null

Copy-Item -LiteralPath (Join-Path $repo 'tex\latex\pratex\prjsarticle.cls') `
  -Destination $demo -Force
Copy-Item -LiteralPath (Join-Path $repo 'tex\latex\pratex\pratex-japanese.sty') `
  -Destination $demo -Force
Copy-Item -LiteralPath (Join-Path $repo 'docs\examples\prjsarticle-sample.tex') `
  -Destination $demo -Force

Push-Location $demo
try {
  # 同じbinaryでformatを作る。latex.fmtがこのdirectoryへ生成される。
  & $pratex --quiet -- latex.ltx
  if ($LASTEXITCODE -ne 0) { throw 'latex.fmt generation failed' }

  # 日本語横組DVI。通常はこちらを組版確認の基準にする。
  & $pratex --quiet -- '&latex' prjsarticle-sample.tex
  if ($LASTEXITCODE -ne 0) { throw 'PraTeX DVI generation failed' }
  dvipdfmx -o prjsarticle-sample-from-dvi.pdf prjsarticle-sample.dvi
  if ($LASTEXITCODE -ne 0) { throw 'dvipdfmx failed' }

  # 実験的な直接PDF。制約は直後の説明を参照する。
  & $pratex --quiet --output-format=pdf -- '&latex' prjsarticle-sample.tex
  if ($LASTEXITCODE -ne 0) { throw 'PraTeX direct PDF generation failed' }
} finally {
  Pop-Location
}
```

Linux/macOSでは同じ確認を次のように実行できます。`pratex`は絶対pathにしてから一時
directoryへ移ります。

```sh
kpsewhich latex.ltx
kpsewhich hyphen.cfg
kpsewhich upjisr-h.tfm
kpsewhich upjisr-h.vf
kpsewhich upjisg-h.tfm
kpsewhich upjisg-h.vf
kpsewhich tcrm1000.tfm

cargo build --release --locked --bin pratex
repo=$(pwd)
pratex="$repo/target/release/pratex"
demo=$(mktemp -d "${TMPDIR:-/tmp}/pratex-demo.XXXXXXXX")
cp "$repo/tex/latex/pratex/prjsarticle.cls" "$demo/"
cp "$repo/tex/latex/pratex/pratex-japanese.sty" "$demo/"
cp "$repo/docs/examples/prjsarticle-sample.tex" "$demo/"

cd "$demo"
"$pratex" --quiet -- latex.ltx
"$pratex" --quiet -- '&latex' prjsarticle-sample.tex
dvipdfmx -o prjsarticle-sample-from-dvi.pdf prjsarticle-sample.dvi
```

`prjsarticle`はhookを定義した後で`pratex-japanese`を読みます。このpackageが
`\pratexjfont`で`upjisr-h at 10pt`を定義し、LaTeXの`selectfont` hookから選択中の横組JFMを
同期します。classの本文・表題・見出しroleはrelation要求をその場で消費してから`\selectfont`を
呼ぶため、文書開始時の要求が最初の`\sffamily`などへ漏れません。
別JFMを試す場合だけ、`docs/examples/prjsarticle-upjisr-h-adapter.tex`と同じ形で
`\pratexsetjapanesefonthook`を後から上書きできます。
直接PDFでは、既定JFM `upjisr-h`を内蔵CID profileへ結ぶため追加optionは要りません。
別JFMだけは`--pdf-japanese-cid-profile=PATH`で対応するprofileを明示してください。
現段階の直接PDFは和文字形を埋め込まず、`HeiseiMin-W3`と`UniJIS-UCS2-H`を解決できる
viewerに依存します。BMP source codeを元のUnicodeへ戻す`/ToUnicode`は持ち、pypdfと
PDFiumで抽出を確認していますが、字形表示の可搬性や全extractorとの互換性はまだありません。
Viewer固有の非表示とDVI/PDF生成不良を混同しないため、可搬な表示確認にはDVIとTeX Liveの
`dvipdfmx`も使ってください。

直接PDFはDvips互換の`\special{papersize=幅,高さ}`を認識します。`in`、`cm`、`mm`、`pt`、
`sp`、`bp`、`pc`、`dd`、`cc`を使え、最初のpage内で最後に指定した正の寸法を後続pageにも
継承します。物理用紙寸法へ`\mag`は掛けません。認識した形式の欠損・未知単位・第二page
以降での変更はPDF出力errorになり、それ以外のraw specialはPDF contentへ注入せず捨てます。

文書またはformatが和文fontを明示選択していない場合も、PraTeXは最初のCJK文字でだけ
`upjisr-h at 10pt`を遅延して選びます。これはplainや一般classで「TFMはあるのにcurrent
和文fontがない」状態を避けるための既定値です。カレントの`upjisr-h.tfm`を最優先し、なければ
同じTFM resolverでTeX Liveを探索します。`\pratexjfont`による明示選択が常に優先されます。
英文だけの実行ではJFMを探索しません。VFはPraTeXが先読みする資材ではなく、DVIを処理する
`dvipdfmx`側が`upjisr-h.vf`をkpathseaで探索します。

別のTFM/JFMを明示する場合、和文にはPraTeX固有の`\pratexjfont`、欧文にはTeXの`\font`を
使います。論理名には通常`.tfm`を付けません。定義した制御綴を実行した時点からcurrent fontに
なります。

```tex
% 和文JFM。カレントの metrics/my-jfm.tfm、次いでTeX Liveを探索する。
\pratexjfont\MyJapanese=metrics/my-jfm at 12pt
\MyJapanese

% 欧文TFM。
\font\MyLatin=cmr10 at 10pt
\MyLatin
```

`prjsarticle`で本文開始時の選択も差し替える場合は、class読込み後に次を置きます。

```tex
\pratexjfont\MyJapanese=metrics/my-jfm at 12pt
\pratexsetjapanesefonthook{\MyJapanese}
```

### `scrartcl`の最小確認

同じPowerShell sessionを続け、KOMA-ScriptもTeX Live側にあることを確認して、上で生成した
同じ`latex.fmt`を使います。この
最小例は上記の公式assetによる標準英語構成で、KOMA-Script 3.49.2のerror 0 DVIまで実測して
います。

```powershell
kpsewhich scrartcl.cls
kpsewhich keyval.sty
kpsewhich upjisr-h.tfm
kpsewhich upjisr-h.vf
Copy-Item -LiteralPath (Join-Path $repo 'docs\examples\scrartcl-minimal.tex') `
  -Destination $demo -Force
Copy-Item -LiteralPath (Join-Path $repo 'tex\latex\pratex\pratex-japanese.sty') `
  -Destination $demo -Force

Push-Location $demo
try {
  & $pratex --quiet -- '&latex' scrartcl-minimal.tex
  if ($LASTEXITCODE -ne 0) { throw 'scrartcl compilation failed' }
} finally {
  Pop-Location
}
```

`pratex-japanese`は一般classをPraTeX上で横組JFMへ接続する明示packageです。和文側にも
encoding / family / series / shapeの独立した属性を持ち、宣言したJFMをNFSSの現在sizeへ
exact spで追随させます。同じJFMとsizeはcacheを共有し、group終了後は外側の和文属性へ戻ります。
また、和文属性から欧文NFSS属性を選ぶPraTeX固有のrelation font APIを持ちます。これは標準NFSS
本体の機能ではなく、pLaTeXがNFSS上へ加えた「従属書体」の意味をPraTeX固有名で提供するものです。
`prjsarticle`の本文・表題・見出しはこの宣言面を使い、和文hookと欧文hookを手続き的に並べません。
publicな`\UsePraTeXRelationFont`はdocument body用で、preamble中に発行して次のpre-document
`\selectfont`へ保留する使い方は未対応です。現段階のJFM宣言は横組exact shapeだけで、NFSSの
size function、shape substitution、縦組font選択は未実装です。公開APIと制約は
[prjsarticleの設計](docs/prjsarticle.md)を参照してください。
`upjisr-h.tfm`が探索できない場合は、文書中のCJK文字を処理する前にpackage読込み位置で
`JFM file was not found`と診断します。

TeX Liveの標準`hyphen.cfg`を使ってformatを生成してください。試験専用の空の
`tests/fixtures/prjsarticle/hyphen.cfg`はlanguage patternを意図的に持たないため、一般の
KOMA-Script確認には使えません。
終了code、log error、非空DVIまで自動検査する自己完結runnerは
`pwsh -File tools/test-scrartcl.ps1 -PraTeXPath target/release/pratex.exe`です。

### 主な実行option

| option | 意味 |
|---|---|
| `--help` | engineを起動せず、実装済みoptionのusageを表示する |
| `--version` | engineを起動せず、release gate未達を反映した開発版bannerを表示する |
| `-fmt=<name>` | `<name>.fmt`を読む。command line先頭の`&fmt`があればそちらを優先する |
| `-ini` | initial engineを選び、format生成を可能にする |
| `-interaction=<mode>` | `batchmode`, `nonstopmode`, `scrollmode`, `errorstopmode`から選ぶ |
| `-halt-on-error` | 最初のTeX errorで回復を打ち切り、失敗終了する |
| `-jobname=<name>` | `\jobname`とlog、DVI/PDF、fmtのbasenameを同じ明示値にする |
| `-output-comment=<text>` | DVI preamble commentを指定する。直接PDFでは受理するが使わない |
| `-no-shell-escape` | shell実行が無効であることを明示する。正方向は未実装errorになる |
| `-no-mktex=tex|tfm` | tex/tfm自動生成が無効であることを明示する。正方向は未実装errorになる |
| `-output-format=dvi` / `--output-format=dvi` | DVIを出力する（既定） |
| `-output-format=pdf` / `--output-format=pdf` | PDFを直接出力する |
| `--pdf-font-map=<map>` | PDF出力でmapを指定し、対応するType 1 fontを埋め込む |
| `--pdf-japanese-cid-profile=<path>` | PDF出力で内蔵`upjisr-h` profileを上書きし、一つのJFMを明示named CID fontへ結ぶ |
| `--quiet` | banner、page番号、通常の出力要約など、自動的な端末出力を抑える |
| `--` | 以降をPraTeXのoptionとして解釈せず、TeXの入力行へ渡す |

`--quiet`でも、文書が明示した`\message`や`\write16`、エラー、prompt、明示したtracingは
残ります。これはTeX文書の観測可能な出力まで捨てるbatch modeではありません。
値の空白分離、未知optionの扱い、`--`境界、未実装Web2C optionを含む正確な対応表は
[docs/cli-options.md](docs/cli-options.md)を参照してください。

PDF backendの現在の範囲と制約は
[docs/pdf-backend-notes.md](docs/pdf-backend-notes.md) を参照してください。
JFM/TFMには文字幅やclassはあっても、outline、bitmap、CID対応表はありません。このため
既定`upjisr-h` profileと`--pdf-japanese-cid-profile`の経路は字形を埋め込まず、指定したBaseFontと
`UniJIS-UCS2-H`を実装するviewer環境でだけ意図した和文表示になります。portableな字形表示や
全extractor互換を保証する機能ではありません。限定BMP経路には`/ToUnicode`がありますが、
内蔵profileのないJFMをtofuへ黙ってfallbackすることもありません。
TeX Live探索の対応範囲、WSL境界、性能値は
[docs/kpathsea-port-notes.md](docs/kpathsea-port-notes.md) に記録しています。

## 試験

通常の回帰試験はsafe Rustのrelease buildで走らせます。

```console
cargo test --release --locked --no-fail-fast
```

PowerShell 7がある環境では、TRIP runnerも実行できます。

```powershell
pwsh -File tools/run-trip.ps1
```

TeX Liveの標準言語設定で`latex.fmt`を作り直し、実物のKOMA-Scriptをerror 0まで検査する
smoke runnerは次です。生成物はrepository外の一意な作業directoryへ置きます。

```powershell
pwsh -File tools/test-scrartcl.ps1 -PraTeXPath target/release/pratex.exe
```

主要class/packageを同じfmtで再測定するrunnerは、公式runtime資材を平坦化したrepository外の
directoryを明示して実行します。既知blockerが別の失敗へ変わった場合も失敗になります。

```powershell
pwsh -File tools/test-package-compat.ps1 `
  -PraTeXPath target/release/pratex.exe `
  -RuntimeRoot C:\path\to\flat-ctan-runtime
```

TRIP資材やLaTeX互換性確認用のCTAN資産はrepositoryへvendorしません。必要な試験でだけ
公式配布元から取得し、出典とhashを固定します。`latex.ltx`を含む既存format・class・
packageは実装の資料として写さず、互換性を測る外部入力として扱います。

## 未完成の領域

- e-TeXおよびpdfTeX原始命令の残りと、広範なclass/packageを処理するLaTeX2e互換性
- `\tfont`と縦組JFM、main-loop JFM/禁則のbox/disc・未検証command境界、
  現在の句読点と横組括弧12対を越えるJLReq禁則、縦組PDF和文glyph
- 埋込み和文font、OTF／TrueType、ToUnicode、font subsetを含むportableなPDF font処理と、
  PraTeX-nativeな`fontspec`相当・和文OTF package相当のfont選択層
- 通常OTF対応後のnative絵文字。plain UTF-8のemoji sequenceをcluster単位でshape・fallbackし、
  color font描画と元Unicode列のPDF抽出まで扱う。現在はroadmapのみ
- `texmf.cnf`、全path expression、alias、`mktex*`を含むkpathseaの完全な互換性
- `jsarticle`、`jlreq`、`ltjsarticle`、`hyperref`を実用的に動かすための互換層
- 実行ごとに明示して有効化するVaak callbackと、低頻度で複雑な拡張向けWASM ABI
- 生文字列registerのliteral/file producerと`\the\rawstring`改行契約、TCX legacy input、`^^^^` / `^^^^^^`
- IVS、外字、嘘字/TRON資産、造字を分けて保つ文字identityとfont mapping
- 明示登録で追加できる寸法単位と、script境界spacing/regionのVaak・WASM拡張
- file監視、incremental再実行、不足packageのopt-in取得、実行経路に根拠を持つLSP
- 公式LaTeX sourceを改変せず独立実装するPraTeX用format **LaPraTeX**

長期設計の現在地は次に分けています。これらは「設計がある」ことと
「利用できる」ことを区別してください。

- [script境界組版とCJKV region](docs/extensible-layout-roadmap.md)
- [UTF-8を保つ文字・異体字・造字の内部表現](docs/glyph-identity-roadmap.md)
- [拡張可能な寸法単位](docs/extensible-dimension-units-roadmap.md)
- [各地域・組版文化の文字サイズ単位調査](docs/international-typographic-units.md)
- [明示登録Vaak phaseと低頻度WASM bulk](docs/vaak-embedding-api-design.md)
- [外向きWASM provider ABI 0.0](docs/wasm-provider-abi-v0.md)
- [WASM module import・名前空間仕様 0.1](docs/wasm-module-import-v0.1.md)
- [監視/incremental実行/package取得/LSP](docs/incremental-tooling-roadmap.md)
- [PraTeX-native OpenType packageと文字別font routing](docs/opentype-package-roadmap.md)
- [OTF完成後のnative絵文字](docs/emoji-native-roadmap.md)
- [日本語の論文・小説・出版実務調査](docs/research/japanese-publishing/README.md)
- [多言語・混植組版調査](docs/research/multilingual-typesetting/README.md)
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

実装と性能調整はsafe Rustだけを使います。頻出経路を測り、出力とTRIPの結果を固定したうえで
最適化します。DVI modeでは同一入力・同一TeX tree・同等DVIでupLaTeXの実行時間の
1.2倍未満を最終的なhard gateとし、探索・fmt復元・組版・出力も分けて追跡します。
`22a8bdd` / `3a4aaaf`のfmt予約checkpointはWindowsのwarm内部A/Bでwallを7.31--15.26%
短縮しましたが、利用者のLinux TeX Live文書で観測されたPraTeX 9.14 sを再測定した結果ではなく、
end-to-end差の解消を意味しません。48標本のraw値は
[docs/benchmarks/fmt-bounded-reservation-20260824.csv](docs/benchmarks/fmt-bounded-reservation-20260824.csv)、
条件と限界は[性能測定](docs/performance.md)にあります。

e-TeX、pTeX、upTeX、pdfTeX由来の拡張は、上流の実装コードを移植せず、公開仕様と
許可された黒箱観測から書き直します。特にpTeX／upTeX系は由来ごとにライセンスが異なる
ため、一括してBSDとみなしません。境界と根拠は
[docs/euptex-port-notes.md](docs/euptex-port-notes.md) および各移植ノートに記録します。


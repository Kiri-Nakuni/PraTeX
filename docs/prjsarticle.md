# `prjsarticle` — PraTeX native横組class

`tex/latex/pratex/prjsarticle.cls`は、PraTeXの日本語glyph/JFM枝へ接続するための
小規模なLaTeX article classである。A4、約40字相当の行長、一字下げ、広めの行送り、
固定高さのtitle block、見出し、基本list、日本語font hookを持つ。縦組、割注、縦中横は
このclassの初版範囲ではない。

## engine identityとfont境界

classは`\pratexversion` primitiveの存在を必須とする。他engineの`\pdftexversion`、
`\luatexversion`、`\XeTeXversion`、pTeX/upTeX判定用primitiveを定義・参照して
package分岐を偽装しない。初版engine identityとして次をPraTeX coreへ要求する。

- `\pratexversion`: read-only integer。開発中は0で、完成条件を満たした正式版から1。
- `\pratexrevision`: expandableな版文字列。現在は`0.1.0-dev`。末尾の零を捨てない。
- 将来の個別能力には、`\pratexfeature{japanese-horizontal-glyph-dvi}`が0または
  契約versionを返す低頻度queryを推奨する。

classは次のPraTeX固有hookを定義した後、公式LaTeX互換track用の
`pratex-japanese` packageを読み込む。このpackageはTeX Liveの`upjisr-h at 10pt`を
既定fontとして登録する。formatへJFMを埋め込まず、LaPraTeXを名乗らない。

- `\pratexsetjapanesefonthook{...}`
- `\pratexsetlatinfonthook{...}`
- `\pratexjapanesefonthook` / `\pratexlatinfonthook`

これらはengine identityを作るAPIではない。pTeX互換名をclass側で補うadapterも置かない。
文書が後から`\pratexsetjapanesefonthook`を呼べば、その明示adapterが既定hookへ後勝ちする。

初版packageはNFSSと和文font sizeを連動しない。`\small`、`\normalsize`、`\Large`で
Latin font sizeが変わっても、和文JFMは10ptのままである。公式CTAN runtimeで「日本」の
box幅が三つとも20.0ptになることを実測している。family/series、任意JFM、縦組を含む
NFSS接続は後続sliceとする。

`upjisr-h.tfm`を置かない隔離runtimeでは、package読込み位置で先に
`Japanese font ...=upjisr-h at 10.0pt not loaded: JFM file was not found`と診断することも
実測した。これはCJK文字へ到達後の`CJK typesetting needs a Japanese font metric`とは別で、
資材探索失敗とcurrent和文font未選択を混同しない。

## production出力までのengine依存

2026-08-23の統合枝では、横組JFM/TFMからwide glyph node、JFM/K/X finalizer、DVI
`set2` / `set3`、明示profileによる非埋込みnamed CID PDFまで接続済みである。classを通常の
和欧混植文書として使うには、現在は次の境界が残る。

1. 固定10pt packageをNFSSのsize/family/seriesと任意JFMへ一般化すること。
2. box/disc境界とmain-loop早期spacingを完成すること。
3. 4文字subsetを越えるJLReq禁則、和文widow処理、縦組を加えること。
4. PDFへ和文字形を埋め、ToUnicodeとfont subsetを持つportableな出力にすること。

標準日本語組版をVaak/WASM callbackへ逃がさない。classのfont hookは組版判断を行わず、
engineが所有するfont/glyph境界を選択するだけである。

## `\maketitle`回帰方針

LaTeX本家との完全DVI一致はLaPraTeX format完成まで要求しない。`prjsarticle`は固定高さの
title/author/date rowを持ち、`tests/fixtures/prjsarticle/maketitle-oracle.tex`の固有ruleを
PraTeX自身のknown semantic oracleとして使う。試験はDVIを公開仕様どおり復号し、title ruleの
sp座標、中央揃え、body開始位置、page数を固定する。plain欧文のorigin/main rTeX完全回帰とは
別gateである。

oracleではtitle、author、date、bodyへそれぞれ`17sp x 23sp`、`19sp x 29sp`、
`21sp x 31sp`、`31sp x 37sp`の固有primitive ruleを置く。10pt・40emの本文を基準に、
`c9bd240`統合後のPraTeXで実測した次のclass固有相対座標をexactに検査し、`±1sp`の
許容は置かない。理想中心へ後から丸め直した値ではない。

| marker | `h - body.h` | `body.v - v` |
|---|---:|---:|
| title | 13,107,212sp | 8,110,068sp |
| author | 13,107,211sp | 5,160,945sp |
| date | 13,107,210sp | 3,194,864sp |

DVIだけでは`set_char`が進めるTFM幅を復元できない。そのためfixtureは四つのmarkerより前に
glyphを置かず、decoderはglyphが先行した場合に推測をせず失敗する。

## asset-fetching runner

`tools/test-prjsarticle.ps1`は公式LaTeX互換track専用である。TeX Live、`kpsewhich`、
jsclassesを子processとして呼ばず、`tests-support/prjsarticle/assets.json`で固定したarchiveから
runtimeの`.tex`/`.tfm`だけをrepository外へ展開する。`latex.ltx`はopaqueな試験入力として
実行し、class実装の生成材料にはしない。

```powershell
pwsh -File tools/test-prjsarticle.ps1 `
  -Fetch `
  -AssetCache C:\temp\prjsarticle-assets `
  -RtexPath target\release\pratex.exe
```

初回だけ`-Fetch`を明示する。offlineではcache欠落、hash不一致、異なるruntime fileの
basename衝突をすべてfailにする。生成した`latex.fmt`、DVI、stdout/stderr、取得記録は
一意な`WorkRoot`だけへ置く。基点engineでは公式format生成まで通り、`\pratexversion`が
未実装だったためclass入口で意図どおり停止した。`aa48367`のidentity枝をmergeした後は
class入口とcompileを通り、1 page / 352 bytesのDVIを生成した。意味座標は上表と一致した。
ignored testは次で有効化する。

```powershell
$env:PRATEX_PRJSARTICLE_ASSET_CACHE = 'C:\temp\prjsarticle-assets'
cargo test --release --locked --test prjsarticle -- --ignored --nocapture
```

日本語glyph/JFM枝を取り込んだ後は、classが`pratex-japanese`を読み、代表和欧混植sampleも
追加adapter無しで同じrunnerからcompileする。

```powershell
pwsh -File tools/test-prjsarticle.ps1 `
  -AssetCache C:\temp\prjsarticle-assets `
  -RtexPath target\release\pratex.exe `
  -CompileSample
```

別JFMを試す時だけ、明示したadapterを`prjsarticle-test-adapter.tex`という試験時の名前で
隔離rootへコピーする。adapterはclassや公式LaTeX sourceをpatchせず、他engine identityも
定義しない。`-CompileSample`だけなら既定packageを使い、`-JapaneseAdapterPath`を指定した
場合だけhookを後勝ちで置き換える。通常利用者がTeX Liveで試す手順は
[README](../README.md)にある。

2026-08-23に上の自己完結経路を公式CTAN資材だけで再実行し、format、title oracle、実時刻date、
代表和欧混植sampleの全logがerror 0になった。title DVIは352 bytes、代表sample DVIは
2356 bytesで、sampleのSHA-256は
`df5b9ff7f1510777f52a15fb5cc6b068e46441475b9ac75c47382a4c02920dce`である。runnerは終了codeだけで
成功扱いせず、各logの`!`行と空のfmt/DVIも拒否する。

固定資材は次のとおり。archiveそのものはrepositoryへ入れない。

| package | version/snapshot | archive SHA-256 | license |
|---|---|---|---|
| latex-base | 2026-06-01 | `424bcbab851723495397f0542db8722a68917f31d9f28055ebc65baa7ed35336` | LPPL-1.3c-or-later |
| l3kernel | 2026-08-10 | `342e0ac756b418d095a23eb37aa771a4df3d27db396d43c9e911e0ab9e138aca` | LPPL-1.3c |
| unicode-data | 1.20 (2026-08-07) | `ef541913356b94a2ed0795e41609b8108db4edf0227080151b865c3a4963c895` | LPPL-1.3c-or-later / Unicode data terms |
| cm-tfm | 2026-08-23 CTAN snapshot | `9c0f99fa34c7d801c40f6b5ff60bc28f200e8ef6ffb2fe75e54ca835c67fc04c` | Knuth License |
| latex-fonts | 2026-08-23 CTAN snapshot | `4e73240c4037643a7ef7c353bedd4a10cf0e180d851c54f1e68fda4397f33936` | LPPL-1.2 |
| uptex-fonts | 2025-02-18 | `d187b57c3abb5a31380b6798f0d374712a97dafccd1e33476fe6485008736a91` | BSD-3-Clause |
| ec | 1.0、2026-08-23 CTAN snapshot | `364ea6dc4c05ca49833c31f8bb510bd7cd94142e8e934c59df48a950695c9ed4` | Free license not otherwise listed |

## jsclasses調査と権利

公開挙動の比較対象として、CTAN `jsclasses` 2025-05-10を2026-08-23に取得した。

- URL: `https://mirrors.ctan.org/macros/jptex/latex/jsclasses.zip`
- SHA-256: `b73ec5e8208dfa1dae6f58cab9b033e8e91780aefaaacc879c26e904ce8953f8`
- license: BSD-2-Clause
- copyright: 1995--1999 ASCII MEDIA WORKS、1999--2016 Haruhiko Okumura、
  2016--2025 Japanese TeX Development Community

`prjsarticle.cls`へjsclassesのsource、macro定義、test本文は移植していない。A4横組、
日本語文書向けの行長・行送り・titleという利用者から見える挙動だけを調査し、LaTeXの
公開class interfaceから独立に実装した。jsclasses自身も、classだけでは日本語対応を有効化せず、
対応engine環境が別途必要であるとREADMEで明記している。

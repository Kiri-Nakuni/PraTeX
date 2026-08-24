# PDF backend移植ノート

## 1. 調査境界

原pdfTeX実装は参照しない。2026-08-22時点で参照した一次資料は次だけである。

- Adobe, *PDF Reference, Third Edition, version 1.4*
  <https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf>
  - §3.4 file structure
  - §3.6.1 document catalog
  - §3.6.2 page tree
  - §3.7.1 content streams
  - §5.5.4 font descriptors
  - §5.6.1--5.6.5 composite fonts、CIDFonts、CMaps
- Hàn Thế Thànhほか, *The pdfTeX user manual*
  <https://mirrors.ctan.org/systems/doc/pdftex/manual/pdftex-a.pdf>
- Adobe, *The Type 1 Font Format*
  <https://adobe-type-tools.github.io/font-tech-notes/pdfs/T1_SPEC.pdf>
- Karl Berryほか, *Dvips: A DVI-to-PostScript Translator*
  <https://www.tug.org/texinfohtml/dvips.html>
  - `papersize=width,height` specialと受理するTeX単位

PDF ReferenceはPDFを生成するsoftwareへ、必要なdata structureとoperatorを使う許諾を
明記している。ここでは仕様本文を転載せず、必要なobject関係だけを独自のRust型へ写す。

## 2. 現在の層

1. `pdf.rs`: object、stream、従来型xref、trailerだけを書く低水準serializer。
2. `pdf_document.rs`: Catalog、Pages、Page、Contentsの一段page treeとMediaBoxを作る。
3. `output_backend.rs`: 組版木を一度だけ走査して表示eventを渡す境界。
4. `pdf_backend.rs`: 同じeventをPDF座標・content operatorへ写すbackend。
5. `font_resources/named_cid.rs`: 明示物理pathの一profileをboundedに読むhost境界。
6. `pdf_cid_font.rs`: Type 0、CIDFontType0、FontDescriptor、ToUnicodeを組み立てる層。

最小文書はCatalog、page-tree root、Page、Contentsの4 objectである。PageはParent、
MediaBox、空でも明示するResources、Contentsを持つ。複数ページは出現順にKidsへ積み、
Countを終了時に確定する。

## 3. 座標

TeXのscaled pointからPDF default user spaceのbig pointへの変換は、magを含めて

```text
sp * mag * 7200 / (1000 * 65536 * 7227)
```

とする。計算はchecked `i128`、保持と印字は10^-6 bp単位の固定小数で行う。これにより
浮動小数点の指数表記、丸めmode、platform差をPDF byte列へ入れない。ページ寸法は正で
なければ拒む。

## 4. backend境界

共通走査が発行するのはpage開始・終了、push/pop、right/down、font定義・選択、文字、
set/put rule、special、finishである。文字eventにはDVI命令が暗黙に進める幅も渡す。
DVI adapterは幅を捨てるため従来byte列を変えず、PDF側は同じ幅で現在位置を進められる。

この境界を静的ジェネリクスにして、node一個ごとのtrait object dispatchを避ける。
PDF用にページ木を再走査しないので、遅延open/write/close whatsitも一度だけ実行される。
fontの定義済み状態はbackend文書ごとの `Vec<bool>` に持ち、fmtへ保存されたDVI固有の
`FontInfo.used` に依存しない。

## 5. 現在の直接出力

`-output-format=pdf` と `--output-format=pdf`（値を次の引数へ分ける形も可）でPDFを選ぶ。
指定がなければ従来どおりDVIである。ページがなければどちらの出力ファイルも作らない。

`--pdf-font-map=<map>` または `--pdf-font-map <map>` をPDF指定と併用すると、mapを
論理名のままresolverへ渡し、Type 1全埋込みを明示的に有効にする。指定しないPDFは
Courier smokeを保つ。DVIとの併用、空値、壊れたmap、欠けた資材は黙って無視せず診断して
終了する。mapのsubset指定をfullへ昇格しない。

既定の横組JFM `upjisr-h`はrepository内の検査済みprofileを自動選択する。
`--pdf-japanese-cid-profile=<path>` または値を次の引数へ分けた形ではそれを上書きし、pathを論理名探索へ
渡さず、明示された物理fileを一回だけ最大64 KiBまで読む。一fileは一JFMだけに対応し、
productionのfont定義時にJFM論理名を照合する。profile loader、path、provider handleはfmtへ
保存せず、glyph loopは書き込み済みfont handleだけを使う。Type 1 mapと同時指定してもよく、
Courierを`/F1`に固定した同じfirst-use resource列へType 1とnamed CIDを衝突なく並べる。

Profile 1はASCIIの次の形式で、各fieldをちょうど一回要求する。未知field、重複、欠損、
`EndProfile`後のdataを拒み、コメントやincludeは実行しない。

```text
PraTeX-Named-CID-Profile 1
JfmName min10
BaseFont HeiseiMin-W3
Flags 6
FontBBox -123 -257 1001 910
ItalicAngle 0
Ascent 880
Descent -120
CapHeight 700
StemV 80
DefaultWidth 1000
EndProfile
```

この版ではPDF 1.4の`/Encoding /UniJIS-UCS2-H`と
`/CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 4 >>`だけを型で固定し、
BMP Unicode scalarだけを2-byte big-endian hex stringへする。非BMPとsurrogateは文字を
書いたり位置を進めたりする前にtyped errorにする。JFM幅は次の`Tm`位置を決めるhost側の
advanceにだけ使い、profileの`DefaultWidth`やPDF `/W`をJFMから推測しない。
Type 0の`/ToUnicode`はCIDからの恒等写像ではない。contentへ書いたUCS-2 source codeから
元のUTF-16BE scalarへ戻すCMapであり、`UniJIS-UCS2-H`と同じくsurrogate帯をcodespaceから
除外する。Unicode--CID写像そのものは引き続きviewerのpredefined CMapだけが担う。

JFM/TFMはmetricとclassを与えるだけで、outline、bitmap、Unicode--CID mappingを持たない。
このsliceはFontFileを埋め込まないため、`BaseFont`を実装し`UniJIS-UCS2-H`を解決できるviewerで
だけ意図した字形になる。Standard 14以外のnamed fontが常に利用できる保証はなく、portableな
表示、全extractor互換、PDF/A適合を主張しない。内蔵profileのないJFMやJFM名不一致を
Courier/tofuへfallbackせず停止する。Repositoryへ第三者font資材は追加していない。

実配布mapのresourceは順序と`<` / `<<` / `<[`を保つ配列として構文解析する。同じ行に
encoding、generic header、font programが複数並んでも、未使用entryだけを理由にmap全体を
拒まない。`< file.pfb`のようにmarkerと名前が分離したdvips互換形も読む。実際に選んだ
TFMについてだけ、現在対応するPFBが一個、encodingが高々一個であることを検査する。
`.t3`等の未対応headerや複数PFBを黙って選ばない。

mapの`fontflags`省略時はpdfTeX manualが定める既定値4（Symbolic）を使い、明記値とは別に
保持する。PDFで必須の`StemV`をAFMが省略した場合は、PFBのeexec部を公開Type 1仕様どおり
safe Rustでstream復号し、最初の四byteを捨ててPrivate辞書の`/StdVW [number]`だけを読む。
PostScriptは実行せず、コメント、文字列、procedure、配列、辞書の中をkeyと誤認せず、
`/Subrs`または`/CharStrings`で停止する。一フォント1 MiBの走査上限を持ち、値がどこにも
なければ固定値を推測せず拒む。

現在の限定readerはPrivate辞書のうちSubrs/CharStringsより前だけを対象にし、数値は通常の
符号付き十進表記だけを受ける。PostScriptのradix・指数表記やSubrs後の定義を実行して補う
ものではない。対応範囲外は推測せずtyped errorにし、必要な実fontが現れた段階で公開仕様と
black-box fixtureを追加して広げる。

- TeX座標は左上・下向き、PDFは左下・上向きなので、物理1inch余白を含むMediaBoxの
  高さから現在の縦位置を引く。
- 余白72bpは `\mag` で拡大せず、内容座標とfont sizeだけへmagを掛ける。
- 第一page内の最後の`papersize=width,height` specialは物理MediaBoxを確定し、specialのない
  後続pageにも継承する。`in`、`cm`、`mm`、`pt`、`sp`、`bp`、`pc`、`dd`、`cc`を受ける。
  十進値を途中で整数spへ丸めず、spを基底とする既約整数比のままPDF固定小数境界へ運ぶ。
  用紙寸法には`\mag`を掛けない。第一pageの途中で読んでも既出contentと座標基底を混ぜず、
  page終了時にcontent全体を新しい上端へ一度だけ平行移動する。
- `papersize`を第二page以降で初指定・変更すること、非正値、未知単位、欠損、overflowは
  typed errorにする。同一値の再指定だけは許す。明示用紙では内容に合わせてMediaBoxを
  勝手に拡張せず、はみ出しは通常のPDF clippingに委ねる。
- 宣言されたbox寸法だけでなく、実際に置いた文字・ruleの範囲も追跡する。負幅や
  overfull内容があるときはcontentを平行移動してMediaBoxを拡張し、1inch余白を保つ。
- set/put ruleは `re f` へ写す。setだけが横位置を進める。
- printable ASCIIはStandard 14 Courierを `/F1` とし、文字ごとの絶対 `Tm` で置く。
  括弧とbackslashはPDF literal stringとしてescapeする。その他の8-bit文字は配置幅だけ
  進め、まだ描かない。明示map経路では、TFMに実在するcodeだけをType 1 fontの連続
  `/Widths` と許可maskへ写し、文字をhex stringで出力する。
- 横組JFMのwide glyphは明示profileがある時だけnamed CID resourceへ`<XXXX> Tj`で置く。
  同一pageで同じresourceを一回だけ登録し、各文字の絶対`Tm`差は渡されたJFM幅をspから
  固定小数bpへ変換した値に一致する。BMP source codeは同じfontの`/ToUnicode`から
  元Unicode scalarへ戻せる。
- 認識しないraw `\special` はcontent streamへ注入せず捨てる。

最小LaTeX文書のCourier実測は1 page / 2169 bytes。Popplerとstrict pypdfが構造を読め、renderで
本文、単純な数式、ページ番号を確認した。Courierの字形へCMの幅を使うため見た目と
text extractionは暫定であり、ここでの「standalone」は外部DVI driver不要という意味に
限る。

Ubuntu-24.04 / TeX Live 2026の実測では、5,573,038 byte・46,380行の正規`pdftex.map`を
最後まで構文解析し、`cmr10 CMR10 <cmr10.pfb`はmap/flags/StemVではなく未実装のsubsetだけで
停止した。別の一時mapで同じresourceを`<<cmr10.pfb`と明示した黒箱照合では、実物PFB/AFMを
用いて1 page / 37,491 bytesのPDFを生成した。strict pypdfとPopplerでPDF 1.4を読み、144 dpi
描画で`ABC`を確認した。埋込みFontDescriptorは`/BaseFont /CMR10`、`/Flags 4`、`/StemV 69`、
`Length1/2/3 = 4287/30900/545`だった。これはsubset要求をfullへ変えた結果ではなく、検証用
mapが明示したfull埋込みだけの結果である。

同じ経路を固定幅の実物`cmtt10`でも照合した。AFMの`IsFixedPitch`からmap flagsを再推論せず、
pdfTeXの省略時契約を優先して`/Flags 4`を出した。strict pypdfで抽出文字列`ABC`、
`/BaseFont /CMTT10`、PFB由来`/StemV 69`、`Length1/2/3 = 4364/26170/545`を確認し、Popplerで
1 page / 32,834 bytesのPDF 1.4を描画した。

利用者から、2026-08-24より少し前の枝で直接PDFを生成すると、欧文がすべてタイプライタ体に
見える事象が報告されている。対象commitとmap指定の有無は未固定なので、現時点で原因をCourier
fallback、NFSS同期、resource identity、viewer代替のいずれかに決めない。再現gateでは同一入力の
DVIと直接PDFについてroman/sans/mono、medium/bold、upright/italicを明示し、選択TFM、PDF
`/BaseFont`・`/FontDescriptor`・`/Widths`、contentのresource名、描画結果を比較する。Standard 14
Courier smokeが通ることを欧文font選択の正しさへ読み替えない。

合成横組JFMの「ああ」を10 pt font・各5 pt advanceで流すnamed CID process試験は、
1 page / 1,735 bytesのPDF 1.4を生成した。strict `pypdf`で`/F1`と`/F2`、Type 0、
CIDFontType0、`UniJIS-UCS2-H`、Adobe-Japan1 supplement 4、`/DW 1000`、`/ToUnicode`を再読込みし、
`pypdf`とPDFiumの独立した抽出でどちらも「ああ」へ戻ることを確認した。
Poppler `pdfinfo`もpage treeを受理した。試験環境のPopplerにはprofileのnamed fontそのものが
なく`MS-Mincho`へ代替して描画したため、render成功は指定fontの可搬性を示さない。合成JFMが
意図的に全角字形へ半角advanceを与えるので、render上の二字は重なる。PDF contentの二つの
絶対`Tm`の差`4.98132 bp`が`5 * 65536 sp`の変換値と一致することをbyte oracleにしている。

このprofileは**非埋込みCID**のhost設定であり、OTF loader、RustyBuzz shaping、pdfTeX相当の
PDF primitive/font処理が完成したという意味ではない。

## 6. 次の段階

1. 上記の欧文font-family回帰matrixを固定し、通常mapのType 1 subset埋込みと、同じ物理fontを
   異なるsizeで使う時のobject共有へ進む。
2. 実装済みpapersize specialと同じ型付き媒体境界へ、PraTeX正規名のpage-size primitiveを
   接続する。pdfTeX互換名は意味が一致してから別途扱う。
3. Type 1が揃ってから `\pdfoutput` を登録し、LaTeXのpdfTeX backend判定を有効にする。
4. 汎用ToUnicode、run batching、TrueType、和文字形の埋込み、複数profile tableは独立した
   後続段階とする。今回のnamed CID profileを埋込みfontやOTFであるかのように扱わない。

生の `\special` をPDF content streamへ注入してはならない。認識したspecialだけを専用の
parser境界から扱う。

## 7. 実資材black-box記録

実物fontとmapはruntime oracleとしてのみ使い、repositoryへ入れない。2026-08-22の照合値は
次である。

| 資材 | SHA-256 |
|---|---|
| TeX Live 2026 `pdftex.map` | `57cd5c139e817f0c6c5929bed6b18c0ee6d1eb90c734b585811d81dac1604318` |
| AMSFonts `cmr10.pfb` | `fdcede8794018df5f2b58f0905fb20a2b418ed8f67b73ee12445855dfbe5b1be` |
| AMSFonts `cmr10.afm` | `8df68c822c3217c67dee325d5f7d80c6b9e492293bca366db679a5b0b5e1825b` |
| Computer Modern `cmr10.tfm` | `87f2d8981927644cbecaf3d639e96e348ea4e7be49d8804468bd8ba9ff3f5244` |

実装時にpdfTeXやfont toolのsource codeは参照していない。map marker、flags既定値、eexec暗号、
Private `StdVW`、PDF FontDescriptorの意味は上記公開manualとAdobe仕様から独立に実装した。

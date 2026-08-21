# PDF backend移植ノート

## 1. 調査境界

原pdfTeX実装は参照しない。2026-08-22時点で参照した一次資料は次だけである。

- Adobe, *PDF Reference, Third Edition, version 1.4*
  <https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf>
  - §3.4 file structure
  - §3.6.1 document catalog
  - §3.6.2 page tree
  - §3.7.1 content streams
- Hàn Thế Thànhほか, *The pdfTeX user manual*
  <https://mirrors.ctan.org/systems/doc/pdftex/manual/pdftex-a.pdf>

PDF ReferenceはPDFを生成するsoftwareへ、必要なdata structureとoperatorを使う許諾を
明記している。ここでは仕様本文を転載せず、必要なobject関係だけを独自のRust型へ写す。

## 2. 現在の層

1. `pdf.rs`: object、stream、従来型xref、trailerだけを書く低水準serializer。
2. `pdf_document.rs`: Catalog、Pages、Page、Contentsの一段page treeとMediaBoxを作る。
3. `output_backend.rs`: 組版木を一度だけ走査して表示eventを渡す境界。
4. `pdf_backend.rs`: 同じeventをPDF座標・content operatorへ写すbackend。

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

- TeX座標は左上・下向き、PDFは左下・上向きなので、物理1inch余白を含むMediaBoxの
  高さから現在の縦位置を引く。
- 余白72bpは `\mag` で拡大せず、内容座標とfont sizeだけへmagを掛ける。
- 宣言されたbox寸法だけでなく、実際に置いた文字・ruleの範囲も追跡する。負幅や
  overfull内容があるときはcontentを平行移動してMediaBoxを拡張し、1inch余白を保つ。
- set/put ruleは `re f` へ写す。setだけが横位置を進める。
- printable ASCIIはStandard 14 Courierを `/F1` とし、文字ごとの絶対 `Tm` で置く。
  括弧とbackslashはPDF literal stringとしてescapeする。その他の8-bit文字は配置幅だけ
  進め、まだ描かない。
- raw `\special` はcontent streamへ注入せず捨てる。

最小LaTeX文書の実測は1 page / 2169 bytes。Popplerとstrict pypdfが構造を読め、renderで
本文、単純な数式、ページ番号を確認した。Courierの字形へCMの幅を使うため見た目と
text extractionは暫定であり、ここでの「standalone」は外部DVI driver不要という意味に
限る。

## 6. 次の段階

1. 公開map/encoding/PFB探索とTeX Type 1 fontの全埋込みへ進む。TFMだけではbyte codeから
   glyph名を決められない。
2. `\pdfpagewidth` / `\pdfpageheight` 相当または認識済みpapersize specialで物理媒体を
   指定できるようにする。
3. Type 1が揃ってから `\pdfoutput` を登録し、LaTeXのpdfTeX backend判定を有効にする。
4. ToUnicode、run batching、TrueType、和文Type 0/CIDFontは独立した後続段階とする。

生の `\special` をPDF content streamへ注入してはならない。認識したspecialだけを専用の
parser境界から扱う。

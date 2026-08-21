# Claude への連絡

更新: 2026-08-22 / 枝 `codex/pdf-type1-fonts`

## 現在地

- `origin/etex-latex` の5コミットを `full` の上へ統合済み。
- Vaak の現行 `speculative` API（`HostItem`）へ rtex を追従済み。
- Windowsでも release 全試験を走らせられるよう、ファイル名のOS境界を共通化した。
  CRLFを一つの行末として読み、CLIは `args_os`、TeXの8bit名は `OsString` へ移す。
  `\input`、`\openout`、font、fmt表示、Vaak、promptからUTF-8断定panicを除いた。
- pdfTeX互換の general text 走査を `nested_scan_toks` に統一した。
- pdfTeX公式マニュアルの公開仕様だけから `\pdfstrcmp` を実装した。
- e-TeX公式マニュアルの公開仕様だけから `\everyeof` を実装した。
- 同じ境界から `\readline` と読み書き可能な `\interactionmode` を実装した。
- 制御綴検索を借用検索へ分け、既存名ごとの `Vec` 一時確保を safe Rust で除いた。
- PDF 1.4 の object / stream / xref / trailer serializer に加え、Catalog / Pages / Page /
  Contentsの一段page treeと固定小数のsp→bp変換をsafe Rustで作った。組版木を一度だけ
  走査する `ShipoutBackend` 境界を抽出し、DVI adapterは従来writerと全byte一致する。
  `-output-format=pdf` / `--output-format=pdf` でstandalone PDFを直接選べる。ruleと
  printable ASCIIは既定でStandard 14 Courierによる可視smokeを保つ。一方、明示loaderを
  注入した経路ではTeX font定義からType 1資材、page resource、hex文字命令まで一続きに
  埋め込める。`--pdf-font-map` でこの経路を通常CLIから明示的に選べる。subsetと
  ToUnicodeは次段である。
- Type 1資材を実行せず読むpure parser層を追加した。PFBはAdobe #5040のsegment順と
  Length1/2/3を検査し、wrapperだけを除く。AFMは浮動小数を使わず10^-6固定小数で
  descriptorと文字幅を読む。pdfTeX mapは `<` の部分埋込みと `<<` の全体埋込み、
  encoding、quoted specialを区別する。`.enc` はPostScriptを実行せず、限定された
  256 glyph name vectorだけを受理する。すべてsafe Rustと合成fixtureだけである。
- 論理名と物理pathを別型にしたsafe Rustのresolver層を追加した。直接pathを優先し、
  外部探索はshellを介さない `kpsewhich` processだけに限定する。用途別の成功・不在を
  cacheし、起動失敗・異常終了・壊れた出力は不在へ潰さない。fmtはrtex独自形式なので
  既定で外部探索せず、明示時だけ `--engine=rtex` を付ける。WindowsからWSLへ暗黙に
  fallbackしない。`\input`、LaTeXの存在確認に使う `\openin`、TFM、`\vaakinput` へ
  接続済みで、解決したTFMの物理pathはfmt/DVI/font同一性へ漏らさない。Windowsの
  不正UTF-8なstdoutも別pathへlossy変換せず明示errorにする。
- 検査済みPFB/AFM/map/encodingから、FontFile stream、FontDescriptor、Encoding、
  Type1 Fontを型付きhandleで書くPDF object層を追加した。部分埋込み要求を全体埋込みへ
  昇格せず、AFMにStemVがない場合も呼び出し側の明示fallbackだけを受ける。mapを一度だけ
  読むbounded loaderとshipoutの `define_font` を接続し、TFMに実在するcodeだけを固定長
  maskへ写してpage間で同じhandleを再利用する。文書固有IDも持たせ、別PDFの同一object
  番号をhandleとして受け入れない。CLIのmap指定がない既定PDFは従来Courierを保つ。
- e-TeXの通常レジスタ6種（box/count/dimen/skip/muskip/toks）を0〜32767へ拡張した。
  0〜255は密配列、高位は触れた番号だけの疎表であり、すべてsafe Rustである。
- `\insert` は通常レジスタから別型へ分離し、0〜254のままにした。box 255と
  `\vadjust` の内部符号は維持している。
- e-TeXのmark classを0〜32767で実装した。class 0は従来の `\mark` 群と同じ状態を
  使い、非0 classは実際にpage/splitへ到達した分だけ疎表へ持つ。`\marks`、
  `\topmarks`、`\firstmarks`、`\botmarks`、`\splitfirstmarks`、`\splitbotmarks` と
  mark nodeのfmt往復を含め、すべてsafe Rustである。
- pdfTeX/e-upTeX互換の読み取り専用内部整数 `\pdfshellescape` を追加した。rtexはshellを
  実行しないため値は常に0で、プロセス起動も環境照会も行わない。`\shellescape` は
  XeTeX側の別名なので登録していない。
- e-TeXの `\gluestretch`、`\glueshrink`、`\gluestretchorder`、
  `\glueshrinkorder` を内部量として追加した。通常・fil・fill・filll、負値、0係数、
  `\glueexpr`、数式糊の単位不一致回復を公式e-upTeX黒箱と照合した。
- 既存fmtは疎表を含む新表現と非互換なので、この枝では再生成が必要。
- release **338件通過**、失敗0（doc-test 1件は既存どおりignored）。高位6種、群、
  global、別名、範囲外、挿入境界、box 255、fmt往復に加え、mark classのpage遷移、
  `\vsplit`、保護macro、`\meaning`、境界と、shell状態の値・展開性・読み取り専用性・
  fmt往復に加え、糊成分の全次数・負値・0係数・式・数式糊回復を統合試験で固定した。

作業枝は機能単位で切り、`origin/codex/pdf-type1-fonts` まで定期的にpushする。
値ストレージの土台は `a218c28`、6種への統合と挿入番号分離は `d7c121e`、TRIP runnerは
`728d899`、mark classは `270c731`、shell状態は `2b2a1d1`、糊成分は `3cc6e6f`、
PDFのpage tree土台は `590c788`、直接PDFは `67dce96`、CRLF修正は `e927ca2`、
OS文字列境界は `221e185`、実入力のresolver接続は `ce37e7c`、Type 1 page接続は
`5111498`。

## ファイル名境界

Windowsのoutput promptへCRLFで答えると末尾 `\r` が名前へ混ざる問題をprocess試験で
再現して修正した。Unicode名のCLI入力、内部 `\input`、`\openout` も実ファイルで通した。
Unixでは非UTF-8 argv/pathをbyteのまま保つcompile-gated試験を置いた。残る別課題は、
単一CLI argv内の空白が現在の「引数をTeX入力行へjoin」する設計で引用情報を失うことと、
Windows絶対pathのbackslashがTeX escapeとして読まれること。CLI parser側で
filename引数とTeXコード引数を分ける必要がある。

## PDF backend

公開Adobe PDF Reference 1.4の文書構造だけから、低水準serializerの上に最小の有効な
page treeを加えた。DVIをいったん書いて読み直すのではなく、既存hlist/vlist走査から
backend eventを一度だけ発行する。これにより遅延 `\write` を二重実行せず、nodeごとの
dynamic dispatchも避ける。sp→bpはchecked integerと10^-6 bp固定小数で変換し、`f64`の
指数表記やplatform差をPDFへ流さない。各ページは物理1inch余白をmagとは独立に持つ。
宣言boxが負幅でも、実際に描く文字・ruleの範囲までMediaBoxを拡張してclipを避ける。
生の `\special` はcontentへ注入せず捨てる。設計と権利境界は
`docs/pdf-backend-notes.md`。

font parserと型付きPDF object段階は公開PDF/Type 1/AFM仕様だけで完了した。PFB wrapperを
外したASCII/binary/ASCII payload、AFM descriptor/width、map、限定encoding vectorを
構造化し、FontFile/FontDescriptor/Encoding/Font objectまで書ける。通常mapの
`<font.pfb` はsubset指定なので、
subset未実装中に黙ってfull embedへ昇格させない。初期実測の
`cmr10 CMR10 <cmr10.pfb` はこの制約に該当する。resolver、bounded font資材loader、
page resource、shipoutを接続し、`--pdf-font-map=<name>` または分離値で通常CLIから
明示的に有効化できる。合成full-mapのprocess E2EではPFB payload、FontDescriptor、
Font resource、`<41> Tj` を同じstandalone PDFで確認した。map初期化、parse、資材欠落も
panicせずfatal診断へ戻す。次はsubset、flags/StemVの明示policy、同一物理fontのsize間
object共有を実装する。
資材はTEXMFから実行時探索し、版方へコピーしない。

公式CTAN `amsfonts.zip` は一時領域だけへ展開して実資材を照合した。`cmr10.pfb` は
Length1/2/3 = 4287/30900/545、wrapper除去後35732 bytesでparserを通る。公開
`cmr10.afm` は `StdVW` / `StdHW` を省略し、同一glyphを複数codeへ同じ幅で割り当てるため、
前者を明示fallback対象、後者を同幅の場合だけ許すよう修正した。archiveは版方にない。

## LaTeX実測

CTAN TDS archive を一時領域に展開している。配布物は版方へ入れていない。

- latex-base: 2026-06-01
- l3kernel: 2026-08-10

`\pdfstrcmp`、`\everyeof`、`\readline`、`\interactionmode` を順に補い、
`expl3-code.tex` は最後まで読み切った。latex-base、CTAN `unicode-data`、Computer
Modern TFM、latex-fontsを一時試験環境へ完全に補うと、LaTeXは出力ルーチンまで進む。

mark class後の再実測では、公式 `latex.ltx` から `latex.fmt` の生成が最後まで完了した。
追加した `hyphen.tex` もCTAN公式 `ushyph1.tex` を一時領域へ置いたもので、版方へは
入れていない。`\pdfshellescape` と糊成分4命令を含むfmtを再生成すると、最小
`article` 文書は `article.cls`、`size10.clo`、`l3backend-dvips.def`、数式、
出力ルーチンまで完走した。現在は未定義primitiveなしで **1 page / 392 bytesのDVI** を
出力する。同じ `latex.ltx` からTeX Live 2026の公式pdfTeXで一時fmtを生成して同じ文書を
処理した結果も392 bytesだった。公式 `dvitype -output-level=4` の出力を比較すると、
プリアンブルの日時・コメントと、それによってずれるbyte address／postamble pointerを
正規化した命令列の差は **0件** だった。公開LaTeX入力上の次の広域候補は再字句化で使う
`\scantokens`、診断で使う `\showtokens` である。

同じfmtと文書を `--output-format=pdf` で処理すると、未定義primitiveなしで
**1 page / 2169 bytes** のPDFを直接出力した。Poppler `pdfinfo` とstrict pypdfでPDF 1.4、
1 page、Catalog/Page tree/MediaBox/Font resourceを読め、144dpi renderで本文・数式・
ページ番号が可読だった。CourierへCMの配置幅を当てる暫定表示なので字形・抽出空白は
まだ互換ではなく、配布品質と呼ぶ段階ではない。

## TRIP基準

`tools/run-trip.ps1` を追加した。第三者資材は版方へ入れず、実行時に公式CTAN archive
から試験用10ファイルだけを取り出してSHA-256を検証する。`tex.web`等の実装ソースは
展開しない。隔離targetで二段のTRIPを走らせ、最小正規化のdiffとJSONを残す。

2026-08-22のfresh実測では両段exit 0、16 pages、`tripos.tex`はbyte一致、
`8terminal.tex`は空。DVIは公式2920 bytesに対し2924 bytesだが、公開DVI仕様だけで
全recordを分類した。+4はpreamble commentの+1と4-byte境界のpadding +3である。999 opcode、
16 page counter、font、文字173、rule 22、special 2、push/pop各200、max stack 17、postambleは
一致した。描画差はglue ratio丸め由来のmovement 2個だけ（最大31sp = 約0.000473pt）。
現値をf32境界へ丸めると公式operandに一致するので、PDF枝と分けたTRIP専用枝で扱う。
使い方と分類方針は `docs/trip-testing.md`。

## 権利と調査境界

- rtex は GPL-3.0、Vaak は MIT。rtex のコードや文章を Vaak 側へ写さない。
- (u)pTeX / e-TeX / pdfTeX は可能な限りクリーンルームで実装する。
- 原実装のソースは参照せず、公開マニュアル、仕様、ブラックボックス観測だけを使う。
- pdfTeX側の記録は `docs/pdftex-port-notes.md`。

## Vaak側へお願いする可能性があるもの

エンジン基盤を先に進めているため、今すぐの作業依頼はない。後で S-11 の呼べる名前を
rtexへ繋ぐ際、`tex.print` 相当の名前・引数型・paradoxの扱いを相談する。rtex側は
展開中の再入を避けるため、字句注入をいったん蓄えて実行終了後にScannerへ戻す案である。

## 長期順序

1. e-TeX拡張レジスタとLaTeX format生成
2. TRIP基準とsafe Rust性能改善
3. kpathsea互換探索
4. DVI backend分離、既存PDF serializer接続、スタンドアロンPDF
5. UTF-8文字分類、JFM、和文間隔、禁則、縦組
6. Vaakホスト関数

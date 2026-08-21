# Claude への連絡

更新: 2026-08-22 / 枝 `codex/euptex-utf8-cjk-token`

## 現在地

- `origin/etex-latex` の5コミットを `full` の上へ統合済み。
- Vaak の現行 `speculative` API（`HostItem`）へ rtex を追従済み。
- Windowsでも release 全試験を走らせられるよう、ファイル名のOS境界を共通化した。
  CRLFを一つの行末として読み、CLIは `args_os`、TeXの8bit名は `OsString` へ移す。
  `\input`、`\openout`、font、fmt表示、Vaak、promptからUTF-8断定panicを除いた。
- pdfTeX互換の general text 走査を `nested_scan_toks` に統一した。
- expandableな `\directvaak` も同じ一般文経路へ統一した。`\message`や`\edef`の外側が
  溜めた`def_ref`、scanner status、warning indexを失わず、rtex process試験で固定した。
- pdfTeX公式マニュアルの公開仕様だけから `\pdfstrcmp` を実装した。
- e-TeX公式マニュアルの公開仕様だけから `\everyeof` を実装した。
- 同じ境界から `\readline` と読み書き可能な `\interactionmode` を実装した。
- 制御綴検索を借用検索へ分け、既存名ごとの `Vec` 一時確保を safe Rust で除いた。
- 数値・条件走査で多用する一字の `back_input` をinline保持へ分け、一回ごとの
  `Vec<Token>` と `Rc` のheap確保を除いた。通常token listは従来の同じ`Rc`を共有し、
  `InputSource`全体のsizeは56 bytesのままである。
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
- release **362件通過**、失敗0（doc-test 1件は既存どおりignored）。高位6種、群、
  global、別名、範囲外、挿入境界、box 255、fmt往復に加え、mark classのpage遷移、
  `\vsplit`、保護macro、`\meaning`、境界と、shell状態の値・展開性・読み取り専用性・
  fmt往復に加え、糊成分の全次数・負値・0係数・式・数式糊回復を統合試験で固定した。

作業枝は機能単位で切り、`origin/codex/pdf-type1-fonts` まで定期的にpushする。
値ストレージの土台は `a218c28`、6種への統合と挿入番号分離は `d7c121e`、TRIP runnerは
`728d899`、mark classは `270c731`、shell状態は `2b2a1d1`、糊成分は `3cc6e6f`、
PDFのpage tree土台は `590c788`、直接PDFは `67dce96`、CRLF修正は `e927ca2`、
OS文字列境界は `221e185`、実入力のresolver接続は `ce37e7c`、Type 1 page接続は
`5111498`、CLIからの明示map接続は `62e58e5`、TRIPのglue比率一致は `58af183`。
一字差し戻しのsafe Rust性能改善は `f830570`。

## e-upTeX `\kcatcode` / UTF-8 token

upTeXの原実装・change file・上流回帰試験を見ず、`uptex-base` の公開
`01uptex_doc_utf8.txt` Ver2.02、Unicode 17.0.0 `Blocks.txt`、公式e-upTeXバイナリの
black-boxだけから表を起こしている。U+0000..U+10FFFFを今回の明示範囲とし、通常の
Unicode block 346個に、公開block番号と実機代入単位が一致する擬似境界12個を足す。
通常358単位と非連続例外7単位を固定配列で持ち、文字ごとのHashMapは使わない。

擬似境界は U+33480、U+40000..U+D0000 の各面先頭、U+E01F0。Extension F/I の重なり、
surrogate三block、7例外集合、`latin_ucs=14` のU+2E7F制限、不正値を16へ直す回復も
実機で確認した。357開始境界、358区間末尾、51個のnamed-block gap候補も全件照合し、
境界・区間の不一致は0件だった。代入、group/global、保存level、内部量、fmtの厳密な
版・個数検証までを
`kcatcode` 表・代入・group・fmtは `9d04c08` で完了し、
`origin/codex/euptex-kcatcode-table` へpush済み。

現在の `codex/euptex-utf8-cjk-token` では、16〜20を符号位置と入力時categoryを持つ
一つの `CjkToken` として実装した。ASCIIは従来のbyte fast path、15はbyte列、14の
`latin_ucs` tokenは後続枝に残す。制御綴名は `Byte(u8)` / `Unicode(u32)` のtyped identityを
持ち、同じ表示bytesのbyte名とwide名を区別する。categoryはtokenへ固定するが制御綴identityには
含めない。macro、`\edef`、`\let`、`\if` / `\ifcat` / `\ifx`、delimiter、文字定数、
`\string` / `\detokenize`、fmt往復まで接続した。

PrinterにはUTF-8一文字をatomicに渡す境界を設け、継続byteを `newlinechar` と誤認すること、
`\write` の二重符号化、log折返しとerror contextの文字途中切断を防いだ。不正UTF-8だけは
公式e-upTeXの黒箱結果どおり先頭1 byteずつ `^^hh` へ戻して再同期する。wide hashと逆引き表は
実際にwide名を作るまで確保しない。Windows 1 MiB stackでCJK fmt読込み時に落ちたため、
大きなfmt戻り値をBox化し、INITEX構築・読込み・実行のframeを分けた。unsafe Rustは未使用。

直前 `9d04c08` と同じrelease LTO・同じVaakでASCII 100万反復fixtureを交互に各11回測定した。
wall中央値は553.506ms→542.120ms、CPU中央値は546.875ms→531.250msで、stdout/log hashは
全回一致した。小差を高速化とは数えないが、CJK層追加によるASCII退行は観測されていない。

release全回帰は **406件通過、失敗0**（doc-test 1件は既存どおりignored）。TRIPは二段とも
exit 0、16 page。`trip.dvi` は直前の既知意味一致baselineとbyte単位で同じ
`27b79b612b94a1d2815a8747d09b6ba665f2adfb9f521fcfe7020c6347a29342`、`trip.fot`、
`tripin.log/fot`、`tripos.tex`、空の `8terminal.tex` も同一。`trip.log`だけ、文字定数の
公式e-upTeX互換診断を `Improper alphabetic or KANJI constant.` へ変えた9 bytes分が異なる。

次の独立実験として、通常のe-upTeX互換経路を残したまま `catcode` と `kcatcode` の区別を
統一できるか、別branchで調査する。LaTeXのbyte-orientedな字句前提を壊す可能性が高いので、
既存 `latex.ltx` をそのまま通すことを成功条件にはせず、必要ならclean-roomのformat層を分ける。
catcodeは将来の追加規則をclosed Rust enumへ埋め込まず、組込み値のfast pathと拡張ID/registryを
分ける。単純な規則または高頻度の規則はVaak、複雑だが低頻度の規則はversioned WASM ABIへ
委ねる方針。WASMへRust layoutやpointerを出さず、数値ID、長さ付きbyte列、opaque handle、
capabilityを使う。tokenごとのABI crossingは避け、buffer/node/eventをまとめて渡す。
Vaak擬似callbackは常時有効にしない。TeXが特定のVaak実行を明示したときだけ、その実行へ
限定scopeのcallback capabilityを渡し、終了時に失効させる。読込みやengine起動だけではhookを
登録せず、暗黙のglobal callback表も持たない。

TeX82の `^^xx` / `^^M` は既存lexerにあるが、XeTeX/LuaTeXの `^^^^hhhh` /
`^^^^^^hhhhhh` は未実装。公式e-upTeX 2026も後二者を展開しないため、e-upTeX互換機能ではなく
明示的なUnicode engine拡張として、統一classifier枝で公式XeTeX/LuaTeXを黒箱照合して足す。

将来拡張として生文字列レジスタも検討する。値はtoken listやRust `String` ではなく任意byteを
保つ列。`\the\rawstring<n>` は合成TextSourceとして現在のclassifierで再字句化し、
`\therawstring\rawstring<n>` は再字句化せず全単位をOther/catcode 12 tokenへする。
通常のbrace引数は取得時点でtokenize済みなので「生」と偽らない。inlineは明示delimiterを
直接TextSourceから読む限定経路、file/Vaak/WASMは長さ付きbyte列のhost経路に分ける。
改行は `\the` では合成行境界、`\therawstring` ではOther文字として扱う初期案。

この枝の最終release試験は362件通過、失敗0。TRIPも二段ともexit 0で、直前の
glue-ratio基準と `trip.dvi`、`trip.fot`、`tripos.tex`、空の `8terminal.tex` がbyte一致した。
log/fot差は `\kcatcode` primitive追加に伴う multiletter control sequence の +1 だけで、
DVI SHA-256は引き続き `27b79b612b94a1d2815a8747d09b6ba665f2adfb9f521fcfe7020c6347a29342`。

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
`8terminal.tex`は空。glue ratioをboxへ保存するproducer 8箇所だけにf32境界を置き、
consumer、累積、fmtはf64のまま保った。修正前に残っていたpage 10のmovement
`639342177` とpage 15の `203921756` は、公式の `639342208` / `203921760` に一致した。
DVIは公式2920 bytesに対し2924 bytesだが、+4はpreamble commentの+1と4-byte境界の
padding +3である。公開DVI仕様だけで再度全recordを復号し、commentとそれに伴うfile
pointer、paddingだけを除くと、公式・rtexとも **999 records、意味差0件** になった。
logにはe-TeX拡張範囲、追加単位、未提供のmemory統計などの診断差を意図的に残している。
使い方、未解消差、分類方針は `docs/trip-testing.md`。

## safe Rust性能

100万反復・約200万回の一字差し戻しを通す130 bytesの合成入力を、release LTOで変更前後
交互に各11回測った。wall中央値は768.249 msから510.810 msへ33.51%、process CPU中央値は
750 msから500 msへ33.33%短縮した。全回で終了値0、stdoutとlogのSHA-256は一致する。
64-bitの`TokenListReader`は16から24 bytesになるが、`InputSource`は56 bytesのままである。
最適化後もTRIPのDVI、両段のlog/fot、`tripos.tex`のhashは直前枝と同一。手順、
fixture hash、全結果は
`docs/performance.md`。unsafe Rustは使っていない。

## 権利と調査境界

- rtex は GPL-3.0、Vaak は MIT。rtex のコードや文章を Vaak 側へ写さない。
- (u)pTeX / e-TeX / pdfTeX は可能な限りクリーンルームで実装する。
- 原実装のソースは参照せず、公開マニュアル、仕様、ブラックボックス観測だけを使う。
- pdfTeX側の記録は `docs/pdftex-port-notes.md`。

## Vaak側へ確認したいもの

Vaak `speculative` のS-11第一段 `2f3dd65` にある `HostItem::Fn` / `HostFns::call` /
`Runner::run_with` だけで、rtex側から同期host関数を接続できることを確認した。ただし
現状はbare nameのcalleeだけをhost関数として解決するため、`tex.print(...)` はmember callに
なって到達しない。当面rtexで `tex_print(...)` をflat aliasとして出すか、Vaak側でdotted
host名を解決するか、どちらを正式形にするかClaudeに決めてほしい。

rtex側では `\directvaak` の入れ子general-text走査を修正済み。次に非展開の `\vaak` と
`tex_print(str)` を実装する。host呼出し中はbyte列をFIFOへ積むだけにし、Runnerのborrowを
解放してからTeXの生入力行としてScannerへ戻す。rtexのコードや実装文章をVaak側へ移さず、
必要なAPI要求だけを伝える。

## 長期順序

1. e-TeX拡張レジスタとLaTeX format生成
2. TRIP基準とsafe Rust性能改善
3. kpathsea互換探索
4. DVI backend分離、既存PDF serializer接続、スタンドアロンPDF
5. UTF-8文字分類、JFM、和文間隔、禁則、縦組
6. Vaakホスト関数

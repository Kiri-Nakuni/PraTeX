# PraTeXのTeX82外機能と独立実装

更新: 2026-08-25
監査対象: `codex3/perf-integration`（code checkpoint `a2765c7`、全release 915 / 0 / 11）

この文書は、PraTeXが現在持つ機能のうちTeX82の中核にはないものと、PraTeXで新たに
書いた実装を区別して記録する。将来構想を現在の対応機能として数えないこと、既存engineの
互換機能をPraTeX発祥の機能と呼ばないことが目的である。

## 読み方と範囲

ここでいう「TeX82」は、このrepositoryが継承したtyti氏のrtexのTeX82中核
（`f174f44`）を基準にする。1982年版と現在のKnuth TeXの歴史的な全差分を数える文書では
ない。rtex中核そのものはPraTeXの新規成果に含めず、原READMEを
[README_origin.md](../README_origin.md) に保存している。

状態は次の意味で使う。

| 状態 | 意味 |
|---|---|
| **実装** | 通常buildの実行経路に接続され、対象を固定する回帰試験がある |
| **部分** | 実行経路はあるが、互換契約の一部、後段、またはfocused testが欠ける |
| **表面のみ** | primitiveやregisterは存在するが、その値が本来起こす動作は未接続 |
| **未実装** | 通常buildに機能がない |
| **設計のみ** | 文書または内部予約だけがあり、利用者が使える実装ではない |

「独自」も二つに分ける。

- **PraTeX独自機能**: 既存TeX engineとの互換を主目的にしない、PraTeX固有の意味や
  host interface。
- **既存仕様の独立実装**: 機能の意味はe-TeX、pdfTeX、(u)pTeX、web2cなどに由来するが、
  その原実装を移植せず公開仕様と許可されたblack-box観測から書き直したもの。

## TeX82にない対応機能

### e-TeX互換

| 状態 | 機能 | 現在の範囲と境界 |
|---|---|---|
| 実装 | 式 `\numexpr`、`\dimexpr`、`\glueexpr`、`\muexpr` | 内部量として数値・寸法・通常糊・数式糊を走査する。括弧、優先順位、e-TeXの丸め、糊の次数を扱う |
| 実装 | 保護macro `\protected` | `\edef`、`\message`、mark本文などの展開走査で保護し、通常実行では展開する。alignmentの`\noalign` / `\omit`先読みは通常macroだけを展開し、protected macroを欄・行の通常入力として残す。fmt往復と専用process試験を含む |
| 実装 | token/展開操作 `\detokenize`、`\unexpanded` | 入れ子のgeneral-text走査を壊さず、外側のtoken蓄積を保つ |
| 実装 | 条件 `\ifdefined`、`\ifcsname`、`\unless` | 未定義制御綴を副作用で作らない。`\unless`は対応する条件を反転する |
| 実装 | 条件 `\iffontchar` | 8-bit TFMの文字存在判定へ接続し、範囲外番号は中央scannerの診断後にcode 0へ回復する。存在するcode 0、欠落字、負数、256をprocess試験済み |
| 実装 | font寸法 `\fontcharwd/ht/dp/ic` | font identifierと0--255の文字番号を既存scannerで読み、8-bit TFMの幅・高さ・深さ・italic correctionを共通typed queryから内部寸法として返す。欠落字・nullfontは0pt、範囲診断、`\the`/`\number`/式・fmtを自作TFMで試験済み |
| 部分 | 内省 `\currentgrouplevel`、`\currentgrouptype`、`\currentiflevel`、`\currentiftype`、`\currentifbranch` | e-TeX番号で現在のgroupと条件状態を返し、`\unless`条件の負符号を入れ子から復元する。複雑なgroup/conditional組合せの網羅が残る |
| 実装 | `\lastnodetype` | node種類の追跡と内部整数化に加え、page→nested list→pageの復帰を保持する。空list、基本node型、page状態のfocused testを持つ |
| 実装 | `\eTeXversion`、`\eTeXrevision` | `\eTeXversion`は内部整数`2`、`\eTeXrevision`は展開可能なother文字`.6`を返す |
| 実装 | `\everyeof` | 実fileと`\scantokens`疑似fileの自然EOFだけで一度挿入し、`\endinput`では挿入しない。自然EOF内の次論理行番号も試験済み |
| 部分 | `\scantokens` | 未展開general textを`\escapechar` / `\newlinechar`で文字化したtyped疑似fileへ積み、読取時catcode/kcatcodeと行ごとの`\endlinechar`で再字句化する。暗黙groupを作らず、`\everyeof`、`\endinput`、行番号、fmt、開始時のtracing snapshotを試験済み。raw byte 10/13、二段診断context、資源超過、`\pausing`監査は残る |
| 実装 | `\readline` | `\read`と同じstreamから一行を読み、空白だけcatcode 10、その他をcatcode 12としてmacroへ定義する |
| 実装 | `\interactionmode` | 0〜3を読み書きし、batch/nonstop/scroll/errorstopを実際のlogger状態へ反映する |
| 実装 | register 0〜32767 | `count`、`dimen`、`skip`、`muskip`、`toks`、`box`の6種。0〜255は密、上位は使用分だけの疎storage。`\insert`は0〜254、box 255は別用途のまま |
| 実装 | mark class 0〜32767 | `\marks`、`\topmarks`、`\firstmarks`、`\botmarks`、`\splitfirstmarks`、`\splitbotmarks`。page遷移、`\vsplit`、fmtを含む |
| 実装 | 糊成分の照会と型変換 | `\gluestretch`、`\glueshrink`、`\gluestretchorder`、`\glueshrinkorder`、`\mutoglue`、`\gluetomu`。normal/fil/fill/filll、負値、零係数、数式糊の回復、fmtを扱う |
| 部分 | tracing register | `\tracingscantokens`は疑似fileの開始時に判定し、値が途中で変わっても対応する括弧を閉じる。他の`\tracingassigns`、`\tracinggroups`、`\tracingifs`、`\tracingnesting`は値の代入・group・fmtだけ |
| 実装 | `\savinghyphcodes` | 正値の`\patterns`時に現在の`\lccode`をlanguage別にsnapshotし、pattern圧縮後の通常hyphenationと`\hyphenation`例外へ適用する。同一languageの正値は置換し、0以下は既存snapshotを保持する。e-TeXのdense 8-bit表とPraTeX Latin-UCS拡張を別型にし、fmt検証を含む |
| 表面のみ | その他の組版制御register | `\predisplaydirection`、`\lastlinefit`は値を保持するだけで、display方向・last-line fitは未接続 |
| 実装 | token列表示 `\showtokens` | general textの入口だけを展開して左braceを探し、balanced text本体は未展開token列のまま既存`token_show`で表示する。外側braceを除外し、入れ子、parameter token、和文token、全mode、101回の非加算、`\let` alias、fmt往復、JFMのnode-less境界をprocess試験済み。公式Web2Cのexit 1に対してPraTeXが0を返す全診断共通のCLI差分は別残件 |
| 未実装 | group/if拡張表示 | `\showgroups`、`\showifs` |
| 実装 | parshape照会 `\parshapelength/indent/dimen` | 現在のpair数、各行のindent・length、奇偶interleaveを内部寸法として返す。非正index、最終pair反復、式・表示、fmtを含む |
| 実装 | 可変delimiter列 `\middle` | `\left`--`\right`内をsegmentごとのsave groupに分け、局所状態を復元して元のmath styleから次のlistを始める。全segmentの最大height/depthを全delimiterへ共有する。境界の左はRight、右はLeft相当のspacingとし、文字・数値delimiter走査、欠落・不対応時の回復、表示、fmtをprocess試験する |
| 実装 | penalty配列 | `\interlinepenalties`、`\clubpenalties`、`\widowpenalties`、`\displaywidowpenalties`。正の個数と整数列、0以下のreset、局所／大域代入、内部照会、fmtを持ち、通常段落とdisplay直前のpost-line-break penaltyへ接続する |
| 実装 | discard保存 | 正の`\savingvdiscards`でpage builderと公開`\vsplit`が捨てた先頭glue・kern・penaltyを別々のrun-local listへ保存する。`\pagediscards` / `\splitdiscards`は`\unvbox`同様に一度だけ現在のvertical listへ移し、output routine終端／各`\vsplit`開始時の消去、非正値、fmt identityをprocess試験する |
| 部分 | TeX--XeT組版 | `\TeXXeTstate`は値を保持しfmt読込時0へ戻す。正値の明示restricted hbox内で`\beginL`/`\endL`/`\beginR`/`\endR`をtyped方向nodeとし、入れ子LR stackをbackend共通で明示反転して通常DVI/PDFへ書く。inline mathはatomic LTR。通常hlistは単一pass・allocation 0、LR treeは最後に一回だけ平坦化する。disc/alignmentでの直接利用と方向hboxのunboxは未対応listへ漏らず診断する。RTL区間がdiscを含む時は部分反転せず方向変換全体を破棄。paragraph、line packing、display、math mode内方向primitiveは未実装 |

実装と試験の主な場所は
[primitive登録](../src/eqtb/primitives.rs)、
[e-TeX基本試験](../tests/etex.rs)、
[showtokens試験](../tests/etex_showtokens.rs)、
[式試験](../tests/etexexpr.rs)、
[mark試験](../tests/etex_marks.rs)、
[糊成分試験](../tests/etex_glue.rs)、
[糊変換試験](../tests/etex_glue_conversion.rs)、
[font寸法試験](../tests/etex_fontchar.rs)、
[条件照会試験](../tests/etex_condition_queries.rs)、
[parshape照会試験](../tests/etex_parshape.rs)、
[`\middle`試験](../tests/etex_middle.rs)、
[`\scantokens`試験](../tests/etex_scantokens.rs)、
[vertical discard試験](../tests/etex_vdiscards.rs)、
[TeX--XeT restricted hbox試験](../tests/etex_texxet_restricted.rs) である。仕様からの書き直し方は
[e-TeX移植記録](etex-port-notes.md)、完全性とTeX--XeTの監査は
[e-TeXとTeX--XeTの対応状況](etex-texxet-status.md) に記録している。

### pdfTeXおよび後発engine互換

| 状態 | 機能 | 現在の範囲と境界 |
|---|---|---|
| 実装 | `\expanded` | general textを完全展開して現在位置へ戻す。外側の走査状態を保つ |
| 実装 | `\pdffilesize` | 展開した論理名をTeX file resolverへ通し、見つかったfileのbyte数を返す。不在・起動不能では空に展開する |
| 実装 | `\pdfmdfivesum` | general textまたはresolverで解決した`file{...}`の全byte列をincremental MD5へ流し、大文字hexで返す。不在・読取不能は空展開。暗号用途ではない |
| 実装 | `\pdfstrcmp` | 展開後の二つのgeneral textをbyte辞書順で比較し、`-1`、`0`、`1`を返す |
| 部分 | PDF文字列変換 | `\pdfescapehex`、`\pdfunescapehex`、`\pdfescapestring`、`\pdfescapename`のbyte変換は実装。focusedなprocess試験は`\pdfescapehex`中心で、残りの互換性検証は薄い |
| 対応 | `\year` / `\month` / `\day` / `\time`、transcript、`\pdfcreationdate` | run開始時のlocal clockを一度だけ共有する。`SOURCE_DATE_EPOCH`はUTC固定。不正値はfallbackせず、非Windows/Unix targetはhost clockがなければ固定epochを要求する |
| 実装 | `\pdfshellescape` | 読み取り専用内部整数。PraTeXはshell escapeを提供しないため常に`0`で、processを起動しない |
| 部分 | PDF 1.4直接出力 | `-output-format=pdf` / `--output-format=pdf`。page tree、rule、printable ASCIIの暫定Courier表示、明示mapによるType 1全埋込み、明示profileによる横組JFM/BMPの非埋込みType 0/CIDFontType0、Dvips互換`papersize` specialまで。外部DVI driverなしでfileを閉じられる |
| 実装 | PDF `papersize` special | 第一page内で最後の`papersize=width,height`を採用し全pageへ継承する。九つの公開TeX単位をsp基底の既約整数比で保持し、`\mag`非依存でMediaBoxへ一度だけ丸める。壊れた認識済み形式と第二page以降の変更は拒む |
| 部分 | `--pdf-font-map` | 明示したmapだけでType 1埋込みを有効化する。実配布mapの複数resourceと分離markerを未使用entryごと拒まず、選択TFMだけを検査する。`<<font.pfb`の全埋込みだけを受け、`<font.pfb`のsubset要求を勝手に全埋込みへ変えない |
| 部分 | `--pdf-japanese-cid-profile` | 明示物理pathから一JFM用profileを64 KiB上限で一回だけ読み、JFM名一致時だけ`UniJIS-UCS2-H` / Adobe-Japan1-4へ結ぶ。BMP source codeを元Unicodeへ戻す限定`/ToUnicode`を持つがFontFileはなく表示はviewer依存。profileなし・名不一致・非BMPをtofuへfallbackしない |
| 実装 | Type 1 FontDescriptor fallback | mapのflags省略は公開pdfTeX契約の既定値4。AFMの`StdVW`省略時はPFB eexecを実行せずstream復号し、Private辞書の値だけを`StemV`へ使う。固定値推測はしない |

PDF直接出力はpdfTeX互換を名乗れる段階ではない。`\pdfoutput`、page-size primitive、PDF
object/link/destination/image primitive、font subset、汎用ToUnicode、TrueType/OpenType、埋込みCID font、
OTF shapingは未実装である。現在のnamed CIDはJFM幅で位置を進めるだけで字形を埋め込まず、
表示はviewer側BaseFontに依存する。詳しい境界は
[pdfTeX互換層](pdftex-port-notes.md)、
[PDF backend](pdf-backend-notes.md)、
[欧文process試験](../tests/pdf_output.rs)、
[和文process試験](../tests/pdf_japanese_output.rs) を参照する。

### pTeX/upTeX互換の入力層

| 状態 | 機能 | 現在の範囲と境界 |
|---|---|---|
| 実装 | 和文寸法単位 `Q`、`H` | どちらも厳密に0.25 mm。通常寸法、糊、式の既存寸法走査を通る |
| 部分 | 和文寸法単位 `zw`、`zh` | current横組JFMを選んだ時はscale済みclass 0の幅、height+depthを返す。未選択時だけ従来の欧文font `em`へ戻る。縦組metricは未接続 |
| 部分 | `\kanjiskip`、`\xkanjiskip`と自動間隔制御 | INITEX既定0の通常glue parameterに加え、`\autospacing` / `\noautospacing`、`\autoxspacing` / `\noautoxspacing`、`\xspcode`、Unicode scalarの`\inhibitxspcode`をtyped eqtbへ持つ。代入、group、`\globaldefs`、内部量、fmtを通す。直結和和Kは寸法・改行・DVIに効くが`\showbox` / `\lastskip` / `\lastnodetype` / `\unskip`から隠れる仮想node、Xはmaterial nodeである。確認済みunshifted hbox edgeはmaterial K/Xへ接続。discは左を遮断し、no-break/post-break末尾から右glyphへのK/Xを枝別material nodeとして保持する。shifted/vboxはbarrier |
| 部分 | JFM reader/modelと横組font | 公開JFM仕様から独立実装。横組11／縦組9、24-bit raw文字code、u8 class、skip・再配置・256超glue/kern indexをboundedに検査する。横組11はbounded loader、TeX互換scale、current font、`\pratexjfont`と意味一致範囲の`\jfont`、group/fmtへ接続済み。`\tfont`と縦組は未接続 |
| 部分 | 横組JFM/K/X/禁則hybrid | WideChar/Char/Ligature境界を中央plannerで一度だけ決め、同一fontのJFM pair glue/kernをKより優先する。JFM/禁則はmain loop、K/Xはclose-time snapshotでmaterializeする。`{}`、`\relax`、`\unskip`、`\message`、semi-simple group、`\showthe`、整数register代入を公式e-upTeXと照合し、削除済みJFMをcloseで復活させない。discのpre/post/no-breakを独立finalizeし、packer・line breaker・DVIで同じ枝を選ぶ。由来をunbox、line break、box寸法、DVI座標、fmtへ保持。禁則は`、。`とJLReq由来の横組括弧12対のbounded subsetで、shifted/vbox・discの全JFM class/禁則matrix・未検証command境界と完全禁則は残る |
| 部分 | PraTeX和文NFSSとrelation font | `PJY1`をPraTeX固有の横組契約として、和文encoding/family/series/shape、横組exact JFM shape宣言、JFM名＋exact-sp size cache、group復元を持つ。relation fontは標準NFSS本体でなくpLaTeXがNFSS上へ加えた拡張の意味をPraTeX固有名で実装し、Declare(global)／Set(local)、document bodyで次の`\selectfont`一回だけのUse、空jshape wildcardを持つ。public Useのpreamble利用、pLaTeX互換名、size function、shape substitution、縦組directionは未実装 |
| 実装 | `\kcatcode`表・照会・代入 | 公開値14〜20。U+0000〜U+10FFFFをUnicode 17.0.0のblock、upTeX擬似境界、7例外集合で保存する。block単位の局所/global/globaldefs復元とfmt往復を含む |
| 実装 | `latin_ucs`（kcatcode 14） | U+0080〜U+2E7FをUnicode欧文一文字tokenとして保持し、cat/lc/uc/sf、group/fmt、active/control identity、特殊catcode、case変換、表示へ通す。pattern/exception/trieもu16 alphabetで一文字として扱う。runtime namespaced Unicode active生成とwide font nodeは後段 |
| 部分 | UTF-8 CJK一文字tokenと横組glyph | kcatcode 16〜20を符号位置と入力時categoryを持つ一tokenにし、macro、`\edef`、`\let`、条件、`\string`、`\detokenize`、`\write`、fmtまで保持する。current横組JFMがあればUnicode・JFM class・scale済みmetricを持つwide nodeにし、DVIはBMPを`set2`、補助面を`set3`、PDFは明示named CID profileがあるBMPだけをType 0へ出す。未選択、縦組、math、PDF非BMPは明示診断する |
| 実装 | Unicodeを含むtyped制御綴 | `Byte(u8)`と`Unicode(u32)`を別identityにし、同じ見た目のraw UTF-8 byte名とwide名を混同しない。CJK categoryはtokenには固定するが制御綴identityには含めない |
| 部分 | upTeX互換UTF-8 decoder | 公式black-box観測に合わせ、overlong・surrogateを含む入力規則と不正列の一byte再同期を実装。入力上限はU+10FFFE、表とtokenはU+10FFFFまでで、upTeX独自の0x110000以上は扱わない |

CJK token、K/X parameter、横組JFM glyphとBuiltIn最小spacing基線だけで「日本語組版対応」とはしない。
横組のmetric付きglyph、DVI `set2`/`set3`、hybrid JFM/K/X、句読点と横組括弧12対の禁則、viewer依存のnamed CID PDFは
生成できるが、`\tfont`、main-loopのshifted/vbox・未検証command境界、discの全matrix、完全禁則、
縦組、portableな埋込みPDF和文字形は未実装である。
`\kchar`、`\kchardef`、`\ucs`、`\forcecjktoken`もまだない。`\uppercase`/`\lowercase`は
CJK tokenを現在変更しない。

根拠と試験は
[e-upTeX移植記録](euptex-port-notes.md)、
[kcatcode実装](../src/eqtb/kcatcodes.rs)、
[kcatcode試験](../tests/kcatcode.rs)、
[CJK token試験](../tests/cjk_tokens.rs)、
[和文単位試験](../tests/jdimen.rs) にある。

### file探索とhost実行機能

| 状態 | 機能 | 現在の範囲と境界 |
|---|---|---|
| 部分 | kpathsea/web2c相当の探索 | 別engine名を使わずprogram名`pratex`をカノンとし、直接pathを優先する。Scanner、Output、直接PDF loaderが一つのrun-local resolverとpositive/negative cacheを共有する。Linux既定は公式TeX Live 2026 Kpathsea 6.4.2を静的buildしたsubprocess禁止C API境界でTEX/TFM/JFM等をin-process探索し、source/build失敗時にCLIへ黙って退行しない。外部fmtの`--engine=rtex`だけは既存safe経路を保つ。明示`system-kpathsea` build、Windows等のtyped CLI/WSL fallbackも残す。VFはDVI driverのconsumerなのでPraTeXが先読みしない。合成treeで子process 0・local/tree DVI byte一致、固定CTAN treeで15組のprocess 0とDVI一致を確認済み |
| 部分 | Windows--WSL TeX Live bridge | native `kpsewhich`の起動fileがない時だけ既定WSLへ移り、選んだbackendをrun-local resolver内で固定する。Linux絶対pathと探索pathを検証つきUNCへ写し、nativeの不在回答や異常を別TeX Liveで覆わない。allocator/CRT境界が未監査なのでWindowsへin-process Kpathseaを有効化しない |
| 実装 | 論理名と物理pathの分離 | 解決したTEXMF上のpathをTFM名、DVI font名、PDF map keyなどの論理identityへ漏らさない |
| 部分 | OS固有file名 | CLIは`args_os`、Unixの非UTF-8名はbyteのまま、WindowsのUnicode名は`OsString`で運ぶ。単一argv中の空白やWindows絶対pathをTeX入力としてどう引用するかは未解決 |
| 実装 | CRLF入力 | `\r\n`を一つの行末として読み、出力先名などへ`\r`を混ぜない |
| 実装 | 引用符つきfile名 | `\openin0="target file.tex"`の引用符を名前から除き、空白を含む論理名として扱う |

Linux既定は監査済みforkを介してKpathsea C APIを使うが、PraTeXの通常sourceはsafe Rustだけである。
既定Linux以外の段階的fallbackが読む`ls-R`索引は完全なKpathsea再実装ではなく、`texmf.cnf`の
全展開、case folding、`mktex*`等は未完成である。Kpathsea source取得、LGPL source/relink条件、
offline build、再現性、binary sizeは配布gateとして残る。実装と合成試験は [file resolver](../src/file_search.rs) と
[入力接続試験](../src/input/file_search_tests.rs)、対応境界は
[TeX Live探索の移植記録](kpathsea-port-notes.md) にある。

## PraTeX独自機能

この節は、互換元のprimitiveを単にRustで書き直したものではなく、PraTeX固有の利用者向け
契約を持つものだけを挙げる。

| 状態 | 独自機能 | 現在の契約 |
|---|---|---|
| 部分 | Vaak bridge | `\directvaak{...}`、`\vaakdef\cs{...}`、`\vaakinput file`。終了値を10進整数として展開し、低位`count[0..255]`と`dimen[0..255]`をi32として読み書きする。書戻しはTeXの保存stackを通りgroupで戻る。同一sourceの`PreparedProgram`と`EmbeddingRunner`を再利用し、固定`HostLayout`の順序・型・名付き型schemaを実行前に照合する |
| 部分 | 名前空間つき制御綴 | catcode 16、`\namespace`、`\usingnamespace`、`\namespacechar`。一文字・複数文字・active・Unicode制御綴、group/global、参照時探索、表示、fmtまで実装。interface名の確定、`\halign` preamble再利用などPhase 8の検証は残る |
| 実装 | typedな組版region状態 | `\pratexregion=0..5`で`und`、`ja`、`zh-Hans`、`zh-Hant`、`ko`、`vi`を選ぶ。local/global/globaldefs、save stack、fmt、`\the`、`\showthe`、`\meaning`、`\let`を通し、TeX `\language`から独立する。BuiltIn既定規則自体はまだregionで変わらないが、明示install済みcompiled tableのlist一貫性keyとして使う |
| 実装 | PraTeXの実行名 | primary/default binaryとbannerは`pratex`。移行用の`rtex`互換binaryは残す。crate/library名も現在は`rtex`のまま |
| 実装 | 自動出力だけを抑える`--quiet` | banner、file括弧、page番号、通常summaryだけをterminalから消す。log、明示`\message`、`\write16`、`\show`、tracing、error、promptは残す |
| 実装 | Type 1埋込みの明示opt-in | `--pdf-font-map`を指定した時だけ限定map parserと埋込み経路を有効にし、暗黙のresource選択を避ける |
| 部分 | 生文字列register | `\rawstring`、`\rawstringdef`、`\therawstring`、専用`\showthe`、群・`\globaldefs`、fmtを持つ。任意byte列を`Rc<Vec<u8>>`でtoken列と分離し、1 slot 16 MiB、active/future slot全体64 MiBに制限する。literal/file producerと`\the\rawstring`の改行契約は未実装 |

Vaak言語runtimeそのものは別repositoryのMIT依存であり、PraTeX独自なのはTeXとのbridge層で
ある。Vaak側のprepared/layout APIは接続済みだが、現bridgeが公開する値は低位count/dimenだけで、
`tex.print`、PraTeX host function、run-local capabilityはまだない。根拠は
[Vaak実装](../src/vaak.rs)、
[統合設計と測定](vaak-integration.md)、
[名前空間roadmap](NAMESPACE_ROADMAP.md)、
[組版region試験](../tests/language_region.rs)、
[拡張可能なscript境界組版](extensible-layout-roadmap.md)、
[quiet試験](../tests/quiet_mode.rs) にある。

## 完全に独立して書いた実装の範囲

この表の「独立」は、PraTeXで追加したcodeについて互換元の実装sourceを移植せず、公開仕様・
標準・許可されたblack-box観測から書いた、という意味である。機能の発明元までPraTeXだと
いう意味ではない。

| 実装範囲 | 機能の意味の出自 | PraTeX側で独立して書いたもの |
|---|---|---|
| e-TeX互換層 | e-TeX公開manual | 式scanner、protected展開、token操作、条件・内省、EOF/line入力、extended register、mark class、glue照会とfmt表現 |
| pdfTeX互換primitive | pdfTeX公開manual | general-text走査との接続、文字列変換、file照会、比較、shell無効状態、RFC 1321に基づく専用MD5 |
| PDF直接出力 | PDF 1.4、pdfTeX manual、Adobe font仕様 | object/stream/xref/trailer serializer、page tree、固定小数座標変換、content backend、型付きfont object |
| Type 1 resource処理 | Adobe PFB/AFM/Type 1/PDF仕様、pdfTeX・dvips map契約 | bounded PFB、AFM、順序付き複数resource map、encoding vector parserとloader。eexec Private `StdVW`をstream復号するが、PostScriptやfont programを実行しない |
| upTeX互換入力層 | upTeX 2.02公開文書、Unicode 17.0.0、公式binaryのblack-box観測 | block型kcatcode表、専用decoder、packed CJK token、typed制御綴key、表示・fmt伝播 |
| kpathsea相当resolver | Kpathsea公開manualと`kpsewhich` CLI契約 | logical/physical name型、用途別query、bounded `ls-R` reader、保守的な探索path照合、process境界、positive/negative cache、異常応答の分類、nativeとWSLを混ぜないUNC bridge |
| Vaak bridge | Vaak公開Rust API | TeX general text/file/定義をVaak VMへ渡すadapter、触れたregisterだけのsnapshotと保存stack経由の書戻し |
| 名前空間 | PraTeX固有仕様 | catcode 16から字句・eqtb・探索・表示・fmtまでのnamespaced control-sequence経路 |
| 出力backend境界 | PraTeX内部設計 | 一度のnode walkをDVI/PDF eventへ流す静的`ShipoutBackend`と、既存DVI writerのbyte互換adapter |
| 文字分類境界 | PraTeX内部設計 | `CatCode`を`repr(u8)`に保ち、`\catcode`をカノン、`\kcatcode`を別公開番号の互換viewとして`InputCategory::{CatCode, Wide, RawBytes}`へ写す`CharacterClassifier`問い合わせ面。layout/JFM/provider IDは別domain |
| 組版region状態 | PraTeX内部設計 | 地域組版localeをTeXのhyphenation番号やUnicode scriptから推定せず、1 byteのtyped parameterとしてeqtb/save stack/fmt/内部量へ通す |
| CLIとportability層 | web2c系option慣行とPraTeX固有policy | OS文字列を壊さないoption parser、PDF/quiet選択、CRLF処理、PraTeX/rtex二binary境界 |
| 検証・性能tooling | TRIP、公開DVI仕様 | 第三者資材をvendorしないhash固定TRIP runner、DVI意味比較、safe Rustの回帰・性能fixture。fmt bounded予約のWindows warm A/B 48標本は[`benchmarks/fmt-bounded-reservation-20260824.csv`](benchmarks/fmt-bounded-reservation-20260824.csv)に固定し、Linux 9.14 sのend-to-end差を解消した値とは扱わない |

低水準PDF、font、resolverの主なsourceは
[pdf.rs](../src/pdf.rs)、
[pdf_document.rs](../src/pdf_document.rs)、
[pdf_backend.rs](../src/pdf_backend.rs)、
[font_resources](../src/font_resources.rs)、
[output_backend.rs](../src/output_backend.rs)、
[file_search.rs](../src/file_search.rs) である。

PraTeX全体が一からの独立実装という意味ではない。TeX82中核はtyti氏のrtexを継承し、Vaak
runtimeは別MIT projectを依存として使う。PraTeX側からMITのVaakへGPL codeを移さない。

## 存在するが、まだ対応機能ではないもの

次は要求または設計があっても、現時点の対応一覧には含めない。

| 状態 | 項目 | 現在地 |
|---|---|---|
| 設計のみ | run-local Vaak疑似callback | 特定のVaak実行が明示要求した時だけ有効にする方針だけ。常設callback表はない |
| 部分的な内部基盤 | version付きWASM ABI | 実験[ABI 0.0](wasm-provider-abi-v0.md)で四operation、固定mailbox、capability、fuel、atomic fallbackを定義。`SpacingTableUpload`を全件検証し、canonical候補を共通native表へcompile完了後だけrun-local dispatcherへinstallするhost入口まで接続。module profile/export検査、runtime、affine lease、実provider登録はない |
| 部分 | PraTeX自身のWASI target | `wasm32-wasip1`へcheck・binary linkし、`pratex.wasm` / `rtex.wasm`を生成できる。現状はargs、stream、preopen filesystem、process exitを使うcommand moduleであり、runtime適合試験、子processなしresolver、host API/VFS、native DVI比較は未達。詳細は[WASM target監査](wasm-target-status.md) |
| 部分 | script境界組版とregion R1〜R7 | 横組JFM/BuiltInに加え、明示run-local compiled tableをlistごとに一度選び、安定したdirect glyph境界のclass pairをfixed/K/X/no-spaceと限定boundary glue/penaltyへmaterializeする。標準日本語はregistry/callback 0。RegionNode、indirect box/disc、tier/line-edge、公開Vaak/runtime登録とWASM batchは未実装 |
| 設計のみ | IVS・外字・造字のidentity | inline Unicode scalarと`AtomRef`、namespaceつき外部文字、嘘字/TRON importer、variant graphの[設計](glyph-identity-roadmap.md)だけ。現在はIVS shapingも造字もない |
| 設計のみ | 拡張可能な寸法単位 | registry、Vaak table、WASM providerの[設計](extensible-dimension-units-roadmap.md)だけ。組込み`Q/H/zw/zh`の現行経路とは分ける |
| 設計のみ | 監視・incremental・package取得・LSP | epoch、checkpoint、managed overlay、実行経路上のsemantic eventの[roadmap](incremental-tooling-roadmap.md)だけ。daemon/LSP serverはない |
| 設計のみ | LaPraTeX | 公式LaTeX sourceから分離したclean-room formatの[roadmap](lapratex-roadmap.md)だけ。`lapratex.ltx` / `lapratex.fmt`はない |
| 部分的な内部基盤 | 外部文字分類provider | 組込み`CharacterClassifier`と別domain IDまではあるが、通常buildの外部provider、registry、cache、generation管理はない。callback adapterはtest専用 |
| 未実装 | `^^^^hhhh`、`^^^^^^hhhhhh` | TeX82の`^^`だけ。XeTeX/LuaTeX型の4/6 caret Unicode入力はない |
| 未実装 | Web2C TCX input translation | `--translate-file`、`%& -translate-file`、`-8bit`、TCXの`xord/xchr/xprn`三表はまだない。既定UTF-8と分けたlegacy input profileは[文字identity roadmap](glyph-identity-roadmap.md)で設計のみ |
| 未実装 | OTF/TrueTypeとRustyBuzz | dependencyもbackendもない。JFM/TFM出力基線後にdefault-offで接続し、PraTeX側はsafe Rust、依存のlicense・unsafe利用・binary sizeを採用前に監査する |
| ロードマップ | PraTeX-native OpenType package | native OTF loader・metric・shaping完成後、PraTeX固有feature queryで`fontspec`上位互換面を作る。和文NFSS/JFM、文字class、exact code point、Unicode範囲・面、`LanguageRegion`、fallback chain別のfont routingを宣言時にhost tableへcompileする。同じHan scalarのja/zh-Hans/zh-Hant地域字形、OpenType language/`locl`、regionを跨がないfallback、元scalarを保つToUnicode、Babel言語区間adapterをcompletion gateにする。単一packageか二層かはAPI実験で決め、XeTeX/LuaTeXのversion primitiveを偽装しない。詳細は[OpenType package roadmap](opentype-package-roadmap.md) |
| ロードマップ | native絵文字 | 通常OTF loader・shaping・fallback・PDF subset完成後に行う。plain UTF-8のVS15/VS16、modifier、flag、keycap、tag、ZWJ sequenceを壊せないclusterとして扱い、cluster単位fallback、COLR/CPAL等のcolor glyph、縦横組、PDF ToUnicode/ActualTextを検査する。現状は入力・node・font・出力とも未実装。詳細は[native emoji roadmap](emoji-native-roadmap.md) |
| 部分 | e-TeX完全性gate | `\showgroups`、`\showifs`、未接続tracing、`\lastlinefit`、TeX--XeTのparagraph・display・math mode内方向・disc/alignment/unbox等が残る。vertical discard listと四方向primitiveの明示restricted hbox限定sliceは実接続済み |
| 部分 | 横組・縦組の日本語組版 | 横組JFM font、wide node、main-loop JFM pair/禁則、close-timeの仮想K・material X、discの枝内K/Xと右端条件付きmaterial K/X、句読点＋横組括弧12対の禁則、box/line幅、DVI glyphまでのBuiltIn基線を実装。main-loopのshifted/vbox・未検証command境界、discの全JFM class/禁則matrix、完全JLReq、paragraph方向node、縦組が残る |
| 部分 | class/package互換 | `article`、宣言的和文NFSS/relation fontを持つ`prjsarticle`、`pratex-japanese`を明示したKOMA-Script 3.49.2 `scrartcl`、`graphicx`、`xcolor`、`hyperref`、TikZ/PGF、`siunitx`の限定smokeをDVIまで実測。BSD 2-Clauseの上流`jlreq`から権利表示を保って派生した最小横組`prjlreq` v0.1も持つが、現在統合枝では静的契約試験だけを再実行済み。package全API、`jsarticle`、上流`jlreq`、`ltjsarticle`の実用互換を保証しない |

文字分類の実装済み境界と未実装部分は
[拡張可能な文字分類器](character-classifier-extension.md) に分離している。

## 更新規則

- production sourceに登録しただけでは「実装」に上げず、意味を固定する回帰試験を置く。
- 設計文書だけの機能は必ず「設計のみ」に置く。
- 互換元の機能とPraTeX独自機能を同じ「独自」の語で混ぜない。
- 対応範囲が広がったcommitでは、この文書、該当port note、READMEの三者を同時に確認する。

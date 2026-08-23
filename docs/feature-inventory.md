# PraTeXのTeX82外機能と独立実装

更新: 2026-08-23
監査対象: `codex2/jlreq-script-spacing`（基点`6ce8315`）

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
| 部分 | 保護macro `\protected` | `\edef`、`\message`、mark本文などの展開走査で保護し、通常実行では展開する。fmt往復を含むが、alignmentの`\noalign` / `\omit`先読みでは誤って展開する |
| 実装 | token/展開操作 `\detokenize`、`\unexpanded` | 入れ子のgeneral-text走査を壊さず、外側のtoken蓄積を保つ |
| 実装 | 条件 `\ifdefined`、`\ifcsname`、`\unless` | 未定義制御綴を副作用で作らない。`\unless`は対応する条件を反転する |
| 部分 | 条件 `\iffontchar` | TFMの文字存在判定へ接続済み。専用のprocess-level回帰試験はまだない |
| 部分 | 内省 `\currentgrouplevel`、`\currentgrouptype`、`\currentiflevel`、`\currentiftype`、`\currentifbranch` | e-TeX番号で現在のgroupと条件状態を返す基本経路はあるが、`\unless`で開始した条件の`\currentiftype`が負にならない |
| 実装 | `\lastnodetype` | node種類の追跡と内部整数化に加え、page→nested list→pageの復帰を保持する。空list、基本node型、page状態のfocused testを持つ |
| 実装 | `\eTeXversion` | 内部整数として`2`を返す |
| 部分 | `\everyeof` | 一つの実file入力源につき挿入する経路はあるが、自然EOFと`\endinput`を区別せずforce EOFにも挿入する。`\scantokens`疑似fileと行番号も未達 |
| 実装 | `\readline` | `\read`と同じstreamから一行を読み、空白だけcatcode 10、その他をcatcode 12としてmacroへ定義する |
| 実装 | `\interactionmode` | 0〜3を読み書きし、batch/nonstop/scroll/errorstopを実際のlogger状態へ反映する |
| 実装 | register 0〜32767 | `count`、`dimen`、`skip`、`muskip`、`toks`、`box`の6種。0〜255は密、上位は使用分だけの疎storage。`\insert`は0〜254、box 255は別用途のまま |
| 実装 | mark class 0〜32767 | `\marks`、`\topmarks`、`\firstmarks`、`\botmarks`、`\splitfirstmarks`、`\splitbotmarks`。page遷移、`\vsplit`、fmtを含む |
| 実装 | 糊成分の照会と型変換 | `\gluestretch`、`\glueshrink`、`\gluestretchorder`、`\glueshrinkorder`、`\mutoglue`、`\gluetomu`。normal/fil/fill/filll、負値、零係数、数式糊の回復、fmtを扱う |
| 表面のみ | tracing register | `\tracingassigns`、`\tracinggroups`、`\tracingifs`、`\tracingscantokens`、`\tracingnesting`は値の代入・group・fmtだけ。対応するtrace出力は未接続 |
| 表面のみ | 組版制御register | `\predisplaydirection`、`\lastlinefit`、`\savingvdiscards`、`\savinghyphcodes`、`\TeXXeTstate`は値を保持するだけで、組版・discard保存・TeX--XeT動作は未接続。`\TeXXeTstate`だけはfmt読込時0へ戻す |
| 未実装 | 疑似入力と表示 | `\scantokens`（[clean-room設計済み](scantokens-design.md)）、`\showtokens`、`\showgroups`、`\showifs`、`\eTeXrevision` |
| 未実装 | font・段落・math照会 | `\fontcharwd/ht/dp/ic`、`\parshapelength/indent/dimen`、`\middle` |
| 未実装 | penalty配列とdiscard | `\interlinepenalties`、`\clubpenalties`、`\widowpenalties`、`\displaywidowpenalties`、`\pagediscards`、`\splitdiscards` |
| 未実装 | TeX--XeT組版 | `\beginL`/`\endL`/`\beginR`/`\endR`、LR node/stack、区間反転、line packing、DVI/PDF shipout |

実装と試験の主な場所は
[primitive登録](../src/eqtb/primitives.rs)、
[e-TeX基本試験](../tests/etex.rs)、
[式試験](../tests/etexexpr.rs)、
[mark試験](../tests/etex_marks.rs)、
[糊成分試験](../tests/etex_glue.rs)、
[糊変換試験](../tests/etex_glue_conversion.rs) である。仕様からの書き直し方は
[e-TeX移植記録](etex-port-notes.md)、完全性とTeX--XeTの監査は
[e-TeXとTeX--XeTの対応状況](etex-texxet-status.md) に記録している。

### pdfTeXおよび後発engine互換

| 状態 | 機能 | 現在の範囲と境界 |
|---|---|---|
| 実装 | `\expanded` | general textを完全展開して現在位置へ戻す。外側の走査状態を保つ |
| 実装 | `\pdffilesize` | 展開した論理名をTeX file resolverへ通し、見つかったfileのbyte数を返す。不在・起動不能では空に展開する |
| 部分 | `\pdfmdfivesum` | general textのMD5を大文字hexで返す。`file`形式は未実装。暗号用途ではない |
| 実装 | `\pdfstrcmp` | 展開後の二つのgeneral textをbyte辞書順で比較し、`-1`、`0`、`1`を返す |
| 部分 | PDF文字列変換 | `\pdfescapehex`、`\pdfunescapehex`、`\pdfescapestring`、`\pdfescapename`のbyte変換は実装。focusedなprocess試験は`\pdfescapehex`中心で、残りの互換性検証は薄い |
| 部分 | `\pdfcreationdate` | 形式は返すが、rtexから継承した固定日時`1776-07-04 12:00 UTC`であり実時計ではない |
| 実装 | `\pdfshellescape` | 読み取り専用内部整数。PraTeXはshell escapeを提供しないため常に`0`で、processを起動しない |
| 部分 | PDF 1.4直接出力 | `-output-format=pdf` / `--output-format=pdf`。page tree、rule、printable ASCIIの暫定Courier表示、明示mapによるType 1全埋込みまで。外部DVI driverなしでfileを閉じられる |
| 部分 | `--pdf-font-map` | 明示したmapだけでType 1埋込みを有効化する。実配布mapの複数resourceと分離markerを未使用entryごと拒まず、選択TFMだけを検査する。`<<font.pfb`の全埋込みだけを受け、`<font.pfb`のsubset要求を勝手に全埋込みへ変えない |
| 実装 | Type 1 FontDescriptor fallback | mapのflags省略は公開pdfTeX契約の既定値4。AFMの`StdVW`省略時はPFB eexecを実行せずstream復号し、Private辞書の値だけを`StemV`へ使う。固定値推測はしない |

PDF直接出力はpdfTeX互換を名乗れる段階ではない。`\pdfoutput`、page-size primitive、PDF
object/link/destination/image primitive、font subset、ToUnicode、TrueType/OpenType、Type 0/CIDFont、
OTF shapingは未実装である。詳しい境界は
[pdfTeX互換層](pdftex-port-notes.md)、
[PDF backend](pdf-backend-notes.md)、
[process試験](../tests/pdf_output.rs) を参照する。

### pTeX/upTeX互換の入力層

| 状態 | 機能 | 現在の範囲と境界 |
|---|---|---|
| 実装 | 和文寸法単位 `Q`、`H` | どちらも厳密に0.25 mm。通常寸法、糊、式の既存寸法走査を通る |
| 部分 | 和文寸法単位 `zw`、`zh` | 綴りは使えるが、JFMがcurrent和文fontへ未接続のため両方とも現在の欧文fontの`em`幅で代用する |
| 表面実装 | `\kanjiskip`、`\xkanjiskip` | INITEX既定0の通常glue parameterとして、代入、group、`\globaldefs`、算術、内部量、表示、fmtを既存の一経路へ通す。文字境界への自動挿入は未接続 |
| 部分 | JFM reader/model | 公開JFM仕様から独立実装。横組11／縦組9、24-bit raw文字code、u8 class、skip・再配置・256超glue/kern indexをboundedに検査し、class対をload時に直接表へcompileする。`\jfont`/`\tfont`、scale、wide nodeには未接続 |
| 実装 | `\kcatcode`表・照会・代入 | 公開値14〜20。U+0000〜U+10FFFFをUnicode 17.0.0のblock、upTeX擬似境界、7例外集合で保存する。block単位の局所/global/globaldefs復元とfmt往復を含む |
| 実装 | `latin_ucs`（kcatcode 14） | U+0080〜U+2E7FをUnicode欧文一文字tokenとして保持し、cat/lc/uc/sf、group/fmt、active/control identity、特殊catcode、case変換、表示へ通す。pattern/exception/trieもu16 alphabetで一文字として扱う。runtime namespaced Unicode active生成とwide font nodeは後段 |
| 部分 | UTF-8 CJK一文字token | kcatcode 16〜20を符号位置と入力時categoryを持つ一tokenにし、macro、`\edef`、`\let`、条件、`\string`、`\detokenize`、`\write`、fmtまで保持する。ただし組版時はJFM不足を診断して文字を捨てる |
| 実装 | Unicodeを含むtyped制御綴 | `Byte(u8)`と`Unicode(u32)`を別identityにし、同じ見た目のraw UTF-8 byte名とwide名を混同しない。CJK categoryはtokenには固定するが制御綴identityには含めない |
| 部分 | upTeX互換UTF-8 decoder | 公式black-box観測に合わせ、overlong・surrogateを含む入力規則と不正列の一byte再同期を実装。入力上限はU+10FFFE、表とtokenはU+10FFFFまでで、upTeX独自の0x110000以上は扱わない |

CJK token、K/X parameter、JFM readerは「日本語をPDF/DVIへ組める」という意味ではない。
和文font/node、DVI `set2`/`set3`、`\jfont`/`\tfont`、K/Xの自動挿入、禁則、縦組は未実装である。
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
| 部分 | kpathsea/web2c相当の探索 | 直接pathを優先し、用途別`--show-path`とrun-local `ls-R`索引で一意な候補を証明できれば外部processを省く。曖昧・stale・未対応ならshellを介さないone-shot `kpsewhich`へ戻す。TeX入力、`\openin`、`\pdffilesize`、TFM、map、encoding、Type 1、AFM、Vaak入力へ接続し、run内で成功・不在をcacheする |
| 部分 | Windows--WSL TeX Live bridge | production既定値ではnative `kpsewhich`の起動fileがない時だけ既定WSLへ移り、選んだbackendをresolver instance内で固定する。Linux絶対pathと探索pathを検証つきUNCへ写す。nativeの不在回答や異常を別TeX Liveで覆わない。ScannerとPDF loaderを跨ぐrun-global固定は未実装 |
| 実装 | 論理名と物理pathの分離 | 解決したTEXMF上のpathをTFM名、DVI font名、PDF map keyなどの論理identityへ漏らさない |
| 部分 | OS固有file名 | CLIは`args_os`、Unixの非UTF-8名はbyteのまま、WindowsのUnicode名は`OsString`で運ぶ。単一argv中の空白やWindows絶対pathをTeX入力としてどう引用するかは未解決 |
| 実装 | CRLF入力 | `\r\n`を一つの行末として読み、出力先名などへ`\r`を混ぜない |
| 実装 | 引用符つきfile名 | `\openin0="target file.tex"`の引用符を名前から除き、空白を含む論理名として扱う |

これはkpathsea C APIの実装ではない。`ls-R`と展開済み探索pathの保守的な部分集合だけを
読む。`texmf.cnf`、未展開変数・brace等を含む完全なpath expression、alias、case folding、
mktex生成は未実装である。一意性を証明できないcache missでは`kpsewhich` processを起動する。
実装と合成試験は [file resolver](../src/file_search.rs) と
[入力接続試験](../src/input/file_search_tests.rs)、対応境界は
[TeX Live探索の移植記録](kpathsea-port-notes.md) にある。

## PraTeX独自機能

この節は、互換元のprimitiveを単にRustで書き直したものではなく、PraTeX固有の利用者向け
契約を持つものだけを挙げる。

| 状態 | 独自機能 | 現在の契約 |
|---|---|---|
| 部分 | Vaak bridge | `\directvaak{...}`、`\vaakdef\cs{...}`、`\vaakinput file`。終了値を10進整数として展開し、低位`count[0..255]`と`dimen[0..255]`をi32として読み書きする。書戻しはTeXの保存stackを通りgroupで戻る。同一sourceの`PreparedProgram`と`EmbeddingRunner`を再利用し、固定`HostLayout`の順序・型・名付き型schemaを実行前に照合する |
| 部分 | 名前空間つき制御綴 | catcode 16、`\namespace`、`\usingnamespace`、`\namespacechar`。一文字・複数文字・active・Unicode制御綴、group/global、参照時探索、表示、fmtまで実装。interface名の確定、`\halign` preamble再利用などPhase 8の検証は残る |
| 実装 | typedな組版region状態 | `\pratexregion=0..5`で`und`、`ja`、`zh-Hans`、`zh-Hant`、`ko`、`vi`を選ぶ。local/global/globaldefs、save stack、fmt、`\the`、`\showthe`、`\meaning`、`\let`を通し、TeX `\language`から独立する。R0の状態だけで、まだ組版結果に影響しない |
| 実装 | PraTeXの実行名 | primary/default binaryとbannerは`pratex`。移行用の`rtex`互換binaryは残す。crate/library名も現在は`rtex`のまま |
| 実装 | 自動出力だけを抑える`--quiet` | banner、file括弧、page番号、通常summaryだけをterminalから消す。log、明示`\message`、`\write16`、`\show`、tracing、error、promptは残す |
| 実装 | Type 1埋込みの明示opt-in | `--pdf-font-map`を指定した時だけ限定map parserと埋込み経路を有効にし、暗黙のresource選択を避ける |

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
| 検証・性能tooling | TRIP、公開DVI仕様 | 第三者資材をvendorしないhash固定TRIP runner、DVI意味比較、safe Rustの回帰・性能fixture |

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
| 設計のみ | 生文字列register、`\therawstring`、raw専用`\showthe` | [設計文書](raw-string-registers.md)だけ。production primitive、storage、testはない |
| 設計のみ | run-local Vaak疑似callback | 特定のVaak実行が明示要求した時だけ有効にする方針だけ。常設callback表はない |
| 設計のみ | version付きWASM ABI | 実験[ABI 0.0](wasm-provider-abi-v0.md)で四operation、固定mailbox、capability、fuel、atomic fallbackまで定義。ABI export、runtime、providerはない |
| 設計のみ | script境界組版とregion R1〜R7 | `ScriptClassId`、RegionNode、JFM/wide glyph、spacing finalizer、Vaak table、WASM batchのroadmapだけ。R0の`\pratexregion`以外は利用できない |
| 設計のみ | IVS・外字・造字のidentity | inline Unicode scalarと`AtomRef`、namespaceつき外部文字、嘘字/TRON importer、variant graphの[設計](glyph-identity-roadmap.md)だけ。現在はIVS shapingも造字もない |
| 設計のみ | 拡張可能な寸法単位 | registry、Vaak table、WASM providerの[設計](extensible-dimension-units-roadmap.md)だけ。組込み`Q/H/zw/zh`の現行経路とは分ける |
| 設計のみ | 監視・incremental・package取得・LSP | epoch、checkpoint、managed overlay、実行経路上のsemantic eventの[roadmap](incremental-tooling-roadmap.md)だけ。daemon/LSP serverはない |
| 設計のみ | LaPraTeX | 公式LaTeX sourceから分離したclean-room formatの[roadmap](lapratex-roadmap.md)だけ。`lapratex.ltx` / `lapratex.fmt`はない |
| 部分的な内部基盤 | 外部文字分類provider | 組込み`CharacterClassifier`と別domain IDまではあるが、通常buildの外部provider、registry、cache、generation管理はない。callback adapterはtest専用 |
| 未実装 | `^^^^hhhh`、`^^^^^^hhhhhh` | TeX82の`^^`だけ。XeTeX/LuaTeX型の4/6 caret Unicode入力はない |
| 未実装 | Web2C TCX input translation | `--translate-file`、`%& -translate-file`、`-8bit`、TCXの`xord/xchr/xprn`三表はまだない。既定UTF-8と分けたlegacy input profileは[文字identity roadmap](glyph-identity-roadmap.md)で設計のみ |
| 未実装 | OTF/TrueTypeとRustyBuzz | dependencyもbackendもない。JFM/TFM出力基線後にdefault-offで接続し、PraTeX側はsafe Rust、依存のlicense・unsafe利用・binary sizeを採用前に監査する |
| 未実装 | 完全なe-TeX | `\eTeXrevision`、`\scantokens`、`\showtokens`、`\showgroups`、`\showifs`、`\middle`、`\fontcharwd/ht/dp/ic`、`\parshapelength/indent/dimen`、各種penalty配列、`\pagediscards`、`\splitdiscards`、`\beginL/\endL/\beginR/\endR`などが残る |
| 未実装 | 横組・縦組の日本語組版 | JFM、和文node/font、和文glue、禁則、方向node、縦組が残る |
| 未実装 | class/package互換の完成 | `jsarticle`、`jlreq`、`ltjsarticle`、`hyperref`を実用的に処理する保証はない |

文字分類の実装済み境界と未実装部分は
[拡張可能な文字分類器](character-classifier-extension.md) に分離している。

## 更新規則

- production sourceに登録しただけでは「実装」に上げず、意味を固定する回帰試験を置く。
- 設計文書だけの機能は必ず「設計のみ」に置く。
- 互換元の機能とPraTeX独自機能を同じ「独自」の語で混ぜない。
- 対応範囲が広がったcommitでは、この文書、該当port note、READMEの三者を同時に確認する。

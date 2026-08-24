# safe Rust 性能測定

## 合格条件: DVI modeでupLaTeXの1.2倍未満

PraTeXの性能合格条件は、単に「Rust実装として十分速い」ことではない。**DVI出力modeの
end-to-end実行時間を、同じ入力、同じTeX tree、同じcold/warm条件で動かすupLaTeXの
1.2倍未満に収める。** 意味が同等でないDVI、機能を省いた短いfixture、探索時間を別processへ
隠した値では合格にしない。日本語機能が増えるほど比較対象へ近づき、条件を満たす価値も難度も
上がるため、主要sliceごとに退行を止める。

合否判定にはprocess起動、native file探索、fmt復元、展開、組版、page build、DVI closeまでを
含むwall timeを使う。診断用にCPU timeと各区間も別列へ記録するが、engine部分だけを切り出して
end-to-endの失敗を打ち消さない。PraTeXだけが通常lookupごとに`kpsewhich`を子process起動したり、
upLaTeX側だけがcache済みだったりする非対称な標本は採用しない。

最低限、次を継続測定する。

- process起動、TeX Live探索、fmt復元、展開、段落整形、JFM class対処理、page build、
  DVI shipoutを分離したmicro/macro benchmark
- ASCII、和文、和欧混植、禁則が多い狭い段落、300頁級、横組、縦組の固定corpus。
  短文・40頁・100頁は起動費と傾きを分ける診断列として残す
- 同一TeX Live tree・warm/cold条件・CPU affinity・release LTOで交互に走らせたwall/CPU値
- 公開DVI仕様に基づく正規化event列、font identity、sp座標、page数が同等の実行だけを性能標本に採用

必須corpusの幾何平均と各主要caseをともに`PraTeX / upLaTeX < 1.2`へ置く。一つの巨大caseで
小さい文書の退行を隠さない。5%以内の幾何平均と10%以内の各caseは、その後のstretch goalとする。
PDF直接出力はupLaTeX単体と同じ仕事ではないためDVI gateへ混ぜず、upLaTeX + driverの
pipelineと別に測る。

最適化はsafe Rustの範囲だけで行う。`unsafe`を前提にした案はこの性能計画の候補に含めない。

### 今回のroadmap再開条件

2026-08-24に始めた`codex3/perf-integration`では、分裂枝の意味統合を先に固定し、その後は
性能作業を優先する。同じ入力・同じTeX tree・同じcold/warm条件・同等DVIを満たすupTeX系との
end-to-end比が**1.3未満**になった時点で、JLReq/JFM等の実装roadmapを再開する。
plain/engine corpusはupTeX、LaTeX corpusはupLaTeXというように比較対象を記録し、異なるformatや
処理段階の値を一つの比へ合算しない。この1.3は作業順を決める中間条件であり、上記の
upLaTeX比1.2未満という最終hard gateを緩めない。

性能変更は、同じrelease設定・同じ合成入力で変更前後を交互に走らせ、出力の一致を
確認してから採用する。測定用入力、実行ファイルの複製、logはリポジトリ外の
`%TEMP%` にだけ置き、版方へ入れない。

## 299頁連続本文のLinux基線（2026-08-25、`13d1ab1`）

巨大教材を主要用途とする利用者条件を受け、`lipsum`を225回ではなく225組の
`\lipsum[1-7]`として反復する[`lipsum-300page.tex`](../tools/fixtures/lipsum-300page.tex)を
primary throughput fixtureにした。TeX Live 2026の実測は299頁、1,396,456 byteである。
短い[`lipsum-short.tex`](../tools/fixtures/lipsum-short.tex)と40頁になる
[`lipsum-30x.tex`](../tools/fixtures/lipsum-30x.tex)は起動・fmt固定費を見る診断列であり、
巨大文書gateを置き換えない。

[`bench-document-throughput-linux.sh`](../tools/bench-document-throughput-linux.sh)は三engineを
逐次、CPU 0へ固定し、測定roundごとに六通りの順序を循環する。wallは時刻補正を受ける
realtime clockでなくLinux `perf stat`の`duration_time`をnanosecondで記録し、GNU timeから
user/systemとpeak RSSを別に取る。PraTeX/upLaTeXは固定preamble commentのraw DVIが一致するか、
一致しなければ[`compare-dvi-semantics.py`](../tools/compare-dvi-semantics.py)がfont番号、整数幅、
file pointer、paddingと、描画eventを一つも含まない空stack frameだけを正規化した公開opcode列で
同等かをhard gateにする。movement、glyph、rule、specialを含むframeは保持する。LuaLaTeX DVIは
format、font番号、backend recordが同じoracleではないため別workload列である。

この基線はwarm-up 3回後、各15回を測った。PraTeX binary SHA-256は
`1733817d508c23ba71cb91e6e375dc6e4c2b8a550c19d97d55e510ff05d890f8`、57,221,231 byteの
PraTeX `latex.fmt`は`4626e02f3d9f4cd710b299de0bab8a9194d1978dc17ddd6ad2cbfcd5be641465`、
公式e-upTeXは`a39eba81da57bab2e96237f9e367d0d6ac92b1fd8a8f42797f3e4e267da18659`、
LuaHBTeXは`9d7a1a55bb2503181d71ada62a6ef78303acdd9d99910ea8da33b059e89c8a8a`である。

| engine | wall平均 ± 母標準偏差 | wall中央値 | MAD | peak RSS中央値 |
|---|---:|---:|---:|---:|
| PraTeX | 2.0997 ± 0.0999 s | 2.0864 s | 0.0674 s | 162,772 KiB |
| upLaTeX | 1.0473 ± 0.0729 s | 1.0671 s | 0.0626 s | 58,380 KiB |
| LuaLaTeX DVI | 5.7300 ± 0.2559 s | 5.7249 s | 0.2076 s | 142,936 KiB |

PraTeX/upLaTeXのround対応比は幾何平均**2.0075**、中央値1.9763、比の範囲1.8043--2.2308である。
絶対中央値の比は1.9553で、1.3未満のroadmap再開条件にも0.98未満のstretch goalにも未達である。
PraTeX/LuaLaTeXの絶対中央値比は0.3644だが、これは異なるformat/backendを含む利用者workload比較で、
upLaTeX互換DVIの合否へ混ぜない。PraTeX/upLaTeXのwarm-upを含む全18 runはDVI SHA-256
`196f46c6ea737d524992e1c93db40d1c10fb59884e412a83a6bf594e76e75ebd`へbyte一致した。

raw 54標本は
[`lipsum-300page-20260825.tsv`](benchmarks/lipsum-300page-20260825.tsv)、binary・fmt・fixture・
計測条件は[`lipsum-300page-20260825-provenance.tsv`](benchmarks/lipsum-300page-20260825-provenance.tsv)
へ固定した。raw TSV SHA-256は
`188f0851ceff6ec86cf51be0e3905e88073a6fdb2cff28ca07bb1a64e1a49e68`である。

同じ299頁をSamply 2,084 samplesで診断すると、`run_loaded_engine` inclusiveは67.42%、
起動・fmt側は32.58%だった。主なinclusiveは`get_x_command_and_token` 37.91%、`main_control` 32.15%、
`expand_token_after_next_token` 17.13%、`get_x_token` 16.94%、`scan_int` 15.40%、
`nested_scan_toks` 11.37%、`prefixed_command` 10.94%、`scan_toks` 10.22%、`end_graf` 9.17%である。
起動側ではKpathsea初期化・path探索8.06%、hyphen trie undump 7.82%、汎用Vec undump 6.48%、
再確保5.57%、PreTrie検証4.22%、制御綴undump 3.26%が見えた。exclusiveでは
`Scanner::get_next` 6.14%、`Command` drop 3.12%、pointer write 2.88%、
`InputStack::get_next` 2.30%、`Command` clone 1.25%、`Node` clone 1.20%で、
変更後の`macro_expand`は0.67%だった。従ってfmtだけを完全に消しても1.3未満には届かず、
undumpとloaded-engineの展開・走査を別々の同一DVI A/Bで短縮する必要がある。

教材型298頁で見つかった脚注内1,161 sp差に対し、通常版の全`make_glue_ratio`をTRIP同様の単精度へ
揃えるcandidateも試した。しかし対象の縦移動は変わらず、目次に新しい1 sp差を作ったため即時撤回した。
後続の一頁自作probeは同じ種類の1,371 sp差を再現した。PraTeXのDVIは公式LaTeX（pdfTeXのDVI mode）と
byte一致し、upLaTeXだけが異なった。logのbox列から、標準LaTeXの`\raggedbottom`が作る
`0pt plus 0.0001fil`をupLaTeX formatが脚注の反対側へ置くformat-level policy差だと確定した。
従ってengineのglue ratio退行ではない。

教材fixtureを`\flushbottom`へ固定すると、PraTeX/upLaTeXの残差はPraTeX側にだけある空のDVI
`push`/`pop`一組だった。空frameだけを除く比較では298頁、247,136 canonical eventが一致し、
元の`\raggedbottom` probeの実座標差は引き続き検出した。性能gateは同一output-routine policyで測り、
canonical SHA-256は`0782322f1ed2cf7531aeefba1139c445af7e26b0b2db5519c004edbae7fa9508`だった。
標準LaTeX profileとupLaTeX互換profileの差は組版互換fixtureで別に保持する。この確認runはwarm-upなし
各一回なので、得たwallを性能標本には採用しない。

### 公式upLaTeX binaryのcounter分解（2026-08-25）

公式sourceを見ず、strip済みTeX Live 2026 binaryへ自作probe、`perf`、`strace`だけを与えた比較では、
空実行/fmtがPraTeX 859.62 ms、upLaTeX 295.45 msで2.91倍、299頁から空実行を引いたcounter差が
PraTeX 1,349.67 ms、upLaTeX 641.03 msで2.11倍だった。どちらか一方だけを同等化しても全体は
1.60--1.76倍に留まるため、0.98にはfmtとloaded-engineの双方が必要である。

PraTeXのASCII `latex.fmt`は57,221,231 byte、公式gzip `uplatex.fmt`は3,641,570 byte、展開後でも
14,995,744 byteだった。最初のsafe Rust A/Bをversion付きbinary fmt、検証済み件数からの一回確保、
decode中のtrie検証統合とする。counter、観測上の関数対応、clean-room限界は
[`benchmarks/uptex-binary-blackbox-20260825.md`](benchmarks/uptex-binary-blackbox-20260825.md)を一次資料にする。

最初に試した行辞書＋可変長ID列は、fmtを57.2 MBから27.2 MBへ縮め、命令数を約17.5%減らしたが、
空実行20組の交互A/Bでcache missを増やしwallを15.0%悪化させたため棄却した。engine差分は残さず、
生counterと不採用理由だけを
[`benchmarks/fmt-line-dictionary-rejected-20260825.md`](benchmarks/fmt-line-dictionary-rejected-20260825.md)
へ固定した。次のA/Bでは全semantic text parseを残す圧縮を避け、loaded runに不要な`PreTrie`をwireから
除く型付き`HyphenRuntimeV1`を最小sliceにする。

その`HyphenRuntimeV1`は採用した。旧ASCIIとの同一binary交互A/Bでは、空実行20組のpaired wall
幾何平均比0.4732、299頁15組は0.8236で、両codecのDVIとauxはbyte一致した。fmtは57.2 MBから
21.5 MBへ縮み、299頁instructionsは21.1%、cache missは51.7%減った。新fmtを既定にした全releaseは
932 passed、0 failed、11 ignoredで、旧ASCII readerも維持する。binary／legacy両fmtのTRIPは
固定commentで公式DVIへbyte一致した。schemaと上限は[`format-file-v1.md`](format-file-v1.md)、raw値と
採用判断は
[`benchmarks/fmt-hyphen-runtime-v1-20260825.md`](benchmarks/fmt-hyphen-runtime-v1-20260825.md)
を一次資料にする。

ただしtask-clockから空実行を引いた本文側は旧1,024.71 ms、新1,027.58 msで変わらない。同日公式
upLaTeXの診断値と比べると新299頁はなお概算1.36倍であり、1.3未満も0.98未満も未達である。
次の支配対象はloaded-engineのtoken取得、整数走査、macro reader、input frameである。

## 現在の一頁budget（Linux perf、`82fa3a2`）

TeX Live 2026、同じ一頁入力を15回測った既知の分解は次である。これは機能完成後の
合格値ではなく、native探索に使える現在の時間budgetである。

| | wall |
|---|---:|
| upLaTeX DVI | 229 ms |
| PraTeX、現在の通常探索 | 524 ms |
| PraTeX、資材を手元へ置いた診断経路 | 140 ms |
| 上の524 ms中の外部`kpsewhich` | 約291 ms |
| 上の524 ms中の自前`ls-R`索引等 | 約93 ms |

このcaseのstrict gateは`229 * 1.2 = 274.8 ms`未満であり、現在値は約2.29倍で不合格である。
一方、外部processだけを同じ意味のnative resolverへ置換した概算233 msは約1.02倍なので、
通常lookupの子process 0は単なる高速化候補でなく最初の必須sliceである。ただし140 msは
TeX tree探索を省いた非対称条件なので合格標本にせず、233 msも実装後に同一tree・同一resolver結果・
等価DVIで再測定する。JFM、K/X、縦組等を加えるたびにこの余裕を再計上し、完成間際まで借金を
隠さない。

### resolver process topology監査（2026-08-24）

利用者の追加Linux profileでも`kpsewhich`がwallの過半を占めたため、HashMap等のengine内部調整より
resolverを再び先に置く。現sourceのread-only監査では、一resolver instanceの最初の一意なTex hitにも
次の固定費がある。

1. `kpsewhich --all --must-exist --progname=pratex --format=ls-R -- ls-R`
2. 最初の用途ごとの`kpsewhich --progname=pratex --show-path=<format>`

従ってTex一用途だけでも最低2 processである。さらに曖昧、stale、未対応path式、aliasを安全に決定
できないqueryはfileごとのone-shotへ戻る。2026-08-24のresolver checkpointでは、Scanner、Output、
直接PDF resource loaderを一つのrun-local resolverへまとめ、catalog、用途path、backend、positive/
negative query cacheを共有した。

同checkpointは`aliases`をboundedに読み、一致aliasまたは壊れたfileだけを公式one-shotへ戻す。
無関係なaliasが存在するだけで後続pathの一意候補を捨てる挙動は解消した。最初のone-shot後に用途pathの
祖先に偶然ある`ls-R`へ昇格するadaptive案は、`TEXMFDBS`外のdatabaseを公式Kpathseaと異なって採用し得る
ためproductionへ入れなかった。最終形はin-process Kpathsea、または環境上書きと`texmf.cnf`の必要部分を
typed planへ原子的にcompileし、通常TeX/JFM/TFM lookupの子processを0にすることである。

次のLinux gateでは`strace -f -T -e trace=process`でargvと件数だけを分類し、wall測定自体はtraceなしで
15組以上交互に行う。plain、LaTeX一頁、`prjsarticle`、optional miss、alias/同名候補を同一treeで測り、
公式`kpsewhich`との物理path・不在status、DVI hashまたはopcode/font/sp座標の一致を必須にする。

#### 利用者提供samply profileによるprocess実測

利用者が同系統の入力で取得した三つのsamply processed profileを2026-08-24に受領した。
profile自体には完全なcommand lineがないため正式な性能gateではなく、process topologyと次のhot pathを
決める診断資料として扱う。PraTeX profileのwallは2,986.394 msで、独立した`kpsewhich` main processが
9件記録されていた。各processの生存時間の合計は1,372.199 ms、平均152.467 ms、中央値152.785 msで、
PraTeX wallの45.95%に当たる。9件は互いに重ならないので、この合計は並列実行で隠れていない。

| profile | engine wall | SHA-256 |
|---|---:|---|
| `lualatex 2026-08-26 04.01 profile.json` | 3,205.070 ms | `53289bb6da0d9b189aa2cca984e3d069af33c51b1d0d90398f2e9275427233c8` |
| `pratex 2026-08-26 04.06 profile.json` | 2,986.394 ms | `b39044680686c65946988e432caf85a66f9637eef3fb6bd096def726587a8f62` |
| `uplatex 2026-08-26 04.10 profile.json` | 1,113.700 ms | `b9bf34b41be23f1ad351156f87025b8e0e72fbb30edd8620ab197bde076862b8` |

`kpsewhich` process時間だけをPraTeX wallから機械的に引いても1,614.195 msであり、このupLaTeX標本の
1.2倍である1,336.440 msより約277.755 ms長い。従ってprocess起動を消すことは最大の第一手だが、
達成後はmacro展開とfmt復元も続けて測る。PraTeX main threadの1,580 samplesではresolver全体が
inclusive 111、`LsRDatabase::load`が105 samplesに現れ、その外側ではmacro parameter走査、
token-list走査、`Vec`再確保が次の候補である。sample数をwall msへ換算して足し引きはしない。

同じ1,580 samplesをprocessed profileのstack tableから再集計すると、重なりを許すinclusive件数は
次の通りだった。exclusiveでは`Scanner::get_next`が198、`Command`のdropが79、allocator内部の
`int_malloc`が78 samplesである。まず外部processを消し、その後に引数bufferの再利用、token-listの
確保、`Command`のclone/dropを別々のA/Bにする根拠とする。

| inclusive frame | samples / 1,580 |
|---|---:|
| `macro_expand::scan_parameters` | 624 |
| `macro_expand::scan_a_parameter` | 554 |
| `Scanner::scan_toks` | 470 |
| `Vec::push` | 207 |
| `RawVecInner::grow_amortized` | 197 |

#### Windows Samply採取runner

Samply 0.13.1はWindowsでETWを`xperf`経由で採取し、採取時に管理者権限を要求する。
`xperf.exe`はWindows標準の`wpr.exe`とは別で、Microsoft Windows ADKの
Windows Performance Toolkitを導入する必要がある。PraTeXのrelease profileは`debug = 1`なので、
追加instrumentationなしでもPraTeX自身のPDBをSamplyへ渡せる。

[`tools/profile-samply-windows.ps1`](../tools/profile-samply-windows.ps1)はSamply、`xperf`、
PraTeX、入力を実fileとして検査し、既定では外部file探索を行わない
[`samply-engine-hotpath.tex`](../tools/fixtures/samply-engine-hotpath.tex)を三回実行する。
これにより、上の利用者profileに見える`kpsewhich` process列と、macro引数走査・token-list展開の
engine内部CPU列を別profileにする。実際の`mainpra.tex`を測る時は`InputPath`、`WorkingDirectory`、
`PraTexArguments`を同じTeX treeとformatへ明示する。

```powershell
cargo build --release --locked --bin pratex
pwsh -File tools/profile-samply-windows.ps1 `
  -SamplyPath C:\tools\samply\samply.exe `
  -PraTexPath target\release\pratex.exe
```

2026-08-24のWindows 23H2 x64確認では、Samply 0.13.1公式ZIP
`samply-x86_64-pc-windows-msvc.zip`のSHA-256
`8fed74dac18197bbb2520125b0199255e09709bed7903a7672a06225a5d7e976`が公式sidecarと一致した。
一方、このsessionには`xperf`がなく、対応するADK 10.1.25398.1のWPT限定installはOSのUACで
`0x80070642`（UAC承認が得られず取消扱い）となったため、新しいprofileを生成していない。
Codexのfilesystem/network承認はWindowsのsecure desktop上のUAC承認を代替しない。
この未採取状態をsample 0や性能改善として扱わず、WPT導入後に同じrunnerで再開する。

#### Rust `kpathsea` crateのin-process候補

利用者の提案を受け、crates.ioの`kpathsea` 0.3.4
（SHA-256 `c573f825f32403aef75bbd955c3427d55e8230e40f0d9b0a98330637b5c8fe1f`）と
`kpathsea_sys` 0.2.3
（SHA-256 `72d72f7d17fa1de89f3fd72ca949733f937ef3ac37a1ba98d36fe09d8f9a0074`）を監査した。
両crateはMIT OR Apache-2.0で、高位crateはsystem `libkpathsea`がbuild時に見つかればin-process FFI、
見つからなければ独自のsubprocess backendを選ぶ。PraTeXはこの自動選択を使わず、監査済みforkの
in-process-only境界だけを使う。

ただし0.3.4のまま既定resolverへ置換しない。現APIは初期化時のprogram nameを`kpsewhich` executableの
basenameから取り、PraTeXが必要とする明示`pratex`を渡せない。公開format定数はTFM、VF、AFM、ENC、FMTを
網羅せず、path/resultはUTF-8 `String`に限定され、非UTF-8のC pathを`unwrap`してpanicする。さらに
`kpathsea_find_file`のowned返値をcopy後に解放しておらず、lookupごとにpath bufferを失う。system libraryが
なければsubprocessへ黙って戻る。
提供profileのloaded-library表にも`libkpathsea.so`は現れず、同じTeX Live binary treeでdynamic libraryが
利用できるとは仮定できない。TUGのTeX Live 2026 build資料でも、binary distributionはstandalone
libraryをinstallしない。このためsystem libraryが偶然あれば速い、なければCLIへ戻る構成は通常配布の
性能契約にならない。

Linux-first checkpointでは、これらを監査済みforkとPraTeX側のsafe adapterへ接続した。その次の
bundled checkpointで、PraTeXのLinux既定featureを`bundled-kpathsea`へ変更した。これは
`default-features=false`のwrapperへ`in-process-only-caller`と`build-from-source`を明示し、公式
TeX Live source mirrorの`fb6158926661cb7a7246b3a94a0cb170a9624d5a`（`svn78399`、
Kpathsea 6.4.2）を静的にbuildする。Kpathsea C sourceはrepositoryへvendorしない。offline buildは
exact treeを`KPATHSEA_SRC_DIR`で与え、取得またはbuildに失敗した場合はCLIへ黙って性能退行せず
buildを失敗させる。既定featureと`KPATHSEA_NO_LINK`の併用も拒否する。crate側の
`subprocess-backend`はcompileしない。

runtimeではprogram名`pratex`、typed format、native `OsStr`/`PathBuf`を一run一handleへ渡す。
linked hitは通常fileとして再確認し、linked missはauthoritativeとする。外部fmtだけは
`--engine=rtex`の意味を保つためin-processへ渡さない。配布側がKpathsea 6.4.2 development libraryを
管理する場合は、`--no-default-features --features stats,system-kpathsea`で従来のdynamic/static
system linkを明示できる。

依存はLinux targetだけに置く。WindowsはC返値のallocator/CRT対応を実測できていないため
typed fallbackのままで性能改善はなく、WASMとその他Unixは依存をcompileしない。feature treeは
`tools/check-kpathsea-features.ps1`で固定する。Rust wrapperのFFI `unsafe`はvendor内へ隔離し、
PraTeXの`src`はsafe Rustだけである。2026-08-24にTeX Liveもsystem KpathseaもないWSLでrelease buildを
行い、固定revisionを取得して55個のC sourceから5分19秒でlinkできることを確認した。これはbuild gateであり、
子process 0、hit/miss、DVI意味、実TeX treeでのend-to-end性能は次のruntime gateまで未達とする。
静的linkを配布する場合はLGPLのsource提供・relink条件、版pin、offline再現、binary sizeを別gateにする。

## 横組JFM glyph sliceの欧文DVI gate（2026-08-23）

`origin/main`のrTeXと横組JFM glyph枝へ、同じ`cmr10.tfm`と次のbyte-only plain入力を与えた。
engine comment長が違うpreambleを除き、最初のBOP (139) からEOP (140) までをbyte列として比較した。

```tex
\catcode`\{=1 \catcode`\}=2
\font\f=cmr10 \f \hsize=200pt \parindent=0pt \tolerance=10000
\shipout\vbox{The quick brown fox jumps over the lazy dog. The quick brown fox jumps.\par}
\end
```

mainはBOP offset 43、glyph枝は45だったが、page bodyは双方183 bytesでbyte差分0だった。
従ってwide node、font selection enum、DVI `set2`/`set3`追加は、このfixtureのbyte glyph opcodeと
sp座標を変えていない。この検査は意味退行gateであり、upLaTeX 1.2倍未満の性能合格を示す値ではない。
横組JFMを含む同等DVI corpusと同一TeX treeが揃った時点でwall timeを別に再測定する。

## 過去のWSL e-upTeX診断値

次の値は、同じPC、同じWSL、同じCPU scheduler上でPraTeXとTeX Live 2026 e-upTeXを
交互に走らせた過去のmicro benchmarkである。upLaTeX format、JFM、page build、DVI出力を
含まないため、現在のhard gateの合否には使わない。hot pathの遅い箇所を見つける診断値として
だけ残す。

`4745f3c`をWSL上でもrelease LTO buildし、INITEX、fmtなし、探索なしで測った。入力は
macro展開と `\advance\count0 by 1` を1000万回行い、終了時に値を検査する。2回warm-up後、
順序を反転しながら各11回測ったwall中央値は次である。

| | PraTeX | e-upTeX | 比 |
|---|---:|---:|---:|
| 空に近いINITEX | 14.361 ms | 151.657 ms | 0.095 |
| 1000万回展開・整数加算 | 1975.460 ms | 1140.525 ms | **1.732** |

起動を概算で控除するとengine部分は約1.98倍であり、1.2倍gateを明確に越えた。このため
`codex/perf-wsl-euptex-safe`を切り、safe Rustのprofile/refactorを先に行う。Windows nativeの
PraTeX/e-upTeX値は環境差の参考にだけ残し、合否へ使わない。

LLVMのinstrumentation profileでは、1000万回入力におよそ次の回数があった。

- `InputStack::get_next` / `Scanner::get_next`: 1.11億 / 1.01億回
- `get_x_token`: 9000万回
- integer参照: 4110万回
- `scan_keyword`: 2000万回
- `RawVec::grow_amortized`: 約1000万回

10M入力だけで学習したLLVM PGOも診断として試した。CPU 0固定の追加測定ではgeneric PGOが
2151.80 msから1479.86 msへ短縮したが、同じ列のe-upTeX 1097.07 msに対してなお1.349倍だった。
狭い入力へのPGOを製品上の解決とはせず、profileが示した確保とdispatchを一件ずつ直す。

## キーワード成功経路の無確保化

TeXの§407に相当する `scan_keyword` は、成功時にも一致済み字句を `Vec`へpushしていた。
1000万回入力では `by` のためだけに約1000万回のgrow/freeが発生する。現行engineの最長語は
6字なので、6字までは局所配列へ置き、失敗して字句を戻す時だけ `Vec`を作る。7字以上も
従来どおり動くheap fallbackを残し、入力上限にはしない。

親 `4745f3c` と `955318e` をWSL rustc 1.97.1、release LTO、CPU 0固定で比較した。100万回版を
4回warm-up後、順序を交互にして各31回測った。

| | 親 | 無確保化 | 短縮 |
|---|---:|---:|---:|
| wall中央値 | 252.708 ms | 240.270 ms | 4.92% |
| child CPU中央値 | 257.403 ms | 243.710 ms | 5.32% |

先頭空白と大文字、部分一致失敗の復元順、7字超の成功と失敗を直接試験した。release全体は
507 passed、0 failed、6 ignored。TRIPは両段exit 0、999 records同士で、preamble comment、
pointer、末尾paddingを除く意味差0だった。PraTeX DVI SHA-256は
`b20af20a1463c6846f0c4c1ce687cd6354ce1a5f65ee401507627570787ae9fe`のままである。
unsafe Rustは使っていない。

## 最上位整数代入の直接化

整数演算の代入は、group外でも毎回 `Definition` と `Variable` へ包み直し、保存levelを調べていた。
最上位では局所・大域代入の意味が同じで、保存すべき外側の値もない。`9bb6023`ではloggerへ同期する
`escapechar` / `newlinechar` を先に処理した後、`cur_level == 0`だけ整数表へ直接書く。group内、
`globaldefs`、高位registerの既存経路は変えない。

独立targetを用い、CPU時間で比較した結果は次である。

| workload | 親 | 直接化 | 短縮 |
|---|---:|---:|---:|
| 100万回、31標本の中央値 | 272.864 ms | 256.476 ms | 6.00% |
| 1000万回、11標本の中央値 | 2447.354 ms | 2257.860 ms | 7.74% |

1000万回の平均でも5.69%短縮した。release全体は507 passed、0 failed、6 ignored。
TRIPは両段exit 0、`tripos.tex`一致、DVI hashと既知の999 records意味差0を維持した。
この時点でも同一WSL e-upTeX比1.2未満には届かなかった。当時はいったん性能専用作業を止めて
e-TeX/pdfTeXと日本語組版の統合へ戻ったが、現在はsafe Rustのまま主要sliceごとに退行を測る。

## 一字の差し戻し

数値や条件の走査は、先読みした一字を `back_input` で頻繁に戻す。従来は一回ごとに
一要素の `Vec<Token>` と `Rc` を作っていた。`TokenListReader` に一字を直接保持する
表現を加え、通常のtoken listは従来どおり同じ `Rc` を共有するようにした。

測定入力は次の130 bytes（末尾LFを含む）で、100万反復中に約200万回の一字差し戻しを
通る。

```tex
\catcode123=1 \catcode125=2 \count0=0\relax \def\x{\advance\count0 by1\relax \ifnum\count0<1000000\relax \expandafter\x\fi}\x\end
```

- fixture SHA-256:
  `891B4D7B8B647F0E05886065C55716E0D195983E2C8F8E0B548E34248D1EE6FC`
- Windows x86_64、rustc 1.98.0、release LTO
- 各実行ファイルを2回warm-up後、順番を交互にして各11回測定
- wall timeとprocess CPU timeの中央値を比較

| | 変更前 | 変更後 | 短縮 |
|---|---:|---:|---:|
| wall中央値 | 768.249 ms | 510.810 ms | 33.51% |
| CPU中央値 | 750.000 ms | 500.000 ms | 33.33% |

全22回で終了値は0、stdout SHA-256は
`C91C3D5D175B00E4D9E00BB5F88A240BFDC339339DC777D6B51419141124E233`、
log SHA-256は
`2F4892969B144313E1A6710D8C5C5DFE18F5B76B3829F174210A489397790609` で一致した。

64-bit buildでは `TokenListReader` は16 bytesから24 bytesになるが、最大variantは別に
あるため `InputSource` 全体は56 bytesのまま変わらない。unsafe Rustは使っていない。
release全343試験を通し、最適化前後のTRIP DVI SHA-256も
`27B79B612B94A1D2815A8747D09B6BA665F2ADFB9F521FCFE7020C6347A29342` で一致した。

## CJK token導入時のASCII退行確認

UTF-8 CJK tokenとtyped制御綴を足した枝でも、上と同じASCII fixtureを使い、直前の
`9d04c08` とrelease LTO buildを交互に各11回測った。両方とも同じVaak checkoutを使い、
2回ずつwarm-upした。

| | `9d04c08` | CJK token枝 | 変化 |
|---|---:|---:|---:|
| wall中央値 | 553.506 ms | 542.120 ms | -2.06% |
| CPU中央値 | 546.875 ms | 531.250 ms | -2.86% |

stdoutとlogのSHA-256は全22回でそれぞれ一種類だけで、変更前後が一致した。小差はcode配置や
測定揺らぎの範囲なので高速化とは数えないが、ASCII fast pathの退行は観測されなかった。
CJK用decoderと `kcatcode` 検索はASCIIでは呼ばず、typed hashと逆引き表もwide制御綴を
初めて作るまで確保しない。測定用source tree、target、logは `%TEMP%` のみに置いた。
同じworktreeでrelease全406試験とTRIP二段を通し、TRIP DVI hashも直前枝と一致した。

## 統一文字分類器のASCII退行確認

`catcode` / `kcatcode` の問い合わせを `CharacterClassifier` traitへ統一した枝を、直接の親
`9af3f19` と比較した。短い計測ではWindowsのprocess CPU timeの15.625ms粒度が相対的に
大きいため、上のfixtureの終了値だけ300万へ増やした。両方を同じrustc 1.98.0、同じVaak
checkout、release LTOでbuildし、1回warm-up後に順番を交互にして各11回測定した。

| | `9af3f19` | 統一分類器枝 | 変化 |
|---|---:|---:|---:|
| wall中央値 | 1642.206 ms | 1636.016 ms | -0.38% |
| CPU中央値 | 1625.000 ms | 1593.750 ms | -1.92% |

stdout SHA-256は全22回で
`25855EADFEEFB5EA17162B1E1E012A6B87758354BB4759C8FE486DFE8B91F5BF`、logは
`E5C427B0A95D409FD86A1C7CA5D4E65583864ACCD45F05AB61F2E0406C621B87` の一種類だけで、
終了値も全て0だった。小差は測定揺らぎとして高速化には数えないが、ASCII退行は観測
されなかった。組込み経路は `Eqtb` 自身へ静的dispatchし、中間object、allocation、
Unicode表引き、拡張class ID生成をASCIIに加えない。`CatCode` は `repr(u8)` である。

## WSL TeX Liveの探索cost

Windows側にnative `kpsewhich`がない実環境で、Ubuntu-24.04上のTeX Live 2026
（Kpathsea 6.4.2）を測った。`kpsewhich`を一件ずつWSLで起動する測定は同じqueryを5回行い、
初回3,892.4994 ms、以後349.3728、321.1636、328.9875、348.1094 msだった。

PraTeXの既定resolverが三つの`ls-R`を発見・UNC越しに索引化し、`cmr10.tfm`を解決して
Windows側から開くend-to-end試験は8.87 sだった。release LTOのlink 3分23秒はこの値に
含めない。索引は以後の同名検索でone-shot processを省けるが、現在の全database先読みは
短い一件だけなら遅い。初回3.89 sと追加query約0.33 sの列と比べると合計16件前後
（初回後さらに約15件）、warm値だけで8.87 sを割ると26--28件が概算の損益境界である。

これは一台のWindows--WSL/UNC構成の値で、native TeX Liveや他のstorageを代表しない。
一回の手測定であり、再現用の環境依存試験は解決経路を索引だけに固定していない。
正しさのため、曖昧な候補や実在する利用者treeでは引き続きone-shot CLIへ戻す。次の性能枝では
同じ解決結果を条件に、lazy/adaptive索引化とWSL内でのbounded読込みを別々に比較する。
詳細は [TeX Live探索の移植記録](kpathsea-port-notes.md) にある。

### `ls-R`索引表現のisolated safe-Rust実験（2026-08-22）

end-to-end変更へ先走らないため、実リポジトリを編集せず、WSL
`/tmp/pratex-lsr-safe-probe-*`の独立prototypeで次の四方式を比較した。

- A: 現行readerの所有`HashMap`意味を再現
- B: `RandomState`を保ち、unique name数とdirectory数を正確に予約
- C: deterministic FNV-1aの所有`HashMap`
- D: 一つのbyte arenaにoffset/lengthを持ち、FNV bucketをcollision-safeなbyte比較で連鎖

環境はWSL2 Ubuntu 24.04、Linux 6.18.33.2、i7-13620H、CPU 2固定、rustc 1.97.1
（LLVM 22.1.6）、`-O -C codegen-units=1`。warm-up 3回後、方式順を回転して24標本を取った。
probe source SHA-256は
`824b6b2220d46315d377552b6a0deda53193e506b75125f25a7302b9ed6f7e87`、binaryは
`71839ced31f7706565482331243cf8bba8452eadd72d3b48b2256004be3f3149`である。
ただしsourceとbinaryはWSL `/tmp/pratex-lsr-safe-probe-20260822`の消去により残っていない。
このhashは当日のartifact同一性の記録であって、hashだけから再現はできない。従って本節は
探索的測定として扱い、性能gateには使わない。採否を決める再測定では、A--D、意味論assert、
合成非UTF-8 fixture、interleaved測定、RSS child modeを持つ`tools/lsr_safe_probe.rs`と、
fixture発見・一時directory・toolchain/hash収集・CPU固定を行うrunnerを先にcommitする。

公開CLI `kpsewhich --all ls-R`で得たfixtureは次の三つ。最大のdist treeは
288,994行、17,298 directory、254,397 accepted entry、231,561 unique basename、
22,836 cross-directory extra candidateだった。

| fixture | byte | SHA-256 |
|---|---:|---|
| config | 80 | `418d569540155c83d3e01fb88cf8ecbf5870deedc3844f86d38df2f9b4d4f5b2` |
| var | 3,330 | `25692224564e8ce593b8bbf8cabd142557b129aa69303d4d2021f4a6433c9e26` |
| dist | 5,674,350 | `17677745673338040a914c26c1935da2c6515d573d3bc7fb3d1b7dbaf4cc0d9e` |

全方式でbasenameをbyte-sortした**全name→candidate directory列**を直接`assert_eq`し、
distのsemantic FNV64 `aa62d954fb168fec`が一致した。4096件の固定hit/miss corpusも結果列を
直接比較し、checksumはhit `11f73eace8743fef`、miss `9eb4e710cf95c4fd`で一致した。
非UTF-8 basename `na\xffme.tex`、重複抑制、hidden entry拒否を含む合成fixtureも一致した。

最大distのbuild時間:

| 方式 | 中央値 | 平均 | p10 / p90 | A比 |
|---|---:|---:|---:|---:|
| A | 49.562 ms | 52.726 ms | 45.849 / 61.076 ms | -- |
| B | 27.112 ms | 27.768 ms | 22.836 / 31.496 ms | -45.3% |
| C | 25.830 ms | 26.871 ms | 21.956 / 30.436 ms | -47.9% |
| D | 24.164 ms | 25.823 ms | 21.556 / 28.646 ms | -51.3% |

最大distのlookup中央値（ns/query）と個別process `/proc` VmHWM:

| 方式 | hit | miss | VmHWM（raw入力込み） |
|---|---:|---:|---:|
| A | 60.287 | 21.101 | 56,832 KiB |
| B | 61.090 | 22.026 | 44,464 KiB |
| C | 53.898 | 31.733 | 44,464 KiB |
| D | 56.285 | 42.479 | 32,168 KiB |

Bはbuildとpeak memoryを大きく改善したが、正確なunique-name/directory件数を測定区間外から
与えたoracle上限である。従って採用結果ではない。実readerが一回の走査で安価に作れる
過大容量hintを設計し、end-to-end resolverで再測定する第一候補とする。

C/DはhitだけならAより速い一方、missが50.4%/101.3%悪化した。さらにunkeyed FNVは、外部から
細工できる`ls-R`に対するhash-flooding DoSを許し、Dのchainはbuild/lookupとも線形へ退化する。
したがって現状は非推奨で、`RandomState`を外さない。Unix prototypeはraw byteを保持したが、
Windows readerがinvalid UTF-8を拒む既存platform policyも表現変更で勝手に変えない。
この絶対値は現行意味を模したprototype内訳であり、PraTeX end-to-end値ではない。

## Linux TeX Liveでの費用分解

Claudeが`codex/euptex-utf8-cjk-token`系の`04d4189`をLinux 7.0、i7-8650U、TeX Live 2026、
release LTOで外部監査した。現在枝そのものやWindows--WSLの数値ではないため絶対値を混ぜないが、
同じ一頁LaTeX入力を12回測って費用を段階的に足した結果は次だった。

| 段階 | PraTeX | 増分 |
|---|---:|---:|
| 書式なし・空入力 | 約1.3 ms | process起動 |
| 16.8 MiBのLaTeX fmtを読む | 約114 ms | fmt復元 約113 ms |
| 一頁を組む | 約141 ms | 組版 約27 ms |
| TeX Live外部探索を使う | 約522 ms | 探索 約381 ms |

同じ条件のpdfTeXは一頁約196 msで、内訳の推定はkpathsea初期化約137 ms、fmtと組版約50 ms。
PraTeXの組版部分は約27 ms対約25 msでほぼ同じであり、少なくともこのfixtureから
「safe Rustの組版意味論が支配的に遅い」とは言えない。現時点の大きな費用は探索とfmt復元である。

監査では用途別`--show-path`等の外部起動が一回約137--144 ms、自前`ls-R`索引が約103 msだった。
したがって優先候補を次とする。

1. 公開`texmf.cnf`の必要な部分集合を独立実装するか、正しさを証明できる場合だけ
   `--show-path`を遅延し、外部kpathsea初期化回数を減らす。
2. fmt 16.8 MiBの内訳を型・表ごとに計測し、既知個数の予約や疎表の表現を個別に比較する。
3. `ls-R`の一行ごとの確保、HashMapの予約不足、短いkeyのhash costをprofileし、変更前後を
   同じ索引結果で比較する。

`texmf.cnf`全体を推測実装して探索順を変える最適化は採らない。曖昧・未対応な式は従来どおり
公開`kpsewhich`へ戻す。1.3 msの探索不要起動、ASCII fast path、TRIP意味一致をhard boundaryにし、
数値は同じcommit、TeX tree、language設定、親processのみの計測で取り直してから採否を決める。

## fmt collectionのbounded予約（2026-08-24）

利用者のLinux測定では、ほぼ同じ長文を三回処理してからDVI driverまで通した列が
upTeX 3.15 sに対してPraTeX 9.14 sだった。この絶対値にはclass、package、TeX tree、
`dvipdfmx`が含まれるため、まずWindows上の隔離CTAN cacheでengine内部を三段に分けた。

- rustc 1.91.0、Windows x86_64、release LTO
- 同じ17,446,628 byteの`latex.fmt`
- formatを読んで直ちに終わるcase、空の`article`、和文を含むLatin 300段落のcase
- 変更前Aと変更後Bを一回ずつwarm-upし、順序を反転しながら各8回
- `PRATEX_PERF_PHASES`を一時的に入れた測定binaryだけでfmt読込みと行分割を計時し、
  計測後にinstrumentationとprobe sourceを版方から除いた

事前の一回測定では、fmt全体468.8 msのうちfile読込み26.9 ms、Eqtb復元441.5 ms、
hyphenation表0.4 ms未満だった。300段落の行分割301回は合計86.4 msであり、少なくとも
このcache済みcaseではEqtb復元が最初のhotspotだった。一般の`Vec`と`HashMap`のfmt復元は
宣言個数を知っているのに空collectionへ逐次pushしていたため、初期capacityを越える
token listやtableではgrowとcopyを繰り返していた。

変更後は最初の予約を4,096要素か要素payload幅換算64 KiB相当の小さい方へ制限し、`try_reserve`を
使う。fmtの宣言長はuntrustedなので、宣言値そのものを`with_capacity`へ渡さない。予約失敗後に
逐次growへ戻すと、同じmemory pressure下でallocationを繰り返すため、typedな
`AllocationFailed`として停止する。`usize::MAX`だけを書いたtruncated Vec/HashMapが巨大確保せず
`IncompleteFile`になる試験と、要素payload幅を含むcapacity hint上限試験を置いた。この64 KiBは
要素payloadの見積り上限であり、`HashMap`のload factor、control byte、allocator metadataを含む
実allocation byte数の上限ではない。

表は中央値 ± 母標準偏差である。

| case | wall A | wall B | 変化 | Eqtb A | Eqtb B | 変化 | peak RSS A | peak RSS B |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| formatだけ | 595.56 ± 51.08 ms | 526.68 ± 56.60 ms | -11.56% | 403.78 ± 39.04 ms | 338.92 ± 51.39 ms | -16.06% | 37.90 ± 0.65 MiB | 35.72 ± 0.69 MiB |
| 空`article` | 807.81 ± 56.24 ms | 684.58 ± 72.81 ms | -15.26% | 422.07 ± 37.39 ms | 342.10 ± 31.65 ms | -18.95% | 38.12 ± 0.87 MiB | 36.65 ± 0.64 MiB |
| 300段落 | 1,412.37 ± 83.09 ms | 1,309.17 ± 82.87 ms | -7.31% | 451.87 ± 23.93 ms | 387.32 ± 53.75 ms | -14.28% | 39.02 ± 0.82 MiB | 36.10 ± 0.98 MiB |

`SOURCE_DATE_EPOCH=1709210096`でA/Bを別々に再実行し、300段落DVIは双方166,940 byte、
SHA-256 `441889a18b75e3aac97c3e7c11e98978a6c6eb3a8399002c6f828acc5edb467c`、logは双方
SHA-256 `54df4ea04fb7133c0514857298561b4123ad35c2aea178b2783d262792df564a`だった。
formatだけと空文書はpageをshipoutしないのでDVI比較対象ではない。

これはOS cacheを明示purgeしていないwarm測定であり、cold filesystem値ではない。また
Windowsの平坦化cacheを使った内部A/Bで、利用者のLinux TeX Live tree、`mainpra.tex`、
`dvipdfmx`を再測定した値ではない。したがって9.14 s全体がこの比率だけ縮むとは扱わず、
Linuxの同一corpusで改めてengine三回とdriver一回を分離する。

A/Bのsource基点はともに`cc65f38`で、Aの実行file SHA-256は
`ea5ecc821fd6416ea75c84dd0657f42d27c12508902c52cf9d8d86a39fded337`、Bは
`56da8321f7fdf7fc9f235e532b537827d4c5b2d6c94bf50b059e5ae2ba31582a`である。48標本のraw値は
[`benchmarks/fmt-bounded-reservation-20260824.csv`](benchmarks/fmt-bounded-reservation-20260824.csv)
へ固定した。Windowsのprocess CPU timerはこの短いcaseに対して15.625 ms刻みと粗く、scheduler
競合も分離できないため採否には使わず、交互実行のwall、process内`Instant`によるfmt区間、
DVI/log一致を使った。

## global制御綴索引の一段復元（2026-08-24）

利用者のbenchmarkは、upTeXとPraTeXではDVI engineを三回実行した後に`dvipdfmx`を一回、
LuaTeXでは直接PDF engineを三回実行する。さらに`jsarticle`、`prjsarticle`、`ltjsarticle`という
異なるclassを使う。この総時間は利用者が見るend-to-end値として残すが、原因の採否ではengine、
fmt、探索、driverを分離する。今回の変更はPraTeX engine内のfmt復元だけを対象にした。

17,446,628 byteの`latex.fmt`では、`ControlSequenceStore`が約28,600個のglobal byte/wide名を
まず`(namespace, active, name)`の中間`HashMap`へ入れ、escaped sidecarを読んだ後に最終hashへ
もう一度挿入していた。名前の`Vec`自体はmoveされるが、bucketを二組保持し、同じkeyを二度hashする。

変更後はglobal名を読込み時に最終`ControlSequenceHash`へ重複検査つきで直接入れる。
namespace付き名だけはnamespace数がまだ読めないため疎な中間表へ残し、`namespaces`を検証した後に
最終表へ移す。これにより、壊れたfmtのnamespace番号だけを根拠に最大65,536個の空hashを先に
確保する退行を避ける。byte/wide二blockの宣言数も合計65,536件を本文読込み前に検査する。
escaped IDの重複・欠落、namespace範囲、byte/wide/active sidecar、
表示byte列の一致は、完成した表を一回走査して従来どおり拒否する。fmt wireは変えない。

Aは`681e06536f4e70e710e797db61e54fd29de77f42`、Vaakは
`4e40e4bbc221c75e2554193b773a8e0f46cf5f36`を基点とし、A/C共通差分は区間計測用の一時的な
`PRATEX_PROFILE_TIMING`だけである。CはAへ上記の一段復元だけを加えた。rustc 1.91.0、
Windows 11 build 22631、x86_64 MSVC、1,750 byteの`prjsarticle-sample.tex`、同じ作業directory、
`SOURCE_DATE_EPOCH=1709210096`で、A→Cを三組warm-upした後、奇数round A→C、偶数round C→Aを
12組測った。表は各12回の中央値である。

| 区間 | A | C | 変化 | Cが短い組 |
|---|---:|---:|---:|---:|
| control-sequence復元 | 456.687 ms | 384.556 ms | -15.79% | 10 / 12 |
| fmt全体 | 557.748 ms | 497.911 ms | -10.73% | 10 / 12 |
| wall | 1,527.123 ms | 1,445.298 ms | -5.36% | 8 / 12 |

全24 runはexit 0で、A/CのDVI SHA-256は
`3ae145d49587b29ae488028c968e92e9dc18a8f0b2a9be1550a2b5a817dbf785`、log SHA-256は
`fcd3b269f4e64263261cc1ee0f3189e8b1426a5ccb2e3977c8065db2d65bf1a`で一致した。raw値は
[`control-sequence-global-hash-20260824.csv`](benchmarks/control-sequence-global-hash-20260824.csv)、
CSV SHA-256は`a64f44d10caff7314ef8dc4dac946ad145a0d581009e940207e5bad23e5158b9`である。

共有枝へ移した同じ意味差分は`cargo fmt --check`、releaseのcontrol-sequence unit
22件、全release 849 passed / 0 failed / 10 ignoredを通した。A/B基点より後の
`\savinghyphcodes`とmain-loop JFM checkpointは制御綴表を
変更しないが、現HEADのLinux end-to-end値を再測定したものではない。従って、この5.36%を
利用者の9.14 s全体やupLaTeX比1.2未満の達成へ外挿しない。

### WSL resolver失敗反復の診断

同じ日に、`hyperref`、`graphicx`、`siunitx`、`pxrubrica`を含む平坦化package probeを
Windows上で走らせた。`graphics.cfg`不足で停止するまで7.59 sかかり、外部processは13回、
合計3.645 sだった。13回すべてが失敗後に繰り返された
`wsl.exe --cd / --exec wslpath -w /`である。backend discoveryの失敗がrun中に記録されず、
別のoptional file lookupごとに同じ発見処理へ戻ることを実測した。

これはLinuxの利用者benchmarkには存在しないWindows固有の列なので、上の190%差の説明には
使わない。ここだけを切り分け、WSL backend発見のfailureをresolver instanceへ保持し、
`clear_external_cache`でだけ再試行するBを作った。個別queryのnegative cache、
stale DB、alias、拡張子補完、casefold、非`!!`利用者treeの判断は変更していない。

同じrelease LTO、同じ`lapratex.fmt`、同じ平坦化runtime、同じ
`SOURCE_DATE_EPOCH=1709210096`で、A→Bを三組交互にwarm実行した。A/Bのsource基点は
`9b52521`で、計測instrumentationを除けば差は発見失敗をbackend stateへ保存する一行だけである。
各行は一回のraw値で、外部時間は終了したWSL process内の`Instant`合計である。

| round | A wall | A WSL process / 合計 | B wall | B WSL process / 合計 |
|---:|---:|---:|---:|---:|
| 1 | 6212.396 ms | 13 / 3549.070 ms | 2389.688 ms | 1 / 275.956 ms |
| 2 | 5553.884 ms | 13 / 3693.908 ms | 1843.571 ms | 1 / 261.403 ms |
| 3 | 4716.424 ms | 13 / 3090.183 ms | 1919.721 ms | 1 / 266.179 ms |
| 中央値 | 5553.884 ms | 13 / 3549.070 ms | 1919.721 ms | 1 / 266.179 ms |

中央値ではwallが65.44%、終了したWSL process時間が92.50%短くなった。Aのbinary SHA-256は
`1147f623802af095f17cac5d26687c97f1396845596b587a62c6ca39e283cc35`、Bは
`269724e6eefd147a69aef9908e700a825d22e79bc735d279f1a5b2ecb7d606d5`である。
双方とも`graphics.cfg`がなく`graphics`のdriver未指定で同じ位置にexit 1し、logは
5917 byte、SHA-256
`d628108d576dfbc9ee4f856ec808a930cec4051b8386976f1ec7bce7b0c23209`で一致した。
DVIを出す前の失敗probeなのでDVI比較値はない。計測instrumentationはproduction sourceへ残していない。

この値は「発見不能なWSL backend」を繰り返した異常系だけの上限効果であり、native/Linuxの
通常探索や利用者の9.14 s benchmarkを短縮した値ではない。出力意味の回帰は合成executorで
初回と再生のerror種別・OS error・診断、明示clear後のfailure→成功まで固定する。

## 次の候補

測定済みの次候補は、探索外部processの削減、TFM lig/kern小表の汎用hash除去、
`ls-R`索引の確保削減、入力行bufferの再利用、PDF文字命令の一時`String`除去である。一つの枝へ混ぜず、同じ
出力hashとTRIPを条件に個別採否を決める。

## 組込みKpathseaの実行時子process 0 gate（2026-08-24）

Linux既定buildは、公式TeX Live 2026 revision `svn78399`のKpathsea 6.4.2を静的に組み込む。
TeX Liveを持たないUbuntu WSLでrelease linkしたbinaryに対し、合成`texmf.cnf`と`ls-R`から
TEX、Latin TFM、JFM、VFのhitとkind別missを検査した。`ldd`には共有`libkpathsea`がなく、
空のtool用`PATH`で`strace -f -e trace=process`したDVI実行は一PID、process生成event 0だった。
TeX tree経由とcurrent-directory経由のDVIはbyte一致し、固定SHA-256は
`658ec798192d67c3a067b8296a3300e580b2aaf7ba8b4fcc04dab78022848993`である。再現手順は
[`test-bundled-kpathsea-linux.sh`](../tools/test-bundled-kpathsea-linux.sh)、探索意味と限界は
[`kpathsea-port-notes.md`](kpathsea-port-notes.md)に置いた。

これはbenchmarkではない。合成treeにおける外部`kpsewhich`起動0を実行時に固定した正しさgateで、
実TeX Live corpusのwall、`ls-R` load、fmt undump、macro展開、DVI driver時間を測っていない。
次の採否は利用者fixtureを同一Linux TeX treeで三回ずつ測り、engineと`dvipdfmx`を分離して行う。

## 固定CTAN runtimeでのlocal/tree交互測定（2026-08-24）

合成fileだけの上記gateに続き、`tools/test-prjsarticle.ps1`がSHA-256固定CTAN archiveから
repository外へ展開したLaTeX 2e 2026-06-01、L3 2026-08-10、CM/LaTeX/upTeX metricを使った。
[`bench-bundled-kpathsea-ctan-linux.sh`](../tools/bench-bundled-kpathsea-ctan-linux.sh)は、このflat
runtime 932 fileとrelease binaryをWSLのext4一時directoryへ先にcopyする。Windows mountのI/Oや
WSL起動時間はengine runへ混ぜない。同じ`latex.fmt`と`prjsarticle-sample.tex`について、次を
奇数roundはlocal→tree、偶数roundはtree→localの順で15組測った。

- `local`: source、metric、PraTeX-private fmtがすべてcurrent directoryにある。
- `tree`: 文書と互換性のないPraTeX-private fmtだけをcurrent directoryに置き、source/metricは
  最小`texmf.cnf`と`ls-R`を持つ`!!` treeから組込みKpathseaで解決する。

PraTeX fmtを一般のTeX Live treeから探さないのは現在の公開契約である。`TEXFORMATS`を与えて
探索できたことにする測定は採らない。format生成、local/tree各一回のaux warm-up、15組の本文run、
最後の`strace -f -e trace=process`を一つのWSL session内で実行した。

| case | n | wall平均 ± 母標準偏差 | wall中央値 | user中央値 | system中央値 | peak RSS中央値 |
|---|---:|---:|---:|---:|---:|---:|
| local | 15 | 0.4807 ± 0.0708 s | 0.49 s | 0.22 s | 0.05 s | 37,892 KiB |
| tree | 15 | 0.4787 ± 0.1071 s | 0.45 s | 0.23 s | 0.05 s | 37,888 KiB |

paired `tree - local`の平均は−0.002 s、中央値0.000 sで、treeが短かった組は5/15だった。
この粒度ではlocal/treeの差はnoise以下であり、「treeの方が速い」とは判断しない。一方、tree runの
process traceは一PID、`clone` / `fork` / `vfork` 0で、15組すべてのDVI SHA-256はlocal/treeとも
`3ae145d49587b29ae488028c968e92e9dc18a8f0b2a9be1550a2b5a817dbf785`へ一致した。利用者profileの
9回・1.372 sの`kpsewhich`列は、このLinux既定経路には残らない。

測定commitは`ea1bef0`、binary SHA-256は
`eca7d735ad7a831d6fbd56d01003623f3663024f836a1dcec5fb65551f4fb85d`、17,452,967 byteのfmtは
SHA-256 `5b53a663e6919180cef08204a337cf5955776f49861ed6361c9a2b4449977a6c`、入力は
SHA-256 `5a4ebf7e06bf89694fe54132d1f4c77e166453c14f9dc053d51de6f5342c2248`である。raw 15組の
TSV SHA-256は`24be2681edb1fe82511c7568966564466b771877a6e26cf179e366888af39ef4`。

これは固定CTAN runtime上の1,750 byte sampleで、利用者の`mainpra.tex`、30回の`lipsum`、追加package、
公式TeX Live全tree、三回engine＋一回`dvipdfmx`を再現していない。したがって9.14 sの新しい絶対値や
upLaTeX比1.2未満の合格とは扱わない。Linux lookupの次の支配候補はfmt undump、macro/token走査、
control-sequence hashであり、同じDVIを保ったA/Bで個別に測る。

## global byte制御綴のdirect cache不採用（2026-08-24）

control-sequence hashの候補として、global・non-activeなbyte名だけを対象に256 slotのdirect-mapped
run-local cacheを試した。cache tagはFNV-1a後にavalancheし、hit時も`escaped` sidecarの完全なbyte名を
比較するため、衝突でidentityを誤らない。missとtag衝突は従来のSipHash付き`HashMap`へ戻し、fmt wire、
namespace、active、wide名は変更しない候補だった。

しかし、この前段は名前全体の軽量hashを毎回追加する。外部file、fmt、fontを使わずmacro引数走査と
token-list展開だけを100,000回反復する`tools/fixtures/samply-engine-hotpath.tex`で、Linux release binaryを
WSLのext4一時directoryへcopyし、奇数roundはbaseline→candidate、偶数roundは逆順で15組測った。
`SOURCE_DATE_EPOCH=1709210096`、一回warm-up、同じCLI引数を使い、終了後のlogはbyte一致した。

| case | n | wall平均 ± 母標準偏差 | wall中央値 | user平均 ± 母標準偏差 | user中央値 | peak RSS中央値 |
|---|---:|---:|---:|---:|---:|---:|
| baseline | 15 | 1.2807 ± 0.2541 s | 1.20 s | 1.1040 ± 0.1910 s | 1.09 s | 5,864 KiB |
| candidate | 15 | 1.2373 ± 0.2164 s | 1.20 s | 1.1027 ± 0.2073 s | 1.05 s | 5,792 KiB |

paired `candidate - baseline`はwall平均−0.0433 s、中央値−0.05 s、user平均−0.0013 s、中央値
−0.02 sで、candidateが短かった組はどちらも8/15だった。user CPU平均差は約−0.12%に過ぎず、
分散より十分小さい。macro hotpathを狙った入力で改善を立証できないため、cache sourceとtestは撤回した。
wallの見かけ上の差やRSS差を採用根拠へ使わない。

baseline binary SHA-256は
`eca7d735ad7a831d6fbd56d01003623f3663024f836a1dcec5fb65551f4fb85d`、candidateは
`ba139e606e48a3dc9e7a2b23c3b3e6d384f00217f5709ff200f51b5ffbbf72d1`。raw値は
[`control-sequence-direct-cache-rejected-20260824.tsv`](benchmarks/control-sequence-direct-cache-rejected-20260824.tsv)
へ置いた。headerなし元TSVのSHA-256は
`204d4b85a4e9501c8139273df3a64bdd2baa217772cc1c12ef19f6aa175881ef`である。

## macro引数arenaと未参照引数の保持省略（2026-08-25）

利用者のLinux profileで`scan_parameters`、`scan_a_parameter`、`Vec::push`、allocatorが次の支配候補に
見えたため、`13d1ab1`ではmacro引数を引数ごとの`Rc<Vec<Token>>`へ分けず、一呼出し一個の
`MacroArguments`へ集約した。token本体は一つの`Vec`、最大九引数の終端は固定配列で持ち、各`#n`
readerは同じ`Rc`と引数番号を共有しながら独立した読取り位置を持つ。外側braceの除去、空引数、
重なるdelimiter、同じ引数の複数展開、tracing、runaway診断は引数rangeだけを見る。

置換本文から参照されない引数もTeXの走査・delimiter照合・診断は行うが、走査直後にscratch末尾を
戻して保持しない。全引数が未参照ならarena用`Rc`を作らず、scannerの空`Vec`容量を次のmacro走査へ
再利用する。無引数macroは置換本文の参照mask自体を走査しない。この経路は空の置換本文だけへの
fixture特化ではなく、参照される引数と未参照引数が混ざるmacroにも同じ型境界を使う。

診断用入力は`tools/fixtures/samply-engine-hotpath.tex`と同じ8引数macroを100,000回呼び、最後に
同一の一頁DVIをshipoutするINITEX fixtureである。入力SHA-256は
`6ddd791439e6c83f19a79327b4538b309e4eaff4ccde94eb0d7d601d4998173b`。TeX Live 2026の同じtree、
release LTO、CPU 0固定、`SOURCE_DATE_EPOCH=1709210096`、4回warm-up後31組を奇偶で逆順にした。
PraTeXと公式upTeXの全70 runは同じDVI SHA-256
`518000c677d9c7a78cf5e4e6c533345bc48129e1179625bcf84c0f4c3390ae62`を生成した。

| 実装段階 | PraTeX wall平均 / 中央値 | upTeX wall平均 / 中央値 | paired比 幾何平均 / 中央値 |
|---|---:|---:|---:|
| 変更前 | 1.567 / 1.550 s | 0.640 / 0.630 s | 2.454 / 2.483 |
| 一共有buffer、開始・終端range | 1.009 / 0.980 s | 0.690 / 0.670 s | 1.462 / 1.475 |
| 終端だけの固定range | 0.952 / 0.930 s | 0.684 / 0.650 s | 1.400 / 1.435 |
| 未参照引数を保持しない | 0.760 / 0.750 s | 0.605 / 0.600 s | **1.257 / 1.267** |

各段階のraw TSV SHA-256は順に`2c47163e8cab1bec9531eee76f066a06c4a8167231550d95e6490160e7061a55`、
`3577c2175775e645af2e1d942ac81983c75a6c56c343fffb35c6033457896940`、
`7d73c8bd9b54d57b493a432d766bcfd6ad3fb170e19f8fc67c0eeae46405a499`、
`390d76940958009b906a261978e62ca2b27f91d603f3d3517ce063da6fe40ea9`である。最後の測定binaryは
`858c1fc8b1c94e8cb62c45243b958adafcabf06f7ad7fcd6f10a67d3a7692772`。その後に加えた無引数macroの
mask走査省略はこの8引数fixtureの実行経路を変えない。

最終sourceを再buildしたbinary
`1733817d508c23ba71cb91e6e375dc6e4c2b8a550c19d97d55e510ff05d890f8`でも31組を追測し、paired wall
中央値比は1.259だった。ただし同時にVaak担当作業が走り、PraTeX/upTeX双方のwall中央値が
0.92/0.82 sへ膨らむ外乱と大きなoutlierがあったため、この追測の平均や幾何平均は採否値に使わない。
raw TSV SHA-256は`5051a6ab33d13489fb86ddea8ae775a55ab9662bbe9b947f05f097b5405c82e5`である。

focused testはmacro走査7件が成功し、そのうち共有parameter source 1件も個別に再実行した。
全releaseは922 passed、0 failed、
11 ignored。公式CTAN TRIPも両段exit 0、`tripos.tex` byte一致、`8terminal.tex` 0 byte、
PLtoTF→TFtoPL byte一致である。独立decoderは公式・PraTeXとも999 records、16 pages、最大stack 17、
意味差0を確認し、固定commentでは公式DVIと2920 byteすべて一致した。

これは探索・fmt・LaTeX・JFM・page buildを含まない狭いmacro診断caseであり、roadmap再開条件の
文書end-to-end比1.3未満を単独で満たしたとは扱わない。`../vaak`も測定中にHEADとdirty差分が動いたため、
上記binary hashを一次識別子とし、Vaak commitだけからの再現を主張しない。続く299頁`lipsum`基線は
本書前半に固定済みである。今後は`docs/research/japanese-publishing/`の学術・小説fixture方針を反映した
日本語、和欧混植、禁則多用、教材型300頁級corpusを、upLaTeXとLuaLaTeXを含む同条件で測る。

## MacroCall引数参照maskの定義時導出は不採用（2026-08-25）

macro呼出しごとのreplacement参照mask走査を`MacroCall`生成時の一度だけにする候補は、
299頁`lipsum`の20組でpaired wall幾何平均比0.990571だった。しかし同じbinary対を用いた
8引数macro 100,000反復の31組ではwall 1.057001、task-clock 1.056055、instructions
1.021428へ悪化した。狙ったhot pathで31組中30組が遅かったため、source差分は撤回した。

299頁だけの約1%短縮は一般化しない。両fixtureとも出力はbyte一致し、rawと再現情報は
[`macro-call-derived-mask-rejected-20260825.md`](benchmarks/macro-call-derived-mask-rejected-20260825.md)に固定した。
次は型サイズを増やさず、数値・寸法・糊走査のfirst-token差戻し／再取得をprivate経路で減らす。

## 寸法走査のfirst-token差戻しを除く（2026-08-25）

寸法scannerが空白・符号と最初の展開済みtokenを読んだ後、non-internal経路はそのtokenを
差し戻し、整数scannerが直後に再取得していた。既に解決済みの`UnexpandableCommand` / `Token`対を
private経路で渡し、一回のbackup、input source push、token dispatchを除いた。

1,600,000回の寸法代入を行うINITEX fixtureの31交互組で、paired幾何平均比はwall 0.965479、
task-clock 0.966130、instructions 0.964690で、全31組でcandidateが短かった。299頁の20組は
wall 0.996439だが分散より小さく、gate達成へは外挿しない。DVI・aux・局所logはbyte一致し、
条件とrawは[`dimension-first-token-20260825.md`](benchmarks/dimension-first-token-20260825.md)に固定した。
全releaseは934 passed、0 failed、11 ignored。公式TRIPは両段exit 0、`tripos.tex`・PLtoTF→TFtoPLは
byte一致、固定comment DVIも公式2,920 byteと完全一致した。

## 糊走査のfirst-token差戻しを除く（2026-08-25）

`scan_glue`のnon-internal幅も、取得済みのcommand/token対と符号を寸法scannerへ直接渡す。
明示糊では符号を幅だけへ掛け、内部Glue/MuGlueを全成分反転する既存branchは変更しない。
共通core抽出で通常寸法へ中間callを残さないよう、token取得だけの薄いwrapper二層を
always-inlineにした。

1,600,000回の糊幅代入を行うINITEX fixtureの31交互組で、paired幾何平均比はwall 0.965541、
task-clock 0.965646、instructions 0.960493だった。wallは31組中30組、task-clockとinstructionsは
31組すべてでcandidateが短かった。299頁の20組はwall 0.994623、task-clock 0.994808だが、
短かったのは各11組でありgate達成へ外挿しない。DVIとauxはbyte一致した。

共有`../vaak`の作業中変更が混入し得る最初の測定は全て破棄した。正式値はVaak `7dc011b`と
PraTeX `763e4a7`をclean detached worktreeへ固定し、同一PraTeX path・同一targetで作ったbinary対だけを
用いた。条件とrawは
[`glue-first-token-20260825.md`](benchmarks/glue-first-token-20260825.md)に固定した。
全releaseは935 passed、0 failed、11 ignored。公式TRIPは両段exit 0、`tripos.tex`・
PLtoTF→TFtoPLはbyte一致、`8terminal.tex`は0 byte、固定comment DVIも公式2,920 byteと
完全一致した。

### `9776e1a`後の299頁hot path再診断

糊handoffを採用したsourceと一致するbinaryを、同じ299頁入力・型付きfmtでCPU 0へ固定し、
Linux `perf record -F 999 -g --call-graph dwarf`で一回だけsamplingした。1,885 samplesの
**self cycle share**上位は次だった。

| symbolまたは層 | self |
|---|---:|
| `Scanner::get_next` | 19.79% |
| `macro_expand` | 7.82% |
| 組込みKpathseaの`hash_insert_normalized` | 6.96% |
| binary fmtのCRC計算 | 3.05% |
| hyphen Trieのlanguage pattern検証 | 2.28% |
| `Node` drop | 2.23% |
| `get_x_command_and_token` | 1.98% |
| line break本体 / `try_break` | 1.71% / 1.56% |
| hlistへのnode追加 | 1.57% |
| 単語hyphenation | 1.54% |

allocator、free、realloc、moveはlibc内の複数symbolへ分散しており、個別最大は`_int_malloc`の
3.41%だった。macro parameter range取得は0.93%、`nested_scan_toks`は0.88%、input frame pushは
0.68%、整数scanner本体の`scan_int_radix_from_first`は0.21% selfだった。inclusive call treeは
LTO binaryのunwindが十分でないため、このrunから推測しない。以前のinclusive 15.40%という
`scan_int`値と、今回のself 0.21%を直接比較して「整数走査が解消した」とも判断しない。

これは正式な複数交互標本でなく、次のA/B候補を選ぶ診断runである。現時点の第一候補は
`get_next`のtoken-only経路、不要な`Command` clone/drop、parameter/input frame dispatchであり、
寸法・糊scannerのさらに狭い調整より先に測る。profileは15,879,144 byte、SHA-256
`cf58942db891675eb8839f5ec09945368fd01ea545ff093e9179a8dee57b2120`で、repository外の
`/tmp/pratex-current-hotpath-9776e1a.data`に置いた。binary SHA-256は
`bdd63fd7e9feaeade8ffba8b61c82a6c5c88b058ac65c7b540fe474b72a1e92f`、入力とfmtはそれぞれ
`265a52f085db6afb43a3f8a420be0a80ec554c7a1052b064edaa85a539f7f2cd`、
`c5cda9564ed3251f450ceb7c63f87ec334c55b9c9ea63afb06e7111f06e0013c`である。

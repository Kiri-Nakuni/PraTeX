# TeX Live探索の移植記録

更新: 2026-08-22

PraTeXはKpathsea libraryを再実装したものではない。公開されている`kpsewhich` CLIと
`ls-R`の書式を境界にして、TeX Liveの探索結果をPraTeXのfile resolverへ接続する。
Kpathseaの実装sourceは移植せず、公開manual、固定したCLI argv、合成fixture、実環境の
black-box観測から独立して書く。

## 現在の探索順

一つのPraTeX実行中は、次の順で同じ`FileResolver`を使う。

1. 論理名そのものが通常fileなら、直接pathとして採用する。
2. formatがlocal限定policyなら、外部探索を行わず不在を返す。
3. 同じfile種別と論理名について、run-localの成功・不在cacheを引く。
4. `ls-R`と用途別探索pathから、先行候補を飛ばさない一意なfileを証明できれば採用する。
5. 証明できなければ、用途を固定したone-shot `kpsewhich`へ問い合わせる。

索引は最終結果を推測するためではなく、外部processを省いても結果が変わらない場合だけ使う。
曖昧さ、壊れた入力、未対応のpath式はerrorへ潰さず、公開CLIへ戻す。

`FileKind`ごとに`tex`、`tfm`、`map`、`enc files`、`type1 fonts`、`afm`などの公開format名を
対応させる。TeX入力、`\input`、`\openin`、`\pdffilesize`、TFM、PDF map、encoding、
Type 1、AFM、Vaak入力は同じresolver契約を通る。Scannerが所有するTeX入力・TFM等は一つの
instanceを共有するが、PDF font resource loaderは現在別instanceなのでcacheも共有しない。
論理名と解決後の物理pathは別の型で保持し、TEXMF上の配置をDVI font名やPDF map keyへ漏らさない。

## `ls-R` fast path

初回の外部探索時に、次の固定argvで利用中のTeX環境自身へdatabaseを尋ねる。

```text
kpsewhich --all --must-exist --progname=euptex --format=ls-R -- ls-R
```

用途別の探索順は`kpsewhich --progname=euptex --show-path=<format>`の展開済み出力から取る。
PraTeX自身で`texmf.cnf`の変数やbraceを展開しない。現在、安全に解釈する部分集合は次である。

したがってfast pathの初期化にも、database発見を一回、最初に使う用途ごとに`--show-path`を
一回起動する。省くのは、それ以後のfile一件ごとのone-shot問い合わせである。productionの
`KpsewhichResolver::default()`だけが自動索引とWindowsの既定WSL fallbackを有効にする。
埋め込み・合成試験用の`ResolverOptions::default()`は両方とも無効である。

- 現在directory `.`
- 絶対directory
- `!!`を付けたdatabase限定要素
- 一要素中に一つだけある再帰記号`//`と、その前後の絶対prefix・相対suffix

未展開の`$`、brace、`~`、相対directory、複数の`//`などは`Unsupported`として残す。
途中だけ都合よく解釈せず、その問い合わせ全体をone-shot CLIへ戻す。

`ls-R` readerはbyte単位でCRLFと最終改行なしを扱う。Unixでは非UTF-8 basenameを保持し、
Windowsでは不正UTF-8を置換せずdatabase全体をfast path不使用にする。各databaseの
読み込み上限は256 MiB、一行1 MiB、entry 800万件である。発見するdatabase数と合計memoryには
まだ別の上限がない。読み込み前後の長さと更新時刻が変わった場合、root外の見出し、NUL、
不正な見出しがある場合も部分索引を使わない。snapshotは長さと取得できたmtimeであり、
content hashやfile identityではない。directoryをinternし、通常の一意basenameには候補`Vec`を
割り当てない。

hidden componentを含むdirectory、separator入りentryは索引しない。絶対論理名と`.`または
`..`を含む論理名もfast path対象外で、直接pathまたはCLIへ委ねる。root外見出しの拒否は
lexicalな検査であり、canonical pathやsymlinkを隔離するfilesystem sandboxではない。

探索pathの各要素では、実在する候補をmetadataで再確認する。最初に該当する要素で候補が
一つなら採用するが、同一要素に複数ある場合はCLIへ戻す。索引にない先行の利用者treeが
実在し得る場合、stale entry、`aliases`が効き得る場合、databaseのsnapshotが変わった場合も
CLIへ戻す。存在しない先行treeと`!!`で不在を証明できる要素だけは安全に飛ばせる。

## WindowsからWSL TeX Liveを使う境界

Windowsのproduction既定resolverは、native `kpsewhich`の**起動fileが見つからない場合だけ**
既定WSLへ移る。native programが起動できた後の「該当なし」、診断つきstatus 1、異常終了、
権限errorをWSLの別TeX Liveで覆わない。一度選んだnative/WSL backendは、そのresolver
instanceの生存中は固定し、`clear_external_cache`でだけ再発見する。Scannerと
PDF font loaderは現在別instanceなので、run-globalに一つのbackendを固定する契約ではない。
埋め込み用の明示constructorではWSL fallbackを既定無効にしている。

既定distributionは次の固定argvでrootのWindows表記を得る。distributionを明示するpolicyでは
同じ呼出しへ`--distribution <name>`を加える。

```text
wsl.exe --cd / --exec wslpath -w /
wsl.exe --distribution <name> --cd / --exec kpsewhich ...
```

shell文字列は組み立てず、すべて一個ずつのargvとして渡す。`wslpath`の結果は
`\\wsl.localhost\<distribution>\`または互換的な`\\wsl$\...`のrootだけを受け入れる。
Linux絶対pathの各componentをUNCへ写し、NUL、改行、`.`、`..`、backslash、Windowsで
無効または予約されたcomponent、不正UTF-8は推測変換しない。返ったfileはWindows側から
metadata確認してから開く。WSL bridgeの失敗は「file不在」としてnegative-cacheしない。
WindowsのcwdやWindows絶対pathをLinuxへ逆変換するbridgeではなく、WSLへ渡す論理名も
UTF-8で表現できる場合に限る。native Windowsの`kpsewhich`出力もstrict UTF-8で検証する。

実機ではUbuntu-24.04上のTeX Live 2026（Kpathsea 6.4.2）について、三つの`ls-R`を発見し、
Windows側の既定resolverから`cmr10.tfm`を解決して内容を開けることを確認した。一回の手測定では
索引経路も確認したが、環境依存の回帰試験は正しさのため索引またはone-shotのどちらも許す。
試験本体は`PRATEX_TEST_WSL=1`を安全弁として要求し、さらに`#[ignore]`である。例えば次のように
環境変数とignored指定の両方が要る。

```powershell
$env:PRATEX_TEST_WSL = '1'
cargo test --release file_search::tests::既定resolverがwsl_tex_liveのtfmをwindowsから開ける -- --ignored
```

## 性能と残る問題

同じWindows/WSL環境で、warmなone-shot `kpsewhich`一件は321--349 msだった。一方、三つの
databaseをUNC越しに初めて読み、一件を解決するend-to-endは8.87 sだった。初回one-shot
3.89 sと追加query約0.33 sの列に対する損益境界は、合計16件前後（初回後さらに約15件）である。
8.87 sをwarm値だけで割るなら26--28件なので、どちらも概算として区別する。現在の全database
先読みは長いLaTeX実行ではprocess回数を減らせる一方、短い一回の探索では遅い。この数値は
一回の手測定で環境固有であり、native TeX Liveの性能を表さない。次段では出力を
変えず、lazy/adaptive起動またはWSL側でのbounded database読込みを比較する。

現在残る主な非互換は次である。

- `texmf.cnf`自体の解析、変数・brace・`~`を含むpath expressionの完全な意味
- Kpathseaのalias、case folding、subdirectory matchingを含む全探索規則
- `mktex*`による生成と設定ごとの副作用
- daemonでTeX Liveが更新された時の世代付きcache無効化とsnapshotの原子的な差替え
- package不足時の取得・検証・専用overlayへの配置
- WSLの短い実行に対するdatabase読込みcostの適応的な回避
- database発見数・全database合計memoryと、外部command出力をread中に止める上限

database発見出力は16 MiB、探索path出力は4 MiBをparse前に検査するが、現在は
`Command::output()`が全量をmemoryへ読んだ後の検査である。fileごとのone-shot stdout/stderrにも
read中の上限はない。`unsafe`を使わずshell文字列を組み立てないことは、この外部programや
filesystemをsandbox化することと同義ではない。

現在のcacheは一実行内だけを対象とする。長寿命daemonへそのまま持ち上げると、cache hitが
database再検証より前に返るため古い成功・不在を保持し得る。監視機能ではresolver plan、
database snapshot、positive/negative cacheを同じgenerationへ結び、完全に読めた次世代へ
原子的に切り替える必要がある。

## 検証と一次資料

合成試験は、探索順、重複、stale entry、壊れたdatabase、`!!`、利用者tree、用途分離、
native/WSLを混ぜない条件、UNC変換、cache消去を固定する。2026-08-22時点で
`cargo test --release --no-fail-fast`は455件通過、失敗0、環境依存等4件skipである。

- [Kpathsea manual](https://tug.org/texinfohtml/kpathsea.html)
- [Microsoft: Run Linux commands from Windows](https://learn.microsoft.com/en-us/windows/wsl/filesystems#run-linux-tools-from-a-windows-command-line)
- [Microsoft: Basic commands for WSL](https://learn.microsoft.com/en-us/windows/wsl/basic-commands)
- [Microsoft: WSL FAQ](https://learn.microsoft.com/en-us/windows/wsl/faq)

# TeX Live探索の移植記録

更新: 2026-08-24

PraTeXはKpathsea libraryを再実装したものではない。基線となるsafe resolverは公開されている
`kpsewhich` CLIと`ls-R`の書式を境界にして、TeX Liveの探索結果をPraTeXへ接続する。
Linux-first checkpointでは、監査して必要最小限をvendorしたRust wrapperからsystem
`libkpathsea`をin-processで呼べる場合だけ先行させる。KpathseaのC sourceは移植・vendorせず、
libraryが使えない環境は同じsafe resolverへ戻す。

## 現在の探索順

一つのPraTeX実行中は、次の順で同じ`FileResolver`を使う。

1. 論理名そのものが通常fileなら、直接pathとして採用する。
2. formatがlocal限定policyなら、外部探索を行わず不在を返す。
3. 外部formatは`--engine=rtex`を必要とするため、program名しか渡せないin-process APIへ
   意味を読み替えず、従来のsafe resolverへ渡す。
4. その他の外部探索は、Unix nativeでsystem `libkpathsea`へlinkできた時だけ、一runに一個の
   handleへprogram名`pratex`、用途別format、`must_exist=true`を渡す。
5. linked hitは通常fileであることを再確認して採用し、linked missはauthoritativeな不在として
   safe resolverへ戻さない。library不在またはpath encoding非対応だけをtyped fallbackにする。
6. fallback後は同じfile種別と論理名について、run-localの成功・不在cacheを引く。
7. `ls-R`と用途別探索pathから、先行候補を飛ばさない一意なfileを証明できれば採用する。
8. 証明できなければ、用途を固定したone-shot `kpsewhich`へ問い合わせる。

索引は最終結果を推測するためではなく、外部processを省いても結果が変わらない場合だけ使う。
曖昧さ、壊れた入力、未対応のpath式はerrorへ潰さず、公開CLIへ戻す。

## in-process Kpathseaの依存境界

PraTeXは`kpathsea` 0.3.4と`kpathsea_sys` 0.2.3を基点にした監査済みforkを
`third_party/rust-kpathsea`へ固定する。crateのdefault featureは使わず、consumerから明示的に
`in-process-only-caller`だけを有効にし、そこから`system-probe`を解決する。`subprocess-backend`を有効にしないため、
system libraryを発見できなかったbuildでもRust crateが別の`kpsewhich` subprocessを構築することは
ない。typedな`InProcessUnavailable`を受けて初めてPraTeX自身のsafe resolverを遅延利用する。

依存は`cfg(all(unix, not(target_family = "wasm")))`のoptional target dependencyである。
WindowsはC返値とCRT allocatorの対応を実測できていないので、このcheckpointでは依存をcompileせず
typed fallbackに固定する。WASMも依存をcompileしない。この二環境の探索性能は改善しておらず、
full upstream APIやWindows fast pathの完成を意味しない。Linuxでもsystem libraryが共有・静的libraryを
提供しなければsafe resolverのままである。

featureの退行は次で検査する。これは`kpathsea`のdefault、`subprocess-backend`、`build-from-source`と、
`kpathsea_sys`のdefault、`build_from_source`が解決treeへ混入したら失敗する。

```powershell
tools/check-kpathsea-features.ps1
```

`system-probe`はunlinked時にもbuild時の検出用として`which`、`pkg-config`、`cc`を解決する。
従ってsourceをvendorしても、これらのcrateを初めて取得するmachineではCargo registry accessまたは
事前に固定したvendor sourceが必要である。依存取得不能をprobe無効化で黙って迂回しない。

### TeX Live 2026のexact libraryを用意する場合

TUGのLinux binary treeはKpathseaを各engineへlinkし、standalone `libkpathsea.so`をinstallしないことが
ある。そのtreeへ`KPATHSEA_LIB_DIR`を向けるだけでは解決しない。crateの`build-from-source`はTL2025を
pinしており、TL2026 oracleと版が違うのでPraTeXでは有効にしない。

共有・静的のどちらも、Cargoの前段で公式TL2026 source archiveまたはinstalled distributionと一致する
公式revisionを取得する。URL/revision、取得日、archive SHA-256、installed `kpsewhich --version`、
compiler/binutils/make版を記録し、generated headerと通常Automake buildを使う。crate内のTL2025
header/source listは流用しない。

2026-08-24のWSL Ubuntu実測では、CTANの年次snapshotをrepository外の`/tmp`へ取得した。
Kpathsea C sourceや生成物はPraTeXへvendorしていない。font assetもgate用外部TeX treeだけへ展開した。

| asset | URL | byte | SHA-256 |
|---|---|---:|---|
| TeX Live 2026 source snapshot | `https://mirrors.ctan.org/systems/texlive/Source/texlive-20260301-source.tar.xz` | 99,342,236 | `32ea827edd3fb80a682ffbdf95d7ba6139ff074516e660c8923260fc82f5e0f0` |
| uptex-fonts 2025-02-18 | `https://mirrors.ctan.org/install/fonts/uptex-fonts.tds.zip` | 4,961,904 | `d187b57c3abb5a31380b6798f0d374712a97dafccd1e33476fe6485008736a91` |
| Computer Modern TFM | `https://mirrors.ctan.org/fonts/cm/tfm.zip` | 69,512 | `9c0f99fa34c7d801c40f6b5ff60bc28f200e8ef6ffb2fe75e54ca835c67fc04c` |

source configure summaryはTeX Live 2026-03-02、Kpathsea 6.4.2だった。GCC 14.2.0、GNU ld 2.42、
GNU Make 4.3で、次のstandalone out-of-tree buildを使った。build directoryを`kpathsea`という
子directoryにするのは、生成した`kpathsea/c-auto.h`を`-I..`から読む公式build規則のためである。

```sh
mkdir -p "$WORK/build/kpathsea" "$PREFIX"
cd "$WORK/build/kpathsea"
"$SOURCE/texk/kpathsea/configure" \
  --prefix="$PREFIX" --enable-shared --disable-static \
  --disable-mktexmf-default --disable-mktexpk-default \
  --disable-mktextfm-default --disable-mktexfmt-default -C
make -j4
make install
PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig" cargo build --release --locked
```

このbuildの`libkpathsea.so.6.4.2`は450,904 byte、SHA-256
`1cb15a0e5de1b47f1f6ec2039ab0faa275bac8147ff3de5a6fd64df653e399ac`だった。これは同一artifactの
監査記録であり、絶対prefix等を含むためbyte再現性の保証値ではない。KpathseaはLGPL-2.1-or-laterであり、
共有libraryはPraTeX本体と別の外部prefixに置いた。

`tools/test-kpathsea-linux.sh`は、そのprefixと外部TeX treeを明示して次を一続きで検査する。

- `ldd`が指定prefixの`libkpathsea.so.6`を選ぶ。
- generic `TEXINPUTS`を不在pathへ向け、`TEXINPUTS.pratex`だけからTEXを引く。
- TEX、欧文TFM、JFM、VFのlinked hitと、用途別missを同じhandleで分ける。
- JFM no-copy runとlocal direct-path referenceのDVIをbyte比較する。
- `strace -f -e trace=process`でdistinct PIDが1、`clone` / `fork` / `vfork`が0である。

実測はignored linked test 1 passed、DVI 1 page / 260 byte、両経路のSHA-256
`49bd1e1cd78832c970e7d6283cee99213cb6e21e8a628fe299484e11d1eb81f9`で一致した。process traceも
distinct PID 1、子process生成call 0だった。VFは現在のDVI engineが先読みしないため、DVI runではなく
同じlinked resolverの用途別gateで確認している。これは探索意味とDVI不変の合格であり、利用者の
LaTeX corpusをupLaTeXと比較するend-to-end性能gateの達成ではない。

実行例:

```sh
PRATEX_KPATHSEA_PREFIX="$PREFIX" \
PRATEX_KPATHSEA_TEXMF_DIST="$PREFIX/share/texmf-dist" \
tools/test-kpathsea-linux.sh
```

制御されたstatic A/Bでは`--disable-shared --enable-static`で同じsourceをbuildし、
`KPATHSEA_LIB_DIR="$PREFIX/lib" KPATHSEA_STATIC=1 cargo build --release`とする。static配布にはexact LGPL
source・noticeと利用者がrelinkするためのmaterialが必要なので、releaseは共有libraryを優先する。
どちらもlibrary artifactと実際に探索するTeX treeは別物であり、runtime program名は`pratex`のままである。

`FileKind`ごとに`tex`、`tfm`、`vf`、`map`、`enc files`、`type1 fonts`、`afm`などの公開format名を
対応させる。TeX入力、`\input`、`\openin`、`\pdffilesize`、TFM、PDF map、encoding、
Type 1、AFM、Vaak入力は同じresolver契約を通る。2026-08-24のrun-global checkpoint以降、
Scanner、Output、遅延生成されるPDF font resource loaderは一つのrun-local instanceを共有し、
positive/negative cache、`ls-R` catalog、用途path、backend選択を分裂させない。
論理名と解決後の物理pathは別の型で保持し、TEXMF上の配置をDVI font名やPDF map keyへ漏らさない。

欧文TFMと、拡張子`.tfm`を共有するJFMはどちらも`FileKind::Tfm`で探す。font定義に現れた
拡張子なしの論理名はloader境界で`.tfm`を補ってresolverへ渡すが、fontのidentity、fmt、DVIへは
元の論理名だけを保持する。TeX Live上にJFMが**存在して解決できること**と、組版中のcurrent
Japanese fontにそれを**選択すること**は別である。明示した`\pratexjfont`またはclassのfont
hookを優先し、未選択のまま最初のCJK文字へ到達した時だけ`upjisr-h at 10pt`を遅延選択する。
この時点で初めて同じ`FileKind::Tfm` resolverを使うため、英文だけのrunへJFM探索costや必須資材を
持ち込まない。カレントに`upjisr-h.tfm`があれば外部探索前に採用する。それもTeX Live上のJFMも
見つからなかった場合に限り、`CJK typesetting needs a Japanese font metric`へ探索失敗理由を併記する。

VFは`FileKind::Vf`から公開CLIの`--format=vf`と用途別`--show-path=vf`へ対応し、同じrun-local
cacheと`ls-R` fast pathを使える。ただし通常のDVI生成でPraTeX自身はVFを展開しない。DVIに残した
論理font名からVFを読むconsumerは`dvipdfmx`等のDVI driverであり、そのdriver自身のkpathsea探索に
属する。将来、直接PDF backendがvirtual fontを展開する時のためにresolver用途は用意するが、
現在のfont定義時に未使用VFを先読みしない。

## `ls-R` fast path

初回の外部探索時に、次の固定argvで利用中のTeX環境自身へdatabaseを尋ねる。

```text
kpsewhich --all --must-exist --progname=pratex --format=ls-R -- ls-R
```

用途別の探索順は`kpsewhich --progname=pratex --show-path=<format>`の展開済み出力から取る。
別engineの名前を探索用aliasにも使わない。TeX Live 2026の実機では`pratex`をprogram名にしても
`upjisr-h`と`upjisg-h`のTFM/VFをすべて解決できることを確認している。
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
権限errorをWSLの別TeX Liveで覆わない。一度選んだnative/WSL backendは、run-local resolver
instanceの生存中は固定し、`clear_external_cache`でだけ再発見する。Scannerと
PDF font loaderも同じhandleを使うため、一runに一つのbackendを固定する。
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

配布JFMと対応VFの用途分離は、native TeX Liveまたは上記WSL bridgeを使える環境で次のignored試験を
有効にする。本文・表題用`upjisr-h`と見出し用`upjisg-h`のTFM/VFを同じresolver instanceから
二回ずつ解決し、物理fileを開けることとrun-local cacheの安定した結果を照合する。

```powershell
$env:PRATEX_TEST_TEXLIVE = '1'
cargo test --release file_search::tests::tex_liveのjfmとvfを用途別に解決できる -- --ignored
```

PraTeXからDVI driverまでのno-copy gateは、JFM/VFを作業directoryへ置かず、同じTeX Liveの
`kpsewhich`と`dvipdfmx`を使う。通常のTeX Liveでは`uptex-fonts`に加え、設定済みの和文font map、
CMap、物理fontが必要である。

```powershell
$env:PRATEX_TEST_TEXLIVE_E2E = '1'
$env:PRATEX_TEXLIVE_BIN = (Split-Path -Parent (Get-Command kpsewhich).Source)
cargo test --release --test japanese_glyph_dvi `
  実tex_liveが資材をコピーせず二つのjfmとvfをdvipdfmxまで解決する -- --ignored --exact
```

2026-08-24には隔離TeX Live 2026で`uptex-fonts` revision 74119、`dvipdfmx` revision 78409、
`ptex-fontmaps` revision 65953、Harano Aji revision 76078、Adobe CMap revision 66552を使った。
PraTeXは空の作業directoryから二つのJFMを解決して1 pageのDVIを生成し、`dvipdfmx`は同じtreeの
`upjisr-h.vf` / `upjisg-h.vf`を読み、明朝・ゴシックを別々のType 0/CID fontとして埋め込んだ。

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
原子的に切り替える必要がある。package取得とLSPを含む段階は
[監視・incremental実行roadmap](incremental-tooling-roadmap.md)へ分けた。

## 検証と一次資料

合成試験は、探索順、重複、stale entry、壊れたdatabase、`!!`、利用者tree、用途分離、
native/WSLを混ぜない条件、UNC変換、cache消去を固定する。2026-08-22時点で
`codex/kpse-lsr-index`の`dc1c554`では`cargo test --release --no-fail-fast`が455件通過、
失敗0、環境依存等4件skipである。

- [Kpathsea manual](https://tug.org/texinfohtml/kpathsea.html)
- [Microsoft: Run Linux commands from Windows](https://learn.microsoft.com/en-us/windows/wsl/filesystems#run-linux-tools-from-a-windows-command-line)
- [Microsoft: Basic commands for WSL](https://learn.microsoft.com/en-us/windows/wsl/basic-commands)
- [Microsoft: WSL FAQ](https://learn.microsoft.com/en-us/windows/wsl/faq)

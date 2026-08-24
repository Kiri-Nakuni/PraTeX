# TeX Live探索の移植記録

更新: 2026-08-24

PraTeXはKpathsea libraryを再実装しない。Linuxの既定buildは、監査して必要最小限をvendorした
Rust wrapperから、公式TeX Live 2026のexact revisionにあるKpathsea 6.4.2を静的にbuildし、
in-processで呼ぶ。KpathseaのC sourceはrepositoryへvendorせず、build時に固定revisionを取得するか、
同じsource treeを`KPATHSEA_SRC_DIR`で与える。Windows、WASM、未監査Unixと、明示的なsystem-library
buildでlibraryを利用できない場合だけ、公開`kpsewhich` CLIと`ls-R`を境界にしたsafe resolverへ戻す。

## 現在の探索順

一つのPraTeX実行中は、次の順で同じ`FileResolver`を使う。

1. 論理名そのものが通常fileなら、直接pathとして採用する。
2. formatがlocal限定policyなら、外部探索を行わず不在を返す。
3. 外部formatは`--engine=rtex`を必要とするため、program名しか渡せないin-process APIへ
   意味を読み替えず、従来のsafe resolverへ渡す。
4. その他の外部探索は、Linux既定buildでは静的に組み込んだKpathseaの一run一handleへ、
   program名`pratex`、用途別format、`must_exist=true`を渡す。明示的な`system-kpathsea` buildも、
   system libraryへlinkできた場合は同じ経路を使う。
5. linked hitは通常fileであることを再確認して採用し、linked missはauthoritativeな不在として
   safe resolverへ戻さない。library不在またはpath encoding非対応だけをtyped fallbackにする。
6. fallback後は同じfile種別と論理名について、run-localの成功・不在cacheを引く。
7. `ls-R`と用途別探索pathから、先行候補を飛ばさない一意なfileを証明できれば採用する。
8. 証明できなければ、用途を固定したone-shot `kpsewhich`へ問い合わせる。

索引は最終結果を推測するためではなく、外部processを省いても結果が変わらない場合だけ使う。
曖昧さ、壊れた入力、未対応のpath式はerrorへ潰さず、公開CLIへ戻す。

## in-process Kpathseaの依存境界

PraTeXは`kpathsea` 0.3.4と`kpathsea_sys` 0.2.3を基点にした監査済みforkを
`third_party/rust-kpathsea`へ固定する。crateのdefault featureは使わない。PraTeXのLinux既定feature
`bundled-kpathsea`は`in-process-only-caller`と`build-from-source`を明示し、そこから
`system-probe`を解決する。`subprocess-backend`はcompileしないため、source取得やbuildに失敗した時に
別の`kpsewhich`へ黙って性能退行せず、buildを失敗させる。明示的なsystem-library buildで受けた
typedな`InProcessUnavailable`、またはKpathseaをcompileしないtargetでだけ、PraTeX自身のsafe
resolverを遅延利用する。

依存は`cfg(target_os = "linux")`のoptional target dependencyである。
WindowsはC返値とCRT allocatorの対応を実測できていないので、このcheckpointでは依存をcompileせず
typed fallbackに固定する。WASMとその他Unixも依存をcompileしない。これらの環境の探索性能は改善しておらず、
full upstream APIやWindows fast pathの完成を意味しない。Linuxで配布側のlibraryを使う場合だけ、
`--no-default-features --features stats,system-kpathsea`を明示する。

featureの退行は次で検査する。Linuxについて`build-from-source`、`in-process-only-caller`、
`system-probe`を要求し、`kpathsea`のdefaultと`subprocess-backend`、`kpathsea_sys`のdefaultが
解決treeへ混入したら失敗する。WASMと未監査Unixに両crateが現れても失敗する。

```powershell
tools/check-kpathsea-features.ps1
```

`system-probe`はunlinked時にもbuild時の検出用として`which`、`pkg-config`、`cc`を解決する。
従ってsourceをvendorしても、これらのcrateを初めて取得するmachineではCargo registry accessまたは
事前に固定したvendor sourceが必要である。依存取得不能をprobe無効化で黙って迂回しない。

### TeX Live 2026のexact library

TUGのLinux binary treeはKpathseaを各engineへlinkし、standalone `libkpathsea.so`をinstallしない。
そのtreeへ`KPATHSEA_LIB_DIR`を向けるだけでは解決しないため、PraTeXのLinux既定buildは
`fb6158926661cb7a7246b3a94a0cb170a9624d5a`（TeX Live source mirror `svn78399`、
Kpathsea 6.4.2）をpinし、静的libraryを直接作る。`KPSE_REF`によるoverrideはdownstreamの明示的な
再build用であり、PraTeX releaseの版契約を変更しない。

sourceはbuild scriptが公式mirrorからsparse fetchする。networkを使えないbuildでは同じrevisionの
`texk/kpathsea` directoryを`KPATHSEA_SRC_DIR`で渡す。sourceを取得できない時はfail closedとし、
system libraryやCLIへ黙って切り替えない。既定featureと`KPATHSEA_NO_LINK`の併用もbuild errorにし、
unlinked診断buildは既定featureを明示的に外す。pinを更新する時はURL/revision、取得日、archive SHA-256、
source manifest、生成header、source list、公式`kpsewhich --version`を再監査する。

distributionが共有libraryを管理する場合は概ね次の形で別prefixへinstallし、その`.so`と`.pc`を
PraTeXの明示的なsystem-library buildへ渡す。

```sh
mkdir Work && cd Work
../configure --disable-native-texlive-build --enable-shared \
  --disable-static --disable-all-pkgs --prefix="$PREFIX"
make -C texk/kpathsea
make -C texk/kpathsea install
PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig" cargo build --release \
  --no-default-features --features stats,system-kpathsea
```

### 配布側libraryの手動実link gate

明示`system-kpathsea`経路を既定bundled経路から独立して検査するrunnerを
[`test-system-kpathsea-linux.sh`](../tools/test-system-kpathsea-linux.sh)に置く。runner自身は
downloadやlibrary buildを行わず、`PRATEX_KPATHSEA_PREFIX`と外部TeX treeを要求する。
一CPU jobでsystem featureだけをbuildし、指定prefixの`libkpathsea.so.6`を`ldd`で確認した後、
program別`TEXINPUTS.pratex`、TEX/TFM/JFM/VFのhit/miss、JFM no-copy DVI、
`strace -f -e trace=process`上の子process 0を一続きで検査する。

元の`codex2/kpathsea-linux-gate`で2026-08-24に使った外部資材は次である。repositoryへvendorしていない。

| asset | URL | bytes | SHA-256 |
|---|---|---:|---|
| TeX Live 2026 source snapshot | `https://mirrors.ctan.org/systems/texlive/Source/texlive-20260301-source.tar.xz` | 99,342,236 | `32ea827edd3fb80a682ffbdf95d7ba6139ff074516e660c8923260fc82f5e0f0` |
| uptex-fonts 2025-02-18 | `https://mirrors.ctan.org/install/fonts/uptex-fonts.tds.zip` | 4,961,904 | `d187b57c3abb5a31380b6798f0d374712a97dafccd1e33476fe6485008736a91` |
| Computer Modern TFM | `https://mirrors.ctan.org/fonts/cm/tfm.zip` | 69,512 | `9c0f99fa34c7d801c40f6b5ff60bc28f200e8ef6ffb2fe75e54ca835c67fc04c` |

当時のGCC 14.2.0、GNU ld 2.42、GNU Make 4.3による共有`libkpathsea.so.6.4.2`は
450,904 bytes、SHA-256 `1cb15a0e5de1b47f1f6ec2039ab0faa275bac8147ff3de5a6fd64df653e399ac`。
ignored linked test 1件が成功し、native/local DVIは260 bytes、SHA-256
`49bd1e1cd78832c970e7d6283cee99213cb6e21e8a628fe299484e11d1eb81f9`で一致、traceは一PID・
process生成0だった。`codex3/perf-integration`ではrunnerを現行featureへ移植したが、同じ外部artifactが
このmachineにないため再実行していない。既往値を現在binaryの性能値として扱わない。

```sh
PRATEX_KPATHSEA_PREFIX="$PREFIX" \
PRATEX_KPATHSEA_TEXMF_DIST="$PREFIX/share/texmf-dist" \
tools/test-system-kpathsea-linux.sh
```

2026-08-24のWSL buildでは、TeX Liveがなくsystem Kpathseaもない環境で、固定revisionを取得して
55個のC sourceからrelease binaryを5分19秒でlinkした。この値はcold source buildの成功記録であり、
runtime探索性能ではない。子process 0、hit/miss、DVI意味は別のruntime gateで固定する。

静的配布にはexact LGPL source・noticeと、利用者がPraTeXを改変済みKpathseaへrelinkできるmaterialが
必要である。source pinとbuild scriptだけで配布義務を満たしたとみなさない。共有・静的のどちらでも、
library artifactと実際に探索するTeX treeは別物であり、runtime program名は`pratex`のままである。

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

## Linux既定の組込みKpathsea実行gate（2026-08-24）

`codex2/perf-resolver-index`では、Linux既定featureが公式TeX Live 2026 revision `svn78399`
（Kpathsea 6.4.2、git commit
`fb6158926661cb7a7246b3a94a0cb170a9624d5a`）を静的に組み込む。sourceを取得できない時やC buildが
失敗した時は、外部`kpsewhich` backendへ黙って退行せずbuildを失敗させる。共有system Kpathseaを
明示選択するbuildは別featureとして残す。

[`../tools/test-bundled-kpathsea-linux.sh`](../tools/test-bundled-kpathsea-linux.sh)はrepository外の
一時directoryに合成`texmf-dist`、`texmf.cnf`、`ls-R`を作り、次を一つのgateで検査する。

- `TEXINPUTS.pratex`からTEX、`TFMFONTS`からLatin TFMとJFM、`VFFONTS`からVFを見つけ、各kindの
  missを`Missing`として返す。合成metric/VF byte列はresolver試験だけに使い、parseしない。
- release binaryの`ldd`に共有`libkpathsea`が現れない。
- TeX treeから見つけた入力と、同じ入力をcurrent directoryへ置いた実行のDVIがbyte一致する。
- `PATH`を空のtool directoryへ固定した実行を`strace -f -e trace=process`で観測し、PIDが一つ、
  `clone` / `clone3` / `fork` / `vfork`が0である。
- 固定`SOURCE_DATE_EPOCH`でDVI SHA-256が
  `658ec798192d67c3a067b8296a3300e580b2aaf7ba8b4fcc04dab78022848993`になる。

2026-08-24のTeX Liveを持たないUbuntu WSL実測では、55個のKpathsea C sourceをrelease linkし、
focused resolver test 1件と上のend-to-end gateが成功した。この試験が証明するのは合成treeの
対象lookup中に子processを起動しないこととDVI意味だけである。実TeX Liveの全`texmf.cnf`、alias、
`mktex*`、stale database、利用者treeの意味互換や、利用者の9.14秒corpusの短縮率はまだ証明しない。

```bash
tools/test-bundled-kpathsea-linux.sh
```

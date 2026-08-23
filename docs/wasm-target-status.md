# PraTeX自身のWASM target監査

更新: 2026-08-23

基点: `codex2/jlreq-script-spacing` の `67cd4bf`

実測target: `wasm32-wasip1`

この文書は、PraTeX自身をWASMへcompileできるかという問いと、生成物を実際のhostへ
埋め込んでTeX runを完走できるかという問いを分ける。前者は確認済みだが、後者は未完成で
ある。ここでの成功をPraTeX 1のWASM target release gate達成とは数えない。

## 実測結果

Rust 1.91.0 (`f8297e351`, host `x86_64-pc-windows-msvc`) で次を実行した。

```powershell
rustup target add wasm32-wasip1
cargo check --target wasm32-wasip1 --locked
cargo build --target wasm32-wasip1 --locked --bins
```

いずれもexit 0であり、source変更なしで`pratex.wasm`と`rtex.wasm`を生成した。debug生成物は
それぞれ33,000,325 byteと33,000,343 byteだった。debug path等を含み得るため、このsizeと
hashは互換oracleにはしない。

近日統合予定だった実時刻slice `codex2/runtime-date` の`003c27c`も同じtargetで別に確認した。

```powershell
cargo check --target wasm32-wasip1 --locked
cargo build --target wasm32-wasip1 --locked --bins
```

checkは43.36秒、buildは2分40秒でexit 0だった。`cargo tree --target wasm32-wasip1
--locked -e features`では`chrono`の`std` / `alloc`だけが選ばれ、OS時計用`clock`は選ばれない。
`Cargo.lock`に`iana-time-zone`やWindows関連packageが存在することと、WASI buildでそれらの
featureが選ばれることを混同しない。

実測machineにはWasmtime、Node、`wasm-tools`が無かったため、WASI runtime上の適合試験と
import/export監査はまだ行っていない。

## いま生成できるもの

生成物はWASI Preview 1の**command module**である。現在の二つのbinary entry pointは
`std::env::args_os`、stdin/stdout/stderr、WASI filesystem、`std::process::exit`を使う。
従って、runtimeが適切なdirectoryをpreopenし、引数とstreamを与える形なら実行可能な構造に
なっている。一方、ブラウザへ直接読み込む`wasm32-unknown-unknown` moduleでも、PraTeXを
関数として呼ぶ埋込みABIでもない。library targetも現状は`rlib`であり、host向け`.wasm`を
生成する`cdylib` facadeは無い。

実時刻sliceを含む場合、`wasm32-wasip1`は`cfg(any(unix, windows))`ではない。現在の
`RunDateTime::capture_local`はこのtargetで`LocalClockUnsupported`を返すため、
`SOURCE_DATE_EPOCH`をhostから渡さないrunは起動時に失敗する。compile成功だけではこのruntime
制約を検出できない。WASI時計を正式に採用するまでは、再現可能な`SOURCE_DATE_EPOCH`を必須に
するか、version付きhost clock capabilityを追加する必要がある。

## host境界として残るもの

### filesystem

TeX入力、fmt、TFM/JFM、font resource、DVI/PDF、logは`std::fs::File`と`Path`へ直接接続されて
いる。WASIではpreopenされたtree内なら利用できるが、browserや任意の埋込みhostへは持ち込め
ない。run単位のTeX treeをhost-owned VFSとして渡す境界が必要である。論理名と物理pathを
分ける既存resolver設計は再利用し、TeX coreの各consumerへ別々のfile callbackを足さない。

### 子process

`KpsewhichResolver`のnative/WSL fallbackは`std::process::Command`を使う。このAPI自体は
`wasm32-wasip1`でcompileできるが、通常のWASI runtimeはhost process起動を提供しないため、
missing fileでfallbackへ入ると実行時errorになる。WASM runでは子process fallbackを無効にし、
preindexed `ls-R` / TeX treeまたはhost resolverを注入しなければならない。

既存の`CommandExecutor`はprocess起動を合成試験から分離できているが、productionの
`Scanner`は`KpsewhichResolver::default()`を直接選ぶ。埋込みAPIではresolver、入力stream、
出力sink、clockを一つのrun contextとして注入し、WASI CLI adapterだけがOS実装を選ぶ形へ
分ける必要がある。

### 終了と診断

`tex_main`とcleanupは`std::process::exit`でrunを終える。WASI commandとしては`proc_exit`へ
対応するが、同じinstance内で複数runを扱うlibrary ABIではhostごと終了させてしまう。
coreは終了statusと生成物を返し、CLI adapterだけがprocess終了へ写す必要がある。

## safe Rustと依存監査

PraTeXの`src/`には`unsafe` block、`unsafe fn`、`unsafe impl`は無い。`wasm32-wasip1`対応にも
`unsafe`は追加しなかった。

ただしpath dependencyのVaakはdefault構成で`pub mod portable`を公開し、そのC ABIに
`vaak_run`と`vaak_free`という二つの`unsafe extern "C" fn`を持つ。PraTeXのbridgeはこの
raw-pointer APIを呼ばないが、依存crateの通常build対象からはfeatureで分離されていない。
PraTeX埋込み用Vaak境界では、safeな`embedding` APIだけを選べるfeatureまたはcrate分割を
検討し、portable C ABIのexport混入、binary size、license、再現性を改めて測る。GPL側の
PraTeX sourceをVaakへ写してはならない。

## 次の適合gate

1. Wasmtime等の固定versionを記録し、preopen treeと`SOURCE_DATE_EPOCH`を与えたplain TeXを
   DVIまで完走させる。
2. native buildとWASI buildのplain欧文DVI page bodyをopcode・座標込みで比較する。
3. missing fileで子processへ行かず、host resolverの決定的なnot-foundを返す。
4. 入力、fmt、font、出力、log、時計、終了statusをrun contextへ集約する。
5. command moduleとは別のversion付き埋込みABIを定め、複数run、資源上限、trap、診断、
   生成物所有権を試験する。
6. release buildのsizeと起動時間を測る。Vaakの未使用portable C ABIが最終moduleへexportされるか
   もbinary inspectorで確認する。

WASM module import/namespace仕様は
[`wasm-module-import-v0.1.md`](wasm-module-import-v0.1.md)、provider ABI実験は
[`wasm-provider-abi-v0.md`](wasm-provider-abi-v0.md)を一次資料とする。PraTeX自身のWASI target
と、文書からimportする外部WASM moduleは別の境界である。

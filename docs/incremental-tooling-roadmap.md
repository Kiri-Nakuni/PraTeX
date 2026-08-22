# 監視・incremental実行・package取得・LSP roadmap

更新: 2026-08-22

本書のdaemon、watcher、downloader、managed overlay、incremental checkpoint、LSP serverは
すべて設計のみであり、現在のproductionには実装されていない。

## 結論

PraTeXの実行器からsource位置、依存file、構造化診断、実行時のtoken意味を取り出せれば、
LSPによる**実行経路上のengine-confirmed semantic highlighting**は実装可能である。
編集後の全処理を毎回やり直さず、前の安全なcheckpointから再実行することも段階的には可能である。

ただしTeXは、catcode、macro展開、`\input`、条件分岐、I/Oによって後続sourceの文法と到達可能性を
実行中に変えられる。任意の文書について、未実行の全分岐を含む完全な意味、局所編集だけの更新、
停止保証を同時に提供する「本当の構文highlight」は不可能である。PraTeXは次の二層を契約にする。

1. 編集直後に返す、誤断定を避けた保守的なTeX字句highlight。
2. sandbox実行で実際に消費できたsource occurrenceへ重ねる、version付きsemantic overlay。

未到達、取消し、fuel切れ、error後の範囲を確定済みの色として見せない。partial/stale状態を
protocol上の結果IDと診断で明示する。

## 一実行を不変なepochにする

incremental化の前に、一回の実行が見た外部世界を固定する。`RunEpoch`は少なくとも次を持つ。

```text
RunEpoch {
  document_versions,
  vfs_generation,
  resolver_generation,
  format_hash,
  engine_options,
  provider_capabilities,
  fixed_clock
}
```

実行途中で`ls-R`や`texmf.cnf`が変わっても、そのepochの探索planは変えない。更新を検知したら
次epochを作る。古い実行はcancelできるが、新旧のpositive/negative cache、依存graph、診断、
semantic tokenを混ぜない。

現在のfile resolverはcache hitを`ls-R` snapshot再確認より先に返すため、長寿命daemonへ
そのまま移せない。resolver plan、database snapshot、成功・不在cacheを同じgenerationへ
結び、次のdatabaseを完全に読めた場合だけatomic swapする。壊れた更新を部分採用しない。

## TeX LiveとPraTeX overlayの監視

最初はportableなpollingから始める。毎回TEXMF tree全体を再帰走査せず、探索結果を決める
次のfileとPraTeX専用overlayだけをfingerprintする。

- 使用中の`ls-R`、`aliases`、`texmf.cnf`
- TeX Live package database（`texlive.tlpdb`）
- PraTeXのpackage lockとmanaged overlay index
- 読み込んだsource、fmt、TFM、font、map等の依存file

fingerprintはpath、size、mtimeに加え、利用可能ならfile identityまたはhashを持つ。同長・同mtimeの
置換も検出対象にする。native filesystem watcherはpollingを省くhintとして後から足す。eventは
debounceし、queue overflow、renameの取りこぼし、watcher errorではfingerprint再走査へ戻す。

変更の単位は`ResolverGeneration`である。設定file、database、overlay indexが変われば探索planと
positive/negative cacheを全消去する。通常source一件の変更なら依存graphから影響するroot文書を
dirtyにし、無関係なprojectを再実行しない。

## 不足packageの自動取得

engine内部で`\input`の途中にnetwork accessしない。resolverが
`MissingFile { kind, logical_name, attempted_roots, generation }`を構造化eventとして返し、外側の
orchestratorが必要なら取得planを作る。取得・検証・overlay更新後は**新epochで先頭または
checkpointから再実行**する。この分離によりTeXの一実行中の探索結果を安定させる。

自動取得は既定無効で、明示opt-inにする。offline/CI/LSP previewでは勝手にnetworkへ出ない。
LSPではまずcode actionとして「このlockへpackageを追加」を提示し、利用者の承認後にだけ行う。

### 解決とlock

TeX Liveの公開repository metadataから、要求した**正確な論理file**を含むpackageを求める。
`tlmgr search --global --file`相当を使う場合も利用者名をregexへ無加工で入れず、候補packageの
file inventoryを完全一致で再確認する。複数候補は暗黙に選ばない。

lockには少なくとも次を残す。

- repository URLとTeX Live release
- repository metadataのhash/signature状態
- package名、revision、依存package
- archiveのSHA-512、size、展開後file inventory
- license metadataと取得時刻ではなく再現可能なidentity

system/coexist profileでは、managed overlayに固定していないpackageだけを既存
TeX Liveから探す。一度packageをoverlayへ取得したら、そのpackageの全inventoryは
そのepochでoverlay側を正とし、同じpackageの不足fileをsystemの別revisionから補わない。
所有packageをmetadataから一意に決められない同名fileは自動取得せず診断する。
locked profileはlockとcontent-addressed cacheだけを使い、system treeへの暗黙fallbackを禁止する。
どちらもsystem TeX Liveや`TEXMFHOME`を書き換えない。

### downloadと展開境界

一時領域へ取得し、metadata signatureとlockしたhash/sizeを検証してからcontent-addressed cacheへ
atomic renameする。archive展開は絶対path、`..`、symlink/hardlink、device、重複path、case collision、
過大なfile数・一file・総展開量・圧縮率を拒む。初期版ではinstallerのpostactionや任意scriptを
実行しない。packageが生成fileを要する場合は、対応済みの宣言的生成だけを別capabilityにする。

managed overlayはread-only snapshotとし、profileとlockから作ったpackage所有表に従って
resolver planへ組み込む。利用者がpath単位で優先順を変え、一つのpackageのrevisionを
混ぜる構成は認めない。更新中のdirectoryを実行中epochへ見せない。garbage collectionは
使用中generationとlockから到達できるcontentを保護し、別commandとして行う。

## source occurrenceと構造化event

TeXの`Token` identityへsource rangeを直接埋めない。一つのmacro tokenは複数箇所で実行され、fmtへ
永続化もされるためである。入力時に次のsidecarを作り、展開・再挿入時はprovenanceを別eventとして
つなぐ。

```text
SourceOccurrence {
  source_id,
  byte_range,
  document_version,
  provenance_id
}
```

最初の段階では、入力fileから直接消費したtokenだけを正確に追跡する。macro replacement、
`\scantokens`、`\readline`、Vaak/WASM生成入力、`\write`再読込みなどのvirtual sourceは固有
`SourceId`を持たせる。raw bytesとUTF-16 LSP positionの変換表はdocument versionごとに保持し、
不正UTF-8を一文字へlossy変換して位置をずらさない。

Loggerの文字列出力を解析してLSP診断を作らない。engineはcode、severity、primary range、related
range、help、epoch/result IDを持つstructured diagnostic eventを発行し、従来log rendererは
そのconsumerにする。semantic eventも「そのoccurrenceが実行時に何として読まれたか」を返す。

## preview sandbox

LSP用実行は通常のCLIと別profileにする。

- 読込みはVFS snapshotと固定したresolver generationだけ
- `\openout` / `\write`等はmemory VFSへ向け、利用者fileを変更しない
- output routineは意味のため実行するが、DVI/PDFの最終writeはdrop可能
- network、process、terminal prompt、実clockを無効にする
- Vaak/WASM/providerは既定無効。明示許可時もcapability、fuel、memory、batch上限を固定
- instruction/time/node/input-depth上限でcancel可能にする

通常のTeX実行と観測可能な意味が異なる点はdiagnostic metadataへ出す。「previewで通ったから
通常実行も必ず通る」とは保証しない。

## checkpointとincremental再実行

TeXの状態はscannerだけではない。安全なcheckpointは少なくとも次の整合したsnapshotを要する。

- input stackと各source位置、scanner status、alignment state
- macro definition/general-textの途中状態、`def_ref`、argument/templateの蓄積、全read streamの位置
- condition/group/save stack、after-token
- Eqtb、macro、catcode/kcatcode、名前空間、region
- font、hyphenation、nest、page builder、alignment、output routine state
- memory VFS positions、resolver generation、options、provider capability state
- source occurrence/provenanceとdependency graphのcursor

最初はdocument先頭からのfull rerunで正しいevent modelを作る。その後、top-level入力境界や
明示checkpoint primitiveのように、page builderが空で外部副作用がなく、macro definitionや
general-text走査の途中でなく、`def_ref`とargument/template蓄積が空である安全点だけを
保存する。この不変条件を緩める段では、上記の途中状態とread stream位置をsnapshot対象に加える。
巨大なEqtbやmacro表はcopy-on-writeまたはjournalで世代共有し、毎checkpointの全cloneを避ける。

編集位置より前で、同じ依存hash・resolver generation・format/options/providerを持つ最後の
checkpointから再開する。再実行結果のstate hashが旧checkpointと一致した時だけ後続eventを
再利用する。TeXの動的性質上、一致を証明できなければ先へ再実行する。

## LSP表面

段階的なprotocol表面は次である。

1. `textDocument/didOpen` / `didChange`でversion付きVFSを同期し、古いepochをcancelする。
2. `publishDiagnostics`で同じdocument versionのstructured診断を置換送信する。空配列も送る。
3. semantic tokens fullで、engine-confirmed範囲が保守的layerを置換した一つの
   重複なしstreamを返す。clientが`overlappingTokenSupport`を明示した場合だけ重複表現を
   別に検討し、`augmentsSyntaxTokens`は登録したserver capabilityと実際の応答を一致させる。
4. stableなresult IDを作れた後にsemantic tokens deltaを足す。
5. missing package、停止limit、sandbox差をcode action/related informationへ出す。

連続入力はdebounceし、最新versionだけをpublishする。cancelされた実行の診断やtokenを新versionへ
流さない。dependency側の診断もroot project/epoch ownershipを持たせ、fileを閉じただけで誤って
消さない。

## 段階

| 段 | 内容 | 完了条件 |
|---|---|---|
| M0 | resolver generationとpoll fingerprint | positive/negative cacheを同世代でatomic swap |
| M1 | watcher hintとdependency graph | overflow時poll fallback、変更projectだけdirty |
| P0 | structured MissingFileとopt-in plan | engine中network 0、曖昧候補を選ばない |
| P1 | signed/hash lockとcontent-addressed cache | offline再現、system tree変更0 |
| P2 | bounded展開とmanaged overlay | traversal/link/device/bomb拒否、atomic publish |
| H0 | source occurrence sidecar | Token/fmt identity不変、byte range round-trip |
| H1 | structured diagnostic/semantic event | legacy logとLSPが同じeventをconsume |
| H2 | sandbox full rerun LSP | version/cancel/partial表示、外部副作用0 |
| H3 | checkpointとCOW/journal | state一致時だけ後続event再利用 |
| H4 | semantic deltaとpackage code action | stale結果0、fullとの差分一致 |

## 性能と正しさの条件

- 監視を使わない通常CLIではwatcher thread、hash、LSP allocationを0にする。
- epoch中はfile queryごとにgeneration lockを取らず、不変snapshot handleを読む。
- semantic追跡無効時はToken sizeとfmtを変えない。
- full rerunのstdout/log/DVI/PDFとpreview event consumer導入前を比較する。
- incremental結果は同じVFS/epochのfull rerunとdiagnostic・semantic event・出力hashを比較する。
- package取得のnetwork/cache hit/missをengine組版benchmarkへ混ぜない。
- cancel、watch overflow、壊れたDB/archive、provider trapをfuzz/property fixtureで固定する。

## 一次資料

- [Kpathsea manual](https://tug.org/texinfohtml/kpathsea.html)
- [TeX Live Manager manual](https://tug.org/texlive/doc/tlmgr.html)
- [TeX Live package object format](https://www.tug.org/texlive/doc/tlpkgdoc/TLPOBJ.html)
- [LSP 3.18: textDocument/didChange](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.18/textDocument/didChange.md)
- [LSP 3.18: publishDiagnostics](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.18/language/publishDiagnostics.md)
- [LSP 3.18: semantic tokens](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.18/language/semanticTokens.md)
- [TeX Live探索の移植記録](kpathsea-port-notes.md)

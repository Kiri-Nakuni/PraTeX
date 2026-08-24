# PraTeX → Vaak: 埋込み境界の現在地と次回連絡条件

- date: 2026-08-25 00:51:01 +0900
- in_reply_to: なし（初回の状態共有）
- target_branch: `codex3/perf-integration`
- target_commit: `b435e1df247191d998fa2afb8608bb481750a170`
- target_layer: embedded Vaak public API / PraTeX phase integration

## 結論

現在のPraTeX統合checkpointから、Vaak repositoryへ直ちに要求するAPI変更はない。
PraTeXは同一入力・同一TeX tree・同等DVIでupTeX系比1.3未満を先に測る段階にあり、
この間にVaakの意味論や公開APIを推測して変更しないでほしい。

既存のtop-level `PreparedProgram`、`HostLayout`、再利用可能な`EmbeddingRunner`、compile時host function
index、read/write/touched解析は利用済みであり、これらを新規課題として作り直す必要はない。

## 現在のPraTeX側境界

- 標準JLReq、JFM、禁則、和文自動間隔はPraTeXのBuiltIn経路に置き、Vaak/WASM call 0を維持する。
- compiled script-spacing tableは、PraTeX側validator/compilerが全候補を検証した後だけrun-localに
  install/revokeできるhost入口まで接続した。
- Vaak runtimeからのprovider登録、phase registry、live-node API、affine lease、RegionNode、
  indirect edgeは未実装である。
- provider handle、compiled table、runner途中状態、generation、leaseをfmtへ保存しない。
- providerがない場合または登録前検証に失敗した場合は、部分適用せずPraTeX BuiltInへ戻る。

## Vaak側へ次に相談する条件

PraTeX側のE3 explicit one-provider phase registryとdisabled-path計測が固まった後、次の順に
一般的な公開契約を相談する。PraTeX固有node、spacing、JFM型をVaakへ入れない。

1. `Paradox`、host runtime error、layout mismatch、返値型違反、host effectを分けるtyped completion。
2. scalar `Leaf` host callのwarm heap allocation 0、またはRunner-owned scratchの再利用。
3. named entryと型検査済み明示引数を束縛し、layout descriptorをexact照合するgeneric entry ABI。
4. 算術・cast・fmt永続化ができないhost-owned nominal token。
5. 非live phaseだけを対象に、借用を保持しないaffine `MaySuspend` start/resumeとfuel/cancel境界。

Vaak側にはPraTeX固有の`PhaseId`、node型、JFM class、capability policyを持ち込まない。初版の
host functionは現行構文でprepare時にindex化できるbare nameとし、qualified namespaceを前提にしない。
capability/effectの意味、grant、lease、phase順序はPraTeXが所有し、Vaakにはlayout-local IDと
generic entry contractだけを渡す。

WASM runtimeはVaakの依存にする必要がない。必要時はVaakがtyped `HostRequest`を返し、PraTeXが
外側でversion付きbulk ABIを一phase 0回または1回だけ実行する二車線を維持する。

## fmt / run-local / fallback契約

- fmtへ保存できるのはdeclarativeなsource/descriptorと要求versionだけである。
- fmt loadはprovider activationではない。prepare、capability承認、registrationは新しいRunEpochで行う。
- registrationは初版ではrun-localだけとし、fmt restore、engine reset、別projectで失効する。
- live token、capability grant、runner、cache generation、WASM handleをTeX registerやfmtへ保存しない。
- trap、fuel切れ、不正ID、不正actionではbatch全体を破棄し、定義されたBuiltIn fallbackへ戻る。

## 権利境界

VaakのMIT公開APIをPraTeXが利用する向きだけを採る。PraTeXのGPL-3.0 source、test、本文を
Vaak repositoryへ転記しない。この連絡で共有するのは要求、不変条件、測定値、失敗条件だけであり、
Vaak側は自身の設計と参照実装から独立に実装する。

## 証拠

- `docs/vaak-embedding-api-design.md`: E0--E10、責任分担、未決事項の正本。
- `docs/wasm-provider-abi-v0.md`: 外向きWASM ABI 0.0のversion/capability/wire/fallback境界。
- `docs/wasm-module-import-v0.1.md`: module import、namespace、transactionの別version domain。
- `docs/feature-inventory.md`: 現在の実装状態。
- `for_CLAUDE.md`: 過去連絡と`codex3/perf-integration`統合checkpoint。

## 未検証事項

- disabled pathのphase境界`Option`一回がend-to-endで0.5%以内かは未測定。
- Vaak runtimeを使う実provider registrationはまだない。
- typed completion、allocation-free Leaf、nominal token、MaySuspendは現行公開APIで未確認である。
- failure時に`host_after`とeffectを失わないtransition、fuel/memory/cancelの決定的上限も未確認である。
- daemonでprepared objectへ`Send + Sync`を要求するかは未決で、初版はengine-localを想定する。

# PraTeX WASMモジュールimport・名前空間仕様 0.1案

状態：基本方針決定済み。control sequence実行ABIは別途策定する。

## 1. 基本方針

PraTeXのWASMモジュールは、hostであるPraTeXが合成・管理する独立したcore WASM moduleとする。

- WASM module同士の直接importは認めない。
- module間でmemory、table、mutable global、instanceを共有しない。
- code共有が必要なら、module生成時に一つのWASM binaryへ静的linkする。
- Component ModelおよびWITは将来のadapter候補に留め、v1の安定面にしない。
- moduleの読込み順はTeX入力の順序そのものとし、PraTeXが依存graphを用いて並べ替えない。

## 2. import primitive

次の非展開・prefix可能なprimitiveを追加する。

```tex
\importwasmmodule
```

`\edef`や`\write`などの展開走査中にはmoduleをimportしない。

### 2.1 Knuth TeX型

```tex
\importwasmmodule foo as bar
\importwasmmodule foo
```

- `foo`はmodule名。
- `as`は`\advance ... by ...`の`by`と同様のASCII keywordである。
- `as`はASCII大文字小文字を区別しない。
- `as`以降を省略した場合、module名をnamespace名として使う。

### 2.2 balanced text型

```tex
\importwasmmodule{foo}{bar}
\importwasmmodule{foo}
```

- 第1群はmodule名。
- 第2群はnamespace名。
- 第2群を省略した場合、module名をnamespace名として使う。
- 第1群の直後に、spaceを飛ばさず見た次の未展開tokenがbegin-groupなら、第2引数として読む。
- spaceがあれば一引数形式を終了する。

```tex
\importwasmmodule{foo}{bar}  % namespace bar
\importwasmmodule{foo} {bar} % namespace foo、その後に通常の群
```

一引数形式の直後に確実に群を置きたい場合は、spaceまたは`\relax`で区切れる。

明示的な空namespaceである次の形式は誤りとする。

```tex
\importwasmmodule{foo}{}
```

namespace省略とglobal空間に、それぞれ既に別の表記があるためである。

## 3. namespace

WASM moduleは、namespaceを含まないlocal control-sequence名の一覧を公開する。

PraTeXはimport時に、それらを指定された既存のPraTeX namespaceへ束縛する。namespace aliasは次を変更しない。

- module identity
- module hash
- provider key
- ABI version
- capability
- dependency identity
- guestへ渡すexport ID

namespaceを省略した場合、展開・検証後の論理module名をnamespace名とする。

### 3.1 global keyword

namespace位置のASCII keyword `global`は、namespaceなしのglobal control-sequence空間を表す。

```tex
\importwasmmodule foo as global
\importwasmmodule{foo}{global}
```

`global`はcontrol sequenceではなくkeywordであり、ASCII大文字小文字を区別しない。namespace引数全体が正確に`global`である場合だけ特別扱いする。

したがって、`globalx`などを`global`の前方一致として扱わない。

このprimitiveでは、文字列`global`そのものを通常namespace名として指定できない。

### 3.2 複数alias

同じexact module identityを複数のnamespaceへimportしてよい。

```tex
\importwasmmodule foo as a
\importwasmmodule foo as b
```

この場合、control sequence bindingは二組作るが、基礎となるmodule registrationとprovider tableを二重登録しない。

## 4. source-order import

`\importwasmmodule`は、読まれた地点で直ちに処理する。

```tex
\importwasmmodule base
\importwasmmodule ruby
```

この場合、常に`base`、次に`ruby`の順である。

PraTeXは次を行わない。

- dependencyの自動import
- 再帰的なmodule探索
- topological sort
- `before` / `after` / `priority`による暗黙の並べ替え
- 後から現れるmoduleを待つ遅延登録
- 実行中のnetwork取得

## 5. module dependency

runtime dependencyはmanifestに宣言できるが、依存先はimport時点ですでにactiveでなければならない。

依存先が未importなら、その場でTeX errorとし、現在のimportを何も登録せず終了する。

```tex
\importwasmmodule ruby % baseが必要で未importなら、この地点で失敗
```

依存先のscopeは、依存するmoduleのscopeと同じか、それより外側でなければならない。localな依存先に依存するmoduleをglobal importすることはできない。

この規則により、循環依存は成立しない。

v1ではoptional runtime dependencyを持たせない。build-timeのcode dependencyはWASM生成時に静的linkする。

同じ論理module名について、異なるhash/versionのmoduleを同時にactiveにしない。同じexact identityの再importはaliasまたはscope bindingの追加として扱う。

## 6. providerの順序

moduleのimport順はsource orderであるが、spacing tableなどの内部rule解決をregistration orderへ依存させない。

- provider keyとprofile keyは一意でなければならない。
- 曖昧なtable overlapとduplicateは拒否する。
- 同じhook/profileへ複数providerを暗黙に連結しない。
- v1では一つのhook/profileにつき一つのactive provider、またはTeX側で明示的に選択されたnamed profileだけを使う。
- 将来複数providerを合成する場合も、hostの隠れたdependency graphではなく、TeXから観測できる明示構文を別途定義する。

`docs/vaak-embedding-api-design.md`のphase provider用`before` / `after` graphは、外向きWASM module importへ適用しない。WASM moduleによる複数phase provider合成は、別仕様ができるまで許可しない。

## 7. activation authority

`\importwasmmodule`はactivationのrequestであって、権限のgrantではない。

RunEpoch開始前に、CLI、orchestrator、またはembedding APIが、次を固定したrun-local module catalogを用意する。

```text
logical module name
exact module bytes
SHA-256
exact byte length
manifest identity/version
WASM ABI range
許可するcapability
operation set
limits
fuel model
failure policy
```

TeX sourceはこのcatalogに登録済みの論理名だけを要求できる。

`\importwasmmodule`は次を行わない。

- 任意pathやURLの指定
- filesystem探索
- `kpsewhich`子process起動
- network取得
- module hashの変更
- capabilityの自己付与
- limitsやfailure policyの変更

module fileがTeX treeに存在するだけでは承認にならない。

WASM runtime依存はCargoのdefault-off optional featureとし、feature付きbuildでも、承認済みcatalogとleaseがなければimportできない。

## 8. import transaction

importは原子的に処理する。

1. 引数を走査する。
2. module名をrun-local catalogで解決する。
3. module hash、length、manifest、WASM profileを検証する。
4. ABI、feature、capability、limits、fuelを照合する。
5. すでにactiveなdependencyとscopeを検査する。
6. 全control-sequence export名とnamespaceを検証する。
7. control-sequence容量、provider table容量などを事前確保する。
8. provider proposalを一時領域で全検証する。
9. すべて成功した場合だけ、control sequenceとprovider activationを一括commitする。

どこか一つでも失敗した場合、次を一切変更しない。

- Eqtb
- control sequence
- namespace table
- provider registry
- spacing/unit table
- capability state
- module activation count
- cache generation

control-sequence名の既存定義との衝突は、通常のTeX定義と同じく選択されたscopeで置換し、群を抜けたときに以前の意味へ戻す。ただしmodule manifest内の重複exportや、不正なprovider key/table conflictはimport全体の誤りとする。

## 9. 群、`\global`、`\globaldefs`

`\importwasmmodule`は定義系primitiveとしてsave stackを通す。

```tex
{
  \importwasmmodule foo as bar
  % bar namespaceのexportとprovider activationが有効
}
% control sequenceとprovider activationが以前の状態へ戻る
```

次を許す。

```tex
\global\importwasmmodule foo as bar
```

`\globaldefs`は既存TeX規則どおり適用する。

- `\globaldefs > 0`：global import
- `\globaldefs = 0`：明示した`\global` prefixに従う
- `\globaldefs < 0`：`\global`があってもlocal import

`\long`、`\outer`、`\protected`は`\importwasmmodule`へ適用できないprefixとして既存方式で診断する。

`as global`は公開先control-sequence空間の指定であり、`\global` prefixは定義の寿命指定である。両者を混同しない。

```tex
\global\importwasmmodule foo as global
```

これは「global scopeで、global control-sequence空間へ公開する」という意味である。

## 10. registrationとscope bindingの分離

既存WASM ABIのRunEpoch-local registrationと、TeXのgroup-local意味を次の二層に分ける。

```text
ModuleRegistration
    RunEpoch-local
    exact module identity、検証済みtable、compiled moduleを所有

ModuleActivationBinding
    TeX group-localまたはglobal
    control-sequence公開先とprovider activationを所有
```

最初のimportで承認済みleaseを消費し、`ModuleRegistration`を作る。同じexact moduleを再importする場合、既存registrationへ新しいscope bindingを追加する。

群を抜けた場合、observableなbindingとprovider activationは消える。ただし、immutableなcompiled moduleや検証済みregistration objectは、意味を持たないdormant cacheとしてRunEpoch終了まで残してよい。

これはTeXが定義を巻き戻しても内部確保領域を直ちに縮めるとは限らないことと同じであり、文書から観測できない。

WASM instance、linear memory、mutable global、mailboxは再利用せず、各invocationでfresh instanceを使う。

## 11. semantic-unit snapshot

provider activationがgroup内で変化しても、実行中のsemantic unitへ可変registryを直接見せない。

paragraph、hpack、unit context、phase invocationなどは、定められた入口でactive registryのimmutable snapshotを取得する。

- control sequence bindingはimport commit直後から有効。
- 寸法単位は各中央`scan_dimen`開始時のregistryを使う。
- spacing providerはlistまたはspacing finalizer仕様で定めたsnapshotを使う。
- phase providerはphase entry時のsnapshotを使う。
- phase実行中にgroupが終了しても、そのphaseのsnapshotは完了まで不変。
- 次のsemantic unitは、group rollback後のregistryを取得する。

hot loopでmodule registryを文字ごとに引き直さない。provider無効時には既存どおり外側の`Option`判定以外の費用を加えない。

各capabilityがどの地点でsnapshotされるかは、そのcapabilityのABI仕様に必ず記載する。

## 12. fmt

fmtへ次を保存しない。

- active module registration
- activation binding
- capability lease
- compiled WASM module
- instance、memory、mailbox
- RegistrationId
- provider-local ID
- runtime table/cache generation
- fuel/cancel途中状態
- module-backed control sequenceのrun-local handle

v1では、activeなWASM module bindingが一つでも存在する状態で`\dump`を実行した場合、明示的なerrorとする。run-local handleをfmtへ黙って保存したり、module-backed control sequenceを未接続stubへ変換したりしない。

次のように、将来importを実行するmacroやtoken list自体はfmtへ保存できる。

```tex
\def\activatefoo{\importwasmmodule foo}
```

fmt load自体はactivation eventではない。fmt load後の明示commandまたはengine APIが改めてimport requestを行い、current RunEpochのpolicy approvalを要求する。

## 13. control sequence exportと現行ABIの境界

このimport仕様は、moduleが宣言したlocal export名をPraTeX namespaceへ束縛する規則を定める。

ただし、現行`WASM provider ABI 0.0`が定義するoperationは次の四つだけである。

1. `SpacingTableUpload`
2. `SpacingBatch`
3. `UnitTableUpload`
4. `UnitContextBatch`

ABI 0.0は、任意control sequence invocation、token emission、TeX scannerへの再入を定義していない。

したがってABI 0.0 moduleは、control sequence exportを0件とし、spacing/unit providerだけを登録できる。あるいは、PraTeX hostが完全に意味を所有する宣言的なselector commandだけを公開できる。

任意のmodule-backed control sequenceを実行するには、別version domainまたはABI 0.1以降で、少なくとも次を定義する必要がある。

- export IDとcommand signature
- expandable / unexpandableの別
- hostが引数を走査するtyped argument schema
- request/response wire format
- 許されるeffect
- token生成の可否
- assignment/save-stackとの関係
- fuel、call limit、cancel
- error recovery
- fresh-instance規則
- phase/scanner/output routineへの再入禁止

最初のcommand ABIでは、次を推奨する。

- unexpandable commandだけ
- 引数はhostが宣言的signatureに従って走査する
- WASMへ渡す前にowned snapshotへ変換する
- live scanner、Eqtb、node、token-list handleを渡さない
- WASMからTeX scannerやoutput routineへ再入しない
- runtime token emissionを許可しない
- responseは検証済みのtyped proposalだけ
- 全結果を検証してから原子的にcommitする

このcommand ABIが決まるまで、import・namespace仕様とcontrol sequenceの実行意味を混ぜない。

## 14. error model

最低限、次の診断を区別する。

```text
WasmRuntimeUnavailable
UnknownWasmModule
ModuleNotApproved
ModuleIdentityMismatch
AbiMismatch
CapabilityDenied
MissingModuleDependency
ModuleDependencyScopeMismatch
ConflictingModuleVersion
InvalidModuleManifest
InvalidModuleExport
EmptyWasmNamespace
ControlSequenceCapacityExceeded
ProviderConflict
ActiveWasmModuleCannotBeDumped
```

すべてのimport errorは、module名、import位置、失敗段階、安定error codeを記録する。guestの自由文診断は補助情報であり、互換面にはしない。

## 15. 必須試験

- `Knuth型のas付きimportが指定namespaceへ公開する`
- `Knuth型のnamespace省略はmodule名を使う`
- `balanced型の二引数が指定namespaceへ公開する`
- `balanced型の一引数がmodule名を使う`
- `balanced型はspace後の群をnamespaceとして奪わない`
- `global keywordがglobal control sequence空間を選ぶ`
- `globalxをglobal keywordの前方一致にしない`
- `空namespaceを拒否する`
- `moduleをsource orderどおり登録する`
- `後から現れるdependencyを待たない`
- `未import dependencyで何も部分登録しない`
- `dependencyより長寿命のimportを拒否する`
- `同じmoduleを別namespaceへ束縛してもproviderを二重登録しない`
- `異なるhashの同名moduleを同時にactiveにしない`
- `local importは群終了時にcontrol sequenceとproviderを共に戻す`
- `global importは群終了後も残る`
- `globaldefs正負がimportへ効く`
- `as globalとglobal prefixを混同しない`
- `失敗したimportがeqtbとprovider registryを変更しない`
- `moduleがcatalogに存在するだけでは権限を得ない`
- `TeX sourceからpath URL networkを指定できない`
- `provider内部のrule選択をregistration orderへ依存させない`
- `phase開始後のregistry変更が現在のsnapshotへ混ざらない`
- `active moduleを含むfmt dumpを拒否する`
- `fmt loadだけではmoduleをactivateしない`
- `WASM無効buildでimportが構造化errorになり状態を変更しない`
- `control sequence export未対応ABIを明示的に拒否する`
- `group rollback後もdormant cacheが文書から観測できない`

## 16. 既存設計との整合

| 既存設計 | 本仕様での整合方法 |
|---|---|
| TeX sourceはcapabilityを自己付与できない | `\importwasmmodule`はrequestだけ。事前承認済みrun-local catalogがgrantを所有する |
| module fileの存在はactivationではない | catalog登録、hash固定、policy approvalが別途必要 |
| provider registrationはRunEpoch-local | run-local registrationとgroup-local activation bindingを分離する |
| leaseは一度だけconsumeする | 最初のregistrationだけがconsumeし、alias/reimportは既存registrationへbindingを追加する |
| instanceとmemoryはfresh | import後もinvocationごとにfresh instanceを使う |
| token emissionとscanner再入は禁止 | importはhostがCommandを束縛する操作。guestによるtoken emissionではない |
| provider tableは登録順を意味にしない | source orderはactivation順だけ。table overlap、duplicate、同一profile競合は拒否する |
| fmtへactive registrationを保存しない | active binding中のdumpを拒否し、fmt loadをactivationにしない |
| disabled pathはhot-loop費用0 | semantic-unit入口の外側Optionだけを許し、文字ごとのregistry/WASM callを置かない |
| phase providerのdependency graph案 | 外向きWASM moduleには適用しない。v1は一hook一providerまたはTeXからの明示選択に限定する |

## 17. 残る未決事項

1. module名の文字集合、正規化、最大長
2. module manifestとcatalog/lock形式
3. module version constraintの表記
4. module-backed control sequenceの実行ABI
5. control sequenceをexpandableにできる条件
6. token生成を将来許す場合のowned token wire schema
7. 複数providerを明示合成するTeX構文
8. module署名と配布package形式

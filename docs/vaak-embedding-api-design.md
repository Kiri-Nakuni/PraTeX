# PraTeX における Vaak 埋め込み内部 API 設計

更新: 2026-08-23
状態: **設計案。phase registry、node API、WASM bridge は未実装**

外向きWASM境界の固定mailbox、wire schema、capability、fuel、atomic fallbackは
[WASM provider ABI 0.0](wasm-provider-abi-v0.md)で別version domainとして定義する。

## 1. 結論と適用範囲

PraTeX の拡張経路は、次の二車線に分ける。

1. **内蔵 Vaak**は、同一 process 内で一つの組版 phase をまとめて処理する。高頻度の
   node 問い合わせは、名前解決済みの native host call を使う。
2. **外向き WASM ABI**は、複雑だが呼出し回数を抑えられる処理だけを、phase あたり原則
   0 回または 1 回の bulk request として扱う。node ごとの getter/setter を WASM import にしない。

ただし、pTeX 互換、JFM、自動和文間隔、禁則、縦組および一級 JLReq 規則は PraTeX の
engine core に置く。**標準日本語組版は Vaak/WASM を一度も呼ばずに完結しなければならない。**
Vaak/WASM は利用者・出版社固有の規則か、明示的に選んだ実験機能だけに使う。

本書でいう「phase hook」は LuaTeX 型の常設 callback 表ではない。次を不変条件とする。

- Vaak source を prepare しただけでは phase は有効にならない。
- `\directvaak`、`\vaakdef`、`\vaakinput`を実行しただけでも phase は有効にならない。
- 許可された Vaak 実行または PraTeX の明示 API が、特定 phase への登録を要求し、登録全体の
  検証に成功した時だけ、その PraTeX run 中の phase が有効になる。
- 登録が一件もない通常実行では、node ごとの分岐、host call、WASM call、allocation、lock を
  一切増やさない。phase 境界の外側に置く `Option` 判定一回だけを上限とする。
- 登録、capability handle、node handle、runner の途中状態を fmt へ保存しない。

添付の研究結果は有用な設計資料だが、PraTeX や Vaak の仕様ではない。本書は現行 checkout の
公開 Rust API と `src/vaak.rs` を照合し、重複、未実装、未決事項を分けている。

## 2. 現行実装の監査

### 2.1 PraTeX 側ですでに動くもの

`src/vaak.rs` の現行 bridge は、phase API ではないが、次をすでに実装している。

| 項目 | 現状 |
|---|---|
| 明示実行 | `\directvaak`、`\vaakdef`、`\vaakinput`を明示した時だけ Vaak を走らせる。暗黙 callback はない |
| compile cache | source byte 列を鍵に parse/check/type-check/compile 済み `Program2` を再利用する |
| named body | `\vaakdef` は定義時に compile し、呼出し時は `u32` ID から引く |
| Runner | thread-local な `vaak::vm::Runner` を繰り返し利用し、arena/stack/frame の capacity を再利用する |
| host value | `count[0..255]` と `dimen[0..255]` を `i32 array` として見せる |
| touch analysis | `Program2::host_used` / `host_touched` により、実際に参照する register を同期する。空集合の曖昧な場合は全体へ安全側 fallbackする |
| TeX 書戻し | S-22 `Runner::run_writeback` で実行時誤りまでの値も回収し、変更された register だけを TeX の `int_define` / `dimen_define` 経由で書き戻す |
| fmt | Vaak call の実体ではなく source を dump し、undump 時に再 intern する |

したがって、「毎回 `Host::run` で parse/check/compile している」「Runner をまだ再利用していない」
という評価は **Vaak の高水準 `Host::run` には当たるが、現行 PraTeX bridge には当たらない。**
phase API は、この既存の低水準利用を正式な prepared API に持ち上げる必要がある。

### 2.2 Vaak 側ですでに動くもの

現行 Vaak 0.1.0 には S-11 第一段が入っている。

- `HostItem::{Value, Fn}` と `HostSig` がある。
- `Host::expose_fn` と `HostFn` がある。
- checker と type checker は host function の引数数・型・返り値型を見る。
- compiler は host function 名を compile 時に登録順の `u16` index へ解決する。
- VM 命令は `HostCall(u16, u16, Span)` であり、実行時に名前検索しない。
- `Program2::host_fns` は index と署名の対応を保持する。
- 木を辿る参照実装と VM の両方に同期 host call がある。
- S-22で`Runner::{run_writeback, run_with_writeback}`、`Program2::{host_reads, host_writes}`、
  `HostBinding::{read_at, write_at, len, is_empty}`が追加された。定数添字だけのhost arrayは要素単位で
  読み書きでき、同値の高水準`HostBinding::write`は抑止される。

したがって、**host function の compile-time index 化は新規課題ではない。** phase API では、
この実装を壊さず、index を決めた `HostLayout` と実行時 dispatcher の一致を検証する層が必要である。
PraTeX の現行 `exposed()` が登録するのは `count` / `dimen` の `HostItem::Value` だけであり、
`tex_print`、NodeOps、phase registration function はまだ PraTeX へ接続されていない。

現行 compiler が host call として認める callee は裸の `Name` である。`node.kind(...)` のような
field-call は namespace ではなく method call の構文へ入る。したがって v1 の公開名は
`node_kind`、`patch_remove`、`tex_print` のような衝突を検査した裸名にする。qualified host
namespace は Vaak の構文・checker・type checker・参照 interpreter・VM を同時に変える将来の
Claude/Vaak 側課題であり、この設計が実装済みと仮定しない。

### 2.3 現行 API の不足と不整合

| 項目 | 現行 | 必要な契約 |
|---|---|---|
| prepared API | PraTeX が parser/checker/compiler を個別に呼ぶ | source、host layout、entry、ABI version を一体で検証した immutable prepared object |
| entry point | `Runner` は `Program2.top` だけを走らせる | prepare 時に選んだ named entry へ、型検査済みの明示引数を渡す |
| host layout | `Program2` と実行時 `HostFns` の対応は呼出し側の規律 | canonical layout ID の完全一致。並替えや署名変更を誤 dispatch しない |
| call class | 全 host function が同期 `HostFns::call` | `Leaf` と `MaySuspend` の静的な区別 |
| host result | `Option<Value>` | 正常値、意図した paradox、host runtime error、suspend を区別 |
| 再入 | PraTeX の `RefCell<Runner>` を借りたまま再入すると panic し得る | Leaf は構造的に再入不能、MaySuspend は借用を切ってから再入可能 |
| call allocation | VM は `HostCall` ごとに `Vec<Value>` を作る | scalar Leaf fast path は allocation 0。一般呼出しも Runner scratch を再利用 |
| runtime error | 互換用`run_with`はerror時のafterを落とすが、S-22の`run_with_writeback`なら回収できる | C-2の値とhost effect、failure、suspendを一つのtyped completionで返す |
| `HostBinding::write` | S-22で同値の全体writeを抑止済み | phase APIはdirty setと実行済みeffectを明示する |
| partial host access | S-15の定数添字`read_at` / `write_at` / `len`を実装済み。ただしread/writeの対を型で保証せず、`write_at == false`を呼出側が扱わない | partial capabilityを対で宣言・検査し、失敗を黙って成功にしない。node APIとは別課題 |
| opaque value | `ValueType` に host-owned opaque token がない | `NodeHandle` 等を算術可能な裸の整数にしない nominal host token |
| resource limits | 一般 VM fuel、guest memory、host-call budget がない | capability と結び付いた deterministic budget |
| phase registry | ない | explicit registration、順序、失効、failure policy |
| WASM bridge | ない | versioned bulk schema と 0/1-call gate |
| daemon generation | PraTeX の cache/runner/buffer/name table は thread-local process lifetime | engine/run/phase generation ごとの所有と失効 |

`Host::run` が毎回 compile すること自体を phase hot path の問題として直す必要はない。PraTeX は
すでに低水準 API を使っている。必要なのは、同じ最適化を private な呼び方に依存せず維持できる
Vaak の prepared/runner API である。なお現行`host_touched`は`Ref` / `Freeze` / `MutMethod`だけの
利用を`Some([])`にし得るため、PraTeXは空集合を「未使用」とせず全体同期へ倒す。Vaak側で
`None`（全部必要）へ分類できた後に、長さ参照だけの空集合fast pathを再開する。

## 3. 所有期間と用語

寿命を次の順に分ける。短い object が長い object へ紛れ込まないことを型と generation で守る。

```text
Process
  └─ EngineGeneration       PraTeX engine instance / fmt restore ごと
      └─ RunEpoch           一文書実行または daemon の一再実行ごと
          └─ PhaseEpoch     一段落、一pack、一page build 等の phase invocation ごと
              ├─ NodeHandle / BatchHandle / PatchBuilderHandle
              └─ Runner invocation / suspension
```

- `PreparedVaakProgram` は immutable であり、Process または EngineGeneration を越えて cache してよい。
  ただし compiler ABI と host layout が一致する場合に限る。
- `PhaseRegistration` は初版では RunEpoch-local とする。fmt、次の daemon rerun、別 engine へ渡さない。
- `Runner` の可変状態は一つの engine/runtime が所有する。prepared program に埋め込まない。
- node、batch、patch、capability の handle は PhaseEpoch-local とする。
- WASM module cache は長寿命でもよいが、instance、linear memory、fuel、結果 handle は RunEpoch-local
  または PhaseEpoch-local とする。

## 4. phase の有効化と状態遷移

### 4.1 状態遷移

```text
Absent
  │ explicit request
  ▼
Requested ── policy approval ─► Approved
  │                             │ exact approved descriptorでprepare
  │ denied                      ▼
  └──────────────────────────► Rejected
                                │
                                ▼
                             Prepared
                                │ consume ApprovedRegistration
                                ▼
                             Registered
  │ matching phase boundary
  ▼
Running ── Leaf ───────► Running
  │
  ├─ MaySuspend request（非live-node modeだけ）─► Suspended ── resume ─► Running
  │                                      │
  ├─ complete ───────────────────────────┘
  ▼
Registered
  │ revoke / end RunEpoch / fmt restore / engine reset
  ▼
Revoked
```

`Prepared` は実行可能な compile 結果にすぎず、engine の phase を変更しない。`Registered` への遷移は
次のいずれかから始まる明示 request だけが起こせる。

1. PraTeX の明示 API を利用者が呼ぶ。
2. 明示的に開始された Vaak 実行へ phase registration capability が与えられ、その実行が
   v1 の裸名 `phase_register` を呼ぶ。

source が capability を自己付与することはできない。PraTeX policy が要求と許可の積を取り、
host function 一覧そのものを許可済み capability から作る。

### 4.2 capability の種類、grant、承認を分ける

永続的に番号と意味を固定するのは `CapabilityKind` だけである。例えば `ReadNodes`、
`BuildPatch`、`InvokeWasm` は stable kind だが、それ自体は権限を表さない。

```rust
pub enum CapabilityKind { /* stable semantic IDs */ }

pub struct GrantLease { /* private fields; RunEpoch-local */ }
pub struct ApprovedRegistration { /* private fields; affine */ }
```

- `GrantLease` は policy が発行する RunEpoch-local な許可で、grant した kind、provider、phase、
  module identity、limit、失効条件へ結び付く。fmt や次 RunEpoch へ移せない。
- `ApprovedRegistration` は canonical registration request、source identity、PraTeX `PhaseAbi`、stable
  prepared descriptor、ordering、effect/failure policy、limits と `GrantLease` の対応を一つの承認
  digest へ束縛する。lease nonce と RunEpoch は承認側だけが持つ。
- 両型の field は利用者が組み立てられず、`ApprovedRegistration` は `Clone` を実装しない。
- `register` は `ApprovedRegistration` を値で consume し、その中に束縛済みの request だけを使う。
  別の `PhaseRegistrationRequest` を同時に渡さず、承認後に provider、phase、source、entry、effect、
  limits を差し替える TOCTOU を作らない。
- cache可能なprepare結果はsource、HostLayout、Vaak `EntryAbi`、compiler ABIから成るstable descriptor
  identityだけを記録し、`GrantLease`、RunEpoch、approval digestを保持しない。`register` はstable
  descriptorの一致とcurrent leaseの有効性を別々に検査する。

`CapabilitySet` のような bitset は stable kind の集合を表すだけで、`GrantLease` の代用ではない。
Vaak guest へ必要なら lease から別の opaque call-local token を発行するが、その token から grant kind や
期限を書き換えられない。

### 4.3 direct 実行と phase 登録を混ぜない

- `\directvaak` / `\vaakinput` は一度走って終了値を TeX input へ返す既存機能である。
- `\vaakdef` は同じ処理に名前を付ける既存機能である。
- これらの実行中に phase 登録 capability が**明示的に与えられていない限り**、phase registry へ
  触れる host function を公開しない。
- prepare cache hit や `\vaakdef` の intern は登録とみなさない。
- expandable な実行 mode では、TeX scanner/output routine へ再入する `MaySuspend` capability を
  原則公開しない。登録 primitive の TeX 表面を expandable にするかは未決であり、初版は
  non-expandable な明示 command または engine API を選ぶ方が安全である。

### 4.4 phase の候補

安定 ID を与える前の候補は次である。

```text
PreLinebreak
LinebreakPlan
PostLinebreak
HpackPlan
VpackPlan
PageBuildPlan
MathListPlan
ShipoutPrepare
```

一つの semantic unit につき host から Vaak へ入る回数は一回とする。例えば
`PreLinebreak` は一段落につき一回であり、一 node、一 glyph、一 script 境界ごとではない。

phase の正確な前後関係、TeX の output routine と group/save stack が観測できる範囲、各 phase で
許す mutation は、Phase ID を公開する前に固定する。特に `LinebreakPlan` は built-in line breaker の
前に計画を返す phase であり、標準 line breaking 自体を Vaak へ移さない。

### 4.5 複数 policy の順序

各登録は `provider_key`、`before[]`、`after[]`、`priority`、`signature` を持つ。登録時に依存 graph を
検査し、cycle と欠けた必須依存を error にする。同順位の最終 tie は RunEpoch 内の明示登録順とする。
順序を phase 実行中に文字列検索して決めない。

最終形では、解決した列を一つの `PreparedDispatcher` にまとめ、host→Vaak entry を phase あたり
一回にする。現行 Vaak には compiled program を link する API も named entry を直接走らせる API も
ない。初版は次のどちらかに限定する。

1. 一つの phase program 内に policy と dispatcher をあらかじめ定義する。
2. 一 phase 一 provider に制限し、複数 provider は Vaak 側の dispatcher/link API 完成後に開放する。

複数 `Runner::run` を closure の `Vec` で順に呼ぶ実装を、最終 API として固定しない。

## 5. Prepared program と Runner の分離

添付案の `PreparedVaakPhase { program, runner, ... }` は、immutable な compile 結果と invocation ごとの
可変 VM state を同居させる。fmt、daemon、再入、同じ program の並行利用を考えると分けるべきである。

概念上の型を次とする。これは Rust source の確定形ではなく、所有関係の公開契約である。

```rust
pub struct PreparedVaakProgram {
    program: PreparedProgram,
    entry: EntryPointId,
    host_layout: HostLayoutId,
    entry_abi: EntryAbiId,
    compiler_abi: VaakCompilerAbi,
    source_identity: SourceIdentity,
    descriptor: StablePreparedDescriptorId,
}

pub struct PhaseRunner {
    runner: vaak::vm::Runner,
    call_scratch: HostCallScratch,
    invocation: Option<InvocationState>,
}

pub struct VaakRuntime {
    engine_generation: EngineGeneration,
    prepared_cache: PreparedCache,
    runner_pool: RunnerPool,
    active: Option<ActivePhaseSet>,
}
```

不変条件は次である。

- `PreparedVaakProgram` は実行で変化しない。source、AST、bytecode、constant table を phase ごとに
  clone しない。
- `PhaseRunner` は実行終了時に stack/frame/arena/scratch の内容を clear し、capacity を再利用する。
- runtime error、cancel、WASM trap のすべてが同じ cleanup path を通る。
- `Runner` を `Rc<RefCell<_>>` で再入させない。再入は suspended runner を pool の外へ退避し、
  別 runner を明示的に借りる。
- process-global cache を置く場合は immutable prepared object だけにする。TeX register snapshot、
  name ID、runner、capability、diagnostic owner を global にしない。

cache key は少なくとも次の canonical tuple である。

```text
source bytes
Vaak language/compiler semantic version
prepared API major version
host layout canonical descriptor
entry pointとVaak EntryAbi
```

hash だけで同一性を決めず、衝突時は canonical descriptor と source bytes を比較する。runtime fuel 等、
compile 結果を変えない limit は cache key に入れず invocation 側に置く。

cache は entry 数と推定 retained byte 数の両方に上限を持つ。上限、eviction policy、cache hit/miss は
engine option と測定記録に含め、phase 実行中に eviction lock や destructor を走らせない。初版は
engine-local bounded cache とし、process-global sharing、LRU の厳密な順序、`Send + Sync` は必要性を
測ってから決める。失敗した parse/prepare の negative cache も無期限には保持しない。

phase API の source は **strict UTF-8** とする。不正 byte は lossy replacement せず、最初の不正 byte
offset を `PrepareError` に返す。source identity は元の byte 列を対象とし、identifier、provider key、
entry 名、host function 名は Unicode normalization や case folding を暗黙に行わず UTF-8 byte 列の
完全一致で扱う。現行 `src/vaak.rs` の `String::from_utf8_lossy` を使う direct bridge の互換挙動を、
この設計だけで黙って変更しない。phase API へ接続する時に別試験と移行方針を用意する。

## 6. HostLayout と compile-time index

### 6.1 canonical layout

```rust
pub struct HostLayout {
    abi: HostAbiVersion,
    values: Box<[HostValueSpec]>,
    functions: Box<[HostFnSpec]>,
    token_kinds: Box<[HostTokenKindSpec]>,
    entry_abi: EntryAbi,
    id: HostLayoutId,
}

pub struct HostFnSpec {
    name: Box<str>,
    signature: HostSig,
    class: HostCallClass,
    capability: HostCapabilityId,
    fuel_cost: FuelCost,
    effect: HostEffectId,
}

pub enum HostCallClass {
    Leaf,
    MaySuspend,
}
```

`HostLayoutId` は次の **exact order を含む canonical descriptor** の identity である。

- host value の登録順、名前、型、mutability、read/writeback 契約
- host function の登録順、裸名の UTF-8 bytes、全引数型、返値型
- call class、layout-local な `HostCapabilityId`、fuel cost、`HostEffectId`
- opaque token kind の登録順、型 ID、複製/等値/永続化規則
- Host ABI major/minor、Vaak `EntryAbi` identity、entry、invocation mode

同じ要素を別順で登録した layout は別 identity である。hash の一致だけを信用せず canonical descriptor
も比較する。compile 時の layout と runtime dispatcher の layout が完全一致しなければ program を
走らせない。fuel cost や effect kind を identity から外すと、cache 済み program が承認時と異なる
予算・副作用で走れるため、これらも含める。

現行の `Program2::host_fns` と `HostCall(u16, ...)` は compile-time index をすでに実現している。
追加すべきものは以下である。

- `u16` index を生成した layout の identity
- call class と必要 capability
- fuel cost、effect kind、token kind、EntryAbi
- runtime dispatcher の同一性検査
- 戻り値の実型が宣言と一致することの runtime 検査
- index 上限、重複名、同名 value/function の扱いの静的診断

hot loop では name、hash map、trait-object の列を検索しない。`u16` index から固定 table を一回引く。
将来 `HostCall1I64`、`HostCall1Token` 等へ特殊化しても、これは Vaak VM の内部最適化であり、
source-level API は固定 signature の host function のままにする。

### 6.2 Vaak EntryAbi と PraTeX PhaseAbi

Vaakが所有するgenericなentry契約と、PraTeX固有のengine phase契約を分ける。MITのVaak側へ
PraTeXの`PhaseId`やnode意味論を持ち込まない。

```rust
pub struct EntryAbi {
    entry: EntryPointName,
    parameters: Box<[ValueType]>,
    result: ValueType,
    allowed_host_effects: Box<[HostEffectId]>,
    suspension: SuspensionPolicy,
}

pub struct EntryArguments {
    values: Box<[Value]>,
}

pub struct PhaseAbi {
    phase: PhaseId,
    version: PhaseAbiVersion,
    entry_abi: EntryAbiId,
    arguments: PhaseArgumentSchema,
    effect: EffectMode,
    failure: FailurePolicy,
}

pub struct PhaseArguments {
    entry: EntryArguments,
    lifetime: PhaseEpoch,
}
```

Vaakのprepareはentryが実在し、引数個数・各型・返値型・host effect・suspension policyが
`EntryAbi`と完全一致することを検査する。PraTeXのapprove/registerは、選んだ`PhaseAbi`がその
`EntryAbiId`を許し、engine境界、effect、failure policy、argument schemaと整合することを検査する。

`HostCapabilityId` と `HostEffectId` は Vaak が意味を解釈しない layout-local な識別子である。
Vaak はそれらを canonical descriptor、型検査、実行時 dispatcher の一致検査に使うだけで、
`ReadNodes` や `EmitTokens` の意味、grant、lease、phase policy は知らない。PraTeX が自分の
`CapabilityKind` / engine effect と ID の全単射を layout 構築時に固定し、approve/register 時に
その対応と current lease を検査する。したがって PraTeX 固有の node/capability 意味論を MIT の
Vaak 側へ持ち込まない。

live-node phaseの最初のrootは、PraTeXがprewalk/reserve後に作るephemeral `NodeHandle`を
`PhaseArguments`のread-only引数として明示的に渡す。host writeback用`HostValues`や暗黙のglobal
`phase_root`へ混ぜない。`PhaseArguments`は`EntryAbi.parameters`と個数・型を開始前に照合する。
ephemeral token kindはentry result、persistent host value、terminal `HostAfter`へ現れてはならず、
prepare/type-checkとrun completionの両方で拒否する。

現行 Vaak の `Runner` は top-levelしか開始できず、named entryの直接実行と引数受渡しは未実装である。
したがってlive-node v1はこのAPIが入るまで公開しない。引数なしtop-level wrapperと暗黙root取得を
第二のv1経路として併設しない。named entry APIはClaude/Vaak側の将来作業であり、存在を先取りしない。

### 6.3 host function の粒度

次のように固定 signature の名前を分ける。

```text
node_kind(node)
node_next(node)
node_width(node)
node_penalty(node)
node_glyph_code(node)
node_set_width(node, width)
node_set_penalty(node, penalty)
patch_remove(builder, node)
patch_insert_before(builder, anchor, prototype)
```

`node_call(operation, node)` のような動的 operation ID に集約しない。型検査、capability、fuel cost、
Leaf/MaySuspend class、compile-time index を操作ごとに確定できなくなるためである。

qualified な `node.kind` 等は可読性の将来候補だが、現行 Vaak の bare-Name HostCall では動かない。
v1 で underscore 名と qualified 名を同時に別実装せず、名前解決の唯一の決定元を Vaak 側に保つ。

## 7. Leaf と MaySuspend

### 7.1 結果型

現行 `HostFns::call -> Option<Value>` は、少なくとも次を区別できない。

- 正常に値を返した。
- 正常に「値なし」を返し、Vaak の paradox にした。
- index が不正だった。
- stale handle や capability 違反だった。
- host 側で実行時 error が起きた。

概念上の結果を次に分ける。

```rust
pub enum HostAnswer {
    Value(Value),
    Paradox,
}

pub enum LeafCallResult {
    Ready(HostAnswer),
    Error(HostError),
}

pub struct RunTransition {
    outcome: RunOutcome,
    host_after: HostAfter,
    effects: EffectSummary,
}

pub enum RunOutcome {
    Complete(RunCompletion),
    Failed(RunFailure),
    Suspended {
        continuation: SuspendedRun,
        request: HostRequest,
    },
}

pub struct HostAfter {
    values: Box<[Value]>,
    dirty: Box<[HostValueIndex]>,
    state: HostAfterState, // SuspendedCheckpoint | Terminal
}

pub struct EffectSummary {
    direct_effects: EffectCounters,
    pending_plan: Option<PlanId>,
    consumed_fuel: u64,
}
```

`Paradox` は Vaak program が扱える値なしの外界面であり、`HostError` は別の diagnostic 経路である。
stale handle を `Paradox` に潰して「末尾が無かった」ように見せない。

実行開始後の通常失敗を外側の `Result::Err` にしない。`RunOutcome::Failed` も必ず `host_after` と
`effects` を伴う。`HostAfter` は Vaak が変更した host value の dirty set と最終値を保持し、C-2 に
従って failure 時にも PraTeX が正式な writeback 経路へ渡せるようにする。`EffectSummary` はすでに
実行された direct host effect、未 commit の plan、消費 fuel を区別する記録であり、暗黙 rollback
journal ではない。

suspend 時の `HostAfter` は continuation が所有する値の read-only checkpoint であり、terminal
writeback として二度適用してはならない。resume は `SuspendedRun` を値で consume し、新しい
`RunTransition` を返す。`SuspendedRun` は `Clone` を実装しない affine object とし、resume/cancel/drop
のいずれか一回だけで畳む。同じ continuation を二回 resume できない。

### 7.2 Leaf

Leaf は高頻度 NodeOps 用であり、次をすべて満たす。

- 同期で完了する。
- Vaak/PraTeX engine へ再入しない。
- filesystem、network、process、WASM、TeX scanner、output routine を呼ばない。
- argument slice を call 終了後に保持しない。
- scalar fast path では allocation しない。
- host-owned node locator table の bounds/revision を検査し、raw pointer を返さない。
- panic を契約上の error path にしない。

Leaf dispatcher へは engine 全体の `&mut Engine` を渡さない。`PhaseNodeAccess`、`PatchBuilderStore`、
`Budget` のように、その phase で許した互いに分離可能な欄だけを safe Rust の借用で渡す。
これにより「再入禁止」をコメントではなく API shape で守る。

runtime にも invocation guard を置き、同じ runner への入れ子開始は `ReentryForbidden` を返す。
`RefCell` panic、deadlock、暗黙の二重 borrow にしない。

### 7.3 MaySuspend と live-node phase の境界

MaySuspend は将来の低頻度 host request に使えるが、**v1 の live-node phase では全面禁止する。**
現行 PraTeX の node は nested `Vec<Node>` にあり、engine 再入で list が動けば locator と借用の両方が
無効になる。`tex_print`、TeX scanner、output routine、別 Vaak 実行を live-node phase から呼べない。

v1 の `PhaseAbi` は invocation mode ごとに次を固定する。

| mode | Leaf | MaySuspend | engine再入 |
|---|---|---|---|
| direct / live nodeなし | 許可 | capability次第 | request完了後だけ |
| live nodes | 許可 | **禁止** | **禁止** |
| detached owned batch | batch操作のみ | WASM 0/1回 | TeX engine再入は禁止 |

WASM を使う場合は、live-node Vaak entry が topology を凍結したまま必要な情報を bounded owned batch へ
集約して**完了する**。その後 PraTeX が live node borrow を閉じ、NodeHandle を含まない dense batch を
WASM へ一回渡す。v1 は live NodeHandle を含む VM continuation を suspend しないし、WASM から
`tex_print` へ回らない。

将来 live phase を detach/resume する必要が出た場合は、opaque token が continuation 内に一つも
残らないことを型/effect 検査し、resume 前に `TopologyRevision` と `ContentRevision` を再検査して
locator table を作り直す別 ABI が要る。これは v1 の約束に含めない。

live node を持たない MaySuspend では、VM が request 到達時に owned 引数、instruction pointer、frame、
stack、guest arena、残 fuel を affine `SuspendedRun` へ移し、全 mutable borrow を解放してから外側へ
返す。許可された再入でも総 fuel と depth を親子で共有する。

`tex_print` は live-node phase ではない non-expandable direct invocation の将来候補である。
expandable な `\directvaak` から TeX の非展開処理へ再入することも許さない。

## 8. v1 の NodeHandle と nested `Vec<Node>`

### 8.1 arena を前提にしない

現行 PraTeX の node list は arena の slot ではなく、nested `Vec<Node>` が所有している。したがって
v1 の公開契約へ `arena_id`、slot、slot generation を書くと、存在しない backend を API の前提に
してしまう。v1 は backend-neutral な `NodeLocator` を host table に置く。

```rust
pub struct NodeHandle(HostToken);

struct NodeLocator {
    phase_epoch: PhaseEpoch,
    root: PhaseRootId,
    list_path: ListPath,
    dense_ordinal: u32,
    expected_kind: NodeKindId,
    revision: ListRevision,
}

pub struct ListRevision {
    topology: TopologyRevision,
    content: ContentRevision,
}
```

`ListPath` は Rust の pointer や `Vec` の address ではなく、phase root から nested list へ降りるための
host-private path である。exact path encoding は current node model と測定から決め、Vaak/WASM ABI
へ公開しない。`dense_ordinal` は topology を凍結した prewalk の安定した順序番号である。

将来 node backend を arena に変えた場合、`NodeLocator` の内部実装が arena/slot/generation を使うことは
できる。しかしそれは将来の backend 案であり、v1 token の意味、Phase ABI、WASM wire schema には
入れない。

### 8.2 topology を phase 中は凍結する

v1 の live-node phase 開始時に対象 root の topology を凍結する。phase 中に許す即時変更は
width、penalty、attribute 等の scalar content だけである。

- insert、remove、relink、list owner の変更は即時 API にしない。
- topology 変更は Patch へ記録し、Vaak entry が terminal `Complete` になり live-node borrow を閉じた
  **後**に検証・commit する。
- `TopologyRevision` は topology commit でだけ進む。
- `ContentRevision` は scalar content の正式な変更で進む。
- v1 の一登録は `DirectScalar` と topology `ValidatedPatch` を同時に選ばない。後者の live phase は
  content read-only とし、Patch が捕えた二 revision を completion 後に再検査する。
- 外部要因でどちらかの revision が変われば plan を捨てる。推測で locator を再解決しない。

この凍結により、`node_next` は phase 中に同じ dense topology を辿り、走査中の remove/reuse 問題を
v1 から外せる。

### 8.3 preassign と reserve

phase 入口で bounded prewalk を行い、公開対象 node 数、nested path 深さ、必要 table/payload byte 数を
先に数える。上限内なら handle table、token table、Patch builder、必要な dense batch を
`try_reserve` してから NodeHandle を一括 preassign する。

- `node_next` 等の Leaf call ごとに handle entry や path を allocation しない。
- reserve に失敗した場合は Vaak を開始せず `MemoryLimitExceeded` にする。
- prewalk 中に node 数/path 深さの上限を越えた場合も partial table を公開しない。
- process 内で単調増加し再利用しない token issuer から、必要数の連続 ID range を phase 開始時に
  一度だけ予約する。同じ payload を別 PhaseEpoch、RunEpoch、EngineGeneration へ再発行しない。
- この issuer と非再利用規則は `NodeHandle` だけでなく、`PatchBuilderHandle`、`BatchHandle`、
  `NodePrototypeId` を含む全 host-owned opaque token kind に共通である。kind ごとに payload 空間を
  分ける場合も、kind identity と payload の組を process 内で再発行しない。
- issuer が ID 空間の末尾へ達したら wrapせず `TokenDomainExhausted` として以後の拡張phase開始を
  拒否する。engine resetでcounterを零へ戻さない。provider無効時はissuerへ触れない。
- phase table は予約した `range_base` と長さを持ち、`token - range_base` のchecked演算とbounds checkで
  locatorをO(1)に引く。rangeの予約はphase入口で一度だけ同期してよいが、Leaf callではatomic、
  lock、hash lookupを行わない。exact packingとprocess-wide issuerのsafe Rust実装は測定して決める。
- phase終了でtable全体を失効させる。ID range自体は失効後もretireし、ABAを防ぐため再利用しない。

### 8.4 Vaak から見える opaque token

Vaak には host-owned nominal token がまだない。`i64` へ落とすと、加算、順序比較、他種 handle との
混同を静的に防げない。production API では次の性質を持つ opaque host token が必要である。

- host が型 ID と scalar-sized opaque payload を作る。
- Vaak は複製、等値比較、配列/構造体への格納、引数/返却だけを行える。ただし ephemeral kind を
  invocation の外へ返せるという意味ではない。
- 算術、順序、byte 列化、fmt 永続化、別 token type への cast はできない。
- `Value` の hot representation を増やさず、payload を box 化して handle ごとの allocation を増やさない。
- token の内部 field と `NodeLocator` を Vaak source から観測できない。

opaque token の `ValueType`、`Value`、型検査、複製、等値、表示、拒否される演算の**唯一の実装元は
Vaak 側**とする。PraTeX は Vaak の公開 token kind API を使い、同じ意味の shadow enum/coercion を
別実装しない。参照 interpreter、VM、STEEL、portable のうち token を実装していない backend は
prepare 時に明示的に拒否し、裸整数へ暗黙変換しない。Vaak 側でこの型が入るまで `i64` handle は
private probe に限る。

ephemeral token の escape 判定は top-level の値型だけを見ない。配列、named/anonymous struct、
optional、将来の closure/capture を含む**型と実値の全 aggregate graph**へ推移的な
`contains_ephemeral_token` 検査を行う。unknown/dynamic 型で不在を証明できない値は live-node
`EntryAbi` の result、persistent `HostValues`、terminal `HostAfter` に許可しない。循環を持つ将来の
値表現では visited set と深さ・要素数上限を用い、検査不能を「token なし」と扱わない。

### 8.5 検証と診断

全 NodeOps は順に次を検査する。

1. token kind が `NodeHandle` である。
2. token が現在の PhaseEpoch 用 table range に入る。
3. table index、root、path、dense ordinal が範囲内である。
4. `TopologyRevision` が一致する。
5. node kind と要求操作が一致する。
6. scalar write なら content permission と現在の `ContentRevision` 契約が合う。
7. `GrantLease` が有効で、必要 `CapabilityKind` が grant されている。

「次の node がない」は正常な `Paradox` にできるが、wrong epoch/revision/kind は `HostError` にする。
NodeHandle は fmt、TeX register、WASM linear memory、別 RunEpoch へ永続化しない。

構造化診断へ live token 自体を保存しない。phase 中に必要な表示用情報だけを
`NodeDiagnosticRef { root_label, list_path_summary, dense_ordinal, node_kind }` へ bounded copy し、phase
終了後の diagnostic、log、LSP event はこの inert な値を使う。opaque payload、locator path の内部
index、PraTeX node への参照を diagnostic owner へ逃がさない。

## 9. node 変更: direct mutation と検証済み計画

### 9.1 direct mutation

direct mutation は、即時性が意味を持つ局所 scalar field に限定する。

```text
node_set_width
node_set_penalty
node_set_attribute
```

各 setter は PraTeX の正式な node API、range check、ownership 規則を通す。Vaak の C-2 に従い、
後続 host error、fuel 切れ、paradox が起きても、完了済み direct mutation は自動では戻さない。
変更ごとに `ContentRevision` を進め、同じ phase の locator は topology が不変な限り利用できる。

このため、`FallbackBuiltIn` を要求する phase で direct mutation を許すなら、PraTeX 側が明示的な
snapshot/journal を用意しなければならない。初版では組合せを禁止し、atomic fallback が必要な
処理を Patch/BreakPlan に限定する。

node の insert/remove/relink は direct mutation にしない。走査 cursor、複数 owner、cycle、
discretionary、box root 等を一操作ずつ壊し得るからである。

### 9.2 Patch

topology 変更と複数 node の一括変更は host-owned builder に記録する。

```rust
pub struct Patch {
    phase_epoch: PhaseEpoch,
    base_revision: ListRevision,
    operations: Box<[PatchOp]>,
}

pub enum PatchOp {
    SetField { target: NodeHandle, field: FieldId, value: ScalarValue },
    Remove { target: NodeHandle },
    InsertBefore { anchor: NodeHandle, prototype: NodePrototypeId },
    ReplaceRange { first: NodeHandle, last: NodeHandle, payload: NodeBatchId },
}
```

Vaak の配列へ巨大な Patch 全体を深く複製させず、`PatchBuilderHandle` を介して host-owned buffer へ
append する。builder も PhaseEpoch-local、件数制限付きで、phase 入口で capacity を reserve する。
Patch は `RunOutcome::Complete` の後にだけ取り出せる。`Failed` / `Suspended` から topology commit を
始めない。

commit 前に少なくとも次を検証する。

- epoch、base `TopologyRevision` / `ContentRevision`、locator
- node kind ごとの writable field
- next/prev、root、owner、list kind、direction の整合
- dangling edge、duplicate ownership、forbidden cycle
- 同じ target/field への競合と policy 順序
- operation 数、生成 node 数、payload byte 数
- dimension、glue、penalty、font、glyph、ratio の範囲
- discretionary、alignment、box boundary をまたぐ変更の可否

一件でも失敗すれば **一件も適用しない。** commit 直前にも二 revision を読み直す。commit 中に
失敗し得る allocation は検証前に上限を
確認して予約し、適用 phase は可能な限り infallible にする。

### 9.3 BreakPlan

line breaking の置換結果は topology Patch と分ける。

```rust
pub struct BreakPlan {
    phase_epoch: PhaseEpoch,
    base_revision: ListRevision,
    breakpoints: Box<[NodeHandle]>,
    ratios: Box<[FixedRatio]>,
    demerits: Box<[i64]>,
    discretionary_choices: Box<[DiscretionaryChoice]>,
}
```

floating point の platform 差を wire contract にしない。PraTeX が使う比率・寸法表現を固定幅で
定義し、行範囲、単調性、全 node の所属、最終行、discretionary choice を検証してから built-in の
正式な line materialization API へ渡す。

## 10. WASM bulk bridge

live-node v1 では Vaak continuation を suspend せず、Vaak entry が owned batch を完成してから外側へ
返す。PraTeX は live node borrow を閉じ、その owned batch だけを WASM runtime へ渡す。node access を
伴わない将来 API では `MaySuspend` request にできるが、どちらも PraTeX/Vaak の借用を保持したまま
WASM runtime を呼ばない。

```text
PraTeX phase context
  → Vaak が Leaf NodeOps または host bulk snapshot で入力を集約
  → Topology/Content revision付きdense indexのowned SoA/AoS bufferを一度構築
  → Vaak live-node entryを完了し、全NodeHandleを失効
  → WASM を phase あたり 0/1 回呼ぶ
  → dense index の BreakPlan/Patch proposal を受け取る
  → frozen snapshotのlocatorへ戻し、両revisionを再検査してPraTeXが全体検証
  → atomic commit または built-in fallback
```

### 10.1 call count

- 一 PhaseEpoch につき外部 WASM request は原則 1 回までである。
- 二回目は `WasmCallLimitExceeded` とする。複数 provider を許す将来版では、registration 時に
  一つの bulk module/dispatcher へ合成できた場合だけ開放する。
- node、glyph、境界ごとの WASM import は公開しない。
- WASM が PraTeX `NodeHandle` を受け取らない。batch 内の `u32` dense index だけを使う。

### 10.2 wire schema

初版は Rust enum、pointer、slice、allocator、`Rc` の layout を公開しない。固定幅 integer と
`(memory, offset, length)` で表す versioned schema にする。

```text
BatchHeaderV1 { abi_major, abi_minor, record_kind, record_count, flags }
NodeRecordV1  { dense_id, kind_id, width, penalty, font_id, glyph_id, flags }
PlanHeaderV1  { topology_revision, content_revision, op_count, payload_bytes, flags }
```

正確な field は各 phase 仕様で固定する。unknown flag、reserved nonzero、範囲外 offset/length、
overlap、整数 overflow、過大 allocation、重複 dense ID を拒否する。

v1 wire は little-endian とし、各 record の byte offset と総 byte size を表で固定する。Rust/C の native
struct layout、暗黙 padding、host endian、pointer width、unaligned typed reference に依存しない。
reserved byte は 0 を要求し、reader/writer は byte slice から checked conversion する。将来別 endian を
足す場合は flag で推測せず ABI major/minor の明示 contract にする。

WASM module は capability で事前登録された identity/hash のものだけを呼ぶ。engine 内実行の既定では
filesystem、network、process、clock、terminal、stdout import を与えない。

## 11. capability、fuel、memory、cancel

### 11.1 capability

`CapabilityKind` は PraTeX が所有する stable な権限の種類であり、source が宣言する希望や実際の
grant そのものではない。PraTeX policy が RunEpoch-local `GrantLease` を発行し、
`ApprovedRegistration` が使用可能な kind と詳細条件を固定する。Vaak の `HostLayout` へは、選ばれた
kind を layout-local `HostCapabilityId` へ写した結果だけを渡す。

```text
CapabilityKind::ReadNodes
CapabilityKind::WriteNodeMetrics
CapabilityKind::BuildPatch
CapabilityKind::ReplaceLinebreak
CapabilityKind::RegisterSpacingTable
CapabilityKind::RegisterCharacterTable
CapabilityKind::EmitTokens
CapabilityKind::InvokeWasm
CapabilityKind::ReadDiagnosticsContext
```

module identity、read/write field、対象 phase、回数等は stable kind の payload ではなく lease/approval の
条件として束縛する。host layout は grant 済み kind だけから組み立てる。未許可 function は実行時
error にするより、原則 compile 時に名前が存在しない状態にする。lease/token は TeX 数値、fmt、
WASM raw bytes へ変換できない。`EmitTokens` は live-node `PhaseAbi` では grant しない。

capability の grant/revoke と active phase set の交換は phase 境界で原子的に行う。node 走査中に
global registry lock を取らない。

### 11.2 budget

```rust
pub struct ExtensionLimits {
    vm_fuel: u64,
    host_call_fuel: u64,
    max_host_calls: u64,
    max_guest_bytes: usize,
    max_host_handle_count: u32,
    max_patch_ops: u32,
    max_patch_bytes: usize,
    max_wasm_calls: u8,
    max_wasm_memory: usize,
    max_wasm_output: usize,
    max_reentry_depth: u16,
    max_provider_state_bytes: usize,
}
```

- VM instruction、Leaf call、bulk snapshot、Patch append、WASM request に deterministic weight を付ける。
- wall-clock timeout は OS scheduling で非決定なので fuel の代わりにしない。cancel flag は一定間隔の
  safe point で別に確認する。
- Vaak runtime arena、host scratch、locator/token table、Patch/WASM buffer は割当前に上限を検査する。
- Rust allocator の process-wide OOM を回復可能と約束しない。上限外の allocation を hot path へ
  隠さず、管理対象 buffer は conservative accounting を行う。
- live-node phase では再入を許さない。その他の許可された親子再入は総 budget を共有し、再入で
  fuel/memory を初期値へ戻さない。

現行 Vaak には一般的な fuel/memory API がない。これは Claude/Vaak 側の実装が必要である。

## 12. error model と fallback

error は安定した code と構造化 context を持つ。

```rust
pub struct ExtensionError {
    code: ExtensionErrorCode,
    phase: PhaseId,
    provider: ProviderKey,
    run_epoch: RunEpoch,
    source_span: Option<VaakSpan>,
    node: Option<NodeDiagnosticRef>,
    message: String,
}
```

最低限の code 群は次である。

```text
StaticError
AbiMismatch
HostLayoutMismatch
CapabilityDenied
FuelExhausted
MemoryLimitExceeded
TokenDomainExhausted
Cancelled
ReentryForbidden
StaleHandle
WrongLocator
WrongNodeKind
RevisionConflict
LiveTokenAcrossSuspend
HostRuntimeError
InvalidPatch
InvalidBreakPlan
WasmTrap
WasmCallLimitExceeded
WasmInvalidResponse
```

human-readable message は互換 ABI にせず、code と field を安定面にする。Vaak の paradox と host error
を混ぜない。panic、`RefCell` borrow panic、index panic、WASM trap を TeX process の unwind にしない。

登録時に effect mode と failure policy の組合せを検証する。

| effect mode | 許す failure policy |
|---|---|
| `ReadOnly` | abort、disable-for-run、ignore-result、built-in fallback |
| `DirectScalar` | abort、disable-for-run。snapshot がない built-in fallback は不可 |
| `ValidatedPatch` | abort、disable-for-run、atomic built-in fallback |
| `BreakPlan` | abort、disable-for-run、atomic built-in fallback |

標準日本語 profile は provider failure へ依存しない。実験 provider の plan を捨て、検証済み built-in
規則へ戻せる phase だけ `FallbackBuiltIn` を選ぶ。

## 13. fmt と daemon generation

### 13.1 fmt

fmt へ保存してよいものは declarative な source/descriptor だけである。

```text
VaakSourceRecord {
  source bytes,
  source identity,
  required Vaak language version,
  requested capability names,
  phase/provider metadata
}
```

次は保存しない。

- `Program2` bytecode と compile-time host indices
- `Runner`、stack、arena、scratch
- active registration/lease
- capability、NodeHandle、PatchBuilderHandle、BatchHandle、suspension
- process-local intern ID、pointer、`Rc` identity

phase 用 `VaakSourceRecord` は fmt 内で **inert data** である。undump は record の長さ・schema・hash 等を
検証して保持できるが、Vaak code を parse/prepare/execute せず、capability policy を問い合わせず、
phase を登録しない。run 開始後の明示 command/API が改めて request を作り、policy が
`ApprovedRegistration` を発行した時だけ current Vaak compiler と HostLayout で prepare する。
major version または必要 capability が合わなければ activation を拒否して構造化診断を出す。

現行 `\vaakdef` が source を dump して undump 時に再 intern する方針は維持する。phase を fmt から
毎 run 有効化したい場合は、run 開始時に declarative descriptor を**明示的に再登録する仕組み**を
別に設計する。`\dump` 時のたまたま active な lease を永続化せず、fmt load 自体を activation event
にしない。

### 13.2 daemon

`VaakRuntime` は engine instance の欄に置き、現行 `thread_local!` の `CACHE`、`NAMED`、`HOST_BUF`、
`BEFORE`、`RUNNER` を長寿命 daemon の意味論にそのまま使わない。

- fmt restore、engine reset、別 project instance で `EngineGeneration` を更新する。
- 一再実行の VFS/resolver/format/provider capability を `RunEpoch` に固定する。
- active registry、host buffer、runner pool、diagnostic owner は engine/run ごとに分離する。
- immutable prepared cache だけは process-wide 共有を検討できる。key に compiler ABI と host layout を
  必ず含める。
- provider/capability の変更は次 RunEpoch で反映し、途中 phase の table と混ぜない。
- incremental checkpoint 初版では suspended Vaak/WASM を checkpoint しない。provider を無効にして
  full rerun するか、phase 完了後の安全点だけを checkpoint にする。

二つの engine を同じ OS thread で交互に走らせても、named ID、register snapshot、error source、
phase registration が混ざらないことを試験する。

### 13.3 provider state の寿命

`PreparedVaakProgram` は immutable、`PhaseRunner` は terminal transition 後に内容を clear する。したがって
v1 は Runner の guest local を暗黙の phase 間 state とみなさない。

phase を越える state が本当に必要なら、PraTeX が bounded な host-owned `ProviderState` を
`RegistrationId` と RunEpoch へ結び付け、専用 capability/host function で明示的に読み書きする。
state は provider 間で共有せず、revoke、RunEpoch 終了、engine reset で drop し、fmt や prepared cache
へ入れない。C-2 に従い完了済み state write は後続 Vaak failure で暗黙 rollback しないため、atomic
更新が必要な state は version 付き proposal/commit に分ける。初版で provider state を実装するか
自体は未決であり、必要性がない間は state API を公開しない。

### 13.4 統計

correctness test 用の call/allocation counter と production telemetry を混ぜない。

- test build は phase entry、Leaf、MaySuspend、WASM、reserve、Patch commit の回数を正確に照合する。
- production の統計は opt-in、RunEpoch-local、provider/phase ごとの bounded aggregate に限る。
- node ごとの label、source文字列、token、locator を蓄積しない。counter は saturating とする。
- wall time は診断と性能測定にだけ使い、fuel、順序、fallback 等の意味判断に使わない。
- provider 無効時は統計 object、clock query、atomic/lock、allocation を作らない。
- log/diagnostic へ出す時は phase 完了後に snapshot し、live token を含めない。

## 14. compatibility と versioning

互換面を混ぜない。

| 面 | version 方針 |
|---|---|
| Vaak source language | Vaak 自身の language/semantics version |
| PraTeX↔Vaak Rust embedding API | `HostAbiVersion { major, minor }`。in-process build-time contract |
| prepared bytecode | process 内 private。fmt/file へ serialize せず互換を約束しない |
| PraTeX phase/host function set | stable Phase ID、function name/signature/capability の versioned layout |
| external WASM | Rust API と独立した `WasmAbiVersion` と固定幅 wire schema |
| PraTeX fmt | source descriptor と要求 version だけを保存 |

major 不一致は拒否する。minor は field/function の additive 追加だけに使い、provider が要求する
minimum minor と feature bit を handshake する。unknown required feature を黙って無視しない。

Vaak crate は現在 0.1.0 であり、`Program2` の内部 field も public であるが、それを安定 serialization
format とみなさない。新 API は既存 `HostFns` / `Runner::run_with` を当面残し、次のような additive
module と adapter から始めるのがよい。

```text
vaak::embedding::PreparedProgram
vaak::embedding::HostLayout
vaak::embedding::EmbeddingRunner
vaak::embedding::RunTransition
```

旧 `HostFns::call -> Option<Value>` は Leaf-only compatibility adapter にできる。MaySuspend、typed host
error、opaque token を必要とする program は新 API を要求し、旧 adapter では prepare を拒否する。

## 15. 最小 API 案

以下は責任境界を review するための擬似 signature であり、そのまま実装 source へ転記することを
要求しない。

### 15.1 PraTeX 側

```rust
pub enum PhaseId { /* stable IDs */ }

pub struct PhaseRegistrationRequest {
    provider: ProviderKey,
    phase: PhaseId,
    source: SourceIdentity,
    entry: EntryPointName,
    requested_capabilities: CapabilitySet,
    ordering: PhaseOrdering,
    effect: EffectMode,
    failure: FailurePolicy,
    limits: ExtensionLimits,
    lifetime: RegistrationLifetime,
}

pub struct ApprovedRegistration { /* private, non-Clone, RunEpoch-local */ }

pub enum RegistrationLifetime {
    Run,
    // Group / document / daemon-persistent は初版に入れない。
}

pub enum PhaseInvocationTransition {
    Skipped,
    RejectedBeforeStart(ExtensionError),
    Started(RunTransition),
}

impl VaakRuntime {
    pub fn approve(
        &self,
        request: PhaseRegistrationRequest,
        policy: &CapabilityPolicy,
    ) -> Result<ApprovedRegistration, ExtensionError>;

    pub fn prepare(
        &mut self,
        approved: &ApprovedRegistration,
        source: &[u8],
    ) -> Result<PreparedVaakProgram, ExtensionError>;

    pub fn register(
        &mut self,
        approved: ApprovedRegistration,
        prepared: PreparedVaakProgram,
    ) -> Result<RegistrationId, ExtensionError>;

    pub fn revoke(&mut self, id: RegistrationId) -> Result<(), ExtensionError>;

    pub fn begin_phase(
        &mut self,
        phase: PhaseId,
        context: PhaseContext<'_>,
    ) -> PhaseInvocationTransition;
}
```

`register` に request 引数はない。consume した approval のstable prepared descriptorとprepared objectの
descriptorが一致し、かつapprovalのRunEpoch-local leaseが現在も有効な時だけ登録する。prepared objectへ
lease nonceやapproval digestを埋め込まない。`begin_phase` は active set が `None` なら `Skipped` を返し、
Vaak runtimeへ入らずbuilt-in callerへ戻る。prewalk/reserve等の開始前errorと、開始後の
`RunTransition`を`PhaseInvocationTransition`で区別し、NodeOps loopの中からregistryを引かない。

### 15.2 Vaak 側に必要な最小追加

```rust
pub fn prepare(
    source: &str,
    layout: &HostLayout,
    entry_abi: &EntryAbi,
) -> Result<PreparedProgram, PrepareError>;

impl EmbeddingRunner {
    pub fn start(
        &mut self,
        program: &PreparedProgram,
        arguments: EntryArguments,
        host_values: HostValues,
        leaf: &mut dyn LeafHost,
        budget: &mut Budget,
    ) -> RunTransition;

    pub fn resume(
        &mut self,
        suspended: SuspendedRun,
        response: HostResponse,
        leaf: &mut dyn LeafHost,
        budget: &mut Budget,
    ) -> RunTransition;
}
```

必要な意味は signature 名より次の不変条件である。

- prepare は parse/check/type-check/compile と host layout binding を一回で行う。
- prepared object は immutable で、entry と layout identity を持つ。
- prepare は Vaak `EntryAbi` のentry引数・返値・host effect・suspension policyを検査する。
- startはPraTeXが生成し、EntryAbiと照合済みのephemeral `EntryArguments`を明示的にconsumeする。
- ephemeral host tokenをentry result、persistent HostValues、terminal HostAfterへ逃がさない。判定は
  配列・構造体等のaggregateへ再帰し、unknown/dynamic値は不在を証明できない限り拒否する。
- host function name は prepare 時に index 化する。
- Leaf と MaySuspend は compile 結果に記録される。
- Leaf scalar call は argument `Vec` を都度確保しない。
- suspension は VM state を所有し、PraTeX の借用を保持せず、`Clone` できない。
- start/resume は failure を裸の `Err` にせず、`RunOutcome::Failed` と `host_after` / `effects` を返す。
- fuel、memory、cancel、return type を runtime が検査する。
- host-owned nominal token を値として運べる。
- 木を辿る参照実装と VM の差分試験を維持する。

### 15.3 Node access

```rust
pub trait PhaseNodeAccess {
    fn kind(&self, node: NodeHandle) -> Result<NodeKindId, HostError>;
    fn next(&self, node: NodeHandle) -> Result<Option<NodeHandle>, HostError>;
    fn width(&self, node: NodeHandle) -> Result<Scaled, HostError>;
    fn penalty(&self, node: NodeHandle) -> Result<i32, HostError>;
    fn set_width(&mut self, node: NodeHandle, value: Scaled) -> Result<(), HostError>;
    fn set_penalty(&mut self, node: NodeHandle, value: i32) -> Result<(), HostError>;
}
```

これは PraTeX crate 内の safe Rust trait であり、external Rust ABI として安定化しない。Vaak からは
各 method に対応する `node_kind` 等の固定 host function index だけが見える。v1 trait に
insert/remove/relink を置かず、topology は frozen locator と completion 後の Patch だけで変更する。

## 16. 実装分担

### 16.1 PraTeX / Codex 側

- Phase ID と正確な engine 境界
- PraTeX `PhaseAbi`、argument schema、EntryAbi identityとの対応
- explicit registry、ordering、lifetime、failure policy
- `VaakRuntime` の engine-owned state と generation
- capability policy と invocation mode
- nested `Vec<Node>` の prewalk、NodeLocator/token table、revision、node permission、PraTeX node API
- Patch/BreakPlan builder、検証、atomic commit
- built-in 日本語経路と provider 経路の分離
- WASM runtime adapter、wire schema、module policy
- fmt descriptor、daemon/incremental generation
- call counter、allocation counter、PraTeX/WSL e-upTeX を含む benchmark
- TRIP、DVI/PDF、pTeX/JLReq の意味回帰

### 16.2 Vaak / Claude 側

- prepared program を正式な公開 API にする
- named entry または compiled dispatcher の実行 API
- bare host name の prepared binding。qualified host namespace は別の将来提案
- HostLayout identity、generic `EntryAbi`、layout-local host capability/effect ID、call class/fuel/token metadata
- `Option<Value>` を置き換える typed host result/error
- Leaf call の allocation-free scalar fast pathまたは Runner scratch
- live-node では拒否される affine MaySuspend continuation と安全な start/resume
- host-owned nominal token の型・値・複製・等値意味の唯一の実装
- VM fuel、guest memory accounting、cancel safe point
- runtime error 時の writeback/effect を失わない `RunTransition`
- 参照 interpreter / VM の host-call 差分試験。STEEL/portable は対応または prepare 時明示拒否

WASM engine を Vaak の依存にする必要はない。Vaak は `HostRequest` を返し、PraTeX が外側で WASM を
実行すればよい。

### 16.3 権利境界

Vaak は MIT、PraTeX/rtex は GPL-3.0 であり、向きは一方通行である。

- Vaak の公開 API を PraTeX が利用するのはよい。
- PraTeX の source、test、本文を Vaak repository へ転記しない。
- Claude へ渡すのは要求、型の意味、測定値、失敗例、互換条件であり、GPL 側の実装行ではない。
- Vaak 側の実装は Vaak 自身の decisions と参照 interpreter を基に独立に行う。

### 16.4 非目標

添付案にある guest-owned first-class typed arena、再帰 AST、sum/match は Vaak 言語自身の独立した
研究課題である。PraTeX の host-owned node locator、Patch、WASM batch を成立させる前提ではなく、
E0--E10 の範囲に入れない。PraTeX node を Vaak guest arena へ複製する設計にも戻さない。

## 17. 性能 budget

### 17.1 参考測定の扱い

添付研究は、compile 済み `Program2` と再利用 Runner で概ね次を報告している。

```text
一引数 native HostCall: 約 75--90 ns/call
二引数 native HostCall: 約 105--130 ns/call
21 calls/node:           約 1.6--2.7 us/node
9,999 nodes / 210,524 NodeOps: 約 0.16--0.32 s
```

これは Vaak `origin/codex/embedding-probe` の `67489a8` にあるnative VM probeであり、負荷中の
絶対時間には振れがある。設計の動機には使えるが、PraTeX の production nested node list、現在の compiler revision、paired
fixture で再測定していないため合格値にはしない。特に現行 VM の `HostCall` は call ごとに
`Vec<Value>` を確保するので、allocation-free Leaf API の前後を同じ fixture で測る必要がある。

親タスクの直近測定では、同じWSLで走らせたPraTeXはe-upTeXの約 **1.73倍**、起動を概算で
控除したengine部分は約 **1.98倍**で、いずれも1.2倍未満の中間目標に達していない。fixture、
commit、測定条件は [性能測定](performance.md) に固定する。この設計から原因や改善率を推定せず、
embedding変更前後を同じ基線で比較する。少なくとも「すでに性能gateを通った」とは扱わない。

### 17.2 hard boundary

| 経路 | budget |
|---|---|
| provider 無効 | Vaak phase entry 0、HostCall 0、WASM call 0、追加 allocation/lock/string lookup 0 |
| 標準日本語 | Vaak/WASM call 0。JFM/spacing/禁則は engine core の直接表・固定 dispatch |
| prepare 済み phase | phase invocation 中の parse/check/type-check/compile 0 |
| host dispatch | 実行時 name lookup 0。compile 済み `u16` index を一回引く |
| scalar Leaf | call ごとの heap allocation 0、再入 0、WASM 0 |
| WASM | 一 PhaseEpoch 0/1 call、node 単位 import 0 |

provider 無効の engine corpus は変更前後 paired 測定で幾何平均 0.5% 超の退行を認めない。有意差を
分離できない短い fixture では反復数を増やし、stdout/log/DVI/PDF hash を固定する。

PraTeX 全体の production gate は、DVI modeの同一入力、同一TeX tree、同等DVIについて
end-to-end wall timeがupLaTeXの **1.2倍未満**であることとする。上記e-upTeX micro benchmarkは
hot pathの診断値であって合否標本ではない。拡張APIを無効にしただけでこのheadroomを消費しては
ならず、spacing/JFM、provider registry、resolver等の主要sliceごとに再測定する。

### 17.3 enabled path の段階 gate

1. API 導入時: empty phase entry、NodeOps 1/2 引数、10万 node、Patch build/commit を記録する。
2. Leaf fast path 完了時: scalar call allocation 0 を allocator counter で固定し、同じ処理を行う
   direct safe-Rust traversal との比を出す。
3. production 高頻度 policy: phase 全体が同じ algorithm の direct safe-Rust reference の 1.2 倍以内を
   目標にする。届かなければ experimental のままにし、typed opcode、scratch、policy fusion を先に行う。
4. WASM: cold instantiate、warm invoke、marshal、validate、commit を分離し、engine benchmark に
   module download/compile cache miss を混ぜない。

最適化のために PraTeX source へ `unsafe` を足さない。このembedding計画の性能改善はsafe Rustの
範囲だけで行う。

### 17.4 safe-Rust CI contract

実装を始める時は embedding module とその全 in-tree target を compiler の `unsafe_code` forbid で
buildする。文字列検索だけを安全性検査にしない。CI は少なくとも release の library、binary、test、
example を同じ forbid 条件で compile/testし、通常の全回帰と TRIP を通す。`unsafe` を含む変更は
CI を通らない契約にする。

この lint は依存 crate 内部の `unsafe` までは禁止しない。WASM runtime 等の新依存は lockfile、license、
公開 safe API、memory/fuel 境界を別に reviewし、「PraTeX source が safe Rust」であることと
「依存 graph 全体に unsafe がない」ことを混同しない。どの CI command/feature set を必須にするかは
実装 commit で repository の実際の target に合わせて固定する。

## 18. 回帰試験

### 18.1 activation と順序

- provider が無い全既存 test で phase/HostCall/WASM counter が 0。
- Vaak を prepare しただけ、`\directvaak` を走らせただけでは phase が有効にならない。
- stable `CapabilityKind` の宣言だけでは有効にならず、RunEpoch-local `GrantLease` に束縛された
  `ApprovedRegistration` を consume した登録だけが matching phase を有効にする。
- approval 後に provider/phase/source/entry/effect/limit を差し替えた request を拒否し、同じ approval を
  二度 register できない。
- phase entry が一段落/一 list/一 page build につきちょうど一回。
- `before/after/priority` が登録時に確定し、cycle と欠けた必須依存を拒否する。
- duplicate provider と二個目の WASM request を決めた error にする。

### 18.2 prepared/ABI/runner

- strict UTF-8 sourceだけをprepareし、不正byte offsetをlossy変換せず返す。
- source、layout、Vaak EntryAbi/entry、compiler ABIが同じ時だけprepared cache hitし、entry/byte上限と
  evictionを守る。別RunEpochのlease/approval digestをcache objectへ保持しない。
- host value/function/token の exact order、署名、call class、capability、fuel、effect の変更で layout
  mismatch になる。
- Vaak EntryAbiとentryの引数・返値・host effect・suspension policy不一致をprepare時に拒否する。
- PraTeX PhaseAbiとEntryAbi identity、engine effect/failure policy、argument schema不一致をapprove/register
  またはphase開始前に拒否する。
- live-node rootをephemeral EntryArgumentsとして一度だけ渡し、result/HostValues/terminal HostAfterへの
  token escapeを拒否する。tokenを配列、構造体、optional、captureへ入れた多段の入れ子も推移的に拒否し、
  unknown/dynamic型を抜け道にしない。
- compile 時 index と runtime dispatcher index が全 function で一致する。
- 同じ prepared program を繰り返し走らせても local value、stack、arena、error span が混ざらない。
- runtime error、cancel、suspend/resume の後も Runner capacity は再利用でき、内容は残らない。
- host が宣言と違う型を返した時に typed runtime error になる。
- `RunOutcome::Failed` でも `host_after` と `effects` が返り、C-2 の writeback を失わない。

### 18.3 Leaf/MaySuspend

- Leaf から PraTeX/Vaak へ再入する経路が API に無く、強制した probe は `ReentryForbidden`。
- Leaf scalar call の allocator count が 0。
- live-node PhaseAbi から `MaySuspend` / `tex_print` / engine再入を prepare または実行前に拒否する。
- WASMへ渡るのはrevision付きowned dense batchだけで、NodeHandleを含むcontinuationを作らない。
- live nodeなしのMaySuspend時にRunner/engineのmutable borrowが残らず、affine `SuspendedRun` を
  clone/二重resumeできない。
- 許可された非live再入のdepth、親子共有fuel、cancelを越えた時に全runnerを安全に畳む。
- expandable invocation から禁止 MaySuspend function を呼べない。

### 18.4 handle/locator/revision

- nested `Vec<Node>` をbounded prewalkし、全token/path tableをreserve後にpreassignする。Leaf call中の
  handle allocationは0。
- 全 opaque token kind で同じ kind/payload を process 内に再発行せず、phase 終了後に同じ node 配置を
  再び処理しても新 token になる。旧 token は新 phase の table を引けず stale として拒否する。
- wrong epoch/token kind、範囲外table/path/ordinal、wrong node kind、TopologyRevision不一致を個別に
  拒否する。
- phase中のtopologyは凍結され、scalar変更だけがContentRevisionを進める。
- 別 engine、fmt restore 後、次 daemon RunEpoch の token を拒否する。
- `next == none` は paradox、invalid locator/revision は host error になり混同しない。
- token を算術、cast、fmt dump、WASM raw handle、永続diagnosticとして使えない。
- opaque token未対応のSTEEL/portable backendはprepare時に拒否し、整数へfallbackしない。

### 18.5 mutation/plan

- direct scalar mutation は後続 runtime error 後も残る（C-2）。
- direct scalarはContentRevisionを進め、topology Patchはterminal Complete後だけcommit候補になる。
- direct mutation と atomic fallback の禁止組合せを登録時に拒否する。
- Patch の一操作が不正なら一件も commit しない。
- dangling edge、cycle、duplicate owner、wrong root、Topology/Content revision conflict、range overflow を拒否する。
- BreakPlan の非単調 breakpoint、別 list node、不正 discretionary、過大 ratio を拒否する。
- 複数 policy の同じ field/topology 競合が決めた順序または明示 error になる。

### 18.6 WASM/security/fmt/daemon

- WASM call は phase あたり 0/1、per-node import は 0。
- little-endianの固定offset/sizeを用い、padding/reserved nonzero、malformed offset/length、oversize、
  unknown ID/flag、trap、fuel/memory 切れで output 全体を捨てる。
- capability がない fs/network/process/clock/stdout import を作らない。
- fmt round-trip は inert source descriptor だけを保存し、load時にprepare/execute/registerせず、active
  registration/lease/token/bytecodeを復元しない。
- 二 engine、二 project、二 RunEpoch を交互に走らせて cache/runner/registration/diagnostic が混ざらない。
- provider stateはRunEpoch/revokeで失効し、統計はbounded aggregateでlive tokenを保持しない。
- provider failure の built-in fallback と provider 無効時の DVI/PDF/hash が該当仕様どおり一致する。

fuzz/property test は opaque token、Patch byte/operation sequence、WASM wire input、suspend/resume/cancel
sequence を対象にし、panic と部分 commit がないことを確かめる。

## 19. 実装順

| 段 | 内容 | 完了条件 |
|---|---|---|
| E0 | call counter と disabled-path benchmark | 標準/TRIP/日本語 core の call 0、退行 0.5%以内 |
| E1 | PraTeX の engine-owned `VaakRuntime` | thread-local mutable state の engine 間混入を除く。既存 direct bridge 不変 |
| E2 | Vaak の正式 prepared/layout API | compile 一回、layout mismatch、Runner 再利用、現行 test 全通過 |
| E3 | explicit one-provider phase registry | prepare だけでは無効、run-local失効、phase entry 一回 |
| E4 | typed host result と allocation-free Leaf | paradox/error 分離、scalar call allocation 0 |
| E5 | opaque token と read-only NodeOps | process 単調・全kind非再利用issuer、PhaseEpoch/locator/ListRevision、preassign、ABA/stale拒否、raw pointer 0。arena backend は v1 外 |
| E6 | direct scalar + Patch/BreakPlan | C-2 と atomic commit を別契約で試験 |
| E7 | 非live MaySuspend/start/resume | borrow 解放、affine continuation、fuel/cancel、live-node拒否。`tex_print` は非live将来modeだけ |
| E8 | one-shot WASM bulk bridge | 0/1 call、bounded schema、trap atomic fallback |
| E9 | multi-policy dispatcher | 登録時順序解決、host→Vaak entry 一回、runtime name lookup 0 |
| E10 | fmt/daemon/incremental 世代 | source-only fmt、engine/run/phase失効、project分離 |

E0--E6 は safe Rust だけで進められる。WASM runtime 選定時も PraTeX 側で raw pointer/memory view を
直接操作せず、長さ検査済みの safe adapter に閉じ込める。

## 20. 公開前に決める未決事項

1. 各 Phase ID の正確な engine 前後位置と、観測・変更可能 state。
2. TeX 表面の登録 command と、expandable/non-expandable の区別。
3. Run-local 以外の registration lifetime を本当に要するか。
4. 複数 compiled Vaak program の dispatcher/link 方法。
5. opaque host token の Vaak 型規則、値 size、等値比較、STEEL/portable での扱い。
6. NodeHandle token の物理 packing、token table、nested path、revision、phase あたり node 数の上限。
7. direct mutation を許す field と、fallback のための snapshot を導入するか。
8. Patch conflict の既定を last-wins、first-wins、error のどれにするか。
9. VM instruction/host operation の fuel weight を ABI minor で固定する範囲。
10. WASM runtime、wire record、module署名/hash policy、Component Model adapter の時期。
11. 非live・non-expandable の将来 `tex_print` が挿入するのは raw bytes、strict UTF-8文字列、token list
    のどれか。catcode/region/provenance をどの時点で与えるか。live-node phase では許可しない。
12. daemon で prepared cache を thread 間共有するため、Vaak prepared object に `Send + Sync` を
    要求するか。初版は engine-local にすれば要求不要である。

未決事項を決める前に Rust enum の discriminant、fmt number、WASM numeric ID を割り当てない。

## 21. 関連文書

- [現行 Vaak bridge の実装・測定](vaak-integration.md)
- [拡張可能な文字分類器](character-classifier-extension.md)
- [script 境界組版と CJKV region](extensible-layout-roadmap.md)
- [拡張可能な寸法単位](extensible-dimension-units-roadmap.md)
- [日本語組版 roadmap](japanese-typesetting-roadmap.md)
- [監視・incremental・LSP roadmap](incremental-tooling-roadmap.md)
- [性能監査](performance.md)

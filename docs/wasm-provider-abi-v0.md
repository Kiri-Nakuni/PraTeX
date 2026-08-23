# PraTeX WASM provider ABI 0.0

更新: 2026-08-23

状態: **実験設計。ABI 0.0、runtime・export・providerは未実装**

## 1. 結論

PraTeXの外向きWASM ABIは、最初から汎用的なnode callback ABIにはしない。ABI 0.0が扱う
operationは次の四つだけとする。

1. `SpacingTableUpload`: 文字・script class対の自動空白rule tableを一括提出する。
2. `SpacingBatch`: tableへ落とせない一つのlistの境界をbounded batchで一括処理する。
3. `UnitTableUpload`: 各国・各組版文化の文字サイズ単位tableを一括提出する。
4. `UnitContextBatch`: tableだけでは解けないcontext依存単位を、不変contextごとに一括解決する。

高頻度の規則はupload時にPraTeXが全体検証し、host-owned tableへcompileする。hot loopは
そのtableをsafe Rustで直接引き、WASMを呼ばない。複雑だが低頻度の規則だけをowned snapshotへ
まとめ、一 semantic unitにつき一度だけWASMへ渡す。

pTeX互換、JFM、標準JLReq、自動和文間隔、禁則、優先順位付き行長調整、組込み寸法単位は
PraTeX engine coreの一級機能である。標準日本語profileはVaak/WASMを一度も呼ばずに完結する。
WASM providerはdefault-offであり、利用者・出版社固有のprofileを明示的に許可したrunだけで
有効になる。

本書の `0.0` は実験ABIである。stable `1.0` ではない。ABI 0.0のmoduleやwire bytesをfmt、
文書交換形式、長期保存cacheの互換面にしてはならない。

## 2. 関連設計との境界

本書は次の既存設計を置き換えず、外向きWASM境界だけを具体化する。

- [Vaak埋め込み内部API設計](vaak-embedding-api-design.md)
- [文字・script境界spacing](extensible-layout-roadmap.md)
- [`\kanjiskip` / `\xkanjiskip` core設計](kanjiskip-core-design.md)
- [拡張可能な寸法単位](extensible-dimension-units-roadmap.md)
- [拡張可能な文字分類器](character-classifier-extension.md)
- [文字・異体字・造字の内部表現](glyph-identity-roadmap.md)
- [JLReq native roadmap](japanese-typesetting-roadmap.md)
- [WASM module import・名前空間仕様 0.1](wasm-module-import-v0.1.md)

PraTeXと内蔵VaakのRust API、Vaak source language、PraTeX phase ABI、外向きWASM ABIは別の
version domainである。

本書はprovider operationとwireを定める。TeX sourceからのmodule activation、source-order、
namespace alias、group/global、transaction、fmt拒否はmodule import仕様0.1が定める。
ABI 0.0は任意control sequence実行を持たず、その実行ABIは引き続き別仕様である。

| 境界 | version |
|---|---|
| Vaak source language | Vaakが所有するlanguage/semantics version |
| PraTeX--Vaak Rust embedding | build内の`HostAbiVersion` |
| PraTeX phase | `PhaseAbiVersion` |
| 外向きWASM | 本書の`WasmAbiVersion 0.0` |
| fmt | source/engine stateの独立schema |

Vaakの既存portable WASM入口は「WASM上でVaak sourceを実行する」別の外界面である。本書の
PraTeX provider ABIとして再利用せず、export名、memory所有、statusを混ぜない。将来Vaak codeが
provider登録を要求する場合も、VaakはPraTeXの同じhost-owned proposal/validatorを呼ぶだけとし、
WASM runtimeをVaak crateの依存にしない。

## 3. ABI 0.0の非目標

次は0.0に入れない。

- WASI
- filesystem、network、process、terminal、stdout/stderr、clock、乱数のimport
- 任意のhost function import
- module start function
- shared memory、thread、atomic instruction
- memory64、multi-memory
- SIMD、relaxed SIMD
- floating-pointを使うprovider profile
- Component Model、WITを安定面とすること
- Rustのenum discriminant、struct layout、pointer、slice、allocator、`Rc`、trait objectの公開
- PraTeX `NodeHandle`、raw node pointer、Eqtb、scanner、token listの公開
- node、glyph、文字、境界、寸法出現ごとのWASM呼出し
- live nodeを保持したsuspend/resume
- TeX scannerやoutput routineへの再入
- token emission、file取得、font program実行
- provider instanceまたはguest memoryのfmt永続化
- provider間の共有mutable state
- 一般的なPatch、BreakPlan、line breaker置換ABI
- IVS、外字、造字descriptorの一般wire schema

Component ModelやWITは、将来この固定幅core-WASM schemaの上へadapterとして載せることはできる。
その時も本書のwire意味をWIT実装のlayoutへ依存させない。

## 4. 所有期間

ABI objectの寿命は次の順で短くなる。

```text
Process
  EngineGeneration
    RunEpoch
      ProviderRegistration
        InvocationEpoch
          request/response mailbox
          batch-local dense ID
```

- module source bytesと検証済みcompiled moduleはimmutableである。
- ABI 0.0ではcompiled module cacheもEngineGeneration-localを既定とする。
- capability lease、provider registration、compiled spacing/unit table、unit context cacheは
  RunEpoch-localである。
- WASM instanceとlinear memoryはInvocationEpoch-localである。
- ABI 0.0は一 invocationごとにfresh instanceを作る。hidden guest stateを次の呼出しへ渡さない。
- provider-local IDは一 registration内だけで意味を持つ。
- batch-local dense ID、request ID、snapshot IDは一 invocation内だけで意味を持つ。
- いずれのIDもfmt、別RunEpoch、別engineへ持ち越さない。

同じcompiled moduleを再利用しても、instance、memory、mutable global、table、fuelを再利用しない。
warm instance poolやprovider stateはABI 0.0の性能契約に含めない。

## 5. module profileとexport

### 5.1 core WASM profile

moduleは[WebAssembly Core Specification](https://webassembly.github.io/spec/core/)に従うcore
WASM binaryであり、ABI 0.0では次を満たさなければならない。

- import sectionが空である。
- start sectionを持たない。
- 32-bit indexのnon-shared linear memoryをちょうど一つ持つ。
- memoryを`memory`という名前でexportする。
- memoryのinitial pagesとmaximum pagesが同じである。
- module bytes、function数、table、global、data segment、memory pagesがhost limit内である。
- shared memory、thread/atomic、memory64、multi-memory、SIMD、relaxed SIMDを使用しない。
- ABI 0.0の決定的profileでは`f32`/`f64` value typeとfloating-point instructionを使用しない。

initialとmaximumを同じにするため、`memory.grow`は常に決定的に失敗する。guest allocatorが必要なら
固定済みlinear memory内で完結させる。PraTeXはguest allocator ABIを公開せず、`alloc`/`free`を
呼ばない。

### 5.2 必須export

moduleは次をexact name、exact typeでexportする。

```text
memory                                  memory
pratex_wasm_abi_min                     immutable i32 global
pratex_wasm_abi_max                     immutable i32 global
pratex_wasm_required_features           immutable i64 global
pratex_wasm_optional_features           immutable i64 global
pratex_wasm_required_capabilities       immutable i64 global
pratex_wasm_optional_capabilities       immutable i64 global
pratex_wasm_request_base                immutable i32 global
pratex_wasm_request_capacity            immutable i32 global
pratex_wasm_response_base               immutable i32 global
pratex_wasm_response_capacity           immutable i32 global
pratex_wasm_invoke_v0                   (i32, i64, i32) -> i64 function
```

`pratex_wasm_invoke_v0`の引数は順に次である。

1. `(major << 16) | minor`でpackしたselected ABI version
2. granted capability bitset
3. request length

返値はbit patternを`u64`として扱い、上位32 bitをtransport status、下位32 bitをresponse lengthと
する。transport status 0だけが「response mailboxを読める」を意味する。0以外の値、未知status、
response capacityを越えるlengthはprovider failureであり、response bytesを一 byteも採用しない。

guestの意味上の成功・拒否・診断はtransport statusへ増殖させず、response envelope内の
`StatusV0`で返す。

### 5.3 versionのpack

ABI version globalは次のunsigned bit layoutを使う。

```text
bits 31..16: major (u16)
bits 15..0 : minor (u16)
```

ABI 0.0のhostは`min=max=0x0000_0000`だけを提供する。module側rangeとの交差が無ければ
provider invocation前の登録handshakeで拒否する。0.0ではminorの加算互換を推測しない。
将来0.xを追加する時は、
各minorの互換範囲とrequired featureを別途固定する。

stable 1.0は0.xの自動的な別名ではない。適合suiteと実装経験を経て、別の明示versionとして
定義する。

## 6. capabilityとcontent-hash lease

### 6.1 capability bit

ABI 0.0は下位4 bitだけを定義する。

| bit | capability | 許すeffect |
|---:|---|---|
| 0 | `RegisterSpacingTable` | spacing proposalを一括登録する |
| 1 | `ProposeSpacingBatch` | 一listのspacing actionを提案する |
| 2 | `RegisterUnitTable` | unit proposalを一括登録する |
| 3 | `ResolveUnitContextBatch` | contextごとのdynamic unit scaleを提案する |

bit 4..63は0.0ではunknownである。unknown required bitは登録拒否、unknown optional bitはgrantせず
maskする。moduleのrequired capabilityが一つでもpolicyからgrantされなければ登録しない。

wire capabilityとは別に、PraTeX policyは外側の`InvokeWasm`許可を必要とする。module自身、TeX
source、Vaak sourceがgrantを作ることはできない。

### 6.2 feature bit

ABI 0.0のfeature bitsetは0だけを受理する。unknown required featureは拒否する。optional featureは
hostとの積を取るが、0.0 hostではすべて0になる。capabilityをfeature bitへ混ぜない。

### 6.3 lease

policyが発行するRunEpoch-local leaseは少なくとも次へ束縛する。

```text
ProviderLeaseV0 {
  provider_key,
  module_sha256,
  exact_module_length,
  selected_abi_version,
  granted_capabilities,
  operation_set,
  limits,
  fuel_model,
  failure_policy,
  run_epoch,
}
```

module identityはSHA-256と元byte lengthで表す。compiled module cacheはdigestだけで同一性を決めず、
衝突時にbounded source byte比較を行う。署名、certificate chain、network取得は0.0外である。

承認後にmodule bytes、provider、operation、capability、limit、fuel model、fallbackを差し替えられない。
registerはleaseを値でconsumeし、同じleaseを二度使えない形にする。

登録handshakeは次の順で行う。

1. module byte数とSHA-256を確定し、core binaryと0.0 module profileを静的検証する。
2. import/startを持たない検査用instanceをhard limit内で作り、immutable export globalを読む。
3. host/moduleのversion range、required/optional feature、required/optional capabilityを照合する。
4. policyがmodule hash、選択version、grant、operation、limit、fallbackを承認する。
5. affine leaseを発行し、検査用instanceをdropする。
6. 実operationでは同じcompiled moduleからfresh instanceを作る。

start functionもimportも無いため、手順2でprovider algorithmは実行されない。export type/valueが不正な
moduleへpolicy leaseを発行しない。

## 7. fixed mailboxの所有権

request/response mailboxはguest linear memory内に置く。base/capacity globalはunsigned `u32`として
解釈する。

hostは呼出し前に次をすべて検査する。

1. base + capacityがchecked加算できる。
2. 両rangeが現在のmemory byte length内にある。
3. requestとresponse rangeが重ならない。
4. 各capacityが64 byte以上である。
5. 各capacityがleaseのmemory/input/output limit以下である。
6. memory initial=maximumで、承認時のpage数と一致する。

invocationは次の順で行う。

1. host側でrequest bytesを完成させ、全長を検証する。
2. fresh instanceを作り、request/response mailbox全体を0で埋める。
3. safe runtime APIでrequest bytesをrequest mailboxへcopyする。
4. fuelを設定して`pratex_wasm_invoke_v0`を一回だけ呼ぶ。
5. trap、fuel切れ、cancel、nonzero transport statusならresponseを読まずinstanceをdropする。
6. response lengthを検査し、そのlengthだけhost-owned bufferへcopyする。
7. instanceとmemoryへの全参照を閉じる。
8. host-owned bytesをdecodeし、全domain validationに成功した時だけpublish/commitする。

PraTeXのRust sliceをWASM call中に保持しない。runtimeのraw memory pointer、`data_ptr`、
`from_raw_parts`等へ依存せず、境界検査を行うsafe copy APIだけを使う。guest pointerやoffsetを
PraTeX node、fmt、diagnostic ownerへ保存しない。

## 8. common wire format

### 8.1 byte orderと整数

- 全multi-byte integerはlittle-endianである。
- signed integerはtwo's complementである。
- pointer width、host endian、native alignment、Rust paddingに依存しない。
- reader/writerはbyte sliceからfieldごとにchecked変換する。
- floating-point fieldは存在しない。
- offsetとlengthの演算は一度`u64`以上へ広げてから上限と`usize`変換を検査する。
- 本書の擬似recordは記載順にfieldを密に並べる。native paddingやalignmentを挿入しない。
- record内の`*_offset`は、別記がなければEnvelopeのpayload先頭からの相対byte offsetである。

### 8.2 EnvelopeV0

全request/responseは固定64 byteのheaderから始まる。

| offset | size | field | 0.0の条件 |
|---:|---:|---|---|
| 0 | 8 | `magic[8]` | ASCII `PRTXW0\0\0` |
| 8 | 2 | `abi_major` | 0 |
| 10 | 2 | `abi_minor` | 0 |
| 12 | 4 | `header_bytes` | 64 |
| 16 | 4 | `message_kind` | 下表の既知値 |
| 20 | 4 | `flags` | bit 0だけresponse、他は0 |
| 24 | 4 | `total_bytes` | buffer全長と一致 |
| 28 | 4 | `section_count` | operation limit内 |
| 32 | 4 | `section_dir_offset` | 64以上 |
| 36 | 4 | `section_dir_bytes` | `section_count * 16` |
| 40 | 4 | `payload_offset` | bounds内 |
| 44 | 4 | `payload_bytes` | bounds内 |
| 48 | 8 | `request_id` | responseがexact echo |
| 56 | 8 | `capabilities` | negotiated grantとexact一致 |

message kindを次のように固定する。

| value | kind |
|---:|---|
| 1 | `SpacingTableUploadRequest` |
| 2 | `SpacingTableUploadResponse` |
| 3 | `SpacingBatchRequest` |
| 4 | `SpacingBatchResponse` |
| 5 | `UnitTableUploadRequest` |
| 6 | `UnitTableUploadResponse` |
| 7 | `UnitContextBatchRequest` |
| 8 | `UnitContextBatchResponse` |

requestでは`flags=0`、responseでは`flags=1`を要求する。unknown flagは拒否する。requestとresponseの
kind、request ID、selected version、capabilityが対応しなければ全体を拒否する。

### 8.3 SectionV0

section directory entryは固定16 byteである。

| offset | size | field |
|---:|---:|---|
| 0 | 4 | `section_kind` |
| 4 | 4 | `record_bytes` |
| 8 | 4 | `record_count` |
| 12 | 4 | `offset` |

section entryは`section_kind`昇順で、kindは重複しない。`record_bytes * record_count`をchecked計算し、
section全体がbuffer内に収まることを検査する。header、directory、payload、各record sectionは互いに
重ならない。0件sectionもそのoperationで必須ならdirectoryに置く。

0.0はunknown sectionをrequired/optionalと推測せず拒否する。record末尾の追加も
`record_bytes`を見て推測せず、selected ABI versionが定めたexact sizeだけを受理する。

section kindを次のように固定する。

| value | section kind |
|---:|---|
| `0x0000_0001` | `StatusV0` |
| `0x0000_0002` | `InvocationLimitsV0` |
| `0x0000_1001` | `SpacingTableConfigV0` |
| `0x0000_1002` | `SpacingClassRangeV0` |
| `0x0000_1003` | `SpacingPairRuleV0` |
| `0x0000_1101` | `SpacingBatchContextV0` |
| `0x0000_1102` | `BoundaryAtomV0` |
| `0x0000_1103` | `BoundaryV0` |
| `0x0000_1104` | `BoundaryActionV0` |
| `0x0000_2001` | `UnitTableConfigV0` |
| `0x0000_2002` | `UnitDeclarationV0` |
| `0x0000_2101` | `UnitContextV0` |
| `0x0000_2102` | `UnitQueryV0` |
| `0x0000_2103` | `UnitScaleResultV0` |

成功messageが持つsection setもexactに固定する。

| message | section set |
|---|---|
| `SpacingTableUploadRequest` | `InvocationLimitsV0`、`SpacingTableConfigV0` |
| `SpacingTableUploadResponse` | `StatusV0`、`SpacingClassRangeV0`、`SpacingPairRuleV0` |
| `SpacingBatchRequest` | `InvocationLimitsV0`、`SpacingBatchContextV0`、`BoundaryAtomV0`、`BoundaryV0` |
| `SpacingBatchResponse` | `StatusV0`、echoした`SpacingBatchContextV0`、`BoundaryActionV0` |
| `UnitTableUploadRequest` | `InvocationLimitsV0`、`UnitTableConfigV0` |
| `UnitTableUploadResponse` | `StatusV0`、`UnitDeclarationV0` |
| `UnitContextBatchRequest` | `InvocationLimitsV0`、`UnitContextV0`、`UnitQueryV0` |
| `UnitContextBatchResponse` | `StatusV0`、echoした`UnitContextV0`、`UnitScaleResultV0` |

成功時は0件の可変sectionもdirectoryに置く。失敗responseは`StatusV0`だけを持ち、他sectionを
付けない。sectionの欠落・余分・重複を拒否する。

### 8.4 StatusV0

全responseは`StatusV0` sectionをちょうど一件持つ。recordは16 byteである。

```text
StatusV0 {
  code: u32,
  detail_offset: u32,
  detail_length: u32,
  reserved: u32,
}
```

`code=0`が成功である。成功時はdetail offset/lengthとも0である。失敗detailはpayload内のbounded
strict UTF-8で、利用者診断の補助にだけ使う。detail本文は互換面ではない。不正UTF-8、範囲外、
過大detail、reserved nonzeroはresponse全体を不正とする。

guestが返せるstatus codeは次だけである。

| value | status |
|---:|---|
| 0 | `Ok` |
| 1 | `UnsupportedOperation` |
| 2 | `InvalidRequest` |
| 3 | `ProviderFailure` |
| 4 | `LimitWouldBeExceeded` |
| 5 | `CannotResolve` |

unknown statusは`WasmInvalidResponse`である。nonzero statusはoperation全体の失敗であり、hostが
部分的なrecordを推測して採用しない。

### 8.5 InvocationLimitsV0

全requestはhostが作る`InvocationLimitsV0` sectionを一件持つ。recordは32 byteである。

```text
InvocationLimitsV0 {
  max_response_bytes: u32,
  max_records: u32,
  max_payload_bytes: u32,
  max_diagnostic_bytes: u32,
  fuel_model_id: u32,
  call_ordinal: u32,
  fuel_limit: u64,
}
```

guestがこれより小さい結果を返してもhost limitは緩まない。guestはlimit値をechoせず、hostが
runtime、decoder、domain validatorの各層で同じleaseから検査する。0は「無制限」ではなく0件・0 byteを
意味する。

## 9. ID domain

内部のcanonical `InputCategory::{CatCode, Wide, RawBytes}`にはABI番号を与えない。
ABI上で同じ`u32`を使う次の値は、それぞれ別domainとする。

- provider-local lexical proposal ID
- layout `ScriptClassId`
- `LanguageRegion`
- TeX language number
- JFM metric class
- text/glyph identity
- provider-local spacing class ID
- provider-local unit ID
- batch-local atom、boundary、font-context、query ID

ABI 0.0はPraTeX内部の`InputCategory`、`ScriptClassId`、`JfmClassId`、Rust enum discriminantを
serializeしない。provider-local IDはregistration内でだけ意味を持ち、host registryが検証済みの
canonical意味へ写す。`\catcode`と`\kcatcode`の公開番号もprovider class IDへ流用しない。
batch-local IDはrequest内で0から密に付け、responseがechoするだけである。

`LanguageRegion`は既存の公開code 0=`und`、1=`ja`、2=`zh-Hans`、3=`zh-Hant`、4=`ko`、5=`vi`を
`u32`へzero-extendする。TeX language numberの意味へ変換しない。

scriptをwireへ出す必要がある`SpacingBatch`では、PraTeX内部enum値ではなく4 byteのISO 15924相当
ASCII tagをrecordへ直接置く。例えば`Latn`、`Hani`、`Hira`、`Kana`、`Hang`、`Zyyy`、`Zinh`で
あり、byte順をintegerへpackしない。

font ID、metric class IDはbatch-local dense IDであり、fmt、別batch、別font generationへ
持ち越せない。OpenType GIDを文字identityとして渡さない。

## 10. operation 1: SpacingTableUpload

### 10.1 目的

利用者・出版社固有の高頻度spacing規則をactivation時に一度だけ提出し、PraTeXが検証済み
`CompiledTable`へ変換する。登録後のglyph/境界処理ではWASMを呼ばない。

標準`PtexCompat` / `Jlreq` profileはこのoperationを通らずnative tableを使う。provider tableは
明示的に選ばれたcustom profileにだけ結び付く。

### 10.2 request

requestは`InvocationLimitsV0`と`SpacingTableConfigV0`を一件ずつ持つ。

```text
SpacingTableConfigV0 {              // 32 bytes
  layout_schema: u32,               // 0
  max_classes: u32,
  max_ranges: u32,
  max_rules: u32,
  max_reason_ids: u32,
  allowed_region_mask: u32,
  allowed_writing_mode_mask: u32,
  reserved: u32,
}
```

region maskのbit nは`LanguageRegion` code nを表す。writing-mode maskはbit 0が横組、bit 1が縦組で、
他bitは0とする。0 maskは「全部」ではなく不正である。

### 10.3 response

成功responseは`StatusV0`に加え、`SpacingClassRangeV0`と`SpacingPairRuleV0` sectionを持つ。
provider-local class ID 0は予約し、実classは1以上とする。

```text
SpacingClassRangeV0 {                // 24 bytes
  first_scalar: u32,
  last_scalar_inclusive: u32,
  class_id: u32,
  region_mask: u32,
  writing_mode_mask: u32,
  reserved: u32,
}
```

Unicode scalar rangeはsurrogateを含めず、U+10FFFF以下とする。同じscalar rangeが重なる場合、regionと
writing-mode maskの積が空でなければ曖昧なので拒否する。登録順によるlast-winsは使わない。

spacing長は次の16 byte recordをinlineで使う。

```text
LengthV0 {
  numerator: i64,
  denominator: u32,
  basis: u16,
  flags: u16,
}
```

denominatorは正である。basisは次だけを認める。

| value | basis |
|---:|---|
| 0 | absolute scaled point |
| 1 | left `em` |
| 2 | right `em` |
| 3 | left `zw` |
| 4 | right `zw` |

flagsは0でなければならない。hostが有理数を既約化し、checked arithmeticでscaled pointへ変換する。
guestにTeXの丸めを実装させない。

```text
SpacingPairRuleV0 {                  // 88 bytes
  left_class_id: u32,
  right_class_id: u32,
  region_mask: u32,
  writing_mode_mask: u32,
  natural: LengthV0,
  shrink_limit: LengthV0,
  stretch_limit: LengthV0,
  shrink_tier: u16,
  stretch_tier: u16,
  break_rule: u16,
  line_edge_rule: u16,
  penalty: i32,
  reason_id: u32,
  flags: u32,
  reserved: u32,
}
```

`break_rule`は0=`UseBuiltIn`、1=`Allow`、2=`Forbid`、3=`Penalty`とする。3以外ではpenaltyを0と
する。`line_edge_rule`は0=`UseBuiltIn`、1=`Retain`、2=`DiscardAtStart`、
3=`DiscardAtEnd`、4=`DiscardAtBoth`とする。flags/reservedは0である。

class、range、pair key、mask、length、tier、penalty、reason ID、entry数、総byteを全部検証する。
一件でも不正なら一件も登録しない。成功時は一時領域で全tableをcompileし、phase/list境界で
active dispatcherを原子的に交換する。

## 11. operation 2: SpacingBatch

### 11.1 目的とcall数

class pair tableへ落とせない低頻度規則だけを扱う。一つのhorizontal listを閉じる時、必要な
`BoundaryAtom`と境界をhost-owned bufferへ一度集約し、そのlistにつき
`pratex_wasm_invoke_v0`を最大一回呼ぶ。glyph単位、boundary単位のcall/importは0である。

PraTeX node listのmutable borrow、NodeHandle、Rust pointerを保持したままWASMへ入らない。

### 11.2 request

```text
SpacingBatchContextV0 {              // 40 bytes
  snapshot_id: u64,
  topology_revision: u64,
  content_revision: u64,
  atom_count: u32,
  boundary_count: u32,
  writing_mode: u32,
  reserved: u32,
}
```

```text
BoundaryAtomV0 {                     // 36 bytes
  atom_id: u32,
  atom_kind: u16,
  flags: u16,
  code_point: u32,
  script_tag: [u8; 4],
  language_region: u32,
  tex_language: u32,
  font_context_id: u32,
  metric_class_id: u32,
  reserved: u32,
}
```

atom kindは1=`UnicodeScalar`、2=`Barrier`だけとする。UnicodeScalarではcode pointが有効な
Unicode scalarでなければならない。Barrierではcode point、script tag、font/metric classを0にする。
IVS、外部文字、GIDを0.0のscalar fieldへ偽装しない。未対応identityはBarrierまたはnative fallbackへ
送る。

```text
BoundaryV0 {                         // 16 bytes
  boundary_id: u32,
  left_atom_id: u32,
  right_atom_id: u32,
  flags: u32,
}
```

atom/boundary IDは0から隙間なく昇順とする。境界は同じrequestのatomだけを参照する。flagsは0である。

### 11.3 response

responseはcontextのsnapshot/revisionをexact echoし、各input boundaryへ一件のactionを
boundary ID昇順で返す。

```text
BoundaryActionV0 {                   // 80 bytes
  boundary_id: u32,
  action: u32,                       // 0 UseBuiltIn, 1 Replace
  natural: LengthV0,
  shrink_limit: LengthV0,
  stretch_limit: LengthV0,
  shrink_tier: u16,
  stretch_tier: u16,
  break_rule: u16,
  line_edge_rule: u16,
  penalty: i32,
  reason_id: u32,
  flags: u32,
  reserved: u32,
}
```

`UseBuiltIn`ではrule fieldをすべて0にする。`Replace`はSpacingTableUploadと同じ検証を受ける。
欠落、重複、余分なboundary IDを許さない。

PraTeXはresponse全体をdecode・検証した後で、topology/content revisionを再読する。一致した時だけ
全actionを一括適用する。一件でも不正、revision conflict、trap、fuel切れなら全actionを捨て、
registrationで定めたnative `PtexCompat`または`Jlreq` profileへ戻る。部分適用、前回batch結果、
暗黙last-winsを使わない。

## 12. operation 3: UnitTableUpload

### 12.1 目的

各国・各組版文化の文字サイズ単位をactivation時に一括登録する。寸法出現ごとにVaak/WASMを
呼ばず、`scan_dimen` / `scan_units`の中央経路だけが検証済みhost tableを引く。

ABI 0.0のunitは次の三種類である。

1. `PhysicalRational`: ptに対する厳密な正の有理比
2. `ContextLinear`: `em`、`ex`、`zw`、`zh`のいずれかに対する厳密な正の有理比
3. `DynamicContext`: table式だけで表せずUnitContextBatchで一括解決する値

通常の「数値×単位」でない非線形な号数・有限size ladderを`scan_units`へ押し込まない。それらは
将来の明示size-table APIで扱う。

### 12.2 request/response

requestは`InvocationLimitsV0`と次のconfigを持つ。

```text
UnitTableConfigV0 {                 // 24 bytes
  unit_schema: u32,                 // 0
  max_units: u32,
  max_name_bytes: u32,
  max_provenance_bytes: u32,
  allowed_region_mask: u32,
  reserved: u32,
}
```

成功responseは次のrecordとpayloadを持つ。

```text
UnitDeclarationV0 {                 // 64 bytes
  unit_id: u32,
  canonical_name_offset: u32,
  canonical_name_length: u16,
  kind: u16,
  region_mask: u32,
  basis: u16,
  flags: u16,
  numerator: i64,
  denominator: u64,
  provenance_offset: u32,
  provenance_length: u32,
  reserved0: u64,
  reserved1: u32,
  reserved2: u32,
  reserved3: u32,
}
```

kindは1=`PhysicalRational`、2=`ContextLinear`、3=`DynamicContext`である。basisは
PhysicalRationalでは0=`Pt`、ContextLinearでは1=`Em`、2=`Ex`、3=`Zw`、4=`Zh`、
DynamicContextでは0とする。

PhysicalRational/ContextLinearはnumerator > 0、denominator > 0を要求する。DynamicContextは
numerator/denominatorとも0とする。入力値の符号はhost scannerが処理する。flagsと全reserved fieldは
0でなければならない。

canonical nameはpayload内のASCII alphabetic 1--16 byteで、ASCII lowercaseへcanonicalizeした
最終形をguestが返す。localized表示名やprovenanceは別のbounded strict UTF-8 metadataにし、scanner
keywordへ暗黙利用しない。

次をtable全体で検証する。

- provider-local unit ID 0を予約し、他IDが一意である。
- canonical nameが一意である。
- 組込みunit、`true`、`sp`、`mu`、`fil`系を上書きしない。
- 組込み名とprovider名、provider名同士のprefix関係を拒否する。
- kind、basis、ratio、region mask、name/provenance lengthが有効である。
- 最大entry数、payload byte数、換算中間値がlimit内である。

一件でも不正ならregistryを変更しない。成功時だけhost-owned tableへcompileする。`true`、`\mag`、
小数、丸め、overflow、`MAX_DIMEN`、未知単位の回復はproviderでなくhostの中央scannerが決める。
`ContextLinear`にはphysical unit用の`true`/`\mag`補正を適用しない。

## 13. operation 4: UnitContextBatch

### 13.1 目的とcall数

DynamicContext unitを単位出現ごとに呼ばない。同じimmutable `UnitContextKey`で初めてdynamic unitが
必要になった時、そのproviderが登録した全dynamic unitを一つのbatchで解決し、結果tableを
RunEpoch-localにcacheする。

同じcontext generationではWASM callは最大一回である。font、JFM、region、writing mode、registry、
`\mag`のいずれかが変われば別contextとし、旧cacheを使わない。複数contextを事前にまとめられる
callerのため、wireは一request内に複数contextを許す。

### 13.2 request

```text
UnitContextV0 {                      // 80 bytes
  context_id: u64,
  registry_generation: u64,
  font_generation: u64,
  jfm_generation: u64,
  em_sp: i64,
  ex_sp: i64,
  zw_sp: i64,
  zh_sp: i64,
  language_region: u32,
  writing_mode: u32,
  mag: i32,
  reserved: u32,
}
```

```text
UnitQueryV0 {                        // 16 bytes
  query_id: u32,
  context_index: u32,
  unit_id: u32,
  reserved: u32,
}
```

context/query IDは0から密に昇順とする。queryは同じregistrationのDynamicContext unitだけを参照する。
同じcontextでは全dynamic unitをちょうど一回ずつ問い合わせる。font/JFMのpointer、名前、GID、
run外handleを渡さない。

### 13.3 response

responseはrequestの`UnitContextV0` sectionをbyte-for-byteでechoする。contextの欠落、並替え、変更は
response全体を不正とする。

```text
UnitScaleResultV0 {                  // 24 bytes
  query_id: u32,
  status: u32,                       // 0 Ok only in 0.0
  numerator_sp: i64,
  denominator: u64,
}
```

scaleは一unit当たりのscaled pointを正の厳密有理数で返す。numerator > 0、denominator > 0を要求する。
入力されたTeX数値との乗算、最終丸め、overflow回復はhostが行う。

全queryへ昇順で一件ずつ結果が必要である。未知、重複、欠落、nonzero status、分母0、範囲外、
過大中間値が一つでもあればbatch全体を捨てる。全件検証後だけcontext cacheへ一括publishする。
失敗時に前回contextの値、部分結果、0、近似floatへfallbackしない。provider診断を出した後、
PraTeXの既存unknown-unit回復へ進む。

## 14. limits、fuel、cancel

leaseは少なくとも次を持つ。

```text
ExtensionLimitsV0 {
  max_module_bytes,
  max_functions,
  max_globals,
  max_table_elements,
  max_data_segments,
  max_memory_pages,
  max_request_bytes,
  max_response_bytes,
  max_sections,
  max_records,
  max_payload_bytes,
  max_diagnostic_bytes,
  max_spacing_classes,
  max_spacing_ranges,
  max_spacing_rules,
  max_spacing_atoms,
  max_spacing_boundaries,
  max_units,
  max_unit_contexts,
  max_unit_queries,
  max_calls_per_spacing_list,
  max_calls_per_unit_context,
  fuel_model_id,
  fuel_per_invocation,
}
```

exact default値はruntime選定とpaired benchmarkで固定する。guestが要求した値をそのままgrantせず、
host policyのhard capとの小さい方をapprovalへ束縛する。0をunlimitedとして扱わない。

WASM instruction fuelは選んだruntimeのweightに依存するため、`fuel_model_id`をengine/runtime profileへ
固定し、module identity、selected ABI、limitとともに実行記録へ残す。fuel modelを変更すると同じ
moduleが成功から失敗へ変わり得るため、cache keyと再現条件にも含める。

wall-clock timeoutをfuelの代用にせず、provider failureからnative fallbackを選ぶ意味条件にも
使わない。OS schedulingで意味が変わるためである。

- `FuelExhausted`は決定的なprovider failureであり、定めたatomic fallbackを使える。
- 利用者、preview、daemonからの`Cancelled`はPraTeX run全体を取消す。providerだけを捨てて別出力を
  続けない。
- wall timeは性能測定、watchdog、利用者への進捗表示にだけ使う。

Rust allocatorのprocess-wide OOMを回復可能とは約束しない。host-owned bufferは件数を数え、
`try_reserve`とconservative byte accountingを行ってからinstanceを開始する。

## 15. 決定性

ABI 0.0の適合providerは、同じ次のtupleから同じresponse bytesを返さなければならない。

```text
module bytes
selected ABI version
granted capability
request bytes
fuel model ID
fuel limit
fixed memory pages
```

このため0.0は次を要求する。

- import、clock、random、filesystem、network、process、stdoutがない。
- fresh instanceを使い、前回memory/global stateへ依存しない。
- float、SIMD、thread/shared memoryを使わない。
- wire上の寸法、比率、scaleはintegerと正の有理数だけである。
- provider tableの曖昧なoverlapとduplicateを拒否し、登録順を意味にしない。
- batch IDとresultを昇順に固定し、last-winsを使わない。
- hostは同じexact arithmeticと丸め経路をすべてのconsumerから共有する。

response cacheを0.0の最初の実装へ入れる必要はない。追加する場合はhashだけで一致を決めず、上の
tuple、RunEpoch、context/revision、入力bytesを比較する。cacheの有無でresponse意味を変えない。

## 16. error modelとatomic fallback

PraTeX側の安定error codeは少なくとも次を区別する。

```text
AbiMismatch
UnknownRequiredFeature
CapabilityDenied
ModuleIdentityMismatch
InvalidModule
InvalidExport
InvalidMailbox
MemoryLimitExceeded
FuelExhausted
Cancelled
WasmTrap
WasmCallLimitExceeded
WasmInvalidEnvelope
WasmInvalidResponse
InvalidSpacingTable
InvalidSpacingAction
InvalidUnitTable
InvalidUnitScale
RevisionConflict
```

human-readable messageとguest detailは互換ABIにしない。panic、index panic、runtime trapをTeX processの
unwindにしない。

operationごとのfailureは次のように扱う。

| operation | failure時 |
|---|---|
| `SpacingTableUpload` | active tableを変更せず、provider registrationを拒否する |
| `SpacingBatch` | proposal全体を捨て、承認済みnative `PtexCompat` / `Jlreq`へ戻る |
| `UnitTableUpload` | unit registryを変更せず、provider registrationを拒否する |
| `UnitContextBatch` | 全結果を捨て、cacheへpublishせず、unknown-unit診断・回復へ進む |

responseのdecode、ID解決、range、dimension、topology/revision、table conflictを一時領域で全部検証し、
必要capacityをreserveしてからpublish/commitする。decoder、Vaak adapter、WASM adapter、consumerに
同じdomain validationを複製しない。

## 17. activation、default-off、fmt

WASM providerは二重にdefault-offとする。

1. WASM runtime依存をCargoのoptional featureに置き、default featureへ入れない。
2. feature付きbuildでも、RunEpoch-local leaseと明示registrationが無ければproviderを有効にしない。

初版のactivation authorityはCLI、orchestrator、またはembedding engine APIとする。TeX sourceが任意の
pathやURLからmoduleを読み、自己承認するsurfaceを作らない。moduleは実行開始前に取得済みで、
content hashがpolicyへ固定されていなければならない。engine実行中にnetworkから取得しない。

次はactivation eventではない。

- module fileが探索pathに存在する。
- moduleをcompile/cacheした。
- `\directvaak`、`\vaakdef`、`\vaakinput`を実行した。
- fmtをloadした。
- source descriptorをundumpした。

fmtへ保存しないもの:

- compiled WASM module
- instance、linear memory、mailbox offset
- active registrationとcapability lease
- provider-local class/unit ID
- batch/snapshot/request ID
- spacing/unit runtime tableとcache generation
- fuel/cancel途中状態

将来fmtへprovider declarationを保存する場合も、module hash、要求ABI/capability等のinert descriptorに
限る。undump時にcompile、instantiate、execute、registerせず、次RunEpochで改めてpolicy approvalを
要求する。

provider無効時はWASM call、Vaak phase call、module lookup、allocation、lock、hash、clock queryを0に
する。spacingではlist開始またはfinalizer入口、unitではregistry snapshot取得位置に一回の外側
`Option`判定だけを許す。文字・境界・単位出現ごとの分岐を増やさない。

## 18. safe Rustとruntime adapter

PraTeX側のABI moduleと全in-tree targetを`unsafe_code` forbidでbuildする。runtime dependency内部の
`unsafe`とPraTeX sourceのsafe Rustを混同しない。runtime採用前に少なくとも次を別途監査する。

- licenseと再配布条件
- lockfileとsupply-chain
- dependency内部のunsafe利用
- safe memory read/write API
- fuel、memory、module feature制限API
- trap/cancel時のcleanup
- binary sizeとcompile/cold instantiate/warm invoke費用
- default feature binaryへ混入しないこと

PraTeX adapterはruntime固有のvalueやerrorをABI公開型へ漏らさず、次の小さいsafe interfaceへ
閉じ込める。

```text
validate_and_compile(module_bytes, module_limits)
inspect_exports(compiled)
instantiate_fresh(compiled, memory_limit, fuel)
write_request(instance, checked_range, bytes)
invoke_once(instance, version, capabilities, length)
read_response(instance, checked_range, length)
```

runtimeを変更してもwire codec、domain proposal、spacing/unit validator、fallbackを変更しない。

## 19. 必須conformance test

試験名はrepository規約どおり日本語にする。最低限次を固定する。

### 19.1 disabled pathとactivation

- `wasm機能を無効にした既定buildはproviderを一度も呼ばない`
- `moduleを置いただけではproviderが有効にならない`
- `directvaakを実行しただけではwasm能力を得ない`
- `承認済みleaseだけが対応operationを一度登録できる`
- `承認後にmoduleとoperationとlimitを差し替えられない`
- `run終了とfmt復元でproviderとcacheが失効する`
- `二engineのproviderとdiagnosticが混ざらない`

### 19.2 version、capability、module

- `abi零点零以外は最初のhostで拒否する`
- `未知の必須featureとcapabilityを拒否する`
- `optional capabilityはgrantとの積だけを渡す`
- `responseのversionとcapabilityの差替えを拒否する`
- `content hashと元module byteが一致する時だけcacheを使う`
- `importとstartとshared memoryを持つmoduleを拒否する`
- `floatとsimdとmemory64を使うmoduleを拒否する`
- `memoryのinitialとmaximumが異なるmoduleを拒否する`

### 19.3 mailboxとwire

- `requestとresponseのmailbox重複を拒否する`
- `mailboxの加算overflowとmemory範囲外を拒否する`
- `response capacityを越える返値を拒否する`
- `magicとendianとheader長をgolden vectorで固定する`
- `sectionの重複と未整列順と未知kindを拒否する`
- `offset長さ積のoverflowと領域overlapを拒否する`
- `reserved非零と切れたrecordを拒否する`
- `不正utf8のdiagnosticを採用しない`
- `trapとfuel切れではresponse mailboxを読まない`

### 19.4 spacing

- `標準ptexとjlreqの段落はwasmを呼ばない`
- `spacing表を登録した後は境界ごとのwasm呼出しがない`
- `spacing表は全件成功または全件失敗になる`
- `unicode範囲の重なりと未知classを拒否する`
- `分母零と不正tierと不正break ruleを拒否する`
- `spacing batchは一listにつき一回だけ呼ばれる`
- `spacing batchはnode handleを含まない`
- `境界actionの欠落重複余分を拒否する`
- `一actionが不正なら一件も適用しない`
- `revision競合ではnative profileへ戻る`
- `provider失敗時のnode意味とdvi_pdf意味はnative fallbackと一致する`

### 19.5 unit

- `組込み単位とtrueをproviderが横取りできない`
- `単位名の大小重複とprefix衝突を拒否する`
- `物理単位を厳密有理数から同じscaled pointへ丸める`
- `context単位へphysicalなmag補正を適用しない`
- `unit tableは全件成功または全件失敗になる`
- `同じcontextでは全dynamic unitを一回で解決する`
- `単位出現ごとにはwasmを呼ばない`
- `fontとjfmとregionと方向の世代変更でcacheが失効する`
- `unit resultの欠落重複と分母零を拒否する`
- `一結果が不正なら一件もcacheへpublishしない`
- `dynamic unit失敗時に前回値や零を使わない`

### 19.6 determinism、cleanup、performance

- `同じmodule入力fuel profileはcold warmで同じresponse byteを返す`
- `fresh instanceは前回memoryとglobalを観測しない`
- `cancelはprovider fallbackでなくrun全体を終了する`
- `runtime error後も次のfresh invocationを実行できる`
- `provider無効時のascii corpusにallocationとlockを追加しない`
- `compiled tableのhot loopはtrait objectとwasm callを使わない`
- `cold compileとcold instantiateとwarm invokeとmarshal validateを分けて測る`

byte-level golden vectorはhost encoder、host decoder、独立した小さなtest guestの三者で照合する。
WASM runtime自身の文字列表現やdebug出力をgolden resultにしない。

## 20. fuzz/property test

少なくとも次へ任意byte・任意sequenceを与え、panic、unwind、部分publish、stale cache利用が無いことを
確かめる。

- `EnvelopeV0`
- section directoryとoffset/length/overlap
- `StatusV0`とUTF-8 detail
- spacing class/range/pair table
- spacing atom/boundary/action batch
- unit declaration/name/payload table
- unit context/query/result batch
- transport status、trap、fuel、cancelのsequence
- registration/revoke/RunEpoch終了のsequence

property testでは次を固定する。

- encode後にdecodeすると同じcanonical valueになる。
- canonical valueの再encodeは同じbyte列になる。
- table record順を変えても曖昧な意味を作らず、非canonical順を拒否する。
- validation failureの前後でactive registryとoutput hashが変わらない。
- checked arithmeticが成功した値だけTeXのdimension範囲へ入る。
- 同じcontext keyに異なるprovider generationを混ぜない。

fuzz corpusへpTeX、upTeX、LuaTeX等の上流source/testを移植しない。ABI自体はPraTeX固有仕様として
自作fixtureだけで検証する。

## 21. 実装順

| 段 | 内容 | 完了条件 |
|---|---|---|
| W0-A | host-owned proposal型とspacing/unit validator | Vaak/WASM/試験adapterが同じvalidatorを使い、部分登録0 |
| W0-B | 0.0 byte codecとgolden/property test | Rust layout非依存、malformed inputでpanic 0 |
| W0-C | optional runtime adapter、module profile、fixed mailbox | default build不変、import/start/memory/fuel境界を検査 |
| W0-D | `SpacingTableUpload` | explicit custom profileだけ有効、登録後WASM call 0 |
| W0-E | `UnitTableUpload` | 中央`scan_units`だけへ接続、組込み単位不変 |
| W0-F | `SpacingBatch` | owned batch一回、revision再検査、atomic native fallback |
| W0-G | `UnitContextBatch` | context一回、全dynamic unit一括、cache世代失効 |
| W0-H | conformance/fuzz/performance gate | disabled退行なし、決定性、safe-Rust CI |

W0-A/B/Cはnative spacing finalizerやVaak named-entry APIと並行できる。W0-D/Fのproduction接続は
host側の`BoundaryRule`、wide glyph、list finalizer、revisionを先に固定してから行う。W0-E/Gは
寸法unit registryと一つの中央conversion経路を先に固定してから行う。

Vaak table uploadもWASM decoderも、wireを読んだ後は同じ`SpacingProfileProposal` /
`UnitTableProposal`へ変換し、同じvalidatorを通す。判断を二箇所で実装しない。

## 22. stable 1.0へ進む条件

次を満たすまで`1.0`を名乗らない。

- 四operationの全field、ID、rounding、fallbackがproduction fixtureで使われた。
- byte-level conformance suiteを少なくともhostと独立guestで通した。
- malformed wire/module fuzzでpanic、partial commit、stale handleがない。
- provider無効時のTRIP、ASCII、pTeX/JLReq、DVI/PDF意味が不変である。
- spacing hot loopとunit scannerにper-item WASM/trait-object callがない。
- runtime、license、unsafe、memory/fuel、binary sizeの監査が完了した。
- fmt、daemon、二engine、二RunEpochの失効試験を通した。
- 実際のcustom spacing/unit providerを用いた経験から、0.0の不足を棚卸しした。
- version negotiationと0.xから1.0への移行・拒否診断を固定した。

1.0で0.0 moduleを暗黙に実行しない。明示adapterが無い限りABI mismatchとして拒否する。

## 23. clean-roomと一次資料

このABIはPraTeX固有の外界面である。pTeX、upTeX、e-upTeX、pdfTeX、XeTeX、LuaTeXの
実装sourceや上流testを移植・翻訳しない。native互換意味は公開manual・標準文書・自作入力による
black-box観測から固定し、WASM wire fixtureは独立に作る。

- [WebAssembly Core Specification](https://webassembly.github.io/spec/core/)
- [Vaak埋め込み内部API設計](vaak-embedding-api-design.md)
- [文字・script境界spacing](extensible-layout-roadmap.md)
- [拡張可能な寸法単位](extensible-dimension-units-roadmap.md)
- [JLReq native roadmap](japanese-typesetting-roadmap.md)

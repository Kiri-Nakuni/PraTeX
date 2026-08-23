# 拡張可能な寸法単位ロードマップ

## 1. 目的

PraTeX の寸法単位を、既存の TeX 互換経路を保ったまま追加可能にする。
単純で呼出し回数の多い規則は Vaak から明示的に登録する表へ落とし、
複雑だが低頻度の規則だけを、将来の versioned WASM ABI へ委ねる。

この文書は将来の実装順を定めるものであり、registry、Vaak の単位表登録、
WASM provider はまだ実装済みではない。

地域・組版文化ごとの実在単位、離散的な号数、font内部FUnitsをどのdomainへ置くかは
[各地域・組版文化の文字サイズ単位](international-typographic-units.md)に一次資料つきで分離した。

## 2. 現在の実装

通常寸法は `src/dimension.rs` の `scan_dimen` から `scan_units` へ入り、
次の判断を一箇所で行っている。

1. 内部寸法、および `em` `ex` `zw` `zh`
2. `true` と `\mag` の補正
3. `pt`、`in` `pc` `cm` `mm` `bp` `dd` `cc` `Q` `H`
4. `sp`
5. 未知単位の診断と `pt` を補う回復

`Q` と `H` は 0.25 mm ちょうどの有理換算である。`zw` と `zh` は、
JFM が未実装の現在は欧文フォントの `em` を暫定的に使っている。
通常 glue と `\dimexpr` も同じ寸法 scanner を通るので、呼出し側へ単位判断を
重複実装してはならない。

数式 glue の `mu` と、無限大次数の `fil` `fill` `filll` は別の走査経路である。
初版の拡張対象には含めない。

## 3. 変えてはならない境界

- registry の接続先は `scan_dimen` / `scan_units` の通常寸法経路だけとする。
- 代入、glue、式などの各 consumer は単位を独自に解釈しない。
- provider は token を読み直さず、scanner、`Eqtb`、save stack へ再入しない。
- 符号、小数、`true`、丸め、算術 overflow、`MAX_DIMEN`、回復診断は host が処理する。
- 拡張が無効な通常実行では provider 呼出しを 0 回にする。
- 組込み単位の認識順、結果、診断、空白消費を変えない。
- `mu` と `fil` 系は、別仕様を定めるまで registry に見せない。

## 4. 単位の二種類

登録単位は、少なくとも次の二種類を内部で区別する。

| 種類 | 意味 | 既存の例 | `true` / `\mag` |
|---|---|---|---|
| `ContextMetric` | 現在のフォント、JFM、組方向などから一単位の寸法を得る | `em` `ex` `zw` `zh` | 適用しない。`true` より前に解決する |
| `PhysicalRational` | pt に対する厳密な有理比で表せる物理単位 | `in` `cm` `mm` `Q` `H` | 既存の物理単位と同じ位置で適用する |

この区別を provider 側の慣習にしてはならない。registry entry 自身が種類を持ち、
`scan_units` が種類に応じた既存の計算経路へ接続する。

`ContextMetric` の結果は一単位の scaled point 値、または scaled point に対する
厳密な有理数とする。`PhysicalRational` は pt に対する符号付き分子と正の分母を持つ。
いずれも浮動小数点を公開値にしない。provider が返すのは尺度だけであり、入力された
整数部・小数部との乗算と最終丸めは host が行う。

## 5. `scan_units` への接続

registry lookup は中央の `scan_units` にだけ置く。内部では二つの型付き段階を持ってよいが、
consumer から見える拡張点を二箇所にしてはならない。

1. 組込みの内部寸法と `em` `ex` `zw` `zh` を従来どおり試す。
2. 登録済み `ContextMetric` を試す。
3. 従来どおり `true` と `\mag` を処理する。
4. `pt` と組込み `PhysicalRational` を従来どおり試す。
5. 登録済み `PhysicalRational` を試す。
6. 従来どおり `sp`、未知単位の回復へ進む。

組込みが成功した場合は registry を見ない。空の registry では、追加の token 化、
allocation、provider dispatch を行わない fast path を保つ。

## 6. 名前、優先順位、衝突

初版の登録名は、有界長の ASCII alphabetic name を大小同一視して扱う。
Unicode 名、記号を含む名、delimiter 付き構文は、字句規則を別に定めるまで受理しない。

- 組込み名とmodifier `true`は予約済みであり、上書きも削除もできない。
- 組込み単位は常に登録単位より先に認識する。
- 大小を正規化した同名登録は拒否する。暗黙の置換を行わない。
- 組込み名または`true`と登録名の一方が他方の prefix になる登録を拒否する。
- 登録名同士が prefix 関係になる登録も初版では拒否する。
- 同一 table 内の失敗を一部だけ採用せず、table 全体を原子的に検証してから公開する。

prefix を拒否するのは、既存 `scan_keyword` の前方一致と token の差戻しを保ち、
登録順によって入力の意味が変わるのを防ぐためである。将来 longest-match などを導入するなら、
別の公開 version として互換性を検討する。

## 7. 宣言的 registry

registry entry は、正規化済み名称、host が割り当てた `UnitId`、種類、尺度または
provider の局所 ID を持つ。crate 内部の Rust enum 値を公開 ID として再利用しない。

純データの `PhysicalRational` と、host が既知の metric を参照する `ContextMetric` は、
provider を呼ばずに解決できる。高頻度の単位は可能な限りこの形へ正規化する。

TeX primitive から登録を公開する段階では、局所代入、`\global`、`\globaldefs` を
既存の save stack に接続する。同じ判断を registry と primitive 側へ二重実装しない。
公開構文と照会 primitive は U2 で別途固定し、U0/U1 では先に内部境界を検証する。

## 8. Vaak からの明示的 table 登録

Vaak は、Vaak 実行が単位登録 capability を明示要求したときだけ table を host へ渡す。
全 Vaak 実行や全 TeX 実行へ常設 callback を登録しない。

1. host が run-local capability handle を Vaak 実行へ渡す。
2. Vaak は名称、種類、厳密な尺度からなる table 全体を一度に提出する。
3. host は名前衝突、範囲、分母、entry 数、総 byte 数を検証する。
4. 成功時だけ host-owned table として原子的に登録する。
5. capability scope 終了後は handle を失効させる。

Vaak の関数を寸法出現ごとに callback してはならない。頻出する単純規則は table へ確定し、
通常の registry lookup と host 算術だけで処理する。

## 9. 低頻度 WASM provider

WASM は、table へ落とせない複雑で低頻度の `ContextMetric` を対象とする。
単位名の認識と `UnitId` の割当は登録時に host が終えておき、使用時に文字列を渡さない。

### 9.1 ABI v1 で固定する値

- ABI version、capability bit、provider ID、unit ID は固定幅整数とする。
- context は host-owned opaque handle、または versioned な固定幅 record とする。
- byte buffer は `(handle, offset, length)` とし、長さと上限を必ず検証する。
- 尺度結果は status、scale kind、符号付き分子、正の分母で表す。
- Rust enum layout、Rust pointer、slice、allocator、`Rc`、trait object を公開しない。
- 最大 table 長、最大 batch 長、memory、fuel、time の上限を handshake する。

context の初版候補は、現在フォント ID と世代、`em`、`ex`、JFM metric ID と世代、
`zw`、`zh`、組方向、`\mag` である。必要な値だけを versioned snapshot にし、
`Eqtb` 全体を見せない。

WASM Component Model / WIT は将来 adapter として利用できるが、進行中の仕様へ
PraTeX ABI の安定性を直接結びつけない。最初に core WASM で表せる固定幅の v1 wire schema を
文書化し、その上へ WIT binding を載せる。

### 9.2 能力と失敗

provider には既定で filesystem、network、clock、process、乱数を import しない。
trap、fuel/time 超過、不正 status、分母 0、範囲外、context version 不一致は、
その lookup 全体の失敗とする。部分的な値や前回値へ黙って fallback せず、
host が一意の診断を出して従来の未知単位回復へ進む。

cache を導入する場合の key は、provider ID、unit ID、context ID、font/JFM/registry の
世代を含む。provider の状態変更と cache 世代更新を同じ操作にする。

## 10. run-local と fmt の境界

- 組込み単位は従来どおり常に存在する。
- provider instance、capability handle、WASM memory、cache、runtime generation は
  run-local であり、fmt に保存しない。
- fmt 読込み後の runtime provider registry は空から始める。
- 純データの宣言的単位を fmt に保存するのは、U2 で dump schema と互換性を固定してからとする。
- provider に結びつく entry を fmt へ保存しない。保存する場合も provider ID ではなく、
  次回実行で再登録を要求する unresolved declaration として別仕様にする。
- fmt の旧版は、新しい registry section が無くても従来単位だけで読めるか、
  明示的な format version 不一致として拒否する。推測して読まない。

## 11. 実装段階

| 段 | 実装 | 完了条件 |
|---|---|---|
| U0 | 現行出力と性能の基準を固定し、内部 `UnitId`、名称、二種類の尺度、空 registry を定義する | registry 無効時の結果と provider 呼出し回数が従来と一致する |
| U1 | 合成 registry を `scan_units` の二つの型付き位置へ接続する。公開 primitive はまだ作らない | fixed/context 合成単位が代入、glue、`\dimexpr` で同じ結果になる |
| U2 | 宣言的な登録・照会、group/global/globaldefs、純データの fmt schema を決める | scope 復元と fmt 往復、衝突診断が固定される |
| U3 | 明示 capability による Vaak table 登録を追加する | table 登録後に Vaak callback なしで処理し、scope 外で capability が失効する |
| U4 | versioned WASM ABI v1 と低頻度 provider を追加する | 制限、trap、不正応答、cache invalidation を含む試験が通る |
| U5 | batch/cache、性能調整、ABI 適合 suite、公開安定化 | 通常経路の回帰なし、provider 有効時も定めた budget 内に収まる |

各段を独立 branch として測定し、U4 の ABI を U1 の内部型へ直接依存させない。

## 12. 試験

### 12.1 互換性

- 組込み単位の整数、小数、負数、空白、大小文字
- `true` と `\mag`、特に `ContextMetric` には適用されないこと
- `Q` `H` の厳密換算と、`zw` `zh` の現在値および将来の JFM 差替え
- `pt` `sp`、`mu`、`fil` `fill` `filll` が registry に横取りされないこと
- 通常寸法代入、glue の width/stretch/shrink、`\dimexpr`
- 算術 overflow、`MAX_DIMEN`、未知単位の既存回復
- 組込み名、大小違い、prefix、登録名同士の衝突
- group、`\global`、`\globaldefs`、fmt 往復
- TRIP、既存 log、DVI/PDF の正規化比較

### 12.2 provider

- provider 無効時の呼出し回数が 0
- Vaak table が全件成功または全件失敗になること
- capability handle の scope 外利用を拒否すること
- WASM の trap、fuel/time 超過、分母 0、不正 kind、範囲外
- provider/context/metric 世代変更で cache が必ず失効すること
- 同一 format、入力、provider table、context で結果が決定的であること

### 12.3 性能

- registry 無効時の `scan_dimen` throughput と allocation 数
- 空 registry、少数 table、大 table の lookup cost
- `\dimexpr` と glue を大量に含む文書の CPU/wall time
- Vaak table 登録一回の費用と、その後の callback 回数
- WASM cold start、warm call、cache hit、budget 超過の費用

通常経路に退行があれば、provider の有無を一 token ごとに分岐する設計へは進まない。
既定は組込み view だけを使う。実行途中の明示登録が成功した時に、sessionの
dispatcherをhost-ownedの拡張 viewへ原子的に差し替える。一つの寸法走査は開始時にview snapshotを
一度だけ取り、その途中で切り替えない。登録やcapability処理を個々の単位名走査に組み込まない。

## 13. safe Rust と clean-room

PraTeX 側は safe Rust だけで実装し、WASM memory の境界も長さ検証済みの safe API に閉じ込める。
`unsafe` を必要とする案はこのロードマップの実装に含めない。

pTeX/upTeX/e-TeX の実装 source を移植・翻訳せず、公開 manual、独立した試験入力、
公式 binary の許可された black-box 観測から仕様を固定する。既存コードの節番号コメント
（`// See 453.` など）を保ち、新しい決定を consumer 側へ複製しない。

## 14. 一次資料

- [Knuth の TeX 配布物（CTAN）](https://ctan.org/tex-archive/systems/knuth/dist/tex)
- [pTeX manual（CTAN）](https://ctan.org/pkg/ptex-manual)
- [WebAssembly Core Specification](https://webassembly.github.io/spec/core/)
- [WebAssembly Component Model design and specification](https://github.com/WebAssembly/component-model)

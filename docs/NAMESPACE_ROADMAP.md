# 名前空間拡張 実装ロードマップ

対象: rtex（TeX82 の Rust 移植）
前提: 「TeX82 名前空間拡張 最小仕様」＋ その後の設計会話で確定した判断

---

## 0. 確定した設計判断

仕様書からの変更点を含む。実装はこちらに従う。

| 項目 | 決定 |
|---|---|
| catcode 値 | **16**（19 は pTeX 統合時に kcatcode を吸収する構想からの逆算だったが、順序が逆になったため撤回） |
| 解決タイミング | トークン生成時（`get_next` / `\csname`）。実行時解決なし |
| トークン表現 | `ControlSequence::Escaped(id)` を共有。種別（active か否か）は store 側に持つ |
| `\namespace` の収集 | `get_next` ベースの自前ループ。`ExpandableCommand::CsName` で停止し、`\csname` の登録処理に**介入する** |
| 空の名前 | エラーにせず global の `NullCs` へ**退化**。「空は global へ落ちる」を統一規則とする |
| 空の名前空間名 | global 領域そのもの（仕様通り） |
| escapechar 制約 | **設けない**。名前空間名の分離は単なるトリックであり、仕様が保証するものではない |
| 探索順 | global 優先のまま。フォーマットを名前空間で上書きする用途は非目標 |
| 受理判定 | catcode を 3 分類だけ見る: `(0,13)` = 終端 / `15` = invalid / それ以外 = 受理 |

---

## 1. フェーズ構成

各フェーズは単独でビルドが通る単位。依存関係:

```
Phase 0 ──> Phase 1 ──┬──> Phase 2 ──┐
                      │              ├──> Phase 4 ──> Phase 5
                      ├──> Phase 3 ──┘
                      └──> Phase 6
                                       Phase 7 ──> Phase 8
```

---

### Phase 0 — 現在の未コミット差分の整理（ビルド復旧）

現状 `cargo check` が通らない。まずここを閉じる。

| ファイル | 作業 |
|---|---|
| `src/eqtb/catcodes.rs` | `Namespace = 16` を確定（末尾カンマ）。`undump` に `"Namespace"` を追加 — 現状 `dump` 側だけで**非対称**になっている |
| `src/command/prefixable.rs:45` | `MAX_CAT_CODE: i32 = 15` → `16` |
| `src/input/line_lexer.rs:120` | `scan_namespace_prefix` 未実装でビルド不通。Phase 2 まで一旦削除するか `todo!()` stub に |
| `src/eqtb/catcodes.rs:3` | `use std::{ io::Write};` を元に戻す |
| 全体 | `cargo fmt` |

**検証**: `cargo check` と `cargo test` が通る。catcode 16 を持つ文字がない状態で既存挙動が不変。

---

### Phase 1 — eqtb: 名前空間付きエントリの格納

中心は `src/eqtb/control_sequences.rs`。**ここが土台**で、他のフェーズはすべてこれに乗る。

**追加するもの**

- `NamespaceId(u16)` と intern テーブル
  - `namespaces: Vec<Vec<u8>>` + `HashMap<Vec<u8>, NamespaceId>`
  - 名前空間名を id 化しておくと、以降 `Vec<u8>` を持ち回らずに済む
- ハッシュキーの拡張
  - `hash: HashMap<Vec<u8>, Id>` → `HashMap<(Option<NamespaceId>, Vec<u8>), Id>`
  - `None` = global。既存挙動は `None` キーで完全に再現される
- エントリに出自を持たせる
  - `escaped: Vec<(Command, Vec<u8>)>` → `Vec<(Command, EntryName)>`
  - `EntryName { ns: Option<NamespaceId>, kind: Name(Vec<u8>) | Active(u8) }`
  - 名前空間付き active char をこの id 空間に載せるために必要
- 新 API
  - `id_lookup_ns(ns, key)` / `add_command_ns(ns, key, levels)`
  - `active_char(cs) -> Option<u8>` ← Phase 5 で使う種別問い合わせ
  - `namespace_of(cs) -> Option<NamespaceId>` ← Phase 4 の印字で使う

**`src/eqtb.rs`**

- `lookup_ns` / `lookup_or_create_ns` を追加
- **1 文字短絡（`eqtb.rs:938` の `Single(name[0])`）と 0 文字短絡（`eqtb.rs:937` の `NullCs`）は名前空間版では行わない**
  - 1 文字: ペアキーのハッシュに入れる必要がある
  - 0 文字: `NullCs` を返す = 退化（決定事項）

**`src/eqtb/levels.rs` — 変更不要**

`escaped: Vec<usize>` が `add_new_escaped_command`（`levels.rs:332`）で伸びる構造なので、名前空間付きエントリを同じ id 空間に載せるだけで **save stack / グループスコープ / `\global` / `\afterassignment` がすべてタダで付いてくる**。これがこの表現を選ぶ最大の理由。

**検証**: `(ns, name)` の定義、グループ離脱での復帰、`\global` 代入を単体テストで。

---

### Phase 2 — 字句層

`src/input/line_lexer.rs` / `src/input/input_stack.rs` / `src/input.rs`

**`LexerToken` に variant 追加**

```
NamespacedWord(&'a [u8], &'a [u8])    // (ns名, CS名)
NamespacedSymbol(&'a [u8], u8)        // (ns名, 1文字CS)
NamespacedActive(&'a [u8], u8)        // (ns名, active char)
```

3 つに分けるのは `to_token` 側で `Escaped` / `Single` 相当 / `Active` 相当の分岐に対応させるため。いずれも `self.line` からの借用なので、両スライスは走査完了後にインデックスから同時に取る。

**`scan_namespace_prefix`**

- `next_unexpanded_character_with_replacement`（`line_lexer.rs:230`）を使う
  → `^^` 置換が CS 名スキャンと同一経路になるので、§7 のチェック項目が**構造的に**満たされる
- catcode は 3 分類のみ: `(0,13)` 終端 / `15` invalid / それ以外 受理
- 空白・タブ・制御文字・行末・ファイル終端 → runaway
- 終端が escape → `scan_control_sequence`（`line_lexer.rs:273`）をそのまま流用
- 終端が active → その文字自体が対象
- **注意**: 名前空間文字自体は「それ以外」に入るので受理される。`*a*b\hoge` は `a*b` の `hoge`。階層ではないので仕様と矛盾しない

**エラー通路の新設**

現状、字句層からのエラー通路は `Result<Option<LexerToken>, ()>` の 1 本だけで、`NextResult::InvalidChar`（`input_stack.rs:840`）に合流し `input.rs:196` で報告される。runaway を運べない。

- `Result<Option<LexerToken>, LexError>` に変更
- `LexError::InvalidChar` / `LexError::RunawayNamespace`
- `NextResult::InvalidChar` → `NextResult::LexError(LexError)`
- `input.rs` の `decry_invalid_character` の隣に runaway 報告を追加

> 空名を退化させる決定により、この通路が運ぶのは **runaway のみ**。

**`to_token`（`line_lexer.rs:371`）**

- 名前空間 variant を `lookup_or_create_ns` へ振る
- `allow_new_cs == false` 経路も同様に処理

**検証**: catcode 16 を持つ文字がない状態でトークン化が不変であること（`\catcode` を設定しない限り `match` に分岐が増えるだけなので構造的に保証される）。

---

### Phase 3 — `\namespace` と `\csname` への介入

`src/input/expansion.rs` / `src/command.rs` / `src/eqtb/primitives.rs` / `src/format/dump_command.rs`

**`ExpandableCommand::Namespace` の追加**

| 箇所 | 作業 |
|---|---|
| `src/command.rs` | enum に variant、`display`（`command.rs:323` 付近の書式に倣う） |
| `src/format/dump_command.rs:173, 324` | dump / undump |
| `src/eqtb/primitives.rs` | `\namespace` を `primitive_expandable` で登録 |
| `src/input/expansion.rs:30` | `expand` の match に arm |

**`manufacture_control_sequence_name` に名前空間を渡す**

`expansion.rs:91`。登録も `\relax` 化も `\endcsname` 到達後の一箇所に集中している:

- `expansion.rs:123` `lookup_or_create` → `lookup_or_create_ns(ns, ...)`
- `expansion.rs:134` の `\relax` 化はそのまま（`cs_define(..., false)` = グループスコープ維持）
- 呼び出し元は `expansion.rs:41` の**1 箇所のみ**なので引数追加は自明

**`\namespace` 本体**

`get_x_token` は使えない。名前空間名の完成を知らせるトークンが `\csname` 自身であり、`get_x_token` は制御が戻る前にそれを展開してしまうため。

```
loop {
    get_next
    ├ 文字トークン                     → 蓄積
    ├ ExpandableCommand::CsName        → 停止。manufacture_control_sequence_name(Some(ns), ..) を直接呼ぶ
    ├ ExpandableCommand::Namespace     → エラー（入れ子禁止）
    ├ その他の Expandable              → expand して継続
    └ その他の Unexpandable            → エラー
}
```

空の名前空間名は許す（`\namespace \csname bar\endcsname` = global への明示構成）。

**検証**: `\namespace foo\csname bar\endcsname` の実行後、global に `bar` が作られていないことを `\ifx` で確認。ここが `\endcsname` 終端案を退けた理由そのもの。

---

### Phase 4 — 印字

**`\namespacechar` の追加** — `\escapechar` と完全に同じ経路。触る箇所は 8 つ、すべて機械的:

| ファイル:行 | 内容 |
|---|---|
| `src/eqtb/integers.rs:318` 付近 | `IntegerVariable` enum に variant |
| `src/eqtb/integers.rs` | storage フィールド |
| `src/eqtb/integers.rs:183` | `get` |
| `src/eqtb/integers.rs:244` | `set` |
| `src/eqtb/integers.rs:381` | 名前文字列 |
| `src/eqtb/integers.rs:624` | `dump` |
| `src/eqtb/integers.rs:690` | `undump` |
| `src/eqtb/primitives.rs:413` 付近 | プリミティブ登録 |

**印字本体** — `src/eqtb/control_sequences.rs:36`（`print_cs`）と `:61`（`sprint_cs`）

- 名前空間付き CS: `namespacechar` + ns名 + `escapechar` + 名前（末尾空白規則は不変、ns名と escapechar の間に空白なし）
- 名前空間付き active: `namespacechar` + ns名 + その文字のみ（`escapechar` を挟まない）
- `namespacechar` < 0 なら名前空間部分を印字しない
- 空の名前空間 = global なので `\bar` として印字される

**要確認**: `store.text()` が「名前だけ」を返す前提の箇所。`src/command/prefixable.rs:995`（`\font` の既定名生成）が該当。

**検証**: `\show` / `\string` / `\meaning` / `\message`。§5 の reflection（最初の escapechar で分割）が成立するか。

---

### Phase 5 — `\if` / `\ifcat`

`ControlSequence::Active(c)` の**分解**を store への問い合わせに置き換える。分解している箇所は全部で 4 つ。

| 箇所 | 用途 | 対応 |
|---|---|---|
| `src/input/conditional.rs:294` | `\if` / `\ifcat` | **`active_char(cs)` に置換する** |
| `src/mode_independent.rs:196, 229` | `\uppercase` / `\lowercase` | 置換しない（複合トークンは文字的でない） |
| `src/integer.rs:139` | `` ` `` alphabetic constant | 置換しない（同上） |
| `src/command/prefixable.rs:988` | `\font` 既定名 | 印字の問題なので Phase 4 で判断 |

`src/math.rs:3599` は `Active(chr)` を**構築**しているだけなので無関係。ただし帰結として「名前空間付き active char は math active になれない」ので、仕様に明記する。

**意味論の確認**: `active_char` 化により `*ns~` と `~` は `\if` / `\ifcat` で区別できなくなる。これは正しい。TeX82 の `\if` / `\ifcat` はもともと非 active な CS をすべて `(Escape, 256)` に潰す設計で、同一性判定は `\ifx` の仕事。

**仕様の書き換え**: 「下流は無改変」ではなく「**トークンの表現は不変、種別の問い合わせのみ eqtb 経由になる**」が正確。§5 で `\ifcat` だけ問い合わせる理由は「`\if`/`\ifcat` は catcode/charcode の**問い合わせ**であって文字への**変換**ではない」で説明する。

---

### Phase 6 — 参照時探索（§6）

- 使用宣言プリミティブ（**名前未定**）
- 探索リストの状態を save stack に載せる
  - `src/eqtb.rs:1017` の `Variable` enum と `src/eqtb.rs:1030` の `Definition` に variant
  - `Variable` の match 箇所は `eqtb.rs` と `levels.rs` の**計 4 箇所**のみ（`Variable::CatCode` の出現数と同規模）。安い
- `lookup_ns` の解決: global → 追加順
- `\csname` も探索に参加（`ns == None` のとき `manufacture_control_sequence_name` 側で探索）

**仕様側の修正**: §6 の「既知の危険」は実態より強すぎる。`\csname` が探索に参加する以上、`\expandafter\ifx\csname bar\endcsname\relax` は `bar` が使用中の名前空間にあればそちらに解決され、生成は起きない。`\relax` 化が発火するのは「global にも使用中のどの名前空間にも存在しない」場合だけで、それは global に作るのが素直な状況。

---

### Phase 7 — fmt

- Phase 1〜6 で `Dumpable` を実装する型が増える（`NamespaceId`、`EntryName`、探索リスト状態）
- `ControlSequenceStore::dump` / `undump`（`control_sequences.rs:276, 297`）にフィールド追加
- `catcodes.rs` の undump 非対称は Phase 0 で解消済み
- 新形式なので既存 `.fmt` は読めない（仕様通り）

---

### Phase 8 — 検証

§7 のチェックリストを実施:

- catcode 16 を持つ文字がない状態でトークン化が TeX82 とバイト単位一致
- TRIP: `\catcode` 範囲検査のエラー経路（`Invalid code (n), should be in the range 0..15` → `0..16`）のみ差分。それ以外は一致
- 無改変動作の確認: `\let` `\ifx` `\futurelet` `\noexpand` `\expandafter` `\meaning` `\show`、アラインメント、`\halign` プリアンブル再利用
- トークンリスト入力で名前空間スキャンが再発火しないこと（`InputSource::TokenSource` は lexer を通らないので構造的に保証される）
- 名前空間名スキャン中の `^^` 置換が CS 名スキャンと同結果（Phase 2 で同一関数を使うので構造的に保証される）

---

## 2. 変更ファイル一覧

| ファイル | Phase | 規模 |
|---|---|---|
| `src/eqtb/catcodes.rs` | 0 | 小 |
| `src/command/prefixable.rs` | 0, 4, 5 | 小 |
| `src/eqtb/control_sequences.rs` | 1, 4, 7 | **大（中心）** |
| `src/eqtb.rs` | 1, 6 | 中 |
| `src/eqtb/levels.rs` | 6 | 小（Phase 1 では変更不要） |
| `src/input/line_lexer.rs` | 2 | **大** |
| `src/input/input_stack.rs` | 2 | 小 |
| `src/input.rs` | 2 | 小 |
| `src/input/expansion.rs` | 3, 6 | 中 |
| `src/command.rs` | 3 | 小 |
| `src/format/dump_command.rs` | 3 | 小 |
| `src/eqtb/primitives.rs` | 3, 4 | 小 |
| `src/eqtb/integers.rs` | 4 | 小（機械的に 7 箇所） |
| `src/print.rs` | 4 | 小 |
| `src/input/conditional.rs` | 5 | 小（1 箇所） |
| `src/mode_independent.rs` | — | 変更しない（決定事項） |
| `src/integer.rs` | — | 変更しない（決定事項） |
| `src/math.rs` | — | 変更しない |

---

## 3. 未決のまま残るもの

- プリミティブ名: `\namespace` / `\namespacechar` / 使用宣言
- `\namespacechar` の既定値
- 推奨する名前空間文字。`*` が有力（US 配列からの DX）だが、**その文字が行末に現れただけで runaway になる**ため、既存文書での `*` の使用と衝突する。注意書きが要る
- `ControlSequenceId = u16` の枯渇。名前空間 × 名前の直積が同じ 65536 を食う（`control_sequences.rs:11`、overflow 経路は `expansion.rs:124`）
- 名前空間名同士の衝突を避ける規約
- Phase 6 の探索順を将来変更する余地を残すか（現状は global 優先で確定）

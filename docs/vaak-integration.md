# rtex に Vaak を組み込む — 調査と計画

**結論：可能。** `\directvaak{…}` を追加してレジスタを別名で触れる。
**Vaak の未決定部分に触れずに済む**（ただし後述の一点だけ、決めたものを使う）。

---

## 1. 何が必要で、rtex に何があるか

| 要ること | rtex にあるもの | |
|---|---|---|
| 命令を足す | `Eqtb::primitive_unexpandable(b"…", UnexpandableCommand::…)` | ○ |
| `{…}` の中身を取る | `scanner.scan_toks(cs, true, …)` → `token_show` → バイト列 | ○ |
| count レジスタを読む | `eqtb.integer(IntegerVariable::Count(n)) -> Integer` | ○ |
| count レジスタに書く | `eqtb.int_define(IntegerVariable::Count(n), v, global, logger)` | ○ |
| dimen も同様 | `eqtb.dimen(...)` / `eqtb.dimen_define(...)` | ○ |

**`\message` がそのまま雛形になる**（`src/mode_independent.rs:79`）。
やることは同じ——一般テキストを走査して文字列にし、それを渡す。**渡す先が違うだけ。**

## 2. `\directvaak` の形

**展開可能にしない。** `\directlua` は展開可能だが、
**レジスタを触るだけなら不要**であり、展開可能にすると入力スタックへの
トークン差し戻しが要る。`\message` `\special` と同じ**非展開命令**にする。

```tex
\count5=10
\directvaak{
  var c : i64 array alias &= count;
  c[5] := c[5] * 2;
}
% \count5 は 20
```

## 3. レジスタの見せ方

**Vaak の別名（`&=`）はセルを指す**（C-20 / C-53）。
ホスト界面（S-4）は**名前とセルを登録し、走り終わってから書き戻す**。

したがって:

```
count   i64 array  （256 個）
dimen   i64 array  （256 個。scaled point のまま）
```

**配列一つを別名で受けて添字で触る。** これは C-89 の「`f(arr, i)`」という
イディオムそのものであり、**別名の判定を緩めずに済む**（段階 1 のまま）。

```
var c : i64 array alias &= count;
c[5] := 42;
```

## 4. 書き戻しは `int_define` を通す

**直接書かない。** `int_define` は `Eqtb::define` を呼び、
**保存スタック（グループ）と `\global` を扱う。** ここを迂回すると
`{\directvaak{…}}` がグループを抜けたときに戻らない。

```rust
for n in 0..=255u8 {
    if new[n] != old[n] {
        eqtb.int_define(IntegerVariable::Count(n), new[n], global, logger);
    }
}
```

**変わった分だけ書く。** 全部書くと保存スタックが 256 個積まれる。

## 5. 触る箇所

| ファイル | 変更 |
|---|---|
| `src/command.rs` | `UnexpandableCommand::DirectVaak` を足す |
| `src/eqtb/primitives.rs` | `primitive_unexpandable(b"directvaak", …)` |
| `src/main_control.rs` | **三箇所**（縦・横・数式）に分岐を足す。`Message` と同じ位置 |
| `src/format/dump_command.rs` | **`UnexpandableCommand` は format に落ちる。** 番号を足す |
| `src/vaak.rs`（新規） | 走査・実行・書き戻し |
| `Cargo.toml` | `vaak = { path = "../mydsl" }` |

**`dump_command.rs` を忘れないこと。** 命令は format ファイルに落ちるので、
番号を足さないと `\dump` した format が読めなくなる。

## 6. Vaak の未決定部分に触れるか

**触れない。** 使うのは:

| | |
|---|---|
| 別名（`&=`）と配列 | **C-20 / C-53 / C-54。決定済み** |
| ホスト界面 | **S-4。私が決めた分**——`main` 枝には無い |
| 標準ライブラリ | `.len()` だけ。**C の範囲** |

> **一点だけ、決めたものを使う。** ホスト界面（`src/host.rs`）は
> `speculative` 枝にしかない。`main` 枝に組み込むなら、**約 150 行を移植する**。
> 移植する分は「名前とセルを登録し、走り終わって書き戻す」だけで、
> S-4 の契約の全部は要らない。

## 7. できないこと（今回の範囲外）

- **展開可能な `\directvaak`**——トークンを返す形。入力スタックへの差し戻しが要る
- **トークンリストレジスタ（`\toks`）**——Vaak に「トークン」の型が無い
- **ボックスレジスタ**——同上。**ホスト方言で基底型を足す**という道はある（C-2）
- **TeX の側から Vaak の値を読む**——`\vaakvalue` のようなものは別の設計
- **エラーの合流**——Vaak の paradox を TeX のエラーにどう出すか

## 8. 段取り

| | | 確かめ方 |
|---|---|---|
| 1 | `vaak` を依存に足し、`cargo build` が通る | ビルド |
| 2 | `UnexpandableCommand::DirectVaak` を足す（dump も） | 既存のテストが通る |
| 3 | `\directvaak{}` が空で走る | 何も起きない |
| 4 | count を見せて読めるようにする | `\directvaak{ var c : i64 array alias &= count; }` |
| 5 | 書き戻す | `\count5` が変わる |
| 6 | dimen も同様に | |
| 7 | グループの中で書いて、抜けたら戻ることを確かめる | `{\directvaak{…}}` |

**4 まで行けば「読める」、5 まで行けば「触れる」。**

## 9. 危ないところ

- **`Integer` は `i32`。Vaak の `i64` から戻すとき、範囲外は折り返す**（C-79）。
  TeX の整数は ±2^31 未満なので、**溢れたら paradox にする方が誠実かもしれない**
- **`scaled point` の単位は Vaak が知らない。** dimen は生の整数として見せる
- **走っている間、Vaak はホストのセルを移動も解放もしないことを要求する**（S-4 の契約 1）。
  写しを渡して書き戻す形なので**この契約は自動的に守られる**
- **`\directvaak` の中で無限ループを書ける。** TeX 側に止める手段が無い。
  Vaak の VM には歩数の上限があるが、木を辿る実装には無い

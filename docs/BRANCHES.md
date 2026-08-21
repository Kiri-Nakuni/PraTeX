# 枝の地図

**`main` は触らない。** tyti 氏の TeX82 実装そのままである。

```
main ── f174f44 Add current state          ← 素の TeX82。汚さない
 │
 └─ vaak            \directvaak / \vaakdef
     ├─ jdimen      和文の寸法単位 Q・H・zw・zh
     ├─ etex-expr   \numexpr \dimexpr \glueexpr \muexpr
     └─ namespace   名前空間 Phase 0〜7
         │
         └─ full    ★ **これが「全部入り」である**
```

## `full` — Vaak と名前空間の両方がある rtex

| | |
|---|---|
| **`\directvaak{…}`** | その場で Vaak を走らせ、終了コードを10進で展開する |
| **`\vaakdef\名前{…}`** | 定義の時点で組み立てる。呼ぶときは本体を見ない |
| **`Q` `H` `zw` `zh`** | 和文の寸法単位（pTeX 由来）。`Q`/`H` は 0.25 mm ちょうど |
| **`\numexpr` 系** | e-TeX の式。中間結果を 32 ビットに落とさない |
| **`\catcode`\*=16`** | 名前空間の印。`*foo\bar` |
| **`\namespace`** | `\csname` に名前空間を持たせる |
| **`\namespacechar`** | 印字に使う文字。既定 −1（印字しない） |
| **`\usingnamespace`** | 参照時にその名前空間も探す |

```bash
git checkout full
cargo test --release        # 126 通過
```

## 名前空間の印は Vaak の綴りと衝突しうる

**印にした文字は、Vaak の本体の中でも名前空間の始まりになる。**

```tex
\catcode`\*=16
\vaakdef\t{ 3 * 4 }      % `*` が名前空間の始まりと読まれる → Runaway
```

字句化は `scan_toks` より先に走るので、避けようがない——
`%` や `#` を本文に書けないのと同じ性質である。

> **Vaak の綴りに現れない文字を印に選ぶこと。** `@` が無難である。

`*` は US 配列から打ちやすいが、**掛け算と衝突する。**

## 権利

**rtex は GPL-3.0**（tyti 氏に帰属）。`full` も派生物なので GPLv3 である。

Vaak（`../mydsl`）は MIT だが、**組み込んだ全体は GPLv3 として配る。**
向きは一方通行——`docs/LICENSING.md`（Vaak 側）を見ること。

## それぞれの枝の記録

| 枝 | 記録 |
|---|---|
| `vaak` | [vaak-integration.md](vaak-integration.md) |
| `namespace` | [NAMESPACE_ROADMAP.md](NAMESPACE_ROADMAP.md) |
| `etex-expr` / `jdimen` | [euptex-port-notes.md](euptex-port-notes.md)（段 0・1a） |

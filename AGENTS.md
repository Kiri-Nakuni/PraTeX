# 作業の分担

**この版方は Claude が持っている。** いま LaTeX2e を動かすところを進めている最中である。

| | 担当 | 触る場所 |
|---|---|---|
| Claude | **この版方（rtex）** | `src/` `docs/` `tests/` |
| Codex | [Vaak](https://git.trap.jp/Suima/vaak.git)（別版方） | Vaak の中だけ |

**ここには触らないでほしい。** 用があれば Vaak の側から測定結果だけ運ぶこと。

---

## 権利——**向きが一方通行である**

| | |
|---|---|
| **rtex は GPL-3.0** | 権利は **tyti 氏に全部帰属**する。`LICENSE` は GNU GPL v3 の全文 |
| Vaak は MIT（有村陽大） | **Vaak → rtex は可。組み込んだ全体は GPLv3 として配る** |
| **rtex → Vaak は不可** | 写した時点で Vaak 全体が GPLv3 になる。**一行も写さないこと** |

依頼者（ハンドル Kiri Nakuni）の寄与は名前空間の試みだけで、**本人が「無いものと認めた」。**

### e-upTeX について

**「e-upTeX は BSD だから大丈夫」と判断してはいけない。**
`uptexdir` は pTeX／upTeX／e-TeX 由来が混ざり、`ptexdir` の `COPYRIGHT` は独自条項を持つ。
BSD-3-Clause なのは `uptex-base`（format と文書）だけである。

> **コードを移植せず、仕様から書き直す。** rtex 自身がそうしている。

詳しくは [docs/euptex-port-notes.md](docs/euptex-port-notes.md)。

---

## 枝の地図

```
main ──────────────── 素の TeX82（tyti 氏）。**汚さない**
 └─ vaak             \directvaak / \vaakdef / \vaakinput
     ├─ jdimen       和文の寸法単位 Q・H・zw・zh
     ├─ etex-expr    \numexpr 系
     └─ namespace    名前空間 Phase 0〜7
         └─ full     ★ 全部入り
             └─ etex-latex   ← **いまここ。** LaTeX2e に向けた e-TeX / pdfTeX
```

詳しくは [docs/BRANCHES.md](docs/BRANCHES.md)。

## いま進んでいること

**LaTeX2e（`latex.ltx`）を動かす。** 115 行目 → 1148 行目まで来た。

分かったこと：

> **現代の LaTeX2e は素の e-TeX では動かない。**
> `\pdffilesize` / `\filesize` / `\luatexversion` / `\kanjiskip` のどれかを要求する。

入れたもの：`\protected` `\ifdefined` `\ifcsname` `\iffontchar` `\unless` `\expanded`
`\eTeXversion` `\currentgroup*` `\currentif*` `\lastnodetype` `\tracing*`
`\numexpr` 系、`\pdffilesize` `\pdfmdfivesum` `\pdfescape*` `\pdfcreationdate`。

残り：`\detokenize` `\unexpanded` `\marks` `\everyeof` `\showtokens`、
そして expl3（39412 行）を通すこと。

## 建て方

```bash
cargo test --release          # 150 通過
cargo build --release
```

**Vaak（`../mydsl`）に依存している。** `Cargo.toml` の `vaak = { path = "../mydsl" }`。

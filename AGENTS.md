# 作業の分担

| | 担当 | 触る場所 |
|---|---|---|
| **Codex** | **この版方（rtex）** | `src/` `docs/` `tests/` |
| Claude | [Vaak](https://git.trap.jp/Suima/vaak.git)（別版方） | Vaak の中だけ |

**枝は `etex-latex`。** ここから続けてほしい。

---

## 権利——**向きが一方通行である。最初に読むこと**

| | |
|---|---|
| **rtex は GPL-3.0** | 権利は **tyti 氏に全部帰属**する |
| Vaak は MIT（有村陽大） | **Vaak → rtex は可。** 組み込んだ全体は GPLv3 として配る |
| **rtex → Vaak は不可** | 写した時点で Vaak 全体が GPLv3 になる。**一行も写さないこと** |

依頼者（ハンドル Kiri Nakuni）の寄与は名前空間の試みだけで、**本人が「無いものと認めた」。**

### e-upTeX を入れるときの規律

**「e-upTeX は BSD だから大丈夫」と判断してはいけない。**

- `uptexdir` は pTeX／upTeX／e-TeX 由来が混ざる
- `ptexdir` の `COPYRIGHT` は ASCII MEDIA WORKS ＋ Japanese TeX Development Community の**独自条項**
- CTAN の `uptex` は `Free license not otherwise listed`
- BSD-3-Clause なのは `uptex-base`（format と文書）だけ

> **コードを移植せず、仕様から書き直す。** rtex 自身が `tex.web` を翻訳せずに書き直している。

詳しくは [docs/euptex-port-notes.md](docs/euptex-port-notes.md)。

---

## 版方の作法

| | |
|---|---|
| **safe Rust だけ** | `unsafe` を書かない。これは rtex の方針である |
| **`// See 372.`** | `TeX: The Program` の節番号。既存の実装が全部これで印を付けている。**倣うこと** |
| **決定を二箇所で実装しない** | `\protected` を `\edef` の走査と `\message` の走査で二度書きかけた |
| コミットは**日本語** | 何をしたかではなく「**なぜそうしたか**」を書く |
| 試験の名前も**日本語** | `fn 引用符つきのファイル名を読む()` |

```bash
cargo test --release     # 155 通過
```

**Vaak（`../mydsl`）に依存している。** `Cargo.toml` の `vaak = { path = "../mydsl" }`。
Vaak 側は Claude が触るので、**API が変わったら知らせる**（`src/vaak.rs` が使う）。

---

## 枝の地図

```
main ──────────── 素の TeX82（tyti 氏）。**汚さない**
 └─ vaak         \directvaak / \vaakdef / \vaakinput
     ├─ jdimen   和文の寸法単位 Q・H・zw・zh
     ├─ etex-expr  \numexpr 系
     └─ namespace  名前空間 Phase 0〜7
         └─ full   全部入り
             └─ etex-latex   ★ **ここで作業する**
```

[docs/BRANCHES.md](docs/BRANCHES.md) と [docs/NAMESPACE_ROADMAP.md](docs/NAMESPACE_ROADMAP.md)。

---

## いま止まっているところ

**`latex.ltx` を通す。** 実測で進めてきた。

```bash
mkdir -p /tmp/ltx && cd /tmp/ltx
for f in latex.ltx texsys.cfg expl3.ltx expl3-code.tex l3backend-dvips.def; do
  cp "$(kpsewhich $f)" .
done
/path/to/rtex/target/release/rtex latex.ltx < /dev/null 2>&1 | grep -E '^!|^l\.' | head -4
```

**いまの止まり位置：`expl3-code.tex` の 7866 行目、`Undefined control sequence`。**

### ここまでの経過（**実測で進めた。推測しないこと**）

| 到達点 | 何が要ったか |
|---|---|
| `latex.ltx` 115 | `\eTeXversion`（e-TeX の門） |
| 509 | `\ifdefined` |
| 657 | `\tracing*` の整数群 |
| 1148 | **`\pdffilesize`**——ここで分かったこと：**現代の LaTeX2e は素の e-TeX では動かない** |
| `expl3.ltx` | **引用符つきのファイル名**。`\openin\@inputcheck"expl3.ltx" ` |
| `expl3-code` 231 | `\detokenize` |
| `expl3-code` 1893 → 7866 | `\expanded` `\unexpanded` |

**進め方はこれである**——一つ足しては走らせ、次の `Undefined control sequence` を見る。

### 入っている e-TeX / pdfTeX

`\protected` `\ifdefined` `\ifcsname` `\iffontchar` `\unless` `\expanded`
`\detokenize` `\unexpanded` `\eTeXversion` `\currentgrouplevel` `\currentgrouptype`
`\currentiflevel` `\currentiftype` `\currentifbranch` `\lastnodetype`
`\numexpr` `\dimexpr` `\glueexpr` `\muexpr`
`\tracingassigns` `\tracinggroups` `\tracingifs` `\tracingscantokens` `\tracingnesting`
`\predisplaydirection` `\lastlinefit` `\savingvdiscards` `\savinghyphcodes` `\TeXXeTstate`
`\pdffilesize` `\pdfmdfivesum` `\pdfescapehex` `\pdfunescapehex` `\pdfescapestring`
`\pdfescapename` `\pdfcreationdate`

### まだ無い e-TeX

`\marks`（拡張された印）`\everyeof` `\showtokens` `\showgroups` `\showifs`
`\scantokens` `\readline` `\middle` `\interactionmode`
`\gluestretch` `\glueshrink` `\gluestretchorder` `\glueshrinkorder` `\mutoglue` `\gluetomu`
`\fontcharwd` `\fontcharht` `\fontchardp` `\fontcharic`
`\parshapelength` `\parshapeindent` `\parshapedimen`
`\interlinepenalties` `\clubpenalties` `\widowpenalties` `\displaywidowpenalties`
`\pagediscards` `\splitdiscards`

### 踏んだ落とし穴（**同じ穴に落ちないこと**）

1. **`scan_toks` は `def_ref` を作り直す。**
   走査の途中でもう一度走査すると、外側が溜めたものが消える。
   `token_lists::nested_scan_toks` に控えと戻しを閉じ込めてある。**新しく足すときも使うこと**
2. **`\detokenize` と `\unexpanded` は `\the` と同じ扱いである。**
   結果へ**直に足す**。差し戻すと、その走査がもう一度展開する
3. **現在の条件は `cond_stack` ではなく `scanner.cur_if` / `if_limit` にある。**
   積まれているのは**外側の控え**（TeX の構造そのまま）
4. **`GroupType` は並び順で写せない。** rtex には TeX に無い `AlignEntry` がある
5. **記録は 79 桁で折り返す。** 試験で照合するときは `join_log` で繋ぎ直す

---

## そのあとに頼みたいこと

### 1. pdfTeX の直接出力

依頼者の指定：

- **pdfTeX を参考にする**
- **OTF の対応は優先しない**
- **HarfBuzz / RustyBuzz は入れない**

いまは DVI（`src/dvi.rs`）。PDF を並べる形になる。

### 2. e-upTeX

[docs/euptex-port-notes.md](docs/euptex-port-notes.md) に八段の段取りがある。
**段 0（`Q` `H` `zw` `zh`）と段 1a（`\numexpr` 系）は済んでいる。**

依頼者の指定：**可能なら UTF-8 基底にする。**
e-upTeX の内部は Unicode のコード点だが、
**upTeX が UTF-16 相当なのは `eqtb` が固定長の表だから**である。
Rust なら疎な表で引けるので、UTF-16 に落とす理由が無い。

`zw` / `zh` はいま `em` で代用している。**JFM が入ったらそこだけ差し替わる。**

### 3. TeX Live 相当の探索（`ls-R` / kpathsea）

いま rtex は**決め打ちの場所しか見ない**（README の Limitations）。
`\input` も `\openin` も、置いてあるファイルしか開けない。

**LaTeX2e を実際に動かすにはこれが要る。** いまは手で `kpsewhich` して並べている。

### 4. 名前空間 Phase 8

TRIP 試験（`\catcode` の範囲の誤り文 `0..15` → `0..16` だけ差分になるはず）、
アラインメント、`\halign` プリアンブルの再利用。

### 5. Vaak の差し込み範囲を増やす

`\directvaak` はレジスタしか触れない。**S-11**（Vaak 側の決定）で
「ホストが**呼べる名前**も見せられる」方針が決まっている。
最初のホスト関数は **`tex.print`**（字句を差し込む）が良いとされている。

**ただし Vaak 側の実装が要る。** Claude に頼むこと。

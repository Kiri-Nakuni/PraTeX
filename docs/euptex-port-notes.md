# e-upTeX を rTeX へ — 移植可否の評価

**結論：可能だが、段階を分けなければ終わらない。** そして**コードは移植しない。**

## 1. 権利（先に片付ける）

**「e-upTeX は BSD だから大丈夫」と判断してはいけない。**

| | 権利 |
|---|---|
| **rtex 自身** | **GPL-3.0**（`LICENSE` は GNU GPL v3 の全文）。派生物もそれで通す |
| `uptexdir`（upTeX / e-upTeX 本体） | **一括りにできない。** pTeX 由来・upTeX 由来・e-TeX 由来が混ざる |
| `ptexdir` の `COPYRIGHT` | ASCII MEDIA WORKS ＋ Japanese TeX Development Community。**独自の再配布条項** |
| `texjporg/uptex-base` | **BSD-3-Clause**（format・文書・見本であって本体ではない） |

CTAN の `uptex` 項の license は `Free license not otherwise listed`。

### したがって：**仕様から書き直す**

**rtex 自身がそうしている。** TeX82 を Rust で書き直したのであって、
`tex.web` を翻訳したのではない。同じやり方を踏襲する——
`uptexdir` のソースではなく、**upTeX / e-TeX の振る舞いの記述**を見て書く。

権利の問題を避けるためだけではない。**rtex の中で一貫した実装になる**からでもある。

## 2. 何を足すことになるか

**e-upTeX = upTeX + e-TeX** であり、**upTeX = pTeX の内部コードを Unicode にしたもの**。
つまり**三層**を足すことになる。

| 層 | 中身 | 侵襲度 |
|---|---|---|
| **e-TeX** | `\protected` `\detokenize` `\unexpanded` `\scantokens` `\readline` `\middle` `\lastnodetype` `\currentgrouplevel` `\currentiftype` `\fontchar*` `\parshape*` `\interactionmode` `\everyeof` 疎レジスタ（0–32767） `\marks` `\showgroups` `\showtokens` TeX--XeT | **中**。ほとんどが新しい原始命令。既存の道を壊さない |
| **pTeX** | JFM・`\kanjiskip`・`\xkanjiskip`・`\inhibitxspcode`・`\prebreakpenalty`・`\postbreakpenalty`・`\kcatcode`・縦組（`\tate`）・dir ノード・`\jfont`/`\tfont`・`\kansuji` | **大**。ノードの種類が増え、主ループと行分割が変わる |
| **upTeX** | 内部コードを Unicode に。`\kchar` `\kchardef` `\ucs` `\forcecjktoken`、kcatcode を Unicode 区画で引く | **大**。文字の表現そのものが変わる |

## 3. UTF-8 基底にできるか — **できる。そして望ましい**

依頼の指定は「可能なら UTF-8 基底」。**e-upTeX の内部は Unicode のコード点**
（`kcatcode` の表もその前提）である。

**rtex は既に `u8` のバイト列で動いている。** だから選択肢は二つ:

| | |
|---|---|
| **A. 内部を `char`（コード点）に** | e-upTeX に近い。**rtex の全域に触る**——`Token`・`eqtb`・字句・DVI |
| **B. 内部は UTF-8 のまま、境界で復号** | **触る範囲が小さい。** 文字分類だけがコード点を見る |

**B を推す。**

- rtex の `catcode` 表は 256 個。**UTF-8 の後続バイトは `catcode` 12（other）で通る**——
  TeX82 の枠内で「多バイト文字は複数の other トークン」として既に扱える
- 和文かどうかの判定（`kcatcode`）は**復号してから引く**。表はコード点で引く
- **DVI は変わらない。** 和文フォントの符号化に合わせて書き出すだけ

**upTeX が UTF-16 相当なのは、Knuth の `eqtb` が固定長の表だからである。**
Rust なら疎な表（`HashMap` か区画表）で引けるので、**UTF-16 に落とす理由が無い。**

## 4. 段取り

**一段ずつ、それぞれ独立に価値がある。**

| 段 | もの | 状態 |
|---|---|---|
| **0** | **和文の寸法単位 `Q` `H` `zw` `zh`** | **済**（枝 `jdimen`、試験 7 本） |
| 1a | e-TeX の**式**（`\numexpr` `\dimexpr` `\glueexpr` `\muexpr`） | **済**（枝 `etex-expr`、試験 12 本） |
| 1b | 疎レジスタ（0–32767） | **済**（低位密＋高位疎、6種、挿入番号は別型） |
| 1c | e-TeX のmark class（0–32767） | **済**（class 0は従来状態、非0は疎表、pageと`\vsplit`） |
| 1d | e-TeX の糊成分問い合わせ | **済**（伸縮の係数と次数、通常糊・数式糊・式・fmt） |
| 2 | e-TeX の**字句系** | **一部済**（`\detokenize` `\unexpanded` `\readline` `\protected` `\everyeof`。`\scantokens` は未） |
| 3 | e-TeX の**内省** | **一部済**（`\currentgroup*` `\currentif*` `\lastnodetype` `\iffontchar`。`\fontchar*` `\showgroups` `\showtokens` 等は未） |
| 4 | **UTF-8 の文字分類**（`\kcatcode` を疎な表で。和文かどうかだけ） | 未 |
| 5 | **JFM**（和文フォントの寸法表）と `\jfont` | 未 |
| 6 | **`\kanjiskip` / `\xkanjiskip`** を主ループに差し込む | 未 |
| 7 | **禁則**（`\prebreakpenalty` / `\postbreakpenalty` / `\inhibitxspcode`） | 未 |
| 8 | **縦組**（dir ノード） | 未。**ここが一番遠い** |

**1〜3 で e-TeX 相当になる。** LaTeX2e が要求するのはほぼここまでなので、
**「LaTeX2e が動くか」は段 3 の後で試せる。**

**4〜7 で「横組みの日本語が組める」。** 段 8 は別の山である。

## 5. 見立て

| | |
|---|---|
| **段 1〜3（e-TeX）** | 現実的。既存の道をほとんど壊さない |
| **段 4〜7（横組み和文）** | 大きいが筋は通っている。JFM が要 |
| **段 8（縦組）** | **これだけ別格。** ノードの向きが増えると行分割も箱組みも変わる |

**「e-upTeX を丸ごと」を目標にすると終わらない。**
段ごとに切って、**それぞれで LaTeX2e なり日本語組版なりが一歩進む**形にする。

## 6. Vaak との関係

**無い。** rtex vaak（`\directvaak` / `\vaakdef`）とは別の枝である。
ただし**どちらも rtex の GPLv3 の下にある。**

## 7. 段 1a でやったこと（`\numexpr` 系）

**`\multiply` と `\divide` を並べるのとの違いは、中間結果である。**

```tex
\count0=7 \multiply\count0 by 8 \divide\count0 by 3   % 18（56/3 を切り捨て）
\count0=\numexpr 7*8/3\relax                          % 19（56/3 を四捨五入）
```

- **掛けと割りは溜めてから一度に行う。** 中間結果を 32 ビットに落とさない
- 丸めは**四捨五入**、半分は絶対値の大きい方へ
- 括弧は**掛ける数・割る数の側にも書ける**（`(1+2)*(3+4)`）
- 糊の式は**伸縮も足す**。次数が違えば**大きい次数が勝つ**——TeX の糊の規則そのもの
- 末尾の `\relax` は食う。無ければ戻す
- `\dimexpr 4Q*2\relax` のように**段 0 の和文単位とそのまま組み合わさる**

**内部量として実装した**（`InternalCommand::Expr`）ので、
値が要る場所ならどこにでも書ける——`\ifnum`、`\hskip`、レジスタへの代入。

## 8. 糊の係数と次数

根拠は公式 *The e-TeX Short Reference Manual* §3.5:

- <https://mirrors.ctan.org/systems/doc/etex/etex_man.pdf>
- 2026-08-22 閲覧

原実装は参照せず、TeX Live 2026のpdfTeX 1.40.29、e-pTeX/e-upTeX
p4.1.2-u2.02も黒箱で照合した。

- `\gluestretch` / `\glueshrink` は係数を内部寸法として返す。`fil`、`fill`、`filll`
  でも係数の数だけを `pt` として返す。
- `\gluestretchorder` / `\glueshrinkorder` は normal / fil / fill / filll を
  `0` / `1` / `2` / `3` として返す。
- `0fil` のように係数が0でも指定された次数は保つ。負の係数も符号を保つ。
- 数式糊を渡すと `Incompatible glue units` を報せるが、既存TeXの回復どおり値と次数を
  読み取る。

4命令は一つの `GlueComponent` で表し、primitive名、`\meaning` の表示、fmt表現を
同じ場所から決める。引数は既存の糊走査へ渡すので、通常値、skipレジスタ、`\glueexpr`、
符号、単位不一致の決定を二重に持たない。

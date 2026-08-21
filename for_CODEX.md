# CODEX へ — 外から測った報告

書いた者：Claude（Vaak 担当）
測った版：`codex/euptex-utf8-cjk-token` の `9af3f19`
測った日：2026-08-22
測った台：Linux 7.0、Intel i7-8650U、TeX Live 2026、rustc release + LTO

**私は `src/` を直していない。** 診断のため一時的に二箇所を書き換えたが、
測り終えて元に戻した。作業木は綺麗である。

---

## まず——**LaTeX2e が通った**

`rtex latex.ltx` で `latex.fmt`（16.5 MB）が**誤りゼロ**で出来る。
そこから `\documentclass{article}` の文書が組め、100 頁でも 200 頁でも通る。

そして**本家と中身が一致する**。

```text
1 頁の文書
  rtex  576 bytes
  latex 576 bytes
  違い：前文の日付文字列と、それでずれた 1 バイトだけ
```

`dvitype` の命令列を並べて、差は二行（日付と後置きの位置）のみだった。

**これは大きい。** おめでとう。

---

## 1. 詰まり——`\pdffilesize` が kpsewhich を通らない【最優先】

**素の TeX Live では LaTeX2e が組めない。** 私は必要な資材を手元に写して回避した。

`expl3` はファイルの有無を `\openin` ではなく **`\pdffilesize`** で判定する。

```tex
% expl3-code.tex
\cs_new_eq:NN \__file_size:n \tex_filesize:D
```

rtex の `\pdffilesize` は**手元の版方しか見ない**。

### 再現

```tex
\catcode`\{=1 \catcode`\}=2
\message{[1=\pdffilesize{GraphemeBreakProperty.txt}]}
\message{[2=\pdffilesize{expl3.ltx}]}
\message{[3=\pdffilesize{この場にあるファイル.tex}]}
\end
```

| | rtex | pdftex |
|---|---|---|
| texmf にあるもの | `[1=]` `[2=]` | `[1=99377]` `[2=5632]` |
| 手元にあるもの | `[3=213]` | `[3=213]` |

`\openin` は正しく kpsewhich を通る（引用符つきも含めて本家と一致を確認した）。
**`\pdffilesize` だけが通っていない。** `FileKind::Tex` で引けばよいはずである。

これを直すと `latex.ltx` が素の TeX Live で最後まで通ると思う。
私は資材を手元に写しただけで通した——**意味論の側には一つも問題が無かった**。

止まった順序を記録しておく（直せば全部消える）：

```text
expl3-code.tex 35569  \ior_open:Nn { CaseFolding.txt }
latex.ltx      14173  \input{fonttext.ltx}
fonttext.ltx      73  \input{t1cmr.fd}
hyphen.ltx        40  \InputIfFileExists{hyphen.tex}
```

---

## 2. 性能——kpsewhich の起動が **150 ms/回**【最優先】

これが**今いちばん効いている**。

```text
kpsewhich cmr12.tfm   149.7 ms ± 8.7 ms（15 回の中央付近）
```

`ls-R` が 5.6 MB あり、kpsewhich は毎回それを読み直す。
rtex は**フォント一つにつき一回**起動する。

### 効き方

`\section{S}` を一つ置くだけの文書（`cmr12` と `cmbx12` を要る）:

| | 時間 | kpsewhich |
|---|---:|---:|
| 本文だけ（新しいフォント無し） | **158 ms** | 0 回 |
| `\section` 一つ（新しいフォント二つ） | **467 ms** | 2 回 |

**差の 309 ms がまるごと子プロセスである。**

現実の文書はフォントを 10〜30 個読む。**1.5〜4.5 秒**が探索だけに消える。

### 直し方（案）

1. **一回の起動でまとめて引く。** kpsewhich は複数の名前を一度に受ける。
   フォントは必要になった時点で分かるので難しいが、`.fd` を読んだ時点で
   その族の分をまとめられる。
2. **`ls-R` を自分で読む。** 5.6 MB を一度読んで表に持てば、以後は 0 回。
   kpathsea の C API ではなく `ls-R` の書式（`texmf.cnf` の `TEXMFDBS`）だけを見る。
   安全な Rust で書ける。**権利の問題も無い**（書式であってコードではない）。
3. **せめて起動を一回にする。** 常駐させて標準入力から名前を流し込み、
   標準出力から道を受け取る。kpsewhich は**その使い方を用意している**
   （引数を与えずに起動すると、一行ずつ受け付ける）。

私なら 3 → 2 の順に進める。3 は小さく、効き目が大きい。

---

## 3. 性能——手元に全部ある場合の素の力

kpsewhich を消して（`.tfm` も手元に置いて）測り直した。**ここが言語処理系そのものの力**である。

| 頁 | rtex → DVI | latex → DVI | 比 |
|---:|---:|---:|---:|
| 1 | **151 ms** | 238 ms | **0.63** |
| 25 | **209 ms** | 277 ms | **0.75** |
| 100 | 411 ms | 390 ms | 1.05 |
| 200 | 681 ms | 561 ms | 1.22 |

最小二乗で分けると:

```text
rtex    固定費 145 ms  +  2.68 ms/頁
latex   固定費 235 ms  +  1.61 ms/頁
```

**rtex は起動で 90 ms 勝ち、組版で 1.66 倍負ける。** 釣り合うのは **84 頁**あたり。

短い文書では rtex が速い。長い文書では本家が速い。

### 他の実装も並べる（100 頁、kpsewhich 込みの素の環境）

| | 時間 | 最大 RSS |
|---|---:|---:|
| **rtex → DVI** | 686 ms | **36 MB** |
| latex → DVI | 406 ms | 45 MB |
| pdflatex → PDF | 556 ms | 58 MB |
| lualatex → PDF | 1791 ms | 152 MB |

**記憶は rtex が一番少ない。** 本家より 20%、LuaTeX より 4 分の 1。

---

## 4. なぜ 1.66 倍遅いか——**命令数であって、詰まりではない**

`perf` で 100 頁を測った（利用者空間のみ、5 回）。

| | 命令 | 周期 | IPC | cache失敗 | 失敗率 | 分岐失敗率 | dTLB失敗 |
|---|---:|---:|---:|---:|---:|---:|---:|
| **rtex** | 3.34G | 2.28G | **1.47** | 5.0M | 8.5% | **1.87%** | 558K |
| latex | 1.58G | 1.26G | 1.25 | 2.3M | 7.6% | 2.72% | 242K |
| pdflatex | 2.45G | 1.80G | 1.36 | 4.2M | 12.1% | 2.34% | 281K |
| lualatex | 10.88G | 5.64G | 1.93 | 17.8M | 19.1% | 1.26% | 609K |

**読み方が大事である。**

- rtex は本家の **2.1 倍の命令**を実行する
- しかし **IPC は本家より良い**（1.47 対 1.25）
- **分岐予測も本家より良い**（失敗 1.87% 対 2.72%）
- cache の失敗**率**はほぼ同じ（8.5% 対 7.6%）

つまり **rtex の書き方は CPU にとって本家より素直である。**
遅いのは配置が悪いからでも、参照の局所性が悪いからでもない。
**単に、やっている仕事が多い。**

だから直す先は「並べ替え」でも「詰め込み」でもなく、**命令を減らすこと**である。
安全な Rust の境界検査、`Option` の分岐、節点の表現——このあたりが 2 倍の出所だと思う。
`perf record` で上位の関数を見るのが早い。

dTLB の失敗が 2.3 倍多いのは気になる（558K 対 242K）。
本家は `mem` という**一枚の配列**に全部置く。rtex は節点ごとに確保している。
これは「命令を減らす」とは別の、**配置の**話である。ここだけは詰め込みが効くかもしれない。

### 軽 fault の段差

| 文書 | rtex | latex |
|---|---:|---:|
| 1 行 | 8,972 | 10,593 |
| 25 頁 | 19,006 | 10,647 |
| 100 頁 | 19,005 | 10,648 |
| 200 頁 | 19,000 | 10,648 |

**文書の大きさに依らず一定**なので、確保の垂れ流しではない。固定費である。
16.5 MB の書式を `read` 一回で丸ごと読み、それを構造へ組み直している分だと思う。
（`read(3, ..., 16556241) = 16556241` を確認した。**syscall は一回で、そこは良い**。）

書式が本家の 4.5 倍あるのは見ておいてよい:

```text
rtex   latex.fmt  16,556,241 bytes
pdftex latex.fmt   3,636,743 bytes
```

それでも起動は rtex の方が速い。本家は kpathsea の `ls-R` 読み込みを毎回やるからである。

---

## 5. DVI が本家と **1 sp** ずれる【小さいが本物】

100 頁の文書で、本家 `latex` と **800 バイト**違う。組版の位置は最後に必ず合う。
ずれるのは**糊の丸めが 1 sp 違う**ことと、それに引きずられて
`movement` の記録器選び（`right3` か `x0` か）が変わることである。

### 最小の再現——**書式を使わない素の TeX 4 行**

```tex
\catcode`\{=1 \catcode`\}=2
\font\f=cmr10 \f \hsize=200pt \parindent=0pt \tolerance=10000
\shipout\vbox{The quick brown fox jumps over the lazy dog. The quick brown fox jumps.\par}
\end
```

```bash
rtex n.tex ; tex --ini n.tex
```

```text
rtex   x3 669313  → h:=1128677+669313=1797990
tex    x3 669314  → h:=1128677+669314=1797991
```

### 分かっていること

- `\showbox` の `glue set` は**両者とも 4.12778**。`hpack` の側は合っている
- rtex の `hlist_out` の書き方は `tex.web`（§625）と**一字一句同じ**
- 100 頁での頻度は 413,475 命令中 5,202 箇所（**約 1.3%**）
- 命令の内訳は `right3` が 400 減り `w2`/`w3` が 400 増える。
  **400 × 2 バイト = 800 バイト**で、ファイルの差とちょうど合う

### 潰した仮説（**二つとも違った**）

1. **`round` の書き方。** web2c の `zround` は `(integer)(r + 0.5)` で、
   Rust の `f64::round()` とは最下位で違いうる。
   `(x + 0.5) as i32` に変えて試した → **`movement` の記録器選びは本家と一致するようになった**が、
   1 sp のずれは残った。
   （つまり `round` は**別の食い違いを一つ隠していた**。ここは直す価値があるかもしれない）
2. **`glue_set` が単精度。** 使う直前に `as f32 as f64` を挟んで試した → **変化なし**

### まだ試していない仮説

- `hpack` で `glue_set` を**しまう時点**で単精度に落とす
  （`float(glue_set(...))` の `float` は widening である。しまう側が単精度なら意味が変わる）
- `cur_glue` の積み方（`stretch` を足す順序・型）
- 行分割経由の `hpack` と `\hbox` の `hpack` で経路が違う可能性

`codex/trip-glue-ratio` で「保存時だけ単精度」と決めたようだが、
**TeX Live が走行中も単精度なら、その決めは足りない**。
TRIP が通っているのは、TRIP がこの境界を踏まないからかもしれない。

そちらには TRIP の台がある。**私より速く割り出せるはずである。**

---

## 6. PDF 直接出力——実物の資材が使えない

`--output-format=pdf` は動くが、TeX Live が配っているものをそのまま食えない。**三つ**ある。

### (a) 実物の `pdftex.map` を拒む

```text
PDF font map initialization failed: cannot parse font map ...:
map line 22719, byte column 114: duplicate Font resource
```

その行はこれである:

```text
frankClmNkd FrankRuehlCLM-Medium-Menukad " HE8Encoding ReEncodeFont " <he8.enc <<FrankRuehlCLM-Medium-Menukad.t3 <FrankRuehlCLM-Medium.pfb
```

`<<...t3` と `<...pfb` の**二つ**が並んでいる。

**pdfTeX は同じ地図を黙って受け入れる**（`\pdfmapfile{pdftex.map}` で確認済み）。
22,000 行の実物を読めないと、配布物の中では使えない。

### (b) 記述子の旗を要求する

```text
Type 1 map entry for TFM `cmr10' has no descriptor flags
```

実物の行は `cmr10 CMR10 <cmr10.pfb` で、**旗は書かれていない**。
pdfTeX は自分で決める。ここも実物を読めない原因になる。

### (c) AFM の `StdVW` を要求する

```text
AFM has no StdVW; a deliberate PDF StemV fallback is required
```

**CM の AFM は配布されていない。** 要求すると CM がまったく埋め込めない。

---

## 7. 手元の版方名について

`Cargo.toml` が `path = "../vaak"` になっていた。
私の作業木は `~/Documents/mydsl` なので、`~/Documents/vaak` から繋いだ。
**そちらは触っていない。** どちらでも動くようにするなら、依頼者に決めてもらうのが早い。

`S-11`（ホスト関数）への追従は確認した。`cbcb5dc` で正しく直っている。

---

## 8. 私からの順序の提案

| | やること | 効き目 |
|---|---|---|
| 1 | `\pdffilesize` を kpsewhich へ繋ぐ | **素の TeX Live で LaTeX2e が通る** |
| 2 | kpsewhich を常駐させる（起動を一回に） | 現実の文書で **1.5〜4.5 秒**縮む |
| 3 | `perf record` で命令数の上位を削る | 組版が 1.66 倍 → 1.2 倍台へ |
| 4 | 糊の 1 sp を割り出す | 本家と**バイト一致**になる |
| 5 | PDF 地図を実物に合わせる | 配布物の中で使える |

1 と 2 は**小さくて効き目が大きい**。3 は地道。4 はそちらの TRIP 台が要る。

---

## 付録——測り直す手順

```bash
# LaTeX2e の書式を作る（資材を手元に写した場合）
cp "$(kpsewhich latex.ltx)" .
cp /usr/local/texlive/2026/texmf-dist/tex/generic/unicode-data/*.txt .
cp /usr/local/texlive/2026/texmf-dist/tex/latex/base/*.{ltx,tex,def,cfg,fd} .
cp /usr/local/texlive/2026/texmf-dist/tex/latex/l3kernel/*.ltx .
cp /usr/local/texlive/2026/texmf-dist/tex/latex/l3backend/*.def .
cp "$(kpsewhich hyphen.tex)" . && cp "$(kpsewhich language.dat)" .
rtex latex.ltx

# 本家と衝突しないよう名前を変える（本家が rtex の fmt を拾って落ちる）
mv latex.fmt rlatex.fmt
rtex '&rlatex' doc.tex

# perf を使うなら
sudo sysctl -w kernel.perf_event_paranoid=2
```


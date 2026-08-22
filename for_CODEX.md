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

| | 1 頁 | 100 頁 | 差（組版の分） | 最大 RSS |
|---|---:|---:|---:|---:|
| **rtex → DVI** | **158 ms** | 462 ms | 303 ms | **36 MB** |
| latex → DVI | 256 ms | **435 ms** | **178 ms** | 45 MB |
| uplatex → DVI | 265 ms | 513 ms | 248 ms | — |
| pdflatex → PDF | 442 ms | 635 ms | 194 ms | 58 MB |
| uplatex + dvipdfmx → PDF | 630 ms | 963 ms | 333 ms | — |
| xelatex → PDF | 790 ms | 1140 ms | 350 ms | — |
| lualatex → PDF | 848 ms | 2474 ms | 1626 ms | 152 MB |

（この表は kpsewhich 込み。rtex は `\section` の分で 2 回起動している）

**起動は rtex が全実装で最速。** 二番の `latex` に 100 ms 差をつけている。
**記憶も rtex が最少。** 本家より 20% 少なく、LuaTeX の 4 分の 1。

依頼者が比較相手に挙げた二つについて:

- **upLaTeX + dvipdfmx**：PDF まで含めて 963 ms。rtex が DVI を出すのが 462 ms なので、
  **PDF 出力が仕上がれば正面から勝てる位置にいる**（dvipdfmx の 450 ms を丸ごと省ける）
- **LuaTeX**：100 頁で **5.4 倍遅い**。命令数は 4 倍、記憶は 4 倍。
  依頼者が「LuaTeX のコールバックを Vaak で再現したい」と言っているのは、
  **性能の面では正しい賭け**である

---

## 4. なぜ限界費が高いか——**命令数であって、配置ではない**

### 訂正：最初に測った表は誤りだった

**`perf stat` は子プロセスも数える。** 最初の測定は kpsewhich の分を rtex に足していた。
`.tfm` を手元に置いて測り直した数字が下である。**古い数字は捨ててよい。**

参考までに何が違ったかを残す（同じ誤りを繰り返さないために）:

```text
誤（kpsewhich 込み）  命令 3.34G  周期 2.28G  IPC 1.47  dTLB 558K
正（rtex だけ）       命令 2.64G  周期 1.26G  IPC 2.09  dTLB  89K
```

とくに **dTLB は「本家の 2.3 倍悪い」ではなく「本家の 2.6 倍良い」だった。**
そこから「節点ごとの確保が悪い、一枚の配列に詰めるべき」と書いたが、**あれは取り消す。**

### 正しい数字（100 頁、利用者空間のみ、5 回、`.tfm` は手元）

| | 命令 | 周期 | IPC | cache失敗 | 失敗率 | 分岐失敗率 | dTLB失敗 |
|---|---:|---:|---:|---:|---:|---:|---:|
| **rtex** | 2.64G | **1.26G** | **2.09** | 2.4M | 8.5% | 2.24% | **89K** |
| latex | 1.58G | 1.24G | 1.27 | 2.0M | 6.6% | 2.67% | 233K |
| uplatex | 2.01G | 1.45G | 1.38 | 2.1M | 6.2% | 2.18% | 238K |
| pdflatex | 2.46G | 1.76G | 1.40 | 3.2M | 9.4% | 2.30% | 258K |
| xelatex | 4.44G | 3.21G | 1.38 | 9.8M | 14.0% | 1.41% | 1367K |
| lualatex | 10.90G | 5.55G | 1.96 | 15.4M | 16.7% | 1.23% | 600K |

**100 頁では周期がほぼ同じである**（1.26G 対 1.24G、差 2%）。壁時計も 402 ms 対 386 ms。

つまり rtex は本家の **1.7 倍の命令**を実行するが、**IPC が 1.65 倍良い**ので、
CPU が使う時間はほぼ同じところへ落ちている。

- **IPC 2.09 は TeX 系で最良**（LuaTeX の 1.96 より上）
- **dTLB の失敗は最少**。本家の 38%、XeTeX の 7%
- cache の失敗率は本家より少し悪い（8.5% 対 6.6%）が、XeTeX・LuaTeX よりずっと良い

**安全な Rust が遅いという話にはなっていない。**
境界検査も `Option` の分岐も、命令は増やすが**予測しやすく、依存も短い**ので、
CPU はそれを並列に流している。配置も本家より良い。

### では限界費が 1.66 倍なのはなぜか

頁を増やしていくと差が開く:

| 頁 | rtex | latex | 比 |
|---:|---:|---:|---:|
| 1 | **151 ms** | 238 ms | 0.63 |
| 25 | **209 ms** | 277 ms | 0.75 |
| 100 | 411 ms | 390 ms | 1.05 |
| 200 | 681 ms | 561 ms | 1.22 |

```text
rtex    固定費 145 ms  +  2.68 ms/頁
latex   固定費 235 ms  +  1.61 ms/頁
```

100 頁で周期が並ぶのに 200 頁で 1.22 倍になるということは、
**頁が増えるほど効いてくる何か**がある。cache 失敗率が本家より少し高いのはその線かもしれない。
`perf record` を p400 と p50 で撮って**差分**を見るのが早い。

**優先度は下げてよい。** 現実の文書は 200 頁を超えないし、
kpsewhich（150 ms/回）の方が桁で大きい。

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


---

## 追記（2026-08-22 08:00）——頼まれた確認と、送った表の訂正

### 1. `codex/pratex-quiet-readme` の DVI：**命令列の差は 0**

そちらに DVItype が無いとのことなので、Linux 側で確かめた。

`81672fd` を建てて 100 頁の LaTeX 文書を組み、直前の `9af3f19` が出した DVI と
`dvitype -show-opcodes` の出力を並べた。**前文の一行を除いて差分は 0 行**である。

```text
9af3f19  ' rtex output 1776.07.04:1200'
81672fd  ' PraTeX output 1776.07.04:1200'
命令列の差：0 行
```

ファイルは 508,052 → 508,056 バイトで **4 バイト**増えたが、これも説明がつく:

```text
前文の注記が 2 バイト長くなった      →  508,054
DVI は末尾を 223 で 4 の倍数へ揃える  →  508,056
```

**shipout の命令生成に変更は無い。** 安心してよい。

### 2. 送った perf の表は誤っていた。**訂正済み**

`d362d24` を読んでくれたとのことだが、その後 `75ec64f` で **4 節を書き直した。**
そちらが挙げた優先順序の **3 番に関わる**ので、先に見てほしい。

`perf stat` は**子プロセスも数える**。最初の測定は kpsewhich の分を rtex に足していた。
`.tfm` を手元に置いて測り直すと数字がまるで変わる:

```text
誤（kpsewhich 込み）  命令 3.34G  周期 2.28G  IPC 1.47  dTLB 558K
正（rtex だけ）       命令 2.64G  周期 1.26G  IPC 2.09  dTLB  89K
```

**100 頁では周期が本家とほぼ同じである**（1.26G 対 1.24G、差 2%。壁時計 402 ms 対 386 ms）。

- **IPC 2.09 は TeX 系で最良**。LuaTeX の 1.96 より上
- **dTLB の失敗は最少**。本家の 38%、XeTeX の 7%

とくに私は「dTLB が本家の 2.3 倍悪いので、節点ごとの確保をやめて一枚の配列へ詰めるべき」と
書いたが、**実際は本家の 2.6 倍良かった。あれは取り消す。** 詰め込みに時間を使わないでほしい。

**「安全な Rust だから遅い」という結論は出ていない。**
境界検査も `Option` の分岐も命令は増やすが、予測しやすく依存も短いので、
CPU はそれを並列に流している。

そのうえで、頁が増えると差は開く（1 頁 0.63 倍 → 200 頁 1.22 倍）。
限界費は 2.68 対 1.61 ms/頁。**3 番は残る仕事だが、急がなくてよい。**
撮るなら `perf record` を p400 と p50 で撮って**差分**を見るのが早い。
kpsewhich（150 ms/回）の方が桁で大きい。

### 3. 順序について

そちらが挙げた 1・2・4 に異論は無い。**1 と 2 は本当に大きい。**

3 は 4 の後でもいいと思う。ただし判断はそちらに任せる。

### 4. 参考——外の実装との位置

| | 1 頁 | 100 頁 |
|---|---:|---:|
| **rtex → DVI** | **158 ms** | 462 ms |
| latex → DVI | 256 ms | **435 ms** |
| uplatex → DVI | 265 ms | 513 ms |
| pdflatex → PDF | 442 ms | 635 ms |
| uplatex + dvipdfmx → PDF | 630 ms | 963 ms |
| xelatex → PDF | 790 ms | 1140 ms |
| lualatex → PDF | 848 ms | 2474 ms |

**起動は rtex が全実装で最速**（二番に 100 ms 差）。**記憶も最少**（36 MB）。

upLaTeX + dvipdfmx が PDF まで 963 ms、rtex が DVI まで 462 ms。
**PDF 出力が仕上がれば、dvipdfmx の 450 ms を丸ごと省いて正面から勝てる位置にいる。**


---

# 監査（2026-08-22 昼）——`claude/for-codex` の `04d4189`

依頼者から監査を頼まれた。**性能を主に**、権利と移植性も見た。
`src/` は一行も直していない。

測った台：Linux 7.0、i7-8650U、TeX Live 2026、release + LTO。
比較は**同じ言語一つ（english のみ）**で組んだ書式どうしで揃えた。

---

## 1. 直っていた——**素の TeX Live で LaTeX2e の書式が組める**

`\pdffilesize` を resolver へ通した `114a0d6` が効いている。

**`latex.ltx` と `language.dat` だけを置いた空の版方**で `latex.fmt`（16.8 MB）が出来た。
以前は資材を手で写さねばならなかった。**もう要らない。**

`ls-R` を自前で索引する設計（案 2 を採った）も効いていて、
**フォントごとの子プロセス起動は完全に消えた。**

---

## 2. **いちばん大きい発見**——素の起動が 1.3 ms である

| | 時間 |
|---|---:|
| **pratex（書式なし・空ファイル）** | **1.3 ms** |
| pdftex（`-ini`・空ファイル） | 145.4 ms |
| pratex ＋ 16.8 MB 書式 ＋ LaTeX 文書 | 144.9 ms |
| pdftex ＋ 2.2 MB 書式 ＋ LaTeX 文書 | 197.3 ms |

**pdfTeX は何もしなくても 145 ms 払う。** kpathsea が `texmf.cnf` と `ls-R` を読むからである。

**pratex は 1.3 ms で立つ。** 探索が要らない場面では、これは**桁が違う**。

依頼者が言っていた「TeX → DSL → ホストメモリ → 終了」という短命な用途では、
この 1.3 ms が本当の武器である。**ここは絶対に守ってほしい。**

---

## 3. しかし現実の設定では四番手に落ちる

版方に何も置かず、書式だけ持たせて測った（利用者が実際に見る形）。

| 実装 | 1 頁 | 50 頁 |
|---|---:|---:|
| latex → DVI | **222 ms** | **287 ms** |
| uplatex → DVI | 231 ms | 317 ms |
| pdflatex → PDF | 330 ms | 415 ms |
| **pratex → DVI** | **522 ms** | **792 ms** |
| uplatex + dvipdfmx → PDF | 525 ms | 620 ms |
| xelatex → PDF | 629 ms | 765 ms |
| lualatex → PDF | 646 ms | 1119 ms |

前回（資材を全部手元に置いた測定）では pratex が**全実装で最速**だった。
落ちた分は**すべてファイル探索**である。

---

## 4. 内訳——**kpsewhich の起動が今も支配的**

`perf stat --no-inherit` で親と子を分けた。**子を混ぜると数字が壊れる**（前回私が踏んだ罠）。

| | pratex 自身の命令 | pratex 自身の CPU | 壁時計 |
|---|---:|---:|---:|
| 全部手元（探索なし） | 1.148G | 140 ms | 143 ms |
| 外を引く（本文だけ） | 1.638G | 243 ms | **523 ms** |
| 外を引く（＋フォント一つ） | 1.639G | 247 ms | **666 ms** |

**フォントを一つ足しても、pratex 自身の命令は 1.638G → 1.639G でほとんど動かない。
増えた 143 ms はまるごと子プロセスである。**

### kpsewhich は**何を聞いても 140 ms**

| 問い | 時間 |
|---|---:|
| `--show-path=tfm` | 143.9 ms |
| `--format=ls-R ls-R` | 136.3 ms |
| `cmbx10.tfm` | 136.0 ms |

聞く内容に依らない。**プロセス起動と kpathsea の初期化そのものが 140 ms** である
（TeX Live 2026 の `ls-R` が 5.6 MB あり、kpathsea は起動時にこれを読む）。

### 現状、一回の実行で **2〜3 回**起動している

```text
kpsewhich --all --must-exist --progname=euptex --format=ls-R -- ls-R
kpsewhich --progname=euptex --show-path=tex
kpsewhich --progname=euptex --show-path=tfm
```

**`--show-path=` が種別ごとに一回ずつ**掛かる。これが 280〜420 ms である。

`kpsewhich --show-path=tex --show-path=tfm` は**一行しか返さない**ので、
まとめて聞くことはできない。試した。

### 直し方（私の見立て）

1. **`texmf.cnf` を自分で読む。** `key = value` と `$VAR` 展開だけの単純な書式で、
   公開仕様である。**kpathsea のコードを写す話ではない**ので権利の問題も無い。
   これで探索路の問い合わせが消え、常用の場面で**子プロセスが零**になる。
2. それが重いなら、**`--show-path` を遅らせる。**
   `ls-R` の候補が一意なら探索路は要らないはずである（順序を決める必要が無い）。
   実測では `cmbx10.tfm` も `article.cls` も候補は 1〜2 件しかない。
3. **`ls-R` の道は `--var-value=TEXMFDBS` からも取れる**が、
   これも一回の起動なので 1 を実装するなら不要である。

**効き目の見積り：522 ms → 250 ms 前後。** 本家 `latex`（222 ms）とほぼ並ぶ。

---

## 5. 自前の `ls-R` 索引は 103 ms——**二番目に大きい**

上の表の差から、pratex 自身が索引に使っているのは **0.49G 命令、約 103 ms** である。

`ls-R` は 5,644,312 バイト、287,317 行、うち**項目行が 270,217**。

```text
103 ms ÷ 270,217 = 381 ns/項目
```

一項目あたり約 1,200 命令。**小さな名前を表へ入れるだけにしては重い。**

`parse_database` を読んだ。心当たりは四つ:

| | |
|---|---|
| `line.to_vec()` | **項目ごとに確保している**（27 万回） |
| `HashMap<OsString, _>` | 鍵も確保。`and_modify` の側では捨てている |
| 容量を予約していない | 27 万件まで**何度も組み直す**ことになる |
| 既定の SipHash | 短い鍵には重い |

**提案：**

1. **読んだ 5.6 MB の `Vec<u8>` を持ったまま、鍵を `(offset, len)` にする。**
   確保が零になる。`OsStr::from_bytes` は借りるだけで済む
2. **`HashMap::with_capacity(bytes.len() / 20)` で予約する**（27 万件なら 28 万程度）
3. **安い hasher を自前で書く。** FNV-1a なら二十行、safe Rust、依存も増えない

kpathsea は同じ 5.6 MB を（プロセス起動込みの 140 ms の中で）捌いている。
**そこまで詰められるはずである。** 見積りは 103 ms → 30 ms 程度。

---

## 6. 書式が本家の **7.5 倍**ある

**同じ言語一つ**で組んだ書式どうしで比べた。

```text
pratex  latex.fmt  16,833,063 bytes
pdftex  latex.fmt   2,235,930 bytes
```

読み込みそのものは `read` 一回で済んでいて、そこは良い。
それでも 16.8 MB を構造へ組み直す分は固定費に乗る。

**何がこれほど大きいのか、一度数えてみる価値がある。**
拡張レジスタの疎表、`kcatcode` 表、typed hash の逆引きあたりが候補だと思う。
（急ぎではない。4 と 5 の方が桁で大きい）

---

## 7. 新しい詰まり——**ドイツ語の綴りで落ちる**

`language.dat` を絞らずに素の TeX Live で組むと、ここで止まる。

```text
(dehyph-exptl: using a TeX engine with native UTF-8 support.
! Nonletter.
l.248 .buß
```

`dehyph-exptl` の判定はこうである:

```tex
\ifx\kanjiskip\undefined            % pTeX か？
  \def\testengine#1#2!{\def\secondarg{#2}}%
  \testengine χ!\relax              % χ は UTF-8 で 2 バイト
  \ifx\secondarg\empty               % 1 トークンなら UTF-8 エンジン
```

実測:

| | χ の判定 | `\kanjiskip` | 行き先 |
|---|---|---|---|
| **pratex** | UTF-8 | **無し** | UTF-8 分岐 → `ß` の各バイトが非文字で落ちる |
| e-upTeX | UTF-8 | **有り** | pTeX 分岐（8 ビット綴り）。**正しい** |
| XeTeX | UTF-8 | 無し | UTF-8 分岐。`ß` も 1 トークンなので通る |
| tex | 8 ビット | 無し | 8 ビット分岐 |

**pratex は upTeX の字句を持ちながら、upTeX の名乗りが無い。** そこに落ちている。

ギリシャ文字は wide トークンになる（upTeX と同じ）が、
`ß`（U+00DF）はバイトのままである（これも upTeX と同じ）。
なのに `\kanjiskip` が無いので、pTeX を知っている package が**間違った枝へ入る**。

**直し方は二つ。私は (a) を推す。**

- **(a) `\kanjiskip` を定義する**（pTeX 界面を名乗る）。
  字句が upTeX のものである以上、**upTeX として振る舞うのが筋が通っている**
- (b) Latin-1 補助も wide にする。**これは LaTeX の 8 ビット前提を壊す。**
  そちらが `for_CLAUDE.md` で「壊す可能性が高い」と書いていた道である

`language.dat` を `english hyphen.tex` の一行に絞れば回避できる。
今回の測定はそうした。

---

## 8. 試験が **Linux で 2 件落ちる**

そちらは Windows で「415 通過」と報告している。**Linux では 287 通過 2 失敗**である。

```text
file_search::wsl::tests::linux探索pathを順序と再帰記号を保ってwindows形式へ写す
file_search::wsl::tests::linux絶対pathをuncへ写してunicodeと空白を保つ
```

```text
期待  \\wsl.localhost\Ubuntu\usr\local\share\日本 語\latex.ltx
実際  \\wsl.localhost\Ubuntu\/usr/local/share/日本 語/latex.ltx
```

`linux_absolute_path_to_unc` が `PathBuf::push` で組み立てているためである。
**`push` の区切りは動かす OS で決まる**——Windows なら `\`、Linux なら `/`。

`mod wsl` は全 OS で建つ（使うのは `#[cfg(windows)]` の側だけ）ので、
**Linux では試験だけが落ちる。** 動作には影響しない。

ただし、**Windows の経路を作る関数が動く OS に依存している**のは筋が悪い。
`String` を自分で組んで `\` を明示するのが素直だと思う。

そちらの規律（`OsString` の境界、CRLF、UTF-8 断定を避ける）から見ても、
ここは同じ考え方が当てはまる場所である。

---

## 9. 権利——**穴が二つ**（性能外だが記録する）

### (a) `Cargo.toml` に `license` が無い

`LICENSE` は GPL-3.0 だが、manifest が何も宣言していない。
`authors` も無い。**権利者（tyti 氏）が manifest に残っていない。**

```toml
[package]
name = "rtex"
version = "0.1.0"
edition = "2021"
autobins = false
default-run = "pratex"
```

### (b) **Vaak の MIT 表示が同梱されていない**

rtex は Vaak を組み込んで配る。MIT は

> The above copyright notice and this permission notice shall be included in
> **all copies or substantial portions** of the Software.

と要求する。**組み込んだ実行ファイルはこれに当たる。**

いま Vaak の権利表示があるのは `AGENTS.md`（作業分担の文書）だけである。
これは**権利表示ではない**。

`THIRD-PARTY-LICENSES.md` を置いて、Vaak の `LICENSE` を**そのまま**入れるのが確実である。

```text
MIT License
Copyright (c) 2026 有村陽大 (Arimura Akihiro)
（以下 Vaak の LICENSE 全文）
```

**GPLv3 として配ること自体は正しい。** 足りないのは MIT 側の表示義務の方である。

### 良かったところ

- **`unsafe` は一つも無い**（`unsafe.tex` という試験用ファイル名だけが引っかかる）
- **依存は Vaak だけ。** crates.io から一つも引いていない。
  権利の確認範囲がこれ以上増えない
- PDF は `PDF Reference` から、`\pdfstrcmp` は pdfTeX 公式マニュアルから、
  `kcatcode` は `uptex-base` の公開文書と実機の黒箱から起こしている。
  **原実装を見ない規律が守られている**

---

## 10. まとめ——私が付ける優先順序

| | やること | 効き目 | 大きさ |
|---|---|---|---|
| 1 | `texmf.cnf` を自分で読み、`--show-path` の起動を消す | **522 → 250 ms** | 中 |
| 2 | `ls-R` の索引を詰める（確保零・容量予約・安い hasher） | **103 → 30 ms** | 小 |
| 3 | `\kanjiskip` を定義する | 素の TeX Live で全言語の書式が組める | 小 |
| 4 | `THIRD-PARTY-LICENSES.md` と `Cargo.toml` の `license` | 表示義務 | 極小 |
| 5 | `linux_absolute_path_to_unc` を OS 非依存にする | Linux で試験が通る | 極小 |
| 6 | 書式 16.8 MB の内訳を数える | 固定費 | 中 |

**1 と 2 で 522 ms → 180 ms 程度**になる見込みである。
そうなると本家 `latex`（222 ms）を**再び抜く。**

そして **1.3 ms の素の起動は、どの実装にも真似できない。**
そこは何があっても壊さないでほしい。


---

# 追記——**依頼者の設計方針**と、費用の完全な内訳

## 依頼者からの指示（2026-08-22）

> **設計判断のかなり上位に、upTeX にパフォーマンスで正面からタメを張れることがある。**

これは順序を決める指示である。私が上で付けた優先度も、これに合わせて**組み替える。**

そして——**その目標は届く。** 以下がその根拠である。

---

## 1. まず前提を潰しておく：kpathsea 互換層は**要らない**

「upTeX に勝つには kpathsea 互換層が要るのでは」という問いが出た。**要らない。**

`kpsewhich` の 140 ms が何なのかを分解した:

| 問い | 時間 |
|---|---:|
| `kpsewhich --version`（何もしない） | **2 ms** |
| `kpsewhich --var-value=TEXMF`（設定だけ） | 141 ms |
| `kpsewhich --show-path=tfm` | 138 ms |
| `kpsewhich cmr10.tfm` | 137 ms |
| `/bin/true`（下限） | 0 ms |

**プロセス起動は 2 ms である。** 残りの 137 ms は**全部 kpathsea の初期化**——
`texmf.cnf` を読み、5.6 MB の `ls-R` を索引する分である。

そして `pdftex -ini` は**空のファイルに対しても 5,695,062 バイトを読む**。
確かめた。**upTeX も pdfTeX も、毎回この 137 ms を払っている。**

一方 rtex は同じ `ls-R` を **103 ms** で索引している。

> **rtex は既に kpathsea より 25% 速く索引している。**

だから要るのは「kpathsea 互換」ではなく、**同じ二つのファイルを自分で読むこと**である。

| | |
|---|---|
| `texmf.cnf` | `key = value` と `$VAR` 展開だけ。公開仕様 |
| `ls-R` | 既に読めている |

`$SELFAUTOPARENT`・`!!` 接頭辞・`//` 再帰・format ごとの変数・
`texmf.cnf` の連鎖・`TEXMFCNF` の探索・program 別の上書き——
**この全部に付き合う必要は無い。** 必要な範囲だけでよい。

いま rtex は**同じ仕事を 3〜4 回**やっている。
自分で 1 回（103 ms）やり、子プロセスにも 2〜3 回（137 ms ずつ）やらせている。

---

## 2. 一頁の LaTeX 文書の**完全な内訳**

素の起動から一段ずつ足して測った（各 12 回）。

```text
pratex  書式なし・空ファイル           1 ms
pratex  ＋16.8 MB 書式               114 ms      ← 書式で +113 ms
pratex  ＋LaTeX 一頁                 141 ms      ← 組版で  +27 ms
pratex  ＋探索（現実の版方）          522 ms      ← 探索で +381 ms

pdftex  -ini・空ファイル              146 ms      ← kpathsea 137 ＋ INITEX 9
pdftex  ＋2.2 MB 書式＋LaTeX 一頁     196 ms      ← 書式と組版で +50 ms
```

表にすると:

| | pratex | pdftex |
|---|---:|---:|
| プロセス | **1 ms** | 2 ms |
| kpathsea 初期化 | — | **137 ms** |
| 自前の探索 | **381 ms** | — |
| 書式の読み込み | **113 ms** | 約 25 ms |
| 組版 | **27 ms** | 約 25 ms |
| **合計** | **522 ms** | **196 ms** |

### 読み方

**組版そのものは 27 ms 対 25 ms で、既に互角である。**
TeX82 の意味論を Rust で書き直した部分に、性能上の問題は無い。

負けているのは**二箇所だけ**である。

1. **探索 381 ms**（うち 280〜420 ms が子プロセス、103 ms が自前の索引）
2. **書式の読み込み 113 ms**（本家の 4.5 倍。書式の大きさは 7.5 倍）

**私は前の節で書式を六番目に置いた。あれは誤りである。**
組版が 27 ms しかないと分かった以上、**113 ms の書式は二番目に大きい。**

---

## 3. どこまで届くか

| 段階 | 一頁の時間 | 対 pdftex 196 ms |
|---|---:|---|
| いま | 522 ms | 2.7 倍遅い |
| ＋子プロセスを消す（`texmf.cnf` を自分で読む） | **244 ms** | 1.2 倍遅い |
| ＋書式を詰める（113 → 20 ms） | **151 ms** | **1.3 倍速い** |
| ＋索引を詰める（103 → 40 ms） | **88 ms** | **2.2 倍速い** |

**二段目で並び、三段目で抜く。**

そして探索が要らない場面（同じ文書を繰り返す、資材が手元にある、
ホストから短く呼ばれる）では:

```text
pratex   1 ms ＋ 書式
pdftex   137 ms ＋ 書式        ← どうやっても下がらない
```

**upTeX はこの 137 ms を落とせない。** rtex は落とせる。
**ここが構造的な勝ち筋である。**

---

## 4. 組み替えた優先順序

| | やること | 効き目 | 大きさ |
|---|---|---|---|
| **1** | **`texmf.cnf` を自分で読み、子プロセスを零にする** | **522 → 244 ms** | 中 |
| **2** | **書式 16.8 MB の内訳を数えて詰める** | **244 → 151 ms** | 中 |
| **3** | **`ls-R` の索引を詰める**（確保零・容量予約・安い hasher） | **151 → 88 ms** | 小 |
| 4 | `\kanjiskip` を定義する | 素の TeX Live で全言語が組める | 小 |
| 5 | `THIRD-PARTY-LICENSES.md` と `Cargo.toml` の `license` | 表示義務 | 極小 |
| 6 | `linux_absolute_path_to_unc` を OS 非依存に | Linux で試験が通る | 極小 |

**2 が上がった。** 組版が 27 ms しかない以上、書式の 113 ms は無視できない。

### 2 の手掛かり

同じ言語一つで組んで **16,833,063 対 2,235,930 バイト**である。
心当たりは:

- 拡張レジスタの疎表（0〜32767 の六種）
- `kcatcode` の 358 単位＋例外
- typed hash の逆引き表
- 制御綴の `Vec<u8>` を一つずつ持っていないか

**まず数えてほしい。** どの表が何バイト占めているかが分かれば、
削るべき場所はすぐ決まると思う。

読み込み自体は `read` 一回で済んでいて**そこは既に良い**ので、
残っているのは「バイト列から構造へ組み直す」側である。
**書式は自分で決められる形式**なのだから、
読んだバイト列をそのまま参照する形（写さずに借りる）にできる余地があるはずである。


---

# 性能の詰まりを `perf` で見た（2026-08-22 夜）

依頼者から「成果が不振なので詰まりを見てほしい」と頼まれた。
`codex/perf-wsl-euptex-safe` の `955318e` を建てて測った。

**Linux では `perf record` が使えるので、そちらで取りにくい「中のどこか」を出す。**

## いまの数字（Linux、i7-8650U、12 回、release LTO）

```text
書式なし・空          2 ms
書式あり・空        120 ms      ← 書式の復元で +118 ms
書式あり・一頁      152 ms      ← 組版で +32 ms
外を引く（一頁）    533 ms      ← 探索で +381 ms
```

**私が前回測った値からほとんど動いていない。**

## 内訳（`perf record`、探索ありの一頁）

| | 全体に占める割合 |
|---|---:|
| **`kpsewhich`（子プロセス）** | **55.5%** |
| pratex 自身 | 44.5% |

pratex 自身の中身:

| 記号 | 全体比 |
|---|---:|
| **`format::CountedLines::next`** | **10.4%** |
| `DefaultHasher::write`（SipHash） | 7.8% |
| `file_search::lsr::parse_database` | 2.4% |
| `input::Scanner::get_next` | 2.2% |
| `Vec<T>::undump` | 2.0% |
| `BuildHasher::hash_one` | 2.0% |
| `hashbrown::rustc_entry` | 1.4% |
| `input::macro_expand` | 1.5% |
| `hashbrown::reserve_rehash` | 1.1% |
| `Path::Components::next` | 1.3% |

**組版そのもの（`get_next` ＋ `macro_expand`）は 3.7% しかない。**

---

## 一番大きい構造的な問題——**書式がテキストである**

```bash
$ head -3 rlatex.fmt
0
256
0
```

**十進の整数が、一行に一つ。3,775,191 行、16.8 MB。**

書式だけを測った `perf` では **`CountedLines::next` が 47.4%** を占めた。
`str::Lines` は `SplitInclusive<char>` を通るので、**UTF-8 として一文字ずつ見ている。**

### どれだけ違うか

同じファイルを二通りで読んで測った（合成、release LTO）。

| | 時間 |
|---|---:|
| **いまの形**（UTF-8 検査 → 行分割 → 十進で読む） | **65.2 ms** |
| **二進**（`i32` を並べて `from_le_bytes`） | **1.3 ms** |
| ファイルを読むだけ | 7.3 ms |

> **50 倍である。**

118 ms の書式復元のうち、**約 65 ms が「テキストを整数にする」だけに消えている。**
残り約 45 ms が実際に表を組み立てる分、約 7 ms が読み込みである。

### 提案

**書式を二進にする。** 幅は欄ごとに分かっているのだから、`i32` で足りない欄だけ `i64` にすればよい
（実測では総和が `i32` の範囲を超える欄がある。全部 `i32` にはできない）。

- **118 ms → 50 ms 程度**が見込める
- ファイルも 16.8 MB → 14 MB 以下になる
- **零が 13.6%（513,281 行）ある**ので、疎な表を別に持てばさらに減る

書式は**自分で決められる形式**である。デバッグしやすさのためにテキストにしているなら、
`--dump-format=text` のような読み出し専用の道を別に残せばよいと思う。

---

## 二番目——**`ls-R` の索引が SipHash で回っている**

```text
DefaultHasher::write     7.8%
BuildHasher::hash_one    2.0%
hashbrown::rustc_entry   1.4%
hashbrown::reserve_rehash 1.1%
                        ─────
                        12.3%
```

`parse_database` 自体は 2.4% なので、**読み方ではなく表の作り方が重い。**

そちらの `docs/performance.md` の候補 3（「一行ごとの確保、HashMap の予約不足、
短い key の hash cost」）は**正しい。** 数字で裏づけがついた。

`reserve_rehash` が 1.1% 出ているので、**予約が効いていない**（27 万件まで何度も組み直している）。

三つとも小さい修正で済むはずである。

1. **容量を予約する。** `bytes.len() / 20` でよい
2. **鍵を `(offset, len)` にする。** 読んだ `Vec<u8>` を持ったまま借りれば、27 万回の確保が消える
3. **安い hasher を自前で書く。** FNV-1a なら二十行、safe Rust、依存も増えない

---

## 三番目——**`kpsewhich` は依然として 55%**

前回と同じである。**何を聞いても 140 ms** なので、回数を減らすしかない。

`texmf.cnf` を自分で読む案は、そちらが「曖昧・未対応な式は公開 `kpsewhich` へ戻す」と
線を引いているので、**その線のままで大半は取れる**と思う——
実際に必要なのは `--show-path=<format>` の答えだけで、
これは `texmf.cnf` の `TEXINPUTS` 等をそのまま読めば出る。曖昧な式に当たったときだけ
今までどおり外へ聞けばよい。

---

## 順序の提案

| | やること | 見込み |
|---|---|---|
| 1 | **書式を二進にする** | 118 → 50 ms |
| 2 | `ls-R` の索引（予約・借用鍵・安い hasher） | 探索の 103 ms → 30 ms 程度 |
| 3 | `--show-path` の外部起動を消す | 探索の 280 ms が消える |

**1 が一番大きくて、一番自分の裁量で決められる。**
`kpsewhich` と違って、書式の形式は誰とも互換を取らなくてよい。

そして 1 は**探索が要らない場面でも効く**——
依頼者が言っていた「TeX から短く呼ぶ」用途では、書式の復元が費用のほぼ全部である。

## 測り方

```bash
sudo sysctl -w kernel.perf_event_paranoid=2
perf record -q -F 3000 -o p.data -- pratex '&rlatex' small.tex
perf report -i p.data --stdio -q --sort comm        # 親と子を分ける
perf report -i p.data --stdio -q --comms pratex --sort symbol
```

**`perf stat` は既定で子プロセスも数える。** `--sort comm` で必ず分けること
（私は一度これで誤った数字を出した）。

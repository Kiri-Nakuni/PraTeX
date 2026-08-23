# 各地域・組版文化の文字サイズ単位

## 1. この文書の位置付け

この文書は、PraTeX の寸法単位を追加する前に行った一次資料調査である。
調査日は **2026-08-23**。単位名が同じでも、時代、地域、製造者、software によって
尺度が違う場合があるため、「ある国の単位」を一つの換算定数へ単純化しない。
初回の範囲は日本語、中国語、韓国語の組版慣行と、欧文DTPに共通するCSS/OpenTypeであり、
世界中の歴史的活字体系を網羅したという意味ではない。

結論として、少なくとも次の四種類を別の domain として扱う。

| 種類 | 例 | 値の決まり方 | PraTeX の接続先 |
|---|---|---|---|
| 連続的な物理単位 | `Q`、`H`、mm、DTP point | 他の物理単位への厳密な有理比 | `scan_units` の `PhysicalRational` |
| font・JFM・組方向依存量 | `em`、`ex`、`zw`、`zh`、CSS `ch` / `ic` | 選択中の metric と組方向から決まる | `scan_units` の `ContextMetric` |
| 離散的な文字サイズ preset | 中国語組版の初号、一号、五号等 | 名称から profile 固有のサイズを選ぶ | 寸法 scanner とは別の `FontSizePresetRegistry` |
| font file 内部座標 | OpenType の FUnits、JFM の design size 比 | font file の UPEM / design size と選択サイズから尺度化 | font/JFM loader と glyph metric 境界だけ |

`docs/extensible-dimension-units-roadmap.md` の `PhysicalRational` と
`ContextMetric` の二分類は、そのまま通常寸法 scanner に使える。後二者を第三、第四の
scanner 単位として足すのではなく、別の registry / loader domain として分離する。

## 2. 一次資料と版

すべて 2026-08-23 に取得した。live draft は、取得日を版の一部として扱う。

| 資料 | 参照した版 | 主な参照箇所 |
|---|---|---|
| [W3C JLReq](https://www.w3.org/TR/2020/NOTE-jlreq-20200811/) | W3C Working Group Note, 2020-08-11 | 2.2.4 の文字サイズに関する注、用語集 |
| [W3C CLReq](https://www.w3.org/International/clreq/) | live Editor's Draft, 2026-08-23 取得 | type area 設計時の中国語文字サイズ体系、用語集 |
| [W3C KLReq](https://www.w3.org/TR/2026/DNOTE-klreq-20260321/) | W3C Group Note Draft, 2026-03-21 | 7.4.1 Line Spacing |
| [CSS Values and Units Level 3](https://www.w3.org/TR/2024/CRD-css-values-3-20240322/) | Candidate Recommendation Draft, 2024-03-22 | 5.1.1、5.2 |
| [CSS Values and Units Level 4](https://www.w3.org/TR/2024/WD-css-values-4-20240312/) | Working Draft, 2024-03-12 | 6.1.1 |
| [pTeX manual](https://tug.ctan.org/info/ptex-manual/ptex-manual.pdf) | CTAN `ptex-manual` 2025-05-10 | 5.2 長さ単位 |
| [JFM file format](https://tug.ctan.org/info/ptex-manual/jfm.pdf) | CTAN `ptex-manual` 2025-05-10 | 1.4、1.5 |
| [OpenType 1.9.1 TrueType fundamentals](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01) | OpenType 1.9.1、page update 2024-05-30 | FUnits and the em square、Scaling a glyph |
| [OpenType 1.9.1 `head`](https://learn.microsoft.com/en-us/typography/opentype/spec/head) | OpenType 1.9.1、page update 2024-05-31 | `unitsPerEm` |

CLReq は dated Recommendation ではなく更新中の Editor's Draft である。その表を
PraTeX の不変な組込み値にすると、取得後の改訂を追跡できなくなる。将来 data table として
採用する場合も、source URL、取得日、table version を一緒に保存する。

## 3. 厳密に換算できる連続的な物理単位

### 3.1 日本の Q、H と二種類の point

[JLReq 2.2.4 の注](https://www.w3.org/TR/2020/NOTE-jlreq-20200811/) は、
日本の書籍で主に point と級（Q/q）が使われるとし、次の値を掲げる。

- JIS Z 8305 に基づく point の公表値は **1 point = 0.3514 mm**。
- 一方、application によっては **1 point = 1/72 inch ≒ 0.3528 mm** を使う。
- 写真植字由来の **1 Q = 0.25 mm**。
- 同じ出版社内でも複数体系を併用する例があり、一体系への統一は難しい。

したがって `point` という自然言語名だけでは厳密値を決められない。PraTeX の既存 `pt` は
TeX 互換値のまま変えず、1/72 inch の DTP/CSS point は既存 `bp` と意味を対応させる。
JIS 公表値 0.3514 mm を必要とする場合も、既存 `pt` の意味を変更せず、将来の明示名または
登録 table で `1757/5000 mm` として扱う。ここで `1757/5000` は公表された小数値を
有理数化したものであり、JIS 原文がそれ以上の桁を意図するとは推定しない。

[pTeX manual 5.2](https://tug.ctan.org/info/ptex-manual/ptex-manual.pdf) は Q と H をともに
**0.25 mm** とする。Q は文字の大きさの級数、H は字送り・行送りの歯数に由来する。
名前の用途は異なるが尺度は同じなので、PraTeX では二つの組込み名を同じ
`PhysicalRational` の決定箇所へ結ぶ。国際 inch を 25.4 mm とする尺度では次が厳密に成り立つ。

| 名称 | mm | inch | CSS/DTP point | TeX point での既存互換比 |
|---|---:|---:|---:|---:|
| 1 Q / 1 H / CSS 1Q | `1/4` | `5/508` | `90/127` | `7227/10160` |

最後の列は、既存 TeX の inch--point 互換比を用いた結果である。scanner は浮動小数点を介さず、
既存 `Q` / `H` の厳密な有理演算と scaled point への最終丸めを維持する。

### 3.2 CSS の絶対長

[CSS Values Level 3, 5.2](https://www.w3.org/TR/2024/CRD-css-values-3-20240322/#absolute-lengths)
は絶対長同士の関係を次のように固定する。

| CSS 単位 | 厳密な関係 | mm 換算 |
|---|---:|---:|
| `Q` | `1/40 cm` | `1/4 mm` |
| `in` | `2.54 cm` | `127/5 mm` |
| `pt` | `1/72 in` | `127/360 mm` |
| `pc` | `1/6 in` | `127/30 mm` |
| `px` | `1/96 in`（CSS 単位間の固定比） | `127/480 mm` |

[OpenType 1.9.1 TrueType fundamentals](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01)
も、digital typography では 1 inch がちょうど 72 points である一方、traditional typography
では 72.2752 points であると区別する。CSS `pt` / `pc` は前者の digital system であり、
CSS `pc = 12 CSS pt = 1/6 in` である。PraTeX の既存 `pt` / `pc` は TeX 互換体系、`bp` は
1/72 inch、`dd` / `cc` は TeX のlegacy互換値として
[`dimension.rs`](../src/dimension.rs) で既に意味が固定されている。
近い歴史値を見つけてもこれらを改定せず、CSS `pc` と TeX `pc` も同名だからと同一視しない。

ただし CSS は出力装置に応じて physical unit または reference pixel のどちらを anchor にするかを
選び、pixel anchor の装置では `in` 等が現実の物理寸法と一致しない場合があるとも規定する。
PraTeX の DVI/PDF 寸法に CSS device semantics を暗黙に導入しない。`px` を将来追加するなら、
「DVI/PDF 上の論理的 1/96 inch」という別契約を明示する必要がある。

CSS `Q` と日本の Q は、参照資料上どちらも 0.25 mm であり物理長は同じである。ただし
CSS の unit identifier は ASCII case-insensitive である一方、PraTeX は既存 TeX/pTeX の
token scanner と診断を保つ。CSS の大文字小文字規則を単位一つだけへ移植しない。

### 3.3 中国語 DTP point と「級」

[CLReq の中国語文字サイズ体系](https://www.w3.org/International/clreq/) は、
金属活字時代に「号」、写真植字時代に「級/Q」、DTP 時代に software の DTP point が
使われたと整理する。CLReq 用語集は DTP point を **1/72 international inch**
（約 0.35146 mm と記載されているが、1/72 inch の厳密換算は `127/360 mm`、
約 0.35278 mm）とする。本文と用語集の近似 mm 表示には不整合があるため、実装値は
同じ箇所に明記された 1/72 inch を優先し、近似小数を規範値にしない。

CLReq は写真植字の「級/Q」の歴史を述べるが、その箇所では 1級の厳密換算を定義していない。
日本の Q や CSS Q と名称が似ていることだけから 0.25 mm と推定しない。中国語 profile で
同じ尺度を採用するには、その profile が参照する標準・publisher 規約を別途記録する。

## 4. font・JFM・組方向に依存する ContextMetric

### 4.1 pTeX/JFM の `zw` と `zh`

[pTeX manual 5.2](https://tug.ctan.org/info/ptex-manual/ptex-manual.pdf) と
[JFM file format](https://tug.ctan.org/info/ptex-manual/jfm.pdf) によれば、概略は次のとおりである。

- `zw` は現在の和文 font の標準文字 class 0 の幅。
- `zh` は同 class の高さと深さの和。
- したがって `zw == zh` は一般には保証されず、JFM と選択 font により値が変わる。
- JFM の glue/kern 等は design size に対する固定小数比として記録され、選択サイズで尺度化される。

`zw` / `zh` は物理単位ではなく `ContextMetric` である。横組・縦組の現在 font/JFM を
選ぶ規則を `scan_units` の外へ複製しない。JFM 未接続時の `em` 代用は移行状態であり、
JFM 接続後は class 0 metric へ一箇所で差し替える。

### 4.2 CSS の font-relative unit

[CSS Values Level 4, 6.1.1](https://www.w3.org/TR/2024/WD-css-values-4-20240312/#font-relative-lengths)
は font-relative unit を次のように区別する。

| 単位 | CSS での基準 | 不足時の主な fallback | PraTeX 候補 |
|---|---|---|---|
| `em` | element の font size | 常に定義 | 実装済み `ContextMetric` |
| `ex` | first available font の x-height | `0.5em` | 実装済み。font metric 接続を監査 |
| `cap` | cap-height | font ascent | OTF metric 接続後の候補 |
| `ch` | U+0030 の inline-axis advance | 通常 `0.5em`、縦で upright なら `1em` | 組方向を含む候補 |
| `ic` | U+6C34「水」の CJK advance | `1em` | JFM/OTF 共通境界の有力候補 |
| `lh` | computed line height | font metric だけの `normal` | 再帰を避ける仕様決定後の候補 |

CSS は `ch` の advance が writing mode、text orientation、font setting、glyph selection に
依存するとし、font-relative unit は shaping なしで計算すると規定する。PraTeX で互換名を
追加する場合も、RustyBuzz の有無で単位値を変えず、font/JFM の unshaped advance を使う。

`ic` と `zw` は多くの全角 font で同値になり得るが、定義は同じではない。`ic` は U+6C34 の
advance（取得不能なら 1em）、`zw` は JFM class 0 の幅である。alias にせず別の metric key とし、
結果が一致する場合だけ同じ host-owned metric value を参照させる。

`lh` は `\baselineskip` 等、自身を寸法走査する設定から参照されると循環し得る。TeX には
CSS の element/root model もないため、`rem`、root 系、viewport 系を名前だけ模倣しない。

### 4.3 KLReq に現れる `ch` と `gp`

[KLReq 7.4.1](https://www.w3.org/TR/2026/DNOTE-klreq-20260321/)
は韓国語組版の行送り・行間・最小行間に、point (`pt`)、mm、cm、pica (`pi`)、pixel (`px`)、
character (`ch`)、Geop/Geup (`gp`)、inch 等を使う例を挙げ、文字サイズに対する百分率も挙げる。

しかし同節は `gp` の換算値を示さず、line spacing について未解決の Issue 13 を残す。
また英語本文が `character (ch)` と書く `ch` を、CSS の U+0030 advance と同じ意味だとは
定義していない。この資料だけから次を行ってはならない。

- `1gp = 0.25mm` 等の値を推定して組込み `PhysicalRational` にする。
- KLReq の `ch` を CSS `ch` の alias にする。
- `pi` を既存 TeX `pc` と同じ token spelling に変更する。

韓国の `gp` を追加する前に、換算と適用範囲を定める韓国の公的標準または版元仕様を取得し、
地域、版、物理基準を記録する。それまでは unresolved candidate とする。

## 5. 離散的な号数は scanner 単位ではない

### 5.1 CLReq の参考表

[CLReq](https://www.w3.org/International/clreq/) は、歴史的に foundry ごとの
「号」が標準化されず、英米式、欧州大陸式、DTP 等の point system の違いによって換算が
複数あると明記する。次の表は CLReq が掲げる**非規範的な参考値**である。

| 号数 | CLReq の参考 point 値 | 不確実性 |
|---|---:|---|
| 初号 | 42 pt | point system / foundry profile 未指定 |
| 一号 | 27.5 pt または 28 pt | 二値 |
| 小（新）一号 | 24 pt | profile 未指定 |
| 二号 | 21 pt または 22 pt | 二値 |
| 小（新）二号 | 18 pt | profile 未指定 |
| 三号 | 15.75 pt または 16 pt | 二値 |
| 四号 | 13.75 pt または 14 pt | 二値 |
| 小（新）四号 | 12 pt | profile 未指定 |
| 五号 | 10.5 pt | profile 未指定 |
| 小（新）五号 | 9 pt | profile 未指定 |
| 六号 | 7.875 pt または 8 pt | 二値 |
| 七号 | 5.25 pt | profile 未指定 |

これは `1号` を基準に任意の数値を掛ける線形単位ではない。番号の増減と寸法の関係も
線形でなく、「小」「新」を含む名前が選択肢を表す。さらに表の `pt` 自体がどの point system
かを profile なしには一意にできない。従って `12hao` のような `scan_dimen` 単位にしない。

### 5.2 `FontSizePresetRegistry` 案

寸法単位 registry とは別に、将来次の宣言的 table を持つ。

- host が割り当てる `PresetProfileId`。`LanguageRegion` や TeX `language` と同一 ID にしない。
- profile の地域、publisher/foundry、point system、source URL、版、取得日。
- 正規化済み preset 名と別名。簡体字・繁体字名は表示名であり内部 enum 値にしない。
- profile が選び終えた一つの厳密な `PhysicalRational` または concrete `Dimension`。
- 元資料が approximate / alternative / non-normative かを保持する provenance flag。

CLReq の参考表そのものは選択肢を含むため、PraTeX の既定 profile として登録しない。
利用者または版元が point system と各二値を選んだ table だけを原子的に検証して公開する。
Vaak/WASM から登録する場合も一 preset ごとの callback にせず、table 全体を host-owned data へ
compile する。

preset の利用は font-size 選択 API または名前付き寸法の取得で一度だけ concrete dimension へ
解決する。一般の `scan_dimen` hot path に locale 判定や文字列 lookup を入れず、
`true` / `\mag` / overflow の意味も preset 側で再実装しない。

## 6. OpenType FUnits と JFM design unit は外部単位ではない

[OpenType 1.9.1 TrueType fundamentals](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01)
は FUnit を em square 内の最小座標単位とし、point size に依存しない相対座標として定義する。
glyph の pixel 座標への尺度化は概念的に次である。

```text
pixel_coordinate = em_coordinate * ppem / units_per_em
```

[OpenType `head`](https://learn.microsoft.com/en-us/typography/opentype/spec/head) は
`unitsPerEm` を font file ごとの 16--16384 の値とする。従って 1 FUnit は font を選ぶ前には
物理長を持たず、同じ font でも選択 font size によって長さが変わる。

FUnits を `fu` のような TeX scanner 単位として公開しない。font loader が
`coordinate / unitsPerEm * selected_size` を厳密な整数・有理演算で評価し、glyph advance、
bounding box、anchor 等の host-owned metric へ変換する。rounding と overflow は loader 境界の
一箇所で決める。

JFM の design size 比も同様である。ただし OpenType FUnits と JFM の fixed-word表現は
file format も意味も異なるため、一つの wire ID や raw integer domain に潰さない。
共通化するのは、尺度化後の glyph/JFM metric を safe Rust の host-owned dimension として
line breaking、packing、DVI/PDFへ渡す境界だけである。

## 7. PraTeX への採用候補

| 候補 | 分類 | 優先度 | 採用条件 |
|---|---|---:|---|
| `Q` / `H` | `PhysicalRational` | 組込み済みを維持 | 0.25 mm の同じ中央実装、互換試験 |
| TeX `pt` / `pc` / `dd` / `cc` | legacy `PhysicalRational` | 現状維持 | 現代の地域標準を理由に換算を変えない |
| JIS 公表 point | `PhysicalRational` | 低 | TeX `pt` を変えない明示名と需要を先に確定 |
| DTP/CSS point | `PhysicalRational` | 既存 `bp` で充足 | `pt` alias にしない |
| CSS pica | `PhysicalRational` | 同名追加不可 | TeX `pc` と区別し、必要なら明示APIで表す |
| CSS `ic` | `ContextMetric` | JFM/OTF metric 接続後の有力候補 | U+6C34 advance、1em fallback、組方向試験 |
| CSS `ch` / `cap` | `ContextMetric` | 次点 | font fallback と縦組を仕様化 |
| CSS `lh` | `ContextMetric` | 保留 | 自己参照・group・現在行送りの snapshot 規則を決定 |
| 中国語の号数 | `FontSizePreset` | registry 設計対象 | publisher/profile が一意の値を選択 |
| 韓国 `gp` | 未解決 | 保留 | 公的・公式一次資料で厳密値と適用範囲を取得 |
| OpenType FUnits | font internal | scanner 採用不可 | loader 内だけで尺度化 |

## 8. 実装時の検証項目

1. 既存 `pt`、`bp`、`Q`、`H` の意味、丸め、空白消費、`true` と `\mag` を変えない。
2. `Q` / `H` は同じ尺度を参照しつつ、diagnostic 上の入力名を保持する。
3. `ContextMetric` の key に font/JFM generation、writing mode、必要な orientation を含める。
4. `ic` / `ch` は RustyBuzz 有効・無効で変わらない unshaped metric とする。
5. preset profile は曖昧値を受理せず、全件成功時だけ原子的に登録する。
6. live draft 由来 data は URL、取得日、版を失わず、fmt に run-local handle を保存しない。
7. 空 registry の寸法走査で allocation、Vaak/WASM call、locale lookup を増やさない。
8. 単位/preset追加前後で DVI 意味を固定し、upLaTeX 比 1.2 未満の全体性能 gate に加えて
   `scan_dimen` microbenchmark と font-size table lookup を別々に測る。

## 9. 既存 roadmap との整合

`docs/extensible-dimension-units-roadmap.md` に対する結論は次のとおりである。

- `PhysicalRational` / `ContextMetric` の区別、中央 `scan_units`、host 側丸めを維持する。
- Q/H、CSS系の厳密物理比を浮動小数点にしない。
- `ic` 等を追加しても provider callback を hot path に置かず、登録時に metric key へcompileする。
- 中国語号数は第三の `UnitKind` にせず、別の `FontSizePresetRegistry` とする。
- FUnits/JFM raw value は registryにもWASM ABIにも露出せず、loader内部で尺度化する。
- 韓国 `gp` のように一次資料で換算が確定しない名称を「よく知られた値」で埋めない。

この分離により、国・言語名から寸法を推測せずに各文化の慣行を追加でき、通常の TeX 寸法走査と
JFM/TFM基線の性能・互換性も保てる。

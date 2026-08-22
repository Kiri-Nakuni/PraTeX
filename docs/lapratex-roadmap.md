# LaPraTeX ロードマップ

## 1. 目的

PraTeX 上に、公開 interface と許可された black-box 観測から独立に実装する format を作る。
名称は **LaPraTeX** とし、公式 LaTeX2e の source を改変・翻訳した format とはしない。

LaPraTeX は将来計画である。現在、`lapratex.ltx` と `lapratex.fmt` は repository に存在せず、
LaPraTeX format は実装済みではない。

## 2. 二つの track を分離する

### 2.1 公式 LaTeX 互換 track

公式の unmodified `latex.ltx` と、その公式依存物を外部資材として PraTeX へ入力し、
既存 package と engine primitive の互換性を測る track である。

- 公式資材は試験時にだけ取得し、repository へ vendor しない。
- version、取得 URL、取得日時、hash、license を記録する。
- `latex.ltx` は変更せず、実装資料として読まない。
- source 本文や source 行を実装側へ渡さず、oracle runner が opaque input として扱う。
- 生成した公式 `latex.fmt` は一時試験物であり、LaPraTeX として配布しない。
- 成功・失敗は、engine が公式 format を処理できるかという互換性の指標にだけ使う。

現在の外部実測では、必要な CTAN 資材を一時環境へ揃えた場合に公式 `latex.ltx` から
format を生成できている。ただし、これは LaPraTeX が存在することも、LaTeX2e 全体との
互換性を保証することも意味しない。

### 2.2 LaPraTeX clean-room track

PraTeX 用の独立 format を一から作る track である。

- 実装名は `lapratex.ltx`、生成物は `lapratex.fmt` とする。
- macro `\fmtname` は展開すると必ず `LaPraTeX` になるよう定義する。
- version と support の責任主体を LaPraTeX 自身として表示する。
- 公式 `latex.ltx` を patch、整形、機械翻訳、再構成して作らない。
- 公式 format を dump/decompile して内部 macro 定義を得ない。
- 公開 interface の互換範囲と、PraTeX 固有機能を別々に宣言する。

二つの track は試験 harness を共有してよいが、source、format、生成規則を共有しない。

## 3. clean-room 資料境界

### 3.1 使用してよい資料

- LaTeX Project が公開する author 向け interface 文書
- class/package writer 向けの公開 interface 文書
- font selection、font encoding、hook の公開 interface 文書
- L3 programming layer の公開 interface 文書
- TeX、e-TeX、pdfTeX の公開 manual
- 公開 interface だけから作った独立試験入力
- 公式 unmodified binary/format を実行した、下記 schema に制限した black-box 結果

公開文書に source への link があっても、interface 記述と実装 source を同一視しない。

### 3.2 使用しない資料

次を LaPraTeX の仕様抽出、設計、実装、comment、試験期待値の作成に使わない。

- `latex.ltx`
- LaTeX の `.dtx` と `.ins`
- `source2e` と、その生成 PDF
- `article` `report` `book` など標準 classes の source または source commentary
- l3kernel / expl3 の実装 source と実装を列挙する source 文書
- 公式 `latex.fmt` その他の format の dump、decompile、macro table 抽出
- 上記 source の翻訳、要約、機械変換、別言語への移植

oracle runner の担当者が運用上 `latex.ltx` を取得しても、その内容を読んで実装者へ伝えない。

### 3.3 許可する black-box schema

oracle から clean-room 実装側へ渡す値を、あらかじめ次へ制限する。

- 試験 ID と、公開 interface から独立に作った入力
- engine 名、version、option、format/assets の hash
- 終了種別と、試験 harness が定義した構造化 diagnostic code
- box の width/height/depth、register 値、page 数
- 正規化した DVI opcode、PDF object の種類、render 比較値
- 複数回実行での aux/toc/ref の収束有無

error log に source 行や macro replacement text が含まれる場合はそのまま渡さない。
診断全文、内部 control sequence 一覧、format memory dump は clean-room oracle にしない。

## 4. product identity

LaPraTeX は LaTeX2e を名乗らない。

| 項目 | LaPraTeX |
|---|---|
| source entry | `lapratex.ltx` |
| format | `lapratex.fmt` |
| `\fmtname` | `LaPraTeX` |
| version | LaPraTeX 独自の公開 version |
| 最初の標準 class | 独自名、例 `laparticle` |
| engine capability | `\pratex...` 系の明示照会を別途設計する |

package の `\NeedsTeXFormat{LaTeX2e}` を通すために `\fmtname=LaTeX2e` と偽らない。
将来の互換 layer は、対応 interface version と capability を明示する。
`\fmtname` の文字列を直接比較する package は、対応 tier 上の既知制限になり得る。

## 5. architecture の接点

LaPraTeX は engine の次の層を利用するが、それぞれの未実装機能を format macro で
隠して実装済みと称さない。

- PraTeX 独自 fmt の load/dump
- e-TeX と pdfTeX 互換 primitive
- 論理名と物理 path を分離した file resolver
- DVI と直接 PDF backend
- catcode/kcatcode、Unicode token、character classifier
- namespace
- 将来の JFM、和文 node、行分割、縦組
- 将来の拡張可能な寸法単位、Vaak table、WASM ABI

公式 LaTeX 互換 track で不足 primitive が見つかった場合は engine 層へ実装し、
LaPraTeX だけの代替 macro として二重実装しない。

## 6. 段階 L0--L8

### L0: charter、provenance、oracle

- 使用可能資料と禁止資料を manifest 化する。
- black-box schema と oracle runner の出力 filter を固定する。
- LaPraTeX の名称、version 規則、support 表示、license notice を決める。
- このGPL-3.0 repositoryに入れるLaPraTeX sourceは、別の権利審査と配布単位を
  定めない限りGPL-3.0とし、後から曖昧に切り離さない。
- 公式 LaTeX 互換試験と LaPraTeX 試験の資材 directory/cache を分ける。

完了条件は、実装者が禁止 source を見なくても試験目的と期待値を説明できることである。

### L1: bootstrap format

- 独立した `lapratex.ltx` を init mode で読み、`lapratex.fmt` を生成する。
- `\def\fmtname{LaPraTeX}`相当のmacro定義と独自 version を表示する。
- allocation、基本 error/file lifecycle、format dump/reload を整える。
- 最小の `document` 相当を開始・終了できるようにする。

公式 latex-base が存在せず、network も無い clean tree で build と reload が成功することを
完了条件にする。

### L2: author-level core

- command と environment の定義
- counter、length、list、section
- box と基本 output routine
- label、reference、aux file
- 二回以上の実行での参照収束

この段階では公式 LaTeX 内部 macro 名の網羅を目標にしない。公開 author interface の
選択 subset と、LaPraTeX 固有 interface を文書化する。

### L3: class/package public interface

- package/class の提供・要求・読込み
- option 処理
- 公開 hook と file hook
- interface version/capability 判定
- 独立した最小 class、例えば `laparticle`

公式 `article.cls` の置換を最初の成功条件にせず、独自 class 上で public contract を固定する。

### L4: font、math、output

- 公開された font selection / encoding interface の独立 subset
- 基本的な text font と math font
- 基本 math environment
- DVI と PraTeX 直接 PDF の backend 境界
- PDF text extraction と、将来の ToUnicode の検証点

font file、encoding、map の探索は resolver を通し、format 内へ物理 path を固定しない。

### L5: 公開 L3 programming interface

実利用で必要な公開 L3 interface を小さな単位で選び、独立実装する。
l3kernel / expl3 の実装 source や source documentation は参照しない。

一つの module ごとに、公開 contract、異常系、expansion 性質、grouping、性能予算を先に試験へ
固定する。公式 expl3 は外部互換 track の opaque oracle にだけ置く。

### L6: PraTeX-native 拡張

- Unicode character classifier
- catcode/kcatcode 統合実験を利用する native profile
- namespace
- JFM と横組み和文
- 任意寸法単位の宣言的 interface
- 明示的 Vaak capability と、将来の WASM capability

この profile では公式 `latex.ltx` の byte-oriented な前提を成功条件にしない。
通常の公式 LaTeX 互換 profile を残したまま別 mode/format として進める。

### L7: package compatibility tiers

package ごとに互換性を宣言し、成功例だけで format 全体の互換性を主張しない。

| tier | 対象 | 主な条件 |
|---|---|---|
| P0 | 純粋な document macro | L2 の公開 author interface だけを使う |
| P1 | class/package public API | option、hook、protected expansion、file lifecycle を使う |
| P2 | file/font resource | resolver、TFM/JFM、encoding、map、複数回実行を使う |
| P3 | backend/PDF | driver 判定、link、annotation、PDF object を使う |
| P4 | engine-specific/CJK | upTeX/LuaTeX 等の固有 interface、和文組版、方向を使う |

初期の native class と、公開 API を使う小さな package corpus を先に通す。
`hyperref`、`jsarticle`、`jlreq` などは、それぞれの license と依存 interface を調べた上で
個別 target にする。

### L8: reproducible distribution

- `lapratex.ltx`、生成手順、format version、asset lock、provenance を配布単位にする。
- 公式 latex-base 無しで build できることを release test にする。
- network 無しで lock 済み cache から同じ試験を再現する。
- format に provider handle、physical path、一時 directory、clock 依存値を保存しない。
- interface の追加、変更、廃止規則と compatibility matrix を公開する。

## 7. CTAN 資材

公式 LaTeX 互換試験や package tier 試験で CTAN 資材が必要な場合、実行時または
明示した準備 step で公式配布元から取得する。

- repository へ archive、展開物、生成 fmt を vendor しない。
- package 名、version、公式 URL、license、archive hash を lock/provenance に記録する。
- temp または content-addressed cache へ展開し、試験終了後も source tree と混ぜない。
- hash 不一致、license 不明、複数候補、取得不能を黙って別資材へ fallback しない。
- offline 試験は lock 済み cache が無ければ明示的に skip/fail する。
- oracle 用資材を LaPraTeX 実装 source の生成入力にしない。

## 8. license と clean-room 判定

公式 latex-base は LPPL 1.3c or later で配布されている。LPPL は Work の実行自体と、
unmodified Work、Compiled Work、Derived Work の配布を別に扱う。外部 oracle として
実行できることから、source を LaPraTeX へコピーできるとは判断しない。

次のいずれかを行った時点で、その実装はこの文書でいう clean-room ではない。

- LPPL source の一行または実質的部分をコピーする。
- 変数名だけ変える、翻訳する、別言語へ移植する。
- source または decompiled fmt から制御構造や macro 本文を再構成する。
- 生成物へ source の実質的部分を埋め込む。

その場合は作業を止め、LPPL の変更・配布条件、PraTeX の GPL-3.0 との関係、表示名、
support 責任、原版取得情報、Compiled Work の配布条件について法務 review を受ける。
「別 format 名にした」「GPL として配る」だけでは LPPL 上の義務が消えると仮定しない。

この節は project の実装規律であり、個別の法的判断を置き換えるものではない。

## 9. 検証

### 9.1 LaPraTeX 自体

- 公式 latex-base が無い clean environment での format build/dump/reload
- banner、`\fmtname`、version、support 表示
- group/global、protected expansion、引数、error recovery
- counter/length/list/section/box/output routine
- aux/toc/ref の複数回収束
- DVI opcode、box 寸法、page 数の構造比較
- PDF parse、render、text extraction
- Unicode、catcode/kcatcode、namespace、JFM の profile 別試験
- source/provenance allowlist と、禁止 filename/content の混入検査

### 9.2 公式互換 track

- 取得した unmodified asset の version と hash
- 同じ公開入力を公式 engine と PraTeX へ与えた構造化結果
- 未定義 primitive、file resolution、font/backend 差を分類した compatibility report
- source 行を含まない sanitized oracle artifact

比較は、宣言した public interface subset にだけ適用する。LaPraTeX 固有 profile へ
公式 LaTeX と同一の診断文、内部 macro 名、byte 単位の字句前提を要求しない。

## 10. 一次資料

- [LaTeX kernel / latex-base（CTAN）](https://ctan.org/tex-archive/macros/latex/base?lang=en)
- [LaTeX Project Public License](https://www.latex-project.org/lppl/)
- [LaTeX Project: Documentation](https://www.latex-project.org/help/documentation/)
- [e-TeX package and manual（CTAN）](https://ctan.org/pkg/etex)
- [pdfTeX package and manual（CTAN）](https://ctan.org/pkg/pdftex)

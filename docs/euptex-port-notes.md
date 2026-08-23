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

## 3. UTF-8 基底にできるか — **入力は UTF-8、token は Unicode 1文字**

依頼の指定は「可能なら UTF-8 基底」。これは**入力バッファを UTF-8 のまま持つ**ことと、
**UTF-8 の各バイトを別々の TeX token にする**ことを区別しなければならない。

後者では upTeX 互換にならない。upTeX の公開仕様では、CJK token は
`kcatcode` 5 bit と文字コード 24 bit の組であり、入力時に決まった `kcatcode` を
token 自身が保持する。同じコード点でも、途中で `\kcatcode` を変更すれば前後で異なる
token になる。したがって UTF-8 の後続バイトを `OtherChar` として通し、後段で束ねる設計は
採用しない。

rtex では UTF-8 を一度コード点として認識し、その時点の `kcatcode` で経路を分ける。

- ASCII は現在の `u8` token と catcode 表を通す。頻出経路に Unicode 表引きを増やさない。
- `15`（`not_cjk`）は従来どおり UTF-8 の各 byte を欧文 TeX の入力へ渡す。
  `inputenc` と8 bit欧文の互換経路であり、ここを一文字一tokenに変えない。
- `14`（`latin_ucs`）は U+2E7F 以下を Unicode の欧文一文字 token にする。
  この段階には 16 bit の catcode 表も必要なので、CJK token とは分けて導入する。
- `16`〜`20` は CJK token 一つを生成し、文字コードとその時点の `kcatcode` を保持する。
  token 化済みの値は後の `\kcatcode` 変更で変えない。

`char` は正規の Unicode scalar だけなので、upTeX が扱う surrogate 値や将来の
24 bit 内部コードまで一つの型に押し込めない。文字コードの格納には範囲を検査した `u32` を使う。
JFM の文字種、文字 node、DVI の `set2` / `set3` も後続段階でこの一文字単位へ揃える。

### 入力UTF-8はRustのscalar復号ではない

TeX Live 2026の公式e-upTeXを、原実装や上流試験を見ずにraw byte入力で黒箱照合した。
現行処理系はUnicode scalarだけを受理するdecoderではない。2 byteは `C2..DF`、3 byteは
`E0..EF`、4 byteは `F0..F4` をleadとし、必要な個数の `80..BF` が続けば値を組み立てる。
3/4 byteではoverlongとsurrogateも一文字tokenになり、組み立てた0はbyte列へ戻る。
4 byteの上限はU+10FFFEであり、U+10FFFFとそれを超える値はbyte列へ戻る。不正lead、
不足、不正continuationも先頭1 byteだけを欧文経路へ戻し、後続から再同期する。

したがって `std::str::from_utf8` やlossyなU+FFFD置換は使わない。専用のbounded decoderで
2〜4 byteだけを検査し、失敗時にも必ず1 byte進める。`kcatcode=15` の正しい並びと
`^^e3^^81^^82` も再結合せず、各byteを現在の8 bit catcode表へ渡す。ASCIIではdecoderも
`kcatcode`表引きも呼ばず、現在の頻出経路を保つ。

制御綴名の同一性もraw UTF-8 byte列だけでは表せない。Unicode一文字として読んだ「あ」と、
`kcatcode=15` で `E3 81 82` の三byteとして読んだ見た目上同じ名前は、公式処理系では別の
制御綴になる。名前の鍵は `Byte(u8)` と `Unicode(u32)` を区別し、CJK category自体は
tokenにだけ固定する。同じ符号位置をcategory 16〜20のどれで読んでも制御綴の同一性は変えない。

段4bでは `CjkToken` を符号位置24 bitとcategory 5 bitの一語に詰め、macro、`\edef`、
`\let`、条件分岐、delimiter、fmt往復まで同じtokenを渡す。typed制御綴のhashは通常の
byte名と分離し、実際にwide名を作ったときだけ疎な表を確保する。ASCIIのtokenと制御綴は
従来表をそのまま通る。表示時はUTF-8を一文字単位でPrinterへ渡すため、継続byteを
`newlinechar` と取り違えず、行折返しやエラー文脈の切詰めも文字途中で分断しない。
不正入力だけはlexerと同じく先頭1 byteを `^^hh` に戻し、次のbyteから再同期する。

`kcatcode` 表は変更可能な値を block ごとに持ち、コード点から block を二分探索する。
`HashMap<u32, _>` を文字ごとに引く設計にはせず、ASCII 経路にも費用を持たせない。

### `kcatcode` のクリーンルーム根拠

データ表と振る舞いは、次の公開文書だけから起こす。upTeX 本体の change file、C実装、
上流の回帰試験は参照しない。

- `texjporg/uptex-base` の
  [`01uptex_doc_utf8.txt`](https://github.com/texjporg/uptex-base/blob/master/01uptex_doc_utf8.txt)、
  2026-02-15 Ver2.02。配布全体は BSD-3-Clause。
- Unicode Consortium の
  [`Blocks-17.0.0.txt`](https://www.unicode.org/Public/17.0.0/ucd/Blocks.txt)、
  2025-08-01。
- 日本語TeX開発コミュニティの
  [pTeX manual](https://texdoc.org/serve/ptex-manual.pdf/0) にある upTeX の入力処理と
  `kcatcode` の説明。

Ver2.02 の既定表では、U+0080 以上は原則 `18` で、列挙された block と7つの例外集合だけが
別値になる。Basic Latin は `15` であり、`14` の既定 block は無い。`14` は利用者が明示設定する
`latin_ucs` の値として実装する。最初のデータ層は U+0000..U+10FFFF に限定し、upTeX 独自の
0x110000 以上の内部コードは対応済みと偽らず、後続段階に残す。

文書中では CJK Extension F の範囲が Extension I を内包しているため、TeX Live 2026 の
e-upTeX `p4.1.2-u2.02` を INITEX として黒箱照合した。実装ソースや上流試験は参照していない。
照合物は CTAN tlnet の `uptex.windows` revision 78020（2026-08-22取得、archive SHA-256
`c878983da002f32a24a507680ccf00261a3761089ed324892668ded589bf9c0d`）で、一時領域からのみ
実行した。
結果は**固定された開始境界から次の開始境界まで**が代入単位だった。Unicode 17.0.0 の
named block 346個に加え、公開文書の block 番号と一致する12個の擬似境界も持つ。
追加境界は U+33480、U+40000..U+D0000 の各面先頭、U+E01F0 であり、通常表は358単位になる。
U+33480..U+3FFFF は既定 `16` の独立単位、U+40000..U+DFFFF は各面ごとの既定 `18`、
U+E0000..U+E00FF、U+E0100..U+E01EF、U+E01F0..U+EFFFF もそれぞれ独立する。通常の
named block 間にある `No_Block` gap は直前の開始境界へ属する。

F は U+2CEB0..U+2EBEF、I は U+2EBF0..U+2F7FF であり、U+2EE60 から代入しても I 全体が
変わる。7例外集合だけは通常境界より先に判定し、非連続範囲を含めて各通し番号ごとに値と
level を共有する。したがって U+0000..U+10FFFF の実装上の総代入単位は365個である。
境界照合は代表点だけでなく、357個の非零開始境界と358区間の末尾をすべて局所代入で
検査した。さらに named block の `end + 1` に生じる51個の gap 候補を全検査し、追加境界が
U+33480 と U+E01F0 の2個だけであること、残る49個（U+E0080を含む）が直前単位に属する
ことを確認した。各面先頭10個と合わせて追加境界は12個である。

同じ照合により、`14` は対象文字コードが U+2E7F 以下のときだけ代入できることも確認した。
それより上の `14`、および範囲外の値は診断後に `16` へ置き換えて代入される。診断文は
U+2E7F 以下でも `15..20` と表示される。この回復も回帰試験へ固定する。

## 4. 段取り

**一段ずつ、それぞれ独立に価値がある。**

| 段 | もの | 状態 |
|---|---|---|
| **0** | **和文の寸法単位 `Q` `H` `zw` `zh`** | **済**（枝 `jdimen`、試験 7 本） |
| 1a | e-TeX の**式**（`\numexpr` `\dimexpr` `\glueexpr` `\muexpr`） | **済**（枝 `etex-expr`、試験 12 本） |
| 1b | 疎レジスタ（0–32767） | **済**（低位密＋高位疎、6種、挿入番号は別型） |
| 1c | e-TeX のmark class（0–32767） | **済**（class 0は従来状態、非0は疎表、pageと`\vsplit`） |
| 1d | e-TeX の糊成分問い合わせと型変換 | **済**（伸縮の係数と次数、`\mutoglue` / `\gluetomu`、通常糊・数式糊・式・fmt） |
| 2 | e-TeX の**字句系** | **一部済**（`\detokenize` `\unexpanded` `\readline` `\protected` `\everyeof`。`\scantokens` は未） |
| 3 | e-TeX の**内省** | **一部済**（`\currentgroup*` `\currentif*` `\lastnodetype` `\iffontchar`。`\fontchar*` `\showgroups` `\showtokens` 等は未） |
| 4a | **Unicode block 分類表と `\kcatcode`**（代入・group・fmtまで） | **済**（U+10FFFFまで） |
| 4b | **UTF-8 字句解析と CJK文字 token**（`16`〜`20`を一文字一token、分類をtokenへ固定） | **済**（typed制御綴、条件・展開・表示・fmtまで。JFM/文字nodeは後段） |
| 4c | **Unicode欧文 token とpattern alphabet**（`14`を一文字一token、cat/lc/uc/sf・case・active・fmt・hyphen trieまで） | **済**（U+0080〜U+2E7F。wide font nodeとnamespaced Unicode active生成は後段） |
| 5 | **JFM**（和文フォントの寸法表）と `\jfont` | **進行中**（公開仕様だけによるbounded reader、横/縦・24-bit code・現行glue/kern拡張・直接class対表まで。font選択とscale接続は未） |
| 6 | **Unicode 文字 node と DVI `set2` / `set3`** | 未 |
| 7 | **`\kanjiskip` / `\xkanjiskip`** を主ループに差し込む | 未 |
| 8 | **禁則**（`\prebreakpenalty` / `\postbreakpenalty` / `\inhibitxspcode`） | 未 |
| 9 | **縦組**（dir ノード） | 未。**ここが一番遠い** |

2026-08-22にWSL TeX Live 2026をresolver経由で通常探索し直したところ、`expl3-code.tex`は
未定義primitiveなしで通過した。最初のhard errorはGerman hyphenation pattern
`dehypht-x-2024-02-28.pat`の`.buß3`に対する`Nonletter`である。一時的な空`hyphen.cfg`で
pattern読込みだけを隔離すると、無改変`latex.ltx`はerror 0で`latex.fmt`をdumpした。
段4cでこの`.buß3`自体は解消した。ただし通常のLaTeX wrapperはPraTeXに`\kanjiskip`が無いため
非pTeXのnative UTF-8 engine分岐へ入り、kcatcode 18のU+2019を含む後続patternで止まる。
現在の次段は初期分類の偽装ではなく、一級の`\kanjiskip` / `\xkanjiskip`実装である。
ASCIIの`ushyph1.tex`だけでformatを作れた過去の測定と、通常TeX Live探索でUnicode patternを
読めない現状を混同しない。

`.buß3`を通す最小条件は`LatinUcs`をUTF-8 byte列へ戻さない一文字token、Unicode cat/lccode表、
Unicode文字を扱うpattern alphabet/trie、byte数でなく文字数を数える上限である。実段落の
hyphenationまで完成させるにはUnicode文字nodeとOFM接続も後続する。pattern本文は互換性を測る
opaque外部入力としてだけ使い、実装資料へ転記しない。

**1〜3を全て終えれば e-TeX 相当になる。** 現在は一部が未完であり、LaTeX2eが動くことを
e-TeX完全対応の代用にはしない。欠落とTeX--XeTの実処理は
[e-TeXとTeX--XeTの対応状況](etex-texxet-status.md)で追跡する。

**4〜8で横組みの日本語が組めるのは中間checkpointである。** 依頼者が定める最低限は
pTeX相当なので、段9の縦組まで含めて完了とする。割注はpTeX primitiveではなく、ここには
含めない。詳細は[pTeX相当からJLReq一級対応へ進むroadmap](japanese-typesetting-roadmap.md)にある。

段7の内部機構は日本語専用に固定しない。JFMとUnicode文字nodeの後で、Han--Latin、
Hangul--Latinなどを同じfinalizerへ渡し、CJKV region、Vaakの宣言表、低頻度WASM batchを
別domainとして接続する。互換primitiveの表面は保持する。詳細は
[拡張可能なscript境界組版とCJKV region](extensible-layout-roadmap.md)にある。

## 5. 見立て

| | |
|---|---|
| **段 1〜3（e-TeX）** | 現実的。既存の道をほとんど壊さない |
| **段 4〜8（横組み和文）** | 大きいが筋は通っている。JFM が要 |
| **段 9（縦組）** | **これだけ別格。** ノードの向きが増えると行分割も箱組みも変わる |

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

同じ節の`\mutoglue` / `\gluetomu`もTeX Live 2026 e-upTeXへ黒箱照合した。前者は数式糊を
通常糊へ、後者は通常糊を数式糊へ変え、幅・伸縮係数・次数の数値を変えない。逆の型を
渡した場合は`Incompatible glue units`を報せた後、1muと1ptを同じscaled値として回復する。
二命令は一つの`GlueConversion`からprimitive名、内部量の入出力型、`\meaning`、fmtを決め、
値の走査とerror回復は既存`scan_glue`だけに持たせる。

# e-TeXとTeX--XeTの対応状況

更新: 2026-08-24

## 結論

PraTeXはe-TeXのmacro処理・拡張register・class別mark・式・typed疑似入力をかなり実装しているが、
**e-TeX完全対応ではない**。組版、group/if表示、discardに未実装が残る。
TeX--XeTは二つの整数parameterを保存できるだけで、**組版機能としては未実装**である。

この文書の「実装」は、primitive名が登録されているだけでなく、本来の処理へ接続され、
対象を固定する試験があることをいう。値の代入、group復元、fmt保存だけなら「表面のみ」とする。

## 機能群ごとの監査

| 機能群 | 判定 | 現在の範囲と残件 |
|---|---|---|
| `\numexpr`、`\dimexpr`、`\glueexpr`、`\muexpr` | 実装 | 優先順位、括弧、丸め、糊次数、fmtを試験済み |
| `\protected`、`\detokenize`、`\unexpanded` | 実装 | 通常の`\edef`、`\message`、`\write`、mark走査とfmtへ接続済み。alignmentの`\noalign` / `\omit`先読みは通常macroだけを展開し、protected macroを通常入力として残す。専用4件と既存e-TeX 41件、plain DVI回帰を通した |
| `\ifdefined`、`\ifcsname`、`\unless` | 実装 | 未定義制御綴を作らず、`\unless`は真偽と`\currentiftype`の符号をともに反転する。通常・反転条件の入れ子と復元をprocess試験済み |
| group/if内省 | 部分 | `\currentgroup*`、`\currentif*`は基本動作し、`\unless`の負符号も入れ子から正しく復元する。複雑なgroup/conditional組合せの網羅、`\showgroups`、`\showifs`が残る |
| 拡張register 0--32767 | 実装 | count/dimen/skip/muskip/toks/boxの局所・大域・別名・fmtを試験済み |
| class別mark 0--32767 | 実装 | page遷移、`\vsplit`、class 0互換、fmtを試験済み |
| `\readline`、`\everyeof` | 実装 | `\readline`は実streamへ接続済み。実fileと`\scantokens`疑似fileの`\everyeof`は自然EOFだけで一度挿入し、`\endinput`では挿入しない。自然EOF内の行番号も試験済み |
| `\scantokens` | 部分 | 未展開general textを一時fileなしのbounded疑似入力へ積む。生成時の`\newlinechar`、行ごとの`\endlinechar`、再分類、暗黙groupなし、入れ子走査、tracing snapshot、fmtをprocess試験済み。raw byte 10/13、二段診断context、資源超過、`\pausing`監査は残る |
| 糊成分の照会と型変換 | 実装 | `\gluestretch`等4種と`\mutoglue`、`\gluetomu`を内部量へ接続。係数・次数・単位不一致回復・式・fmtを試験済み |
| `\eTeXversion`、`\eTeXrevision`、対話状態 | 実装 | `\eTeXversion`は内部整数`2`、`\eTeXrevision`はother文字`.6`へ展開。`\interactionmode`、`\errorcontextlines`も実経路へ接続 |
| `\lastnodetype` | 実装 | 空list、基本node型、page→nested box→page復帰をprocess試験済み |
| font寸法照会 | 実装 | `\fontcharwd`、`\fontcharht`、`\fontchardp`、`\fontcharic`はfont identifierと0--255の文字番号を読み、8-bit TFMの共通typed metric queryから内部寸法を返す。欠落字・範囲外glyph・nullfontは0pt、文字番号自体が範囲外なら既存8-bit scannerがcode 0へ診断回復する。寸法代入、`\dimexpr`、`\number`、`\the`のother文字token化、fmtを自作TFMで試験済み |
| `\iffontchar` | 実装 | font identifierと0--255の文字番号を読み、8-bit TFMの存在判定へ接続する。範囲外文字番号は中央scannerで診断してcode 0へ回復し、欠落字とnullfontは偽を返す。code 0だけを持つ自作TFMでprocess試験済み |
| parshape拡張 | 実装 | `\parshapelength`、`\parshapeindent`、`\parshapedimen`は現在の`\parshape`を内部寸法として照会する。非正index、最終pair反復、奇偶のinterleave、式・`\the`・`\number`、fmtを試験済み |
| penalty配列 | 実装 | `\interlinepenalties`、`\clubpenalties`、`\widowpenalties`、`\displaywidowpenalties`は正の個数に続く整数列を局所／大域代入でき、0以下でresetする。内部整数照会は負添字またはresetで0、添字0で長さ、正添字で値を返し、範囲外では末尾を反復する。4配列をfmtへ保存し、通常段落とdisplay直前の部分段落でpost-line-break penaltyへ接続する。`\interlinepenalties`だけは段落終了時に`\parshape`と同じreset経路を通る。独立process試験で照会、group・`\globaldefs`、fmt、実node列を固定済み |
| discard list | 未実装 | `\pagediscards`、`\splitdiscards`がない。`\savingvdiscards`は値だけを保存する |
| math | 実装 | `\middle`は`\left`--`\right`内部をsegmentごとのsave groupへ分け、各境界で局所代入を復元して元のmath styleから次のlistを始める。全delimiterを全segmentの最大height/depthから同じ大きさにし、左右のspacingをそれぞれClose/Openとして扱う。文字・`\delimiter`走査、delimiter欠落と対応しない境界の回復、表示、fmtを独立process試験済み |
| `\showtokens` | 実装 | 入口で左braceを探す間だけ通常展開を許し、balanced text本体を展開せず既存token表示へ渡す。外側brace除外、入れ子、parameter tokenの二重表示、和文token、全mode、通常error上限への非加算、`\let` alias、fmt、JFM continuity切断をprocess試験済み。公式Web2Cのshow診断がexit 1になるのに対しPraTeXの通常終了statusは0である既存CLI差分は別残件 |
| tracing/show | 部分／未実装 | `\tracingscantokens`は実出力へ接続済み。他のtracing parameterは値だけで、`\showgroups`、`\showifs`と対応trace出力がない |
| `\savinghyphcodes` | 実装 | 正値の`\patterns`時にlanguage別の小文字写像を保存し、pattern圧縮後の通常hyphenationと例外登録へ使う。同一languageの再snapshot、0以下での保持、fmt、8-bitとPraTeX Latin-UCS拡張の型分離を試験済み |
| その他の組版制御 | 表面のみ | `\lastlinefit`は処理本体へ未接続 |

fmtの表現があることと、読み戻した値が全ての後段へ効くことは分けて検査する。現在の
process-level fmt往復試験はregister、mark、糊成分を中心とし、全命令を網羅していない。

## TeX--XeT

現状の`\TeXXeTstate`と`\predisplaydirection`は整数の登録、代入、group復元だけであり、
`\TeXXeTstate`はformatへ非零値を持ち越さず既定offに戻す。`\predisplaydirection`は自動計算を
まだ行わない。次の意味論は一つも実装していない。

- `\beginL`、`\endL`、`\beginR`、`\endR`
- LR方向nodeと対応するstackの整合性検査
- paragraph、line breaking、hpackにおける方向区間
- 区間の反転と境界処理
- DVI/PDF shipoutでの方向付き配置
- TeX--XeT専用の回帰試験

したがってPraTeXを「TeX--XeT対応」とは表示しない。pTeXの横・縦方向とTeX--XeTの
left-to-right/right-to-left区間は公開意味論が異なるため、一つの互換primitiveへ潰さない。
内部の`WritingMode`、方向node、backend座標処理を型付きで共有するに留める。

## 実装順

日本語組版を優先しつつ、後で同じ基盤を作り直さない順序を採る。

1. 実装済みの`FontCharDimension` query種別を、将来JFM・Unicode font metricへ広げる。
   e-TeX primitiveの公開文字番号は0--255のまま保ち、別の文字identityを暗黙に混ぜない。
2. discard保存、`\lastlinefit`、show/tracingを実処理へ接続する。
3. TeX--XeTはrestricted hboxで方向node、LR stack、共通DVI/PDF shipoutまでを最初の縦sliceにし、
   次にparagraph、display、mathへ広げる。parameterだけ先に「対応済み」へ格上げしない。

完全対応の完了条件は、公開e-TeX manualの全primitiveを一覧照合し、通常実行、group、
error回復、fmt往復、DVI/PDFへの効果を該当機能ごとに試験した状態である。LaTeXが通ることだけを
完全対応の代用にはしない。

## クリーンルーム資料

原実装のsourceは参照・移植せず、公開仕様と許可されたblack-box観測から書き直す。

- [The e-TeX Short Reference Manual](https://mirrors.ctan.org/systems/doc/etex/etex_man.pdf)
- [e-TeX移植記録](etex-port-notes.md)
- [`\savinghyphcodes`実装契約](etex-savinghyphcodes.md)
- [TeXにない機能の実装一覧](feature-inventory.md)

`\middle`は同manual 3.9と5.4の公開契約から、segmentごとに元のstyleの新しいgroup/math listを始め、
完成済みの内部listを次segmentのleft boundaryとして引き継ぐ形で独立実装した。全delimiterの
共通寸法、Close/Open spacing、文字・数値delimiter、診断回復、fmt/表示は自作TFMを生成する
[process試験](../tests/etex_middle.rs)で固定し、原実装sourceや上流testは参照していない。

`\showtokens`は同manual 3.3・3.12・5.1のgeneral text契約から、入口の展開と本文の非展開走査を既存scannerで
分離し、表示は`Token::display`へ集約した。公式TeX Live 2026のe-TeXとe-upTeXに対する自作
black-boxで、入れ子brace、制御綴後の空白、catcode 6のparameter tokenが`##`になること、
和文token、和文glyph間でJFM continuityを切ることを照合した。実装sourceや上流testは参照していない。

4つのpenalty配列は同manual 3.8の公開契約から、添字方向と末尾反復を一つのtyped storageへ
集約して独立実装した。従来の単一parameterへ戻るreset状態、部分段落ごとのclub添字、段落末から
逆向きのwidow添字、display直前の選択を[process試験](../tests/etex_penalty_arrays.rs)で固定し、
原実装sourceや上流testは参照していない。

`\eTeXrevision`は公開manualの文字列契約と、TeX Live 2026のe-upTeX
`p4.1.2-u2.02`に対する自作black-boxの`.6`展開を照合した。実装sourceや上流testは
参照していない。照合binaryはCTAN tlnet `uptex.windows` revision 78020、
archive SHA-256 `c878983da002f32a24a507680ccf00261a3761089ed324892668ded589bf9c0d`。

`\fontcharwd/ht/dp/ic`は同manual 3.4の公開契約に加え、公式CTAN配布のpdfTeX
3.141592653-2.6-1.40.29（e-TeX extended mode）への
自作入力で、内部寸法、0--255走査、code 0への範囲回復、欠落字とnullfontの0pt、
`\the`のother文字列、`\number`のsp値、font-id・現在font・math family指定を照合した。
probeはrepositoryの試験を上流testから写さず独立に作成している。照合archiveは
TeX Live revision 78097、2026-08-24取得の`pdftex.windows.tar.xz`（874,164 bytes、SHA-256
`6794c3c173d1c3e9add63ed3d631b07312c208ed7d60dbed7764f588ce09ee6e`）、取得URLは
`https://mirrors.ctan.org/systems/texlive/tlnet/archive/pdftex.windows.tar.xz`である。
`pdftex.exe` / `pdftex.dll`のSHA-256はそれぞれ
`4b582d0be712b74ae5090aba2d7338f185082f6446cbee7b26115e8ab6e21184` /
`199788b93da06b355cedc4bab1a3695e5c199413176020a708e7658eb2c835bf`。
metricはTeX Live revision 57963、2026-08-23取得の`cm.tar.xz`（238,064 bytes、SHA-256
`ebedd3dc7ece433d366d848ea8bd9cd2642a0f49c000c46a2ed1dde5b1cebc1c`、
`https://mirrors.ctan.org/systems/texlive/tlnet/archive/cm.tar.xz`）に含まれる
`cmr10.tfm`（1,296 bytes、SHA-256
`87f2d8981927644cbecaf3d639e96e348ea4e7be49d8804468bd8ba9ff3f5244`）を用いた。

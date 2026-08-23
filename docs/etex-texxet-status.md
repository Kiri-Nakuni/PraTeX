# e-TeXとTeX--XeTの対応状況

更新: 2026-08-23

## 結論

PraTeXはe-TeXのmacro処理・拡張register・class別mark・式をかなり実装しているが、
**e-TeX完全対応ではない**。組版、表示、discard、penalty配列、疑似入力に未実装が残る。
TeX--XeTは二つの整数parameterを保存できるだけで、**組版機能としては未実装**である。

この文書の「実装」は、primitive名が登録されているだけでなく、本来の処理へ接続され、
対象を固定する試験があることをいう。値の代入、group復元、fmt保存だけなら「表面のみ」とする。

## 機能群ごとの監査

| 機能群 | 判定 | 現在の範囲と残件 |
|---|---|---|
| `\numexpr`、`\dimexpr`、`\glueexpr`、`\muexpr` | 実装 | 優先順位、括弧、丸め、糊次数、fmtを試験済み |
| `\protected`、`\detokenize`、`\unexpanded` | 部分 | 通常の`\edef`、`\message`、`\write`、mark走査とfmtへ接続済み。ただしalignmentの`\noalign` / `\omit`先読みがprotected macroを展開してしまう |
| `\ifdefined`、`\ifcsname`、`\unless` | 部分 | 未定義制御綴を作らない条件処理は接続済み。`\unless`で開始した条件の`\currentiftype`が負値にならない |
| group/if内省 | 部分 | `\currentgroup*`、`\currentif*`は基本動作する。unless符号、複雑な入れ子、`\showgroups`、`\showifs`が残る |
| 拡張register 0--32767 | 実装 | count/dimen/skip/muskip/toks/boxの局所・大域・別名・fmtを試験済み |
| class別mark 0--32767 | 実装 | page遷移、`\vsplit`、class 0互換、fmtを試験済み |
| `\readline`、`\everyeof` | 部分 | `\readline`は実streamへ接続済み。`\everyeof`は自然EOFと`\endinput`を区別せず、force EOFでも挿入する不具合がある。疑似fileと行番号も未達 |
| 糊成分の照会と型変換 | 実装 | `\gluestretch`等4種と`\mutoglue`、`\gluetomu`を内部量へ接続。係数・次数・単位不一致回復・式・fmtを試験済み |
| `\eTeXversion`、対話状態 | 部分 | `\eTeXversion=2`、`\interactionmode`、`\errorcontextlines`は動く。`\eTeXrevision`はない |
| `\lastnodetype` | 実装 | 空list、基本node型、page→nested box→page復帰をprocess試験済み |
| font照会 | 部分 | `\iffontchar`は8-bit TFMへ接続済みだが、範囲外入力を黙って偽にし公開8-bit numberのerror/recoveryを通らない。`\fontcharwd/ht/dp/ic`もない |
| parshape拡張 | 未実装 | `\parshapelength`、`\parshapeindent`、`\parshapedimen`がない |
| penalty配列 | 未実装 | `\interlinepenalties`、`\clubpenalties`、`\widowpenalties`、`\displaywidowpenalties`がない |
| discard list | 未実装 | `\pagediscards`、`\splitdiscards`がない。`\savingvdiscards`は値だけを保存する |
| math | 部分 | `\left`、`\right`はTeX82経路。e-TeXの`\middle`は未実装 |
| tracing/show | 表面のみ／未実装 | tracing parameterは値だけ。`\showtokens`、`\showgroups`、`\showifs`と対応trace出力がない |
| その他の組版制御 | 表面のみ | `\lastlinefit`、`\savinghyphcodes`は処理本体へ未接続 |

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

1. `\currentiftype`のunless符号、protected alignment、`\iffontchar`回復、TeXXeT format既定offを直す。
2. 自然EOF、`\endinput`、shutdownをtyped終了理由として分けてから、
   [`\scantokens`をboundedなvirtual input sourceとして実装](scantokens-design.md)し、
   `\everyeof`、`\tracingscantokens`、nested token走査を一箇所へ接続する。
3. `\fontchar*`を8-bit TFM専用APIにせず、TFM、将来のJFM、Unicode font metricを同じ
   typed query境界から参照する。
4. parshape照会とpenalty配列を局所代入、fmt、line breakingまで実装する。JLReqの段落処理も
   同じ保存・照会基盤を利用できるようにする。
5. `\eTeXrevision`と`\middle`を補う。
6. discard保存、`\lastlinefit`、`\savinghyphcodes`、show/tracingを実処理へ接続する。
7. TeX--XeTはrestricted hboxで方向node、LR stack、共通DVI/PDF shipoutまでを最初の縦sliceにし、
   次にparagraph、display、mathへ広げる。parameterだけ先に「対応済み」へ格上げしない。

完全対応の完了条件は、公開e-TeX manualの全primitiveを一覧照合し、通常実行、group、
error回復、fmt往復、DVI/PDFへの効果を該当機能ごとに試験した状態である。LaTeXが通ることだけを
完全対応の代用にはしない。

## クリーンルーム資料

原実装のsourceは参照・移植せず、公開仕様と許可されたblack-box観測から書き直す。

- [The e-TeX Short Reference Manual](https://mirrors.ctan.org/systems/doc/etex/etex_man.pdf)
- [e-TeX移植記録](etex-port-notes.md)
- [TeXにない機能の実装一覧](feature-inventory.md)

# e-TeXとTeX--XeTの対応状況

更新: 2026-08-22

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
| `\protected`、`\detokenize`、`\unexpanded` | ほぼ実装 | `\edef`、`\message`、`\write`、mark走査とfmtへ接続済み |
| `\ifdefined`、`\ifcsname`、`\unless` | 実装 | 未定義制御綴を作らない条件処理まで接続済み |
| group/if内省 | 部分 | `\currentgroup*`、`\currentif*`は動くが、複雑な入れ子の試験と`\showgroups`、`\showifs`がない |
| 拡張register 0--32767 | 実装 | count/dimen/skip/muskip/toks/boxの局所・大域・別名・fmtを試験済み |
| class別mark 0--32767 | 実装 | page遷移、`\vsplit`、class 0互換、fmtを試験済み |
| `\readline`、`\everyeof` | 部分 | 実fileでは動く。`\scantokens`疑似fileがないため、そのEOF契約は未達 |
| 糊成分の照会 | 部分 | `\gluestretch`等4種は実装。`\mutoglue`、`\gluetomu`は未実装 |
| `\eTeXversion`、対話状態 | 部分 | `\eTeXversion=2`、`\interactionmode`、`\errorcontextlines`は動く。`\eTeXrevision`はない |
| `\lastnodetype` | 部分 | node追跡はあるが専用試験がなく、base pageへ移す経路で型を同期しない可能性がある |
| font照会 | 部分 | `\iffontchar`は8-bit TFMへ接続済み。process試験と`\fontcharwd/ht/dp/ic`がない |
| parshape拡張 | 未実装 | `\parshapelength`、`\parshapeindent`、`\parshapedimen`がない |
| penalty配列 | 未実装 | `\interlinepenalties`、`\clubpenalties`、`\widowpenalties`、`\displaywidowpenalties`がない |
| discard list | 未実装 | `\pagediscards`、`\splitdiscards`がない。`\savingvdiscards`は値だけを保存する |
| math | 部分 | `\left`、`\right`はTeX82経路。e-TeXの`\middle`は未実装 |
| tracing/show | 表面のみ／未実装 | tracing parameterは値だけ。`\showtokens`、`\showgroups`、`\showifs`と対応trace出力がない |
| その他の組版制御 | 表面のみ | `\lastlinefit`、`\savinghyphcodes`は処理本体へ未接続 |

fmtの表現があることと、読み戻した値が全ての後段へ効くことは分けて検査する。現在の
process-level fmt往復試験はregister、mark、糊成分を中心とし、全命令を網羅していない。

## TeX--XeT

現状の`\TeXXeTstate`と`\predisplaydirection`は、整数の登録、代入、group復元、fmt保存だけで
ある。次の意味論は一つも実装していない。

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

1. `\lastnodetype`のpage遷移を直し、全node種・空list・page builder移動を試験する。
2. `\scantokens`をboundedなvirtual input sourceとして実装し、`\everyeof`、
   `\tracingscantokens`、nested token走査を一箇所へ接続する。
3. `\fontchar*`を8-bit TFM専用APIにせず、TFM、将来のJFM、Unicode font metricを同じ
   typed query境界から参照する。
4. parshape照会とpenalty配列を局所代入、fmt、line breakingまで実装する。JLReqの段落処理も
   同じ保存・照会基盤を利用できるようにする。
5. `\eTeXrevision`、糊変換、`\middle`を補う。
6. discard保存、`\lastlinefit`、`\savinghyphcodes`、show/tracingを実処理へ接続する。
7. TeX--XeTは方向node、LR stack、line packing、DVI/PDF shipoutまでを一つの独立機能枝で
   実装する。parameterだけ先に「対応済み」へ格上げしない。

完全対応の完了条件は、公開e-TeX manualの全primitiveを一覧照合し、通常実行、group、
error回復、fmt往復、DVI/PDFへの効果を該当機能ごとに試験した状態である。LaTeXが通ることだけを
完全対応の代用にはしない。

## クリーンルーム資料

原実装のsourceは参照・移植せず、公開仕様と許可されたblack-box観測から書き直す。

- [The e-TeX Short Reference Manual](https://mirrors.ctan.org/systems/doc/etex/etex_man.pdf)
- [e-TeX移植記録](etex-port-notes.md)
- [TeXにない機能の実装一覧](feature-inventory.md)

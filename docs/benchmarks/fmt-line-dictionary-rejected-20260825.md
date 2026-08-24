# 行辞書fmt案の棄却

更新: 2026-08-25

## 結論

既存のASCII fmtを行単位の辞書と可変長ID列へ変換する案は、同じ意味decodeを残したまま
on-disk sizeだけを57,221,231 byteから27,228,893 byteへ縮めた。しかし空実行20組の交互A/Bでは
wallのpaired geometric meanが`binary / text = 1.150271329`となり、15.0%遅かったため棄却した。
この試作のengine差分はcommitへ残さない。

| | ASCII text | 行辞書binary |
|---|---:|---:|
| wall平均 | 711,761,052 ns | 818,799,559 ns |
| wall中央値 | 711,534,255 ns | 813,872,822 ns |
| instructions概数 | 4.208 billion | 3.472 billion |
| cache missesの主な範囲 | 23.7--27.3 million | 31.7--38.6 million |

命令数は約17.5%減った一方、辞書の文字列参照と全semantic text parseを併用したためcache missが増え、
wallを悪化させた。圧縮率や命令数だけをfmt採用条件にしてはいけない反例として保存する。

raw counterは[`fmt-line-dictionary-rejected-20260825.tsv`](fmt-line-dictionary-rejected-20260825.tsv)、
binary・fmt・実行条件は
[`fmt-line-dictionary-rejected-20260825-provenance.tsv`](fmt-line-dictionary-rejected-20260825-provenance.tsv)
に固定した。CPU 0へ固定し、各fmtを3回warm-upした後、同一binaryでtext→binaryを20組実行した。
これは空実行の局所診断であり、299頁gateの値ではない。

## 次の境界

次案は文字列を別表へ移すだけでなく、version、section長、要素数、checksumを持つsectioned envelopeと、
型付きfixed-width recordを使う。実fmtでは`Hyphenator`が39,624,356 byte、うち`PreTrie`が
30,179,276 byteを占める。loaded formatはpattern登録へ戻らないので、実行時に必要な圧縮済みtrieだけを
`HyphenRuntimeV1`へ保存し、pattern構築用`PreTrie`とbuild hashを復元しない。

旧ASCII readerは互換fallbackとして維持する。新経路はlittle-endian fixed-width、decode前checksum、
checkedな長さ積、section重複・重なり・未消費byte拒否、全体allocation budgetを持たせる。破損fixture、
旧／新fmtのprocess試験、TRIP、299頁DVI意味一致、同一binaryの交互A/Bを通るまで既定値へしない。

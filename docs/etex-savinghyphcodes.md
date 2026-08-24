# `\savinghyphcodes` 実装契約

PraTeXの一次仕様は、公式 [e-TeX manual 2.0, §3.10](https://mirrors.ctan.org/systems/doc/etex/etex_man.pdf)
である。e-TeX/pTeX/upTeX系の実装sourceや上流testは参照しない。

## e-TeXの8-bit契約

- `\patterns`実行時の`\savinghyphcodes`が正なら、その時点の`\lccode`を現在の
  `\language`用hyphenation codeとして保存する。
- 保存codeを使うのはpattern trieが圧縮された後だけである。したがって圧縮前の
  `\hyphenation`は現在の`\lccode`を使い、圧縮後の`\hyphenation`と通常の単語走査は
  保存codeを使う。
- 同じlanguageで後から正値の`\patterns`を実行するとsnapshotを置き換える。
  0以下の`\patterns`は新規snapshotを作らず、既存snapshotも消さない。
- snapshotはlanguageごとに独立し、圧縮済みpatternとともにfmtへ保存する。

内部型`EtexHyphenationCodes`はkeyとvalueをともに`u8`へ閉じた256要素のdense表として
保持する。これはe-TeXが公開する8-bitの意味境界であり、active snapshotの通常文字lookupは
直接添字になる。language slotは`Option<Box<_>>`なので、snapshotを持たないlanguageのために
256要素表を確保しない。

## PraTeX Latin-UCS拡張

PraTeXは入力文字またはhyphenation codeがbyteを越える写像を
`LatinUcsHyphenationCodes`へ分ける。範囲はPraTeXのLatin-UCS上限U+2E7Fまでであり、
case-tableだけが受理するone-past sentinel U+2E80はhyphenation codeとして保存しない。
8-bit表を単に`u16`へ広げず別型にすることで、e-TeX互換範囲とPraTeX固有拡張をfmt検証でも
混同しない。

snapshotを持たない通常runの単語走査は、run-globalな`has_saved_hyphenation_codes`の
予測可能なfalse branchを一度通った後、従来どおり`Eqtb::lc_code`を直接読む。snapshot生成は
`\patterns`時だけのcold pathで、PraTeXの全Latin-UCS範囲11,904 code pointを一度走査する。

## 公式binaryによるblack-box観測

2026-08-24に、隔離したTeX Live 2026の
`e-upTeX 3.141592653-p4.1.2-u2.02-251130-2.6`を`-etex --ini`で実行した。
repositoryへ公式資材は取り込んでいない。

- CTAN `systems/texlive/tlnet/install-tl-unx.tar.gz` SHA-256:
  `f2cbb1bee21b13d5a844dc93eab6bb6dc5287cf404e5124a2eeb99c572c2f6b3`
- 隔離treeの`bin/x86_64-linux/euptex` SHA-256:
  `a39eba81da57bab2e96237f9e367d0d6ac92b1fd8a8f42797f3e4e267da18659`

| 最小入力 | 観測 |
|---|---|
| 正値で`a1b`を読んだ後、A/B/a/bの`\lccode`を0へ変更 | `AB`は保存codeで`A-B`へ分割 |
| 同一languageでA→xへ変更し、正値で別patternを追加 | snapshotが更新され、旧`a1b`は`AB`へ一致しない |
| 同じ変更後、0または-1で別patternを追加 | どちらも以前のsnapshotを保持し、`AB`は旧`a1b`で分割 |
| pattern圧縮前にA/Bの`\lccode`を0にして`\hyphenation{A-B}` | `Not a letter`が2件 |
| 一度line breakingして圧縮後、同じ例外を登録 | 診断なし。保存codeで例外を登録 |

production testは正値、0と負値、再snapshot、複数language、圧縮後の例外、fmt往復、
Latin-UCS写像を固定する。fmt読込みでは0値、重複key、Latin-UCS範囲外を拒否する。

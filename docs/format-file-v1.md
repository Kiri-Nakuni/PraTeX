# PraTeX format file v1

更新: 2026-08-25

## 目的と互換境界

PraTeXの既定fmtは`PRATEXF\0` magicを持つsectioned binary envelopeである。raw Rust structの
memory imageやpointerを保存せず、全整数をlittle-endianの固定幅で明示する。旧ASCII fmtは今後も
magicなしの互換入力として読む。INITEXで旧ASCIIを生成する診断経路は
`PRATEX_FMT_CODEC=legacy-text`で選べる。

fmtはPraTeXの内部状態を保存するもので、upTeX、pTeX、pdfTeX等のformat互換を主張しない。
旧PraTeX binaryは新fmtを読めない。PraTeXはまだversion 0なので、配布formatは生成binaryの版・hashと
一緒に固定する。

## envelope

先頭16 byteは次の順である。

| field | 幅 | v1 |
|---|---:|---:|
| magic | 8 | `PRATEXF\0` |
| format major | u16 | 1 |
| format minor | u16 | 0 |
| section count | u16 | 3 |
| reserved | u16 | 0 |

各section table entryは28 byteで、`kind:u16`、`codec_version:u16`、`flags:u32`、
`offset:u64`、`length:u64`、`crc32:u32`の順である。v1では全sectionのflagsが1（required）で、
table順とpayload順は次へ固定する。

| kind | codec | 内容 |
|---:|---:|---|
| 1 | 0 | `EqtbLegacyTextV0` |
| 2 | 1 | `HyphenRuntimeV1` |
| 3 | 0 | `RunMetadataLegacyTextV0` |

offsetはtable直後から隙間なく連続しなければならない。重なり、隙間、範囲外、末尾余剰、未知の必須
section、未対応versionを拒否する。各CRC32はpayloadを意味decodeする前に検査する。CRC32は偶発破損を
検出するものであり、真正性や敵対的改竄への署名ではない。

format全体は512 MiBまで読む。これは生文字列registerが合法に持てる64 MiBを十進一byte一行で保存する
旧wireも収めるための上限である。streamは`MAX+1` byteまでに制限して読み、無制限な`read_to_end`の
後で初めて上限を検査することはしない。

## HyphenRuntimeV1

このsectionだけは全semantic text parseを止め、実行時に必要な状態を型付きrecordで持つ。

- 256言語のhyphenation exception。wordは2--63個のu16、位置はu8で、wordは辞書順に書く。
- 256言語の`\savinghyphcodes` snapshot。presence tag、e-TeX用256 byte dense表、PraTeX
  Latin-UCS拡張のstrict昇順`(u16,u16)`列を持つ。
- 圧縮済みTrie。nodeは`link:u32`、`chr:u16`、`op:u32`の10 byteで、各Noneは型domain外の最大値を使う。
- 256言語のhyphen operation。`distance:u8`、`num:u8`、`next:u32`の6 byteである。

decoderはcount×record幅をchecked演算し、section残量と全体上限を確認してから一回だけ確保する。
exceptionは全言語合計262,144、Trie nodeは4,194,304、hyphen operationは全言語合計1,048,576を
v1上限にする。重複word、範囲外文字・位置・index、前方参照するoperation、重複operation、深さ63超、
壊れたlanguage family、未消費byteを拒否する。

`PreTrie`、その`subtrie_hash`、pattern構築用`op_code_hash`は保存しない。loaded engineでは
`\patterns`を追加できず、実行時検索は圧縮Trieのnodeとoperationだけを読むからである。decode後の
`PreTrie`はroot一個の空状態、`has_saved_hyphenation_codes`はsnapshotのpresenceから再計算する。
runtime Trie自身からもbuild hashを除いた。旧ASCII readerはhashとoperationの完全な対応を検証して
からhashだけを捨て、旧ASCII writerは互換wireへ必要な間だけoperation列から再構成する。

## 検証gate

- envelope unit: version、CRC32既知値、checksum、truncation、隙間、末尾余剰。
- runtime unit: deterministic round-trip、空PreTrie、例外・snapshot、範囲外link、前方参照、重複op、
  未圧縮Trieの書出し拒否。
- process: binary生成・自動判別、checksum破損、metadata、Latin-UCS snapshot、pattern、dump前例外、
  旧ASCII生成・読込。
- 全release: 932 passed、0 failed、11 ignored。
- TRIP: binary fmtと旧ASCII fmtのStage 1/2がともにexit 0。固定commentのDVIは公式2920 byteへ
  byte一致した。

性能のraw dataと採用判断は
[`benchmarks/fmt-hyphen-runtime-v1-20260825.md`](benchmarks/fmt-hyphen-runtime-v1-20260825.md)
を一次資料にする。

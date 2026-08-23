# JFM clean-room実装記録

更新: 2026-08-23

## 境界

PraTeXのJFM readerは、日本語TeX開発コミュニティの公開
[JFM仕様](https://mirrors.ctan.org/info/ptex-manual/jfm.pdf)から独立実装する。pTeX/upTeXの
engine source、change file、上流回帰試験は参照・移植しない。2026-08-22に取得した仕様PDFは
235,593 bytes、SHA-256
`0f1afd9bc8542335c4ff41213c49fb4dcbebd46b8ca88fd558f0c4106e9e700a`だった。
同じ公開文書のTeX source `jfm.tex`（engine sourceではない）も参照し、取得物のSHA-256は
`a95051b22b8454e41568d300e9c4adb5c3c923ebc8884ed10f1057014626be80`だった。

JFMは`.tfm`を名乗るが、欧文TFMへ条件分岐を散らさず、`src/jfm.rs`の純粋な
`parse(&[u8])`で全長と全参照を検査する。I/O、font選択、指定sizeへのscale、node生成は別moduleに
置き、parserへ実行状態を混ぜない。壊れたfileを読んでもpanicせず、未検査indexを組版経路へ渡さない。

## 実装済みのbinary意味

- 横組ID 11、縦組ID 9と、先頭7 wordの14個の非負halfword
- `lf`の総和とbyte長、`bc=0`、`ec<=255`、非空の寸法表、`ng % 3 == 0`
- header checksumとdesign size、24-bit raw文字code + u8 JFM class
- 先頭`(0, 0)`、raw codeの厳密昇順、未登録codeをclass 0にする検索
- char infoのwidth/height/depth/italic index、tag 0/1、予約tag 2/3の拒否
- 12.20 fix word、glue三語、kern、font parameter
- glue/kern programの`skip=0..128`、開始語だけの`skip>128`再配置、開始後の終了語
- `op_byte`を含む256超のglue/kern indexと、全jump・class・表indexの範囲検査

JFM内の24-bit値は、それ自体ではUnicodeか旧JISかを示さない。parserは`character_code`という
raw値だけを返す。後続の`\jfont`/`\tfont` loaderがencodingを明示的に持ち、font内容から
推測しない。

## 組版時の表現

glue/kern programはfont読込み時にclass対の直接表へcompileする。同じ右classが複数回現れた
場合はprogram順で最初の規則を保持する。実行時はwide glyph nodeに保存した`JfmClassId`二つで
一回だけ表を引き、raw codeの二分探索やprogram再解釈をしない。

classは最大256なので、行優先`u16`表は最大128 KiB/fontである。`0xffff`を規則なし、bit 15を
kern、残りをindexに使う。JFM全体の`lf<2^15`と必須表の語数により、glue/kernの実indexが
予約値と衝突することはない。標準日本語経路にVaak/WASM callbackは置かない。

raw fix wordと指定sizeへscale済みの値は別層にする。特に`zh`はrawのheight+depthを一度に
scaleせず、各成分をTeX互換にscaleしてからspで加える。slant parameterも寸法と同じ変換へ
混ぜない。

## 横組glyphへの接続

横組ID 11だけを`src/japanese_fonts.rs`のbounded loaderへ通す。JFMの`lf < 2^15`からfile読取に
上限を置き、論理font名とresolverが返した物理pathを分離する。12.20 fix wordは欧文TFMと共有する
中央scale関数で一度だけspへ変換する。縦組ID 9は横組fontへ黙って転用せず、現在は明示診断する。

- PraTeX nativeの正規名は`\pratexjfont`。`\jfont`は同じ横組JFM定義・選択の決定箇所へ結ぶ
  compatibility aliasであり、pTeX/upTeXのversion primitiveは捏造しない。
- current和文fontは欧文`FontIndex`と別の`JapaneseFontIndex`で持ち、選択はgroup、`\globaldefs`、
  fmtの既存保存経路を通す。fmtにはraw JFMと論理名・指定sizeを保存し、machine-local pathや
  run-local handleを保存しない。
- `zw`はscale済みclass 0 width、`zh`はscale済みclass 0 height+depth。未選択時だけ従来の
  欧文`em` fallbackを保つ。
- 横組CJK tokenはUnicode scalar、JFM class、width/height/depth/italicを保持する`WideCharNode`
  になる。外部vertical modeのliteral CJKも、validなcurrent横組JFMがあればparagraphを開始する。
- DVI font numberは欧文8-bit fontの増加に依存しない256始まりの別namespaceとし、BMP scalarを
  unsigned big-endian `set2`、補助面scalarを`set3`で出す。byte glyphの命令経路は変えない。
- PDF backendは和文glyph対応を偽らず型付きerrorにする。OTF shaping、縦組、PDF和文glyphは
  このsliceに含めない。

## 横組spacingへの接続

font load時に、検証済みJFM programの全class対を選択sizeへscaleし、
`CompiledJfmPairSpacingTable`へ一度だけcompileする。fmtにはrun-localなIDやcompile済みcacheを
保存せずraw JFMを保存し、undump後に`JapaneseFontIndex`からmetric IDを再束縛する。
横組finalizerは`WideCharNode`が保持するUnicode・font・JFM classを使うため、文字ごとに
raw code表やJFM programを再解釈しない。異なるfont instance間ではclass対表を横断せずKへ戻る。

2026-08-23のproduction checkpointでは、JFM glue/kern、`\kanjiskip`、`\xkanjiskip`、
4文字だけのBuiltIn禁則をhbox、段落、alignment cell、line breaking、DVI座標へ接続した。
自動nodeはJFM/K/X/禁則ごとのprovenanceを持ち、unbox後の再finalizeで利用者の明示nodeを
消さずに再生成する。

ただしJFM/禁則も現段階ではlist-closeでmaterializeするcorrectness sliceであり、
main loop中の`\unskip` / `\lastnodesubtype`のpTeX意味は未完成である。直結glyph間Kは
`VirtualKanjiSkip`としてnode introspectionから隠し、unshifted hbox境界Kは可視な
`MaterialKanjiSkip`へ分けた。`xspcode`、`inhibitxspcode`、auto switchと箱edgeのK/Xは
接続済みだが、`\inhibitglue`、discの非空三分岐、JLReqの全禁則classはまだ接続していない。

## TeX Live 2026黒箱オラクル

WSLのTeX Live 2026（upTFtoPL 3.3-p240427、Kpathsea 6.4.2）に配布された`uptex-fonts`と
`ptex-fonts`のJFM 96件を、先頭fieldと公開toolの出力だけで調べた。実装sourceと上流testは
使っていない。

| file | bytes | SHA-256 | 先頭14 halfword |
|---|---:|---|---|
| `upjisr-h.tfm` | 812 | `7d686f3edaa70f30195b2ced00c0babfc54910dcadfe93a80061d99b61dfaedf` | `11,113,203,18,0,6,3,2,2,1,25,1,15,9` |
| `upjisr-v.tfm` | 536 | `8cf599e3cebe322db5d4f01a272d0462433c21d3477a31f8abd96ac01ac615dc` | `9,50,134,18,0,5,3,2,2,1,20,1,15,9` |
| `umin10.tfm` | 1300 | `f7a7f2b76d279e0cdf8713c30f09bf337f0aad2dd91346a4159362907e5c591b` | `11,146,325,18,0,12,5,2,2,1,94,4,24,9` |
| `urml.tfm` | 108 | `e11b50450f3f9f6836cf029236f6941e48ba79bf75b9f396034e1d52d688eb97` | `11,1,27,2,0,0,2,2,2,1,0,0,0,9` |
| `urmlv.tfm` | 108 | `4b45e0a0f1fb05b009f990f8b8333a70a7d9669a759806f84eb1b116198e994c` | 縦組の最小例 |

96件の内訳は横56、縦40で、全て`bc=0,np=9`だった。実在最大は
`nt=146,lf=325,ec=12,nl=94,nk=4,ng=24`。この集合のprogramは`skip=0/128`だけで、
非零skip、再配置、256超indexを
使わない。したがって配布実物は通常互換のoracle、2018/2023年拡張は独立した合成fixtureで
試験する。

環境依存試験は配布binaryをrepositoryへvendorしない。

```powershell
$env:PRATEX_JFM_ORACLE = 'C:\path\to\upjisr-h.tfm'
cargo test --release 'jfm::tests::配布upjisr横組を公開仕様どおりに読む' --lib -- --ignored --exact
$env:PRATEX_JFM_ORACLE_DIR = 'C:\path\to\copied-tfm-root'
cargo test --release 'jfm::tests::配布jfm九十六件をすべて読む' --lib -- --ignored --exact
```

## 残る接続

- `\tfont`、縦組JFMのmetric解釈、方向つきwide nodeとDVI/PDF出力
- JFM/禁則のmain-loop早期挿入、disc非空三分岐の条件付きspacing境界
- 4文字subsetを越える禁則、ぶら下げ、行長調整を含むJLReq規則
- PDFの日本語font resource、OTF/TrueType、default-off RustyBuzz

横組glyphと最小spacingが一頁出ることも日本語組版対応を意味しない。上記を横・縦とも接続し、black-boxの
node・sp座標・DVI意味をe-upTeXと比較して初めてpTeX相当P0の一部になる。

## この段階の検証

- 合成JFM 14試験: 横・縦、全切断位置、各byte破損、skip、終了、再配置、256超glue/kern、
  tag・全index・fix word境界。失敗0
- TeX Live実物`upjisr-h.tfm`単体と、配布JFM 96件全件のignored試験を実行し、失敗0
- `cargo test --release --no-fail-fast`: 503 passed、0 failed、6 ignored
- TRIP二段ともexit 0、`tripos.tex`は最小正規化後一致。DVI SHA-256は変更前と同じ
  `b20af20a1463c6846f0c4c1ce687cd6354ce1a5f65ee401507627570787ae9fe`
- `src/jfm.rs`にunsafe Rustなし、`git diff --check`通過

## 2026-08-23の横組glyph checkpoint

- 合成JFMでclass 0/1/2のscale、`zw`/`zh`、box width/height/depth、node表示のUnicodeとclassを固定
- `\pratexjfont`と`\jfont` alias、current選択のgroup復元、fmt往復、縦JFM拒否をprocess試験で固定
- BMP U+3042の`set2`、補助面U+20000の`set3`、二glyph後のkern/rule座標をDVI decoderで照合
- validなcurrent JFMを選んだ外部vertical modeのCJKがparagraphを開始し、捨てられないことを固定
- `origin/main`と同じ欧文plain fixtureのBOPからEOPまで183 bytesを比較し、byte差分0
- `cargo test --release --locked --no-fail-fast`: 594 passed、0 failed、6 ignored
- 公式CTAN TRIPは両段exit 0、`tripos.tex`は最小正規化後一致。DVI SHA-256は既知正常値
  `b20af20a1463c6846f0c4c1ce687cd6354ce1a5f65ee401507627570787ae9fe`を維持
- TRIP archiveは`https://mirrors.ctan.org/systems/knuth/dist/tex.zip`から2026-08-23に取得し、
  SHA-256 `1d419b1bd7efa575ead0174e47d542a0099a73e0e4deb5031980d109e8c3c645`を記録した。
  このmachineにはPLtoTF/TFtoPL/DVItypeがないため、hash検証済み公式`trip.tfm`を使い、
  DVItypeによる今回の意味比較は未実行
- PraTeX通常sourceにunsafe Rustなし。他engineのsource・上流test・version偽装は使用していない

## 2026-08-23の横組spacing checkpoint

- JFM class pair、K/X、4文字BuiltIn禁則を中央finalizerから実hlistへ一度だけmaterialize
- hbox、段落、alignment、fmt、unbox再評価、line break、DVI glyph/rule座標の合成fixture 6件が成功
- `cargo test --release --locked --no-fail-fast`: 627 passed、0 failed、7 ignored
- origin/main plain DVIのBOP--EOP 183 bytesはbyte差分0、body SHA-256
  `980ceaa638dd272dac0b46ec0870ac166715db10655b004917de96615396337a`
- CTAN TRIP両段exit 0、`tripos.tex`最小正規化後一致、DVI SHA-256
  `b20af20a1463c6846f0c4c1ce687cd6354ce1a5f65ee401507627570787ae9fe`
- これはlist-close correctness checkpointで、main-loop JFM、仮想K、完全JLReq、縦組の完成ではない

## 2026-08-24のhbox edge checkpoint

- pTeX manual 2025-05-10版と公式e-upTeX TL2026自作probeで、unshifted hbox前後の
  material K/X、raise/lower除外、nested/空hbox、先頭kernを確認した
- 箱edge Kを`MaterialKanjiSkip`として直結glyph間の`VirtualKanjiSkip`から型で分離し、
  show/lastbox、unhcopy再評価、fmt、line break、DVI座標へ接続した
- 空discretionaryはK/Xを越境させない。非空discは左側を遮断し、no-break/post-break枝末尾から
  右側だけを接続することを確認したが、枝別eventが必要なのでproduction未実装である

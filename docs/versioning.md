# PraTeXの版1と、それ以後の版規則

更新: 2026-08-23

## 正式版1の完了条件

次のすべてを満たした最初のreleaseだけを **PraTeX 1** とする。途中のcheckpointや、
一部の標本だけが通る状態を1として公開しない。

1. 日本語組版をJLReqに対する一級のengine-native機能として実装する。横組だけでなく縦組、
   和欧混植、禁則、行長調整、ruby、縦中横、割注とDVI/PDF出力まで適合試験を持つ。
2. e-TeXをTeX--XeTを含めて完全実装する。primitive名の登録だけでなく、group、fmt、
   error回復、node、line breaking、DVI/PDFへの効果を公開manualの全項目で監査する。
3. `\year`、`\month`、`\day`、`\time`とLaTeXの既定`\date`が実行時刻を反映し、
   再現build用の明示的な時刻固定経路も持つ。
4. pdfTeX相当の直接PDF出力を完成させる。font埋込み・subset、CID、ToUnicode、画像、link・
   annotation、色、metadata、抽出Unicodeと主要package回帰を含む。
5. OTF/TrueTypeをfont選択、metric、cmap、埋込みまで実装する。標準の和欧組版はshapingへ
   依存させず、RustyBuzzは既定offの明示機能とする。
6. Vaak API、WASM ABI、WASM module systemを完成させる。WASM module systemは
   [import・名前空間仕様0.1](wasm-module-import-v0.1.md)を一次資料とし、別途未決定の
   control sequence実行ABIをPraTeX側で先回りして作らない。
7. PraTeX自身をWASM targetへcompileし、定めた適合suiteを実行できるようにする。

この一覧はrelease gateであって、実装順を強制するものではない。各領域の詳細な合格条件は
個別roadmapへ置き、この文書から参照する。

## 開発版の識別

正式版1まで、整数primitive `\pratexversion`は0を返す。存在確認はPraTeX native engineの
識別に使えるが、値0を完成版の能力保証に使ってはならない。版文字列はexpandableな
`\pratexrevision`とbannerで示し、現在は`0.1.0-dev`である。個別機能の判定は将来の
PraTeX固有feature queryで行い、他engineのversion primitiveへ偽装しない。

## 版1以後

正式版の表示列は次の順で、小数点以下へ一桁ずつ追加する。**末尾の零も版の一部であり、
省略しない。**

```text
1
1.1
1.11
1.110
1.1100
1.11000
1.110001
…
```

小数部はリウヴィル定数

```text
L = sum(n >= 1, 10^(-n!)) = 0.110001000000000000000001…
```

の桁である。したがってPraTeXの版表示は、releaseごとに既知の次の一桁を固定し、
`1 + L = 1.110001000000000000000001…`へ収束する。過去の版文字列を丸め直したり、
`1.1100`を`1.11`へ正規化したりしない。

Cargoのpackage versionなど三成分を要求する配送面では、表示版`1.1100`を例えば
`1.1100.0`へ機械的に写してよい。ただし利用者に見せるcanonical版とTeX transcriptでは
上の文字列を保つ。整数の大小だけから小数桁を復元せず、canonical文字列を一箇所で管理する。

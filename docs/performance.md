# safe Rust 性能測定

性能変更は、同じrelease設定・同じ合成入力で変更前後を交互に走らせ、出力の一致を
確認してから採用する。測定用入力、実行ファイルの複製、logはリポジトリ外の
`%TEMP%` にだけ置き、版方へ入れない。

## 一字の差し戻し

数値や条件の走査は、先読みした一字を `back_input` で頻繁に戻す。従来は一回ごとに
一要素の `Vec<Token>` と `Rc` を作っていた。`TokenListReader` に一字を直接保持する
表現を加え、通常のtoken listは従来どおり同じ `Rc` を共有するようにした。

測定入力は次の130 bytes（末尾LFを含む）で、100万反復中に約200万回の一字差し戻しを
通る。

```tex
\catcode123=1 \catcode125=2 \count0=0\relax \def\x{\advance\count0 by1\relax \ifnum\count0<1000000\relax \expandafter\x\fi}\x\end
```

- fixture SHA-256:
  `891B4D7B8B647F0E05886065C55716E0D195983E2C8F8E0B548E34248D1EE6FC`
- Windows x86_64、rustc 1.98.0、release LTO
- 各実行ファイルを2回warm-up後、順番を交互にして各11回測定
- wall timeとprocess CPU timeの中央値を比較

| | 変更前 | 変更後 | 短縮 |
|---|---:|---:|---:|
| wall中央値 | 768.249 ms | 510.810 ms | 33.51% |
| CPU中央値 | 750.000 ms | 500.000 ms | 33.33% |

全22回で終了値は0、stdout SHA-256は
`C91C3D5D175B00E4D9E00BB5F88A240BFDC339339DC777D6B51419141124E233`、
log SHA-256は
`2F4892969B144313E1A6710D8C5C5DFE18F5B76B3829F174210A489397790609` で一致した。

64-bit buildでは `TokenListReader` は16 bytesから24 bytesになるが、最大variantは別に
あるため `InputSource` 全体は56 bytesのまま変わらない。unsafe Rustは使っていない。
release全343試験を通し、最適化前後のTRIP DVI SHA-256も
`27B79B612B94A1D2815A8747D09B6BA665F2ADFB9F521FCFE7020C6347A29342` で一致した。

## CJK token導入時のASCII退行確認

UTF-8 CJK tokenとtyped制御綴を足した枝でも、上と同じASCII fixtureを使い、直前の
`9d04c08` とrelease LTO buildを交互に各11回測った。両方とも同じVaak checkoutを使い、
2回ずつwarm-upした。

| | `9d04c08` | CJK token枝 | 変化 |
|---|---:|---:|---:|
| wall中央値 | 553.506 ms | 542.120 ms | -2.06% |
| CPU中央値 | 546.875 ms | 531.250 ms | -2.86% |

stdoutとlogのSHA-256は全22回でそれぞれ一種類だけで、変更前後が一致した。小差はcode配置や
測定揺らぎの範囲なので高速化とは数えないが、ASCII fast pathの退行は観測されなかった。
CJK用decoderと `kcatcode` 検索はASCIIでは呼ばず、typed hashと逆引き表もwide制御綴を
初めて作るまで確保しない。測定用source tree、target、logは `%TEMP%` のみに置いた。
同じworktreeでrelease全406試験とTRIP二段を通し、TRIP DVI hashも直前枝と一致した。

## 統一文字分類器のASCII退行確認

`catcode` / `kcatcode` の問い合わせを `CharacterClassifier` traitへ統一した枝を、直接の親
`9af3f19` と比較した。短い計測ではWindowsのprocess CPU timeの15.625ms粒度が相対的に
大きいため、上のfixtureの終了値だけ300万へ増やした。両方を同じrustc 1.98.0、同じVaak
checkout、release LTOでbuildし、1回warm-up後に順番を交互にして各11回測定した。

| | `9af3f19` | 統一分類器枝 | 変化 |
|---|---:|---:|---:|
| wall中央値 | 1642.206 ms | 1636.016 ms | -0.38% |
| CPU中央値 | 1625.000 ms | 1593.750 ms | -1.92% |

stdout SHA-256は全22回で
`25855EADFEEFB5EA17162B1E1E012A6B87758354BB4759C8FE486DFE8B91F5BF`、logは
`E5C427B0A95D409FD86A1C7CA5D4E65583864ACCD45F05AB61F2E0406C621B87` の一種類だけで、
終了値も全て0だった。小差は測定揺らぎとして高速化には数えないが、ASCII退行は観測
されなかった。組込み経路は `Eqtb` 自身へ静的dispatchし、中間object、allocation、
Unicode表引き、拡張class ID生成をASCIIに加えない。`CatCode` は `repr(u8)` である。

## 次の候補

測定済みの次候補は、入力行bufferの再利用、PDF文字命令の一時 `String` 除去、fmt復元時の
既知個数による容量予約である。一つの枝へ混ぜず、同じ出力hashとTRIPを条件に個別採否を
決める。

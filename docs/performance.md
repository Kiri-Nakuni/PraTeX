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

## 次の候補

測定済みの次候補は、入力行bufferの再利用、PDF文字命令の一時 `String` 除去、fmt復元時の
既知個数による容量予約である。一つの枝へ混ぜず、同じ出力hashとTRIPを条件に個別採否を
決める。

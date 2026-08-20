# e-upTeX を rTeX へ — 着手前の覚え書き

**まだ調べていない。** 着手時に消す。

## 制約（依頼者の指定）

- **可能なら UTF-8 基底にする。** e-upTeX の内部は UTF-16 相当
  （`upTeX` の内部文字コードは Unicode をコードポイントで持ち、
  `kcatcode` の表もその前提で組まれている）
- **safe-Rust で書く**——rTeX の方針をそのまま守る
- 別ブランチを切る

## 先に確かめること

1. **LICENSE**：e-TeX 拡張と e-upTeX それぞれ。取り込めるか
2. rTeX の現状（tyti 氏の名前空間の作業と衝突しないか）
3. UTF-8 基底にしたとき何が壊れるか——
   `\kcatcode`、`\ucs`、`\kansuji`、`\inhibitxspcode` あたりが
   コードポイントで添字を取るはずなので、**表の引き方が変わる**

## Vaak との関係

**無い。** rTeX vaak（`\directvaak` / `\vaakdef`）は凍結中であり、
この作業とは別のブランチである。

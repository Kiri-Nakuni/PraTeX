# `prjlreq`の権利・来歴と移植境界

## 結論

`prjlreq`は、Noriyuki Abe氏の`jlreq`をPraTeXへ移植する派生classとして扱う。
独立実装とは呼ばない。上流の現行配布はBSD 2-Clauseであり、source/binaryの利用、改変、
再配布を明示的に許諾しているため、copyright notice、条件、免責を保持すればGPL-3.0-onlyの
PraTeX配布物へ組み込める。この文書は権利移転を表明するものではなく、上流著作権者の
排他的権利がPraTeXまたは作業者へ移ったとは扱わない。

PraTeX adaptationへCodex名義のcopyrightは追加しない。派生fileのheaderには上流の
copyright、BSD 2-Clauseであること、固定した上流revision、PraTeX向けの改変であることを残す。
完全な条文は
`tex/latex/pratex/prjlreq-UPSTREAM-LICENSE.txt`に改変せず収録する。

## 監査した一次資料

- 公式repository: <https://github.com/abenori/jlreq>
- 固定した上流commit:
  [`ac06fd0770096be8a118197d670a34da06384e02`](https://github.com/abenori/jlreq/commit/ac06fd0770096be8a118197d670a34da06384e02)
  （commit timestamp `2026-07-24T13:12:06+09:00`）
- 上流LICENSE:
  <https://github.com/abenori/jlreq/blob/ac06fd0770096be8a118197d670a34da06384e02/LICENSE>
- CTAN package page: <https://ctan.org/pkg/jlreq>
- CTAN source archive: <https://mirrors.ctan.org/macros/jptex/latex/jlreq.zip>
- CTAN表示version: `2026-07-17`
- 2026-08-24取得CTAN archive SHA-256:
  `4f990534dcf3bebce7c399fa3855eaab1d5fba2ca0a5142dd2173c987f494a5c`
- copyright: `Copyright 2017-2026, Noriyuki Abe.`
- license: BSD 2-Clause（CTAN表記はSimplified BSD License）

CTAN archiveの`LICENSE`、`jlreq.cls`、`jlreq-helpers.sty`、
`jlreq-complements.sty`は、上記Git commitの対応fileと改行をLFへ正規化した内容が一致した。
作業tree上のbyte hashはWindowsのCRLF変換で異なるため、同一性の根拠にそのhashを使わない。

## 条文から生じる義務

BSD 2-Clause本文で要求される事項は次の二つである。

1. source再配布では、上流copyright notice、条件一覧、免責を保持する。
2. binary再配布では、同じnotice、条件一覧、免責をdocumentationまたは同梱資料へ再掲する。

条文には、派生物のsource公開、同一filenameの維持、rename禁止、改変file名の変更、
UIでのcredit表示、改変日表示を要求する条項はない。明示的なpatent grantもない。
ただしPraTeX側では混同を避けるためclass名を`prjlreq`とし、移植元revisionと改変履歴を
この文書および派生file headerへ自発的に表示する。これはBSD 2-Clauseの追加条件ではなく、
追跡可能性のためのPraTeX側運用である。

## 名前とengine identity

- 公開class名は当面`prjlreq`とする。上流そのものの`jlreq`を名乗らない。
- `\ProvidesClass{prjlreq}`を使う。
- `\pratexversion`の存在だけでPraTeXを検出する。値0は開発中の正当なPraTeXなので、
  `\ifnum\pratexversion<1`では拒否しない。
- `\pdftexversion`、`\luatexversion`、`\XeTeXversion`、`\pTeXversion`、
  `\upTeXversion`、`\epTeXversion`を定義・偽装しない。
- 上流の`platex`、`uplatex`、`lualatex` class optionをPraTeX branchの選択に流用しない。
  当面は明示errorにし、将来もPraTeX固有feature契約から分岐する。

## 最小ロード方針

最初のproduction sliceは、横組articleの一頁fixtureをerror 0で処理する範囲に限定する。

1. LaTeXの公開`article` class interfaceを土台にする。
2. `\pratexversion`、`\pratexjfont`、`\kanjiskip`、`\xkanjiskip`を明示検査し、
   `pratex-japanese`のPraTeX固有和文font境界を使う。
3. 上流`jlreq.cls`の横組既定に由来する`\jlreqkanjiskip`、
   `\jlreqxkanjiskip`、一字下げの最小意味を移す。
4. 初期sliceで未接続の`tate`、`book`、`report`、`warichu`、`tatechuyoko`、
   JFM切替、全版面key、見出し定義APIは黙って近似せず、明示的に未対応とする。
5. 横組classがloadできることをもって上流`jlreq`互換とは呼ばない。
   公開commandまたはoptionごとにfixtureと欠落表を追加してから対応済みにする。

縦組はPraTeXの方向node、縦JFM、DVI/PDF出力が完成する前にclass側のbox回転で代用しない。
また、上流の非Lua branchを選ぶためにupTeX identityを偽装しない。

## source参照・改変追跡

派生sourceを追加するたび、次の表へ上流file、固定commit、対応範囲、PraTeX側の差分理由を記録する。
行番号は上流commitに固定した表示上の補助情報であり、識別のカノンはfile pathとcommit hashである。

| PraTeX側 | 上流 | 参照・移植範囲 | 状態 |
|---|---|---|---|
| `docs/prjlreq-provenance.md` | `LICENSE`, `README-ja.md`, `jlreq.cls` at `ac06fd0` | license、公開engine/class契約、最小移植順の監査 | 文書のみ |
| `tex/latex/pratex/prjlreq-UPSTREAM-LICENSE.txt` | `LICENSE` at `ac06fd0` | BSD 2-Clause全文 | 無改変収録 |

今後の派生codeについて、上流sourceを参照した箇所を「公開仕様だけからの独立実装」と
記載してはならない。逆に、PraTeX固有identity checkや既存`pratex-japanese`とのadapterは、
上流との差分として明示する。

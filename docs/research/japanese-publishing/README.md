# 日本語の論文・小説・出版実務調査

更新: 2026-08-24

## このfolderの目的

日本で実際にLaTeXを使って論文、学位論文、小説、エッセイ、文芸書を作る時のclass、package、
engine、DVI/PDF backend、周辺toolを、PraTeXの互換性計画へ再利用できる形で整理する。
「TeXは論文用」という前提は置かない。小説・エッセイ・長文出版をPraTeXの第一級用途として扱う。

これはPraTeXの現在の対応表ではない。各項目は、将来どの実在文書をoracle/fixtureにするかを決める
調査資料である。現在の実装状態は[`../../feature-inventory.md`](../../feature-inventory.md)を参照する。

## 読み分け

- [学術論文・学位論文の実務](academic-writing.md)
- [小説・エッセイ・文芸書の実務](fiction-writing.md)
- [DTP・word processorとの機能差](dtp-word-processor-gap.md)
- [互換性fixture matrix](compatibility-fixtures.md)
- [一次資料索引](sources.md)

## 調査上の二つの尺度

互換fixtureの優先度は次で表す。

| 優先度 | 意味 |
|---|---|
| P0 | 日本語の実用文書を名乗る前に必須。class全体と最小fixtureの両方を固定する |
| P1 | 代表的な出版・学術workflowを成立させるために高優先 |
| P2 | 分野別、旧文書、入稿先別の互換性を広げる段階 |

DTP/word processorとの機能差は次で表す。

| 評価 | 現行TeX/LaTeXでの実用性 |
|---|---|
| A | 標準的なclass/packageで容易かつ安定 |
| B | packageまたは特定engineで可能だが、合成・移植・tagging等に制約がある |
| C | 大きな自作macro、外部tool、手動工程が実質的に必要 |
| D | engine/backend/semantic API/対話APIの不足が支配的 |

「理論上TeXで描ける」はA/Bを意味しない。著者、編集者、製版担当が変更を反復し、誤りを検出し、
再現可能な成果物へ到達できるかで判定する。

## 基準にする実行profile

### 論文・学位論文

1. `(u)pLaTeX -> DVI -> dvipdfmx`
2. `LuaLaTeX + LuaTeX-ja + fontspec -> PDF`
3. `jlreq`、`BXjscls`、学会class等の複数engine対応profile

### 小説・エッセイ

1. `jlreq + upLaTeX + dvipdfmx`
2. `jlreq + LuaLaTeX + LuaTeX-ja`
3. 旧来文書用の`utbook/tbook + plext`と`ltjtbook + lltjext`

PraTeXは他engineのversion primitiveを偽装せず、PraTeX固有feature queryと個別互換契約を使う。

## 互換性判定を始めるgate

class/packageの最終互換判定は、少なくとも次が揃ってから行う。

- TeX--XeTを含むe-TeX完全対応
- upTeX上位互換の入力、font、JFM、縦横組、DVI意味
- LuaTeX級を目標にした直接PDFのfont、resource、link、metadata、graphics機能

それ以前にも公開構文、fixture、期待値、取得元、licenseを固定してよい。ただし失敗を
「package非互換」と断定せず、engine prerequisite不足と分けて記録する。

## 資材の扱い

- CTAN、作者公式repository、学会公式配布を一次資料にする。
- class/templateをrepositoryへ入れる前に個別licenseを確認する。取得限定の資材は試験時だけ取得し、
  URL、version/revision、取得日、SHA-256を記録する。
- 公式sample全体を通すgateと、原因を一つに絞った自作最小fixtureを分ける。
- DVIはopcode/font/sp座標、PDFはobject/resource/font/ToUnicode/page box/tagと描画を分けて比較する。
- 画像比較だけでfont選択や文字抽出の正しさを判定しない。

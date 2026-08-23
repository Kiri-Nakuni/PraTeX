# 拡張可能な文字分類器

更新: 2026-08-23

## 目的

通常の `catcode` とupTeX互換 `kcatcode` を、字句解析器から見て一つの意味へ統合する。
カノンはcatcode側の `InputCategory` であり、`kcatcode`は別の公開番号を持つ互換viewである。
生の公開整数をcastして内部IDにせず、それぞれのprimitive境界で意味へ写す。

| 公開入口 | 公開番号 | カノン分類への写像 |
|---|---:|---|
| `\catcode` | 0..=16 | `InputCategory::CatCode(CatCode)` |
| `\kcatcode` | 14 | 対象符号位置のUnicode catcodeへ委譲し、`CatCode`へ写す |
| `\kcatcode` | 15 | `InputCategory::RawBytes` |
| `\kcatcode` | 16..=20 | `InputCategory::Wide(Kanji/Kana/OtherKChar/Hangul/Modifier)` |

したがって `catcode=14` のcommentと `kcatcode=14` の委譲、`catcode=16` のnamespaceと
`kcatcode=16` のkanjiは、公開数値が同じでもカノン分類では混同しない。これはcatcodeと
kcatcodeを別domainに保つという意味ではなく、**二つの数値codecを一つの意味型へ入れる**
という意味である。`InputCategory`のRust discriminantをfmtやWASM ABIへ公開しない。

この`InputCategory`は**字句分類**であり、組版用の`ScriptClassId`ではない。Han/Kana/Hangulと
CJKV layout region、TeX `\language`を分ける規則は
[script境界組版roadmap](extensible-layout-roadmap.md)に置く。
異体字、IVS、外部文字集合、造字のidentityも字句分類とは別であり、
[文字・異体字・造字の内部表現](glyph-identity-roadmap.md)で扱う。

## 現在の組込み経路

組込み経路では `Eqtb` 自身がtraitを実装し、中間objectを作らず次を返す。

- byte: 最頻経路の `CatCode`
- Unicode: `InputCategory::{CatCode, RawBytes, Wide}`

ASCIIは従来と同じ256要素表を一回引くだけで、Unicode decoderもkcatcode表も呼ばない。
非ASCIIの正しい入力列を認識した時だけkcatcode表を引く。拡張規則を使わない通常実行に
optional providerの分岐を一文字ごとに置かない。

物理保存は当面二層を維持する。

- `CatCodes`: byte表と、符号位置ごとのUnicode catcode。`kcatcode=14`でない間も隠れ値を保つ。
- `KCatCodes`: Unicode block単位の互換route。block単位の局所/global復元を保つ。

ここを一表へ物理的に潰すと、隠れたcatcode値、保存stackの粒度、ASCII fast pathを壊す。
統合するのは**字句器が受け取る意味**であり、互換保存の粒度ではない。`InputCategory`は
一時値なのでfmtへ保存せず、現行の二表とtoken snapshotを保存する。

`ClassifierView` は一tokenの走査中に変わらないsnapshotとする。既に読んだtokenは後の
catcode/kcatcode変更で再分類しない。`SyntheticRescan`などが新しく読むときだけ現在のviewを使う。

`CatCode::public_number/from_public_number` と
`KCatCode::public_number/from_public_number` を別codecとして持つ。primitiveの入力、内部量、
表示、token packing、fmt decodeで列挙値への直接castを増やさない。

## 拡張時のdispatch

通常経路と拡張経路は、入力runを開始する外側で選ぶ。

1. built-in view: 静的dispatch。現在のfast path。
2. explicitly enabled view: 組込み表に加え、その実行にだけ渡されたproviderを参照。

Vaakの疑似callbackは、特定のVaak実行が能力を明示要求した時だけ2を作る。能力handleは
その実行のscopeを抜けたら失効し、engine全体の常設callback表には登録しない。providerが
提出する局所IDは`InputCategory`の番号ではない。中央registryが検証後に意味へ写し、fmtへ
run-local IDやcacheを保存しない。単純で呼出し回数の多い規則はVaak側でbatch化し、複雑だが
頻度の低い処理はversioned WASM ABIへ送る。どちらも一tokenごとのABI往復を標準経路にはしない。

## WASM ABIで固定すべき境界

- provider-local `u32` class/rule ID、`u32` code point、固定幅context ID
- byte列は `(handle, offset, length)` またはhost-owned opaque buffer
- Rustのenum表現、slice pointer、allocator、`Rc`を公開しない
- ABI version、要求capability、最大batch長、fuel/time上限をhandshakeする
- provider失敗時のfallbackと診断を明示し、途中までの分類結果を黙って採用しない
- callback能力はrun-localであり、fmtへ保存しない

## 回帰条件

- ASCII hot pathの出力hashとCPU/wall基準を変更前後で比較する
- 同じ公開数値14/16をcatcode/kcatcodeそれぞれのcodecで別の意味へ写す
- kcatcode 14は符号位置のcatcodeへ委譲し、15は元のUTF-8 byte列へ戻す
- kcatcode 16..=20は五つのwide分類へ写す
- kcatcodeが14でない間も符号位置別catcodeの隠れ値を保つ
- group/global/globaldefs/fmt後も既存catcode/kcatcode値が一致する
- CJK token、wide制御綴、namespace、`\ifcat` の既存試験を全て通す
- provider無効時のprovider呼出し回数は0である

実装は公開マニュアルとブラックボックス観測から行い、(u)pTeX/e-TeXの実装ソースを
参照・転記しない。

# 拡張可能な文字分類器

## 目的

通常の `catcode`、upTeX の `kcatcode`、将来の Vaak/WASM 規則を、字句解析器から見て
一つの問い合わせ面にする。ただし、互換性上異なる保存表と公開数値を物理的には混ぜない。

`catcode=14` は comment、`kcatcode=14` は Latin UCS、`catcode=16` は vaak-rtex の
namespace、`kcatcode=16` は kanji である。同じ整数を同じ列挙値として扱うと既存入力の
意味が壊れる。このため内部の `CharClassId` は byte catcode、Unicode kcatcode、拡張
class を別領域に置く。これは crate 内部IDであり、将来公開するWASM ABIの値は別途
versionを付けて固定する。

この`CharClassId`は**字句分類**であり、組版用の`ScriptClassId`ではない。Han/Kana/Hangulと
CJKV layout region、TeX `\language`を分ける規則は
[script境界組版roadmap](extensible-layout-roadmap.md)に置く。
異体字、IVS、外部文字集合、造字のidentityも字句分類とは別であり、
[文字・異体字・造字の内部表現](glyph-identity-roadmap.md)で扱う。

## 現在の組込み経路

組込み経路では `Eqtb` 自身がtraitを実装し、中間objectを作らず次の二つを返す。

- byte: 最頻経路の `CatCode`。class IDは拡張経路で要求された時だけ組み立てる。
- Unicode: `UnicodeDisposition::{RawBytes, Wide}`

ASCIIは従来と同じ256要素表を一回引くだけで、Unicode decoderもkcatcode表も呼ばない。
非ASCIIの正しい入力列を認識した時だけkcatcode表を引く。拡張規則を使わない通常実行に
optional providerの分岐を一文字ごとに置かない。

`ClassifierView` は一tokenの走査中に変わらないsnapshotとする。将来provider結果を
block/batch単位でcacheする時は、中央registryがproviderごとの局所IDをglobal IDへ割り当て、
分類表の実変更で進む世代番号をcache keyに含める。このregistry・cache・世代は同じ
コミットで導入し、callback側へglobal ID constructorを直接公開しない。fmtにはcacheや
世代を保存せず、読込み後に空のregistry stateから始める。

## 拡張時のdispatch

通常経路と拡張経路は、入力runを開始する外側で選ぶ。

1. built-in view: 静的dispatch。現在のfast path。
2. explicitly enabled view: 組込み表に加え、その実行にだけ渡されたproviderを参照。

Vaakの疑似callbackは、特定のVaak実行が能力を明示要求した時だけ2を作る。能力handleは
その実行のscopeを抜けたら失効し、engine全体の常設callback表には登録しない。単純で
呼出し回数の多い分類はVaak側でbatch化し、複雑だが頻度の低い処理はversioned WASM ABIへ
送る。どちらも一tokenごとのABI往復を標準経路にはしない。

## WASM ABIで固定すべき境界

- `u32` class/rule ID、`u32` code point、固定幅context ID
- byte列は `(handle, offset, length)` またはhost-owned opaque buffer
- Rustのenum表現、slice pointer、allocator、`Rc`を公開しない
- ABI version、要求capability、最大batch長、fuel/time上限をhandshakeする
- provider失敗時のfallbackと診断を明示し、途中までの分類結果を黙って採用しない
- callback能力はrun-localであり、fmtへ保存しない

## 回帰条件

- ASCII hot pathの出力hashとCPU/wall基準を変更前後で比較する
- `catcode 14/16` と `kcatcode 14/16` が別IDである
- group/global/globaldefs/fmt後も既存catcode/kcatcode値が一致する
- CJK token、wide制御綴、namespace、`\ifcat` の既存試験を全て通す
- provider無効時のprovider呼出し回数は0である

実装は公開マニュアルとブラックボックス観測から行い、(u)pTeX/e-TeXの実装ソースを
参照・転記しない。

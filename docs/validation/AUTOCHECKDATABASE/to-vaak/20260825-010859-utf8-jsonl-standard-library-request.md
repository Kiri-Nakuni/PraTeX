# PraTeX → Vaak: UTF-8 JSON / JSON Lines標準ライブラリ要求

- date: 2026-08-25 01:08:59 +0900
- in_reply_to: `20260825-005101-pratex-embedding-status.md`
- related_request: AUTOCHECKDATABASE
  `20260825-004706-tex-engine-boundary-initial.md`
- target_branch: `codex3/perf-integration`
- target_commit: `6328456e69eccf00e377f072a3b33d11a4f2e124`
- target_layer: Vaak standard library / host I/O capability / build artifact

## 結論

Vaakの標準ライブラリへ、UTF-8 JSONとJSON Lines（JSONL）を読み書きする一般APIを追加してほしい。
PraTeX固有のbuild manifest型をVaakへ入れる要求ではない。AUTOCHECKDATABASEが要求するversion付き
build manifest、diagnostic配列、将来のsource-map artifactを、PraTeX／orchestratorが構造化byte列として
生成・検査するための汎用codecと逐次record境界が必要である。

この要求は、直前の「PraTeX phase/node統合について即時のVaak API変更要求はない」という連絡を
訂正するものではない。phase hot pathとは別の標準ライブラリ／tooling機能として追加を相談する。

## 必要な最小能力

### `PRATEX-VAAK-JSON-UTF8-001`

- 検証済みUTF-8 byte列からJSON valueを一つだけparseする。
- JSON valueをUTF-8 byte列へserializeする。writerはBOMを出さない。
- object、array、string、boolean、null、numberを区別する。
- requirement ID、byte offset、line/column、page/node ordinal等の整数を浮動小数点へ黙って丸めない。
  初版で全JSON numberを表せない場合は、受理範囲と範囲外errorを公開契約にする。
- parse errorはstable category/code、入力byte offset、可能ならline/columnを持つtyped errorにする。
- trailing non-whitespace、壊れたescape、unpaired surrogate、無効UTF-8、深すぎる入れ子、上限超過を
  panicや部分valueではなくerrorにする。
- serializerのobject順序は非決定的なhash iterationへ依存させない。key順を保存するかsortするかは
  Vaak側で決め、同じvalue／同じoptionから同じbyte列を得る決定性を契約する。

### `PRATEX-VAAK-JSONL-STREAM-001`

- 全fileを一度に保持せず、一recordずつparse／serializeできるreader/writerを持つ。
- readerはLFとCRLFを受理し、最終recordだけは末尾LFなしでも受理する。
- 空行または空白だけの行をvalueとして捏造せず、初版ではrecord errorとして位置を返す。
- writerは一valueを完全にencode・上限検査した後、UTF-8 JSONとLF一つを一recordとしてhostへ渡す。
- record番号、record先頭byte offset、record内error位置を再照合できる。
- 一record、総読込量、文字列長、入れ子深さにhost指定の上限を適用できる。

正確な公開名とVaak value表現はVaak側で決めてよい。上記はfunction名の指定ではなく、観測可能な
意味とfailure境界の要求である。

## codecとfile権限を分ける

JSON/JSONL codec自体はVaak標準ライブラリに置く。一方、PraTeXへ埋め込んだVaak programへ任意pathを
直接開くprimitiveを無条件に公開しない。PraTeXまたは外部orchestratorがpolicy確認済みの
`ReadHandle` / `WriteHandle`相当を渡し、Vaak側reader/writerはそのbounded byte streamだけを使う。

- path解決、許可directory、symlink、overwrite、atomic publish、file modeはhostが所有する。
- handleはcapabilityとRunEpochに結び付け、fmt、TeX register、次run、別projectへ保存しない。
- write先はPraTeX sourceから自己承認できない。build manifest出力はCLI／orchestratorの明示許可を要する。
- codec error、host read/write error、budget超過、cancelを別のtyped failureにする。
- parse成功前の部分value、serialize失敗後の不完全recordを次段へcommitしない。
- network accessや子process起動をJSON標準ライブラリの暗黙動作にしない。

standalone Vaakで具体的なfilesystem APIを提供する場合も、codec／streamとpath-based I/Oを別module・
別capabilityにしてほしい。これによりPraTeX埋込みはcodecだけ、または承認済みhandleだけを公開できる。

## PraTeX側の利用予定

最初のconsumer候補は、組版phase callbackではなくbuild終了後の低頻度toolingである。

1. engine identity、input revision、能力profile、errors/warnings、出力path/hash、coverageを持つ
   version付きbuild manifest。
2. stable diagnostic code/category、source-relative path、span、severityを持つJSON配列またはJSONL。
3. 将来のpage/node/文字identityからsource spanへ戻るsource-map event列。

通常の日本語paragraph、glyph、script境界ごとにJSON codecやVaakを呼ばない。standard JLReq/JFMの
callback 0契約と性能gateを維持する。

## 必要な試験

- ASCII／多byte UTF-8／escape／NUL／改行を含むstringのround-trip。
- 無効UTF-8、壊れたescape、unpaired surrogate、number範囲、trailing garbageのerror位置。
- LF／CRLF／末尾LFなし、空行、chunk境界がUTF-8 scalar・escape・number途中に来るJSONL reader。
- 同じvalueの決定的byte出力と、一record一LFのwriter。
- 深さ、record byte数、総量、cancel、host read/write失敗でpanic・部分commitがないこと。
- 二つのengine/project/runでhandle、buffer、diagnostic位置が混ざらないこと。

## fallbackと未決事項

- Vaak codecが未提供の間、PraTeX production CLIがJSON対応済みとは表示しない。
- build manifest schema、diagnostic schema、source-map schemaはPraTeX／AUTOCHECKDATABASEとの別契約であり、
  Vaak標準ライブラリがPraTeX固有fieldを所有しない。
- JSON numberの内部表現、object key順、duplicate keyの扱い、canonical JSON mode、incremental parserの
  suspension形式、standalone Vaakのfile capability UIはVaak側との設計相談が必要である。
- 初版は同期bounded streamでよく、PraTeX live-node phase中のMaySuspendと結び付けない。

## 権利境界

この連絡は要求、不変条件、failure条件だけであり、PraTeXのGPL-3.0 sourceやtest本文をVaakへ
転記しない。Vaak側はMITの標準ライブラリとして自身の設計と試験から独立実装する。

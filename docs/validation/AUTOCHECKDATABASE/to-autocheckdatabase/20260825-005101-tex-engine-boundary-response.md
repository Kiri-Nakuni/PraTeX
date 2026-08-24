# PraTeX → AUTOCHECKDATABASE: TeX engine境界への初回回答

- date: 2026-08-25 00:51:01 +0900
- in_reply_to: `20260825-004706-tex-engine-boundary-initial.md`
- target_branch: `codex3/perf-integration`
- target_commit: `b435e1df247191d998fa2afb8608bb481750a170`
- target_layer: source / typesetting / output / build

## 結論

三requirementsは、PraTeXの現行production CLIが満たす能力としてはまだ公開していない。
個別の試験runnerにはversion付きJSONとbinary・fmt・出力hashを保存する先例があるが、一般文書用の
build manifestではない。stable diagnostic code/source spanと、出力から入力へ戻るsource mapも未実装である。
したがって、現在の能力profileではこれらを「対応済み」または`NOT_REFUTED`として扱わず、
未実装・被覆ゼロとして渡してほしい。

PraTeXをLuaTeX、pTeX、upTeX等へ偽装する予定はない。engine identityの正本はPraTeX固有の
`\pratexversion`であり、version 1のrelease gateを満たすまでは値0を維持する。

## requirementごとの現在地

### `ACDB-PRATEX-BUILD-MANIFEST-001`

状態: **一般CLIでは未実装**。

- TRIP runnerは`source-record.json`、`stage1.json`、`stage2.json`、`comparison.json`へ取得資材、
  入力、binary、DVI、比較結果を記録する。
- package互換runnerは各sessionの`result.json`へbinary/fmt hash、exit、log error、DVI hashを記録する。
- どちらも固定fixture専用の検証artifactであり、任意文書のprofile、warning、coverageを表す共通schemaではない。

一般build manifestの生成主体は未決である。現行の固定試験では外部runnerがpass列とartifactを
所有しているため、一般化する場合もorchestratorが最終manifestを集約し、PraTeXがengine固有ID、
能力profile、各runのstructured diagnostic、出力identityを供給する案がある。ただし、この分担、
schema version、freshnessはいずれもまだ正本化していない。

### `ACDB-PRATEX-DIAGNOSTIC-SPAN-001`

状態: **未実装**。

現行CLIのlogは人間向けであり、公開されたstable code/category付きJSON diagnostic入口はない。
file/line文脈を表示する診断はあるが、安定codeや全診断のline/column・byte spanを契約していない。
`docs/incremental-tooling-roadmap.md`のH1にstructured diagnostic/semantic eventを置いているが、
これは設計であってproduction能力ではない。unsupported、build failure、検査不能を機械的に分ける
共通categoryも今後のschema設計が必要である。

### `ACDB-PRATEX-SOURCE-MAP-001`

状態: **未実装、被覆ゼロ**。

DVI/PDF出力、限定したPDF `ToUnicode`、文字identityの設計は存在するが、page/node/glyphから
入力source spanへ戻るversion付きartifactはない。研究roadmapにsource spanとpage boxの対応案はあるが、
実装済み能力表へは数えていない。現時点のmanifestを試作する場合はsource-map capabilityをfalse、
coverageを0として明示するのが正しい。

## 確認事項への回答

1. build manifestに近いものは**固定試験runnerには既存、一般CLIは未決・未実装**である。
2. stable diagnostic codeとsource spanを機械取得する公開入口は**現在ない**。
3. format生成はPraTeXを`-ini`で明示起動する。run中のTeX Live資材探索と`ls-R`/Kpathsea境界、
   DVIまたは直接PDF生成はengineが所有する。DVI後段driver、複数pass全体、索引tool、最終manifestの
   所有契約は未決である。現行の固定試験では外部runnerがこれらを統括するが、一般orchestratorはない。
4. 現在能力の正本は`docs/feature-inventory.md`、生きた引継ぎは`docs/HANDOFF.md`である。
   e-TeX/TeX--XeTは`docs/etex-texxet-status.md`、package実測は
   `docs/package-compatibility.md`、日本語組版の長期順は`docs/japanese-typesetting-roadmap.md`、
   性能条件は`docs/performance.md`を参照してほしい。
5. PraTeX側で自作したfixtureのblack-box観測結果をAUTOCHECKDATABASEが読む運用で問題ない。
   fixture本文を別repositoryへ複製・再配布する場合はGPL-3.0等、そのfileのlicenseを維持する。
   公開仕様、自作最小fixture、公式binaryの許可されたblack-box観測は利用できるが、他engineの
   sourceや上流testの移植・翻訳をPraTeXへ要求しないでほしい。

## 証拠

- `docs/feature-inventory.md`: 実装／部分／表面のみ／未実装／設計のみを分離した能力表。
- `docs/trip-testing.md`: 固定TRIP fixtureのJSON artifactと比較契約。
- `docs/package-compatibility.md`: 固定package probeの`result.json`契約。
- `docs/incremental-tooling-roadmap.md`: structured diagnosticとsource spanを将来設計として分離。
- `docs/versioning.md`: `\pratexversion`とrelease gateの正本。
- `AGENTS.md`: clean-room、engine偽装禁止、権利境界。

## 未検証事項

- AUTOCHECKDATABASE提案schemaを実corpusの複数pass buildへ適用した試作はない。
- CLI diagnosticのうち、現在すでに保持しているfile/line情報の被覆率は測っていない。
- source mapのobject identity、source edit後のstaleness、DVI/PDF間の共通座標は未設計である。
- 一般orchestratorとengine内structured eventのschema/version ownershipは未決である。

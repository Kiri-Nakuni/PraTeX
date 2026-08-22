# UTF-8を保つ文字・異体字・造字の内部表現

更新: 2026-08-22

## 結論

嘘字方式と超漢字/TRONの考え方はPraTeXに組み込める。ただし、入力を
TRON codeやfont slotへ変換するのではない。目標とするPraTeX既定profileは最初から
最後までstrict UTF-8とし、Unicodeで表せる文字とIVSを第一の交換表現にする。
Unicode scalarで表せない文字だけでなく、Unicodeまたは登録済みIVSだけでは
losslessに区別できない異体も、**版の固定された文字集合namespaceと面内IDの組**で表す。

文字の同一性、異体関係、組版上の文字分類、特定fontのglyphを別domainにする。
見た目が同じだけで文字を同一化せず、同じ文字のglyph差を別文字とも断定しない。

これは設計・導入roadmapであり、現在のproductionに新しい文字identity型、IVS shaping、
TRON/FontSlot importer、造字rendererが実装済みという意味ではない。

## 分けるべき八層

| 層 | 内容 | 永続identityにするか |
|---|---|---|
| source bytes | 入力byte列とsource range | epochのsidecarにexact byte列として保存 |
| input profile | PraTeX strict UTF-8、e-upTeX互換、Web2C TCX 8-bit | decoder/translationの契約だけ |
| TeX token | Unicode scalar/legacy code point/byte、catcode/kcatcode、control sequence | TeX実行のidentity |
| resolved character | Unicode scalar/IVS、またはnamespaceつき文字ID | 文書内の意味上のidentity |
| unresolved artifact | 未解決font slot、composition/IDS recipe | losslessな入力記録。semantic equalityに使わない |
| variant relation | 異体、同形、簡体/繁体、誤字、近似等の有向関係 | identityと別のprovenanceつきgraph |
| layout class/context | `ScriptClassId`、`LanguageRegion`、JFM class | 組版規則の入力だけ |
| rendered glyph | font fingerprint、variation instance、GID、position | 実行中の出力結果だけ |

OpenTypeのGIDはfont版やsubsetで変わる。GID、CID、TFMの8-bit slot、嘘字方式の
`(font, slot)`をfmtやWASM ABIの文字identityにしない。

## resolved identityとlossless artifact

通常のUnicode scalarは小さなtagged integerとしてlist内にinlineし、一文字ごとのhash lookupと
mutable internerを避ける。IVS、外部文字、局所外字だけを有界なappend-only/segmented arenaに
internし、`AtomRef(u32)`相当で参照する。Rust enum layoutやrun-localの数値は公開ABIと
fmtに保存せず、dump時はversionつきdescriptorを書き、undump後に新しくinternする。
元source byte列はnodeごとにcloneせず、epochのsource bufferとrange sidecarにだけ保ち、fmtへ入れない。

resolved identityは次の四種に限る。

1. inline `UnicodeScalar { value }`
2. `UnicodeSequence { kind, scalars }`
   - 登録済みIVS/standardized variation sequenceを第一の対象にする。
   - 元のsequenceを保存し、暗黙のNFC/NFKCやvariation selector削除をしない。
3. `ExternalCharacter { registry_key, repertoire_version, data_hash, code }`
   - TRONの「面＋面内code」に対応する一般化形である。
   - `registry_key`は衝突しないURI/UUID等、`code`は有界のbyte列にする。
4. `LocalCharacter { set_uuid, revision, local_code }`
   - 文書や組織が管理する外字。PUAを使う場合もこのmappingを伴わせる。

次はresolved identityではなく、入力を失わないための別domainに置く。

- `CompositionArtifact { registry_key, syntax_version, recipe, digest }`
  - 造字、未符号化漢字、IDS等の記述。recipeは説明・検索keyであり、
    一意な文字同一性や自動描画を単独で保証しない。
- `UnresolvedImportRef { scheme, font_fingerprint, slot, import_provenance }`
  - 嘘字方式等の入力adapter用。Unicode/IVS/external IDへ解決できた場合はそちらを保存し、
    解決できない場合はこの参照を保存する。semantic equality、hyphenation、正確なUnicode抽出に
    使わず、byte-exact descriptor equalityだけを定義する。

UnicodeのPUA scalarを直接UTF-8で入力することは禁じないが、それだけでは私的合意の対象を
特定できない。特定のglyphや外字として再現するには、`set_uuid`、版、mapping hashを
伴う明示的な`PrivateUseMap`を要求する。

## 三つの入力profileとTCX

入力byte列はどのprofileでもsource sidecarにexactに保存する。decoderの契約は次の三つを
推測で混ぜない。

1. `PraTeXUtf8`（目標とする既定）
   - strict UTF-8だけを受理し、surrogateを拒否し、U+10FFFFを含むUnicode scalar valueへする。
2. `EuptexCompat`
   - 現在のproductionが使う互換decoder。overlongとsurrogateを意図的に受理し、
     U+10FFFFは受理しない。scalarでない値は`UnicodeScalar`と呼ばず、
     `LegacyCodePoint`として互換経路に閉じ込める。
3. `Web2cTcx8Bit`
   - TCXの外部byte変換を行ってから0..255のTeX内部byteをscannerへ渡す。UTF-8 decodeはしない。

`PraTeXUtf8`での既定pathは次である。

```text
UTF-8 bytes -> strict decoder -> Unicode scalar tokens -> macro expansion
            -> text/glyph node construction -> font mapping/shaping -> DVI/PDF event
```

現行の`EuptexCompat`から`PraTeXUtf8`へ既定を変える時は、互換profileを残したうえで
fmt/input optionと診断を明示する。現在の挙動をstrict UTF-8実装済みと表示しない。

利用者が指していたのはWeb2CのTCX（TeX character translation file）である。TCXは
TeX82の`xord` / `xchr`の考え方を実行時fileで差し替える互換機構で、
8-bitの各外部code `src`に0..255の内部code `dest`を一つ割り当てる。複数の`src`が
同じ`dest`へ行く多対一は可能であり、数学的な一対一写像ではない。
互換状態は入力用`xord[256]`、外部出力用`xchr[256]`、診断用`xprn[256]`の三表として保ち、
最終`xord`から他の表を逆算しない。一の入力codeから複数文字への変換、Unicode sequence、
異体関係、glyph identityを表す機構ではない。

PraTeXは`--translate-file=<name>` / `-translate-file=<name>`、optionと分離したname、および
先頭の`%& -translate-file=...`を互換test対象にする。`-8bit`とTCXの両方がある時の
printability、ASCII 32..126の予約動作、同じ`src`を再定義した最終値も固定する。
TCXは入力fileのraw byteに適用し、その後の内部byte列をscannerが`^^`処理する。
UTF-8 byte列の各byteにTCXを重ねない。明示TCXを選んだ実行だけ、run内の対象text inputを
`Web2cTcx8Bit`で読む。
このモードがUnicode外文字を表す場合は、TCXの`dest`だけを永続identityにせず、
font/encoding mappingと合わせたimport adapterからnamespaceつきdescriptorへ明示解決する。

TCX fileは新しい`FileKind::Web2c`相当としてTEXMFの`WEB2C`系pathからresolverで探し、
行長・file size・entry数を固定してparseする。現行fmtには全体magic/schema versionがないため、
TextAtomやinput profileを永続化する前に、全体schema version、section上限、旧fmtの拒否/移行方針を固定する。

Web2C互換profileはfmtに`xord/xchr/xprn`のdefaultを保存し、実行時の明示TCXまたは`-8bit`が
あればWeb2Cの順序で上書きする。profileをbyte列から推定したり、TCXを指定せずに
UTF-8とTCXを同時適用したりはしない。Web2C 2026 manualにはINITEXとTCXについて矛盾した
記述がある。TeX Live 2026の現行binaryをblack-box観測した範囲では、CLI指定と
first-line parsingのどちらでもINITEXがTCXを読みfmtへ保存した。実装時はこの観測を
独立fixtureとして再現し、manualの一文だけから逆の契約を決めない。

`^^` はTCXと別である。catcode 7の反復をscannerがtoken確定前に入力buffer上で
変換する字句escapeであり、glyph namespaceではない。TeX82互換の`^^`と将来の
`^^^^` / `^^^^^^`はUnicode scalarを入れるescapeに限定し、外字IDの運搬に流用しない。

直接のUTF-8 IVSはbaseとvariation selectorを別々のUnicode scalar tokenとして読む。macro delimiter、
catcode、`\string`等のTeX意味は変えず、macro展開後にtext/glyph nodeを作る段だけで
base+VSを一のresolved atomへ束ねる。未登録IVSも入力を捨てず保存し、interchangeに使う
場合の診断とfont fallbackを別に行う。Unicode外文字の初版入力は、例えば
`\pratexglyph{registry}{code}`のような明示構文にする。実際のprimitive名とdelimiterは実装段で
固定する。超漢字の`&Txxyyyy;`はUTF-8/ASCII textに外部IDを保存するimport/exportの
参考になるが、TeXの`&`はalignment tabであるため通常scannerで暗黙解釈しない。

## IVS、IDS、造字の境界

- 登録済みIVSはbase ideographとvariation selectorのUnicode sequenceとして保ち、
  IVD collection名と版を検証用metadataにできる。
- OpenTypeは`cmap` format 14でUnicode variation sequenceからglyphへmappingできる。
  OTF対応ではこれを先に実装し、GIDをtext identityに逆輸入しない。
- IDSは未符号化表意文字の説明と検索に使える。しかしUnicodeは一意な記述、
  semantic identity、同値関係、合成描画を保証しない。そのためIDS byte列だけを
  canonical文字IDや必ず描画すべきrecipeにしない。
- 実際に造字を描画する場合は、名前つきregistry、固定specification version、
  bounded recipe、metric、fallback glyph、権利provenanceを一体で登録する。

## 超漢字と嘘字方式から採るもの

超漢字/TRONは、TRON code自体を個別文字対応ではなく、文字集合を取り込む
「器」と説明している。ここから次を採る。

- namespaceつき外部文字集合と面内codeを分ける。
- 文字集合、対応font、検索・異体関係dataの版を別々に固定する。
- 非対応環境でもIDを失わないASCII形式のimport/exportを持つ。

一方、statefulな「現在の面」切替えは内部表現に採らない。読込み時に各文字を
`{registry, version, code}`へ完全にし、random access、macro copy、incremental checkpoint、
fmtで直前の面状態を再現する必要をなくす。

嘘字方式は、複数fontの同じJIS slotへ異なるglyphを置き、`(font name, code point)`で
文字を選ぶ。既存資産のimportには必要だが、そのままではfont名変更、版差、subset、
置換、検索を安定させられない。入力adapterとしてのみ保ち、可能な限り別の
canonical descriptorへ変換する。

## 組版・font・PDFへの接続

wide glyph nodeは、inline Unicode scalarまたは`AtomRef`のtext identity、
`ScriptClassId`、`LanguageRegion`、font use contextを分けて持つ。未解決artifactは別variantにする。
sourceのexact byte/rangeはnodeではなくsource-occurrence sidecarから追跡する。registry snapshotは
epoch中immutable、atom arenaは有界なappend-onlyとし、文字列やdescriptorを毎回cloneしない。

font resolverは次の順に試す。

1. 明示されたfontのUnicode scalarまたはIVS `cmap`
2. external/local registryが固定したfont fingerprintとmapping
3. 明示的に許可されたregion/script fallback
4. missing-glyph nodeと再現可能な診断

JFMはglyph mappingではなくmetric/class lookupとして上記の後段に置き、font `cmap`の役割と
混ぜない。font registryは利用権限とPDF内での配布権限も別に持つ。

```text
FontRightsPolicy {
  local_use,
  embed_full,
  embed_subset,
  redistribute,
  provenance
}
```

不明な権利を許可と推定しない。local DVI/displayで使えるfontでも、`embed_full`または
`embed_subset`が明示的に許されなければ直接PDFに埋め込まない。

PDF `ToUnicode` / `ActualText`は、Unicode/IVSなら元sequenceを使う。外部文字に検証済みの
exact Unicode mappingがある場合だけそれを使う。近似字、IDS、見た目が同じPUAを
正確なUnicodeだと偽らない。mappingがなければdiagnosticと別metadataに外部IDを残し、
関係ないUnicodeを`ToUnicode`へ書かない。

## registry、Vaak、WASM

registryはhost-ownedのimmutable tableとし、次を一括登録する。

- registry key、repertoire version、manifest/data hash、license/provenance
- codeとUnicode/IVS/external identityのexact mapping
- 方向つきvariant relationとその種類・根拠
- font fingerprint、metric、glyph mapping、fallback policy
- 必要な場合だけbounded composition recipe

一文字ごとにVaakやWASMを呼ばない。対応表、variant graph、単純な造字recipeは
Vaakの明示capabilityで一度だけuploadし、hostが検証・compileしたtableをsafe Rustで引く。
複雑だが低頻度の検索、mapping、合成だけをversioned WASM ABIへbounded batchで渡す。
明示したVaak実行がcapabilityを要求した時だけ登録を認め、handleはrun-local、
scope-localとしてfmtに保存しない。

fmtに永続化する外部descriptorはregistry key、version、hashを持つ。undump時に同じ
registryが無い場合は、別版で推測解決せずunresolved identityのまま保ち、描画時に明示診断する。

## incremental/LSPとsecurity

`RunEpoch`に使用registryのkey/version/hashを含める。registry差替えは新epochでのみ反映し、
旧新のvariant graphやfont mappingを混ぜない。LSP semantic eventはsourceのexact byte rangeと
入力profileを、resolved character descriptorへ結び、未解決・近似mapping・missing glyphを
確定済みと表示しない。`PraTeXUtf8`のときだけ、そのrangeをUTF-8 scalar rangeとしても公開する。

- registryのentry数、ID/recipe長、relation数、IDS/composition深さに上限を置く。
- recipe graphのcycle、重複ID、不正Unicode scalar、未登録IVSを検証する。
- hashはalgorithm、digest長、元data長をmanifestで固定し、同一digestでも必要な場合は
  bounded content比較で確定する。「hash衝突が無い」とは仮定しない。
- font、outline、PostScript、registry内のscriptを実行しない。
- networkから実行中にregistryを取得せず、orchestratorが事前にhash固定する。
- unknown identityでpanicしたり他のglyphへ黙って置換したりしない。

## 導入段階

| 段 | 内容 | 完了条件 |
|---|---|---|
| G0 | 用語、型境界、registry manifest、権利表を固定 | 文字/glyph/layoutを同じIDで表さない |
| T0 | 全体fmt schemaと三input profile、TCXの`xord/xchr/xprn`、`-8bit`、resolver | strict UTF-8はscalarだけ、e-upTeX互換値を混ぜず、TCX無効時に分岐・allocation追加0 |
| G1 | wide glyph node、inline scalar/`AtomRef`、Unicode/IVS descriptor | scalar tokenを結合せずmacro意味を保ち、node/list/fmtで元sequenceが失われない |
| G2 | OTF `cmap` format 14、PDF `ToUnicode`、DVI/PDF event | IVS fixtureの字形とtext extractionを別々検証 |
| G3 | external registryと名前つきexplicit input/import | 同codeの別registryを混同せずfmtを往復 |
| G4 | 嘘字/font-slot、TRON `&T`、PUA mapping importer | inputのみで正規descriptorへ変換、未解決も保存 |
| G5 | provenanceつきvariant relation graphと照会 | relationをidentity/equalityとして使わない |
| G6 | bounded composition registryとoptional renderer | IDS単独で自動同一化せず、cycle/limit/fallbackを検証 |
| G7 | Vaak table uploadとWASM batch adapter | 無効時call 0、per-glyph ABI call 0、trapでatomic fallback |

G1/G2は[script境界組版roadmap](extensible-layout-roadmap.md)のR2と同じwide glyph基盤で行い、
別のnode列を作らない。嘘字/TRON importerをUnicode/IVSより先に実装しない。

## 権利とclean-room境界

- Unicode specificationから意味を実装し、IVD等のUnicode data filesを同梱する場合は
  対象fileのUnicode License v3 noticeとversion/hashを保つ。code chartからglyph/font dataを抽出しない。
- TRON/BTRONの公開説明からnamespace構造を独立実装しても、個別の割当表、辞書、
  異体関係DB、product codeを無許諾で転載しない。
- Tフォントは無改変の再配布も非営利条件であるためGPL repositoryにvendorしない。
  表示・印刷の利用許諾だけをPDF内のfont再配布許諾とみなさない。full embeddingは再配布、
  subsetは改変・再編に当たり得るため、利用者が導入済みでも別の許諾が確認できなければ
  PraTeXの直接PDFでfull/subset embedしない。local参照とPDF配布を`FontRightsPolicy`で分ける。
- 今昔文字鏡/嘘字方式は技法上の入力adapterとして独立実装する。font、glyph、
  文字鏡番号対応表、検索DB、異体関係dataは個別licenseを確認せずに取り込まない。
- (u)pTeX/e-upTeXの文字node実装も転記・翻訳せず、公開manualと許された
  black-box観測からPraTeXのnodeと出力eventを書き直す。

## 一次資料と補助資料

- [Unicode Character Encoding Model: characters versus glyphs](https://www.unicode.org/reports/tr17/)
- [Unicode Ideographic Variation Database / UTS #37](https://www.unicode.org/reports/tr37/)
- [Unicode Core Specification, Ideographic Description Characters](https://www.unicode.org/versions/Unicode17.0.0/core-spec/chapter-18/)
- [Unicode private-use characters](https://www.unicode.org/faq/private_use.html)
- [OpenType `cmap`, format 14](https://learn.microsoft.com/en-us/typography/opentype/spec/cmap)
- [Unicode Terms of Use](https://www.unicode.org/copyright.html)
- [超漢字: TRONコードとは](https://www.chokanji.com/ckv/manual/01-07-01.html)
- [超漢字: 言語指定コード](https://www.chokanji.com/ckv/manual/01-07-02.html)
- [超漢字: テキスト形式TRONコード](https://www.chokanji.com/ckv/manual/01-07-03.html)
- [超漢字検索のUnicode IVS対応](https://www.chokanji.com/ckk/ckkivs.html)
- [Tフォント利用規定](https://charcenter.tron.org/tfont/license.html)
- [Web2C manual: TCX files](https://tug.org/texinfohtml/web2c.html)
- [TeX82 `tex.web`: TCXの土台になった`xord` / `xchr`](https://cs.stanford.edu/~knuth/programs/tex.web)
- [JAGATによる文字鏡関係者への取材（嘘字方式の補助資料）](https://www.jagat.or.jp/past_archives/story/836.html)

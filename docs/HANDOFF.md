# PraTeX 作業引継ぎ

更新: 2026-08-24（機能枝 `codex2/etex-showtokens`、統合枝 `codex2/jlreq-script-spacing`）

この文書は、現在の Codex セッションから別のエージェントへ作業が移っても、
検証済みの境界と未commitの作業を失わないための生きた引継ぎである。
各作業単位が完了するたびに、commit、試験結果、残件を更新する。

## 最初に守ること

1. リポジトリ直下の `AGENTS.md` を最初から最後まで読む。
2. 現在の統合枝は `codex2/jlreq-script-spacing`。`main`と歴史的`full`を汚さない。
3. 横組JFM、K/X、WASM仕様、plain DVI回帰、実時刻、日本語CID PDFの統合済み差分を
   無関係な変更として差し戻さない。日本語CID PDFの検証済み元commitは`8035d1c`である。
4. 通常実装と性能調整はsafe Rustだけで行う。
5. pTeX/upTeX/e-TeX/pdfTeXの実装sourceや上流testを移植しない。公開manual、公開file format、
   自作最小入力による公式binaryの黒箱観測から独立実装する。
6. 標準日本語組版はengine coreの一級機能であり、通常経路をVaak/WASM callbackへ逃がさない。
7. commitは日本語で「なぜ分けたか」を書き、試験名も日本語にする。

## 枝と共有状態

- 現在の枝: `codex2/etex-showtokens`
- push済み統合checkpoint: `cfa6ece`
- local機能checkpoint: `6b03b70`（具体的payloadへのremote push承認待ち）
- push済み`\scantokens` code checkpoint: `d90e98f`（歴史的基点は`6ce8315`）
- 日本語CID PDFの検証済み元commit: `8035d1c`
- 基点のrelease全suite: **564 passed、0 failed、6 ignored**（Vaak `89804b4`）
- 最新の記録済みrelease全suite: `6b03b70`で**857 passed、0 failed、10 ignored**。
  plain DVI byte回帰、`\showtokens`、NFSS、fmt索引改善、WSL発見失敗cacheを含む。
- 直近の共有commit:
  - `6b03b70`: show診断の既存終了status差分を試験に固定しない
  - `46ff19a`: 未展開token列の表示をgeneral textとJFM境界へ接続
  - `cfa6ece`: global制御綴名をfmt読込時に二度hashしない
  - `fe9fa5c`: 別engine名を使わず本文・見出しJFM/VFのno-copy TeX Live gateを固定
  - `4acf8a8`: 発見不能なWSL backendをoptional lookupごとに再起動しない
  - `3a4aaaf`: fmt予約A/Bを再検証できるraw標本と予約上限を固定
  - `22a8bdd`: fmt復元collectionの再確保をboundedな初期予約で削減
  - `00dc469`: relation消費と和文JFM同期を初期化時期に依存しない境界へ修正
  - `d90e98f`: KOMAが必要とする`\scantokens`をtyped疑似入力へ接続
  - `9118097`: JFM/K/X/最小禁則finalizerを実hlistへ統合
  - `801784a`: PraTeX自身のWASI target生成結果と未達ABI境界を統合
  - `51b1e95`: 横組JFM glyphのnamed CID PDF基線を統合
  - `0f3c51e`: run単位の実時刻をTeX/DVI/PDFへ統合
  - `4f832b1`: 横組JFMからwide glyphとDVIまでを統合
  - `6d4ff82`: 完成前の版1を避け、`0.1.0-dev`とrelease gateを分離
  - `e57a4f6`: `prjsarticle`と`\maketitle`の実DVI基準を統合
  - `d2807f8`: 選択済み横組JFMの和文を段落開始時にも保持
  - `41408dd`: `c9bd240`までのVaak・plain回帰を横組glyph枝へ統合
  - `be719da`: 横組JFMをtyped current font、wide node、DVIまで接続
  - `9587e25`: `\mutoglue` / `\gluetomu`
  - `43cd6c9`: e-TeX機能一覧の同期
  - `eda7dd1`: `\scantokens` clean-room設計
  - `df6e71e`: 本引継ぎ文書とREADMEからの導線
  - `961920e`: `ls-R` safe Rust実験の結果と再現限界
  - `fb58687`: Unicode欧文の契約とLaTeX停止理由
  - `d0b6f46`: `\kanjiskip` / `\xkanjiskip` core設計
  - `565c0d3`: Unicode欧文token、u16 hyphen trie、`\lastnodetype` page状態
- 旧性能枝 `codex/perf-wsl-euptex-safe` は `9bb6023`までpush済みで停止中。新しいfmt予約の
  checkpointは統合枝の`22a8bdd` / `3a4aaaf`に入った
- Claudeの連絡枝 `origin/claude/for-codex` は `82fa3a2`まで確認済み
- Vaak `origin/codex/main` は `64ccf4e`まで確認済み
- Vaakの壊れていた`full`は`codex2/full`の`7c5ccd7`で修復済み。release全suiteは
  **727 passed、0 failed、1 ignored**で、GPL側のPraTeX sourceは一行も移していない。

WASM module importとnamespaceの基本方針は
[`wasm-module-import-v0.1.md`](wasm-module-import-v0.1.md)を一次資料とする。ABI 0.0は
spacing/unit providerだけを扱い、任意control sequenceの実行ABIは未決定の別仕様である。

PraTeX自身は`67cd4bf`のまま`wasm32-wasip1`へcheck・binary linkでき、`pratex.wasm`と
`rtex.wasm`を生成した。実時刻slice `003c27c`も、WASI targetで`chrono/clock`を選ばずcheck・
linkできる。ただしruntime、preopen filesystem、子processを使わないresolver、clock、終了status、
nativeとのDVI適合試験は未完成である。compile成功をPraTeX 1のWASM gate達成とは数えない。
実測と次の境界は[`wasm-target-status.md`](wasm-target-status.md)に記録した。

## 完了済み: upTeX `latin_ucs` とUnicode pattern

状態: **`565c0d3`で実装・commit済み**。safe Rustのみ。

目的は、通常のTeX Liveで`latex.ltx`を作る際に
`dehypht-x-2024-02-28.pat`の`.buß3`で出る`Nonletter`を解消することである。
一時的に空の`hyphen.cfg`を使うと`latex.ltx`はerror 0でfmtをdumpしたため、このsliceの
hard blockerは未定義primitiveではなくStage 4cだった。`565c0d3`で解消済みである。

実装済みの範囲:

- U+0000..U+2E7Fのcatcode/lccode/uccode/sfcode表、group level、save stack、fmt
- ASCII/8 bit hot pathを従来の固定表のまま保つUnicode欧文token
- Unicode一文字制御綴とUnicode active文字の別identity、group、fmt、表示
- `\if` / `\ifcat`、case conversion、`\string` / `\detokenize`周辺
- hyphen exception、pre-trie、圧縮trieの文字を`u8`から`u16`へ広げる
- `.buß3`をbyte数でなく文字数として登録するprocess test

公式e-upTeX 2026への自作黒箱で確定した重要契約:

- table添字の上限はU+2E7F。U+2E80以上は`Bad character code`で添字0へ回復する。
- lc/ucの**代入値**だけは0..=U+2E80を受理する。U+2E80は後続で停止し得るsentinelなので、
  Rust panicにせず明示境界として扱う。
- case conversionはbyteとLatinUcsを跨ぎ、元のcatcodeを保持する。
- catcode 1/2はmain実行時にgroupを開閉するが、通常のbalanced textやmacro replacement、
  undelimited macro引数では構造括弧にならない。raw tokenのbrace判定はASCIIだけにする。
- 一方、明示的な`scan_left_brace`はLatinUcs catcode 1を受け入れ、alignmentの`align_state`は
  LatinUcs catcode 1/2でも増減する。raw token、command実行、明示brace要求、alignmentの
  四つの意味を一つのpredicateへ潰さない。
- catcode 3/4/6/7/8はmath shift、alignment tab、macro parameter、super/subscriptとして実動する。
- catcode 0/5/9/10/14もescape、line end、ignore、space、commentとして実動する。
- kcatcode 14 + catcode 15はinvalid errorではなく和文tokenへfallbackする。
- Unicode active文字と同じ符号位置の一文字制御綴は別identityで、`\ifx`も偽。
- `\detokenize`はcatcode 12を返す。
- catcode 11/12の同一符号位置は`\if`が真、`\ifcat`が偽。

既知差分・後段の追試:

- e-upTeX固有の`\string`表示（U+0100が`^^@`、activeが`.6`等）は既知差分として
  後段に残してよいが、互換済みとは記載しない。
- namespaced Unicode activeはstore/fmt/searchの土台だけで、lexerと生成APIが未接続。
  Stage 4cの完了範囲に含めず、対応済みとは記載しない。

### 完了済み監査: LatinUcs command意味論

状態: **2026-08-22に読取専用監査を完了。source編集なし**。

公式e-upTeX 2026へ自作最小入力だけを与え、次を確認した。上流engine source/testは見ていない。

- raw構造braceはASCIIだけ、mainのgroup commandと必須`scan_left_brace`はLatin cat1/2を含み、
  alignmentのdeltaもLatinを含む。この四経路を一つのpredicateへ統合しない。
- Latin cat1/2はundelimited argument、balanced general text、`\def` replacementの構造braceに
  ならない。Latin cat1は`\def` replacement開始braceにもならない。
- non-INITEX `\patterns`のerror recoveryはLatin cat2で停止する。
- `\string`は現在のcatcodeにかかわらずcat12、Latin cat6の`\meaning`は文字を一度だけ表示する。

監査終了時の`cargo test --release --test latin_ucs`は16 passed、0 failed。最終実装では
17 passed、0 failedへ増えた。監査担当がrepo rootへ
作った`codex-latin-*` log/DVIは正確な対象だけ削除し、再列挙0を確認した。

監査終了時点の重要残件は、破損fmtが`LatinUcsToken(U+0000..U+007F)`を作れることだった。
runtimeでは同範囲はbyte token専用なので、undumpをU+0080..=U+2E7Fへ制限し、U+007F拒否と
U+0080受理を試験する。実装担当へ連絡済み。cat7/8の実数式、cat0..16 lexer行列、activeの
条件・表示・fmt、Unicode pattern照合・例外・fmtは後続のprocess-level回帰として残る。

### `latex.ltx` hyphen分岐の真因

Stage 4cの通常resolver実測では、LatinUcsを手で有効化すれば`.buß3`を越えたが、これは
production profileではない。公式e-upTeXのINITEX/preloaded既定値は次で、PraTeX側の既定を
LatinUcsへ変える理由にはならない。

| code point | `kcatcode` | `catcode` | `lccode` | `uccode` | `sfcode` |
|---:|---:|---:|---:|---:|---:|
| 223 | 15 | 12 | 0 | 0 | 1000 |
| 256 | 15 | 12 | 0 | 0 | 1000 |
| 8217 | 18 | 12 | 0 | 0 | 1000 |

公開hyphen wrapperは、`\kanjiskip`が無い非pTeX engineについてGreek chiが一tokenかでnative
UTF-8を判定する。PraTeXはCJK/wide tokenによりこの試験を真にする一方、Latinのkcatcode 15は
正しくraw UTF-8 byte経路なので、wrapperがnative patternを選んだ後`.buß3`で食い違う。
公式e-upTeXは`\kanjiskip`を持つためpTeX branchへ入り、EC patternを完走した。

従って真の次段は初期kcat/lccodeの改変や検出用stubではなく、group/fmt/組版意味を持つ一級の
`\kanjiskip` / `\xkanjiskip`実装である。将来のscript-pair抽象化をcore側に置き、通常の
日本語経路をVaak/WASM callbackへ逃がさない。

Stage 4c単体の完了条件は満たした:

1. focused unit/process testsとfmt roundtripが通る。
2. 明示的なkcatcode 14 fixtureで`.buß3`を一文字patternとして登録できる。
3. 特殊catcode、active/control分離、case cross-lane、detokenizeを黒箱期待値で固定する。
4. corrupt fmtと境界値でpanicしない。
5. ASCII pathへUnicode表引き・heap確保を増やさない。

通常resolverの無改変`latex.ltx`は、Stage 4cのLatin初期値を偽装して越えない。
本物の`\kanjiskip`を追加しpTeX branchへ入れる次段で再測定する。

2026-08-23の追測では、K/X追加後のrelease binaryへ通常の`hyphen.cfg`を含むCTAN fixtureを
与え、無改変のLaTeX2e 2026-06-01と`expl3-code.tex`を最後まで読み、`latex.fmt`を
error 0でdumpした。28,640個の複数文字control sequenceを保存し、process exitは0だった。
したがって上の記述はStage 4c当時の履歴であり、「再測定が未実施」という現在状態ではない。
ただし、これは一般のclass/package互換性、pTeX互換の日本語format、LaTeX DVIの完全回帰を
意味しない。

最終実測:

- debug全suite: 563 passed、0 failed、6 ignored（最終二修正前。二修正は個別回帰済み）
- `cargo test --release --locked --test latin_ucs`: 17 passed、0 failed
- `.buß3`は明示kcatcode 14 fixtureで完走。次のU+2019 `.af6ro’`はkcatcode 18なので本slice外
- 最小fmt: 102,891 byte → 419,593 byte（+316,702、約4.08倍）。dense Latin表の費用
- ASCII/NotCjk hot pathへ追加のkcat表引き・heap確保は入れていない。性能再測定は未実施

破損fmt監査で今回のtoken/trie境界は固めた。一方、既存の broader gapとして
`VariableLevels.escaped`と`ControlSequenceStore.escaped`の長さを`Eqtb::undump`で照合しておらず、
短い前者を後でindexしてpanicし得る。両型へcrate内の長さgetterを足し、undump直後に一致検査する。
`ControlSequence::Escaped(n)`と実CS数の包括的照合も後段である。

## 横組production接続中: `\kanjiskip` / `\xkanjiskip`

状態: **2026-08-24に通常glue parameter面とBuiltIn最小横組hybridを実装**。

- 文書: `docs/kanjiskip-core-design.md`
- INITEX既定0pt、通常glue parameterとしての代入・group、`\globaldefs`、算術、fmt、表示を
  既存のglue経路へ接続した。release focused testは3 passed、0 failed。
- K/Xはlist終端値で元のWideChar/Char/Ligature境界を再評価する。直結和和Kは
  寸法・伸縮・改行・DVIに効く一方、`\showbox`、`\lastskip`、`\lastnodetype`、`\unskip`から
  隠れる仮想nodeとして公式e-upTeX black-boxと照合済みである。X、JFM、禁則、将来の箱境界Kは
  material nodeとしてこの型へ混ぜない。auto switch、`xspcode`、`inhibitxspcode`もtyped eqtb、
  群・`globaldefs`、fmt、中央finalizerへ接続済みである。
- JFM/禁則はmain loopで早期挿入し、K/Xだけclose-timeで再評価するhybridへ接続した。
  `{}`、`\relax`、`\unskip`、`\message`、semi-simple group、`\showthe`、整数register代入を
  e-upTeX自作probeと照合し、途中で削除されたJFMをcloseで作り直さない。
- 最終形のKはwide glyphのbit＋hlist単位specをline breaker/packer/outputが仮想glueとして扱い、
  純和文のnode数を倍増させない。
- standard Japaneseは`BuiltInPtex`のmonomorphic core経路で、Vaak/WASM callは0。
  Hangul–Latin等は同じscript-pair機構へ後から載せる。

この段階はLaTeXのpTeX検出を変えるが、「日本語組版完成」や検出stubとは呼ばない。JFM/禁則の
box/disc・未検証command境界、完全JLReq、縦組、PDFまで連続して進める。内部tableはprovider-local IDと
engine内部IDを分け、標準日本語ではVaak/WASM call 0、組版中のallocation 0を保つ。

## 横組JFM glyphからDVIまでの最小基線

状態: **`codex2/japanese-glyph-dvi`でfocused checkpointを実装**。

- `\pratexjfont`を正規名、意味が一致する横組定義・選択だけ`\jfont` aliasとして、bounded JFM
  loader、TeX互換scale、current和文fontへ接続した。pTeX/upTeX version primitiveは偽装しない。
- current選択はgroupとfmtを通り、`zw`/`zh`はclass 0のscale済みmetricを返す。
- CJK tokenをUnicode、JFM class、width/height/depth/italicを持つ`WideCharNode`にし、hpack、
  line break、box表示、fmt、DVIまで通した。validなJFM選択時は外部vertical modeでも捨てずに
  paragraphを開始する。
- DVIは欧文fontと衝突しない256始まりのfont番号を使い、BMPを`set2`、補助面を`set3`で出す。
  合成fixtureでglyph間と後続ruleのsp座標まで解釈している。
- DVIに加え、明示named CID profileがある時だけBMP wide glyphをPDF Type 0へ出す最小基線を
  `codex2/pdf-japanese-cid`で追加した。JFM pair adjustment、K/X自動空白、bounded禁則を
  まずlist-closeで接続し、その後JFM/禁則をmain loopへ移した。`\tfont`、縦組、box/disc境界、完全禁則、
  埋込みPDF和文字形、OTF shapingは未実装で、横組smokeを日本語組版完成とは呼ばない。
- 同じ欧文plain fixtureを`origin/main`と比較し、BOP--EOPの183 bytesは差分0だった。

## 横組JFM wide glyphのnamed CID PDF基線

状態: **`codex2/pdf-japanese-cid`でproduction接続済み。全releaseは631 passed、0 failed、
7 ignored**。

- `--pdf-japanese-cid-profile=<path>`は明示物理pathを最大64 KiBで一回だけ読み、一file・一JFMの
  strict ASCII profileをfont定義時にJFM論理名へ結ぶ。`kpsewhich`や暗黙font探索はしない。
- PDF 1.4の非埋込みType 0/CIDFontType0、`UniJIS-UCS2-H`、Adobe-Japan1-4だけをこの型で
  固定した。BMP Unicode scalarだけを出し、非BMP、surrogate、profile欠損、JFM名不一致、
  byte/wide font種別不一致はtyped errorにする。tofuや他engine偽装へfallbackしない。
- JFM幅はhostの現在位置だけを進める。CID fontの`/DW`はprofile明示値で、JFMから`/W`を
  捏造しない。2 glyphの絶対`Tm`差をspからの固定小数変換で試験する。
- Type 0 fontは、contentへ書いたBMP UCS-2 source codeを元のUTF-16BE scalarへ戻す
  `/ToUnicode` CMapを持つ。CIDとの恒等写像は使わず、surrogate帯を除外する。pypdfと
  PDFiumで「ああ」へ抽出できることを独立確認した。
- Courierは`/F1`を保ち、Type 1とnamed CIDを同じfirst-use resource列へ置くため番号衝突しない。
  同一pageのCID resourceは一回だけ登録する。
- JFM/TFMにはoutline、bitmap、CID mappingがない。FontFileを埋め込まないこのsliceの表示は
  profileのBaseFontとpredefined CMapを解決できるviewerに依存し、portableな字形表示や
  全extractor互換を保証しない。OTF/RustyBuzz、embedded font、WASM module意味はこのsliceへ
  入れていない。
- focused oracleは`src/font_resources/named_cid.rs`、`src/pdf_cid_font.rs`、
  `src/pdf_backend.rs`、`src/pdf_document.rs`と`tests/pdf_japanese_output.rs`にある。

## 横組JFM/K/X/禁則hybridの最小production slice

状態: **list-close基線を`9118097`で統合後、2026-08-24にJFM/禁則をmain loopへ接続。
現checkpointの全releaseは846 passed、0 failed、10 ignored**。

- font load時にJFM class対glue/kernを選択sizeへscaleしたdense表へcompileし、wide nodeが持つ
  Unicode、font/metric ID、JFM classだけで中央plannerを引く。同一fontのpair調整をKより優先し、
  異なるfont間は保守的にKへ戻る。
- JFM/K/X/禁則を由来付きGlue/Kern/Penalty nodeにする。JFM/禁則はmain loopで挿入し、
  close時はK/Xだけを除去・再評価するため、利用者が`\unskip`で消したJFMを復活させない。
  明示penaltyは境界に透明、明示glue/kern/math/whatsit/list/rule/disc等はbarrierである。
- hbox、段落、alignment cell、display math移行を`unsave` / pop前にfinalizeし、局所K/Xを
  snapshotする。unbox後の再評価、fmt後のJFM表再束縛、line break、DVI glyph/rule座標を
  合成JFM/TFMのproduction process試験で固定した。
- 禁則は`、。`とJLReq Appendix A.1/A.2由来の横組括弧12対だけのBuiltIn bounded subsetで、
  該当する行頭・行末禁止位置へpenalty 10000を置く。全JLReq classではない。
- ASCII-only listは一bit gateでplanner callback、JFM/provider表引き、追加allocation 0。
  標準日本語でもVaak/WASM registryを引かない。
- node-less境界の公式e-upTeX oracleは、class 0両側を持つ合成JFMで直結12.5pt、`{}` / `\relax`
  15pt、`\unskip`後12.5ptとなる。`\message`、semi-simple group、整数代入、`\showthe`も
  JFM二個・15ptで、main-loop provenanceのfmt/unhcopy往復を含む16試験へ固定した。
- 既知限界は、box/discと未検証commandのmain-loop境界、Kの完全な仮想event化、完全禁則、縦組である。
- spacing元枝の`cargo test --release --locked --no-fail-fast`は627 passed、0 failed、7 ignored。
  統合後は652 passed、0 failed、7 ignoredで、spacing process試験6件、glyph 5件、
  K/X parameter 3件もreleaseで全緑。
- origin/main固定fixtureとのplain DVI BOP--EOPは双方183 bytesでbyte差分0、body SHA-256
  `980ceaa638dd272dac0b46ec0870ac166715db10655b004917de96615396337a`。
- CTAN TRIPは両段exit 0、`tripos.tex`最小正規化後一致、DVIは既知正常hash
  `b20af20a1463c6846f0c4c1ce687cd6354ce1a5f65ee401507627570787ae9fe`を維持。

## 完了済み: `\lastnodetype` page状態

状態: **`565c0d3`で実装・commit済み**。release focused testは2 passed、0 failed。

修正前はpage上のpenalty後に空のnested `\setbox`を作ると、e-upTeXは13だがPraTeXは-1へ
化けた。page builderが`LastNodeInfo`だけを保持し、e-TeX node typeを同期していなかったためである。

変更:

- `Eqtb`にpage側のlast node typeを持たせる。
- node typeとlast glue/kern/penaltyの圧縮を一つのhelperへ統一する。
- page開始、page node更新、base vertical list復帰を同じAPIへ通す。
- `tests/etex.rs`に空list、type 1--6/8--13、page→nested box→page復元を追加する。

主なfile: `src/eqtb.rs`、`src/page_breaking.rs`、`src/semantic_nest.rs`、
`src/vertical_mode.rs`、`tests/etex.rs`。

Unicode差分も`src/eqtb.rs`を触るため、安全なcheckpointを優先して同じcommitへ収めた。

## 完了済み: `\savinghyphcodes`

状態: **2026-08-24にproduction接続済み。全releaseは846 passed、0 failed、10 ignored**。

- 正値の`\patterns`時に現在の小文字写像をlanguage別にsnapshotし、pattern trie圧縮後の
  通常hyphenationと`\hyphenation`例外へ使う。圧縮前の例外は従来どおり現在の`\lccode`を使う。
- 同じlanguageの後続する正値はsnapshotを置換し、0または負値では既存snapshotを消さない。
- e-TeX互換範囲は`[u8; 256]`、PraTeXのLatin-UCS拡張はU+2E7Fまでの別表とし、fmtで値、
  一意性、範囲を検証する。snapshotのない通常runは従来の`\lccode`参照へ戻る。
- 公式e-TeX manual §3.10と、SHA-256を固定したTeX Live 2026 e-upTeXへの自作probeだけを
  clean-room oracleにした。契約とhashは`docs/etex-savinghyphcodes.md`に記録している。

## 完了済みproduction slice: `\scantokens`と最小`scrartcl`

状態: **`d90e98f`で実装・commit・push済み**。

- 文書: `docs/scantokens-design.md`
- 設計commit: `eda7dd1`
- 実装commit: `d90e98f`

実装済みの境界:

- general textは`nested_scan_toks`で未展開のまま取り、外側の`def_ref`を壊さない。
- 文字byteと論理改行を分けた`PseudoText`を既存`LineLexer`へ一行ずつ渡す。
- `newlinechar`は生成時、`endlinechar`は各行読取時、catcode/kcatcodeは再字句化時の現在値を使う。
- 暗黙groupを作らず、KOMAが行う先頭`\endgroup`後の同一行`#`再分類を許す。
- 実fileと疑似fileの自然EOFだけで`\everyeof`を一度挿入し、`\endinput`では挿入しない。
- nested line number、tracing開始時snapshot、fmt往復、16 MiB/source・100万行/source・
  64 MiB/liveの明示上限を持つ。

公式KOMA-Script 3.49.2、Babel 26.9、hyph-utf8英語patternと試験生成した英語aliasの
`language.dat`で、無改変`scrartcl`最小文書はexit 0、logの`^!` 0件、DVI 1 page / 332 bytes。
DVIのSHA-256は`d1d2085d21aaf95eb135e3e17f7bda2177c2417efcd5dde19f9b57749622eed5`。
旧binaryは`\scantokens`未定義7件から77 errors・36 pagesへ崩れた。新実装を空の
`tests/fixtures/prjsarticle/hyphen.cfg`で試すと`\languagename`未定義由来の36 errorsだけが残るため、
このstubをKOMA gateへ使わない。TeX Live前提runnerは`tools/test-scrartcl.ps1`。

2026-08-23取得の追加CTAN資材は次である。`language.dat`だけは`english`と
`usenglish` / `USenglish` / `american` aliasの4行を試験側で生成し、公式assetとは数えない。

| asset | bytes | SHA-256 |
|---|---:|---|
| [KOMA-Script 3.49.2](https://mirrors.ctan.org/install/macros/latex/contrib/koma-script.tds.zip) | 9,470,014 | `a9d25d9dbdf7b43842bcb94b6fcef18762d4d7730583c019494b4f5e50995993` |
| [latex-graphics 2026-06-01](https://mirrors.ctan.org/install/macros/latex/required/latex-graphics.tds.zip) | 3,088,829 | `285842279287adea831ec9019f3b766d91a89d4ee742bcb436ebe7982ad2e684` |
| [Babel/base 26.9](https://mirrors.ctan.org/install/macros/latex/required/babel-base.tds.zip) | 4,071,520 | `4ad3c8e93a20b9dc3ee1437f3063098bab5168abc3e06350900229bd1fefed8b` |
| [hyph-utf8 2026-02-21](https://mirrors.ctan.org/install/language/hyph-utf8.tds.zip) | 4,737,241 | `d4768692494d8e9b8585cdd8a64edec43c9b9310af55c7414a7004c56ef855fc` |

未完了なのはraw byte 10/13、二段error context、実fileを挟む入れ子、資源超過、`\pausing`の
追加black-boxと、固定上限を共通run-local `InputLimits`へ移す構成面である。詳細な20項目は
設計文書を読むこと。

## 完了済み実験: `ls-R`のsafe Rust表現

状態: **2026-08-22に読取専用実験を完了、repoへの実装・統合なし**。

WSL `/tmp/pratex-lsr-safe-probe-*`だけで、現行意味の再現(A)、`RandomState`のまま正確な
件数を予約(B)、deterministic FNV-1a(C)、byte arena＋offset/hash bucket(D)を比較した。
TeX Live 2026の最大`ls-R`は5,674,350 byte、254,397 entry、231,561 unique basename。
全方式でbyte-sortした全name→candidate directory列、固定hit/miss corpus、合成した非UTF-8・
重複・hidden entry fixtureが完全一致した。

24回を交互に測った最大fixtureの結果:

| 方式 | build中央値 | hit ns/query | miss ns/query | VmHWM |
|---|---:|---:|---:|---:|
| A 現行意味の再現 | 49.562 ms | 60.287 | 21.101 | 56,832 KiB |
| B `RandomState`＋正確な予約 | 27.112 ms | 61.090 | 22.026 | 44,464 KiB |
| C FNV `HashMap` | 25.830 ms | 53.898 | 31.733 | 44,464 KiB |
| D FNV byte arena | 24.164 ms | 56.285 | 42.479 | 32,168 KiB |

追試の第一候補はBだが、今回の予約数は計測区間外で得たoracleであり、そのまま統合しない。
現実の一回読取りで安価に得られる容量hintを使って再測定する。C/Dはmissが50.4%/101.3%悪化し、
unkeyed FNVによるhash-flooding DoSもあるため非推奨。Windowsのstrict UTF-8拒否方針もarena化を
理由に変えない。詳細なfixture hash、toolchain、p10/p90は[性能測定](performance.md)に固定した。
prototype sourceは一時WSL `/tmp`の消去で残っておらず、hashだけからbit-for-bit再現はできない。
従ってこの値を性能gateにせず、採否時は永続的なprobeとrunnerを先にcommitして測り直す。

この「完成後まで保留」は当時の判断で、現在は主要機能sliceごとに性能gateを再測定する。
unsafe Rustは試さない。

Claude `82fa3a2`のLinux perf分解:

- uplatex DVI一頁: 229 ms
- PraTeX通常探索: 524 ms
- PraTeXで資材が全て手元: 140 ms
- 外部`kpsewhich`: 約291 ms
- 自前`ls-R`索引等: 約93 ms

これは探索を消せば届くという構造的根拠だが、資材を手元へ写した140 msをそのまま互換gateにしない。
将来は同じTeX tree、同じresolver結果、同じ出力で独立枝ごとに再測定する。ClaudeはLinux perfの
再測定を引き受けると連絡している。

## Vaakの現在地

- Vaakの`codex2/pratex-embedding-api`（`4e40e4b`）で、top-level用の`HostLayout`、`PreparedProgram`、
  `EmbeddingRunner`を追加し、PraTeX bridgeを公開prepared APIへ移した。parse/check/type-check/compileと
  layout/schema照合はcache miss時だけで、runnerとhost値bufferを再利用する。
- S-22のhost read/write/touched解析を`Ref`、`Freeze`、`MutMethod`まで直し、
  `HostBinding::supports_partial_writeback`でreadだけのbindingへ部分writeしない契約を固定した。
- host値は配列、map/hash、struct/wrapの中まで検証し、host関数返値の型/schema違反は
  呼出し直後に停止する。それ以前のhost register変更はC-2/S-22どおりTeX save stackへ書き戻す。
- host slot/index、VM operand、型descriptorの上限はcompile/prepare前に検査し、`u16` wrapや
  深すぎる公開型によるstack overflowを拒否する。
- named entry＋引数、typed HostFn完了値の公開enum、Leaf allocation 0、opaque token、
  suspend/resumeはまだない。これらが固まる前にPraTeX phase hookを先走らせない。
- 標準日本語経路のcallback数は0のままにする。
- PraTeX側の外向きWASM provider ABI 0.0は、version・feature・capability・operationを
  instantiate前に照合する層と、固定envelope/section/mailbox/transportを検査するbyte codecを
  runtime非依存で実装した。Vaakのprepared APIやportable ABIとは別version domainであり、
  module parser/runtime/affine lease/provider接続はまだない。

## NFSS/relation font checkpoint

`968d1e7`で`pratex-japanese`を和文encoding/family/series/shape、exact JFM shape、
NFSS size cache、従属欧文relationへ一般化し、`00dc469`でrelation保留の漏れとJFM同期時期を
修正した。relation fontは標準NFSS本体でなくpLaTeXがNFSS上へ加えた拡張の意味を、互換名を
生やさずPraTeX固有名で実装する。Declareはglobal、Setはgroup-local、Useはdocument bodyの
次の`\selectfont`一回だけで、preambleからpre-document選択へ保留する用法は未対応。
`prjsarticle`のbody/title/headingはその場でrelationを消費してから選択する。JFM宣言は横組exact
だけで、size function、shape substitution、縦組directionとpLaTeX互換名はまだない。

## 完了済み性能checkpoint: fmt collectionのbounded予約

`22a8bdd` / `3a4aaaf`は、untrustedなfmt宣言長をそのままcapacityにせず、一般の`Vec`と
`HashMap`へ4,096要素またはpayload換算64 KiBまでの初期予約を行う。Windows x86_64、release
LTO、17,446,628 byteの同一`latex.fmt`を使ったwarm交互A/B各8回では、formatだけ、空`article`、
300段落のwall中央値が7.31--15.26%、Eqtb復元が14.28--18.95%短縮し、300段落のDVI/log hashは
一致した。48標本のraw値は
[`benchmarks/fmt-bounded-reservation-20260824.csv`](benchmarks/fmt-bounded-reservation-20260824.csv)
に固定した。

これはWindowsの平坦化cacheを使ったwarm内部A/Bであり、利用者のLinux TeX Live tree、
`mainpra.tex`三回、`dvipdfmx`一回を含む9.14 sの再測定ではない。従ってLinuxの190%差を
解消したとは扱わず、次は同一corpusでengineとdriver、外部processを分けて測る。

## 完了済み性能checkpoint: global制御綴索引の一段復元

`codex2/fmt-control-sequence-hash`では、fmtのglobal byte/wide制御綴名を中間hashへ入れてから
最終hashへ移す二重処理をなくし、最初から最終`ControlSequenceHash`へ重複検査つきで入れる。
namespace付き名だけはnamespace数の検証まで疎な中間表へ残す。byte/wide二blockの宣言数は
合計65,536件を本文読込み前に拒否し、壊れたfmtから不要なhash成長や空namespace表を作らない。

Windows warm A/B 12組では、control-sequence復元中央値15.79%、fmt全体10.73%、wall 5.36%を
短縮し、DVI/log hashは一致した。raw 24標本は
[`benchmarks/control-sequence-global-hash-20260824.csv`](benchmarks/control-sequence-global-hash-20260824.csv)
に固定した。共有sourceのrelease focusedは22 passed、全releaseは849 passed、0 failed、
10 ignoredである。A/B基点は`681e065`であり、利用者のLinux 9.14 sやupLaTeX比1.2未満を
再測定した値ではない。

## 完了済み性能checkpoint: WSL backend発見失敗のrun-local化

`4acf8a8`は、Windowsでnative `kpsewhich`が起動不能かつWSL backendの発見にも失敗した時、
同じresolver instanceのoptional lookupごとに`wsl.exe`を再起動せず、同じtyped errorを返す。
明示的な`clear_external_cache`後だけ再発見し、個別fileのnegative cache、Linux既定、native成功、
WSL成功の意味は変更しない。

現行sourceから失敗状態の保存だけを外した厳密AとBを同じ失敗probeで三組交互に測り、中央値は
外部WSL process 13回から1回、wall 5,553.884 msから1,919.721 msへ短縮した。双方とも同じ
`graphics.cfg`不足位置でexit 1し、5,917 byteのlog hashが一致した。これはWindowsの異常系だけの
上限効果で、Linuxの9.14 s benchmarkへ外挿しない。詳細なraw三組とbinary hashは
[`performance.md`](performance.md)にある。

## LaTeXと日本語組版の次順

1. 利用者Linux profileで9回・1.372秒を占めた`kpsewhich`を最優先にする。resolver専用枝では
   Scanner/PDFをrun-local共有し、無関係aliasによるqueryごとのone-shotを解消した。続くLinux-first
   checkpointで、監査済みRust Kpathsea forkのsubprocess禁止constructorを一run一instanceで接続した。
   Unixでsystem libraryへlinkできた時だけ明示`pratex`、非UTF-8 `PathBuf`、C返値解放、typed formatの
   fast pathを使い、library不在とencoding非対応だけ既存safe resolverへ戻す。外部fmtは`--engine=rtex`
   意味を保つためsafe経路のままである。Windowsはallocator/CRT境界未実測なのでtyped fallback、WASMは
   dependencyなしである。公式TeX Live 2026 sourceからbuildしたKpathsea 6.4.2へのLinux実linkでは、
   `TEXINPUTS.pratex`、欧文TFM、JFM、VFのhit/miss、distinct PID 1・子process生成0を確認した。
   JFM no-copy runとlocal referenceの260-byte DVIもSHA-256
   `49bd1e1cd78832c970e7d6283cee99213cb6e21e8a628fe299484e11d1eb81f9`で一致する。再現runnerは
   `tools/test-kpathsea-linux.sh`、source/asset hashとbuild手順は`docs/kpathsea-port-notes.md`。
   次は利用者と同一のLaTeX corpusでupLaTeXとの交互end-to-end測定を行う。
2. 接続済みmain-loop JFM/禁則をbox/disc・残るcommand境界へ広げ、discの枝別意味を完成する。
3. `\tfont`と縦組metric/node/outputを追加し、spacingと禁則を横組から縦組へ広げる。
4. discard、`\showgroups` / `\showifs`、tracing等のe-TeX残件とTeX--XeTのLR組版を進める。

日本語の最低線は横組smokeではなくpTeX相当とJLReq native対応であり、縦組を含む。縦中横と
割注は2026-08-23に案Bへ決定した。coreでは縦中横を固定`InlineObject`、割注を分割可能な
`InlineSubflow`として扱い、用途名primitiveを直足ししない。

## 検証

`4acf8a8`のcheckpoint後にfocused testと全release gateを通した。再開時は
変更対象のfocused testを先に走らせ、全release gateへ戻る。

```powershell
cargo test --release --locked --test kanjiskip
cargo test --release --locked --no-fail-fast
```

その後、公式CTAN資材を新しい隔離rootへ取得してTRIPを走らせる。

```powershell
pwsh -NoProfile -File tools/run-trip.ps1
```

2026-08-24の既知正常値（TRIPは統合したspacing元枝で実測）:

- `4acf8a8`の統合枝release: 836 passed、0 failed、9 ignored
- TRIP Stage1/Stage2 exit 0
- `tripos.tex`正規化後一致
- DVI SHA-256: `b20af20a1463c6846f0c4c1ce687cd6354ce1a5f65ee401507627570787ae9fe`
- 上のDVI hashは、独立decoderで999 records・意味差0を確認した直前checkpointと一致
- このmachineにはPLtoTF/TFtoPL/DVItypeが無いため、hash検証済み公式`trip.tfm`を使い、
  今回のDVItype比較だけは未実行

Unicode table追加後の最小fmt増分は上記のとおり実測済み。full LaTeX fmtは本物のpTeX検出面を
入れた後に測る。

## その他の利用者指定

- binary/banner名はPraTeX。quiet modeとREADME更新は既に別commitで入っている。
- LaTeX名はLaPraTeX。
- 十分に固まった機能checkpointは`codex2/*`で検証後、`full`へ順次merge・pushしてよい。
  `main`へは入れず、設計だけ・production未接続・既知退行ありのsliceを完成機能扱いしない。
- 生文字列registerは`\rawstring` / `\rawstringdef` / `\therawstring`、raw専用`\showthe`、
  `Rc<Vec<u8>>` storage、群・`globaldefs`、fmt、production testまで実装済みである。
  1 slot 16 MiB、active/future slot全体64 MiBの上限を持つ。literal/file producerと
  `\the\rawstring`のLF/CRLF契約は未実装であり、`\therawstring`へ黙って統合しない。
- TCXはWeb2C input translation profileとして未実装。xord/xchrや`^^`記法と混同しない。
- `^^^^hhhh` / `^^^^^^hhhhhh`は未実装。
- PDF直接出力をOTF対応より先に進め、JFM/TFMだけのDVI/PDF基線を完成する。
  後続でRustyBuzzを接続する場合はdefault-offとする。
- 現在のcatcodeは`repr(u8)`。入力分類はcatcode側をカノンとし、`\catcode`と
  `\kcatcode`の公開番号は別codecで同じ意味へ写す。layout/JFM/provider IDは別domainに保つ。
- `for_CLAUDE.md`へClaude/Vaak向けの変更とAPI要求を追記し、commit時に
  `origin/claude/for-codex`をfetchする。

## 引継ぎ直後の安全な確認

```powershell
git status --short --branch
git log --oneline --decorate -8
git diff --stat
```

`codex2/jlreq-script-spacing`ではK/X、汎用spacing、性能条件、各国単位の文書を同時に更新している。
まず`AGENTS.md`、本書、`docs/kanjiskip-core-design.md`、`docs/etex-texxet-status.md`を読み、
誰の変更かを確認してから触る。

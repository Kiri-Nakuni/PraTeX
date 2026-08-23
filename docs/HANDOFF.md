# PraTeX 作業引継ぎ

更新: 2026-08-23（`codex2/jlreq-script-spacing`、checkpoint `4f832b1`）

この文書は、現在の Codex セッションから別のエージェントへ作業が移っても、
検証済みの境界と未commitの作業を失わないための生きた引継ぎである。
各作業単位が完了するたびに、commit、試験結果、残件を更新する。

## 最初に守ること

1. リポジトリ直下の `AGENTS.md` を最初から最後まで読む。
2. 現在の統合枝は `codex2/jlreq-script-spacing`。`main`と歴史的`full`を汚さない。
3. push済み基点は `4f832b1`。作業中のK/X、spacing、文書更新を無関係な差分として
   差し戻さない。横組JFM focused枝は統合済みである。
4. 通常実装と性能調整はsafe Rustだけで行う。
5. pTeX/upTeX/e-TeX/pdfTeXの実装sourceや上流testを移植しない。公開manual、公開file format、
   自作最小入力による公式binaryの黒箱観測から独立実装する。
6. 標準日本語組版はengine coreの一級機能であり、通常経路をVaak/WASM callbackへ逃がさない。
7. commitは日本語で「なぜ分けたか」を書き、試験名も日本語にする。

## 枝と共有状態

- 枝: `codex2/jlreq-script-spacing`
- push済み統合checkpoint: `4f832b1`（歴史的基点は`6ce8315`）
- 基点のrelease全suite: **564 passed、0 failed、6 ignored**（Vaak `89804b4`）
- 現在の統合枝release全suite: **618 passed、0 failed、7 ignored**
- 直近の共有commit:
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
- 性能枝 `codex/perf-wsl-euptex-safe` は `9bb6023`までpush済みで、現在は停止中
- Claudeの連絡枝 `origin/claude/for-codex` は `82fa3a2`まで確認済み
- Vaak `origin/codex/main` は `64ccf4e`まで確認済み
- Vaakの壊れていた`full`は`codex2/full`の`7c5ccd7`で修復済み。release全suiteは
  **727 passed、0 failed、1 ignored**で、GPL側のPraTeX sourceは一行も移していない。

WASM module importとnamespaceの基本方針は
[`wasm-module-import-v0.1.md`](wasm-module-import-v0.1.md)を一次資料とする。ABI 0.0は
spacing/unit providerだけを扱い、任意control sequenceの実行ABIは未決定の別仕様である。

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

## 完了済み設計: `\kanjiskip` / `\xkanjiskip`

状態: **2026-08-23に通常glue parameter面を実装、汎用script class対tableを作業中**。

- 文書: `docs/kanjiskip-core-design.md`
- INITEX既定0pt、通常glue parameterとしての代入・group、`\globaldefs`、算術、fmt、表示を
  既存のglue経路へ接続した。release focused testは3 passed、0 failed。
- K/Xとxsp/inhibitはlist終端値で全境界を再評価する。暗黙Kはshow/lastskipへ見せず、
  Xは幅0でも実glue nodeとして残る。
- JFMは途中で観測・除去できるためmain-loopで早期挿入し、K/Xだけclose-timeで再評価する
  hybridにする。
- 最終形のKはwide glyphのbit＋hlist単位specをline breaker/packer/outputが仮想glueとして扱い、
  純和文のnode数を倍増させない。
- standard Japaneseは`BuiltInPtex`のmonomorphic core経路で、Vaak/WASM callは0。
  Hangul–Latin等は同じscript-pair機構へ後から載せる。

この段階はLaTeXのpTeX検出を変えるが、「日本語組版完成」や検出stubとは呼ばない。実挿入、
JFM、禁則、line breaking、DVI/PDFまで連続して進める。内部tableはprovider-local IDと
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
- 範囲は横組DVIだけ。`\tfont`、縦組、JFM pair adjustment、K/X自動空白、禁則、PDF和文glyph、
  OTF shapingは未実装で、横組smokeを日本語組版完成とは呼ばない。
- 同じ欧文plain fixtureを`origin/main`と比較し、BOP--EOPの183 bytesは差分0だった。

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

## 完了済み設計: `\scantokens`

状態: **設計commit・push済み、code未着手**。

- 文書: `docs/scantokens-design.md`
- commit: `eda7dd1`

次のcodeは三段に分ける。

1. 実fileの自然EOFと`\endinput`を分離し、自然EOF行番号と外側context行番号を直す。
2. raw byte 10/13と論理改行を分けたtyped疑似inputを既存input stackへ統合する。
3. clean-room観測、資源上限、機能一覧を同期する。

疑似入力を一時fileへ書かず、単一byte buffer＋行末offsetで所有する。`\everyeof`は自然EOFだけ、
`\endinput`では発火しない。`newlinechar`は生成時、`endlinechar`は各行読取時である。
詳細と20個の必須回帰は設計文書を読むこと。

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

## LaTeXと日本語組版の次順

1. K/Xの下のscript class対table、auto switch、xsp/inhibit stateを固定し、公式CTAN資材で
   LaTeXのpTeX分岐を再測定する。
2. 横組wide nodeのJFM classをcompile済みpair adjustmentと中央spacing finalizerへ接続する。
3. `\tfont`と縦組metric/node/outputを追加し、K/X spacingと禁則を横組から縦組へ広げる。
4. LaTeXが次に要求した時点で`\scantokens`を設計どおり実装する。

日本語の最低線は横組smokeではなくpTeX相当とJLReq native対応であり、縦組を含む。縦中横と
割注は2026-08-23に案Bへ決定した。coreでは縦中横を固定`InlineObject`、割注を分割可能な
`InlineSubflow`として扱い、用途名primitiveを直足ししない。

## 検証

Unicode/lastnodetypeのcheckpoint後、次はまず現状確認から始める。

```powershell
cargo test --release --locked --test kanjiskip
cargo test --release --locked --no-fail-fast
```

その後、公式CTAN資材を新しい隔離rootへ取得してTRIPを走らせる。

```powershell
pwsh -NoProfile -File tools/run-trip.ps1
```

2026-08-23の現作業枝での既知正常値:

- 現在release: 594 passed、0 failed、6 ignored
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
- 生文字列registerは[設計](raw-string-registers.md)のみ。`\rawstring` / `\rawstringdef` /
  `\therawstring`、raw専用`\showthe`、任意byte storage、fmt、production testは未実装である。
  `InternalValue::RawString`まで取得を共通化し、再字句化する`\the`、全Other化、非実行表示の
  三consumerを分ける。font map内の生byte保持とは別機能である。
- TCXはWeb2C input translation profileとして未実装。xord/xchrや`^^`記法と混同しない。
- `^^^^hhhh` / `^^^^^^hhhhhh`は未実装。
- OTF対応はPDF直接出力と同順位だが、先にJFM/TFMだけのDVI/PDF基線を完成する。
  RustyBuzzを接続する場合はdefault-offとする。
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

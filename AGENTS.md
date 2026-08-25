# PraTeX 作業規約

## 作業の分担

| | 担当 | 触る場所 |
|---|---|---|
| **Codex** | **PraTeX（この版方、package名は`rtex`）** | `src/` `docs/` `tests/` `tools/` |
| **Codex** | PraTeX埋込みに必要なVaak公開境界 | Vaakの`codex2/pratex-embedding-api`だけ |
| Claude | Vaakの意味論判断（利用可能な時） | Vaak の中だけ |

PraTeX は `Cargo.toml` から **`../vaak`** をpath dependencyとして使う。
PraTeX側が必要とするAPIは `src/vaak.rs`、`docs/vaak-embedding-api-design.md`、
`for_CLAUDE.md` に契約として残し、Vaak側の変更はClaudeに伝える。

現在の通常作業枝は **`codex3/<目的>`** とする。統合・性能作業枝は
`codex3/perf-integration`である。`codex2/perf-resolver-index`の`f414757`を基点に、
vertical discard、run-local script spacing dispatcher、最小横組`prjlreq`を取り込んだ。
各focused testに加え、統合checkpoint `a2765c7`で
`cargo test --release --locked --no-fail-fast`は**915 passed、0 failed、11 ignored**だった。
macro引数arena checkpoint `13d1ab1`では**922 passed、0 failed、11 ignored**、plain欧文DVI
byte回帰も成功し、`origin`と`github`へpush済みである。公式CTAN TRIPは両段exit 0、
`tripos.tex` byte一致、DVIは999 records・16 pages・最大stack 17・意味差0、固定commentでは
公式2920-byte DVIとbyte一致した。巨大文書測定checkpoint `efa9ddd`では299頁の`lipsum`を
三engine、CPU 0、三warm-up＋15標本、六順序回転で測り、PraTeX/upLaTeXの全DVIをbyte一致させた。
paired wall比は幾何平均2.0075、中央値1.9763で、1.3未満には未達である。LuaLaTeX DVIの
絶対中央値に対するPraTeX比は0.3644だが、format/backendが異なる別workload列として扱う。
最新code checkpoint `8e0e543`は未展開token取得の不要な`Command`所有を除き、通常取得と同じ
不正token・alignment・outer回復境界を試験で固定した。299頁の変更前後A/B 20組はwall比
0.963979、instructions比0.968622、DVI/aux byte一致だった。全releaseは
**941 passed、0 failed、11 ignored**、公式TRIP固定comment DVIは公式2,920 byteへ完全一致した。
同じ最終binaryの三engine再測定はPraTeX/upLaTeX paired wall比が中央値1.615624、幾何平均
1.590993で、1.3未満には未達である。LuaLaTeXとの中央値比は0.280760だが、format/backendが
異なる別workload列として扱う。
旧統合checkpoint `6bc9ba4`は
`cargo test --release --locked --no-fail-fast` exit 0だが、aggregate件数を記録していないため、
過去checkpointの件数を流用しない。
`\detokenize`から`\scantokens`、横組JFM glyph、K/X/finalizer、JLReqの最小禁則、
直接PDFのnamed CIDと限定`/ToUnicode`、WASM ABI 0.0のwire/domain境界、
e-TeX `\middle`まで取り込み済みである。
`fe9fa5c`ではresolverの公開program名を`pratex`へ正し、TeX Live 2026の`upjisr-h` / `upjisg-h`
TFM・VFを作業directoryへcopyせず、PraTeX DVIから同じtreeの`dvipdfmx`による明朝・ゴシック
Type 0/CID PDFまで実測した。Linux no-copy gate 1件と公開CLI argvのfocused test 1件は成功済み。
`main`は歴史的基点として触らない。`full`へ直接実装はせず、focused test、全release、必要な
TRIP/DVI・PDF意味比較を通して十分に固まった機能checkpointを`codex3/*`から順次mergeする。
設計文書だけ、production未接続、既知の意味退行があるsliceは`full`へ送らない。

---

## 目的と優先順位

PraTeXは、日本語組版、欧文組版、和欧混植をengine coreの一級機能として扱う。
日本語についてはupTeXの再現だけを終点にせず、W3C JLReqをnativeに実装する。
学術論文だけでなく、日本語の小説・エッセイ・文芸書を第一級用途とする。DTPやword processorで
実用化されている縦組、ルビ、圏点、縦中横、割注、版面、柱・ノンブル、校正、入稿PDF等を
「TeXの用途外」として落とさず、engine core、package、toolingの適切な層へ置く。
和中混植も主要な完成目標とする。同じHan Unicode scalarを日本語、簡体字中国語、繁体字中国語の
言語区間ごとに自然な地域字形で出し分け、Han unificationを利用者へ露出させない。scalarや
ToUnicodeを別文字へ書き換えて見た目だけを合わせず、`LanguageRegion`、font routing、
OpenType language system / `locl`、地域別fallbackと約物・禁則を一つの組版contextとして扱う。

優先順位は次のとおり。

1. **upTeX以上のW3C JLReq native対応**
   - 横組・縦組、JFM、和文font/node、和欧文間隔、禁則、行分割、箱、DVI/PDFを連続して扱う。
   - 縦中横と割注もengine-levelで支える。2026-08-23に案Bを採択し、縦中横は固定`InlineObject`、割注は分割可能な`InlineSubflow`とする。用途名primitiveは直足ししない。
   - 標準の日本語組版をVaak/WASM callbackへ逃がさない。
2. **e-TeX拡張の完成**
   - 字句、内省、配列parameter、discard、表示、TeX--XeTのLR node・反転・出力まで含む。
   - TeX--XeTは既定で無効だが、`\TeXXeTstate`が有効な時は意味を持たせる。
3. **互換primitive**
   - pdfTeX相当をclean-roomで実装する。PraTeX固有の正規名は `\pratex...` とし、
     必要な `\pdf...` 名は同じ決定箇所へ結ぶ互換aliasとしてだけ残す。
   - XeTeX/LuaTeXの有用な拡張primitiveも、engine全体の互換を偽らず個別契約として追加する。
4. **Vaakとの統合とversion付きWASM ABI**
   - 高頻度の単純規則は登録時に検証し、host-owned tableへcompileする。
   - 複雑で低頻度の規則だけをbounded batchとしてWASMへ渡す。
5. **主要LaTeX packageのPraTeX-native対応**
   - `graphicx` / `xcolor`、`hyperref`、`siunitx`、`tikz`、PraTeX用にpatchした`pxrubrica`の順を基準にする。
   - 小説・エッセイ用途では`jlreq`、`plext`相当、`pxrubrica`、`jlreq-trimmarks`、日本語bookmark、
     右綴じ・入稿PDFを独立した互換profileにする。engine未完成とpackage非互換を混同しないため、
     e-TeX/TeX--XeT完全、upTeX上位互換、LuaTeX級PDFのcore gate後に最終互換判定する。
   - 他engineのversion primitiveを生やして検査を通さない。PraTeX固有検出と個別feature契約を使う。
6. **PDF直接出力**
   - JFM/TFMだけで和文・欧文・混植を出せる基線の次に完成させる。OTF対応より先である。
7. **OTF対応**
   - OTF shapingはdefault-offのRustyBuzz接続を第一候補とする。
   - native loader・metric・shaping境界が固まった後、PraTeX固有feature queryを使う
     `fontspec`上位互換のOpenType選択packageを作る。和文NFSS/JFM、文字class、exact code point、
     Unicode範囲・面、`LanguageRegion`ごとのfont routingまで含める。同じHan scalarに対する
     日本語・簡体字・繁体字の地域字形、regionを跨がないfallback、Babel言語区間adapterを
     completion gateにする。一packageか二層かはAPI試作で決め、
     XeTeX/LuaTeXへの偽装で通さない。詳細は`docs/opentype-package-roadmap.md`。
8. **safe Rustの範囲での性能調整**
   - 機能完成を優先するが、利用者測定でhot pathが見えた箇所は意味回帰を固定して並行に調整する。
   - 局所A/Bの改善をend-to-end gate達成と読み替えない。
   - 最終gateは、同一入力・同一TeX tree・同等のDVI意味でupLaTeXの実行時間の**1.2倍未満**。
9. **native絵文字（OTF完成後）**
   - PDF直接出力と通常OTF loader・shaping・font fallbackを先に完成し、絵文字実装はその後に行う。
   - plain UTF-8のVS15/VS16、modifier、regional indicator、keycap、tag、ZWJ sequenceをmacroなしで
     一つの壊せないclusterとして扱う。color font、PDF描画、元scalar列のToUnicode/ActualTextまでを
     completion gateにし、単にUnicode入力を受理するだけを対応と呼ばない。
   - 詳細は`docs/emoji-native-roadmap.md`。現在は設計のみで、OTF作業へ割り込ませない。

TeX Live/kpathsea互換探索は全段に必要な横断基盤である。file lookupごとに外部`kpsewhich`を
起動する実装を最終形にしない。run全体で一つのTeX treeを固定し、`texmf.cnf`、path展開、
`ls-R`、alias等をnativeに索引化して、通常処理の子process呼出し0を目標にする。
外部`kpsewhich`は移行中の明示fallbackまたはblack-box oracleに限定し、hit/miss双方を測る。

アラビア語を含む汎用多言語組版を完成目標にはしない。ただしTeX--XeTを実装し、
RustyBuzzを **default-offのoptional機能** として接続できる境界を保ち、理論上の拡張余地は残す。
PraTeX自身の通常実装には`unsafe`を書かない。optional依存は採用前にlicense、unsafe利用、
再現性、binary size、既定featureへの混入を別途監査する。

---

## 文字分類と自動空白の境界

`\kanjiskip` / `\xkanjiskip`は互換primitiveの表面であり、内部を日本語二値に固定しない。
中央の仕組みは **文字・script class対から自動空白規則を選ぶ機構** とする。

- 入力字句分類はcatcode側の`InputCategory`をカノンとする。`\catcode`と`\kcatcode`は
  同じ意味へ入る別々の公開数値viewなので、生の整数をcastして比較・保存しない。
- layout用`ScriptClassId`、`LanguageRegion`、TeXの`language`、JFM metric class、
  provider-local ID、文字identityは入力字句分類と別domainに保つ。
- HanのscalarやUnicode Script propertyから日本語・簡体字・繁体字を推測しない。Babel等の
  package adapterは明示された言語区間をtyped `LanguageRegion`へ写し、catcodeやTeXの
  hyphenation `language`を上書きしない。
- 同じscalarでもregionによりfont face、OpenType language/`locl`、glyph ID、fallback、約物・
  禁則規則は変わり得る。一方、文字identityとPDF `ToUnicode`は元のscalar/IVSを保つ。
- pTeX/JLReq標準規則は`BuiltIn`のnative表で処理し、通常paragraphのVaak/WASM callは0にする。
- 高頻度の利用者規則は、明示capabilityで提出された全tableをPraTeXが検証し、
  `CompiledTable`へ原子的にcompileする。
- tableへ落とせない低頻度規則だけを`ExplicitWasm`のbounded batchへ渡す。
- provider handle、WASM instance、cache世代はrun-localであり、fmtへ保存しない。
- trap、fuel切れ、不正ID、不正actionでは部分適用せず、batch全体を破棄して定めたfallbackへ戻す。
- per-character/per-boundaryのtrait object callやABI往復をhot loopへ置かない。

詳細は `docs/kanjiskip-core-design.md` と `docs/extensible-layout-roadmap.md`。

---

## 寸法単位の境界

各国・各組版文化の文字サイズ単位を調査し、追加可能にする。ただしconsumerごとに単位判断を
複製せず、`scan_dimen` / `scan_units`の中央経路だけへ接続する。

- 組込み単位と互換単位はPraTeX nativeで処理する。
- 物理単位は浮動小数点でなく厳密な有理比`PhysicalRational`として持つ。
- font、JFM、組方向に依存する単位は`ContextMetric`として区別する。
- Vaakは単位出現ごとのcallbackではなく、明示capabilityによる検証済みtable登録を使う。
- WASMはtable化できない低頻度のcontext変換だけをboundedに処理する。
- `true`、`\mag`、丸め、overflow、`MAX_DIMEN`、未知単位の回復はhost側の一箇所で決める。

詳細は `docs/extensible-dimension-units-roadmap.md`。単位を追加する前に、標準・公的資料、
適用地域、歴史的定義と現行DTP定義の違い、厳密換算の可否を記録する。

---

## font・出力の順序

1. TFM/JFMのmetricをfont選択、glyph node、line breaking、packing、DVIへ接続する。
2. 同じhost-owned glyph/metric境界をPDF backendも使い、組版判断をbackendへ複製しない。
3. OTF/TrueType loaderとdefault-off RustyBuzzは同じ境界へ後付けする。
4. 同じnative font境界へ、`fontspec`上位互換と和文NFSS/JFM、文字class・code point・範囲・面別
   routingを持つPraTeX-native packageを接続する。routeとshape planは`LanguageRegion`もkeyにし、
   同じHan scalarのja / zh-Hans / zh-Hant字形をfont側の公開能力に従って選ぶ。
5. OTFの有無でJFM/TFM経路や標準JLReqの意味を変えない。
6. 通常OTFのloader、shaping、fallback、PDF subsetを完成した後、同じcluster/font境界へ
   native絵文字を接続する。color glyph paintingはshapingと別層にし、DVIで表現不能な経路を黙って落とさない。

PDF直接出力をOTF対応より先に行う。どちらもJFM/TFM基線を飛び越えない。
native絵文字はOTF対応のさらに後である。

---

## 権利——向きは一方通行

| | |
|---|---|
| **PraTeX/rtexはGPL-3.0** | 基礎実装の権利は **tyti氏に帰属**する |
| VaakはMIT（有村陽大） | **Vaak → PraTeXは可。** 組み込んだ全体はGPLv3として配る |
| **PraTeX → Vaakは不可** | GPL側を写すとVaak全体がGPLv3になる。**一行も写さない** |

依頼者（ハンドル Kiri Nakuni）の過去の寄与は名前空間の試みだけで、本人が
「無いものと認めた」と記録されている。

### e-upTeX・pTeX・他engineの規律

**「e-upTeXはBSDだから大丈夫」と判断してはいけない。** `uptexdir`はpTeX、upTeX、e-TeX
由来が混ざり、`ptexdir/COPYRIGHT`には独自条項がある。BSD-3-Clauseなのは
`uptex-base`のformatと文書であってengine全体ではない。

e-TeX、pTeX、upTeX、e-upTeX、pdfTeX、XeTeX、LuaTeXの実装sourceや上流testを移植・翻訳しない。
公開manual、公開file format、標準文書、自作最小入力による公式binaryのblack-box観測から
独立実装する。詳しくは `docs/euptex-port-notes.md` と各port noteを読む。

このマシンにはTeX Liveがない。black-boxや互換試験に必要な公式資材はCTAN等の公式配布元から
一時領域へ取得し、URL、版、取得日、SHA-256を記録する。repositoryへ無断でvendorしない。

PraTeXをpTeX、upTeX、pdfTeX、XeTeX、LuaTeXとして偽装しない。native検出は読み取り専用の
`\pratexversion`をカノンとし、必要ならPraTeX固有feature queryを追加する。互換primitiveを
個別に持つことと、engine全体を名乗ることを混同しない。

### PraTeX 1のrelease gate

`docs/versioning.md`を版番号の一次資料とする。JLReq一級、TeX--XeTを含むe-TeX完全対応、
実時刻、pdfTeX相当PDF、OTF、Vaak API、WASM ABI/module system、PraTeX自身のWASM targetが
すべて完成するまで`\pratexversion`は0であり、bannerも版1を名乗らない。完成後の版は
`1`, `1.1`, `1.11`, `1.110`, `1.1100`, … と末尾の零を保ち、
`1 +`リウヴィル定数へ収束させる。WASM module systemは
`docs/wasm-module-import-v0.1.md`を一次資料とする。control sequence実行ABIはなお別途策定であり、
import/namespace仕様から推測して補わない。

---

## 実装の作法

| | |
|---|---|
| **safe Rustだけ** | PraTeXの通常sourceへ`unsafe`を書かない |
| **`// See 372.`** | `TeX: The Program`の節番号。既存実装に倣う |
| **決定を二箇所で実装しない** | 走査、spacing、単位、表示、ABI検証をconsumer側へ複製しない |
| コミットは**日本語** | 変更列挙でなく「なぜその境界が必要か」を書く |
| 試験名も**日本語** | 例: `fn 引用符つきのファイル名を読む()` |

通常の検証:

```powershell
cargo test --release --locked --no-fail-fast
```

機能追加ではfocused testを先に通し、その後に全release、必要ならTRIPとDVI/PDF意味比較を行う。
未展開token取得checkpoint `8e0e543`で実行した`cargo test --release --locked --no-fail-fast`は
**941 passed、0 failed、11 ignored**（2026-08-25）。全integration suiteにplain DVI byte回帰、
e-TeX `\middle`・`\showtokens`・penalty配列・discard、日本語spacingと和文NFSS relation、PDF、
Vaak連携を含む。ignoredは実TeX Live、配布JFM、公式dvipdfmx、pinned CTAN、doctest等の明示手動gateである。

`8e0e543`では公式CTAN TRIPをcleanなVaak dependencyと隔離buildで再実行した。両段exit 0、
`tripos.tex` byte一致、`8terminal.tex` 0 byte、PLtoTF→TFtoPL往復byte一致である。TeX Live 2026
DVItypeは固定comment時に公式出力へ一致した。以前の独立decoderで999 records、16 pages、最大stack 17を
確認した公式DVIと固定comment出力がbyte一致するため、公開record列も同一である。既定DVIのraw hashは実時刻commentで変わるため
gateにしない。公式の27-byte commentを`-output-comment`で固定した対照runは公式DVIとbyte一致し、
SHA-256 `09802695e330d34acec9192c15debe2de65e34fcbd3f947db9c8924240b1fe0a`だった。
log/terminalの既知の拡張診断差はDVI一致へ混ぜず、別の未解消差として保持する。

plain formatの欧文DVIは`origin/main`のrTeXにopcode・座標を含めて完全回帰させる。
TRIP再現用の単精度`glue_set`境界は`trip` featureだけへ閉じ込める。通常版の基準fixtureでは
2026-08-23にpage body 183 bytesの差分0を確認した。LaTeX DVIの完全回帰は`latex.ltx`を
LaPraTeX用に適合させるまでは要求しない。ただし`efa9ddd`の教材型298頁fixtureでは、同じpage数、
aux、tocにもかかわらず最初の脚注内部でupLaTeXと1,161 spの縦位置差を検出した。通常版の倍精度
`glue_set`を全面的に単精度化するcandidateは対象差を変えず、目次へ新しい1 sp差を作ったため撤回済み。
この仮説を繰り返さず、font番号差として正規化したり性能標本へ採用したりせず、footnote insertionと
page builderが出力boxへ渡す寸法を調べる。

upLaTeX比1.2未満の最終gateは維持する。今回の性能専用作業では、同じ入力・同じTeX tree・
同等DVIのupTeX系比較でPraTeXを1.3倍未満へ置くことをroadmap再開の中間条件とする。
比較対象がplain/upTeXかLaTeX/upLaTeXかをcorpusごとに記録し、異なる仕事の値を混ぜない。
`22a8bdd` / `3a4aaaf`ではfmt collectionの初期予約をuntrustedな宣言長から切り離し、
Windowsのwarm A/Bでwallを7.31--15.26%短縮した。これは利用者のLinux end-to-end 9.14 sを
再測定した値ではなく、差の解消を意味しない。48標本のraw値は
`docs/benchmarks/fmt-bounded-reservation-20260824.csv`にある。探索、fmt読込み、字句化、組版、
DVI出力を分け、外部process時間を隠さず、cold/warmとhit/missを同じfixtureで測る。
`4acf8a8`はWindowsで発見不能なWSL backendをoptional lookupごとに再起動する異常系を
run-local状態へ固定した。これはLinuxの通常探索や利用者の9.14 sを短縮した値ではない。
`codex2/fmt-control-sequence-hash`ではglobal制御綴のfmt復元を一段化し、Windows warm A/Bで
control-sequence区間15.79%、fmt全体10.73%、wall 5.36%を短縮した。DVI/log hashは一致し、
全releaseは849 passed、0 failed、10 ignoredである。A/B基点は`681e065`なので、この局所値を
現行Linux end-to-endやupLaTeX比1.2未満の達成へ外挿しない。

### 既知の落とし穴

1. `scan_toks`は`def_ref`を作り直す。入れ子走査には`token_lists::nested_scan_toks`を使う。
2. `\detokenize`と`\unexpanded`は`\the`と同様、結果へ直に足す。差し戻して再展開しない。
3. 現在の条件は`cond_stack`でなく`scanner.cur_if` / `if_limit`にあり、stackは外側の控えである。
4. `GroupType`は並び順で写さない。PraTeXにはTeXにない`AlignEntry`がある。
5. logは79桁で折り返す。試験照合には`join_log`を使う。
6. fmtへrun-local provider ID、pointer、WASM handle、cache世代を保存しない。
7. `\scantokens`の疑似入力は暗黙groupを作らず、同じ行の途中でも現在のcatcodeを使って
   逐次再字句化する。引数を一括tokenizeしたり、一時fileへ逃がしたりしない。
8. `tests/fixtures/prjsarticle/hyphen.cfg`はlanguage patternを意図的に省いた試験stubである。
   KOMA-ScriptのgateにはTeX Liveの標準`hyphen.cfg`を使い、同じbinaryでfmtを再生成する。
9. DVIのraw hashが違う時はfont番号、opcode整数幅、pointer、paddingだけを正規化できる。
   glyph、rule、special、stack、移動量の差は小さくても意味差であり、許容幅を設けて性能gateへ入れない。
10. 教材脚注の1,161 sp差は全`make_glue_ratio`の単精度化では直らない。対象値は不変のまま、
    それ以前の目次へ新しい1 sp差を作ることを確認済みなので、このcandidateを再実装しない。

---

## 現在地

詳細な対応表は `docs/feature-inventory.md`、生きた引継ぎは `docs/HANDOFF.md` を優先する。

- e-TeX/pdfTeX primitiveを追加し、`expl3-code.tex`の未定義primitive段階は通過済み。
- 限定fixtureでは公式`latex.ltx`からfmtをdumpし、最小articleをDVI/PDFまで処理済み。
- 2026-08-23の追測では、通常の`hyphen.cfg`を含むCTAN fixtureで無改変の
  LaTeX2e 2026-06-01と`expl3-code.tex`を完走し、`latex.fmt`をerror 0でdumpした。
  これは一般のclass/package互換性やLaTeX DVIの完全回帰を保証しない。
- `\kanjiskip` / `\xkanjiskip`の通常glue parameter面と、検証済みscript class対tableは実装済み。
  横組BuiltIn最小hybridはJFM pairと`、。`・JLReq由来の横組括弧12対の禁則をmain loopで、
  K/Xをlist close時に由来付きnodeとしてhbox、段落、alignment、line break、DVIへ接続する。
  `{}`、`\relax`、`\unskip`、`\message`、semi-simple group、`\showthe`、整数register代入の
  node-less境界を公式e-upTeXで照合済みで、削除されたJFMをclose時に復活させない。auto switch、`xspcode`、
  `inhibitxspcode`はtyped eqtb、群・`globaldefs`、fmt、中央finalizerへ接続済み。直結和文glyph間の
  Kは公式e-upTeXのblack-box結果に合わせた仮想nodeとなり、寸法・改行・DVIには効く一方、
  `\showbox`、`\lastskip`、`\lastnodetype`、`\unskip`からは隠れる。確認済みunshifted hbox境界は
  material Kへ分けて接続済みである。discはpre/no-break先頭への左spacingを遮断し、no-breakと
  post-breakの末尾から右glyphへのK/Xを枝内の条件付きmaterial nodeとして個別に保持する。枝内直結Kだけは
  virtualのまま、packer・line breaker・DVIが選んだ枝へ追随する。shifted hbox/vboxはbarrierのままである。
  `codex2/jlreq-box-disc`の全releaseは876 passed、0 failed、10 ignoredで、plain欧文DVI byte回帰も維持した。
- `\readline`、`\interactionmode`、mark class、糊成分・型変換、
  `\currentiftype`の`\unless`符号、font照会、`\middle`は実装済み。`\middle`は
  segmentごとのsave group復元、元のmath styleでのnew list、全delimiter共通寸法、
  Close/Open spacing、fmt/error回復をprocess試験する。`\scantokens`はboundedな
  typed疑似fileとして接続済みで、実file・疑似fileとも`\everyeof`は自然EOFだけで
  一度発火し、`\endinput`では発火しない。fmtとKOMA-Scriptの動的catcode経路も試験済み。
- `\showtokens`はgeneral text入口だけを展開し、本文を未展開token列のまま既存表示へ渡す。
  外側brace、入れ子、空白、catcode 6/12、制御語・記号、和文token、全mode、101回、alias、fmt、
  JFMのnode-less境界を公式TeX Live 2026へ照合済み。公式Web2Cはshow診断後exit 1だが、PraTeXは
  全非fatal診断後の通常終了を0にする既存CLI差分があり、primitive意味と混ぜず別残件にする。
- `\showifs`は開いている条件を内側から外側へlevel、`\unless`、`\else`、開始行つきで表示する。
  空条件、`\ifcase`枝、状態不変、全mode、101回、alias、fmt、JFMのnode-less境界を公開manualと
  公式TeX Live 2026への自作black-boxで固定済み。`\showgroups`と残るtracingは未実装である。
- `\interlinepenalties`、`\clubpenalties`、`\widowpenalties`、
  `\displaywidowpenalties`は局所／大域代入、内部照会、fmt、通常段落とdisplay直前の
  post-line-break nodeまで接続済み。正の`\savingvdiscards`によるpage builder／公開`\vsplit`の
  discard保存と、所有権を一度だけ移す`\pagediscards`／`\splitdiscards`も接続済み。
  page listはoutput routine終端、split listは各`\vsplit`開始時に世代を切り、fmtには保存しない。
- `\savinghyphcodes`は正値の`\patterns`時にlanguage別の小文字写像を保存し、pattern圧縮後の
  通常hyphenationと例外登録へ接続済み。同一languageの再snapshot、0以下での保持、fmt、
  PraTeX Latin-UCS拡張を試験する。e-TeXの8-bit表とPraTeX拡張は別型に保つ。
- `\TeXXeTstate`はfmt読込時0へ戻る。正値の明示restricted hboxでは
  `\beginL` / `\endL` / `\beginR` / `\endR`をtyped方向nodeにし、入れ子LR stackを
  backend共通shipoutで明示反転して通常DVI/PDFへ書く最初sliceを接続した。
  inline mathはmath-surroundを含むatomic LTRとして外側RTLから守る。方向nodeのない純LTRは
  従来のnode走査中にmarkerを確認する単一passでallocationなし、LR frameはtreeのまま
  閉じて最後に一回だけiterativeに平坦化する。paragraph、display、math mode内方向primitive、
  改行をまたぐLR stackは未実装であり、TeX--XeT完了とは表示しない。
  disc枝・alignment spanの直接方向primitiveと方向node入りhboxのunboxは診断し、
  未対応listへ方向境界を漏らさない。
  RTL区間がdisc nodeを直接含む場合も部分反転をせず、hlist全体の方向変換を破棄して診断する。
- 外向きWASM provider ABI 0.0は、version range、required/optional feature、capability、
  operationのruntime非依存交渉を`src/wasm_provider_abi.rs`へ、固定envelope、section集合、
  mailbox範囲、transport返値、lease上限のbyte codecを`src/wasm_wire_v0.rs`へ実装した。
  `SpacingTableUpload`はscalar/mask/class/context重複の共通domain validator、有理数長さ、
  tier/break/edge/penalty/reason、atomic candidate交換に加え、canonical候補を共通native表へ
  compileするsealed境界を持つ。compile完了後だけrun-local dispatcherへ原子的にinstallする
  host入口と、list単位の世代・region一貫性を検査する中央finalizer接続も実装済みである。
  direct glyph境界ではclass pairのNoSpace/K/X/fixed glueと、表現可能なboundary glue/penaltyを
  実nodeへ変換する。RegionNode、indirect box/discのcustom規則、adjustment tier、line-edge discardは
  未接続なので、その意味を黙って落とさずBuiltInへ境界単位でfallbackする。
  module profile、export検査、affine lease、runtime、provider registrationは未実装であり、
  TeX sourceから自己承認する経路はない。標準日本語はこの境界を通らずcallback 0回を維持する。
- 生文字列registerは`\rawstring`、`\rawstringdef`、`\therawstring`、専用`\showthe`、
  `Rc<Vec<u8>>` storage、群・`globaldefs`、fmt、production testまで実装済み。1 slotは16 MiB、
  active/future slot全体は64 MiBに制限し、0--255をdense、256--32767をsparseに持つ。
  literal/file producerと`\the\rawstring`のLF/CRLF契約は未実装なので、`\therawstring`と混同しない。
- 横組JFMはbounded loader、TeX互換scale、current和文font、`\pratexjfont`と意味が一致する
  範囲の`\jfont` alias、`zw`/`zh`、wide node、class pair、DVI `set2`/`set3`まで接続済み。
  `pratex-japanese`は和文encoding/family/series/shapeとJFM shape宣言、NFSS sizeのexact-sp cache、
  和文tupleから欧文NFSS tupleを一回だけ選ぶrelation fontを持つ。relation fontは標準NFSS本体
  ではなくpLaTeXがNFSS上へ加えた拡張の意味を、PraTeX固有名で実装したもの。publicなUseは
  document body用で、preambleの次回`\selectfont`へ保留する契約はない。JFM宣言は横組exactだけ。
  `prjsarticle`の通常font roleはこの宣言面を使い、手続き的な和欧hook列を使わない。
  `\tfont`、縦組、shifted/vboxとnode-less commandを含むmain-loop境界の網羅、完全禁則は未接続。PDF和文glyphは明示named CID profileを
  使う非埋込みBMP最小経路だけ接続済みで、portableな字形表示ではない。
- TeX Live探索の既定CLI名は`--progname=pratex`であり、別engine名へ偽装しない。通常DVIでは
  PraTeXがTFM/JFMを解決し、VFはDVI driver自身が解決する。本文用`upjisr-h`だけでなく
  `prjsarticle`見出し用`upjisg-h`もno-copy実機gateでDVI→Type 0/CID PDFまで確認済み。
- plain formatで`\directvaak`、`\vaakdef`、`let` / `var`、host aliasを使う実行例は
  `examples/plain-vaak.tex`。静的失敗はprepare段階・行・桁・診断本文を表示して0へ展開する。
- `\pdfmdfivesum file{...}`はresolver経由の実file byte列をincremental MD5へ流し、最小
  `hyperref`文書はDVIまで到達済み。PDF直接出力、Type 1全埋込み、`ls-R`/`kpsewhich` resolverは
  なお部分実装である。named CIDのBMP content codeには限定`/ToUnicode`があり、
  copy/searchは改善したが、FontFileはなく字形表示はviewer側fontに依存する。
- 利用者のLinux profileでは9回の`kpsewhich`子processが1.372秒、wallの45.95%を占めた。
  `codex2/resolver-kpse-bootstrap`ではScanner、Output、直接PDF loaderを一つのrun-local resolverへまとめ、
  positive/negative cache、`ls-R` catalog、用途path、backendを共有した。`aliases`はboundedに読み、
  一致・壊れた場合だけone-shotへ戻し、無関係aliasで後続の一意hitを捨てない。
  Rust `kpathsea` 0.3.4 / `kpathsea_sys` 0.2.3を基点に、明示`pratex` program名、非UTF-8 `PathBuf`、
  LinuxのC返値解放、typed format、subprocess禁止constructorを持つ監査済みforkを接続した。
  Linux既定`bundled-kpathsea`は公式TeX Live 2026 revision
  `fb6158926661cb7a7246b3a94a0cb170a9624d5a`のKpathsea 6.4.2を静的buildし、
  `in-process-only-caller`だけを使う。source取得/build失敗時にCLIへ黙って退行しない。
  TeX Liveもsystem KpathseaもないWSLで55 C sourceのrelease link、子process 0、TEX/TFM/JFM/VF
  hit/miss、local/tree DVI byte一致を実行時に固定した。固定CTAN LaTeX runtimeをext4上の`ls-R`
  treeへ組み直した15組では、local/tree wall中央値0.49/0.45 s、paired差中央値0.00 s、全DVIが
  `3ae145d...`へ一致し、tree runのprocess生成も0だった。これは利用者の30×`lipsum`、追加package、
  `dvipdfmx`を含まないので9.14 sや1.2倍gateの再測定ではない。
  配布側libraryを使う明示`system-kpathsea` buildは残す。外部fmtは`--engine=rtex`を保つsafe経路である。
  Windowsはallocator/CRT境界未実測のためtyped fallback、WASMとその他Unixはdependencyをcompileしない。
- 名前空間はPhase 0--7済み。Phase 8のTRIPとalignment再利用検証が残る。

## 直近の実装順

1. `8e0e543`の未展開token取得はfocused test、全release、plain DVI、公式TRIPを通した。
   299頁の変更前後A/Bはwall比0.963979、instructions比0.968622、全DVI/aux byte一致だった。
2. 同じ最終binaryの299頁三engine再測定はPraTeX 1.607243秒、upLaTeX 0.998914秒、
   LuaLaTeX 5.724616秒（各中央値）だった。PraTeX/upLaTeXのpaired比は中央値1.615624、
   幾何平均1.590993で1.3未満には未達だが、利用者判断により性能調整をいったん停止し、
   機能roadmapを再開する。短文・40頁・100頁は再開時に起動固定費と傾きを分ける診断列とし、
   300頁級をprimaryに保つ。
   教材型298頁はTikZ/数式/表/参照を三engineで完走しaux/tocも一致したが、脚注内1,161 sp差があるため
   性能gateから除外し、`glue_set`境界を先に直す。LuaTeX/LuaLaTeXはformat・font・backend差を
   明記した別列で測る。corpus設計は`docs/research/japanese-publishing/`を使う。
3. Vaak担当枝のcleanなpush済みcheckpointを固定し、`directvaak`と`directlua`を既定cache、
   毎回prepare、named reuseの対称な三面で測る。Vaak担当者の性能変更をPraTeX側で重複実装しない。
4. 性能調整は利用者判断でいったん停止する。1.3未満、1.1未満、0.98未満は未達の再開基線として
   区別して残す。性能条件を待たず、接続済みmain-loop JFM/禁則をshifted/vbox・残るcommand
   境界へ広げ、
   discの枝内JFM class・禁則・unbox再評価matrixを完成する。
5. glyph時点のRegionNode/context伝播をbox/disc/unboxへ接続し、compiled tableのindirect edge、
   adjustment tier、line-edge discardをline breakerまで完成する。
6. `\tfont`と縦組metric/node/outputを追加し、JFM/K/X/禁則を横組から縦組へ広げる。
7. `\showgroups`と残るtracing、`\lastlinefit`を接続し、TeX--XeTの実装済み
   restricted hbox sliceをparagraph・display・mathへ広げるe-TeX残件を公開仕様どおり実装する。
8. PraTeX-native package adapterを順に通し、PDF直接出力をOTFより先に完成する。
9. Vaak table uploadとversion付きWASM ABIは、内部表現を固定した段階から並行して適合試験を作る。
10. WASM target自体のcompile実験は、横組みcheckpoint後に行う。性能gateはroadmap再開後も継続する。

`\kanjiskip` / `\xkanjiskip`をLaTeX検出だけ通すstubにしない。INITEX既定0、代入、群、
`\globaldefs`、算術、内部量、表示、fmtを既存glue経路へ通し、その後の実spacingへ連続して接続する。

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

通常作業の枝は **`codex2/<目的>`** とする。現在の統合枝は
`codex2/jlreq-script-spacing`であり、2026-08-23のpush済みroot checkpointは`c9bd240`。
横組JFM glyphのfocused枝は`codex2/japanese-glyph-dvi`で、そのcheckpointを取り込み済みである。
`main`は歴史的基点として触らない。`full`へ直接実装はせず、focused test、全release、必要な
TRIP/DVI・PDF意味比較を通して十分に固まった機能checkpointを`codex2/*`から順次mergeする。
設計文書だけ、production未接続、既知の意味退行があるsliceは`full`へ送らない。

---

## 目的と優先順位

PraTeXは、日本語組版、欧文組版、和欧混植をengine coreの一級機能として扱う。
日本語についてはupTeXの再現だけを終点にせず、W3C JLReqをnativeに実装する。

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
   - 他engineのversion primitiveを生やして検査を通さない。PraTeX固有検出と個別feature契約を使う。
6. **PDF直接出力**
   - JFM/TFMだけで和文・欧文・混植を出せる基線の次に完成させる。OTF対応より先である。
7. **OTF対応**
   - OTF shapingはdefault-offのRustyBuzz接続を第一候補とする。
8. **safe Rustの範囲での性能調整**
   - 当面は機能完成を優先して後回し。ただし退行を測れるfixtureは各sliceで残す。
   - 最終gateは、同一入力・同一TeX tree・同等のDVI意味でupLaTeXの実行時間の**1.2倍未満**。

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
4. OTFの有無でJFM/TFM経路や標準JLReqの意味を変えない。

PDF直接出力をOTF対応より先に行う。どちらもJFM/TFM基線を飛び越えない。

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

`6ce8315`を手元のVaak `89804b4`と組み合わせた基準は
**564 passed、0 failed、6 ignored**。ignoredは実TeX Live、配布JFM、doctestの手動照合である。
機能追加ではfocused testを先に通し、その後に全release、必要ならTRIPとDVI/PDF意味比較を行う。
現在のK/X、script spacing、TeXXeT fmt、横組JFM glyph sliceを含む作業枝は
**594 passed、0 failed、6 ignored**（2026-08-23）である。
同日の公式CTAN TRIPも両段exit 0、`tripos.tex`一致、DVI hashは既知正常値
`b20af20a1463c6846f0c4c1ce687cd6354ce1a5f65ee401507627570787ae9fe`を維持した。
このmachineにはDVItypeが無いため、今回のrecord意味比較はhash一致で代替している。

plain formatの欧文DVIは`origin/main`のrTeXにopcode・座標を含めて完全回帰させる。
TRIP再現用の単精度`glue_set`境界は`trip` featureだけへ閉じ込める。通常版の基準fixtureでは
2026-08-23にpage body 183 bytesの差分0を確認した。LaTeX DVIの完全回帰は`latex.ltx`を
LaPraTeX用に適合させるまでは要求しない。

性能gateは機能完成後に一度だけ測るものではない。JFM/TFM接続、spacing finalizer、縦組、
provider registry、resolver、OTF等の主要sliceごとに、探索、fmt読込み、字句化、組版、DVI出力を
分けて測る。外部process時間をPraTeX本体から隠さず、cold/warmとhit/missを同じfixtureで記録する。

### 既知の落とし穴

1. `scan_toks`は`def_ref`を作り直す。入れ子走査には`token_lists::nested_scan_toks`を使う。
2. `\detokenize`と`\unexpanded`は`\the`と同様、結果へ直に足す。差し戻して再展開しない。
3. 現在の条件は`cond_stack`でなく`scanner.cur_if` / `if_limit`にあり、stackは外側の控えである。
4. `GroupType`は並び順で写さない。PraTeXにはTeXにない`AlignEntry`がある。
5. logは79桁で折り返す。試験照合には`join_log`を使う。
6. fmtへrun-local provider ID、pointer、WASM handle、cache世代を保存しない。

---

## 現在地

詳細な対応表は `docs/feature-inventory.md`、生きた引継ぎは `docs/HANDOFF.md` を優先する。

- e-TeX/pdfTeX primitiveを追加し、`expl3-code.tex`の未定義primitive段階は通過済み。
- 限定fixtureでは公式`latex.ltx`からfmtをdumpし、最小articleをDVI/PDFまで処理済み。
- 通常のLaTeX資材では、従来`\kanjiskip`がないためnative UTF-8分岐へ入り、kcatcode 18の
  U+2019を含むhyphenation patternで停止した。K/X追加後の公式CTAN資材での再測定は未実施。
- `\kanjiskip` / `\xkanjiskip`の通常glue parameter面と、検証済みscript class対tableは実装済み。
  自動挿入、xsp/inhibit、JFM class対調整のspacing接続は未実装。
- `\readline`、`\interactionmode`、mark class、糊成分・型変換は実装済み。`\everyeof`は
  `\endinput`との区別が未修正で部分実装。
- `\TeXXeTstate`はfmt読込時0へ戻るが、LR組版自体は未実装。
- 生文字列registerは`docs/raw-string-registers.md`に契約があるだけで、`\rawstring`、
  `\rawstringdef`、`\therawstring`、専用`\showthe`、storage、fmt、production testは未実装。
  font mapが生byteを保存する既存処理は、このregister機能の実装ではない。
- 横組JFMはbounded loader、TeX互換scale、current和文font、`\pratexjfont`と意味が一致する
  範囲の`\jfont` alias、`zw`/`zh`、wide node、DVI `set2`/`set3`まで接続済み。`\tfont`、
  縦組、JFM pair adjustment、自動空白、禁則、PDF和文glyphは未接続。
- plain formatで`\directvaak`、`\vaakdef`、`let` / `var`、host aliasを使う実行例は
  `examples/plain-vaak.tex`。静的失敗はprepare段階・行・桁・診断本文を表示して0へ展開する。
- PDF直接出力、Type 1全埋込み、`ls-R`/`kpsewhich` resolverは部分実装済み。
- 現resolverは曖昧・stale・未対応pathでone-shot `kpsewhich`へ戻る。これは移行実装であり、
  通常lookupの最終設計ではない。
- 名前空間はPhase 0--7済み。Phase 8のTRIPとalignment再利用検証が残る。

## 直近の実装順

1. auto switch、`xspcode`、`inhibitxspcode`をtyped state化し、実spacingの入力を揃える。
2. compile済みscript class対tableをlist単位dispatcherと中央finalizerへ接続する。
3. 横組wide glyphが保持するJFM classをpair adjustmentと中央spacing finalizerへ接続する。
4. `\tfont`と縦組metric/node/outputを追加し、JFM/K/X/禁則を横組から縦組へ広げる。
5. kpathsea互換resolverをrun-global化し、native path解決を広げて通常の子process呼出しをなくす。
6. LaTeXが実際に要求した境界で`\scantokens`等のe-TeX残件を設計どおり実装する。
7. PraTeX-native package adapterを順に通し、PDF直接出力をOTFより先に完成する。
8. Vaak table uploadとversion付きWASM ABIは、内部表現を固定した段階から並行して適合試験を作る。
9. WASM target自体のcompile実験と性能調整は、横組みcheckpoint後に行う。

`\kanjiskip` / `\xkanjiskip`をLaTeX検出だけ通すstubにしない。INITEX既定0、代入、群、
`\globaldefs`、算術、内部量、表示、fmtを既存glue経路へ通し、その後の実spacingへ連続して接続する。

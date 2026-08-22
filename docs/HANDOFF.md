# PraTeX 作業引継ぎ

更新: 2026-08-22 夜

この文書は、現在の Codex セッションから別のエージェントへ作業が移っても、
検証済みの境界と未commitの作業を失わないための生きた引継ぎである。
各作業単位が完了するたびに、commit、試験結果、残件を更新する。

## 最初に守ること

1. リポジトリ直下の `AGENTS.md` を最初から最後まで読む。
2. 現在の枝は `codex/euptex-integration-resume`。`main`を汚さない。
3. 現在の作業木には複数の未commit実装がある。`reset --hard`、checkoutによる破棄、
   一括format、無関係な差戻しをしない。
4. 通常実装はsafe Rustだけ。unsafe Rustを試す場合は、利用者の明示方針どおり専用枝を切る。
   ただしunsafe tuningは「一通り動いた後」まで保留中である。
5. pTeX/upTeX/e-TeX/pdfTeXの実装sourceや上流testを移植しない。公開manual、公開file format、
   自作最小入力による公式binaryの黒箱観測から独立実装する。
6. 標準日本語組版はengine coreの一級機能であり、通常経路をVaak/WASM callbackへ逃がさない。
7. commitは日本語で「なぜ分けたか」を書き、試験名も日本語にする。

## 枝と共有状態

- 枝: `codex/euptex-integration-resume`
- 現在の共有HEAD: `df6e71e`
- `origin/codex/euptex-integration-resume`へ`df6e71e`までpush済み
- 直近の共有commit:
  - `9587e25`: `\mutoglue` / `\gluetomu`
  - `43cd6c9`: e-TeX機能一覧の同期
  - `eda7dd1`: `\scantokens` clean-room設計
  - `df6e71e`: 本引継ぎ文書とREADMEからの導線
- 性能枝 `codex/perf-wsl-euptex-safe` は `9bb6023`までpush済みで、現在は停止中
- Claudeの連絡枝 `origin/claude/for-codex` は `82fa3a2`まで確認済み
- Vaak `origin/codex/main` は `64ccf4e`まで確認済み

## 進行中 1: upTeX `latin_ucs` とUnicode pattern

状態: **実装中、未commit**。現在の作業木の大半を占める。途中で分割して破棄しない。

目的は、通常のTeX Liveで`latex.ltx`を作る際に
`dehypht-x-2024-02-28.pat`の`.buß3`で出る`Nonletter`を解消することである。
一時的に空の`hyphen.cfg`を使うと`latex.ltx`はerror 0でfmtをdumpするため、現在の最初の
hard blockerは未定義primitiveではなくStage 4cである。

実装中の範囲:

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

主な未commit fileは`src/token.rs`、`src/input*`、`src/eqtb*`、
`src/hyphenation*`、`src/command*`、`src/alignment.rs`、`src/math.rs`、
`src/macros.rs`、`tests/latin_ucs.rs`。正確な一覧は必ず`git status --short`で取り直す。

完了条件:

1. focused unit/process testsとfmt roundtripが通る。
2. 通常resolver経由の`latex.ltx`が`.buß3`を越える。
3. 特殊catcode、active/control分離、case cross-lane、detokenizeを黒箱期待値で固定する。
4. corrupt fmtと境界値でpanicしない。
5. ASCII pathへUnicode表引き・heap確保を増やさない。

## 進行中 2: `\lastnodetype` page状態

状態: **実装済み、Unicode作業と同じ作業木で未commit**。

修正前はpage上のpenalty後に空のnested `\setbox`を作ると、e-upTeXは13だがPraTeXは-1へ
化けた。page builderが`LastNodeInfo`だけを保持し、e-TeX node typeを同期していなかったためである。

変更:

- `Eqtb`にpage側のlast node typeを持たせる。
- node typeとlast glue/kern/penaltyの圧縮を一つのhelperへ統一する。
- page開始、page node更新、base vertical list復帰を同じAPIへ通す。
- `tests/etex.rs`に空list、type 1--6/8--13、page→nested box→page復元を追加する。

主なfile: `src/eqtb.rs`、`src/page_breaking.rs`、`src/semantic_nest.rs`、
`src/vertical_mode.rs`、`tests/etex.rs`。

Unicode差分も`src/eqtb.rs`を触るため、部分差戻しで互いを消さない。Unicode compileが安定した後に
focused testを実行し、可能なら意味単位を分けてcommitする。

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

性能統合は利用者の方針により、一通りの機能が動いた後まで保留する。unsafe Rustは試さない。

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

- `Runner::run_writeback`をPraTeXへ接続済み（`7a47ec8`）。実行時errorより前のhost register変更を
  TeX save stack経由で残す。
- Vaak S-22はhost read/write setと`HostBinding::read_at/write_at/len`を持つ。
- PraTeXは`host_touched=Some([])`を安全側の全同期へ倒す。Ref/Freeze/MutMethodの解析漏れに備える。
- Vaak `64ccf4e`ではresolved Placeにより入れ子代入の根cloneを除去した。PraTeXが使う
  `Program2` / `Runner` APIに破壊的変更はない。
- prepared/layoutの正式API、named entry＋引数、typed HostFn完了値、opaque token、
  suspend/resumeはまだない。これらが固まる前にPraTeX phase hookを先走らせない。
- 標準日本語経路のcallback数は0のままにする。

## LaTeXと日本語組版の次順

1. `latin_ucs`＋Unicode patternで通常`latex.ltx`の現在blockerを越える。
2. `\lastnodetype`を検証・commitする。
3. `\scantokens`を設計どおり実装する。
4. JFM reader枝`codex/ptex-jfm-core`の検証済みcoreを統合し、`\jfont`/`\tfont`、wide node、
   DVI `set2/set3`へ進む。
5. `\kanjiskip`/`\xkanjiskip`、禁則、横組、縦組をengine coreへ入れる。

日本語の最低線は横組smokeではなくpTeX相当であり、縦組を含む。割注はP0には含めない。

## 検証

Unicode/lastnodetypeの作業がまとまった後、少なくとも次を順に行う。

```powershell
cargo test --release --locked --test latin_ucs
cargo test --release --locked --test etex lastnodetype
cargo test --release --locked --no-fail-fast
```

その後、既存のTRIP作業rootを再利用する。

```powershell
pwsh -NoProfile -File tools/run-trip.ps1 `
  -WorkRoot "C:\Users\868ha\AppData\Local\Temp\rtex-trip-20260822-163044-db7f94d9" `
  -Step Build,Stage1,Stage2,Compare
```

直前の既知正常値:

- release: 507 passed、0 failed、6 ignored
- TRIP Stage1/Stage2 exit 0
- `tripos.tex`正規化後一致
- DVI SHA-256: `b20af20a1463c6846f0c4c1ce687cd6354ce1a5f65ee401507627570787ae9fe`
- 既知の999 records差分は許容項目除外後の意味差0

Unicode table追加でtext fmtのsizeは増える。増分を実測し、破損fmtがparse errorになることを確認する。

## その他の利用者指定

- binary/banner名はPraTeX。quiet modeとREADME更新は既に別commitで入っている。
- LaTeX名はLaPraTeX。
- TCXはWeb2C input translation profileとして未実装。xord/xchrや`^^`記法と混同しない。
- `^^^^hhhh` / `^^^^^^hhhhhh`は未実装。
- OTF/RustyBuzzは低優先かつdefault-off方針。現在は触らない。
- catcodeは`repr(u8)`。catcode、kcatcode、将来の外部分類IDを同じdomainへ潰さない。
- `for_CLAUDE.md`へClaude/Vaak向けの変更とAPI要求を追記し、commit時に
  `origin/claude/for-codex`をfetchする。

## 引継ぎ直後の安全な確認

```powershell
git status --short --branch
git log --oneline --decorate -8
git diff --stat
```

作業木がdirtyなのは現在正常である。まずこの文書と`for_CLAUDE.md`、
`docs/euptex-port-notes.md`、`docs/etex-texxet-status.md`を読み、誰の変更かを確認してから触る。

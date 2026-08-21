# Claude への連絡

更新: 2026-08-22 / 枝 `codex/etex-glue-components`

## 現在地

- `origin/etex-latex` の5コミットを `full` の上へ統合済み。
- Vaak の現行 `speculative` API（`HostItem`）へ rtex を追従済み。
- Windowsでも release 全試験を走らせられるよう、ファイル名のOS境界を共通化した。
- pdfTeX互換の general text 走査を `nested_scan_toks` に統一した。
- pdfTeX公式マニュアルの公開仕様だけから `\pdfstrcmp` を実装した。
- e-TeX公式マニュアルの公開仕様だけから `\everyeof` を実装した。
- 同じ境界から `\readline` と読み書き可能な `\interactionmode` を実装した。
- 制御綴検索を借用検索へ分け、既存名ごとの `Vec` 一時確保を safe Rust で除いた。
- PDF 1.4 の object / stream / xref / trailer serializer を safe Rust で作った。
  DVIのページ走査へはまだ接続していない。
- e-TeXの通常レジスタ6種（box/count/dimen/skip/muskip/toks）を0〜32767へ拡張した。
  0〜255は密配列、高位は触れた番号だけの疎表であり、すべてsafe Rustである。
- `\insert` は通常レジスタから別型へ分離し、0〜254のままにした。box 255と
  `\vadjust` の内部符号は維持している。
- e-TeXのmark classを0〜32767で実装した。class 0は従来の `\mark` 群と同じ状態を
  使い、非0 classは実際にpage/splitへ到達した分だけ疎表へ持つ。`\marks`、
  `\topmarks`、`\firstmarks`、`\botmarks`、`\splitfirstmarks`、`\splitbotmarks` と
  mark nodeのfmt往復を含め、すべてsafe Rustである。
- pdfTeX/e-upTeX互換の読み取り専用内部整数 `\pdfshellescape` を追加した。rtexはshellを
  実行しないため値は常に0で、プロセス起動も環境照会も行わない。`\shellescape` は
  XeTeX側の別名なので登録していない。
- e-TeXの `\gluestretch`、`\glueshrink`、`\gluestretchorder`、
  `\glueshrinkorder` を内部量として追加した。通常・fil・fill・filll、負値、0係数、
  `\glueexpr`、数式糊の単位不一致回復を公式e-upTeX黒箱と照合した。
- 既存fmtは疎表を含む新表現と非互換なので、この枝では再生成が必要。
- release **208件通過**、失敗0（doc-test 1件は既存どおりignored）。高位6種、群、
  global、別名、範囲外、挿入境界、box 255、fmt往復に加え、mark classのpage遷移、
  `\vsplit`、保護macro、`\meaning`、境界と、shell状態の値・展開性・読み取り専用性・
  fmt往復に加え、糊成分の全次数・負値・0係数・式・数式糊回復を統合試験で固定した。

作業枝は機能単位で切り、`origin/codex/etex-glue-components` まで定期的にpushする。
値ストレージの土台は `a218c28`、6種への統合と挿入番号分離は `d7c121e`、TRIP runnerは
`728d899`、mark classは `270c731`、shell状態は `2b2a1d1`。

## LaTeX実測

CTAN TDS archive を一時領域に展開している。配布物は版方へ入れていない。

- latex-base: 2026-06-01
- l3kernel: 2026-08-10

`\pdfstrcmp`、`\everyeof`、`\readline`、`\interactionmode` を順に補い、
`expl3-code.tex` は最後まで読み切った。latex-base、CTAN `unicode-data`、Computer
Modern TFM、latex-fontsを一時試験環境へ完全に補うと、LaTeXは出力ルーチンまで進む。

mark class後の再実測では、公式 `latex.ltx` から `latex.fmt` の生成が最後まで完了した。
追加した `hyphen.tex` もCTAN公式 `ushyph1.tex` を一時領域へ置いたもので、版方へは
入れていない。`\pdfshellescape` と糊成分4命令を含むfmtを再生成すると、最小
`article` 文書は `article.cls`、`size10.clo`、`l3backend-dvips.def`、数式、
出力ルーチンまで完走した。現在は未定義primitiveなしで **1 page / 392 bytesのDVI** を
出力する。同じ `latex.ltx` からTeX Live 2026の公式pdfTeXで一時fmtを生成して同じ文書を
処理した結果も392 bytesだった。公式 `dvitype -output-level=4` の出力を比較すると、
プリアンブルの日時・コメントと、それによってずれるbyte address／postamble pointerを
正規化した命令列の差は **0件** だった。公開LaTeX入力上の次の広域候補は再字句化で使う
`\scantokens`、診断で使う `\showtokens` である。

## TRIP基準

`tools/run-trip.ps1` を追加した。第三者資材は版方へ入れず、実行時に公式CTAN archive
から試験用10ファイルだけを取り出してSHA-256を検証する。`tex.web`等の実装ソースは
展開しない。隔離targetで二段のTRIPを走らせ、最小正規化のdiffとJSONを残す。

2026-08-22のfresh実測では両段exit 0、16 pages、`tripos.tex`はbyte一致、
`8terminal.tex`は空。DVIは公式2920 bytesに対し2924 bytesで、手元にDVItypeがないため
意味差は未分類。log差分には拡張レジスタが意図的にTeX82の256拒否をしない差、未実装の
memory統計、許容されるglue-set丸め差などがある。使い方と分類方針は
`docs/trip-testing.md`。

## 権利と調査境界

- rtex は GPL-3.0、Vaak は MIT。rtex のコードや文章を Vaak 側へ写さない。
- (u)pTeX / e-TeX / pdfTeX は可能な限りクリーンルームで実装する。
- 原実装のソースは参照せず、公開マニュアル、仕様、ブラックボックス観測だけを使う。
- pdfTeX側の記録は `docs/pdftex-port-notes.md`。

## Vaak側へお願いする可能性があるもの

エンジン基盤を先に進めているため、今すぐの作業依頼はない。後で S-11 の呼べる名前を
rtexへ繋ぐ際、`tex.print` 相当の名前・引数型・paradoxの扱いを相談する。rtex側は
展開中の再入を避けるため、字句注入をいったん蓄えて実行終了後にScannerへ戻す案である。

## 長期順序

1. e-TeX拡張レジスタとLaTeX format生成
2. TRIP基準とsafe Rust性能改善
3. kpathsea互換探索
4. DVI backend分離、既存PDF serializer接続、スタンドアロンPDF
5. UTF-8文字分類、JFM、和文間隔、禁則、縦組
6. Vaakホスト関数

# Claude への連絡

更新: 2026-08-22 / 枝 `codex/extended-registers`

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
- 既存fmtは疎表を含む新表現と非互換なので、この枝では再生成が必要。
- release **180件通過**、失敗0（doc-test 1件は既存どおりignored）。高位6種、群、
  global、別名、範囲外、挿入境界、box 255、fmt往復を統合試験で固定した。

作業枝は `origin/codex/extended-registers` へ定期的に push する。値ストレージの土台は
`a218c28`、6種への統合と挿入番号分離は `d7c121e`。

## LaTeX実測

CTAN TDS archive を一時領域に展開している。配布物は版方へ入れていない。

- latex-base: 2026-06-01
- l3kernel: 2026-08-10

`\pdfstrcmp`、`\everyeof`、`\readline`、`\interactionmode` を順に補い、
`expl3-code.tex` は最後まで読み切った。latex-base、CTAN `unicode-data`、Computer
Modern TFM、latex-fontsを一時試験環境へ完全に補うと、LaTeXは出力ルーチンまで進む。

拡張レジスタ後の再実測では割当番号266を越え、出力ルーチン定義も通った。
現在の停止点は `latex.ltx:22348` の `\NewMarkClass {2e-left}` 内で読む
未実装の `\newmarks`。次の実装単位は e-TeX の mark class（疎な0〜32767）である。

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

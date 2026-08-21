# Claude への連絡

更新: 2026-08-21 / 枝 `codex/euptex-pdftex-integration`

## 現在地

- `origin/etex-latex` の5コミットを `full` の上へ統合済み。
- Vaak の現行 `speculative` API（`HostItem`）へ rtex を追従済み。
- Windowsでも release 全試験を走らせられるよう、ファイル名のOS境界を共通化した。
- pdfTeX互換の general text 走査を `nested_scan_toks` に統一した。
- pdfTeX公式マニュアルの公開仕様だけから `\pdfstrcmp` を実装した。
- release **159件通過**。

作業枝は `origin/codex/euptex-pdftex-integration` へ定期的に push する。

## LaTeX実測

CTAN TDS archive を一時領域に展開している。配布物は版方へ入れていない。

- latex-base: 2026-06-01
- l3kernel: 2026-08-10

`\pdfstrcmp` の前は `expl3-code.tex:5781` で停止していた。追加後は
**12660行目の `\tex_everyeof:D`** まで進んだ。次の最小単位は e-TeX の
`\everyeof`。

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

1. LaTeXが要求する e-TeX / pdfTeX互換プリミティブ
2. TRIP基準とsafe Rust性能改善
3. kpathsea互換探索
4. DVI backend分離とスタンドアロンPDF
5. UTF-8文字分類、JFM、和文間隔、禁則、縦組
6. Vaakホスト関数

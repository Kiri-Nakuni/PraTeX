# LaTeX class/package互換性の実測

2026-08-24時点の結論は、`article`、最小`scrartcl`、`graphicx`、`xcolor`、
`hyperref`、TikZ/PGF、`siunitx`、代表`prjsarticle`は同一のPraTeX生成`latex.fmt`から
DVIまで到達する。`pxrubrica`はgeneric fallbackで最小文書が通るだけで、
PraTeX-native対応とは数えない。

これは最小入力のload/compile smoke testである。package全API、DVI specialの後段driver、
表示品質、LaTeX DVIの他engineとの一致、pTeX/upTeX互換、JLReq適合を保証しない。

## 固定した実行条件

- PraTeX: `pdfmdfivesum file`、raw string、virtual K、`eTeXrevision`と、並行中の
  resolver/font lookup作業を含む2026-08-24の統合worktreeのrelease executable。
  `eTeXrevision`差分単独のbuildではない。3,429,888 bytes、SHA-256
  `13fe597ab50115b683677843fd3866e2afa0710a2760dad95a4a8b6487a890ff`
- `latex.fmt`: 上のPraTeX自身が公式資材から一度だけ生成、17,698,581 bytes、
  SHA-256 `ca5d8c97c62107ec1d69e93f7cab1c53587ac4c0e32fea7e85d15449a411055f`
- format: LaTeX2e 2026-06-01、L3 programming layer 2026-08-10、生成logの`!`は0件
- 全probeは同じexecutableと同じ`latex.fmt`を使用し、DVI modeで実行した。
- `graphicx`、`xcolor`、`hyperref`には`dvips` driverを明示した。`siunitx`にもDVI用
  color driverを明示した。PraTeXをpdfTeX、pTeX、upTeX等として偽装していない。

TikZの成功をrunnerの期待値へ反映した後、並行buildでrelease binaryのhashが
`52b21c693a28283f92e0d9697da62dbb04316a840f1b4871260aca1b79cc6baf`へ更新されたため、
上記prepared fmtとの再実行も行った。runnerはexit 0で、9 probeのDVI bytes/hashは上表の
初回実測とすべて一致した。

共有`target/release/pratex.exe`は並行作業で更新され得るため、runnerが実行時のbinary/fmt hashを
`result.json`へ固定する。以前の表は最終runnerとは別の探索runのDVI hashを、固定条件へ
誤って混ぜていた。保存済み
`9a68851` binaryと上記統合binaryを、それぞれのbinaryが生成した対応fmtで再実行すると、
従来成功していた7 probeのDVIは全fileでbyte単位に一致した。したがって最初の差分offsetも
record意味差もない。両binary間で同一fmtを共用する追加実験は、raw string等によるfmt schema
変更のため旧binaryが`Format error`で拒否した。上表は保存された最終runnerの再現値に統一し、
以前の値が混入した文書provenance誤りを訂正した。

## 結果matrix

| 対象 | 実測 | exit / `!` | DVI bytes / SHA-256 | 判定と最初の制限 |
|---|---:|---:|---|---|
| `prjsarticle` 0.3 | 到達 | 0 / 0 | 2,636 / `112cc36111479242bdbcbd093e549dd8224fd12c654f9a4c3ec9813737523ea8` | 和文4属性、exact JFM+sp cache、従属欧文relationを使い、`maketitle`、節、list、和欧混植を横組DVI化した限定smoke。JLReq完全対応ではない。 |
| `article` 1.4n | 到達 | 0 / 0 | 432 / `868d392a535b054db9e5329a1dd03678d0503831b20d9a5b4ba9aa4fead402db` | 欧文baseline。 |
| `scrartcl` 3.49.2 | 到達 | 0 / 0 | 496 / `bcf8881826166f9b021e7d29394213b0af7ebcbe2ef26c4dd577eed0e291602b` | classを無改変でloadし、sectionをDVI化。KOMA-Script全体の互換性は未確認。 |
| `graphicx` 1.2e | 到達 | 0 / 0 | 648 / `f5dc83fa6438e00869c2fc4e3f0b865fed2ba6e3e38bfc16aa0f331ed841167f` | `rotatebox`と`scalebox`のDVI special smoke。外部画像の探索・変換は未試験。 |
| `xcolor` 3.02 | 到達 | 0 / 0 | 592 / `1cc8deba184d0273ba38f5f7b0156052f67b843a315c3afee51f9f4183508624` | 文字色と背景色のDVI special smoke。 |
| `hyperref` 7.01r | 到達 | 0 / 0 | 2,696 / `b875cb068bf4b52cb9c8939610059196a159c2552795639f2734fd8906dc2d0d` | linkとURIをDVI化。`pdfmdfivesum file{...}`がPraTeX resolver経由で動作し、従来のload blockerを解消した限定smoke。 |
| TikZ/PGF 3.1.12 | 到達 | 0 / 0 | 11,024 / `a8f854fe0176061fdb48dc1d656068a333e906cc872ef18f695f01f8fb5a9e03` | `eTeXrevision=.6`を公開契約どおり追加し、三角形とnodeのDVI smokeまで到達。TikZ/PGF全APIの互換性は未確認。 |
| `siunitx` 3.5.5 | 到達 | 0 / 0 | 588 / `293710fcd41be438117dc79b6cf5c25d5bd202c1b7b18f1f6742476b6e4f9e4d` | `qty`と不確かさ付き`num`のsmoke。 |
| `pxrubrica` 1.3e | fallbackのみ到達 | 0 / 0 | 412 / `ae2f9462770683ca5d30d7342b13cbdd608a8556b27a5c457a6adc0464ed4b20` | 正式な熟語ruby構文`ruby[j]{日本語}{に\|ほん\|ご}`は処理できたが、logは`PRATEX-PXRUBRICA-UNICODE-BRANCH=0`。native対応ではない。 |

`hyperref`は名前だけの`pdfmdfivesum`存在確認では通らなかったが、packageが使用する
`file{...}` scanner契約とfile byte列のMD5を実装して到達した。これは上表の最小文書に限る。
TikZ/PGFは`eTeXversion`だけではgateを越えなかったが、展開可能な
`eTeXrevision=.6`の追加後は上表の限定smokeへ到達した。

`pxrubrica`の既存Unicode/pTeX系branchは、`kchardef`、和文spacing、penalty等の
engine固有契約を前提にする。engine identityやprimitiveを偽装するadapterは作らなかった。
PraTeX固有feature queryと実在する和文node/spacing契約を使う隔離adapterは未完成である。

## 再現方法

probe sourceは[package-compat examples](examples/package-compat/)にあり、runnerは
[test-package-compat.ps1](../tools/test-package-compat.ps1)である。通常のTeX Liveから必要fileを
集める場合も、runnerへ渡す`RuntimeRoot`はruntime fileを平坦化したrepository外directoryにする。
`latex.ltx`、標準`hyphen.cfg`、各packageと依存、TFM/JFMを含める。repository内の空の
`tests/fixtures/prjsarticle/hyphen.cfg`は拒否される。

```powershell
pwsh -File tools/test-package-compat.ps1 `
  -PraTeXPath C:\path\to\pratex.exe `
  -RuntimeRoot C:\path\to\flat-ctan-runtime
```

runnerは新しいrepository外sessionを作り、一つの`latex.fmt`を生成して9 probeへ使う。
既に同じbinary/runtimeから生成したfmtがある場合だけ、次のように再利用できる。

```powershell
pwsh -File tools/test-package-compat.ps1 `
  -PraTeXPath C:\path\to\pratex.exe `
  -RuntimeRoot C:\path\to\flat-ctan-runtime `
  -PreparedFormatPath C:\path\to\latex.fmt
```

成功probeはexit 0、log error 0、非空DVIを要求する。既知blockerは非0 exitだけでなく、上表の
診断signatureも要求する。予期しない成功、別の失敗、signature変化はいずれもrunnerを失敗させる。
各sessionの`result.json`にbinary/fmt hashと実測値を保存する。

## 公式資材の来歴

取得日は2026-08-23から2026-08-24。repositoryへはvendorしていない。基底format資材は
次の公式CTAN archiveを使った。

`\pdfmdfivesum file`のclean-room契約は、公開
[pdfTeX manual](https://tug.ctan.org/systems/doc/pdftex/manual/pdftex-a.pdf)と公式TeX Live 2026
Windows binaryのblack-box観測から固定した。観測に使った
`pdftex.windows.tar.xz`は874,164 bytes、SHA-256
`6794c3c173d1c3e9add63ed3d631b07312c208ed7d60dbed7764f588ce09ee6e`、
bannerはpdfTeX 3.141592653-2.6-1.40.29 / kpathsea 6.4.2である。取得URLは
`https://mirrors.ctan.org/systems/texlive/tlnet/archive/pdftex.windows.tar.xz`。keywordとfilename
general textのmacro展開、UTF-8名、fileの全byte列に対する大文字MD5、不在・読取不能時の
空展開を自作probeで観測した。PraTeX側はさらに、対になった外側quoteだけを除くこと、
任意拡張子の直接相対pathでresolver子processを起動しないことをfocused testで固定した。

| asset / version | URL | bytes | SHA-256 |
|---|---|---:|---|
| latex-base 2026-06-01 | `https://mirrors.ctan.org/install/macros/latex/latex-base.tds.zip` | 49,711,384 | `424bcbab851723495397f0542db8722a68917f31d9f28055ebc65baa7ed35336` |
| l3kernel 2026-08-10 | `https://mirrors.ctan.org/install/macros/latex/required/l3kernel.tds.zip` | 14,556,634 | `342e0ac756b418d095a23eb37aa771a4df3d27db396d43c9e911e0ab9e138aca` |
| unicode-data 1.20 | `https://mirrors.ctan.org/install/macros/generic/unicode-data.tds.zip` | 635,350 | `ef541913356b94a2ed0795e41609b8108db4edf0227080151b865c3a4963c895` |
| KOMA-Script 3.49.2 | `https://mirrors.ctan.org/install/macros/latex/contrib/koma-script.tds.zip` | 9,470,014 | `a9d25d9dbdf7b43842bcb94b6fcef18762d4d7730583c019494b4f5e50995993` |
| latex-graphics 2026-06-01 | `https://mirrors.ctan.org/install/macros/latex/required/latex-graphics.tds.zip` | 3,088,829 | `285842279287adea831ec9019f3b766d91a89d4ee742bcb436ebe7982ad2e684` |
| babel-base 26.9 | `https://mirrors.ctan.org/install/macros/latex/required/babel-base.tds.zip` | 4,071,520 | `4ad3c8e93a20b9dc3ee1437f3063098bab5168abc3e06350900229bd1fefed8b` |
| hyph-utf8 2026-02-21 | `https://mirrors.ctan.org/install/language/hyph-utf8.tds.zip` | 4,737,241 | `d4768692494d8e9b8585cdd8a64edec43c9b9310af55c7414a7004c56ef855fc` |

主要packageは公式TeX Live/CTAN archive
`https://mirrors.ctan.org/systems/texlive/tlnet/archive/<name>.tar.xz`を使った。

| archive | bytes | SHA-256 |
|---|---:|---|
| `xcolor.tar.xz` | 18,300 | `081d28d119901fca408b82a4c12862ed829e45f14a8678034ef02f8ecfd5da54` |
| `hyperref.tar.xz` | 89,224 | `bf387626e9da0fbd1b8df9317215f0c2b653559fc561a3f840370d86c5bb02fe` |
| `pgf.tar.xz` | 719,492 | `901912098c641860184ec2e1ad53f4bfedfc6a7fbd802d14947bef3d7afa65ee` |
| `siunitx.tar.xz` | 70,164 | `733304d41fc117966787b58d287b7cef5f683bdabedc2591462a94164bc41538` |
| `pxrubrica.tar.xz` | 13,692 | `fef58fee914254316e103a38f6b0a496332ad7f9dac90348a83d9ab22d44cadc` |
| `uptex-fonts.tar.xz` | 168,104 | `28541dbebc85163bcf7d8237a16f3b61f4f3803e18c555cc44d6c962db39ac65` |
| `ec.tar.xz` | 263,716 | `bb85425c214b1056b5f0b8f3bf1478b81e89bd7d290d61c7c73289b534786897` |

`hyperref`/`siunitx`の依存closureも同じ公式tlnetから取得した。直接必要になったarchiveは
`kvoptions`、`ltxcmds`、`refcount`、`graphics-def`、`iftex`、`kvsetkeys`、
`kvdefinekeys`、`pdfescape`、`hycolor`、`etoolbox`、`stringenc`、`intcalc`、`url`、
`bitset`、`bigintcalc`、`rerunfilecheck`、`uniquecounter`、`gettitlestring`、
`letltxmacro`、`auxhook`、`atbegshi`、`atveryend`、`infwarerr`、`etexcmds`、
`amsmath`、`translations`、`pdftexcmds`、`tools`である。

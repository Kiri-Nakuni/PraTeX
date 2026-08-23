# LaTeX class/package互換性の実測

2026-08-24時点の結論は、`article`、最小`scrartcl`、`graphicx`、`xcolor`、
`siunitx`、代表`prjsarticle`は同一のPraTeX生成`latex.fmt`からDVIまで到達する。
一方、`hyperref`とTikZ/PGFには再現可能な未対応primitiveがある。`pxrubrica`は
generic fallbackで最小文書が通るだけで、PraTeX-native対応とは数えない。

これは最小入力のload/compile smoke testである。package全API、DVI specialの後段driver、
表示品質、LaTeX DVIの他engineとの一致、pTeX/upTeX互換、JLReq適合を保証しない。

## 固定した実行条件

- PraTeX: source checkpoint `9a68851`を保存したrelease executable、3,390,464 bytes、
  SHA-256 `4ba70b58c5b7e127cccf5a5bf07d79fe0627f157ae71d364b5c4340a99b86530`
- `latex.fmt`: 上のPraTeX自身が公式資材から一度だけ生成、17,695,121 bytes、
  SHA-256 `cfb4990020c3e39c4b660dcd2763b11fda34374cfe83eaf2257d1a5fd6707cb8`
- format: LaTeX2e 2026-06-01、L3 programming layer 2026-08-10、生成logの`!`は0件
- 全probeは同じexecutableと同じ`latex.fmt`を使用し、DVI modeで実行した。
- `graphicx`、`xcolor`、`hyperref`には`dvips` driverを明示した。`siunitx`にもDVI用
  color driverを明示した。PraTeXをpdfTeX、pTeX、upTeX等として偽装していない。

共有`target/release/pratex.exe`は並行作業で更新され得るため、この測定では上記hashの
保存binaryを使った。fmtとbinaryが一致しない場合の結果はこの表へ混ぜない。

## 結果matrix

| 対象 | 実測 | exit / `!` | DVI bytes / SHA-256 | 判定と最初の制限 |
|---|---:|---:|---|---|
| `prjsarticle` 0.1 | 到達 | 0 / 0 | 1,180 / `0ab165c189cf50cf6bea03b73aaee55d407415c0e51f5e4b6a6c2172daac7338` | repositoryの`upjisr-h` adapterを外付けし、`maketitle`、節、list、和欧混植を横組DVI化した限定smoke。JLReq完全対応ではない。 |
| `article` 1.4n | 到達 | 0 / 0 | 432 / `748db4613cfc841ffff66e1cd0a423a74dbcc9c4bb86e38fb5f4cdaebf105990` | 欧文baseline。 |
| `scrartcl` 3.49.2 | 到達 | 0 / 0 | 496 / `2da436e2758b3846ec9b83f21a60769bc66e5299f3cb8c4ee062d40769e6a668` | classを無改変でloadし、sectionをDVI化。KOMA-Script全体の互換性は未確認。 |
| `graphicx` 1.2e | 到達 | 0 / 0 | 648 / `f8fea40ac47e62ad910a6a3a0c2e69b03127edbe91b1e8ce115b2fb326721c30` | `rotatebox`と`scalebox`のDVI special smoke。外部画像の探索・変換は未試験。 |
| `xcolor` 3.02 | 到達 | 0 / 0 | 592 / `85de0d33cbc6f5cfd1ff8a228ca82559278b95fb45f8f3305d6d9aa1624273df` | 文字色と背景色のDVI special smoke。 |
| `hyperref` 7.01r | blocker | 1 / 2 | なし | `begin{document}`で `Missing { inserted.`。展開stackは `pdf@filemdfivesum -> pdfmdfivesum file{...}`であり、PraTeXの`pdfmdfivesum`にfile形式がない。 |
| TikZ/PGF 3.1.12 | blocker | 1 / 2 | なし | package load中に `eTeXrevision`が未定義で、`Package PGF Error: PGF requires etex in extended mode.`。 |
| `siunitx` 3.5.5 | 到達 | 0 / 0 | 588 / `58bbb23e4a5a6cea6ec016d2700655d1bf27d7d436b2eb71827f1d9e321806a3` | `qty`と不確かさ付き`num`のsmoke。 |
| `pxrubrica` 1.3e | fallbackのみ到達 | 0 / 0 | 412 / `d90cd2e2a9dafe8938374abdc5d0d8b491e3fb24de90f0e48077492603bc5cfe` | 正式な熟語ruby構文`ruby[j]{日本語}{に\|ほん\|ご}`は処理できたが、logは`PRATEX-PXRUBRICA-UNICODE-BRANCH=0`。native対応ではない。 |

`hyperref`については名前だけの`pdfmdfivesum`存在確認では不足する。packageが使用する
`file{...}` scanner契約まで実装・試験する必要がある。TikZ/PGFについても`eTeXversion`だけでは
gateを越えず、公開e-TeXの`eTeXrevision`契約が必要である。

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

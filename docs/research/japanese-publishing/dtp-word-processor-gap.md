# DTP・word processorと日本語TeXの機能差

更新: 2026-08-24

## 結論

TeXは小説・エッセイの規則に基づく大量組版に向く。現行TeX系が弱いのは最終的な描画能力より、
高度な機能を同時に扱う共通semantic model、対話的な編集・校正、印刷入稿、accessibilityまでを
一続きにするtoolingである。

[W3C JLReq](https://www.w3.org/TR/jlreq/?lang=ja)は縦横組、文字class、行長調整、ruby、圏点、
縦中横、割注、行取り、柱・ノンブル、注、図表を一体の仕様として扱う。
[JAGATのDTP curriculum](https://www.jagat.or.jp/cat5/dtp/exam/curriculum/1-5)も禁則、約物、文字詰め、
行長調整、組方向、複合fontを中核に置く。

## 製品別の強み

| 領域 | InDesign | Affinity | Word | 一太郎 | 現行TeX |
|---|---|---|---|---|---|
| 日本語縦組 | 強い | V2は非対応、現行も要実測 | 対応 | 小説用途として強い | B: engine/package依存 |
| ruby・圏点・縦中横・割注 | 専用UIとlayout object | 日本語機能は未確認 | 東アジア機能あり | 専用機能あり | B: 複数packageに分散 |
| 禁則・文字組・追込み/追出し | 日本語composerとpreset | 欧文DTP中心 | 文書grid・禁則 | 見開き行合わせ等 | B: class/engineごとに分散 |
| style・長文構造 | 強い | 強い | 強い | 小説preset | A–B: source定義は強い |
| 校正・変更履歴・comment | review機能 | 限定 | 非常に強い | 小説校正が強い | C–D |
| 印刷preflight・PDF/X・色 | 非常に強い | 一般DTPとして強い | 高級製版向けではない | POD/入稿向け | B–C、統合tool不足 |
| tagged PDF | 対応 | 要実測 | 対応 | 不明/限定 | 日本語stackはD |
| EPUB | 対応するが日本語制約あり | 対応 | 外部変換中心 | EPUB 3.0.1 | C、adapter/実機QAが必要 |

### InDesign

日本語段落composer、文字組preset、利用者定義禁則、追込み優先順位、ぶら下がりを持つ。

- [CJK composition](https://helpx.adobe.com/jp/indesign/using/composing-cjk-characters.html)
- [文字組設定](https://helpx.adobe.com/indesign/desktop/language-and-proofing/chinese-japanese-and-korean/customize-mojikumi-spacing-sets.html)
- [禁則設定](https://helpx.adobe.com/indesign/desktop/language-and-proofing/chinese-japanese-and-korean/use-kinsoku-settings.html)
- [ruby](https://helpx.adobe.com/indesign/desktop/language-and-proofing/chinese-japanese-and-korean/add-and-format-ruby-text-annotations.html)、
  [圏点](https://helpx.adobe.com/indesign/desktop/language-and-proofing/chinese-japanese-and-korean/apply-kenten-to-text.html)、
  [縦中横](https://helpx.adobe.com/indesign/desktop/language-and-proofing/chinese-japanese-and-korean/apply-tate-chu-yoko-in-vertical-text.html)、
  [割注](https://helpx.adobe.com/indesign/desktop/language-and-proofing/chinese-japanese-and-korean/add-and-update-warichu-options.html)

frame grid、回込み、脚注、柱、索引、style、GREP検索、変更履歴も一document modelにある。
印刷では[PDF export](https://helpx.adobe.com/indesign/desktop/save-export-and-publish/save-and-export/adobe-pdf-export-options.html)、
[preflight](https://helpx.adobe.com/indesign/desktop/print/preflight/configure-and-use-the-preflight-panel.html)、
[package](https://helpx.adobe.com/indesign/desktop/print/preflight/package-files-for-output.html)、
[overprint](https://helpx.adobe.com/indesign/desktop/print/color-output-and-separations/overprint-strokes-and-fills.html)を
一工程にする。PraTeXは座標編集を模倣するより、規則組版のsemantic objectと検査APIを参考にする。

### Affinity

[Affinity Publisher V2はRTL/vertical text非対応](https://support.serif.com/hc/en-us/articles/10259496602895-Does-Affinity-V2-support-Right-to-Left-or-Vertical-text)
と明記される。一方、一般DTPのmaster page、style、baseline grid、PDF/X、preflight、bleed、CMYK/ICCは
強い。[preflight](https://affinity.help/publisher2/English.lproj/pages/Publishing/preflight.html)と
[PDF export](https://affinity.help/publisher2/English.lproj/pages/Publishing/exportSettings.html)を、
日本語組版ではなく入稿toolingの比較対象にする。

### Microsoft Word

[変更履歴](https://support.microsoft.com/en-us/word/training/track-changes-in-word)、
[comment](https://support.microsoft.com/en-US/Word/give-and-receive-feedback-in-word)、
[style](https://support.microsoft.com/en-us/word/customize-or-create-new-styles)、共同編集が強い。
東アジア向けには[ruby](https://learn.microsoft.com/en-us/globalization/fonts-layout/ruby)、
[禁則OOXML](https://learn.microsoft.com/ja-jp/openspecs/office_standards/ms-oe376/1ed6a072-e2ec-4b71-a42d-20f007bd097d)、
[文書grid](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.docgrid?view=openxml-3.0.1)、
[縦中横API](https://learn.microsoft.com/ja-jp/office/vba/api/word.range.horizontalinvertical)がある。
ただし[PDF出力](https://support.microsoft.com/en-us/office/save-or-convert-to-pdf-or-xps-in-office-desktop-apps-d85416c5-7d77-4fd6-a216-6f4bf7c7c110)は
PDF/X、特色、overprint、裁ち落とし等を統合した高級製版backendではない。

### 一太郎

一太郎は小説を明示的な第一級用途にし、文庫/新書/公募preset、会話文・地の文、段落字下げ、
二倍ダーシ、三点リーダー、縦組、ruby/圏点、見開き行合わせ、行取り見出し、奥付、扉、塗足し、
ノンブル、PDF入稿を統合する。

- [一太郎の小説執筆機能](https://www.justsystems.com/jp/products/ichitaro/features/feature03.html)
- [校正と縦組](https://www.justsystems.com/jp/products/ichitaro/features/feature02.html)
- [印刷・PDF](https://www.justsystems.com/jp/products/ichitaro/features/feature05.html)
- [縦中横・割注](https://support.justsystems.com/faq/1032/app/servlet/qadoc?QID=039121)
- [mono/group ruby](https://support.justsystems.com/faq/1032/app/servlet/qadoc?QID=031032)

PraTeXの小説class、LSP、preflight設計で最も直接的な比較対象にする。

## 日本語本文組版のgap

| 機能 | 現行評価 | PraTeXで置く層 | 必須の不変条件 |
|---|---:|---|---|
| 縦組 | B | engine core + font/output | glyph方向、座標、抽出文字、横組との切替 |
| mono/group/jukugo ruby | B | semantic ruby node + package | 親/注の長短、行頭末、分割、両側、ActualText |
| 圏点 | B | annotation lane + package | rubyとの衝突、縦横、色、tag |
| 縦中横 | B | 固定`InlineObject` | 本文gridを変えず、抽出順を保つ |
| 割注 | B | 分割可能`InlineSubflow` | 自動二行化、複数行/段/page境界、単回処理で収束 |
| 禁則 | B | 文字class対break table | JLReq全class、連続約物、利用者table |
| 追込み/追出し | B | 優先順位付きline adjustment | 詰め/空き縮小/拡大/追出しの順と閾値 |
| 文字詰め・文字組 | B | pair spacing min/natural/max/priority | 本文/見出しpreset、glyph座標 |
| ぶら下がり | B | line-edge action | none/regular/force、禁則優先、TrimBox |
| 行grid | B | baseline/grid core + class | ruby/割注/見出し/注があっても見開き基線一致 |

## page buildingのgap

| 機能 | 評価 | 必要な境界 |
|---|---:|---|
| 段組 | B | 段抜き図、脚注、最終page balancing、縦組を合成するpage builder API |
| 脚注 | B | 分割可能note、複数page、段抜き、縦組、reading order |
| 見出し・行取り | A–B | page/column break、奇偶page、指定行取り、outline/tag |
| 柱・ノンブル | A | mark core、白page、章開始、artifact tag |
| 扉・奥付 | A | 右/左開き、白page、番号継続/非表示、front/main matter |
| 画像回込み | B | anchored object、paragraph shape、reflow、reading order |
| 表 | B | 複数page、見出し反復、縦書cell、注、tagged table |
| 索引 | A | 読み付き日本語sort、異体字、複数索引、再現性 |

個別packageが単独で描画できることより、段、float、脚注、回込み、割注をtyped APIで合成できることを
重視する。

## 執筆・校正・入稿のgap

| 機能 | 評価 | PraTeX側 |
|---|---:|---|
| 小説校正 | C | LSP profile。会話文、字下げ、ダーシ、リーダー、括弧、表記揺れ |
| 変更履歴・comment | C–D | stable object ID、review operation、GUI/API |
| 入稿package | C | source、画像、font license、ICC、PDF、log、profile、hash manifest |
| 裁ち落とし・トンボ | B | PDF boxとpackage。面付けとは分離 |
| CMYK/ICC | B | DeviceCMYK/ICCBased/output intent、RGB negative test |
| 特色・overprint | B | `/Separation`、spot名、overprint/knockout、分版検査 |
| PDF/X | B–C | backend profileと独立validator、positive/negative fixture |
| preflight | C | missing font/link、解像度、overset、RGB、ink、box、tagをbuild failure化 |
| IVS・異体字 | D | font identity/shaping、cmap 14、AJ1 CID、selector UI、copy/paste |
| tagged PDF | D | semantic tree、reading order、ruby、柱artifact、PDF/UA validator |
| EPUB | C | semantic IR、EPUB 3.3、vertical/ruby、EPUBCheck、複数reader |

## 現行TeXで実用上不足する統合機能

- 縦組preview上のhit-testとsource spanへの逆対応
- ruby、割注、圏点、図、注、tagを同じsemantic objectとして保持するtree
- overset、grid逸脱、禁則fallback、missing glyph、RGB混入の常時preflight
- 色分解、overprint、ink coverage preview
- 著者ID付き変更提案、comment、accept/reject
- 入稿packageとfont/license/link audit
- IVS/異体字の視覚selectorとcode point/glyph identity表示
- 日本語semantic structureを失わないtagged PDF/EPUB同時出力
- 段、float、脚注、回込み、割注を合成するpage-builder API
- 手動例外をsource objectへ結び、原稿変更後にstaleとして警告する仕組み

## 推奨する層分け

- Engine core: direction、glyph orientation、spacing/break、ruby、`InlineObject`、`InlineSubflow`、
  grid、note/float/page-builder境界、font identity、semantic node、source span。
- Package: jlreq/PXrubrica/font routing adapter、小説用page style、扉、奥付、章、行取り、注、画像、索引。
- PDF backend: font embed、ToUnicode、tag tree、PDF box、ICC、spot、overprint、PDF/X profile。
- Tooling: preflight、入稿package、visual/object/text diff、font/license audit、PDF validator、EPUB exporter。
- GUI/LSP: 縦組preview、source hit-test、ruby/font/IVS inspector、文字組overlay、小説校正、review。
- Versioned API: immutable semantic/layout tree、stable object ID、source span–page box、incremental reflow、diagnostic。

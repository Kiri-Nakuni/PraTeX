# 多言語・混植組版調査

更新: 2026-08-24

## 目的

PraTeXの完成目標は日本語、欧文、和欧混植と、Han unificationを感じさせない正確な和中混植で
ある。アラビア語を含む汎用多言語engineの完成はPraTeX 1のgateへ追加しない。ただしTeX--XeT、
font routing、OpenType shaping、PDF semanticsの型境界を壊さず、将来の拡張と既存package互換性を
評価できるよう、各言語圏の実務と公開標準を調査する。

このfolderは現在の機能一覧ではない。実装状態は[`../../feature-inventory.md`](../../feature-inventory.md)、
CJKVのengine設計は[`../../extensible-layout-roadmap.md`](../../extensible-layout-roadmap.md)、OpenType
package設計は[`../../opentype-package-roadmap.md`](../../opentype-package-roadmap.md)を参照する。

## 読み分け

- [Arabic--English混植](arabic-english-mixing.md)
- [中国語TeX文化、和中・中英混植、Babel、localized Han glyph](chinese-tex-and-cjk-mixing.md)

## 共通のdomain分離

少なくとも次を生の整数や一つの「language」値へ統合しない。

- 入力字句の`InputCategory`と公開`\catcode` / `\kcatcode` view
- Unicode `Script` / `Script_Extensions`とPraTeX `ScriptClassId`
- 組版localeの`LanguageRegion` / BCP 47 tag
- TeX hyphenation tableの`language`
- bidi class、paragraph base direction、resolved embedding level
- font route、OpenType script/language、shaper glyph/cluster ID
- Unicode scalar/IVS、外字、PDF `ToUnicode`上の文字identity

fontや字形を選ぶcontextは増やせるが、source scalarを互換漢字やpresentation formへ置換して
見た目だけを合わせない。glyph orderとlogical text order、描画とcopy/search/taggingも分けて検査する。

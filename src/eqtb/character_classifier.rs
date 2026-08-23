use super::{CatCode, Eqtb, KCatCode};
use crate::token::CjkCategory;

/// 分類規則が呼ばれた場所。
///
/// 組込み表は文脈に依存しない。将来の明示的に有効化された Vaak/WASM 規則が、
/// 入力字句化と表示用の再分類を取り違えないために境界だけを先に固定する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // 後続の raw string / WASM 境界で残り二文脈を接続する。
pub(crate) enum ClassificationContext {
    Input,
    ControlSequenceName,
    SyntheticRescan,
    Detokenize,
}

/// Unicode 一単位のカノンな入力分類。
///
/// `KCatCode` の 14..=20 は upTeX 互換のアクセス符号であり、この型へ入る前に
/// 意味へ写す。通常の TeX category は `CatCode` をそのまま保持するため、
/// `catcode=14` と `kcatcode=14` のような公開数値の衝突を内部 ID に持ち込まない。
/// layout の script/JFM/provider class は入力分類ではないので、この型へ混ぜない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputCategory {
    /// 元の UTF-8 byte 列を、8 bit `catcode` 表で処理する。
    RawBytes,
    /// current Unicode catcodeを持つ欧文一文字token。
    CatCode(CatCode),
    /// 一つの和文 token として処理する。
    Wide(CjkCategory),
}

/// 字句解析器が参照する統一問い合わせ面。
///
/// byte と Unicode は互換性上別の表を保つが、利用側は一つの view だけを受け取る。
/// 拡張規則を有効化していない通常実行では、この組込み view が静的 dispatch される。
pub(crate) trait CharacterClassifier {
    /// 最頻経路。拡張IDを組み立てずTeX82の表だけを返す。
    fn byte_cat_code(&self, byte: u8, context: ClassificationContext) -> CatCode;

    fn unicode_category(
        &self,
        code_point: u32,
        context: ClassificationContext,
    ) -> InputCategory;
}

/// 組込み経路は `Eqtb` 自身へ静的dispatchし、中間objectを作らない。
impl CharacterClassifier for Eqtb {
    #[inline(always)]
    fn byte_cat_code(&self, byte: u8, _context: ClassificationContext) -> CatCode {
        self.cat_code(byte)
    }

    #[inline(always)]
    fn unicode_category(
        &self,
        code_point: u32,
        _context: ClassificationContext,
    ) -> InputCategory {
        let kcat_code = self.kcat_code(code_point);
        let category = match kcat_code {
            KCatCode::Kanji => CjkCategory::Kanji,
            KCatCode::Kana => CjkCategory::Kana,
            KCatCode::OtherKChar => CjkCategory::OtherKChar,
            KCatCode::Hangul => CjkCategory::Hangul,
            KCatCode::Modifier => CjkCategory::Modifier,
            KCatCode::LatinUcs => {
                return InputCategory::CatCode(self.latin_ucs_cat_code(code_point));
            }
            KCatCode::NotCjk => {
                return InputCategory::RawBytes;
            }
        };
        InputCategory::Wide(category)
    }
}

#[cfg(test)]
pub(crate) struct CallbackClassifier<C, K> {
    cat_code: C,
    kcat_code: K,
}

#[cfg(test)]
impl<C, K> CallbackClassifier<C, K> {
    pub(crate) const fn new(cat_code: C, kcat_code: K) -> Self {
        Self {
            cat_code,
            kcat_code,
        }
    }
}

#[cfg(test)]
impl<C, K> CharacterClassifier for CallbackClassifier<C, K>
where
    C: Fn(u8) -> CatCode,
    K: Fn(u32) -> KCatCode,
{
    fn byte_cat_code(&self, byte: u8, _context: ClassificationContext) -> CatCode {
        (self.cat_code)(byte)
    }

    fn unicode_category(
        &self,
        code_point: u32,
        _context: ClassificationContext,
    ) -> InputCategory {
        let kcat_code = (self.kcat_code)(code_point);
        let category = match kcat_code {
            KCatCode::Kanji => CjkCategory::Kanji,
            KCatCode::Kana => CjkCategory::Kana,
            KCatCode::OtherKChar => CjkCategory::OtherKChar,
            KCatCode::Hangul => CjkCategory::Hangul,
            KCatCode::Modifier => CjkCategory::Modifier,
            KCatCode::LatinUcs => {
                // 単体試験adapterにはUnicode catcode callbackを増やさず、低位表と
                // 既定値だけで字句の分岐を試す。実表の試験はEqtb経路で行う。
                let cat_code = if code_point <= u8::MAX.into() {
                    (self.cat_code)(code_point as u8)
                } else {
                    CatCode::OtherChar
                };
                return InputCategory::CatCode(cat_code);
            }
            KCatCode::NotCjk => {
                return InputCategory::RawBytes;
            }
        };
        InputCategory::Wide(category)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kcatcodeの公開数値はcatcode側の意味へ写す() {
        assert_eq!(CatCode::Comment.public_number(), 14);
        assert_eq!(KCatCode::LatinUcs.public_number(), 14);
        assert_eq!(CatCode::Namespace.public_number(), 16);
        assert_eq!(KCatCode::Kanji.public_number(), 16);

        let latin = CallbackClassifier::new(|_| CatCode::Letter, |_| KCatCode::LatinUcs);
        assert_eq!(
            latin.unicode_category(0x00DF, ClassificationContext::Input),
            InputCategory::CatCode(CatCode::Letter),
        );

        let kanji = CallbackClassifier::new(|_| CatCode::Namespace, |_| KCatCode::Kanji);
        assert_eq!(
            kanji.unicode_category(0x4E00, ClassificationContext::Input),
            InputCategory::Wide(CjkCategory::Kanji),
        );
    }

    #[test]
    fn 組込み分類はbyteとunicodeの保存表を混ぜない() {
        let eqtb = Eqtb::new();
        assert_eq!(
            eqtb.byte_cat_code(b'A', ClassificationContext::Input),
            CatCode::Letter,
        );
        assert!(matches!(
            eqtb.unicode_category(0x3042, ClassificationContext::Input),
            InputCategory::Wide(CjkCategory::Kana)
        ));
        assert!(matches!(
            eqtb.unicode_category(0x41, ClassificationContext::Input),
            InputCategory::RawBytes
        ));

        let mut eqtb = eqtb;
        eqtb.kcat_code_define(0x00DF, KCatCode::LatinUcs, true);
        eqtb.latin_ucs_cat_code_define(0x00DF, CatCode::Letter, true);
        assert!(matches!(
            eqtb.unicode_category(0x00DF, ClassificationContext::Input),
            InputCategory::CatCode(CatCode::Letter)
        ));
    }

    #[test]
    fn 組込みcatcodeは一byteである() {
        assert_eq!(std::mem::size_of::<CatCode>(), 1);
    }
}

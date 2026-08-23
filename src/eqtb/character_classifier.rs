use super::{CatCode, Eqtb, KCatCode};
use crate::token::CjkCategory;

/// 文字分類を ABI や token の内部表現から切り離して識別する番号。
///
/// `catcode` と `kcatcode` は公開数値が重なるため、同じ数値空間へ直接
/// 押し込まない。上位領域で出自を区別し、拡張クラスには別領域を予約する。
/// この符号化は現時点では crate 内部用であり、将来の WASM ABI そのものではない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CharClassId(u32);

const KCAT_CODE_DOMAIN: u32 = 0x0001_0000;
#[allow(dead_code)] // versioned extension registry が次段でこの予約領域を使う。
const EXTENSION_DOMAIN: u32 = 0x8000_0000;
#[allow(dead_code)] // 同上。組込み class と混ざらない局所 ID の上限。
const EXTENSION_REGISTRY_ID_MASK: u32 = !EXTENSION_DOMAIN;

impl CharClassId {
    #[allow(dead_code)]
    pub(crate) const fn from_cat_code(cat_code: CatCode) -> Self {
        Self(cat_code as u32)
    }

    pub(crate) const fn from_kcat_code(kcat_code: KCatCode) -> Self {
        Self(KCAT_CODE_DOMAIN | kcat_code as u32)
    }

    /// 中央host registryが一意に割り当てた番号を拡張クラス番号へ変換する。
    /// provider自身の局所番号を直接渡してはならない。
    #[allow(dead_code)]
    pub(crate) const fn from_extension_registry(registry_id: u32) -> Option<Self> {
        if registry_id <= EXTENSION_REGISTRY_ID_MASK {
            Some(Self(EXTENSION_DOMAIN | registry_id))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub(crate) const fn raw(self) -> u32 {
        self.0
    }

    #[allow(dead_code)]
    pub(crate) const fn is_extension(self) -> bool {
        self.raw() & EXTENSION_DOMAIN != 0
    }
}

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

/// Unicode 一単位を、現在の組込み規則でどちらの入力経路へ送るか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnicodeDisposition {
    /// 元の UTF-8 byte 列を、8 bit `catcode` 表で処理する。
    RawBytes { class_id: CharClassId },
    /// upTeX `latin_ucs`: current Unicode catcodeを持つ欧文一文字token。
    LatinUcs {
        class_id: CharClassId,
        cat_code: CatCode,
    },
    /// 一つの和文 token として処理する。
    Wide {
        class_id: CharClassId,
        category: CjkCategory,
    },
}

/// 字句解析器が参照する統一問い合わせ面。
///
/// byte と Unicode は互換性上別の表を保つが、利用側は一つの view だけを受け取る。
/// 拡張規則を有効化していない通常実行では、この組込み view が静的 dispatch される。
pub(crate) trait CharacterClassifier {
    /// 最頻経路。拡張IDを組み立てずTeX82の表だけを返す。
    fn byte_cat_code(&self, byte: u8, context: ClassificationContext) -> CatCode;

    fn unicode_disposition(
        &self,
        code_point: u32,
        context: ClassificationContext,
    ) -> UnicodeDisposition;
}

/// 組込み経路は `Eqtb` 自身へ静的dispatchし、中間objectを作らない。
impl CharacterClassifier for Eqtb {
    #[inline(always)]
    fn byte_cat_code(&self, byte: u8, _context: ClassificationContext) -> CatCode {
        self.cat_code(byte)
    }

    #[inline(always)]
    fn unicode_disposition(
        &self,
        code_point: u32,
        _context: ClassificationContext,
    ) -> UnicodeDisposition {
        let kcat_code = self.kcat_code(code_point);
        let class_id = CharClassId::from_kcat_code(kcat_code);
        let category = match kcat_code {
            KCatCode::Kanji => CjkCategory::Kanji,
            KCatCode::Kana => CjkCategory::Kana,
            KCatCode::OtherKChar => CjkCategory::OtherKChar,
            KCatCode::Hangul => CjkCategory::Hangul,
            KCatCode::Modifier => CjkCategory::Modifier,
            KCatCode::LatinUcs => {
                return UnicodeDisposition::LatinUcs {
                    class_id,
                    cat_code: self.latin_ucs_cat_code(code_point),
                };
            }
            KCatCode::NotCjk => {
                return UnicodeDisposition::RawBytes { class_id };
            }
        };
        UnicodeDisposition::Wide { class_id, category }
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

    fn unicode_disposition(
        &self,
        code_point: u32,
        _context: ClassificationContext,
    ) -> UnicodeDisposition {
        let kcat_code = (self.kcat_code)(code_point);
        let class_id = CharClassId::from_kcat_code(kcat_code);
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
                return UnicodeDisposition::LatinUcs { class_id, cat_code };
            }
            KCatCode::NotCjk => {
                return UnicodeDisposition::RawBytes { class_id };
            }
        };
        UnicodeDisposition::Wide { class_id, category }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 組込み二領域と拡張領域は数値が衝突しない() {
        let comment = CharClassId::from_cat_code(CatCode::Comment);
        let latin_ucs = CharClassId::from_kcat_code(KCatCode::LatinUcs);
        let namespace = CharClassId::from_cat_code(CatCode::Namespace);
        let kanji = CharClassId::from_kcat_code(KCatCode::Kanji);
        let extension = CharClassId::from_extension_registry(16).unwrap();

        assert_ne!(comment, latin_ucs);
        assert_ne!(namespace, kanji);
        assert!(extension.is_extension());
        assert!(!comment.is_extension());
        assert!(CharClassId::from_extension_registry(EXTENSION_DOMAIN).is_none());
    }

    #[test]
    fn 組込み分類はbyteとunicodeの保存表を混ぜない() {
        let eqtb = Eqtb::new();
        assert_eq!(
            eqtb.byte_cat_code(b'A', ClassificationContext::Input),
            CatCode::Letter,
        );
        assert!(matches!(
            eqtb.unicode_disposition(0x3042, ClassificationContext::Input),
            UnicodeDisposition::Wide {
                category: CjkCategory::Kana,
                ..
            }
        ));
        assert!(matches!(
            eqtb.unicode_disposition(0x41, ClassificationContext::Input),
            UnicodeDisposition::RawBytes { .. }
        ));

        let mut eqtb = eqtb;
        eqtb.kcat_code_define(0x00DF, KCatCode::LatinUcs, true);
        eqtb.latin_ucs_cat_code_define(0x00DF, CatCode::Letter, true);
        assert!(matches!(
            eqtb.unicode_disposition(0x00DF, ClassificationContext::Input),
            UnicodeDisposition::LatinUcs {
                cat_code: CatCode::Letter,
                ..
            }
        ));
    }

    #[test]
    fn 組込みcatcodeは一byteである() {
        assert_eq!(std::mem::size_of::<CatCode>(), 1);
    }
}

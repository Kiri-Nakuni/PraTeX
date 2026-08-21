use crate::format::{Dumpable, FormatError};

use std::collections::HashMap;
use std::io::Write;

const DENSE_REGISTER_COUNT: usize = 256;
pub(super) const MAX_EXTENDED_REGISTER_INDEX: u16 = 32_767;

/// e-TeX の拡張レジスタを、TeX82 と共通な低位だけ密に保持する。
///
/// 低位 0..=255 は通常の文書でも頻繁に触るため連続領域に置き、高位
/// 256..=32767 は使った番号だけを確保する。高位の未使用番号を読むと、
/// コンテナが保持する既定値を返す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExtendedRegisterStorage<T> {
    default: T,
    dense: Vec<T>,
    sparse: HashMap<u16, T>,
}

impl<T: Clone> ExtendedRegisterStorage<T> {
    pub(super) fn new(default: T) -> Self {
        Self {
            dense: vec![default.clone(); DENSE_REGISTER_COUNT],
            sparse: HashMap::new(),
            default,
        }
    }

    pub(super) fn get_mut(&mut self, index: u16) -> &mut T {
        Self::assert_valid_index(index);
        if (index as usize) < DENSE_REGISTER_COUNT {
            &mut self.dense[index as usize]
        } else {
            self.sparse
                .entry(index)
                .or_insert_with(|| self.default.clone())
        }
    }

    /// 値を設定し、設定前の値を返す。
    pub(super) fn set(&mut self, index: u16, value: T) -> T {
        std::mem::replace(self.get_mut(index), value)
    }
}

impl<T> ExtendedRegisterStorage<T> {
    pub(super) fn get(&self, index: u16) -> &T {
        Self::assert_valid_index(index);
        if (index as usize) < DENSE_REGISTER_COUNT {
            &self.dense[index as usize]
        } else {
            self.sparse.get(&index).unwrap_or(&self.default)
        }
    }

    fn assert_valid_index(index: u16) {
        assert!(
            index <= MAX_EXTENDED_REGISTER_INDEX,
            "extended register index must be in 0..={MAX_EXTENDED_REGISTER_INDEX}"
        );
    }
}

impl<T: Clone + Default> Default for ExtendedRegisterStorage<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Dumpable> Dumpable for ExtendedRegisterStorage<T> {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.default.dump(target)?;
        self.dense.dump(target)?;

        self.sparse.len().dump(target)?;
        let mut indices: Vec<u16> = self.sparse.keys().copied().collect();
        indices.sort_unstable();
        for index in indices {
            index.dump(target)?;
            self.sparse[&index].dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let default = T::undump(lines)?;
        let dense = Vec::<T>::undump(lines)?;
        if dense.len() != DENSE_REGISTER_COUNT {
            return Err(FormatError::ParseError);
        }

        let sparse_len = usize::undump(lines)?;
        if sparse_len > MAX_EXTENDED_REGISTER_INDEX as usize + 1 - DENSE_REGISTER_COUNT {
            return Err(FormatError::ParseError);
        }
        let mut sparse = HashMap::with_capacity(sparse_len);
        for _ in 0..sparse_len {
            let index = u16::undump(lines)?;
            if (index as usize) < DENSE_REGISTER_COUNT || index > MAX_EXTENDED_REGISTER_INDEX {
                return Err(FormatError::ParseError);
            }
            let value = T::undump(lines)?;
            if sparse.insert(index, value).is_some() {
                return Err(FormatError::ParseError);
            }
        }

        Ok(Self {
            default,
            dense,
            sparse,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 低位レジスタは連続領域で読み書きする() {
        let mut registers = ExtendedRegisterStorage::new(0_i32);

        assert_eq!(*registers.get(0), 0);
        assert_eq!(*registers.get(255), 0);
        assert_eq!(registers.set(0, 12), 0);
        *registers.get_mut(255) = 34;

        assert_eq!(*registers.get(0), 12);
        assert_eq!(*registers.get(255), 34);
        assert!(registers.sparse.is_empty());
    }

    #[test]
    fn 高位レジスタは既定値から必要な番号だけを作る() {
        let mut registers = ExtendedRegisterStorage::new(String::from("空"));

        assert_eq!(registers.get(256), "空");
        assert_eq!(registers.get(MAX_EXTENDED_REGISTER_INDEX), "空");
        assert!(registers.sparse.is_empty());

        registers.get_mut(256).push_str("でない");
        assert_eq!(
            registers.set(MAX_EXTENDED_REGISTER_INDEX, String::from("末尾")),
            "空"
        );

        assert_eq!(registers.get(256), "空でない");
        assert_eq!(registers.get(MAX_EXTENDED_REGISTER_INDEX), "末尾");
        assert_eq!(registers.sparse.len(), 2);
    }

    #[test]
    fn 書式の往復で密領域と疎領域と既定値を保つ() {
        let mut before = ExtendedRegisterStorage::new(-7_i32);
        before.set(0, 10);
        before.set(255, 20);
        before.set(1_000, 30);
        before.set(256, 40);
        before.set(MAX_EXTENDED_REGISTER_INDEX, 50);

        let mut dumped = Vec::new();
        before.dump(&mut dumped).unwrap();
        let text = String::from_utf8(dumped).unwrap();
        let mut lines = text.lines();
        let after = ExtendedRegisterStorage::<i32>::undump(&mut lines).unwrap();

        assert_eq!(after, before);
        assert_eq!(*after.get(500), -7);
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn 書式から低位や範囲外の疎キーを受け入れない() {
        let dense = vec!["0"; DENSE_REGISTER_COUNT].join("\n");
        for invalid_index in [255_u16, MAX_EXTENDED_REGISTER_INDEX + 1] {
            let text = format!("0\n{DENSE_REGISTER_COUNT}\n{dense}\n1\n{invalid_index}\n1\n");
            let mut lines = text.lines();
            assert!(matches!(
                ExtendedRegisterStorage::<i32>::undump(&mut lines),
                Err(FormatError::ParseError)
            ));
        }
    }
}

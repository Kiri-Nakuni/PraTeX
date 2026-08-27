//! 制御綴の索引に使う速い hash。
//!
//! 標準の `HashMap` は SipHash-1-3 を使う。これは意図的な衝突を作られても
//! 性能が落ちないための選択だが、制御綴の索引では一文字から十数文字の短い
//! byte 列を、文書に現れる制御綴ごとに引く。299 頁の `lipsum` を LaTeX で
//! 組んだ profile では、SipHash の計算だけで全体の 4.3% を占めていた。
//!
//! ここでは rustc が自身の symbol 表に使うのと同じ、乗算と回転による hash へ
//! 置き換える。短い鍵に対して数命令で終わる。
//!
//! 引き換えに、衝突を狙って作られた入力では探索が線形に近づく。壊れるのでは
//! なく遅くなるだけであり、fmt も文書も利用者が自分で与えるものなので、
//! ここでは速さを採る。ただし利用者以外が与える入力を鍵にする表へは使わない。

use std::hash::{BuildHasherDefault, Hasher};

/// 黄金比を 64 bit へ写した定数。rustc の `FxHasher` と同じ値である。
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// 乗算と回転による hasher。
#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut rest = bytes;
        while let Some((chunk, tail)) = rest.split_first_chunk::<8>() {
            self.add(u64::from_ne_bytes(*chunk));
            rest = tail;
        }
        if let Some((chunk, tail)) = rest.split_first_chunk::<4>() {
            self.add(u32::from_ne_bytes(*chunk) as u64);
            rest = tail;
        }
        if let Some((chunk, tail)) = rest.split_first_chunk::<2>() {
            self.add(u16::from_ne_bytes(*chunk) as u64);
            rest = tail;
        }
        if let Some(&byte) = rest.first() {
            self.add(byte as u64);
        }
    }

    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.add(value as u64);
    }

    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.add(value as u64);
    }

    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.add(value as u64);
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// `FxHasher` を使う `HashMap`。
pub type FxHashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<FxHasher>>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    fn hash_of<T: Hash>(value: &T) -> u64 {
        let mut hasher = FxHasher::default();
        value.hash(&mut hasher);
        hasher.finish()
    }

    /// 同じ鍵は同じ値を返し、違う鍵は（実用上）違う値を返す。
    #[test]
    fn 同じ鍵は同じ値になる() {
        assert_eq!(hash_of(&b"relax".to_vec()), hash_of(&b"relax".to_vec()));
        assert_ne!(hash_of(&b"relax".to_vec()), hash_of(&b"hbox".to_vec()));
    }

    /// 長さが 8 の倍数でない鍵、8 を超える鍵、空の鍵で末端処理が壊れていない。
    #[test]
    fn 半端な長さの鍵を扱える() {
        let mut seen = std::collections::HashSet::new();
        for len in 0..40 {
            let key: Vec<u8> = (0..len).map(|i| b'a' + (i % 26) as u8).collect();
            seen.insert(hash_of(&key));
        }
        assert_eq!(seen.len(), 40, "長さ違いの鍵が衝突している");
    }

    /// 表として使ったときに引けること。
    #[test]
    fn 表として引ける() {
        let mut map: FxHashMap<Vec<u8>, u32> = FxHashMap::default();
        for (i, name) in [b"par".as_slice(), b"hbox", b"vbox", b"relax"]
            .into_iter()
            .enumerate()
        {
            map.insert(name.to_vec(), i as u32);
        }
        assert_eq!(map.get(b"vbox".as_slice()), Some(&2));
        assert_eq!(map.get(b"nonesuch".as_slice()), None);
        assert_eq!(map.len(), 4);
    }
}

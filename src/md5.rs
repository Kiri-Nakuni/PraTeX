//! MD5。**`\pdfmdfivesum` のためだけにある。**
//!
//! expl3 が engine の見分けと文字列の畳み込みに使う。
//! 暗号の用途には使わないこと——**MD5 はもう安全ではない。**

use std::io::{self, Read};

const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

// RFC 1321 defines these as floor(2^32 * abs(sin(i + 1))).  Keeping the
// resulting constants here makes every incremental block independent of
// platform floating-point behavior.
const K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// RFC 1321のincremental state。
///
/// file用primitiveが入力全体を一括確保せず、同じMD5決定箇所を文字列形式と共有する。
pub struct Md5 {
    state: [u32; 4],
    total_len: u64,
    buffer: [u8; 64],
    buffer_len: usize,
}

impl Md5 {
    pub fn new() -> Self {
        Self {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            total_len: 0,
            buffer: [0; 64],
            buffer_len: 0,
        }
    }

    pub fn update(&mut self, mut input: &[u8]) {
        self.total_len = self.total_len.wrapping_add(input.len() as u64);

        if self.buffer_len != 0 {
            let copied = (64 - self.buffer_len).min(input.len());
            self.buffer[self.buffer_len..self.buffer_len + copied]
                .copy_from_slice(&input[..copied]);
            self.buffer_len += copied;
            input = &input[copied..];
            if self.buffer_len == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffer_len = 0;
            }
            if input.is_empty() {
                return;
            }
        }

        while input.len() >= 64 {
            let block: &[u8; 64] = input[..64]
                .try_into()
                .expect("64-byte slice has an array representation");
            self.compress(block);
            input = &input[64..];
        }

        self.buffer[..input.len()].copy_from_slice(input);
        self.buffer_len = input.len();
    }

    pub fn finalize(mut self) -> [u8; 16] {
        let bit_len = self.total_len.wrapping_mul(8);
        self.update(&[0x80]);
        let zeroes = [0; 64];
        let padding_len = if self.buffer_len <= 56 {
            56 - self.buffer_len
        } else {
            64 + 56 - self.buffer_len
        };
        self.update(&zeroes[..padding_len]);
        debug_assert_eq!(self.buffer_len, 56);
        self.update(&bit_len.to_le_bytes());
        debug_assert_eq!(self.buffer_len, 0);

        let mut output = [0; 16];
        for (chunk, word) in output.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        output
    }

    fn compress(&mut self, chunk: &[u8; 64]) {
        let mut words = [0u32; 16];
        for (slot, bytes) in words.iter_mut().zip(chunk.chunks_exact(4)) {
            *slot = u32::from_le_bytes(bytes.try_into().expect("four-byte MD5 word"));
        }

        let [mut a, mut b, mut c, mut d] = self.state;
        for index in 0..64 {
            let (function, word_index) = match index / 16 {
                0 => ((b & c) | (!b & d), index),
                1 => ((d & b) | (!d & c), (5 * index + 1) % 16),
                2 => (b ^ c ^ d, (3 * index + 5) % 16),
                _ => (c ^ (b | !d), (7 * index) % 16),
            };
            let next = function
                .wrapping_add(a)
                .wrapping_add(K[index])
                .wrapping_add(words[word_index]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(next.rotate_left(S[index]));
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
    }
}

impl Default for Md5 {
    fn default() -> Self {
        Self::new()
    }
}

/// RFC 1321 の MD5。
pub fn md5(input: &[u8]) -> [u8; 16] {
    let mut state = Md5::new();
    state.update(input);
    state.finalize()
}

/// Readerを固定長bufferで最後まで読み、RFC 1321のMD5を返す。
pub fn md5_reader(reader: &mut impl Read) -> io::Result<[u8; 16]> {
    let mut state = Md5::new();
    let mut buffer = [0; 16 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(state.finalize()),
            Ok(read) => state.update(&buffer[..read]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{md5, md5_reader, Md5};
    use std::io::Cursor;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn rfc1321の見本と一致する() {
        assert_eq!(hex(&md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(&md5(b"a")), "0cc175b9c0f1b6a831c399e269772661");
        assert_eq!(hex(&md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            hex(&md5(b"message digest")),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
        assert_eq!(
            hex(&md5(b"abcdefghijklmnopqrstuvwxyz")),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
        assert_eq!(
            hex(&md5(
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
            )),
            "d174ab98d277d9f5a5611c2c9f419d9f"
        );
        assert_eq!(
            hex(&md5(
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            )),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }

    #[test]
    fn 任意のchunk境界で一括入力と一致する() {
        let input = (0..=255).cycle().take(4097).collect::<Vec<_>>();
        let expected = md5(&input);
        for chunk_size in [1, 7, 55, 56, 63, 64, 65, 1024] {
            let mut state = Md5::new();
            for chunk in input.chunks(chunk_size) {
                state.update(chunk);
            }
            assert_eq!(state.finalize(), expected, "chunk size {chunk_size}");
        }
    }

    #[test]
    fn reader経路は一括入力と一致する() {
        let input = (0..=255).rev().cycle().take(100_003).collect::<Vec<_>>();
        assert_eq!(md5_reader(&mut Cursor::new(&input)).unwrap(), md5(&input));
    }
}

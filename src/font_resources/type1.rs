//! PFB に包まれた Type 1 font program を PDF の `FontFile` 用にほどく。
//!
//! PFB segment header は Adobe Technical Note #5040, 3.3 の公開仕様だけを
//! 境界にする。先頭 byte は 128、type 1/2 の長さは 4-byte little endian、
//! type 3 は長さを持たない EOF marker である。
//! <https://www.adobe.com/content/dam/acom/en/devnet/font/pdfs/5040.Download_Fonts.pdf>
//!
//! PDF 1.4, 5.8 が要求する `Length1`、`Length2`、`Length3` は、それぞれ
//! 最初の ASCII 部、binary 部、末尾の ASCII 部を連結した byte 数である。
//! <https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf>

use std::fmt;

const PFB_MARKER: u8 = 0x80;
const ASCII_SEGMENT: u8 = 1;
const BINARY_SEGMENT: u8 = 2;
const END_SEGMENT: u8 = 3;
const DATA_HEADER_LENGTH: usize = 6;
const END_HEADER_LENGTH: usize = 2;
// A Type 1 program is normally measured in tens or hundreds of KiB.  Keep the
// parser from making an unbounded second copy of an already-loaded resource.
const MAX_PFB_PROGRAM_BYTES: usize = 128 * 1024 * 1024;

/// PFB wrapper を除いた、PDF に埋め込める Type 1 font program。
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Type1FontProgram {
    pub(crate) bytes: Vec<u8>,
    pub(crate) length1: usize,
    pub(crate) length2: usize,
    pub(crate) length3: usize,
}

/// PFB segment の論理的な種類。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PfbSegmentKind {
    Ascii,
    Binary,
    End,
}

/// 現在位置で受理できる segment。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExpectedPfbSegment {
    InitialAscii,
    Binary,
    TrailingAscii,
    TrailingAsciiOrEnd,
}

/// PFB wrapper の構造が公開仕様に合わない理由。
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PfbError {
    MissingEndMarker {
        offset: usize,
    },
    TruncatedHeader {
        offset: usize,
        available: usize,
    },
    InvalidMarker {
        offset: usize,
        found: u8,
    },
    InvalidSegmentType {
        offset: usize,
        found: u8,
    },
    SegmentLengthDoesNotFit {
        offset: usize,
        declared_length: u32,
    },
    SegmentRangeOverflow {
        offset: usize,
        declared_length: u32,
    },
    TruncatedSegment {
        offset: usize,
        declared_length: u32,
        available: usize,
    },
    UnexpectedSegment {
        offset: usize,
        found: PfbSegmentKind,
        expected: ExpectedPfbSegment,
    },
    ProgramLengthOverflow {
        offset: usize,
    },
    ProgramTooLarge {
        offset: usize,
        length: usize,
        limit: usize,
    },
    TrailingData {
        offset: usize,
        length: usize,
    },
}

impl fmt::Display for PfbError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEndMarker { offset } => {
                write!(formatter, "PFB end marker is missing at byte {offset}")
            }
            Self::TruncatedHeader { offset, available } => write!(
                formatter,
                "PFB segment header at byte {offset} has only {available} bytes"
            ),
            Self::InvalidMarker { offset, found } => write!(
                formatter,
                "PFB segment at byte {offset} starts with {found:#04x}, not 0x80"
            ),
            Self::InvalidSegmentType { offset, found } => {
                write!(formatter, "unknown PFB segment type {found} at byte {offset}")
            }
            Self::SegmentLengthDoesNotFit {
                offset,
                declared_length,
            } => write!(
                formatter,
                "PFB segment length {declared_length} at byte {offset} does not fit this platform"
            ),
            Self::SegmentRangeOverflow {
                offset,
                declared_length,
            } => write!(
                formatter,
                "PFB segment length {declared_length} at byte {offset} overflows its byte range"
            ),
            Self::TruncatedSegment {
                offset,
                declared_length,
                available,
            } => write!(
                formatter,
                "PFB segment at byte {offset} declares {declared_length} data bytes but only {available} remain"
            ),
            Self::UnexpectedSegment {
                offset,
                found,
                expected,
            } => write!(
                formatter,
                "unexpected PFB {found:?} segment at byte {offset}; expected {expected:?}"
            ),
            Self::ProgramLengthOverflow { offset } => write!(
                formatter,
                "PFB program length overflows while reading segment at byte {offset}"
            ),
            Self::ProgramTooLarge {
                offset,
                length,
                limit,
            } => write!(
                formatter,
                "PFB program reaches {length} bytes at byte {offset}, above the {limit}-byte limit"
            ),
            Self::TrailingData { offset, length } => write!(
                formatter,
                "PFB end marker is followed by {length} bytes beginning at byte {offset}"
            ),
        }
    }
}

impl std::error::Error for PfbError {}

#[derive(Clone, Copy)]
enum ParsePhase {
    Start,
    InitialAscii,
    Binary,
    TrailingAscii,
}

/// PFB segment を検査し、wrapper を除いた font program と三つの長さを返す。
///
/// 同じ type の segment が連続していても一つの論理部として連結する。PDF の
/// Type 1 `FontFile` は三部構成なので、type 1 → type 2 → type 1 → type 3 の
/// 順序は厳密に検査する。
pub(crate) fn parse_pfb(input: &[u8]) -> Result<Type1FontProgram, PfbError> {
    parse_pfb_with_limit(input, MAX_PFB_PROGRAM_BYTES)
}

fn parse_pfb_with_limit(
    input: &[u8],
    max_program_bytes: usize,
) -> Result<Type1FontProgram, PfbError> {
    let mut cursor = 0;
    let mut phase = ParsePhase::Start;
    let mut program = Type1FontProgram {
        bytes: Vec::new(),
        length1: 0,
        length2: 0,
        length3: 0,
    };

    loop {
        if cursor == input.len() {
            return Err(PfbError::MissingEndMarker { offset: cursor });
        }
        let remaining = input.len() - cursor;
        if remaining < END_HEADER_LENGTH {
            return Err(PfbError::TruncatedHeader {
                offset: cursor,
                available: remaining,
            });
        }
        if input[cursor] != PFB_MARKER {
            return Err(PfbError::InvalidMarker {
                offset: cursor,
                found: input[cursor],
            });
        }

        let segment_type = input[cursor + 1];
        if segment_type == END_SEGMENT {
            check_end_order(phase, cursor)?;
            let trailing_offset = cursor + END_HEADER_LENGTH;
            if trailing_offset != input.len() {
                return Err(PfbError::TrailingData {
                    offset: trailing_offset,
                    length: input.len() - trailing_offset,
                });
            }
            return Ok(program);
        }
        if segment_type != ASCII_SEGMENT && segment_type != BINARY_SEGMENT {
            return Err(PfbError::InvalidSegmentType {
                offset: cursor,
                found: segment_type,
            });
        }
        if remaining < DATA_HEADER_LENGTH {
            return Err(PfbError::TruncatedHeader {
                offset: cursor,
                available: remaining,
            });
        }

        let declared_length = u32::from_le_bytes([
            input[cursor + 2],
            input[cursor + 3],
            input[cursor + 4],
            input[cursor + 5],
        ]);
        let length =
            usize::try_from(declared_length).map_err(|_| PfbError::SegmentLengthDoesNotFit {
                offset: cursor,
                declared_length,
            })?;
        let data_start =
            cursor
                .checked_add(DATA_HEADER_LENGTH)
                .ok_or(PfbError::SegmentRangeOverflow {
                    offset: cursor,
                    declared_length,
                })?;
        let data_end = data_start
            .checked_add(length)
            .ok_or(PfbError::SegmentRangeOverflow {
                offset: cursor,
                declared_length,
            })?;
        if data_end > input.len() {
            return Err(PfbError::TruncatedSegment {
                offset: cursor,
                declared_length,
                available: input.len() - data_start,
            });
        }

        let kind = if segment_type == ASCII_SEGMENT {
            PfbSegmentKind::Ascii
        } else {
            PfbSegmentKind::Binary
        };
        phase = advance_phase(phase, kind, cursor)?;
        let part_length = match phase {
            ParsePhase::InitialAscii => &mut program.length1,
            ParsePhase::Binary => &mut program.length2,
            ParsePhase::TrailingAscii => &mut program.length3,
            // `advance_phase` は Start から data segment を受け取れば必ず遷移する。
            // それでも将来の変更で不変条件を壊したとき、font input で panic しない。
            ParsePhase::Start => {
                return Err(PfbError::UnexpectedSegment {
                    offset: cursor,
                    found: kind,
                    expected: ExpectedPfbSegment::InitialAscii,
                });
            }
        };
        *part_length = part_length
            .checked_add(length)
            .ok_or(PfbError::ProgramLengthOverflow { offset: cursor })?;
        let next_program_length = program
            .bytes
            .len()
            .checked_add(length)
            .ok_or(PfbError::ProgramLengthOverflow { offset: cursor })?;
        if next_program_length > max_program_bytes {
            return Err(PfbError::ProgramTooLarge {
                offset: cursor,
                length: next_program_length,
                limit: max_program_bytes,
            });
        }
        program
            .bytes
            .extend_from_slice(&input[data_start..data_end]);
        cursor = data_end;
    }
}

fn advance_phase(
    phase: ParsePhase,
    found: PfbSegmentKind,
    offset: usize,
) -> Result<ParsePhase, PfbError> {
    match (phase, found) {
        (ParsePhase::Start, PfbSegmentKind::Ascii) => Ok(ParsePhase::InitialAscii),
        (ParsePhase::InitialAscii, PfbSegmentKind::Ascii) => Ok(ParsePhase::InitialAscii),
        (ParsePhase::InitialAscii, PfbSegmentKind::Binary) => Ok(ParsePhase::Binary),
        (ParsePhase::Binary, PfbSegmentKind::Binary) => Ok(ParsePhase::Binary),
        (ParsePhase::Binary, PfbSegmentKind::Ascii) => Ok(ParsePhase::TrailingAscii),
        (ParsePhase::TrailingAscii, PfbSegmentKind::Ascii) => Ok(ParsePhase::TrailingAscii),
        (ParsePhase::Start, PfbSegmentKind::Binary) => Err(PfbError::UnexpectedSegment {
            offset,
            found,
            expected: ExpectedPfbSegment::InitialAscii,
        }),
        (ParsePhase::TrailingAscii, PfbSegmentKind::Binary) => Err(PfbError::UnexpectedSegment {
            offset,
            found,
            expected: ExpectedPfbSegment::TrailingAsciiOrEnd,
        }),
        (ParsePhase::Start, PfbSegmentKind::End) => Err(PfbError::UnexpectedSegment {
            offset,
            found,
            expected: ExpectedPfbSegment::InitialAscii,
        }),
        (ParsePhase::InitialAscii, PfbSegmentKind::End) => Err(PfbError::UnexpectedSegment {
            offset,
            found,
            expected: ExpectedPfbSegment::Binary,
        }),
        (ParsePhase::Binary, PfbSegmentKind::End) => Err(PfbError::UnexpectedSegment {
            offset,
            found,
            expected: ExpectedPfbSegment::TrailingAscii,
        }),
        (ParsePhase::TrailingAscii, PfbSegmentKind::End) => Ok(ParsePhase::TrailingAscii),
    }
}

fn check_end_order(phase: ParsePhase, offset: usize) -> Result<(), PfbError> {
    let expected = match phase {
        ParsePhase::Start => ExpectedPfbSegment::InitialAscii,
        ParsePhase::InitialAscii => ExpectedPfbSegment::Binary,
        ParsePhase::Binary => ExpectedPfbSegment::TrailingAscii,
        ParsePhase::TrailingAscii => return Ok(()),
    };
    Err(PfbError::UnexpectedSegment {
        offset,
        found: PfbSegmentKind::End,
        expected,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        parse_pfb, parse_pfb_with_limit, ExpectedPfbSegment, PfbError, PfbSegmentKind,
        Type1FontProgram, ASCII_SEGMENT, BINARY_SEGMENT, END_SEGMENT, PFB_MARKER,
    };

    fn データsegment(kind: u8, data: &[u8]) -> Vec<u8> {
        let mut segment = vec![PFB_MARKER, kind];
        segment.extend_from_slice(
            &u32::try_from(data.len())
                .expect("synthetic segment fits in u32")
                .to_le_bytes(),
        );
        segment.extend_from_slice(data);
        segment
    }

    fn 終端segment() -> [u8; 2] {
        [PFB_MARKER, END_SEGMENT]
    }

    fn 連結(parts: &[&[u8]]) -> Vec<u8> {
        parts.iter().flat_map(|part| part.iter().copied()).collect()
    }

    #[test]
    fn 三つの論理部からwrapperだけを外す() {
        let first = データsegment(ASCII_SEGMENT, b"clear");
        let encrypted = データsegment(BINARY_SEGMENT, &[0, 0x80, 0xff]);
        let last = データsegment(ASCII_SEGMENT, b"tail");
        let end = 終端segment();
        let input = 連結(&[&first, &encrypted, &last, &end]);

        assert_eq!(
            parse_pfb(&input),
            Ok(Type1FontProgram {
                bytes: b"clear\x00\x80\xfftail".to_vec(),
                length1: 5,
                length2: 3,
                length3: 4,
            })
        );
    }

    #[test]
    fn 同じ種類の連続segmentを論理部へまとめる() {
        let ascii1 = データsegment(ASCII_SEGMENT, b"ab");
        let ascii2 = データsegment(ASCII_SEGMENT, b"c");
        let binary1 = データsegment(BINARY_SEGMENT, &[1, 2]);
        let binary2 = データsegment(BINARY_SEGMENT, &[3]);
        let trailing1 = データsegment(ASCII_SEGMENT, b"de");
        let trailing2 = データsegment(ASCII_SEGMENT, b"f");
        let end = 終端segment();
        let input = 連結(&[
            &ascii1, &ascii2, &binary1, &binary2, &trailing1, &trailing2, &end,
        ]);

        let program = parse_pfb(&input).unwrap();
        assert_eq!(program.bytes, b"abc\x01\x02\x03def");
        assert_eq!(program.length1, 3);
        assert_eq!(program.length2, 3);
        assert_eq!(program.length3, 3);
    }

    #[test]
    fn little_endianの四byte長を読む() {
        let ascii = データsegment(ASCII_SEGMENT, &vec![b'a'; 258]);
        let binary = データsegment(BINARY_SEGMENT, &[7]);
        let trailing = データsegment(ASCII_SEGMENT, &[8]);
        let end = 終端segment();
        let input = 連結(&[&ascii, &binary, &trailing, &end]);

        let program = parse_pfb(&input).unwrap();
        assert_eq!(program.length1, 258);
        assert_eq!(program.bytes.len(), 260);
    }

    #[test]
    fn 空のsegmentも順序を保って連結する() {
        let ascii = データsegment(ASCII_SEGMENT, b"");
        let binary = データsegment(BINARY_SEGMENT, b"");
        let trailing = データsegment(ASCII_SEGMENT, b"");
        let end = 終端segment();
        let input = 連結(&[&ascii, &binary, &trailing, &end]);

        assert_eq!(
            parse_pfb(&input),
            Ok(Type1FontProgram {
                bytes: Vec::new(),
                length1: 0,
                length2: 0,
                length3: 0,
            })
        );
    }

    #[test]
    fn markerと長さheaderの途中切れを分けて拒む() {
        assert_eq!(
            parse_pfb(&[PFB_MARKER]),
            Err(PfbError::TruncatedHeader {
                offset: 0,
                available: 1,
            })
        );
        assert_eq!(
            parse_pfb(&[PFB_MARKER, ASCII_SEGMENT, 1, 0]),
            Err(PfbError::TruncatedHeader {
                offset: 0,
                available: 4,
            })
        );
    }

    #[test]
    fn 宣言長より短いdataと巨大な宣言長を拒む() {
        assert_eq!(
            parse_pfb(&[PFB_MARKER, ASCII_SEGMENT, 3, 0, 0, 0, b'a']),
            Err(PfbError::TruncatedSegment {
                offset: 0,
                declared_length: 3,
                available: 1,
            })
        );
        assert_eq!(
            parse_pfb(&[PFB_MARKER, ASCII_SEGMENT, 0xff, 0xff, 0xff, 0xff]),
            Err(PfbError::TruncatedSegment {
                offset: 0,
                declared_length: u32::MAX,
                available: 0,
            })
        );
    }

    #[test]
    fn 展開後programの上限を越えて複製しない() {
        let ascii = データsegment(ASCII_SEGMENT, b"abc");
        let binary = データsegment(BINARY_SEGMENT, b"de");
        let trailing = データsegment(ASCII_SEGMENT, b"f");
        let end = 終端segment();
        let input = 連結(&[&ascii, &binary, &trailing, &end]);

        assert_eq!(
            parse_pfb_with_limit(&input, 5),
            Err(PfbError::ProgramTooLarge {
                offset: ascii.len() + binary.len(),
                length: 6,
                limit: 5,
            })
        );
    }

    #[test]
    fn markerと未知のtypeを検査する() {
        assert_eq!(
            parse_pfb(&[0, END_SEGMENT]),
            Err(PfbError::InvalidMarker {
                offset: 0,
                found: 0,
            })
        );
        assert_eq!(
            parse_pfb(&[PFB_MARKER, 4]),
            Err(PfbError::InvalidSegmentType {
                offset: 0,
                found: 4,
            })
        );
    }

    #[test]
    fn binaryが先行する順序を拒む() {
        let binary = データsegment(BINARY_SEGMENT, b"x");
        assert_eq!(
            parse_pfb(&binary),
            Err(PfbError::UnexpectedSegment {
                offset: 0,
                found: PfbSegmentKind::Binary,
                expected: ExpectedPfbSegment::InitialAscii,
            })
        );
    }

    #[test]
    fn 各論理部を欠いた終端を拒む() {
        let end = 終端segment();
        assert_eq!(
            parse_pfb(&end),
            Err(PfbError::UnexpectedSegment {
                offset: 0,
                found: PfbSegmentKind::End,
                expected: ExpectedPfbSegment::InitialAscii,
            })
        );

        let ascii = データsegment(ASCII_SEGMENT, b"a");
        let input = 連結(&[&ascii, &end]);
        assert_eq!(
            parse_pfb(&input),
            Err(PfbError::UnexpectedSegment {
                offset: ascii.len(),
                found: PfbSegmentKind::End,
                expected: ExpectedPfbSegment::Binary,
            })
        );

        let binary = データsegment(BINARY_SEGMENT, b"b");
        let input = 連結(&[&ascii, &binary, &end]);
        assert_eq!(
            parse_pfb(&input),
            Err(PfbError::UnexpectedSegment {
                offset: ascii.len() + binary.len(),
                found: PfbSegmentKind::End,
                expected: ExpectedPfbSegment::TrailingAscii,
            })
        );
    }

    #[test]
    fn 末尾asciiからbinaryへ戻る順序を拒む() {
        let ascii = データsegment(ASCII_SEGMENT, b"a");
        let binary = データsegment(BINARY_SEGMENT, b"b");
        let trailing = データsegment(ASCII_SEGMENT, b"c");
        let wrong = データsegment(BINARY_SEGMENT, b"d");
        let input = 連結(&[&ascii, &binary, &trailing, &wrong]);
        assert_eq!(
            parse_pfb(&input),
            Err(PfbError::UnexpectedSegment {
                offset: ascii.len() + binary.len() + trailing.len(),
                found: PfbSegmentKind::Binary,
                expected: ExpectedPfbSegment::TrailingAsciiOrEnd,
            })
        );
    }

    #[test]
    fn eofの欠落と後続byteを拒む() {
        let ascii = データsegment(ASCII_SEGMENT, b"a");
        let binary = データsegment(BINARY_SEGMENT, b"b");
        let trailing = データsegment(ASCII_SEGMENT, b"c");
        let without_end = 連結(&[&ascii, &binary, &trailing]);
        assert_eq!(
            parse_pfb(&without_end),
            Err(PfbError::MissingEndMarker {
                offset: without_end.len(),
            })
        );

        let end = 終端segment();
        let with_junk = 連結(&[&ascii, &binary, &trailing, &end, b"junk"]);
        assert_eq!(
            parse_pfb(&with_junk),
            Err(PfbError::TrailingData {
                offset: ascii.len() + binary.len() + trailing.len() + end.len(),
                length: 4,
            })
        );
    }
}

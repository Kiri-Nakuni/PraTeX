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

use crate::font_resources::afm::AfmNumber;
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

// Adobe Type 1 Font Format, chapter 7.2.2.  Metadata is read once per font and
// decrypted as a stream; the complete eexec section is never copied.
const EEXEC_INITIAL_KEY: u16 = 55_665;
const EEXEC_C1: u16 = 52_845;
const EEXEC_C2: u16 = 22_719;
const EEXEC_RANDOM_PREFIX_BYTES: usize = 4;
// Private dictionary metadata normally precedes Subrs/CharStrings by only a
// few KiB.  A finite ceiling prevents a malicious font from turning a missing
// boundary into a scan of the complete 128 MiB resource.
const MAX_EEXEC_PRIVATE_SCAN_BYTES: usize = 1024 * 1024;
const MAX_METADATA_TOKEN_BYTES: usize = 128;

/// PFB wrapper を除いた、PDF に埋め込める Type 1 font program。
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Type1FontProgram {
    pub(crate) bytes: Vec<u8>,
    pub(crate) length1: usize,
    pub(crate) length2: usize,
    pub(crate) length3: usize,
}

/// eexec Private dictionary の `StdVW` を読めなかった理由。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Type1MetadataError {
    InvalidEncryptedRange {
        length1: usize,
        length2: usize,
        program_length: usize,
    },
    EncryptedDataTooShort {
        length: usize,
    },
    ScanLimitExceeded {
        limit: usize,
    },
    UnterminatedLiteralString {
        offset: usize,
    },
    UnterminatedHexString {
        offset: usize,
    },
    MissingPrivateDictionary,
    MissingMetadataBoundary,
    DuplicatePrivateDictionary {
        first_offset: usize,
        duplicate_offset: usize,
    },
    DuplicateStdVw {
        first_offset: usize,
        duplicate_offset: usize,
    },
    InvalidStdVw {
        offset: usize,
        kind: StdVwSyntaxError,
    },
}

/// `/StdVW` に続く一要素配列の壊れ方。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StdVwSyntaxError {
    ExpectedArray,
    ExpectedNumber,
    ExpectedArrayEnd,
    InvalidNumber,
    NumberOverflow,
    NumberTooPrecise,
}

impl fmt::Display for Type1MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEncryptedRange {
                length1,
                length2,
                program_length,
            } => write!(
                formatter,
                "Type 1 eexec range {length1}+{length2} is outside the {program_length}-byte program"
            ),
            Self::EncryptedDataTooShort { length } => write!(
                formatter,
                "Type 1 eexec data has {length} bytes and cannot contain its four random bytes"
            ),
            Self::ScanLimitExceeded { limit } => write!(
                formatter,
                "Type 1 Private metadata has no Subrs or CharStrings boundary within {limit} decrypted bytes"
            ),
            Self::UnterminatedLiteralString { offset } => write!(
                formatter,
                "Type 1 Private literal string beginning at decrypted byte {offset} is unterminated"
            ),
            Self::UnterminatedHexString { offset } => write!(
                formatter,
                "Type 1 Private hexadecimal string beginning at decrypted byte {offset} is unterminated"
            ),
            Self::MissingPrivateDictionary => {
                formatter.write_str("Type 1 eexec metadata has no Private dictionary")
            }
            Self::MissingMetadataBoundary => formatter.write_str(
                "Type 1 eexec metadata ends before its Subrs or CharStrings boundary",
            ),
            Self::DuplicatePrivateDictionary {
                first_offset,
                duplicate_offset,
            } => write!(
                formatter,
                "Type 1 Private dictionary at decrypted byte {duplicate_offset} duplicates the declaration at byte {first_offset}"
            ),
            Self::DuplicateStdVw {
                first_offset,
                duplicate_offset,
            } => write!(
                formatter,
                "Type 1 Private StdVW at decrypted byte {duplicate_offset} duplicates the value at byte {first_offset}"
            ),
            Self::InvalidStdVw { offset, kind } => write!(
                formatter,
                "invalid Type 1 Private StdVW at decrypted byte {offset}: {kind}"
            ),
        }
    }
}

impl fmt::Display for StdVwSyntaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExpectedArray => "expected a one-element array",
            Self::ExpectedNumber => "expected the array's decimal number",
            Self::ExpectedArrayEnd => "expected the end of the one-element array",
            Self::InvalidNumber => "the array element is not a signed decimal",
            Self::NumberOverflow => "the decimal does not fit the AFM fixed-point representation",
            Self::NumberTooPrecise => "the decimal is more precise than 10^-6",
        })
    }
}

impl std::error::Error for Type1MetadataError {}

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

/// PFB の eexec 部を復号し、Private dictionary の `StdVW` だけを取り出す。
///
/// Adobe *Type 1 Font Format* の公開仕様に従い、seed 55665 で byte ごとに復号して
/// 最初の四 byte を捨てる。PostScript は実行せず、コメントと文字列を読み飛ばし、
/// `/Subrs` または `/CharStrings` に着いた時点で止める。したがって charstring の
/// binary data を metadata として解釈する経路はない。
/// <https://adobe-type-tools.github.io/font-tech-notes/pdfs/T1_SPEC.pdf>
pub(crate) fn extract_private_std_vw(
    program: &Type1FontProgram,
) -> Result<Option<AfmNumber>, Type1MetadataError> {
    let encrypted_end = program.length1.checked_add(program.length2).ok_or(
        Type1MetadataError::InvalidEncryptedRange {
            length1: program.length1,
            length2: program.length2,
            program_length: program.bytes.len(),
        },
    )?;
    if encrypted_end > program.bytes.len() {
        return Err(Type1MetadataError::InvalidEncryptedRange {
            length1: program.length1,
            length2: program.length2,
            program_length: program.bytes.len(),
        });
    }
    let encrypted = &program.bytes[program.length1..encrypted_end];
    if encrypted.len() < EEXEC_RANDOM_PREFIX_BYTES {
        return Err(Type1MetadataError::EncryptedDataTooShort {
            length: encrypted.len(),
        });
    }

    let mut scanner = MetadataScanner::new(encrypted);
    let mut found = None;
    let mut procedure_depth = 0usize;
    let mut array_depth = 0usize;
    let mut dictionary_depth = 0usize;
    let mut private_offset = None;
    while let Some(token) = scanner.next_token()? {
        match token.kind {
            MetadataTokenKind::OpenProcedure => {
                procedure_depth += 1;
            }
            MetadataTokenKind::CloseProcedure => {
                procedure_depth = procedure_depth.saturating_sub(1);
            }
            MetadataTokenKind::OpenArray if procedure_depth == 0 => {
                array_depth += 1;
            }
            MetadataTokenKind::CloseArray if procedure_depth == 0 => {
                array_depth = array_depth.saturating_sub(1);
            }
            MetadataTokenKind::OpenDictionary => {
                dictionary_depth += 1;
            }
            MetadataTokenKind::CloseDictionary => {
                dictionary_depth = dictionary_depth.saturating_sub(1);
            }
            MetadataTokenKind::LiteralName(name)
                if procedure_depth == 0 && array_depth == 0 && dictionary_depth == 0 =>
            {
                match name {
                    MetadataName::Private => {
                        if let Some(first_offset) = private_offset {
                            return Err(Type1MetadataError::DuplicatePrivateDictionary {
                                first_offset,
                                duplicate_offset: token.offset,
                            });
                        }
                        private_offset = Some(token.offset);
                    }
                    MetadataName::StdVw if private_offset.is_some() => {
                        let value = scan_std_vw_value(&mut scanner, token.offset)?;
                        if let Some((first_offset, _)) = found {
                            return Err(Type1MetadataError::DuplicateStdVw {
                                first_offset,
                                duplicate_offset: token.offset,
                            });
                        }
                        found = Some((token.offset, value));
                    }
                    MetadataName::Subrs | MetadataName::CharStrings => {
                        if private_offset.is_none() {
                            return Err(Type1MetadataError::MissingPrivateDictionary);
                        }
                        return Ok(found.map(|(_, value)| value));
                    }
                    MetadataName::StdVw | MetadataName::Other => {}
                }
            }
            _ => {}
        }
    }
    Err(if private_offset.is_some() {
        Type1MetadataError::MissingMetadataBoundary
    } else {
        Type1MetadataError::MissingPrivateDictionary
    })
}

fn scan_std_vw_value(
    scanner: &mut MetadataScanner<'_>,
    key_offset: usize,
) -> Result<AfmNumber, Type1MetadataError> {
    let opening = scanner
        .next_token()?
        .ok_or(Type1MetadataError::InvalidStdVw {
            offset: key_offset,
            kind: StdVwSyntaxError::ExpectedArray,
        })?;
    let expects_procedure_end = match opening.kind {
        MetadataTokenKind::OpenArray => false,
        MetadataTokenKind::OpenProcedure => true,
        _ => {
            return Err(Type1MetadataError::InvalidStdVw {
                offset: opening.offset,
                kind: StdVwSyntaxError::ExpectedArray,
            })
        }
    };

    let number = scanner
        .next_token()?
        .ok_or(Type1MetadataError::InvalidStdVw {
            offset: key_offset,
            kind: StdVwSyntaxError::ExpectedNumber,
        })?;
    let MetadataTokenKind::Bare { bytes, truncated } = number.kind else {
        return Err(Type1MetadataError::InvalidStdVw {
            offset: number.offset,
            kind: StdVwSyntaxError::ExpectedNumber,
        });
    };
    if truncated {
        return Err(Type1MetadataError::InvalidStdVw {
            offset: number.offset,
            kind: StdVwSyntaxError::NumberOverflow,
        });
    }
    let value =
        parse_metadata_decimal(&bytes).map_err(|kind| Type1MetadataError::InvalidStdVw {
            offset: number.offset,
            kind,
        })?;

    let closing = scanner
        .next_token()?
        .ok_or(Type1MetadataError::InvalidStdVw {
            offset: key_offset,
            kind: StdVwSyntaxError::ExpectedArrayEnd,
        })?;
    let closes_value = if expects_procedure_end {
        matches!(closing.kind, MetadataTokenKind::CloseProcedure)
    } else {
        matches!(closing.kind, MetadataTokenKind::CloseArray)
    };
    if !closes_value {
        return Err(Type1MetadataError::InvalidStdVw {
            offset: closing.offset,
            kind: StdVwSyntaxError::ExpectedArrayEnd,
        });
    }
    Ok(value)
}

fn parse_metadata_decimal(bytes: &[u8]) -> Result<AfmNumber, StdVwSyntaxError> {
    let (negative, unsigned) = match bytes.first() {
        Some(b'+') => (false, &bytes[1..]),
        Some(b'-') => (true, &bytes[1..]),
        _ => (false, bytes),
    };
    if unsigned.is_empty() {
        return Err(StdVwSyntaxError::InvalidNumber);
    }

    let mut integer = 0i128;
    let mut fraction = 0i128;
    let mut fraction_digits = 0usize;
    let mut after_decimal = false;
    let mut saw_digit = false;
    for &byte in unsigned {
        if byte == b'.' {
            if after_decimal {
                return Err(StdVwSyntaxError::InvalidNumber);
            }
            after_decimal = true;
            continue;
        }
        if !byte.is_ascii_digit() {
            return Err(StdVwSyntaxError::InvalidNumber);
        }
        saw_digit = true;
        let digit = i128::from(byte - b'0');
        if after_decimal {
            if fraction_digits < 6 {
                fraction = fraction
                    .checked_mul(10)
                    .and_then(|number| number.checked_add(digit))
                    .ok_or(StdVwSyntaxError::NumberOverflow)?;
                fraction_digits += 1;
            } else if digit != 0 {
                return Err(StdVwSyntaxError::NumberTooPrecise);
            }
        } else {
            integer = integer
                .checked_mul(10)
                .and_then(|number| number.checked_add(digit))
                .ok_or(StdVwSyntaxError::NumberOverflow)?;
        }
    }
    if !saw_digit {
        return Err(StdVwSyntaxError::InvalidNumber);
    }
    while fraction_digits < 6 {
        fraction = fraction
            .checked_mul(10)
            .ok_or(StdVwSyntaxError::NumberOverflow)?;
        fraction_digits += 1;
    }
    let magnitude = integer
        .checked_mul(i128::from(AfmNumber::SCALE))
        .and_then(|number| number.checked_add(fraction))
        .ok_or(StdVwSyntaxError::NumberOverflow)?;
    let signed = if negative { -magnitude } else { magnitude };
    let scaled = i64::try_from(signed).map_err(|_| StdVwSyntaxError::NumberOverflow)?;
    Ok(AfmNumber::from_scaled(scaled))
}

struct DecryptedEexec<'a> {
    encrypted: &'a [u8],
    encrypted_offset: usize,
    key: u16,
    discarded: usize,
    emitted: usize,
}

impl<'a> DecryptedEexec<'a> {
    fn new(encrypted: &'a [u8]) -> Self {
        Self {
            encrypted,
            encrypted_offset: 0,
            key: EEXEC_INITIAL_KEY,
            discarded: 0,
            emitted: 0,
        }
    }

    fn next_byte(&mut self) -> Result<Option<(usize, u8)>, Type1MetadataError> {
        loop {
            if self.encrypted_offset == self.encrypted.len() {
                return Ok(None);
            }
            let cipher = self.encrypted[self.encrypted_offset];
            self.encrypted_offset += 1;
            let plain = cipher ^ (self.key >> 8) as u8;
            self.key = self
                .key
                .wrapping_add(u16::from(cipher))
                .wrapping_mul(EEXEC_C1)
                .wrapping_add(EEXEC_C2);
            if self.discarded < EEXEC_RANDOM_PREFIX_BYTES {
                self.discarded += 1;
                continue;
            }
            if self.emitted == MAX_EEXEC_PRIVATE_SCAN_BYTES {
                return Err(Type1MetadataError::ScanLimitExceeded {
                    limit: MAX_EEXEC_PRIVATE_SCAN_BYTES,
                });
            }
            let offset = self.emitted;
            self.emitted += 1;
            return Ok(Some((offset, plain)));
        }
    }
}

struct MetadataScanner<'a> {
    input: DecryptedEexec<'a>,
    pending: Option<(usize, u8)>,
}

impl<'a> MetadataScanner<'a> {
    fn new(encrypted: &'a [u8]) -> Self {
        Self {
            input: DecryptedEexec::new(encrypted),
            pending: None,
        }
    }

    fn next_byte(&mut self) -> Result<Option<(usize, u8)>, Type1MetadataError> {
        if self.pending.is_some() {
            return Ok(self.pending.take());
        }
        self.input.next_byte()
    }

    fn put_back(&mut self, byte: (usize, u8)) {
        debug_assert!(self.pending.is_none());
        self.pending = Some(byte);
    }

    fn next_token(&mut self) -> Result<Option<MetadataToken>, Type1MetadataError> {
        loop {
            let Some((offset, byte)) = self.next_byte()? else {
                return Ok(None);
            };
            if is_postscript_whitespace(byte) {
                continue;
            }
            let kind = match byte {
                b'%' => {
                    self.skip_comment()?;
                    continue;
                }
                b'(' => {
                    self.skip_literal_string(offset)?;
                    MetadataTokenKind::Other
                }
                b'<' => {
                    let Some(next) = self.next_byte()? else {
                        return Err(Type1MetadataError::UnterminatedHexString { offset });
                    };
                    if next.1 == b'<' {
                        MetadataTokenKind::OpenDictionary
                    } else {
                        self.skip_hex_string(offset, next)?;
                        MetadataTokenKind::Other
                    }
                }
                b'>' => match self.next_byte()? {
                    Some((_, b'>')) => MetadataTokenKind::CloseDictionary,
                    Some(next) => {
                        self.put_back(next);
                        MetadataTokenKind::Other
                    }
                    None => MetadataTokenKind::Other,
                },
                b'/' => MetadataTokenKind::LiteralName(self.read_literal_name()?),
                b'[' => MetadataTokenKind::OpenArray,
                b']' => MetadataTokenKind::CloseArray,
                b'{' => MetadataTokenKind::OpenProcedure,
                b'}' => MetadataTokenKind::CloseProcedure,
                byte if is_postscript_delimiter(byte) => MetadataTokenKind::Other,
                first => {
                    let (bytes, truncated) = self.read_bare_token(first)?;
                    MetadataTokenKind::Bare { bytes, truncated }
                }
            };
            return Ok(Some(MetadataToken { offset, kind }));
        }
    }

    fn skip_comment(&mut self) -> Result<(), Type1MetadataError> {
        while let Some((_, byte)) = self.next_byte()? {
            if matches!(byte, b'\r' | b'\n') {
                break;
            }
        }
        Ok(())
    }

    fn skip_literal_string(&mut self, offset: usize) -> Result<(), Type1MetadataError> {
        let mut depth = 1usize;
        let mut escaped = false;
        while let Some((_, byte)) = self.next_byte()? {
            if escaped {
                escaped = false;
                continue;
            }
            match byte {
                b'\\' => escaped = true,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        Err(Type1MetadataError::UnterminatedLiteralString { offset })
    }

    fn skip_hex_string(
        &mut self,
        offset: usize,
        first: (usize, u8),
    ) -> Result<(), Type1MetadataError> {
        if first.1 == b'>' {
            return Ok(());
        }
        while let Some((_, byte)) = self.next_byte()? {
            if byte == b'>' {
                return Ok(());
            }
        }
        Err(Type1MetadataError::UnterminatedHexString { offset })
    }

    fn read_literal_name(&mut self) -> Result<MetadataName, Type1MetadataError> {
        let (bytes, truncated) = self.read_token_bytes(None)?;
        if truncated {
            return Ok(MetadataName::Other);
        }
        Ok(match bytes.as_slice() {
            b"Private" => MetadataName::Private,
            b"StdVW" => MetadataName::StdVw,
            b"Subrs" => MetadataName::Subrs,
            b"CharStrings" => MetadataName::CharStrings,
            _ => MetadataName::Other,
        })
    }

    fn read_bare_token(&mut self, first: u8) -> Result<(Vec<u8>, bool), Type1MetadataError> {
        self.read_token_bytes(Some(first))
    }

    fn read_token_bytes(
        &mut self,
        first: Option<u8>,
    ) -> Result<(Vec<u8>, bool), Type1MetadataError> {
        let mut bytes = Vec::with_capacity(16);
        let mut truncated = false;
        if let Some(first) = first {
            bytes.push(first);
        }
        while let Some(next) = self.next_byte()? {
            if is_postscript_delimiter(next.1) {
                self.put_back(next);
                break;
            }
            if bytes.len() < MAX_METADATA_TOKEN_BYTES {
                bytes.push(next.1);
            } else {
                truncated = true;
            }
        }
        Ok((bytes, truncated))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MetadataToken {
    offset: usize,
    kind: MetadataTokenKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MetadataTokenKind {
    LiteralName(MetadataName),
    Bare { bytes: Vec<u8>, truncated: bool },
    OpenArray,
    CloseArray,
    OpenProcedure,
    CloseProcedure,
    OpenDictionary,
    CloseDictionary,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MetadataName {
    Private,
    StdVw,
    Subrs,
    CharStrings,
    Other,
}

fn is_postscript_whitespace(byte: u8) -> bool {
    matches!(byte, 0 | b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

fn is_postscript_delimiter(byte: u8) -> bool {
    is_postscript_whitespace(byte)
        || matches!(
            byte,
            b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
        )
}

#[cfg(test)]
mod tests {
    use super::{
        extract_private_std_vw, parse_pfb, parse_pfb_with_limit, ExpectedPfbSegment, PfbError,
        PfbSegmentKind, StdVwSyntaxError, Type1FontProgram, Type1MetadataError, ASCII_SEGMENT,
        BINARY_SEGMENT, EEXEC_C1, EEXEC_C2, EEXEC_INITIAL_KEY, EEXEC_RANDOM_PREFIX_BYTES,
        END_SEGMENT, MAX_EEXEC_PRIVATE_SCAN_BYTES, PFB_MARKER,
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

    fn eexec暗号化(random: [u8; EEXEC_RANDOM_PREFIX_BYTES], private: &[u8]) -> Vec<u8> {
        let mut key = EEXEC_INITIAL_KEY;
        random
            .into_iter()
            .chain(private.iter().copied())
            .map(|plain| {
                let cipher = plain ^ (key >> 8) as u8;
                key = key
                    .wrapping_add(u16::from(cipher))
                    .wrapping_mul(EEXEC_C1)
                    .wrapping_add(EEXEC_C2);
                cipher
            })
            .collect()
    }

    fn metadata_program(private: &[u8]) -> Type1FontProgram {
        metadata_program_with_random(*b"rand", private)
    }

    fn metadata_program_with_random(
        random: [u8; EEXEC_RANDOM_PREFIX_BYTES],
        private: &[u8],
    ) -> Type1FontProgram {
        let mut decrypted = b"dup /Private 18 dict dup begin\n".to_vec();
        decrypted.extend_from_slice(private);
        metadata_program_from_decrypted(random, &decrypted)
    }

    fn metadata_program_from_decrypted(
        random: [u8; EEXEC_RANDOM_PREFIX_BYTES],
        decrypted: &[u8],
    ) -> Type1FontProgram {
        let initial = b"%!PS synthetic\ncurrentfile eexec\n";
        let encrypted = eexec暗号化(random, decrypted);
        let trailing = b"cleartomark\n";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(initial);
        bytes.extend_from_slice(&encrypted);
        bytes.extend_from_slice(trailing);
        Type1FontProgram {
            bytes,
            length1: initial.len(),
            length2: encrypted.len(),
            length3: trailing.len(),
        }
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

    #[test]
    fn eexec_private辞書からstdvwだけを実行せず読む() {
        let program = metadata_program(
            b"% /StdVW [11] def\n\
              (/StdVW [12] (nested\\) text))\n\
              <~/StdVW [13]~>\n\
              { /StdVW [14] def }\n\
              /Other [ /StdVW [15] ] def\n\
              /OtherDict << /StdVW [16] >> def\n\
              /StdVW [+69.25] ND\n\
              /Subrs 1 array\n",
        );

        assert_eq!(
            extract_private_std_vw(&program).unwrap().unwrap().scaled(),
            69_250_000
        );
    }

    #[test]
    fn procedure形の一要素arrayもstdvwとして読む() {
        let program = metadata_program(b"/StdVW {72.5} ND\n/CharStrings 0 dict\n");

        assert_eq!(
            extract_private_std_vw(&program).unwrap().unwrap().scaled(),
            72_500_000
        );
    }

    #[test]
    fn stdvw直後の文字列をtokenごと飛ばして受理しない() {
        for private in [
            b"/StdVW (ignored) [69] ND\n/Subrs 0 array\n".as_slice(),
            b"/StdVW <00> [69] ND\n/Subrs 0 array\n".as_slice(),
            b"/StdVW [(ignored) 69] ND\n/Subrs 0 array\n".as_slice(),
        ] {
            assert!(matches!(
                extract_private_std_vw(&metadata_program(private)),
                Err(Type1MetadataError::InvalidStdVw { .. })
            ));
        }
    }

    #[test]
    fn private辞書より前の偽stdvwを採用しない() {
        let program = metadata_program_from_decrypted(
            *b"rand",
            b"/StdVW [99] pop\n\
              dup /Private 18 dict dup begin\n\
              /StdVW [69] ND\n\
              /Subrs 0 array\n",
        );

        assert_eq!(
            extract_private_std_vw(&program).unwrap().unwrap().scaled(),
            69_000_000
        );
    }

    #[test]
    fn eexecの最初の四byteをstem値として誤認しない() {
        let program = metadata_program_with_random(
            *b"/Std",
            b"VW [999] def\n/StdVW [69] def\n/Subrs 0 array\n",
        );

        assert_eq!(
            extract_private_std_vw(&program).unwrap().unwrap().scaled(),
            69_000_000
        );
    }

    #[test]
    fn 連結したbinary_segment境界を越えてstdvwを読む() {
        let private = b"dup /Private 18 dict dup begin\n/BlueValues [] def\n/StdVW [71] def\n/CharStrings 1 dict\n";
        let encrypted = eexec暗号化(*b"rand", private);
        let split = EEXEC_RANDOM_PREFIX_BYTES
            + b"dup /Private 18 dict dup begin\n/BlueValues [] def\n/St".len();
        let initial = データsegment(ASCII_SEGMENT, b"%!PS\ncurrentfile eexec\n");
        let binary1 = データsegment(BINARY_SEGMENT, &encrypted[..split]);
        let binary2 = データsegment(BINARY_SEGMENT, &encrypted[split..]);
        let trailing = データsegment(ASCII_SEGMENT, b"cleartomark\n");
        let end = 終端segment();
        let input = 連結(&[&initial, &binary1, &binary2, &trailing, &end]);
        let program = parse_pfb(&input).unwrap();

        assert_eq!(
            extract_private_std_vw(&program).unwrap().unwrap().scaled(),
            71_000_000
        );
    }

    #[test]
    fn 重複したstdvwを最初の値で隠さない() {
        let program = metadata_program(
            b"/StdVW [69] def\n/BlueValues [] def\n/StdVW [70] def\n/Subrs 0 array\n",
        );

        assert!(matches!(
            extract_private_std_vw(&program),
            Err(Type1MetadataError::DuplicateStdVw { .. })
        ));
    }

    #[test]
    fn 壊れたstdvwの形と数値を区別して拒む() {
        let cases: &[(&[u8], StdVwSyntaxError)] = &[
            (
                b"/StdVW 69 def\n/Subrs 0 array\n",
                StdVwSyntaxError::ExpectedArray,
            ),
            (
                b"/StdVW [wrong] def\n/Subrs 0 array\n",
                StdVwSyntaxError::InvalidNumber,
            ),
            (
                b"/StdVW [69 70] def\n/Subrs 0 array\n",
                StdVwSyntaxError::ExpectedArrayEnd,
            ),
            (
                b"/StdVW [999999999999999999999999999999999999999] def\n/Subrs 0 array\n",
                StdVwSyntaxError::NumberOverflow,
            ),
            (
                b"/StdVW [1.0000001] def\n/Subrs 0 array\n",
                StdVwSyntaxError::NumberTooPrecise,
            ),
        ];

        for (private, expected) in cases {
            let program = metadata_program(private);
            assert!(matches!(
                extract_private_std_vw(&program),
                Err(Type1MetadataError::InvalidStdVw { kind, .. }) if kind == *expected
            ));
        }
    }

    #[test]
    fn charstringsより後のbyte列をstdvwとして誤認しない() {
        let program = metadata_program(
            b"/BlueValues [] def\n/CharStrings 1 dict dup begin\n/StdVW [99] def\n",
        );
        assert_eq!(extract_private_std_vw(&program), Ok(None));

        let program = metadata_program(b"/BlueValues [] def\n/Subrs 1 array\n/StdVW [98] def\n");
        assert_eq!(extract_private_std_vw(&program), Ok(None));
    }

    #[test]
    fn stdvwが無いprivate辞書を未指定として保つ() {
        let program = metadata_program(
            b"/BlueValues [-20 0 680 700] def\n\
              /stdvw [98] def\n\
              /StdVWX [99] def\n\
              /Subrs 0 array\n",
        );
        assert_eq!(extract_private_std_vw(&program), Ok(None));
    }

    #[test]
    fn 閉じない文字列とscan上限を型付きerrorにする() {
        let unterminated = metadata_program(b"(/StdVW [99]\n");
        assert!(matches!(
            extract_private_std_vw(&unterminated),
            Err(Type1MetadataError::UnterminatedLiteralString { .. })
        ));

        let oversized = vec![b'a'; MAX_EEXEC_PRIVATE_SCAN_BYTES + 1];
        let oversized = metadata_program(&oversized);
        assert_eq!(
            extract_private_std_vw(&oversized),
            Err(Type1MetadataError::ScanLimitExceeded {
                limit: MAX_EEXEC_PRIVATE_SCAN_BYTES
            })
        );
    }

    #[test]
    fn private境界の欠落と暗号範囲の不整合を拒む() {
        let no_private =
            metadata_program_from_decrypted(*b"rand", b"/StdVW [99] pop\n/CharStrings 0 dict\n");
        assert_eq!(
            extract_private_std_vw(&no_private),
            Err(Type1MetadataError::MissingPrivateDictionary)
        );

        let no_boundary = metadata_program(b"/StdVW [69] ND\n");
        assert_eq!(
            extract_private_std_vw(&no_boundary),
            Err(Type1MetadataError::MissingMetadataBoundary)
        );

        let invalid_range = Type1FontProgram {
            bytes: vec![0; 8],
            length1: usize::MAX,
            length2: 2,
            length3: 0,
        };
        assert!(matches!(
            extract_private_std_vw(&invalid_range),
            Err(Type1MetadataError::InvalidEncryptedRange { .. })
        ));
    }
}

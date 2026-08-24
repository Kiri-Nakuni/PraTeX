use super::FormatError;

use std::io::Write;

const MAGIC: &[u8; 8] = b"PRATEXF\0";
const FORMAT_MAJOR: u16 = 1;
const FORMAT_MINOR: u16 = 0;
const REQUIRED_SECTION: u32 = 1;
const SECTION_COUNT: u16 = 3;
const HEADER_PREFIX_BYTES: usize = 16;
const SECTION_ENTRY_BYTES: usize = 28;
const HEADER_BYTES: usize = HEADER_PREFIX_BYTES + SECTION_ENTRY_BYTES * SECTION_COUNT as usize;

const EQTB_LEGACY_TEXT: u16 = 1;
const HYPHEN_RUNTIME: u16 = 2;
const RUN_METADATA_LEGACY_TEXT: u16 = 3;
const EQTB_LEGACY_TEXT_VERSION: u16 = 0;
const HYPHEN_RUNTIME_VERSION: u16 = 1;
const RUN_METADATA_LEGACY_TEXT_VERSION: u16 = 0;

/// `RawStringRegisters` alone can represent 64 MiB as one decimal byte per line.
/// Keep the file bound above that legal legacy encoding while stopping an untrusted
/// stream before an unbounded `read_to_end` allocation.
pub(crate) const MAX_FORMAT_FILE_BYTES: usize = 512 * 1024 * 1024;

pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

pub(crate) struct FormatSections<'a> {
    pub(crate) eqtb_legacy_text: &'a [u8],
    pub(crate) hyphen_runtime: &'a [u8],
    pub(crate) run_metadata_legacy_text: &'a [u8],
}

struct Section<'a> {
    kind: u16,
    version: u16,
    bytes: &'a [u8],
}

pub(crate) fn write_format(
    target: &mut impl Write,
    eqtb_legacy_text: &[u8],
    hyphen_runtime: &[u8],
    run_metadata_legacy_text: &[u8],
) -> Result<(), std::io::Error> {
    let sections = [
        Section {
            kind: EQTB_LEGACY_TEXT,
            version: EQTB_LEGACY_TEXT_VERSION,
            bytes: eqtb_legacy_text,
        },
        Section {
            kind: HYPHEN_RUNTIME,
            version: HYPHEN_RUNTIME_VERSION,
            bytes: hyphen_runtime,
        },
        Section {
            kind: RUN_METADATA_LEGACY_TEXT,
            version: RUN_METADATA_LEGACY_TEXT_VERSION,
            bytes: run_metadata_legacy_text,
        },
    ];

    let mut header = BinaryWriter::with_capacity(HEADER_BYTES);
    header.write_bytes(MAGIC);
    header.write_u16(FORMAT_MAJOR);
    header.write_u16(FORMAT_MINOR);
    header.write_u16(SECTION_COUNT);
    header.write_u16(0);

    let mut offset = u64::try_from(HEADER_BYTES).expect("format header length fits u64");
    for section in &sections {
        let length = u64::try_from(section.bytes.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "format section is too large",
            )
        })?;
        header.write_u16(section.kind);
        header.write_u16(section.version);
        header.write_u32(REQUIRED_SECTION);
        header.write_u64(offset);
        header.write_u64(length);
        header.write_u32(crc32(section.bytes));
        offset = offset.checked_add(length).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "format length overflow")
        })?;
    }
    debug_assert_eq!(header.len(), HEADER_BYTES);
    target.write_all(header.as_slice())?;
    for section in sections {
        target.write_all(section.bytes)?;
    }
    Ok(())
}

pub(crate) fn parse_format(bytes: &[u8]) -> Result<FormatSections<'_>, FormatError> {
    if bytes.len() > MAX_FORMAT_FILE_BYTES {
        return Err(FormatError::AllocationFailed);
    }
    let mut reader = BinaryReader::new(bytes);
    if reader.read_bytes(MAGIC.len())? != MAGIC {
        return Err(FormatError::ParseError);
    }
    if reader.read_u16()? != FORMAT_MAJOR {
        return Err(FormatError::UnsupportedVersion);
    }
    if reader.read_u16()? != FORMAT_MINOR
        || reader.read_u16()? != SECTION_COUNT
        || reader.read_u16()? != 0
    {
        return Err(FormatError::UnsupportedVersion);
    }

    let expected = [
        (EQTB_LEGACY_TEXT, EQTB_LEGACY_TEXT_VERSION),
        (HYPHEN_RUNTIME, HYPHEN_RUNTIME_VERSION),
        (RUN_METADATA_LEGACY_TEXT, RUN_METADATA_LEGACY_TEXT_VERSION),
    ];
    let mut ranges = [(0usize, 0usize); SECTION_COUNT as usize];
    let mut next_offset = HEADER_BYTES;
    for (index, (expected_kind, expected_version)) in expected.into_iter().enumerate() {
        let kind = reader.read_u16()?;
        let version = reader.read_u16()?;
        let flags = reader.read_u32()?;
        let offset = usize::try_from(reader.read_u64()?).map_err(|_| FormatError::ParseError)?;
        let length = usize::try_from(reader.read_u64()?).map_err(|_| FormatError::ParseError)?;
        let expected_checksum = reader.read_u32()?;
        if kind != expected_kind || version != expected_version || flags != REQUIRED_SECTION {
            return Err(FormatError::UnsupportedVersion);
        }
        if offset != next_offset {
            return Err(FormatError::ParseError);
        }
        let end = offset
            .checked_add(length)
            .filter(|&end| end <= bytes.len())
            .ok_or(FormatError::IncompleteFile)?;
        let section = &bytes[offset..end];
        if crc32(section) != expected_checksum {
            return Err(FormatError::WrongChecksum);
        }
        ranges[index] = (offset, end);
        next_offset = end;
    }
    if reader.position() != HEADER_BYTES || next_offset != bytes.len() {
        return Err(FormatError::ParseError);
    }

    Ok(FormatSections {
        eqtb_legacy_text: &bytes[ranges[0].0..ranges[0].1],
        hyphen_runtime: &bytes[ranges[1].0..ranges[1].1],
        run_metadata_legacy_text: &bytes[ranges[2].0..ranges[2].1],
    })
}

pub(crate) struct BinaryWriter {
    bytes: Vec<u8>,
}

impl BinaryWriter {
    pub(crate) fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn write_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

pub(crate) struct BinaryReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> BinaryReader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, FormatError> {
        Ok(self.read_bytes(1)?[0])
    }

    pub(crate) fn read_u16(&mut self) -> Result<u16, FormatError> {
        let bytes: [u8; 2] = self
            .read_bytes(2)?
            .try_into()
            .map_err(|_| FormatError::IncompleteFile)?;
        Ok(u16::from_le_bytes(bytes))
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32, FormatError> {
        let bytes: [u8; 4] = self
            .read_bytes(4)?
            .try_into()
            .map_err(|_| FormatError::IncompleteFile)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64, FormatError> {
        let bytes: [u8; 8] = self
            .read_bytes(8)?
            .try_into()
            .map_err(|_| FormatError::IncompleteFile)?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub(crate) fn read_usize_u32(&mut self) -> Result<usize, FormatError> {
        usize::try_from(self.read_u32()?).map_err(|_| FormatError::ParseError)
    }

    pub(crate) fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], FormatError> {
        let end = self
            .position
            .checked_add(length)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(FormatError::IncompleteFile)?;
        let bytes = &self.bytes[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    pub(crate) fn finish(self) -> Result<(), FormatError> {
        (self.position == self.bytes.len())
            .then_some(())
            .ok_or(FormatError::ParseError)
    }
}

const fn make_crc32_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut index = 0;
    while index < table.len() {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 == 1 {
                0xedb8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

const CRC32_TABLE: [u32; 256] = make_crc32_table();

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        let index = ((crc ^ u32::from(byte)) & 0xff) as usize;
        crc = CRC32_TABLE[index] ^ (crc >> 8);
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_format() -> Vec<u8> {
        let mut bytes = Vec::new();
        write_format(&mut bytes, b"eqtb\n", b"hyphen", b"metadata\n").unwrap();
        bytes
    }

    #[test]
    fn crc32の既知値をlittle_endian_sectionへ使う() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn 三sectionを長さとchecksum付きで往復する() {
        let bytes = sample_format();
        let sections = parse_format(&bytes).unwrap();
        assert_eq!(sections.eqtb_legacy_text, b"eqtb\n");
        assert_eq!(sections.hyphen_runtime, b"hyphen");
        assert_eq!(sections.run_metadata_legacy_text, b"metadata\n");
    }

    #[test]
    fn checksumが違うsectionをdecode前に拒否する() {
        let mut bytes = sample_format();
        *bytes.last_mut().unwrap() ^= 1;
        assert!(matches!(
            parse_format(&bytes),
            Err(FormatError::WrongChecksum)
        ));
    }

    #[test]
    fn 未対応majorとsection版を拒否する() {
        let mut major = sample_format();
        major[8..10].copy_from_slice(&2u16.to_le_bytes());
        assert!(matches!(
            parse_format(&major),
            Err(FormatError::UnsupportedVersion)
        ));

        let mut section = sample_format();
        section[18..20].copy_from_slice(&1u16.to_le_bytes());
        assert!(matches!(
            parse_format(&section),
            Err(FormatError::UnsupportedVersion)
        ));
    }

    #[test]
    fn sectionの隙間と末尾余剰を拒否する() {
        let mut gap = sample_format();
        let offset_start = HEADER_PREFIX_BYTES + 8;
        let offset = u64::from_le_bytes(gap[offset_start..offset_start + 8].try_into().unwrap());
        gap[offset_start..offset_start + 8].copy_from_slice(&(offset + 1).to_le_bytes());
        assert!(matches!(parse_format(&gap), Err(FormatError::ParseError)));

        let mut trailing = sample_format();
        trailing.push(0);
        assert!(matches!(
            parse_format(&trailing),
            Err(FormatError::ParseError)
        ));
    }
}

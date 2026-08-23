//! PraTeX WASM provider ABI 0.0 の runtime 非依存 byte codec。
//!
//! `docs/wasm-provider-abi-v0.md` 8 節の固定 header、section directory、status、
//! invocation limits だけを扱う。module の version range、feature、capability、operation の
//! 交渉は `wasm_provider_abi` が一度だけ行い、この module では重複して判断しない。
//!
//! decoder は全 range、record、section set、reserved field、diagnostic を検証し終えるまで
//! `DecodedMessageV0` を返さない。従って caller が結果を受け取れた時点では message 全体が
//! valid であり、途中まで読んだ section を publish する API はない。

#![forbid(unsafe_code)]

const MAGIC_V0: [u8; 8] = *b"PRTXW0\0\0";
const ENVELOPE_BYTES_V0: u32 = 64;
const SECTION_DIRECTORY_ENTRY_BYTES_V0: u32 = 16;
const WIRE_OFFSET_ALIGNMENT_V0: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageKindV0 {
    SpacingTableUploadRequest,
    SpacingTableUploadResponse,
    SpacingBatchRequest,
    SpacingBatchResponse,
    UnitTableUploadRequest,
    UnitTableUploadResponse,
    UnitContextBatchRequest,
    UnitContextBatchResponse,
}

impl MessageKindV0 {
    pub(crate) const fn to_wire(self) -> u32 {
        match self {
            Self::SpacingTableUploadRequest => 1,
            Self::SpacingTableUploadResponse => 2,
            Self::SpacingBatchRequest => 3,
            Self::SpacingBatchResponse => 4,
            Self::UnitTableUploadRequest => 5,
            Self::UnitTableUploadResponse => 6,
            Self::UnitContextBatchRequest => 7,
            Self::UnitContextBatchResponse => 8,
        }
    }

    const fn from_wire(value: u32) -> Option<Self> {
        Some(match value {
            1 => Self::SpacingTableUploadRequest,
            2 => Self::SpacingTableUploadResponse,
            3 => Self::SpacingBatchRequest,
            4 => Self::SpacingBatchResponse,
            5 => Self::UnitTableUploadRequest,
            6 => Self::UnitTableUploadResponse,
            7 => Self::UnitContextBatchRequest,
            8 => Self::UnitContextBatchResponse,
            _ => return None,
        })
    }

    pub(crate) const fn is_response(self) -> bool {
        matches!(
            self,
            Self::SpacingTableUploadResponse
                | Self::SpacingBatchResponse
                | Self::UnitTableUploadResponse
                | Self::UnitContextBatchResponse
        )
    }

    const fn flags(self) -> u32 {
        if self.is_response() {
            1
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SectionKindV0 {
    Status,
    InvocationLimits,
    SpacingTableConfig,
    SpacingClassRange,
    SpacingPairRule,
    SpacingBatchContext,
    BoundaryAtom,
    Boundary,
    BoundaryAction,
    UnitTableConfig,
    UnitDeclaration,
    UnitContext,
    UnitQuery,
    UnitScaleResult,
}

impl SectionKindV0 {
    pub(crate) const fn to_wire(self) -> u32 {
        match self {
            Self::Status => 0x0000_0001,
            Self::InvocationLimits => 0x0000_0002,
            Self::SpacingTableConfig => 0x0000_1001,
            Self::SpacingClassRange => 0x0000_1002,
            Self::SpacingPairRule => 0x0000_1003,
            Self::SpacingBatchContext => 0x0000_1101,
            Self::BoundaryAtom => 0x0000_1102,
            Self::Boundary => 0x0000_1103,
            Self::BoundaryAction => 0x0000_1104,
            Self::UnitTableConfig => 0x0000_2001,
            Self::UnitDeclaration => 0x0000_2002,
            Self::UnitContext => 0x0000_2101,
            Self::UnitQuery => 0x0000_2102,
            Self::UnitScaleResult => 0x0000_2103,
        }
    }

    const fn from_wire(value: u32) -> Option<Self> {
        Some(match value {
            0x0000_0001 => Self::Status,
            0x0000_0002 => Self::InvocationLimits,
            0x0000_1001 => Self::SpacingTableConfig,
            0x0000_1002 => Self::SpacingClassRange,
            0x0000_1003 => Self::SpacingPairRule,
            0x0000_1101 => Self::SpacingBatchContext,
            0x0000_1102 => Self::BoundaryAtom,
            0x0000_1103 => Self::Boundary,
            0x0000_1104 => Self::BoundaryAction,
            0x0000_2001 => Self::UnitTableConfig,
            0x0000_2002 => Self::UnitDeclaration,
            0x0000_2101 => Self::UnitContext,
            0x0000_2102 => Self::UnitQuery,
            0x0000_2103 => Self::UnitScaleResult,
            _ => return None,
        })
    }

    const fn record_bytes(self) -> u32 {
        match self {
            Self::Status => 16,
            Self::InvocationLimits => 32,
            Self::SpacingTableConfig => 32,
            Self::SpacingClassRange => 24,
            Self::SpacingPairRule => 88,
            Self::SpacingBatchContext => 40,
            Self::BoundaryAtom => 36,
            Self::Boundary => 16,
            Self::BoundaryAction => 80,
            Self::UnitTableConfig => 24,
            Self::UnitDeclaration => 64,
            Self::UnitContext => 80,
            Self::UnitQuery => 16,
            Self::UnitScaleResult => 24,
        }
    }

    const fn requires_one_record(self) -> bool {
        matches!(
            self,
            Self::Status
                | Self::InvocationLimits
                | Self::SpacingTableConfig
                | Self::SpacingBatchContext
                | Self::UnitTableConfig
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusCodeV0 {
    Ok,
    UnsupportedOperation,
    InvalidRequest,
    ProviderFailure,
    LimitWouldBeExceeded,
    CannotResolve,
}

impl StatusCodeV0 {
    pub(crate) const fn to_wire(self) -> u32 {
        match self {
            Self::Ok => 0,
            Self::UnsupportedOperation => 1,
            Self::InvalidRequest => 2,
            Self::ProviderFailure => 3,
            Self::LimitWouldBeExceeded => 4,
            Self::CannotResolve => 5,
        }
    }

    const fn from_wire(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::Ok,
            1 => Self::UnsupportedOperation,
            2 => Self::InvalidRequest,
            3 => Self::ProviderFailure,
            4 => Self::LimitWouldBeExceeded,
            5 => Self::CannotResolve,
            _ => return None,
        })
    }

    const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InvocationLimitsV0 {
    pub(crate) max_response_bytes: u32,
    pub(crate) max_records: u32,
    pub(crate) max_payload_bytes: u32,
    pub(crate) max_diagnostic_bytes: u32,
    pub(crate) fuel_model_id: u32,
    pub(crate) call_ordinal: u32,
    pub(crate) fuel_limit: u64,
}

impl InvocationLimitsV0 {
    pub(crate) fn to_record_bytes(self) -> [u8; 32] {
        let mut output = [0; 32];
        output[0..4].copy_from_slice(&self.max_response_bytes.to_le_bytes());
        output[4..8].copy_from_slice(&self.max_records.to_le_bytes());
        output[8..12].copy_from_slice(&self.max_payload_bytes.to_le_bytes());
        output[12..16].copy_from_slice(&self.max_diagnostic_bytes.to_le_bytes());
        output[16..20].copy_from_slice(&self.fuel_model_id.to_le_bytes());
        output[20..24].copy_from_slice(&self.call_ordinal.to_le_bytes());
        output[24..32].copy_from_slice(&self.fuel_limit.to_le_bytes());
        output
    }

    fn from_record_bytes(bytes: &[u8]) -> Result<Self, WireErrorV0> {
        if bytes.len() != 32 {
            return Err(WireErrorV0::CutRecord {
                section: SectionKindV0::InvocationLimits,
                bytes: bytes.len() as u64,
                record_bytes: 32,
            });
        }
        Ok(Self {
            max_response_bytes: read_u32(bytes, 0, "max_response_bytes")?,
            max_records: read_u32(bytes, 4, "max_records")?,
            max_payload_bytes: read_u32(bytes, 8, "max_payload_bytes")?,
            max_diagnostic_bytes: read_u32(bytes, 12, "max_diagnostic_bytes")?,
            fuel_model_id: read_u32(bytes, 16, "fuel_model_id")?,
            call_ordinal: read_u32(bytes, 20, "call_ordinal")?,
            fuel_limit: read_u64(bytes, 24, "fuel_limit")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WireValidationLimitsV0 {
    pub(crate) max_request_bytes: u32,
    pub(crate) max_response_bytes: u32,
    pub(crate) max_sections: u32,
    pub(crate) max_records: u32,
    pub(crate) max_payload_bytes: u32,
    pub(crate) max_diagnostic_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveWireValidationLimitsV0 {
    max_message_bytes: u32,
    max_sections: u32,
    max_records: u32,
    max_payload_bytes: u32,
    max_diagnostic_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EnvelopeMetaV0 {
    pub(crate) message_kind: MessageKindV0,
    pub(crate) request_id: u64,
    pub(crate) capabilities: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WireExpectationsV0 {
    pub(crate) envelope: EnvelopeMetaV0,
    pub(crate) invocation_limits: InvocationLimitsV0,
}

impl WireExpectationsV0 {
    pub(crate) const fn request(
        message_kind: MessageKindV0,
        request_id: u64,
        capabilities: u64,
        invocation_limits: InvocationLimitsV0,
    ) -> Self {
        Self {
            envelope: EnvelopeMetaV0 {
                message_kind,
                request_id,
                capabilities,
            },
            invocation_limits,
        }
    }

    pub(crate) const fn response(
        message_kind: MessageKindV0,
        request_id: u64,
        capabilities: u64,
        invocation_limits: InvocationLimitsV0,
    ) -> Self {
        Self {
            envelope: EnvelopeMetaV0 {
                message_kind,
                request_id,
                capabilities,
            },
            invocation_limits,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EnvelopeV0 {
    pub(crate) message_kind: MessageKindV0,
    pub(crate) total_bytes: u32,
    pub(crate) request_id: u64,
    pub(crate) capabilities: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WireRangeV0 {
    start: usize,
    end: usize,
}

impl WireRangeV0 {
    const fn len(self) -> usize {
        self.end - self.start
    }

    const fn is_empty(self) -> bool {
        self.start == self.end
    }

    const fn overlaps(self, other: Self) -> bool {
        !self.is_empty() && !other.is_empty() && self.start < other.end && other.start < self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SectionV0 {
    pub(crate) kind: SectionKindV0,
    pub(crate) record_bytes: u32,
    pub(crate) record_count: u32,
    range: WireRangeV0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StatusV0<'a> {
    pub(crate) code: StatusCodeV0,
    pub(crate) detail: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DecodedMessageV0<'a> {
    bytes: &'a [u8],
    pub(crate) envelope: EnvelopeV0,
    sections: Vec<SectionV0>,
    payload: WireRangeV0,
    status: Option<StatusV0<'a>>,
    invocation_limits: Option<InvocationLimitsV0>,
}

impl<'a> DecodedMessageV0<'a> {
    pub(crate) fn sections(&self) -> &[SectionV0] {
        &self.sections
    }

    pub(crate) fn section_records(&self, kind: SectionKindV0) -> Option<&'a [u8]> {
        self.sections
            .iter()
            .find(|section| section.kind == kind)
            .and_then(|section| self.bytes.get(section.range.start..section.range.end))
    }

    pub(crate) fn payload(&self) -> &'a [u8] {
        self.bytes
            .get(self.payload.start..self.payload.end)
            .unwrap_or(&[])
    }

    pub(crate) const fn status(&self) -> Option<StatusV0<'a>> {
        self.status
    }

    pub(crate) const fn invocation_limits(&self) -> Option<InvocationLimitsV0> {
        self.invocation_limits
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WireSectionSourceV0<'a> {
    pub(crate) kind: SectionKindV0,
    pub(crate) records: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MailboxRangeV0 {
    pub(crate) base: u32,
    pub(crate) capacity: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MailboxLimitsV0 {
    pub(crate) max_request_bytes: u32,
    pub(crate) max_response_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValidatedMailboxesV0 {
    pub(crate) request: MailboxRangeV0,
    pub(crate) response: MailboxRangeV0,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WireErrorV0 {
    AllocationFailed,
    Truncated {
        field: &'static str,
    },
    MessageTooLarge {
        actual: u64,
        maximum: u64,
    },
    InvalidMagic,
    UnsupportedAbiVersion {
        major: u16,
        minor: u16,
    },
    InvalidHeaderBytes {
        actual: u32,
    },
    UnknownMessageKind {
        value: u32,
    },
    UnexpectedMessageKind {
        expected: MessageKindV0,
        actual: MessageKindV0,
    },
    InvalidFlags {
        actual: u32,
        expected: u32,
    },
    TotalLengthMismatch {
        declared: u32,
        actual: u64,
    },
    UnexpectedRequestId {
        expected: u64,
        actual: u64,
    },
    UnexpectedCapabilities {
        expected: u64,
        actual: u64,
    },
    TooManySections {
        actual: u32,
        maximum: u32,
    },
    SectionDirectoryBeforeHeader {
        offset: u32,
    },
    SectionDirectoryBytesMismatch {
        declared: u32,
        expected: u64,
    },
    PayloadTooLarge {
        actual: u32,
        maximum: u32,
    },
    ArithmeticOverflow {
        field: &'static str,
    },
    RangeOutOfBounds {
        field: &'static str,
        start: u64,
        length: u64,
        total: u64,
    },
    UnalignedOffset {
        field: &'static str,
        offset: u32,
    },
    UnknownSectionKind {
        value: u32,
    },
    SectionsNotStrictlyOrdered {
        previous: SectionKindV0,
        current: SectionKindV0,
    },
    RecordSizeMismatch {
        section: SectionKindV0,
        actual: u32,
        expected: u32,
    },
    CutRecord {
        section: SectionKindV0,
        bytes: u64,
        record_bytes: u32,
    },
    RecordCountMismatch {
        section: SectionKindV0,
        actual: u32,
        expected: u32,
    },
    TooManyRecords {
        actual: u64,
        maximum: u32,
    },
    OverlappingRanges {
        first: &'static str,
        second: &'static str,
    },
    SectionSetMismatch {
        message_kind: MessageKindV0,
    },
    NonZeroReserved {
        section: SectionKindV0,
        record_index: u32,
        offset_in_record: u32,
    },
    MissingStatus,
    UnknownStatus {
        code: u32,
    },
    SuccessfulStatusHasDetail {
        offset: u32,
        length: u32,
    },
    EmptyDiagnosticHasOffset {
        offset: u32,
    },
    DiagnosticTooLarge {
        actual: u32,
        maximum: u32,
    },
    InvalidDiagnosticUtf8 {
        valid_up_to: usize,
    },
    InvocationLimitExceedsCodec {
        field: &'static str,
        declared: u32,
        maximum: u32,
    },
    InvocationLimitsMismatch,
    MailboxCapacityTooSmall {
        mailbox: &'static str,
        capacity: u32,
    },
    MailboxCapacityTooLarge {
        mailbox: &'static str,
        capacity: u32,
        maximum: u32,
    },
    MailboxRangeOverflow {
        mailbox: &'static str,
    },
    MailboxOutsideMemory {
        mailbox: &'static str,
        end: u64,
        memory_bytes: u64,
    },
    MailboxesOverlap,
    NonZeroTransportStatus {
        status: u32,
    },
    ResponseLengthTooSmall {
        actual: u32,
    },
    ResponseLengthExceedsCapacity {
        actual: u32,
        capacity: u32,
    },
    ResponseLengthExceedsLease {
        actual: u32,
        maximum: u32,
    },
}

pub(crate) fn validate_mailboxes_v0(
    memory_bytes: u64,
    request: MailboxRangeV0,
    response: MailboxRangeV0,
    limits: MailboxLimitsV0,
) -> Result<ValidatedMailboxesV0, WireErrorV0> {
    let request_range =
        validate_mailbox_range("request", memory_bytes, request, limits.max_request_bytes)?;
    let response_range = validate_mailbox_range(
        "response",
        memory_bytes,
        response,
        limits.max_response_bytes,
    )?;
    if request_range.overlaps(response_range) {
        return Err(WireErrorV0::MailboxesOverlap);
    }
    Ok(ValidatedMailboxesV0 { request, response })
}

/// `pratex_wasm_invoke_v0` のpacked返値を、response memoryへ触れる前に検査する。
///
/// runtime adapterはtrap、fuel切れ、cancel時にはこの関数にもdecodeにも進まず、response
/// mailboxを一byteも読まない。`Err`でも同様で、`Ok(length)`だけがsafe copyへ進める値である。
pub(crate) fn validate_transport_result_v0(
    packed_result: i64,
    response_capacity: u32,
    max_response_bytes: u32,
) -> Result<u32, WireErrorV0> {
    let bits = packed_result as u64;
    let transport_status = (bits >> 32) as u32;
    if transport_status != 0 {
        return Err(WireErrorV0::NonZeroTransportStatus {
            status: transport_status,
        });
    }
    let response_length = bits as u32;
    if response_length < ENVELOPE_BYTES_V0 {
        return Err(WireErrorV0::ResponseLengthTooSmall {
            actual: response_length,
        });
    }
    if response_length > response_capacity {
        return Err(WireErrorV0::ResponseLengthExceedsCapacity {
            actual: response_length,
            capacity: response_capacity,
        });
    }
    if response_length > max_response_bytes {
        return Err(WireErrorV0::ResponseLengthExceedsLease {
            actual: response_length,
            maximum: max_response_bytes,
        });
    }
    Ok(response_length)
}

fn validate_mailbox_range(
    name: &'static str,
    memory_bytes: u64,
    mailbox: MailboxRangeV0,
    maximum: u32,
) -> Result<WireRangeV0, WireErrorV0> {
    if mailbox.capacity < ENVELOPE_BYTES_V0 {
        return Err(WireErrorV0::MailboxCapacityTooSmall {
            mailbox: name,
            capacity: mailbox.capacity,
        });
    }
    if mailbox.capacity > maximum {
        return Err(WireErrorV0::MailboxCapacityTooLarge {
            mailbox: name,
            capacity: mailbox.capacity,
            maximum,
        });
    }
    let end = mailbox
        .base
        .checked_add(mailbox.capacity)
        .ok_or(WireErrorV0::MailboxRangeOverflow { mailbox: name })?;
    if u64::from(end) > memory_bytes {
        return Err(WireErrorV0::MailboxOutsideMemory {
            mailbox: name,
            end: u64::from(end),
            memory_bytes,
        });
    }
    Ok(WireRangeV0 {
        start: usize::try_from(mailbox.base).map_err(|_| WireErrorV0::ArithmeticOverflow {
            field: "mailbox base usize",
        })?,
        end: usize::try_from(end).map_err(|_| WireErrorV0::ArithmeticOverflow {
            field: "mailbox end usize",
        })?,
    })
}

pub(crate) fn decode_message_v0<'a>(
    bytes: &'a [u8],
    expectations: WireExpectationsV0,
    limits: WireValidationLimitsV0,
) -> Result<DecodedMessageV0<'a>, WireErrorV0> {
    validate_invocation_limits(expectations.invocation_limits, limits)?;
    let active_limits = limits_for_message(expectations, limits);
    if bytes.len() as u64 > u64::from(active_limits.max_message_bytes) {
        return Err(WireErrorV0::MessageTooLarge {
            actual: bytes.len() as u64,
            maximum: u64::from(active_limits.max_message_bytes),
        });
    }
    if bytes.len() < ENVELOPE_BYTES_V0 as usize {
        return Err(WireErrorV0::Truncated {
            field: "EnvelopeV0",
        });
    }
    if bytes.get(0..8) != Some(MAGIC_V0.as_slice()) {
        return Err(WireErrorV0::InvalidMagic);
    }

    let abi_major = read_u16(bytes, 8, "abi_major")?;
    let abi_minor = read_u16(bytes, 10, "abi_minor")?;
    if abi_major != 0 || abi_minor != 0 {
        return Err(WireErrorV0::UnsupportedAbiVersion {
            major: abi_major,
            minor: abi_minor,
        });
    }
    let header_bytes = read_u32(bytes, 12, "header_bytes")?;
    if header_bytes != ENVELOPE_BYTES_V0 {
        return Err(WireErrorV0::InvalidHeaderBytes {
            actual: header_bytes,
        });
    }

    let raw_kind = read_u32(bytes, 16, "message_kind")?;
    let message_kind = MessageKindV0::from_wire(raw_kind)
        .ok_or(WireErrorV0::UnknownMessageKind { value: raw_kind })?;
    if message_kind != expectations.envelope.message_kind {
        return Err(WireErrorV0::UnexpectedMessageKind {
            expected: expectations.envelope.message_kind,
            actual: message_kind,
        });
    }
    let flags = read_u32(bytes, 20, "flags")?;
    if flags != message_kind.flags() {
        return Err(WireErrorV0::InvalidFlags {
            actual: flags,
            expected: message_kind.flags(),
        });
    }

    let total_bytes = read_u32(bytes, 24, "total_bytes")?;
    if u64::from(total_bytes) != bytes.len() as u64 {
        return Err(WireErrorV0::TotalLengthMismatch {
            declared: total_bytes,
            actual: bytes.len() as u64,
        });
    }
    let request_id = read_u64(bytes, 48, "request_id")?;
    if request_id != expectations.envelope.request_id {
        return Err(WireErrorV0::UnexpectedRequestId {
            expected: expectations.envelope.request_id,
            actual: request_id,
        });
    }
    let capabilities = read_u64(bytes, 56, "capabilities")?;
    if capabilities != expectations.envelope.capabilities {
        return Err(WireErrorV0::UnexpectedCapabilities {
            expected: expectations.envelope.capabilities,
            actual: capabilities,
        });
    }

    let section_count = read_u32(bytes, 28, "section_count")?;
    if section_count > active_limits.max_sections {
        return Err(WireErrorV0::TooManySections {
            actual: section_count,
            maximum: active_limits.max_sections,
        });
    }
    let section_dir_offset = read_u32(bytes, 32, "section_dir_offset")?;
    if section_dir_offset < ENVELOPE_BYTES_V0 {
        return Err(WireErrorV0::SectionDirectoryBeforeHeader {
            offset: section_dir_offset,
        });
    }
    validate_alignment("section_dir_offset", section_dir_offset)?;
    let expected_directory_bytes = u64::from(section_count)
        .checked_mul(u64::from(SECTION_DIRECTORY_ENTRY_BYTES_V0))
        .ok_or(WireErrorV0::ArithmeticOverflow {
            field: "section_count * 16",
        })?;
    let section_dir_bytes = read_u32(bytes, 36, "section_dir_bytes")?;
    if u64::from(section_dir_bytes) != expected_directory_bytes {
        return Err(WireErrorV0::SectionDirectoryBytesMismatch {
            declared: section_dir_bytes,
            expected: expected_directory_bytes,
        });
    }
    let directory = checked_wire_range(
        "section directory",
        u64::from(section_dir_offset),
        expected_directory_bytes,
        bytes.len() as u64,
    )?;

    let payload_offset = read_u32(bytes, 40, "payload_offset")?;
    validate_alignment("payload_offset", payload_offset)?;
    let payload_bytes = read_u32(bytes, 44, "payload_bytes")?;
    if payload_bytes > active_limits.max_payload_bytes {
        return Err(WireErrorV0::PayloadTooLarge {
            actual: payload_bytes,
            maximum: active_limits.max_payload_bytes,
        });
    }
    let payload = checked_wire_range(
        "payload",
        u64::from(payload_offset),
        u64::from(payload_bytes),
        bytes.len() as u64,
    )?;

    let mut sections = Vec::new();
    let section_capacity =
        usize::try_from(section_count).map_err(|_| WireErrorV0::ArithmeticOverflow {
            field: "section count usize",
        })?;
    sections
        .try_reserve_exact(section_capacity)
        .map_err(|_| WireErrorV0::AllocationFailed)?;
    let mut previous_kind = None;
    let mut total_records = 0_u64;
    for index in 0..section_count {
        let entry_offset = u64::from(section_dir_offset)
            .checked_add(
                u64::from(index)
                    .checked_mul(u64::from(SECTION_DIRECTORY_ENTRY_BYTES_V0))
                    .ok_or(WireErrorV0::ArithmeticOverflow {
                        field: "section directory index",
                    })?,
            )
            .ok_or(WireErrorV0::ArithmeticOverflow {
                field: "section directory offset",
            })?;
        let entry_offset =
            usize::try_from(entry_offset).map_err(|_| WireErrorV0::ArithmeticOverflow {
                field: "section directory usize",
            })?;
        let raw_section_kind = read_u32(bytes, entry_offset, "section_kind")?;
        let kind =
            SectionKindV0::from_wire(raw_section_kind).ok_or(WireErrorV0::UnknownSectionKind {
                value: raw_section_kind,
            })?;
        if let Some(previous) = previous_kind {
            if kind <= previous {
                return Err(WireErrorV0::SectionsNotStrictlyOrdered {
                    previous,
                    current: kind,
                });
            }
        }
        previous_kind = Some(kind);

        let record_bytes = read_u32(bytes, entry_offset + 4, "record_bytes")?;
        if record_bytes != kind.record_bytes() {
            return Err(WireErrorV0::RecordSizeMismatch {
                section: kind,
                actual: record_bytes,
                expected: kind.record_bytes(),
            });
        }
        let record_count = read_u32(bytes, entry_offset + 8, "record_count")?;
        if kind.requires_one_record() && record_count != 1 {
            return Err(WireErrorV0::RecordCountMismatch {
                section: kind,
                actual: record_count,
                expected: 1,
            });
        }
        total_records = total_records.checked_add(u64::from(record_count)).ok_or(
            WireErrorV0::ArithmeticOverflow {
                field: "total record count",
            },
        )?;
        if total_records > u64::from(active_limits.max_records) {
            return Err(WireErrorV0::TooManyRecords {
                actual: total_records,
                maximum: active_limits.max_records,
            });
        }
        let record_length = u64::from(record_bytes)
            .checked_mul(u64::from(record_count))
            .ok_or(WireErrorV0::ArithmeticOverflow {
                field: "record_bytes * record_count",
            })?;
        let record_offset = read_u32(bytes, entry_offset + 12, "section offset")?;
        validate_alignment("section offset", record_offset)?;
        if record_offset < ENVELOPE_BYTES_V0 {
            return Err(WireErrorV0::RangeOutOfBounds {
                field: "section records",
                start: u64::from(record_offset),
                length: record_length,
                total: bytes.len() as u64,
            });
        }
        let range = checked_wire_range(
            "section records",
            u64::from(record_offset),
            record_length,
            bytes.len() as u64,
        )?;
        sections.push(SectionV0 {
            kind,
            record_bytes,
            record_count,
            range,
        });
    }

    validate_non_overlapping(directory, payload, &sections)?;
    for section in &sections {
        validate_reserved_fields(bytes, *section)?;
    }

    let status = if message_kind.is_response() {
        Some(decode_status(
            bytes,
            payload,
            &sections,
            active_limits.max_diagnostic_bytes,
        )?)
    } else {
        None
    };
    validate_section_set(message_kind, status.map(|value| value.code), &sections)?;

    let invocation_limits = if message_kind.is_response() {
        None
    } else {
        let expected = expectations.invocation_limits;
        let section = sections
            .iter()
            .find(|section| section.kind == SectionKindV0::InvocationLimits)
            .ok_or(WireErrorV0::SectionSetMismatch { message_kind })?;
        let record =
            bytes
                .get(section.range.start..section.range.end)
                .ok_or(WireErrorV0::Truncated {
                    field: "InvocationLimitsV0",
                })?;
        let actual = InvocationLimitsV0::from_record_bytes(record)?;
        validate_invocation_limits(actual, limits)?;
        if actual != expected {
            return Err(WireErrorV0::InvocationLimitsMismatch);
        }
        Some(actual)
    };

    Ok(DecodedMessageV0 {
        bytes,
        envelope: EnvelopeV0 {
            message_kind,
            total_bytes,
            request_id,
            capabilities,
        },
        sections,
        payload,
        status,
        invocation_limits,
    })
}

pub(crate) fn encode_message_v0(
    envelope: EnvelopeMetaV0,
    invocation_limits: InvocationLimitsV0,
    sections: &[WireSectionSourceV0<'_>],
    payload: &[u8],
    limits: WireValidationLimitsV0,
) -> Result<Vec<u8>, WireErrorV0> {
    validate_invocation_limits(invocation_limits, limits)?;
    let expectations = WireExpectationsV0 {
        envelope,
        invocation_limits,
    };
    let active_limits = limits_for_message(expectations, limits);
    let section_count =
        u32::try_from(sections.len()).map_err(|_| WireErrorV0::TooManySections {
            actual: u32::MAX,
            maximum: active_limits.max_sections,
        })?;
    if section_count > active_limits.max_sections {
        return Err(WireErrorV0::TooManySections {
            actual: section_count,
            maximum: active_limits.max_sections,
        });
    }
    let payload_bytes = u32::try_from(payload.len()).map_err(|_| WireErrorV0::PayloadTooLarge {
        actual: u32::MAX,
        maximum: active_limits.max_payload_bytes,
    })?;
    if payload_bytes > active_limits.max_payload_bytes {
        return Err(WireErrorV0::PayloadTooLarge {
            actual: payload_bytes,
            maximum: active_limits.max_payload_bytes,
        });
    }

    let directory_bytes = u64::from(section_count)
        .checked_mul(u64::from(SECTION_DIRECTORY_ENTRY_BYTES_V0))
        .ok_or(WireErrorV0::ArithmeticOverflow {
            field: "section directory bytes",
        })?;
    let mut record_cursor = u64::from(ENVELOPE_BYTES_V0)
        .checked_add(directory_bytes)
        .ok_or(WireErrorV0::ArithmeticOverflow {
            field: "record section start",
        })?;
    let mut record_counts = Vec::new();
    record_counts
        .try_reserve_exact(sections.len())
        .map_err(|_| WireErrorV0::AllocationFailed)?;
    let mut previous = None;
    let mut total_records = 0_u64;
    for section in sections {
        if let Some(previous_kind) = previous {
            if section.kind <= previous_kind {
                return Err(WireErrorV0::SectionsNotStrictlyOrdered {
                    previous: previous_kind,
                    current: section.kind,
                });
            }
        }
        previous = Some(section.kind);
        let record_bytes = section.kind.record_bytes();
        if section.records.len() as u64 % u64::from(record_bytes) != 0 {
            return Err(WireErrorV0::CutRecord {
                section: section.kind,
                bytes: section.records.len() as u64,
                record_bytes,
            });
        }
        let record_count_u64 = section.records.len() as u64 / u64::from(record_bytes);
        let record_count =
            u32::try_from(record_count_u64).map_err(|_| WireErrorV0::TooManyRecords {
                actual: record_count_u64,
                maximum: active_limits.max_records,
            })?;
        if section.kind.requires_one_record() && record_count != 1 {
            return Err(WireErrorV0::RecordCountMismatch {
                section: section.kind,
                actual: record_count,
                expected: 1,
            });
        }
        total_records =
            total_records
                .checked_add(record_count_u64)
                .ok_or(WireErrorV0::ArithmeticOverflow {
                    field: "total record count",
                })?;
        if total_records > u64::from(active_limits.max_records) {
            return Err(WireErrorV0::TooManyRecords {
                actual: total_records,
                maximum: active_limits.max_records,
            });
        }
        validate_source_reserved_fields(*section, record_count)?;
        record_counts.push(record_count);
        record_cursor = record_cursor
            .checked_add(section.records.len() as u64)
            .ok_or(WireErrorV0::ArithmeticOverflow {
                field: "record section end",
            })?;
    }
    let payload_offset = record_cursor;
    let total_bytes = payload_offset.checked_add(payload.len() as u64).ok_or(
        WireErrorV0::ArithmeticOverflow {
            field: "message total bytes",
        },
    )?;
    if total_bytes > u64::from(active_limits.max_message_bytes) {
        return Err(WireErrorV0::MessageTooLarge {
            actual: total_bytes,
            maximum: u64::from(active_limits.max_message_bytes),
        });
    }
    let total_u32 = u32::try_from(total_bytes).map_err(|_| WireErrorV0::MessageTooLarge {
        actual: total_bytes,
        maximum: u64::from(active_limits.max_message_bytes),
    })?;
    let payload_offset_u32 =
        u32::try_from(payload_offset).map_err(|_| WireErrorV0::ArithmeticOverflow {
            field: "payload offset u32",
        })?;
    validate_alignment("canonical payload offset", payload_offset_u32)?;

    let total_usize = usize::try_from(total_bytes).map_err(|_| WireErrorV0::MessageTooLarge {
        actual: total_bytes,
        maximum: u64::from(active_limits.max_message_bytes),
    })?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(total_usize)
        .map_err(|_| WireErrorV0::AllocationFailed)?;
    output.resize(total_usize, 0);

    write_bytes(&mut output, 0, &MAGIC_V0)?;
    write_u16(&mut output, 8, 0)?;
    write_u16(&mut output, 10, 0)?;
    write_u32(&mut output, 12, ENVELOPE_BYTES_V0)?;
    write_u32(&mut output, 16, envelope.message_kind.to_wire())?;
    write_u32(&mut output, 20, envelope.message_kind.flags())?;
    write_u32(&mut output, 24, total_u32)?;
    write_u32(&mut output, 28, section_count)?;
    write_u32(&mut output, 32, ENVELOPE_BYTES_V0)?;
    write_u32(
        &mut output,
        36,
        u32::try_from(directory_bytes).map_err(|_| WireErrorV0::ArithmeticOverflow {
            field: "directory bytes u32",
        })?,
    )?;
    write_u32(&mut output, 40, payload_offset_u32)?;
    write_u32(&mut output, 44, payload_bytes)?;
    write_u64(&mut output, 48, envelope.request_id)?;
    write_u64(&mut output, 56, envelope.capabilities)?;

    let mut section_offset = u64::from(ENVELOPE_BYTES_V0)
        .checked_add(directory_bytes)
        .ok_or(WireErrorV0::ArithmeticOverflow {
            field: "section write start",
        })?;
    for (index, (section, record_count)) in sections.iter().zip(&record_counts).enumerate() {
        let directory_offset = index
            .checked_mul(SECTION_DIRECTORY_ENTRY_BYTES_V0 as usize)
            .and_then(|offset| (ENVELOPE_BYTES_V0 as usize).checked_add(offset))
            .ok_or(WireErrorV0::ArithmeticOverflow {
                field: "directory write offset",
            })?;
        let section_offset_u32 =
            u32::try_from(section_offset).map_err(|_| WireErrorV0::ArithmeticOverflow {
                field: "section offset u32",
            })?;
        validate_alignment("canonical section offset", section_offset_u32)?;
        write_u32(&mut output, directory_offset, section.kind.to_wire())?;
        write_u32(
            &mut output,
            directory_offset + 4,
            section.kind.record_bytes(),
        )?;
        write_u32(&mut output, directory_offset + 8, *record_count)?;
        write_u32(&mut output, directory_offset + 12, section_offset_u32)?;
        write_bytes(
            &mut output,
            usize::try_from(section_offset).map_err(|_| WireErrorV0::ArithmeticOverflow {
                field: "section write offset",
            })?,
            section.records,
        )?;
        section_offset = section_offset
            .checked_add(section.records.len() as u64)
            .ok_or(WireErrorV0::ArithmeticOverflow {
                field: "section write end",
            })?;
    }
    write_bytes(
        &mut output,
        usize::try_from(payload_offset).map_err(|_| WireErrorV0::ArithmeticOverflow {
            field: "payload write offset",
        })?,
        payload,
    )?;

    let _ = decode_message_v0(&output, expectations, limits)?;
    Ok(output)
}

pub(crate) fn status_record_v0(
    code: StatusCodeV0,
    detail_offset: u32,
    detail_length: u32,
) -> [u8; 16] {
    let mut output = [0; 16];
    output[0..4].copy_from_slice(&code.to_wire().to_le_bytes());
    output[4..8].copy_from_slice(&detail_offset.to_le_bytes());
    output[8..12].copy_from_slice(&detail_length.to_le_bytes());
    output
}

fn validate_alignment(field: &'static str, offset: u32) -> Result<(), WireErrorV0> {
    if offset % WIRE_OFFSET_ALIGNMENT_V0 != 0 {
        Err(WireErrorV0::UnalignedOffset { field, offset })
    } else {
        Ok(())
    }
}

fn checked_wire_range(
    field: &'static str,
    start: u64,
    length: u64,
    total: u64,
) -> Result<WireRangeV0, WireErrorV0> {
    let end = start
        .checked_add(length)
        .ok_or(WireErrorV0::ArithmeticOverflow { field })?;
    if end > total {
        return Err(WireErrorV0::RangeOutOfBounds {
            field,
            start,
            length,
            total,
        });
    }
    Ok(WireRangeV0 {
        start: usize::try_from(start).map_err(|_| WireErrorV0::ArithmeticOverflow { field })?,
        end: usize::try_from(end).map_err(|_| WireErrorV0::ArithmeticOverflow { field })?,
    })
}

fn validate_non_overlapping(
    directory: WireRangeV0,
    payload: WireRangeV0,
    sections: &[SectionV0],
) -> Result<(), WireErrorV0> {
    let header = WireRangeV0 {
        start: 0,
        end: ENVELOPE_BYTES_V0 as usize,
    };
    for (name, range) in [("directory", directory), ("payload", payload)] {
        if header.overlaps(range) {
            return Err(WireErrorV0::OverlappingRanges {
                first: "header",
                second: name,
            });
        }
    }
    if directory.overlaps(payload) {
        return Err(WireErrorV0::OverlappingRanges {
            first: "directory",
            second: "payload",
        });
    }
    for (index, section) in sections.iter().enumerate() {
        for (name, range) in [
            ("header", header),
            ("directory", directory),
            ("payload", payload),
        ] {
            if section.range.overlaps(range) {
                return Err(WireErrorV0::OverlappingRanges {
                    first: "section records",
                    second: name,
                });
            }
        }
        for previous in &sections[..index] {
            if section.range.overlaps(previous.range) {
                return Err(WireErrorV0::OverlappingRanges {
                    first: "section records",
                    second: "section records",
                });
            }
        }
    }
    Ok(())
}

fn reserved_fields(kind: SectionKindV0) -> &'static [(u32, u32)] {
    match kind {
        SectionKindV0::Status => &[(12, 4)],
        SectionKindV0::InvocationLimits => &[],
        SectionKindV0::SpacingTableConfig => &[(28, 4)],
        SectionKindV0::SpacingClassRange => &[(20, 4)],
        SectionKindV0::SpacingPairRule => &[(84, 4)],
        SectionKindV0::SpacingBatchContext => &[(36, 4)],
        SectionKindV0::BoundaryAtom => &[(32, 4)],
        SectionKindV0::Boundary => &[],
        SectionKindV0::BoundaryAction => &[(76, 4)],
        SectionKindV0::UnitTableConfig => &[(20, 4)],
        SectionKindV0::UnitDeclaration => &[(44, 8), (52, 4), (56, 4), (60, 4)],
        SectionKindV0::UnitContext => &[(76, 4)],
        SectionKindV0::UnitQuery => &[(12, 4)],
        SectionKindV0::UnitScaleResult => &[],
    }
}

fn validate_reserved_fields(bytes: &[u8], section: SectionV0) -> Result<(), WireErrorV0> {
    let record_bytes = section.record_bytes as usize;
    for record_index in 0..section.record_count {
        let record_index_usize =
            usize::try_from(record_index).map_err(|_| WireErrorV0::ArithmeticOverflow {
                field: "reserved record index usize",
            })?;
        let record_offset = record_index_usize.checked_mul(record_bytes).ok_or(
            WireErrorV0::ArithmeticOverflow {
                field: "reserved record index",
            },
        )?;
        let record_start = section.range.start.checked_add(record_offset).ok_or(
            WireErrorV0::ArithmeticOverflow {
                field: "reserved record offset",
            },
        )?;
        for &(offset, length) in reserved_fields(section.kind) {
            let start = record_start.checked_add(offset as usize).ok_or(
                WireErrorV0::ArithmeticOverflow {
                    field: "reserved field offset",
                },
            )?;
            let end =
                start
                    .checked_add(length as usize)
                    .ok_or(WireErrorV0::ArithmeticOverflow {
                        field: "reserved field end",
                    })?;
            let field = bytes.get(start..end).ok_or(WireErrorV0::Truncated {
                field: "reserved field",
            })?;
            if field.iter().any(|byte| *byte != 0) {
                return Err(WireErrorV0::NonZeroReserved {
                    section: section.kind,
                    record_index,
                    offset_in_record: offset,
                });
            }
        }
    }
    Ok(())
}

fn validate_source_reserved_fields(
    section: WireSectionSourceV0<'_>,
    record_count: u32,
) -> Result<(), WireErrorV0> {
    let record_bytes = section.kind.record_bytes() as usize;
    for record_index in 0..record_count {
        let record_index_usize =
            usize::try_from(record_index).map_err(|_| WireErrorV0::ArithmeticOverflow {
                field: "source reserved record index usize",
            })?;
        let record_start = record_index_usize.checked_mul(record_bytes).ok_or(
            WireErrorV0::ArithmeticOverflow {
                field: "source reserved record index",
            },
        )?;
        for &(offset, length) in reserved_fields(section.kind) {
            let start = record_start.checked_add(offset as usize).ok_or(
                WireErrorV0::ArithmeticOverflow {
                    field: "source reserved field offset",
                },
            )?;
            let end =
                start
                    .checked_add(length as usize)
                    .ok_or(WireErrorV0::ArithmeticOverflow {
                        field: "source reserved field end",
                    })?;
            let field = section
                .records
                .get(start..end)
                .ok_or(WireErrorV0::CutRecord {
                    section: section.kind,
                    bytes: section.records.len() as u64,
                    record_bytes: section.kind.record_bytes(),
                })?;
            if field.iter().any(|byte| *byte != 0) {
                return Err(WireErrorV0::NonZeroReserved {
                    section: section.kind,
                    record_index,
                    offset_in_record: offset,
                });
            }
        }
    }
    Ok(())
}

fn decode_status<'a>(
    bytes: &'a [u8],
    payload: WireRangeV0,
    sections: &[SectionV0],
    max_diagnostic_bytes: u32,
) -> Result<StatusV0<'a>, WireErrorV0> {
    let section = sections
        .iter()
        .find(|section| section.kind == SectionKindV0::Status)
        .ok_or(WireErrorV0::MissingStatus)?;
    let record = bytes
        .get(section.range.start..section.range.end)
        .ok_or(WireErrorV0::Truncated { field: "StatusV0" })?;
    let raw_code = read_u32(record, 0, "status code")?;
    let code =
        StatusCodeV0::from_wire(raw_code).ok_or(WireErrorV0::UnknownStatus { code: raw_code })?;
    let detail_offset = read_u32(record, 4, "status detail offset")?;
    let detail_length = read_u32(record, 8, "status detail length")?;
    if code.is_ok() {
        if detail_offset != 0 || detail_length != 0 {
            return Err(WireErrorV0::SuccessfulStatusHasDetail {
                offset: detail_offset,
                length: detail_length,
            });
        }
        return Ok(StatusV0 { code, detail: "" });
    }
    if detail_length == 0 && detail_offset != 0 {
        return Err(WireErrorV0::EmptyDiagnosticHasOffset {
            offset: detail_offset,
        });
    }
    if detail_length > max_diagnostic_bytes {
        return Err(WireErrorV0::DiagnosticTooLarge {
            actual: detail_length,
            maximum: max_diagnostic_bytes,
        });
    }
    let relative = checked_wire_range(
        "status detail",
        u64::from(detail_offset),
        u64::from(detail_length),
        payload.len() as u64,
    )?;
    let absolute_start =
        payload
            .start
            .checked_add(relative.start)
            .ok_or(WireErrorV0::ArithmeticOverflow {
                field: "status detail absolute start",
            })?;
    let absolute_end =
        payload
            .start
            .checked_add(relative.end)
            .ok_or(WireErrorV0::ArithmeticOverflow {
                field: "status detail absolute end",
            })?;
    let detail_bytes = bytes
        .get(absolute_start..absolute_end)
        .ok_or(WireErrorV0::Truncated {
            field: "status detail",
        })?;
    let detail =
        std::str::from_utf8(detail_bytes).map_err(|error| WireErrorV0::InvalidDiagnosticUtf8 {
            valid_up_to: error.valid_up_to(),
        })?;
    Ok(StatusV0 { code, detail })
}

fn validate_invocation_limits(
    actual: InvocationLimitsV0,
    limits: WireValidationLimitsV0,
) -> Result<(), WireErrorV0> {
    for (field, declared, maximum) in [
        (
            "max_response_bytes",
            actual.max_response_bytes,
            limits.max_response_bytes,
        ),
        ("max_records", actual.max_records, limits.max_records),
        (
            "max_payload_bytes",
            actual.max_payload_bytes,
            limits.max_payload_bytes,
        ),
        (
            "max_diagnostic_bytes",
            actual.max_diagnostic_bytes,
            limits.max_diagnostic_bytes,
        ),
    ] {
        if declared > maximum {
            return Err(WireErrorV0::InvocationLimitExceedsCodec {
                field,
                declared,
                maximum,
            });
        }
    }
    Ok(())
}

fn limits_for_message(
    expectations: WireExpectationsV0,
    hard_limits: WireValidationLimitsV0,
) -> ActiveWireValidationLimitsV0 {
    let lease = expectations.invocation_limits;
    ActiveWireValidationLimitsV0 {
        max_message_bytes: if expectations.envelope.message_kind.is_response() {
            hard_limits
                .max_response_bytes
                .min(lease.max_response_bytes)
        } else {
            hard_limits.max_request_bytes
        },
        max_sections: hard_limits.max_sections,
        max_records: hard_limits.max_records.min(lease.max_records),
        max_payload_bytes: hard_limits.max_payload_bytes.min(lease.max_payload_bytes),
        max_diagnostic_bytes: hard_limits
            .max_diagnostic_bytes
            .min(lease.max_diagnostic_bytes),
    }
}

fn validate_section_set(
    message_kind: MessageKindV0,
    status: Option<StatusCodeV0>,
    sections: &[SectionV0],
) -> Result<(), WireErrorV0> {
    const STATUS_ONLY: &[SectionKindV0] = &[SectionKindV0::Status];
    const SPACING_TABLE_REQUEST: &[SectionKindV0] = &[
        SectionKindV0::InvocationLimits,
        SectionKindV0::SpacingTableConfig,
    ];
    const SPACING_TABLE_RESPONSE: &[SectionKindV0] = &[
        SectionKindV0::Status,
        SectionKindV0::SpacingClassRange,
        SectionKindV0::SpacingPairRule,
    ];
    const SPACING_BATCH_REQUEST: &[SectionKindV0] = &[
        SectionKindV0::InvocationLimits,
        SectionKindV0::SpacingBatchContext,
        SectionKindV0::BoundaryAtom,
        SectionKindV0::Boundary,
    ];
    const SPACING_BATCH_RESPONSE: &[SectionKindV0] = &[
        SectionKindV0::Status,
        SectionKindV0::SpacingBatchContext,
        SectionKindV0::BoundaryAction,
    ];
    const UNIT_TABLE_REQUEST: &[SectionKindV0] = &[
        SectionKindV0::InvocationLimits,
        SectionKindV0::UnitTableConfig,
    ];
    const UNIT_TABLE_RESPONSE: &[SectionKindV0] =
        &[SectionKindV0::Status, SectionKindV0::UnitDeclaration];
    const UNIT_CONTEXT_REQUEST: &[SectionKindV0] = &[
        SectionKindV0::InvocationLimits,
        SectionKindV0::UnitContext,
        SectionKindV0::UnitQuery,
    ];
    const UNIT_CONTEXT_RESPONSE: &[SectionKindV0] = &[
        SectionKindV0::Status,
        SectionKindV0::UnitContext,
        SectionKindV0::UnitScaleResult,
    ];

    let expected = if message_kind.is_response() && !status.is_some_and(StatusCodeV0::is_ok) {
        STATUS_ONLY
    } else {
        match message_kind {
            MessageKindV0::SpacingTableUploadRequest => SPACING_TABLE_REQUEST,
            MessageKindV0::SpacingTableUploadResponse => SPACING_TABLE_RESPONSE,
            MessageKindV0::SpacingBatchRequest => SPACING_BATCH_REQUEST,
            MessageKindV0::SpacingBatchResponse => SPACING_BATCH_RESPONSE,
            MessageKindV0::UnitTableUploadRequest => UNIT_TABLE_REQUEST,
            MessageKindV0::UnitTableUploadResponse => UNIT_TABLE_RESPONSE,
            MessageKindV0::UnitContextBatchRequest => UNIT_CONTEXT_REQUEST,
            MessageKindV0::UnitContextBatchResponse => UNIT_CONTEXT_RESPONSE,
        }
    };
    if sections.len() != expected.len()
        || sections
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual.kind != *expected)
    {
        return Err(WireErrorV0::SectionSetMismatch { message_kind });
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize, field: &'static str) -> Result<u16, WireErrorV0> {
    let end = offset
        .checked_add(2)
        .ok_or(WireErrorV0::ArithmeticOverflow { field })?;
    let source = bytes
        .get(offset..end)
        .ok_or(WireErrorV0::Truncated { field })?;
    let mut word = [0; 2];
    word.copy_from_slice(source);
    Ok(u16::from_le_bytes(word))
}

fn read_u32(bytes: &[u8], offset: usize, field: &'static str) -> Result<u32, WireErrorV0> {
    let end = offset
        .checked_add(4)
        .ok_or(WireErrorV0::ArithmeticOverflow { field })?;
    let source = bytes
        .get(offset..end)
        .ok_or(WireErrorV0::Truncated { field })?;
    let mut word = [0; 4];
    word.copy_from_slice(source);
    Ok(u32::from_le_bytes(word))
}

fn read_u64(bytes: &[u8], offset: usize, field: &'static str) -> Result<u64, WireErrorV0> {
    let end = offset
        .checked_add(8)
        .ok_or(WireErrorV0::ArithmeticOverflow { field })?;
    let source = bytes
        .get(offset..end)
        .ok_or(WireErrorV0::Truncated { field })?;
    let mut word = [0; 8];
    word.copy_from_slice(source);
    Ok(u64::from_le_bytes(word))
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) -> Result<(), WireErrorV0> {
    write_bytes(bytes, offset, &value.to_le_bytes())
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), WireErrorV0> {
    write_bytes(bytes, offset, &value.to_le_bytes())
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), WireErrorV0> {
    write_bytes(bytes, offset, &value.to_le_bytes())
}

fn write_bytes(bytes: &mut [u8], offset: usize, source: &[u8]) -> Result<(), WireErrorV0> {
    let end = offset
        .checked_add(source.len())
        .ok_or(WireErrorV0::ArithmeticOverflow {
            field: "write range",
        })?;
    let target = bytes.get_mut(offset..end).ok_or(WireErrorV0::Truncated {
        field: "write target",
    })?;
    target.copy_from_slice(source);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codec_limits() -> WireValidationLimitsV0 {
        WireValidationLimitsV0 {
            max_request_bytes: 16 * 1024,
            max_response_bytes: 16 * 1024,
            max_sections: 8,
            max_records: 128,
            max_payload_bytes: 8 * 1024,
            max_diagnostic_bytes: 1024,
        }
    }

    fn invocation_limits() -> InvocationLimitsV0 {
        InvocationLimitsV0 {
            max_response_bytes: 4096,
            max_records: 16,
            max_payload_bytes: 1024,
            max_diagnostic_bytes: 128,
            fuel_model_id: 7,
            call_ordinal: 9,
            fuel_limit: 0x0102_0304_0506_0708,
        }
    }

    fn request_meta() -> EnvelopeMetaV0 {
        EnvelopeMetaV0 {
            message_kind: MessageKindV0::SpacingTableUploadRequest,
            request_id: 0x0102_0304_0506_0708,
            capabilities: 1,
        }
    }

    fn golden_request() -> Vec<u8> {
        let invocation = invocation_limits().to_record_bytes();
        let config = [0_u8; 32];
        encode_message_v0(
            request_meta(),
            invocation_limits(),
            &[
                WireSectionSourceV0 {
                    kind: SectionKindV0::InvocationLimits,
                    records: &invocation,
                },
                WireSectionSourceV0 {
                    kind: SectionKindV0::SpacingTableConfig,
                    records: &config,
                },
            ],
            &[],
            codec_limits(),
        )
        .unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(DIGITS[(byte >> 4) as usize] as char);
            output.push(DIGITS[(byte & 0x0f) as usize] as char);
        }
        output
    }

    #[test]
    fn envelopeとsectionと整数のlittle_endianをgolden_vectorで固定する() {
        let bytes = golden_request();
        assert_eq!(
            hex(&bytes),
            concat!(
                "50525458573000000000000040000000",
                "0100000000000000a000000002000000",
                "4000000020000000a000000000000000",
                "08070605040302010100000000000000",
                "02000000200000000100000060000000",
                "01100000200000000100000080000000",
                "00100000100000000004000080000000",
                "07000000090000000807060504030201",
                "00000000000000000000000000000000",
                "00000000000000000000000000000000",
            )
        );
        let decoded = decode_message_v0(
            &bytes,
            WireExpectationsV0::request(
                request_meta().message_kind,
                request_meta().request_id,
                request_meta().capabilities,
                invocation_limits(),
            ),
            codec_limits(),
        )
        .unwrap();
        assert_eq!(decoded.envelope.total_bytes, 160);
        assert_eq!(decoded.invocation_limits(), Some(invocation_limits()));
        assert_eq!(decoded.payload(), b"");
    }

    #[test]
    fn encode後のdecodeと再encodeは同じcanonical_byte列になる() {
        let bytes = golden_request();
        let decoded = decode_message_v0(
            &bytes,
            WireExpectationsV0::request(
                request_meta().message_kind,
                request_meta().request_id,
                request_meta().capabilities,
                invocation_limits(),
            ),
            codec_limits(),
        )
        .unwrap();
        let sources: Vec<_> = decoded
            .sections()
            .iter()
            .map(|section| WireSectionSourceV0 {
                kind: section.kind,
                records: decoded.section_records(section.kind).unwrap(),
            })
            .collect();
        let encoded = encode_message_v0(
            request_meta(),
            invocation_limits(),
            &sources,
            decoded.payload(),
            codec_limits(),
        )
        .unwrap();
        assert_eq!(encoded, bytes);
    }

    #[test]
    fn big_endianに見えるheader長を受理しない() {
        let mut bytes = golden_request();
        bytes[12..16].copy_from_slice(&64_u32.to_be_bytes());
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::request(
                    request_meta().message_kind,
                    request_meta().request_id,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::InvalidHeaderBytes { .. })
        ));
    }

    #[test]
    fn sectionの重複と降順と未知kindを拒否する() {
        let expectations = WireExpectationsV0::request(
            request_meta().message_kind,
            request_meta().request_id,
            request_meta().capabilities,
            invocation_limits(),
        );
        let mut duplicate = golden_request();
        duplicate[80..84].copy_from_slice(&SectionKindV0::InvocationLimits.to_wire().to_le_bytes());
        assert!(matches!(
            decode_message_v0(&duplicate, expectations, codec_limits()),
            Err(WireErrorV0::SectionsNotStrictlyOrdered { .. })
        ));

        let mut descending = golden_request();
        descending[64..68]
            .copy_from_slice(&SectionKindV0::SpacingTableConfig.to_wire().to_le_bytes());
        descending[80..84]
            .copy_from_slice(&SectionKindV0::InvocationLimits.to_wire().to_le_bytes());
        assert!(matches!(
            decode_message_v0(&descending, expectations, codec_limits()),
            Err(WireErrorV0::SectionsNotStrictlyOrdered { .. })
        ));

        let mut unknown = golden_request();
        unknown[80..84].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
        assert!(matches!(
            decode_message_v0(&unknown, expectations, codec_limits()),
            Err(WireErrorV0::UnknownSectionKind { .. })
        ));
    }

    #[test]
    fn 未整列offsetとrecord領域の重複を拒否する() {
        let expectations = WireExpectationsV0::request(
            request_meta().message_kind,
            request_meta().request_id,
            request_meta().capabilities,
            invocation_limits(),
        );
        let mut unaligned = golden_request();
        unaligned[76..80].copy_from_slice(&97_u32.to_le_bytes());
        assert!(matches!(
            decode_message_v0(&unaligned, expectations, codec_limits()),
            Err(WireErrorV0::UnalignedOffset { .. })
        ));

        let mut overlap = golden_request();
        overlap[92..96].copy_from_slice(&96_u32.to_le_bytes());
        assert!(matches!(
            decode_message_v0(&overlap, expectations, codec_limits()),
            Err(WireErrorV0::OverlappingRanges { .. })
        ));
    }

    #[test]
    fn 切れたrecordとreserved非零をencode前に拒否する() {
        let invocation = invocation_limits().to_record_bytes();
        let cut = [0_u8; 31];
        assert!(matches!(
            encode_message_v0(
                request_meta(),
                invocation_limits(),
                &[
                    WireSectionSourceV0 {
                        kind: SectionKindV0::InvocationLimits,
                        records: &invocation,
                    },
                    WireSectionSourceV0 {
                        kind: SectionKindV0::SpacingTableConfig,
                        records: &cut,
                    },
                ],
                &[],
                codec_limits(),
            ),
            Err(WireErrorV0::CutRecord { .. })
        ));

        let mut config = [0_u8; 32];
        config[28] = 1;
        assert!(matches!(
            encode_message_v0(
                request_meta(),
                invocation_limits(),
                &[
                    WireSectionSourceV0 {
                        kind: SectionKindV0::InvocationLimits,
                        records: &invocation,
                    },
                    WireSectionSourceV0 {
                        kind: SectionKindV0::SpacingTableConfig,
                        records: &config,
                    },
                ],
                &[],
                codec_limits(),
            ),
            Err(WireErrorV0::NonZeroReserved { .. })
        ));
    }

    fn failure_response(detail: &[u8]) -> Vec<u8> {
        let status = status_record_v0(
            StatusCodeV0::ProviderFailure,
            0,
            u32::try_from(detail.len()).unwrap(),
        );
        encode_message_v0(
            EnvelopeMetaV0 {
                message_kind: MessageKindV0::SpacingTableUploadResponse,
                request_id: request_meta().request_id,
                capabilities: request_meta().capabilities,
            },
            invocation_limits(),
            &[WireSectionSourceV0 {
                kind: SectionKindV0::Status,
                records: &status,
            }],
            detail,
            codec_limits(),
        )
        .unwrap()
    }

    fn success_unit_response() -> Vec<u8> {
        let status = status_record_v0(StatusCodeV0::Ok, 0, 0);
        encode_message_v0(
            EnvelopeMetaV0 {
                message_kind: MessageKindV0::UnitTableUploadResponse,
                request_id: request_meta().request_id,
                capabilities: 4,
            },
            invocation_limits(),
            &[
                WireSectionSourceV0 {
                    kind: SectionKindV0::Status,
                    records: &status,
                },
                WireSectionSourceV0 {
                    kind: SectionKindV0::UnitDeclaration,
                    records: &[],
                },
            ],
            &[],
            codec_limits(),
        )
        .unwrap()
    }

    #[test]
    fn 失敗diagnosticはpayload内のstrict_utf8だけを受理する() {
        let bytes = failure_response("失敗".as_bytes());
        let decoded = decode_message_v0(
            &bytes,
            WireExpectationsV0::response(
                MessageKindV0::SpacingTableUploadResponse,
                request_meta().request_id,
                request_meta().capabilities,
                invocation_limits(),
            ),
            codec_limits(),
        )
        .unwrap();
        assert_eq!(decoded.status().unwrap().detail, "失敗");

        let mut malformed = failure_response(b"ok");
        let payload_offset = read_u32(&malformed, 40, "payload offset").unwrap() as usize;
        malformed[payload_offset] = 0xc3;
        malformed[payload_offset + 1] = 0x28;
        assert!(matches!(
            decode_message_v0(
                &malformed,
                WireExpectationsV0::response(
                    MessageKindV0::SpacingTableUploadResponse,
                    request_meta().request_id,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::InvalidDiagnosticUtf8 { .. })
        ));
    }

    #[test]
    fn 成功statusのdetailと未知statusを拒否する() {
        let ok = status_record_v0(StatusCodeV0::Ok, 0, 1);
        assert!(matches!(
            encode_message_v0(
                EnvelopeMetaV0 {
                    message_kind: MessageKindV0::UnitTableUploadResponse,
                    request_id: 1,
                    capabilities: 4,
                },
                invocation_limits(),
                &[
                    WireSectionSourceV0 {
                        kind: SectionKindV0::Status,
                        records: &ok,
                    },
                    WireSectionSourceV0 {
                        kind: SectionKindV0::UnitDeclaration,
                        records: &[],
                    },
                ],
                b"x",
                codec_limits(),
            ),
            Err(WireErrorV0::SuccessfulStatusHasDetail { .. })
        ));

        let mut bytes = failure_response(b"x");
        let status_offset = read_u32(&bytes, 76, "status offset").unwrap() as usize;
        bytes[status_offset..status_offset + 4].copy_from_slice(&99_u32.to_le_bytes());
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::response(
                    MessageKindV0::SpacingTableUploadResponse,
                    request_meta().request_id,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::UnknownStatus { code: 99 })
        ));
    }

    #[test]
    fn 成功と失敗のsection集合は欠落も余分も許さない() {
        let invocation = invocation_limits().to_record_bytes();
        assert!(matches!(
            encode_message_v0(
                request_meta(),
                invocation_limits(),
                &[WireSectionSourceV0 {
                    kind: SectionKindV0::InvocationLimits,
                    records: &invocation,
                }],
                &[],
                codec_limits(),
            ),
            Err(WireErrorV0::SectionSetMismatch { .. })
        ));

        let mut extra_on_failure = success_unit_response();
        let status_offset = read_u32(&extra_on_failure, 76, "status offset").unwrap() as usize;
        extra_on_failure[status_offset..status_offset + 4]
            .copy_from_slice(&StatusCodeV0::ProviderFailure.to_wire().to_le_bytes());
        assert!(matches!(
            decode_message_v0(
                &extra_on_failure,
                WireExpectationsV0::response(
                    MessageKindV0::UnitTableUploadResponse,
                    request_meta().request_id,
                    4,
                    invocation_limits(),
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::SectionSetMismatch { .. })
        ));
    }

    #[test]
    fn directory長とrecord範囲と総長の虚偽を拒否する() {
        let expectations = WireExpectationsV0::request(
            request_meta().message_kind,
            request_meta().request_id,
            request_meta().capabilities,
            invocation_limits(),
        );
        let mut wrong_directory = golden_request();
        wrong_directory[36..40].copy_from_slice(&16_u32.to_le_bytes());
        assert!(matches!(
            decode_message_v0(&wrong_directory, expectations, codec_limits()),
            Err(WireErrorV0::SectionDirectoryBytesMismatch { .. })
        ));

        let mut outside = golden_request();
        outside[92..96].copy_from_slice(&156_u32.to_le_bytes());
        assert!(matches!(
            decode_message_v0(&outside, expectations, codec_limits()),
            Err(WireErrorV0::RangeOutOfBounds { .. })
        ));

        let mut wrong_total = golden_request();
        wrong_total[24..28].copy_from_slice(&159_u32.to_le_bytes());
        assert!(matches!(
            decode_message_v0(&wrong_total, expectations, codec_limits()),
            Err(WireErrorV0::TotalLengthMismatch { .. })
        ));
    }

    #[test]
    fn decoderもreserved非零をpublish前に拒否する() {
        let mut bytes = golden_request();
        bytes[156] = 1;
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::request(
                    request_meta().message_kind,
                    request_meta().request_id,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::NonZeroReserved {
                section: SectionKindV0::SpacingTableConfig,
                ..
            })
        ));
    }

    #[test]
    fn diagnosticとresponseのbyte上限を越えない() {
        let expectations = WireExpectationsV0::response(
            MessageKindV0::SpacingTableUploadResponse,
            request_meta().request_id,
            request_meta().capabilities,
            invocation_limits(),
        );
        let mut too_long = failure_response(b"ok");
        let status_offset = read_u32(&too_long, 76, "status offset").unwrap() as usize;
        too_long[status_offset + 8..status_offset + 12].copy_from_slice(&2048_u32.to_le_bytes());
        assert!(matches!(
            decode_message_v0(&too_long, expectations, codec_limits()),
            Err(WireErrorV0::DiagnosticTooLarge { .. })
        ));

        let mut outside_payload = failure_response(b"ok");
        outside_payload[status_offset + 8..status_offset + 12]
            .copy_from_slice(&100_u32.to_le_bytes());
        assert!(matches!(
            decode_message_v0(&outside_payload, expectations, codec_limits()),
            Err(WireErrorV0::RangeOutOfBounds {
                field: "status detail",
                ..
            })
        ));

        let bytes = golden_request();
        let mut small = codec_limits();
        small.max_request_bytes = 159;
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::request(
                    request_meta().message_kind,
                    request_meta().request_id,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                small,
            ),
            Err(WireErrorV0::MessageTooLarge { .. })
        ));
    }

    #[test]
    fn requestとresponseのbyte上限を別々に適用する() {
        let request = golden_request();
        let mut request_limited = codec_limits();
        request_limited.max_request_bytes = 159;
        assert!(matches!(
            decode_message_v0(
                &request,
                WireExpectationsV0::request(
                    request_meta().message_kind,
                    request_meta().request_id,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                request_limited,
            ),
            Err(WireErrorV0::MessageTooLarge {
                maximum: 159,
                ..
            })
        ));

        let response = failure_response(b"response exceeds its independent cap");
        let mut response_limited = codec_limits();
        response_limited.max_response_bytes = 100;
        let mut response_lease = invocation_limits();
        response_lease.max_response_bytes = 100;
        assert!(matches!(
            decode_message_v0(
                &response,
                WireExpectationsV0::response(
                    MessageKindV0::SpacingTableUploadResponse,
                    request_meta().request_id,
                    request_meta().capabilities,
                    response_lease,
                ),
                response_limited,
            ),
            Err(WireErrorV0::MessageTooLarge {
                maximum: 100,
                ..
            })
        ));

        let mut lease_exceeds_response_policy = codec_limits();
        lease_exceeds_response_policy.max_response_bytes = 4095;
        assert!(matches!(
            decode_message_v0(
                &request,
                WireExpectationsV0::request(
                    request_meta().message_kind,
                    request_meta().request_id,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                lease_exceeds_response_policy,
            ),
            Err(WireErrorV0::InvocationLimitExceedsCodec {
                field: "max_response_bytes",
                declared: 4096,
                maximum: 4095,
            })
        ));
    }

    #[test]
    fn requestのlimitを同じleaseとcodec上限へ束縛する() {
        let bytes = golden_request();
        let mut different = invocation_limits();
        different.call_ordinal += 1;
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::request(
                    request_meta().message_kind,
                    request_meta().request_id,
                    request_meta().capabilities,
                    different,
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::InvocationLimitsMismatch)
        ));

        let mut small = codec_limits();
        small.max_records = 8;
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::request(
                    request_meta().message_kind,
                    request_meta().request_id,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                small,
            ),
            Err(WireErrorV0::InvocationLimitExceedsCodec {
                field: "max_records",
                ..
            })
        ));
    }

    #[test]
    fn request_idとcapabilityとresponse_kindの差替えを拒否する() {
        let bytes = failure_response(b"");
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::response(
                    MessageKindV0::SpacingTableUploadResponse,
                    request_meta().request_id + 1,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::UnexpectedRequestId { .. })
        ));
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::response(
                    MessageKindV0::SpacingTableUploadResponse,
                    request_meta().request_id,
                    request_meta().capabilities << 1,
                    invocation_limits(),
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::UnexpectedCapabilities { .. })
        ));
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::response(
                    MessageKindV0::SpacingBatchResponse,
                    request_meta().request_id,
                    request_meta().capabilities,
                    invocation_limits(),
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::UnexpectedMessageKind { .. })
        ));
    }

    #[test]
    fn responseはrequest時と同じlease上限へ束縛する() {
        let bytes = failure_response(b"ok");
        let mut lease = invocation_limits();
        lease.max_response_bytes = u32::try_from(bytes.len() - 1).unwrap();
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::response(
                    MessageKindV0::SpacingTableUploadResponse,
                    request_meta().request_id,
                    request_meta().capabilities,
                    lease,
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::MessageTooLarge { .. })
        ));

        let mut lease = invocation_limits();
        lease.max_records = 0;
        assert!(matches!(
            decode_message_v0(
                &bytes,
                WireExpectationsV0::response(
                    MessageKindV0::SpacingTableUploadResponse,
                    request_meta().request_id,
                    request_meta().capabilities,
                    lease,
                ),
                codec_limits(),
            ),
            Err(WireErrorV0::TooManyRecords { maximum: 0, .. })
        ));
    }

    fn packed_transport_result(status: u32, length: u32) -> i64 {
        ((u64::from(status) << 32) | u64::from(length)) as i64
    }

    #[test]
    fn transport_status非零ではresponse長を公開しない() {
        assert!(matches!(
            validate_transport_result_v0(packed_transport_result(1, 64), 128, 128),
            Err(WireErrorV0::NonZeroTransportStatus { status: 1 })
        ));
        assert!(matches!(
            validate_transport_result_v0(packed_transport_result(u32::MAX, 64), 128, 128),
            Err(WireErrorV0::NonZeroTransportStatus { status: u32::MAX })
        ));
    }

    #[test]
    fn response返値はheaderとcapacityとleaseの内側だけを許す() {
        assert!(matches!(
            validate_transport_result_v0(packed_transport_result(0, 63), 128, 128),
            Err(WireErrorV0::ResponseLengthTooSmall { actual: 63 })
        ));
        assert!(matches!(
            validate_transport_result_v0(packed_transport_result(0, 129), 128, 256),
            Err(WireErrorV0::ResponseLengthExceedsCapacity {
                actual: 129,
                capacity: 128,
            })
        ));
        assert!(matches!(
            validate_transport_result_v0(packed_transport_result(0, 129), 256, 128),
            Err(WireErrorV0::ResponseLengthExceedsLease {
                actual: 129,
                maximum: 128,
            })
        ));
        assert_eq!(
            validate_transport_result_v0(packed_transport_result(0, 64), 128, 128),
            Ok(64)
        );
    }

    #[test]
    fn mailboxは加算overflowとmemory外と相互重複を拒否する() {
        let limits = MailboxLimitsV0 {
            max_request_bytes: 4096,
            max_response_bytes: 4096,
        };
        assert!(matches!(
            validate_mailboxes_v0(
                u64::from(u32::MAX),
                MailboxRangeV0 {
                    base: u32::MAX - 31,
                    capacity: 64,
                },
                MailboxRangeV0 {
                    base: 0,
                    capacity: 64,
                },
                limits,
            ),
            Err(WireErrorV0::MailboxRangeOverflow { .. })
        ));
        assert!(matches!(
            validate_mailboxes_v0(
                200,
                MailboxRangeV0 {
                    base: 160,
                    capacity: 64,
                },
                MailboxRangeV0 {
                    base: 0,
                    capacity: 64,
                },
                limits,
            ),
            Err(WireErrorV0::MailboxOutsideMemory { .. })
        ));
        assert!(matches!(
            validate_mailboxes_v0(
                1024,
                MailboxRangeV0 {
                    base: 100,
                    capacity: 128,
                },
                MailboxRangeV0 {
                    base: 200,
                    capacity: 128,
                },
                limits,
            ),
            Err(WireErrorV0::MailboxesOverlap)
        ));
        assert!(validate_mailboxes_v0(
            1024,
            MailboxRangeV0 {
                base: 64,
                capacity: 128,
            },
            MailboxRangeV0 {
                base: 192,
                capacity: 256,
            },
            limits,
        )
        .is_ok());
    }

    #[test]
    fn すべての切断位置と任意byte列でpanicしない() {
        let bytes = golden_request();
        let expectations = WireExpectationsV0::request(
            request_meta().message_kind,
            request_meta().request_id,
            request_meta().capabilities,
            invocation_limits(),
        );
        for length in 0..bytes.len() {
            let result = std::panic::catch_unwind(|| {
                decode_message_v0(&bytes[..length], expectations, codec_limits())
            });
            assert!(result.is_ok(), "prefix {length} panicked");
            assert!(result.unwrap().is_err());
        }

        let mut state = 0x1234_5678_u32;
        for length in 0..256_usize {
            let mut arbitrary = vec![0_u8; length];
            for byte in &mut arbitrary {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *byte = state as u8;
            }
            assert!(std::panic::catch_unwind(|| {
                let _ = decode_message_v0(&arbitrary, expectations, codec_limits());
            })
            .is_ok());
        }
    }
}

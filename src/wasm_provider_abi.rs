//! PraTeX WASM provider ABI 0.0 のversion・能力交渉。
//!
//! runtime固有のmodule型やRustのenum配置を外向きABIにしない。このmoduleは
//! `docs/wasm-provider-abi-v0.md`で確定している固定幅整数だけを受け取り、WASMを
//! instantiateする前にhost policyとの共通部分を一度だけ確定する。
//!
//! 標準日本語組版はこの交渉を呼ばない。JFM、和欧間空白、禁則はengine coreだけで
//! 完結し、Vaak/WASM callback数を0に保つ。

#![forbid(unsafe_code)]

/// `(major << 16) | minor` でwireへ載せるWASM ABI version。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct WasmAbiVersion(u32);

impl WasmAbiVersion {
    pub(crate) const ZERO_ZERO: Self = Self::new(0, 0);

    pub(crate) const fn new(major: u16, minor: u16) -> Self {
        Self((major as u32) << 16 | minor as u32)
    }

    pub(crate) const fn from_wire(value: u32) -> Self {
        Self(value)
    }

    pub(crate) const fn to_wire(self) -> u32 {
        self.0
    }

    pub(crate) const fn major(self) -> u16 {
        (self.0 >> 16) as u16
    }

    pub(crate) const fn minor(self) -> u16 {
        self.0 as u16
    }
}

/// ABI 0.0でmoduleが要求・任意指定できる能力bit。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WasmCapabilitySet(u64);

impl WasmCapabilitySet {
    pub(crate) const REGISTER_SPACING_TABLE: Self = Self(1 << 0);
    pub(crate) const PROPOSE_SPACING_BATCH: Self = Self(1 << 1);
    pub(crate) const REGISTER_UNIT_TABLE: Self = Self(1 << 2);
    pub(crate) const RESOLVE_UNIT_CONTEXT_BATCH: Self = Self(1 << 3);
    pub(crate) const KNOWN_V0: Self = Self(
        Self::REGISTER_SPACING_TABLE.0
            | Self::PROPOSE_SPACING_BATCH.0
            | Self::REGISTER_UNIT_TABLE.0
            | Self::RESOLVE_UNIT_CONTEXT_BATCH.0,
    );
    pub(crate) const EMPTY: Self = Self(0);

    pub(crate) const fn from_wire(bits: u64) -> Self {
        Self(bits)
    }

    pub(crate) const fn to_wire(self) -> u64 {
        self.0
    }

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    const fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// `pratex_wasm_invoke_v0`のoperation ID集合。
///
/// Rust enumのdiscriminantは公開せず、wireで確定した1--4だけをbit集合へ写す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WasmOperationSet(u8);

impl WasmOperationSet {
    pub(crate) const SPACING_TABLE_UPLOAD: Self = Self(1 << 0);
    pub(crate) const SPACING_BATCH: Self = Self(1 << 1);
    pub(crate) const UNIT_TABLE_UPLOAD: Self = Self(1 << 2);
    pub(crate) const UNIT_CONTEXT_BATCH: Self = Self(1 << 3);
    pub(crate) const KNOWN_V0: Self = Self(
        Self::SPACING_TABLE_UPLOAD.0
            | Self::SPACING_BATCH.0
            | Self::UNIT_TABLE_UPLOAD.0
            | Self::UNIT_CONTEXT_BATCH.0,
    );

    pub(crate) const fn from_wire_bits(bits: u8) -> Self {
        Self(bits)
    }

    pub(crate) const fn to_wire_bits(self) -> u8 {
        self.0
    }

    const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// immutable export globalから読み取った、runtime非依存のmodule宣言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WasmModuleAbiDeclaration {
    pub(crate) minimum_version: WasmAbiVersion,
    pub(crate) maximum_version: WasmAbiVersion,
    pub(crate) required_features: u64,
    pub(crate) optional_features: u64,
    pub(crate) required_capabilities: WasmCapabilitySet,
    pub(crate) optional_capabilities: WasmCapabilitySet,
}

/// RunEpoch policyがこのmoduleへ許可した範囲。
///
/// `InvokeWasm`自体の外側の承認、module hash、limits、fuel、failure policyはこの値を
/// 作るcallerが先に固定する。この型から権限を自己生成するTeX/Vaak surfaceは設けない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WasmProviderApproval {
    pub(crate) allowed_capabilities: WasmCapabilitySet,
    pub(crate) allowed_operations: WasmOperationSet,
}

/// instantiate前に確定し、leaseへ値で束縛する交渉結果。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NegotiatedWasmProviderAbi {
    selected_version: WasmAbiVersion,
    granted_features: u64,
    granted_capabilities: WasmCapabilitySet,
    operations: WasmOperationSet,
}

impl NegotiatedWasmProviderAbi {
    pub(crate) const fn selected_version(&self) -> WasmAbiVersion {
        self.selected_version
    }

    pub(crate) const fn granted_features(&self) -> u64 {
        self.granted_features
    }

    pub(crate) const fn granted_capabilities(&self) -> WasmCapabilitySet {
        self.granted_capabilities
    }

    pub(crate) const fn operations(&self) -> WasmOperationSet {
        self.operations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WasmAbiNegotiationError {
    InvalidAbiRange {
        minimum: WasmAbiVersion,
        maximum: WasmAbiVersion,
    },
    AbiMismatch {
        minimum: WasmAbiVersion,
        maximum: WasmAbiVersion,
    },
    UnknownRequiredFeature {
        bits: u64,
    },
    UnknownRequiredCapability {
        bits: u64,
    },
    CapabilityDenied {
        bits: u64,
    },
    UnknownOperation {
        bits: u8,
    },
    OperationCapabilityMissing {
        operation_bit: u8,
        capability_bit: u64,
    },
}

impl WasmAbiNegotiationError {
    /// 機械可読診断へ使う安定code。自由文messageとは分離する。
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::InvalidAbiRange { .. } | Self::AbiMismatch { .. } => "AbiMismatch",
            Self::UnknownRequiredFeature { .. } => "UnknownRequiredFeature",
            Self::UnknownRequiredCapability { .. } => "UnknownRequiredCapability",
            Self::CapabilityDenied { .. } | Self::OperationCapabilityMissing { .. } => {
                "CapabilityDenied"
            }
            Self::UnknownOperation { .. } => "InvalidModule",
        }
    }
}

/// ABI 0.0との交差とpolicy grantを、実行前に決定的な順序で検査する。
pub(crate) fn negotiate_v0(
    module: WasmModuleAbiDeclaration,
    approval: WasmProviderApproval,
) -> Result<NegotiatedWasmProviderAbi, WasmAbiNegotiationError> {
    if module.minimum_version > module.maximum_version {
        return Err(WasmAbiNegotiationError::InvalidAbiRange {
            minimum: module.minimum_version,
            maximum: module.maximum_version,
        });
    }
    if !(module.minimum_version..=module.maximum_version).contains(&WasmAbiVersion::ZERO_ZERO) {
        return Err(WasmAbiNegotiationError::AbiMismatch {
            minimum: module.minimum_version,
            maximum: module.maximum_version,
        });
    }

    // ABI 0.0のhost feature集合は空である。optional bitも一つもgrantしない。
    if module.required_features != 0 {
        return Err(WasmAbiNegotiationError::UnknownRequiredFeature {
            bits: module.required_features,
        });
    }

    let unknown_required = module
        .required_capabilities
        .difference(WasmCapabilitySet::KNOWN_V0);
    if !unknown_required.is_empty() {
        return Err(WasmAbiNegotiationError::UnknownRequiredCapability {
            bits: unknown_required.to_wire(),
        });
    }
    let denied_required = module
        .required_capabilities
        .difference(approval.allowed_capabilities);
    if !denied_required.is_empty() {
        return Err(WasmAbiNegotiationError::CapabilityDenied {
            bits: denied_required.to_wire(),
        });
    }

    let unknown_operations = approval
        .allowed_operations
        .difference(WasmOperationSet::KNOWN_V0);
    if unknown_operations.to_wire_bits() != 0 {
        return Err(WasmAbiNegotiationError::UnknownOperation {
            bits: unknown_operations.to_wire_bits(),
        });
    }

    let optional_capabilities = module
        .optional_capabilities
        .intersection(WasmCapabilitySet::KNOWN_V0)
        .intersection(approval.allowed_capabilities);
    let granted_capabilities = module.required_capabilities.union(optional_capabilities);

    for (operation, capability) in [
        (
            WasmOperationSet::SPACING_TABLE_UPLOAD,
            WasmCapabilitySet::REGISTER_SPACING_TABLE,
        ),
        (
            WasmOperationSet::SPACING_BATCH,
            WasmCapabilitySet::PROPOSE_SPACING_BATCH,
        ),
        (
            WasmOperationSet::UNIT_TABLE_UPLOAD,
            WasmCapabilitySet::REGISTER_UNIT_TABLE,
        ),
        (
            WasmOperationSet::UNIT_CONTEXT_BATCH,
            WasmCapabilitySet::RESOLVE_UNIT_CONTEXT_BATCH,
        ),
    ] {
        if approval.allowed_operations.contains(operation)
            && !granted_capabilities.contains_all(capability)
        {
            return Err(WasmAbiNegotiationError::OperationCapabilityMissing {
                operation_bit: operation.to_wire_bits(),
                capability_bit: capability.to_wire(),
            });
        }
    }

    Ok(NegotiatedWasmProviderAbi {
        selected_version: WasmAbiVersion::ZERO_ZERO,
        granted_features: module.optional_features & 0,
        granted_capabilities,
        operations: approval.allowed_operations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(required: WasmCapabilitySet) -> WasmModuleAbiDeclaration {
        WasmModuleAbiDeclaration {
            minimum_version: WasmAbiVersion::ZERO_ZERO,
            maximum_version: WasmAbiVersion::ZERO_ZERO,
            required_features: 0,
            optional_features: 0,
            required_capabilities: required,
            optional_capabilities: WasmCapabilitySet::EMPTY,
        }
    }

    fn approval(
        capabilities: WasmCapabilitySet,
        operations: WasmOperationSet,
    ) -> WasmProviderApproval {
        WasmProviderApproval {
            allowed_capabilities: capabilities,
            allowed_operations: operations,
        }
    }

    #[test]
    fn abi版は上位十六bitと下位十六bitを往復する() {
        let version = WasmAbiVersion::new(0x1234, 0xabcd);
        assert_eq!(version.to_wire(), 0x1234_abcd);
        assert_eq!(version.major(), 0x1234);
        assert_eq!(version.minor(), 0xabcd);
        assert_eq!(WasmAbiVersion::from_wire(version.to_wire()), version);
    }

    #[test]
    fn abi零点零との交差がないmoduleを拒否する() {
        let mut declaration = module(WasmCapabilitySet::EMPTY);
        declaration.minimum_version = WasmAbiVersion::new(0, 1);
        declaration.maximum_version = WasmAbiVersion::new(1, 0);
        let error = negotiate_v0(
            declaration,
            approval(
                WasmCapabilitySet::EMPTY,
                WasmOperationSet::from_wire_bits(0),
            ),
        )
        .unwrap_err();
        assert_eq!(error.code(), "AbiMismatch");
    }

    #[test]
    fn 逆転したabi範囲を交差として扱わない() {
        let mut declaration = module(WasmCapabilitySet::EMPTY);
        declaration.minimum_version = WasmAbiVersion::new(1, 0);
        declaration.maximum_version = WasmAbiVersion::ZERO_ZERO;
        assert!(matches!(
            negotiate_v0(
                declaration,
                approval(
                    WasmCapabilitySet::EMPTY,
                    WasmOperationSet::from_wire_bits(0)
                ),
            ),
            Err(WasmAbiNegotiationError::InvalidAbiRange { .. })
        ));
    }

    #[test]
    fn 未知の必須featureを拒否し任意featureはgrantしない() {
        let mut required = module(WasmCapabilitySet::EMPTY);
        required.required_features = 1;
        assert!(matches!(
            negotiate_v0(
                required,
                approval(
                    WasmCapabilitySet::EMPTY,
                    WasmOperationSet::from_wire_bits(0)
                ),
            ),
            Err(WasmAbiNegotiationError::UnknownRequiredFeature { bits: 1 })
        ));

        let mut optional = module(WasmCapabilitySet::EMPTY);
        optional.optional_features = u64::MAX;
        let negotiated = negotiate_v0(
            optional,
            approval(
                WasmCapabilitySet::EMPTY,
                WasmOperationSet::from_wire_bits(0),
            ),
        )
        .unwrap();
        assert_eq!(negotiated.granted_features(), 0);
    }

    #[test]
    fn 未知の必須能力とpolicyが拒否した必須能力を区別する() {
        let unknown = module(WasmCapabilitySet::from_wire(1 << 63));
        assert!(matches!(
            negotiate_v0(
                unknown,
                approval(WasmCapabilitySet::from_wire(u64::MAX), WasmOperationSet::from_wire_bits(0)),
            ),
            Err(WasmAbiNegotiationError::UnknownRequiredCapability { bits }) if bits == 1 << 63
        ));

        let denied = module(WasmCapabilitySet::REGISTER_SPACING_TABLE);
        assert!(matches!(
            negotiate_v0(
                denied,
                approval(WasmCapabilitySet::EMPTY, WasmOperationSet::from_wire_bits(0)),
            ),
            Err(WasmAbiNegotiationError::CapabilityDenied { bits }) if bits == 1
        ));
    }

    #[test]
    fn 任意能力は既知bitとpolicy許可の積だけをgrantする() {
        let mut declaration = module(WasmCapabilitySet::REGISTER_SPACING_TABLE);
        declaration.optional_capabilities = WasmCapabilitySet::from_wire(
            WasmCapabilitySet::PROPOSE_SPACING_BATCH.to_wire() | (1 << 60),
        );
        let negotiated = negotiate_v0(
            declaration,
            approval(
                WasmCapabilitySet::REGISTER_SPACING_TABLE
                    .union(WasmCapabilitySet::PROPOSE_SPACING_BATCH),
                WasmOperationSet::SPACING_TABLE_UPLOAD,
            ),
        )
        .unwrap();
        assert_eq!(
            negotiated.granted_capabilities().to_wire(),
            WasmCapabilitySet::REGISTER_SPACING_TABLE.to_wire()
                | WasmCapabilitySet::PROPOSE_SPACING_BATCH.to_wire()
        );
    }

    #[test]
    fn operationに対応する能力がなければ実行集合へ入れない() {
        let error = negotiate_v0(
            module(WasmCapabilitySet::EMPTY),
            approval(
                WasmCapabilitySet::REGISTER_SPACING_TABLE,
                WasmOperationSet::SPACING_TABLE_UPLOAD,
            ),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WasmAbiNegotiationError::OperationCapabilityMissing {
                operation_bit: 1,
                capability_bit: 1,
            }
        ));
        assert_eq!(error.code(), "CapabilityDenied");
    }

    #[test]
    fn 未知operationをmodule実行前に拒否する() {
        assert!(matches!(
            negotiate_v0(
                module(WasmCapabilitySet::EMPTY),
                approval(
                    WasmCapabilitySet::EMPTY,
                    WasmOperationSet::from_wire_bits(1 << 7),
                ),
            ),
            Err(WasmAbiNegotiationError::UnknownOperation { bits: 0x80 })
        ));
    }

    #[test]
    fn version能力operationを一つの交渉結果へ固定する() {
        let negotiated = negotiate_v0(
            module(WasmCapabilitySet::REGISTER_UNIT_TABLE),
            approval(
                WasmCapabilitySet::REGISTER_UNIT_TABLE,
                WasmOperationSet::UNIT_TABLE_UPLOAD,
            ),
        )
        .unwrap();
        assert_eq!(negotiated.selected_version(), WasmAbiVersion::ZERO_ZERO);
        assert_eq!(
            negotiated.granted_capabilities(),
            WasmCapabilitySet::REGISTER_UNIT_TABLE
        );
        assert_eq!(negotiated.operations(), WasmOperationSet::UNIT_TABLE_UPLOAD);
    }
}

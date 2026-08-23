use crate::dimension::{Dimension, MAX_DIMEN};
use crate::eqtb::LanguageRegion;
use crate::nodes::{DimensionOrder, HigherOrderDimension};
use crate::spacing_table_domain::{
    find_contextual_pair_overlap, find_contextual_scalar_overlap, provider_class_index,
    valid_nonzero_mask, ContextOverlapSearchError, ContextualPairKey, ContextualScalarRange,
    DomainWritingMode, ScalarRangeDomainError, LANGUAGE_REGION_COUNT, VALID_LANGUAGE_REGION_MASK,
    VALID_WRITING_MODE_MASK, WRITING_MODE_COUNT,
};

use std::num::NonZeroU32;

pub(crate) mod finalizer;
pub(crate) mod planner;
// This adapter is deliberately a child module: only validated adapters may construct the
// canonical native-table types whose compiler relies on release-build invariants.
#[allow(dead_code)]
#[path = "wasm_spacing_compiler_v0.rs"]
pub(crate) mod wasm_compiler_v0;

pub(crate) const MAX_SCRIPT_SPACING_CLASSES: u32 = 64;
pub(crate) const MAX_SCRIPT_SPACING_RANGES: usize = 4_096;
pub(crate) const MAX_SCRIPT_SPACING_RULES: usize = 16_384;

const REGION_COUNT: usize = LANGUAGE_REGION_COUNT;
const CLASSIFICATION_CONTEXT_COUNT: usize = REGION_COUNT * WRITING_MODE_COUNT;

/// A class number in a provider's declaration.
///
/// This is deliberately not convertible to [`ScriptClassId`] outside this module. Provider
/// numbers are one-based and only meaningful while one complete proposal is being validated;
/// zero is reserved by the WASM ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderScriptClassId(u32);

impl ProviderScriptClassId {
    pub(crate) const fn from_wire(value: u32) -> Self {
        Self(value)
    }
}

/// A class number in one host-owned, validated table.
///
/// It is neither a JFM character type nor a value that may be saved in a format file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScriptClassId(u16);

impl ScriptClassId {
    const fn index(self) -> usize {
        self.0 as usize
    }

    /// Constructs a dense ID obtained from the shared provider-class codec.
    fn from_validated_dense_index(index: u32) -> Option<Self> {
        u16::try_from(index).ok().map(Self)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WritingMode {
    Horizontal = 0,
    Vertical = 1,
}

impl WritingMode {
    const ALL: [Self; WRITING_MODE_COUNT] = [Self::Horizontal, Self::Vertical];

    const fn index(self) -> usize {
        self as usize
    }
}

/// A provider's bit mask over the public [`LanguageRegion`] codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderRegionMask(u32);

impl ProviderRegionMask {
    pub(crate) const fn from_wire(value: u32) -> Self {
        Self(value)
    }

    pub(crate) const fn all() -> Self {
        Self(VALID_LANGUAGE_REGION_MASK)
    }

    pub(crate) const fn one(region: LanguageRegion) -> Self {
        Self(1 << region.code())
    }

    const fn contains_code(self, region_code: usize) -> bool {
        self.0 & (1 << region_code) != 0
    }
}

/// A provider's bit mask whose bits 0 and 1 mean horizontal and vertical writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderWritingModeMask(u32);

impl ProviderWritingModeMask {
    pub(crate) const fn from_wire(value: u32) -> Self {
        Self(value)
    }

    pub(crate) const fn all() -> Self {
        Self(VALID_WRITING_MODE_MASK)
    }

    pub(crate) const fn one(writing_mode: WritingMode) -> Self {
        Self(1 << writing_mode.index())
    }

    const fn contains(self, writing_mode: WritingMode) -> bool {
        self.0 & (1 << writing_mode.index()) != 0
    }
}

/// A fixed glue value which no longer needs validation in the list finalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FixedGlue {
    width: Dimension,
    stretch: HigherOrderDimension,
    shrink: HigherOrderDimension,
}

impl FixedGlue {
    pub(crate) const fn width(self) -> Dimension {
        self.width
    }

    pub(crate) const fn stretch(self) -> HigherOrderDimension {
        self.stretch
    }

    pub(crate) const fn shrink(self) -> HigherOrderDimension {
        self.shrink
    }
}

/// A contextual length basis after the public API or wire codec has been removed.
///
/// Context-dependent values remain exact recipes until a boundary supplies one immutable metric
/// snapshot. In particular, registration must not freeze `em` or `zw` to whichever font happened
/// to be current at that time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ContextualSpacingLengthBasis {
    AbsoluteScaledPoint,
    LeftEm,
    RightEm,
    LeftZw,
    RightZw,
}

/// A reduced rational length owned by the native compiled table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContextualSpacingLength {
    numerator: i64,
    denominator: NonZeroU32,
    basis: ContextualSpacingLengthBasis,
}

impl ContextualSpacingLength {
    /// Constructs a value that has already passed the canonical proposal validator.
    ///
    /// This constructor deliberately takes `NonZeroU32`; callers cannot accidentally reintroduce
    /// the raw wire denominator-zero state while crossing into the native table.
    const fn from_canonical_parts(
        numerator: i64,
        denominator: NonZeroU32,
        basis: ContextualSpacingLengthBasis,
    ) -> Self {
        Self {
            numerator,
            denominator,
            basis,
        }
    }

    pub(crate) const fn numerator(self) -> i64 {
        self.numerator
    }

    pub(crate) const fn denominator(self) -> u32 {
        self.denominator.get()
    }

    pub(crate) const fn basis(self) -> ContextualSpacingLengthBasis {
        self.basis
    }

    /// Resolves this exact recipe using one boundary-local metric snapshot.
    pub(crate) fn resolve(
        self,
        metrics: BoundaryMetricSnapshot,
    ) -> Result<Dimension, ContextualSpacingResolutionError> {
        let basis_value = match self.basis {
            ContextualSpacingLengthBasis::AbsoluteScaledPoint => 1,
            ContextualSpacingLengthBasis::LeftEm => metrics.left_em,
            ContextualSpacingLengthBasis::RightEm => metrics.right_em,
            ContextualSpacingLengthBasis::LeftZw => metrics.left_zw,
            ContextualSpacingLengthBasis::RightZw => metrics.right_zw,
        };
        let product = i128::from(self.numerator)
            .checked_mul(i128::from(basis_value))
            .ok_or(ContextualSpacingResolutionError::ArithmeticOverflow { basis: self.basis })?;
        let denominator = i128::from(self.denominator.get());
        let quotient = product / denominator;
        let remainder = product % denominator;
        let twice_remainder = remainder
            .abs()
            .checked_mul(2)
            .ok_or(ContextualSpacingResolutionError::ArithmeticOverflow { basis: self.basis })?;
        let rounded = if twice_remainder >= denominator {
            quotient
                .checked_add(product.signum())
                .ok_or(ContextualSpacingResolutionError::ArithmeticOverflow { basis: self.basis })?
        } else {
            quotient
        };
        if !(-i128::from(MAX_DIMEN)..=i128::from(MAX_DIMEN)).contains(&rounded) {
            return Err(ContextualSpacingResolutionError::DimensionOutOfBounds {
                basis: self.basis,
                rounded,
            });
        }
        i32::try_from(rounded).map_err(|_| ContextualSpacingResolutionError::DimensionOutOfBounds {
            basis: self.basis,
            rounded,
        })
    }
}

/// Font/JFM dimensions captured once for one left/right boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoundaryMetricSnapshot {
    left_em: Dimension,
    right_em: Dimension,
    left_zw: Dimension,
    right_zw: Dimension,
}

impl BoundaryMetricSnapshot {
    pub(crate) fn new(
        left_em: Dimension,
        right_em: Dimension,
        left_zw: Dimension,
        right_zw: Dimension,
    ) -> Result<Self, BoundaryMetricSnapshotError> {
        for (metric, value) in [
            (BoundaryMetric::LeftEm, left_em),
            (BoundaryMetric::RightEm, right_em),
            (BoundaryMetric::LeftZw, left_zw),
            (BoundaryMetric::RightZw, right_zw),
        ] {
            if !(0..=MAX_DIMEN).contains(&value) {
                return Err(BoundaryMetricSnapshotError { metric, value });
            }
        }
        Ok(Self {
            left_em,
            right_em,
            left_zw,
            right_zw,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryMetric {
    LeftEm,
    RightEm,
    LeftZw,
    RightZw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoundaryMetricSnapshotError {
    pub(crate) metric: BoundaryMetric,
    pub(crate) value: Dimension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextualSpacingResolutionError {
    ArithmeticOverflow {
        basis: ContextualSpacingLengthBasis,
    },
    DimensionOutOfBounds {
        basis: ContextualSpacingLengthBasis,
        rounded: i128,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScriptSpacingBreakRule {
    UseBuiltIn,
    Allow,
    Forbid,
    Penalty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScriptSpacingLineEdgeRule {
    UseBuiltIn,
    Retain,
    DiscardAtStart,
    DiscardAtEnd,
    DiscardAtBoth,
}

/// Native form of one custom class-pair boundary rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScriptSpacingBoundaryRule {
    natural: ContextualSpacingLength,
    shrink_limit: ContextualSpacingLength,
    stretch_limit: ContextualSpacingLength,
    shrink_tier: u8,
    stretch_tier: u8,
    break_rule: ScriptSpacingBreakRule,
    line_edge_rule: ScriptSpacingLineEdgeRule,
    penalty: i32,
    reason_id: u32,
}

impl ScriptSpacingBoundaryRule {
    #[allow(clippy::too_many_arguments)]
    const fn from_canonical_parts(
        natural: ContextualSpacingLength,
        shrink_limit: ContextualSpacingLength,
        stretch_limit: ContextualSpacingLength,
        shrink_tier: u8,
        stretch_tier: u8,
        break_rule: ScriptSpacingBreakRule,
        line_edge_rule: ScriptSpacingLineEdgeRule,
        penalty: i32,
        reason_id: u32,
    ) -> Self {
        Self {
            natural,
            shrink_limit,
            stretch_limit,
            shrink_tier,
            stretch_tier,
            break_rule,
            line_edge_rule,
            penalty,
            reason_id,
        }
    }

    pub(crate) const fn lengths(
        self,
    ) -> (
        ContextualSpacingLength,
        ContextualSpacingLength,
        ContextualSpacingLength,
    ) {
        (self.natural, self.shrink_limit, self.stretch_limit)
    }

    pub(crate) const fn tiers(self) -> (u8, u8) {
        (self.shrink_tier, self.stretch_tier)
    }

    pub(crate) const fn break_rule(self) -> ScriptSpacingBreakRule {
        self.break_rule
    }

    pub(crate) const fn line_edge_rule(self) -> ScriptSpacingLineEdgeRule {
        self.line_edge_rule
    }

    pub(crate) const fn penalty(self) -> i32 {
        self.penalty
    }

    pub(crate) const fn reason_id(self) -> u32 {
        self.reason_id
    }

    pub(crate) fn resolve(
        self,
        metrics: BoundaryMetricSnapshot,
    ) -> Result<ResolvedScriptSpacingBoundaryRule, ContextualSpacingResolutionError> {
        Ok(ResolvedScriptSpacingBoundaryRule {
            glue: FixedGlue {
                width: self.natural.resolve(metrics)?,
                stretch: HigherOrderDimension {
                    value: self.stretch_limit.resolve(metrics)?,
                    order: DimensionOrder::Normal,
                },
                shrink: HigherOrderDimension {
                    value: self.shrink_limit.resolve(metrics)?,
                    order: DimensionOrder::Normal,
                },
            },
            shrink_tier: self.shrink_tier,
            stretch_tier: self.stretch_tier,
            break_rule: self.break_rule,
            line_edge_rule: self.line_edge_rule,
            penalty: self.penalty,
            reason_id: self.reason_id,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedScriptSpacingBoundaryRule {
    glue: FixedGlue,
    shrink_tier: u8,
    stretch_tier: u8,
    break_rule: ScriptSpacingBreakRule,
    line_edge_rule: ScriptSpacingLineEdgeRule,
    penalty: i32,
    reason_id: u32,
}

impl ResolvedScriptSpacingBoundaryRule {
    pub(crate) const fn glue(self) -> FixedGlue {
        self.glue
    }

    pub(crate) const fn tiers(self) -> (u8, u8) {
        (self.shrink_tier, self.stretch_tier)
    }

    pub(crate) const fn break_rule(self) -> ScriptSpacingBreakRule {
        self.break_rule
    }

    pub(crate) const fn line_edge_rule(self) -> ScriptSpacingLineEdgeRule {
        self.line_edge_rule
    }

    pub(crate) const fn penalty(self) -> i32 {
        self.penalty
    }

    pub(crate) const fn reason_id(self) -> u32 {
        self.reason_id
    }
}

/// The result consumed by native Japanese spacing and by uploaded compiled profiles alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScriptSpacingAction {
    BuiltInFallback,
    NoAutomaticSpace,
    KanjiSkip,
    XKanjiSkip,
    FixedGlue(FixedGlue),
    BoundaryRule(ScriptSpacingBoundaryRule),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FixedGlueProposal {
    width: i64,
    stretch: i64,
    stretch_order: u8,
    shrink: i64,
    shrink_order: u8,
}

impl FixedGlueProposal {
    pub(crate) const fn new(
        width: i64,
        stretch: i64,
        stretch_order: u8,
        shrink: i64,
        shrink_order: u8,
    ) -> Self {
        Self {
            width,
            stretch,
            stretch_order,
            shrink,
            shrink_order,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScriptSpacingActionProposal {
    BuiltInFallback,
    NoAutomaticSpace,
    KanjiSkip,
    XKanjiSkip,
    FixedGlue(FixedGlueProposal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnicodeScalarRangeProposal {
    start: u32,
    end: u32,
    class: ProviderScriptClassId,
    region_mask: ProviderRegionMask,
    writing_mode_mask: ProviderWritingModeMask,
}

impl UnicodeScalarRangeProposal {
    pub(crate) const fn new(
        start: u32,
        end: u32,
        class: ProviderScriptClassId,
        region_mask: ProviderRegionMask,
        writing_mode_mask: ProviderWritingModeMask,
    ) -> Self {
        Self {
            start,
            end,
            class,
            region_mask,
            writing_mode_mask,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScriptPairRuleProposal {
    left: ProviderScriptClassId,
    right: ProviderScriptClassId,
    region: LanguageRegion,
    writing_mode: WritingMode,
    action: ScriptSpacingActionProposal,
}

impl ScriptPairRuleProposal {
    pub(crate) const fn new(
        left: ProviderScriptClassId,
        right: ProviderScriptClassId,
        region: LanguageRegion,
        writing_mode: WritingMode,
        action: ScriptSpacingActionProposal,
    ) -> Self {
        Self {
            left,
            right,
            region,
            writing_mode,
            action,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptSpacingTableProposal {
    class_count: u32,
    ranges: Vec<UnicodeScalarRangeProposal>,
    rules: Vec<ScriptPairRuleProposal>,
}

impl ScriptSpacingTableProposal {
    pub(crate) fn new(
        class_count: u32,
        ranges: Vec<UnicodeScalarRangeProposal>,
        rules: Vec<ScriptPairRuleProposal>,
    ) -> Self {
        Self {
            class_count,
            ranges,
            rules,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlueComponent {
    Width,
    Stretch,
    Shrink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScriptSpacingTableError {
    EmptyClassSet,
    TooManyClasses {
        actual: u32,
        maximum: u32,
    },
    TooManyRanges {
        actual: usize,
        maximum: usize,
    },
    TooManyRules {
        actual: usize,
        maximum: usize,
    },
    ReversedScalarRange {
        range_index: usize,
    },
    NonScalarRange {
        range_index: usize,
    },
    RangeClassOutOfBounds {
        range_index: usize,
        class: u32,
        class_count: u32,
    },
    InvalidRangeRegionMask {
        range_index: usize,
        mask: u32,
    },
    InvalidRangeWritingModeMask {
        range_index: usize,
        mask: u32,
    },
    OverlappingScalarRanges {
        first_range_index: usize,
        second_range_index: usize,
        region_code: u8,
        writing_mode: WritingMode,
    },
    RuleClassOutOfBounds {
        rule_index: usize,
        class: u32,
        class_count: u32,
    },
    DuplicatePairRules {
        first_rule_index: usize,
        second_rule_index: usize,
    },
    InvalidGlueOrder {
        rule_index: usize,
        component: GlueComponent,
        order: u8,
    },
    GlueComponentOutOfBounds {
        rule_index: usize,
        component: GlueComponent,
        value: i64,
    },
    CompiledTableTooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnicodeScalarRange {
    start: u32,
    end: u32,
    class: ScriptClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RangeSpan {
    start: usize,
    end: usize,
}

impl RangeSpan {
    const EMPTY: Self = Self { start: 0, end: 0 };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ScriptPairKey {
    left: u16,
    right: u16,
    region: u8,
    writing_mode: WritingMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidatedRule {
    original_index: usize,
    key: ScriptPairKey,
    action: ScriptSpacingAction,
}

/// A scalar range whose Unicode, class and context domains were already validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CanonicalScriptSpacingRange {
    start: u32,
    end: u32,
    class: ScriptClassId,
    region_mask: ProviderRegionMask,
    writing_mode_mask: ProviderWritingModeMask,
}

impl CanonicalScriptSpacingRange {
    const fn from_validated_parts(
        start: u32,
        end: u32,
        class: ScriptClassId,
        region_mask: ProviderRegionMask,
        writing_mode_mask: ProviderWritingModeMask,
    ) -> Self {
        Self {
            start,
            end,
            class,
            region_mask,
            writing_mode_mask,
        }
    }
}

/// A class-pair rule whose class IDs and context masks were already validated as one table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CanonicalScriptSpacingRule {
    left: ScriptClassId,
    right: ScriptClassId,
    region_mask: ProviderRegionMask,
    writing_mode_mask: ProviderWritingModeMask,
    action: ScriptSpacingAction,
}

impl CanonicalScriptSpacingRule {
    const fn from_validated_parts(
        left: ScriptClassId,
        right: ScriptClassId,
        region_mask: ProviderRegionMask,
        writing_mode_mask: ProviderWritingModeMask,
        action: ScriptSpacingAction,
    ) -> Self {
        Self {
            left,
            right,
            region_mask,
            writing_mode_mask,
            action,
        }
    }
}

/// Provider-independent input to the one native table compiler.
///
/// Construction is restricted to validators/adapters that have already checked the complete
/// proposal. The compiler below expands masks and allocates the dense table, but intentionally
/// does not make the scalar/mask/overlap decisions a second time.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalScriptSpacingTableCandidate {
    class_capacity: u16,
    ranges: Vec<CanonicalScriptSpacingRange>,
    rules: Vec<CanonicalScriptSpacingRule>,
}

impl CanonicalScriptSpacingTableCandidate {
    fn from_validated_parts(
        class_capacity: u16,
        ranges: Vec<CanonicalScriptSpacingRange>,
        rules: Vec<CanonicalScriptSpacingRule>,
    ) -> Self {
        Self {
            class_capacity,
            ranges,
            rules,
        }
    }
}

/// A host-owned table. All allocation and validation happens when it is compiled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledScriptSpacingTable {
    class_count: u16,
    ranges: Vec<UnicodeScalarRange>,
    /// Slices in `ranges`, indexed by `(writing mode, region)`.
    range_spans: [RangeSpan; CLASSIFICATION_CONTEXT_COUNT],
    /// Dense `(writing mode, region, left, right)` slots containing indexes into `actions`.
    action_indices: Vec<u16>,
    /// Slot zero is always `BuiltInFallback`.
    actions: Vec<ScriptSpacingAction>,
}

impl CompiledScriptSpacingTable {
    /// Validates the complete proposal before returning a table visible to the caller.
    pub(crate) fn compile(
        proposal: ScriptSpacingTableProposal,
    ) -> Result<Self, ScriptSpacingTableError> {
        validate_proposal_size(&proposal)?;

        let class_capacity = u16::try_from(proposal.class_count)
            .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
        let mut validated_range_classes = Vec::new();
        validated_range_classes
            .try_reserve_exact(proposal.ranges.len())
            .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
        for (range_index, range) in proposal.ranges.iter().copied().enumerate() {
            validated_range_classes.push(validate_scalar_range(
                range_index,
                range,
                proposal.class_count,
            )?);
        }
        let range_overlap = find_contextual_scalar_overlap(proposal.ranges.iter().enumerate().map(
            |(source_index, range)| ContextualScalarRange {
                source_index,
                first: range.start,
                last_inclusive: range.end,
                region_mask: range.region_mask.0,
                writing_mode_mask: range.writing_mode_mask.0,
            },
        ))
        .map_err(map_context_overlap_search_error)?;
        if let Some(overlap) = range_overlap {
            return Err(ScriptSpacingTableError::OverlappingScalarRanges {
                first_range_index: overlap.first_source_index,
                second_range_index: overlap.second_source_index,
                region_code: overlap.region_code,
                writing_mode: writing_mode_from_domain(overlap.writing_mode),
            });
        }

        let mut canonical_ranges = Vec::new();
        canonical_ranges
            .try_reserve_exact(proposal.ranges.len())
            .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
        for (range, class) in proposal.ranges.into_iter().zip(validated_range_classes) {
            canonical_ranges.push(CanonicalScriptSpacingRange::from_validated_parts(
                range.start,
                range.end,
                class,
                range.region_mask,
                range.writing_mode_mask,
            ));
        }

        let mut rules = Vec::new();
        rules
            .try_reserve_exact(proposal.rules.len())
            .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
        for (rule_index, rule) in proposal.rules.into_iter().enumerate() {
            let left = validate_rule_class(rule_index, rule.left, proposal.class_count)?;
            let right = validate_rule_class(rule_index, rule.right, proposal.class_count)?;
            rules.push(ValidatedRule {
                original_index: rule_index,
                key: ScriptPairKey {
                    left: left.0,
                    right: right.0,
                    region: rule.region.code(),
                    writing_mode: rule.writing_mode,
                },
                action: validate_action(rule_index, rule.action)?,
            });
        }
        let rule_overlap =
            find_contextual_pair_overlap(rules.iter().map(|rule| ContextualPairKey {
                source_index: rule.original_index,
                left_class_id: u32::from(rule.key.left),
                right_class_id: u32::from(rule.key.right),
                region_mask: 1 << rule.key.region,
                writing_mode_mask: 1 << rule.key.writing_mode.index(),
            }))
            .map_err(map_context_overlap_search_error)?;
        if let Some(overlap) = rule_overlap {
            return Err(ScriptSpacingTableError::DuplicatePairRules {
                first_rule_index: overlap.first_source_index,
                second_rule_index: overlap.second_source_index,
            });
        }
        rules.sort_unstable_by_key(|rule| (rule.key, rule.original_index));

        let mut canonical_rules = Vec::new();
        canonical_rules
            .try_reserve_exact(rules.len())
            .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
        for rule in rules {
            canonical_rules.push(CanonicalScriptSpacingRule::from_validated_parts(
                ScriptClassId(rule.key.left),
                ScriptClassId(rule.key.right),
                ProviderRegionMask::from_wire(1 << rule.key.region),
                ProviderWritingModeMask::from_wire(1 << rule.key.writing_mode.index()),
                rule.action,
            ));
        }

        Self::compile_canonical(CanonicalScriptSpacingTableCandidate::from_validated_parts(
            class_capacity,
            canonical_ranges,
            canonical_rules,
        ))
    }

    /// Expands one fully validated provider-independent candidate into the native dense table.
    ///
    /// Domain validation deliberately stays outside this method. Both the native proposal adapter
    /// above and external adapters must produce this typed candidate before reaching the one
    /// allocation/expansion implementation.
    fn compile_canonical(
        candidate: CanonicalScriptSpacingTableCandidate,
    ) -> Result<Self, ScriptSpacingTableError> {
        Self::compile_canonical_with_max_dense_slots(candidate, usize::MAX)
    }

    fn compile_canonical_with_max_dense_slots(
        candidate: CanonicalScriptSpacingTableCandidate,
        max_dense_slots: usize,
    ) -> Result<Self, ScriptSpacingTableError> {
        let class_count = candidate.class_capacity;
        let mut ranges_by_context: [Vec<UnicodeScalarRange>; CLASSIFICATION_CONTEXT_COUNT] =
            std::array::from_fn(|_| Vec::new());
        for context_ranges in &mut ranges_by_context {
            context_ranges
                .try_reserve_exact(candidate.ranges.len())
                .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
        }
        for range in candidate.ranges {
            let native_range = UnicodeScalarRange {
                start: range.start,
                end: range.end,
                class: range.class,
            };
            for writing_mode in WritingMode::ALL {
                if !range.writing_mode_mask.contains(writing_mode) {
                    continue;
                }
                for region_code in 0..REGION_COUNT {
                    if range.region_mask.contains_code(region_code) {
                        ranges_by_context[classification_context_index(region_code, writing_mode)]
                            .push(native_range);
                    }
                }
            }
        }

        let expanded_range_count = ranges_by_context
            .iter()
            .try_fold(0usize, |count, ranges| count.checked_add(ranges.len()))
            .ok_or(ScriptSpacingTableError::CompiledTableTooLarge)?;
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(expanded_range_count)
            .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
        let mut range_spans = [RangeSpan::EMPTY; CLASSIFICATION_CONTEXT_COUNT];
        for writing_mode in WritingMode::ALL {
            for region_code in 0..REGION_COUNT {
                let context = classification_context_index(region_code, writing_mode);
                let context_ranges = &mut ranges_by_context[context];
                context_ranges.sort_unstable_by_key(|range| (range.start, range.end));
                let start = ranges.len();
                ranges.extend(context_ranges.iter().copied());
                range_spans[context] = RangeSpan {
                    start,
                    end: ranges.len(),
                };
            }
        }

        let slot_count = usize::from(class_count)
            .checked_mul(usize::from(class_count))
            .and_then(|count| count.checked_mul(REGION_COUNT))
            .and_then(|count| count.checked_mul(WRITING_MODE_COUNT))
            .ok_or(ScriptSpacingTableError::CompiledTableTooLarge)?;
        if slot_count > max_dense_slots {
            return Err(ScriptSpacingTableError::CompiledTableTooLarge);
        }
        let mut action_indices = Vec::new();
        action_indices
            .try_reserve_exact(slot_count)
            .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
        action_indices.resize(slot_count, 0);
        let action_capacity = candidate
            .rules
            .len()
            .checked_add(1)
            .ok_or(ScriptSpacingTableError::CompiledTableTooLarge)?;
        let mut actions = Vec::new();
        actions
            .try_reserve_exact(action_capacity)
            .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
        actions.push(ScriptSpacingAction::BuiltInFallback);
        for rule in candidate.rules {
            let action_index = u16::try_from(actions.len())
                .map_err(|_| ScriptSpacingTableError::CompiledTableTooLarge)?;
            actions.push(rule.action);
            for writing_mode in WritingMode::ALL {
                if !rule.writing_mode_mask.contains(writing_mode) {
                    continue;
                }
                for region_code in 0..REGION_COUNT {
                    if !rule.region_mask.contains_code(region_code) {
                        continue;
                    }
                    let slot = slot_index(
                        usize::from(class_count),
                        rule.left,
                        rule.right,
                        region_code,
                        writing_mode,
                    );
                    debug_assert_eq!(action_indices[slot], 0, "canonical rules must not overlap");
                    action_indices[slot] = action_index;
                }
            }
        }

        Ok(Self {
            class_count,
            ranges,
            range_spans,
            action_indices,
            actions,
        })
    }

    #[cfg(test)]
    /// Supplies a deterministic dense-allocation ceiling so atomic failure can be tested without
    /// relying on the process allocator to exhaust memory.
    fn try_replace_canonical_with_max_dense_slots(
        &mut self,
        candidate: CanonicalScriptSpacingTableCandidate,
        max_dense_slots: usize,
    ) -> Result<(), ScriptSpacingTableError> {
        let replacement = Self::compile_canonical_with_max_dense_slots(candidate, max_dense_slots)?;
        *self = replacement;
        Ok(())
    }

    /// Compiles first and only then replaces the active table.
    pub(crate) fn try_replace(
        &mut self,
        proposal: ScriptSpacingTableProposal,
    ) -> Result<(), ScriptSpacingTableError> {
        let replacement = Self::compile(proposal)?;
        *self = replacement;
        Ok(())
    }

    pub(crate) const fn class_count(&self) -> u16 {
        self.class_count
    }

    /// Classifies one already-validated Unicode scalar in its layout context without allocation.
    #[inline]
    pub(crate) fn classify_scalar(
        &self,
        scalar: char,
        region: LanguageRegion,
        writing_mode: WritingMode,
    ) -> Option<ScriptClassId> {
        let span =
            self.range_spans[classification_context_index(region.code() as usize, writing_mode)];
        let ranges = &self.ranges[span.start..span.end];
        let scalar = scalar as u32;
        let insertion_point = ranges.partition_point(|range| range.start <= scalar);
        if insertion_point == 0 {
            return None;
        }
        let range = ranges[insertion_point - 1];
        (scalar <= range.end).then_some(range.class)
    }

    /// Performs a direct table lookup without allocation, dynamic dispatch, or an ABI call.
    #[inline]
    pub(crate) fn action_for(
        &self,
        left: ScriptClassId,
        right: ScriptClassId,
        region: LanguageRegion,
        writing_mode: WritingMode,
    ) -> ScriptSpacingAction {
        let class_count = usize::from(self.class_count);
        if left.index() >= class_count || right.index() >= class_count {
            debug_assert!(false, "script class belongs to another compiled table");
            return ScriptSpacingAction::BuiltInFallback;
        }
        let slot = slot_index(
            class_count,
            left,
            right,
            region.code() as usize,
            writing_mode,
        );
        self.actions[self.action_indices[slot] as usize]
    }
}

/// The dispatcher chooses one variant once per list, then the hot loop keeps the returned table.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ScriptSpacingProfileRef<'a> {
    BuiltIn(&'a CompiledScriptSpacingTable),
    CompiledTable(&'a CompiledScriptSpacingTable),
}

impl<'a> ScriptSpacingProfileRef<'a> {
    pub(crate) const fn select_table(self) -> &'a CompiledScriptSpacingTable {
        match self {
            Self::BuiltIn(table) | Self::CompiledTable(table) => table,
        }
    }
}

fn validate_proposal_size(
    proposal: &ScriptSpacingTableProposal,
) -> Result<(), ScriptSpacingTableError> {
    if proposal.class_count == 0 {
        return Err(ScriptSpacingTableError::EmptyClassSet);
    }
    if proposal.class_count > MAX_SCRIPT_SPACING_CLASSES {
        return Err(ScriptSpacingTableError::TooManyClasses {
            actual: proposal.class_count,
            maximum: MAX_SCRIPT_SPACING_CLASSES,
        });
    }
    if proposal.ranges.len() > MAX_SCRIPT_SPACING_RANGES {
        return Err(ScriptSpacingTableError::TooManyRanges {
            actual: proposal.ranges.len(),
            maximum: MAX_SCRIPT_SPACING_RANGES,
        });
    }
    if proposal.rules.len() > MAX_SCRIPT_SPACING_RULES {
        return Err(ScriptSpacingTableError::TooManyRules {
            actual: proposal.rules.len(),
            maximum: MAX_SCRIPT_SPACING_RULES,
        });
    }
    Ok(())
}

fn validate_scalar_range(
    range_index: usize,
    range: UnicodeScalarRangeProposal,
    class_count: u32,
) -> Result<ScriptClassId, ScriptSpacingTableError> {
    match crate::spacing_table_domain::validate_scalar_range(range.start, range.end) {
        Ok(()) => {}
        Err(ScalarRangeDomainError::Reversed) => {
            return Err(ScriptSpacingTableError::ReversedScalarRange { range_index })
        }
        Err(ScalarRangeDomainError::NonScalar) => {
            return Err(ScriptSpacingTableError::NonScalarRange { range_index })
        }
    }
    if !valid_nonzero_mask(range.region_mask.0, VALID_LANGUAGE_REGION_MASK) {
        return Err(ScriptSpacingTableError::InvalidRangeRegionMask {
            range_index,
            mask: range.region_mask.0,
        });
    }
    if !valid_nonzero_mask(range.writing_mode_mask.0, VALID_WRITING_MODE_MASK) {
        return Err(ScriptSpacingTableError::InvalidRangeWritingModeMask {
            range_index,
            mask: range.writing_mode_mask.0,
        });
    }
    translate_provider_class(range.class, class_count).ok_or(
        ScriptSpacingTableError::RangeClassOutOfBounds {
            range_index,
            class: range.class.0,
            class_count,
        },
    )
}

fn validate_rule_class(
    rule_index: usize,
    class: ProviderScriptClassId,
    class_count: u32,
) -> Result<ScriptClassId, ScriptSpacingTableError> {
    translate_provider_class(class, class_count).ok_or(
        ScriptSpacingTableError::RuleClassOutOfBounds {
            rule_index,
            class: class.0,
            class_count,
        },
    )
}

/// ABI class zero is reserved; this is the sole wire-to-dense-ID translation point.
fn translate_provider_class(
    class: ProviderScriptClassId,
    class_count: u32,
) -> Option<ScriptClassId> {
    provider_class_index(class.0, class_count).map(|index| ScriptClassId(index as u16))
}

fn map_context_overlap_search_error(error: ContextOverlapSearchError) -> ScriptSpacingTableError {
    match error {
        ContextOverlapSearchError::AllocationFailed => {
            ScriptSpacingTableError::CompiledTableTooLarge
        }
    }
}

fn writing_mode_from_domain(writing_mode: DomainWritingMode) -> WritingMode {
    match writing_mode {
        DomainWritingMode::Horizontal => WritingMode::Horizontal,
        DomainWritingMode::Vertical => WritingMode::Vertical,
    }
}

fn validate_action(
    rule_index: usize,
    action: ScriptSpacingActionProposal,
) -> Result<ScriptSpacingAction, ScriptSpacingTableError> {
    Ok(match action {
        ScriptSpacingActionProposal::BuiltInFallback => ScriptSpacingAction::BuiltInFallback,
        ScriptSpacingActionProposal::NoAutomaticSpace => ScriptSpacingAction::NoAutomaticSpace,
        ScriptSpacingActionProposal::KanjiSkip => ScriptSpacingAction::KanjiSkip,
        ScriptSpacingActionProposal::XKanjiSkip => ScriptSpacingAction::XKanjiSkip,
        ScriptSpacingActionProposal::FixedGlue(glue) => {
            ScriptSpacingAction::FixedGlue(validate_fixed_glue(rule_index, glue)?)
        }
    })
}

fn validate_fixed_glue(
    rule_index: usize,
    glue: FixedGlueProposal,
) -> Result<FixedGlue, ScriptSpacingTableError> {
    Ok(FixedGlue {
        width: validate_glue_value(rule_index, GlueComponent::Width, glue.width)?,
        stretch: HigherOrderDimension {
            value: validate_glue_value(rule_index, GlueComponent::Stretch, glue.stretch)?,
            order: validate_glue_order(rule_index, GlueComponent::Stretch, glue.stretch_order)?,
        },
        shrink: HigherOrderDimension {
            value: validate_glue_value(rule_index, GlueComponent::Shrink, glue.shrink)?,
            order: validate_glue_order(rule_index, GlueComponent::Shrink, glue.shrink_order)?,
        },
    })
}

fn validate_glue_value(
    rule_index: usize,
    component: GlueComponent,
    value: i64,
) -> Result<i32, ScriptSpacingTableError> {
    if !(-(MAX_DIMEN as i64)..=MAX_DIMEN as i64).contains(&value) {
        return Err(ScriptSpacingTableError::GlueComponentOutOfBounds {
            rule_index,
            component,
            value,
        });
    }
    Ok(value as i32)
}

fn validate_glue_order(
    rule_index: usize,
    component: GlueComponent,
    order: u8,
) -> Result<DimensionOrder, ScriptSpacingTableError> {
    match order {
        0 => Ok(DimensionOrder::Normal),
        1 => Ok(DimensionOrder::Fil),
        2 => Ok(DimensionOrder::Fill),
        3 => Ok(DimensionOrder::Filll),
        _ => Err(ScriptSpacingTableError::InvalidGlueOrder {
            rule_index,
            component,
            order,
        }),
    }
}

#[inline]
fn classification_context_index(region: usize, writing_mode: WritingMode) -> usize {
    writing_mode.index() * REGION_COUNT + region
}

#[inline]
fn slot_index(
    class_count: usize,
    left: ScriptClassId,
    right: ScriptClassId,
    region: usize,
    writing_mode: WritingMode,
) -> usize {
    (((writing_mode.index() * REGION_COUNT + region) * class_count + left.index()) * class_count)
        + right.index()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(value: u32) -> ProviderScriptClassId {
        ProviderScriptClassId::from_wire(value)
    }

    fn range(start: u32, end: u32, class_id: u32) -> UnicodeScalarRangeProposal {
        UnicodeScalarRangeProposal::new(
            start,
            end,
            class(class_id),
            ProviderRegionMask::all(),
            ProviderWritingModeMask::all(),
        )
    }

    fn contextual_range(
        start: u32,
        end: u32,
        class_id: u32,
        region_mask: ProviderRegionMask,
        writing_mode_mask: ProviderWritingModeMask,
    ) -> UnicodeScalarRangeProposal {
        UnicodeScalarRangeProposal::new(start, end, class(class_id), region_mask, writing_mode_mask)
    }

    fn rule(
        left: u32,
        right: u32,
        region: LanguageRegion,
        writing_mode: WritingMode,
        action: ScriptSpacingActionProposal,
    ) -> ScriptPairRuleProposal {
        ScriptPairRuleProposal::new(class(left), class(right), region, writing_mode, action)
    }

    fn 最小の正しい提案() -> ScriptSpacingTableProposal {
        ScriptSpacingTableProposal::new(
            3,
            vec![
                range(0x4e00, 0x9fff, 3),
                range(0x3040, 0x309f, 2),
                range(u32::from('A'), u32::from('Z'), 1),
            ],
            vec![
                rule(
                    3,
                    3,
                    LanguageRegion::Ja,
                    WritingMode::Horizontal,
                    ScriptSpacingActionProposal::KanjiSkip,
                ),
                rule(
                    3,
                    1,
                    LanguageRegion::Ja,
                    WritingMode::Horizontal,
                    ScriptSpacingActionProposal::XKanjiSkip,
                ),
                rule(
                    1,
                    3,
                    LanguageRegion::Ja,
                    WritingMode::Vertical,
                    ScriptSpacingActionProposal::FixedGlue(FixedGlueProposal::new(
                        65_536, 32_768, 1, 16_384, 0,
                    )),
                ),
            ],
        )
    }

    #[test]
    fn 検証済み範囲と文字クラス対を割当てなしで引ける() {
        let table = CompiledScriptSpacingTable::compile(最小の正しい提案()).unwrap();
        assert_eq!(table.class_count(), 3);

        let latin = table
            .classify_scalar('A', LanguageRegion::Ja, WritingMode::Horizontal)
            .unwrap();
        let hiragana = table
            .classify_scalar('あ', LanguageRegion::Ja, WritingMode::Horizontal)
            .unwrap();
        let han = table
            .classify_scalar('漢', LanguageRegion::Ja, WritingMode::Horizontal)
            .unwrap();
        assert_ne!(latin, hiragana);
        assert_ne!(hiragana, han);
        assert_eq!(
            table.classify_scalar('a', LanguageRegion::Ja, WritingMode::Horizontal),
            None
        );

        assert_eq!(
            table.action_for(han, han, LanguageRegion::Ja, WritingMode::Horizontal),
            ScriptSpacingAction::KanjiSkip
        );
        assert_eq!(
            table.action_for(han, latin, LanguageRegion::Ja, WritingMode::Horizontal),
            ScriptSpacingAction::XKanjiSkip
        );
        assert_eq!(
            table.action_for(han, latin, LanguageRegion::Ko, WritingMode::Horizontal),
            ScriptSpacingAction::BuiltInFallback
        );

        let ScriptSpacingAction::FixedGlue(glue) =
            table.action_for(latin, han, LanguageRegion::Ja, WritingMode::Vertical)
        else {
            panic!("固定糊でなければならない");
        };
        assert_eq!(glue.width(), 65_536);
        assert_eq!(glue.stretch().value, 32_768);
        assert_eq!(glue.stretch().order, DimensionOrder::Fil);
        assert_eq!(glue.shrink().value, 16_384);
        assert_eq!(glue.shrink().order, DimensionOrder::Normal);
    }

    #[test]
    fn 組込みと登録済み表は同じ意味型を返す() {
        let built_in = CompiledScriptSpacingTable::compile(最小の正しい提案()).unwrap();
        let uploaded = CompiledScriptSpacingTable::compile(最小の正しい提案()).unwrap();
        let built_in = ScriptSpacingProfileRef::BuiltIn(&built_in).select_table();
        let uploaded = ScriptSpacingProfileRef::CompiledTable(&uploaded).select_table();

        let left = built_in
            .classify_scalar('漢', LanguageRegion::Ja, WritingMode::Horizontal)
            .unwrap();
        let right = built_in
            .classify_scalar('A', LanguageRegion::Ja, WritingMode::Horizontal)
            .unwrap();
        let uploaded_left = uploaded
            .classify_scalar('漢', LanguageRegion::Ja, WritingMode::Horizontal)
            .unwrap();
        let uploaded_right = uploaded
            .classify_scalar('A', LanguageRegion::Ja, WritingMode::Horizontal)
            .unwrap();
        assert_eq!(
            built_in.action_for(left, right, LanguageRegion::Ja, WritingMode::Horizontal),
            uploaded.action_for(
                uploaded_left,
                uploaded_right,
                LanguageRegion::Ja,
                WritingMode::Horizontal
            )
        );
    }

    #[test]
    fn wireの一始まり文字クラスを内部の零始まり密番号へ写す() {
        let table = CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
            2,
            vec![
                range(u32::from('A'), u32::from('A'), 1),
                range(u32::from('B'), u32::from('B'), 2),
            ],
            Vec::new(),
        ))
        .unwrap();
        assert_eq!(
            table.classify_scalar('A', LanguageRegion::Und, WritingMode::Horizontal),
            Some(ScriptClassId(0))
        );
        assert_eq!(
            table.classify_scalar('B', LanguageRegion::Und, WritingMode::Horizontal),
            Some(ScriptClassId(1))
        );

        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                1,
                vec![range(u32::from('A'), u32::from('A'), 0)],
                Vec::new(),
            )),
            Err(ScriptSpacingTableError::RangeClassOutOfBounds {
                range_index: 0,
                class: 0,
                class_count: 1,
            })
        );
        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                1,
                Vec::new(),
                vec![rule(
                    0,
                    1,
                    LanguageRegion::Und,
                    WritingMode::Horizontal,
                    ScriptSpacingActionProposal::BuiltInFallback,
                )],
            )),
            Err(ScriptSpacingTableError::RuleClassOutOfBounds {
                rule_index: 0,
                class: 0,
                class_count: 1,
            })
        );
    }

    #[test]
    fn 同じ符号位置を地域と組方向ごとに別の文字クラスへできる() {
        let scalar = u32::from('A');
        let table = CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
            3,
            vec![
                contextual_range(
                    scalar,
                    scalar,
                    1,
                    ProviderRegionMask::one(LanguageRegion::Ja),
                    ProviderWritingModeMask::one(WritingMode::Horizontal),
                ),
                contextual_range(
                    scalar,
                    scalar,
                    2,
                    ProviderRegionMask::one(LanguageRegion::Ko),
                    ProviderWritingModeMask::one(WritingMode::Horizontal),
                ),
                contextual_range(
                    scalar,
                    scalar,
                    3,
                    ProviderRegionMask::one(LanguageRegion::Ja),
                    ProviderWritingModeMask::one(WritingMode::Vertical),
                ),
            ],
            Vec::new(),
        ))
        .unwrap();

        assert_eq!(
            table.classify_scalar('A', LanguageRegion::Ja, WritingMode::Horizontal),
            Some(ScriptClassId(0))
        );
        assert_eq!(
            table.classify_scalar('A', LanguageRegion::Ko, WritingMode::Horizontal),
            Some(ScriptClassId(1))
        );
        assert_eq!(
            table.classify_scalar('A', LanguageRegion::Ja, WritingMode::Vertical),
            Some(ScriptClassId(2))
        );
        assert_eq!(
            table.classify_scalar('A', LanguageRegion::Ko, WritingMode::Vertical),
            None
        );
    }

    #[test]
    fn 同じ地域と組方向でだけ重複範囲を拒む() {
        let scalar = u32::from('A');
        let ja_and_ko = ProviderRegionMask::from_wire(
            (1 << LanguageRegion::Ja.code()) | (1 << LanguageRegion::Ko.code()),
        );
        let both_modes = ProviderWritingModeMask::all();
        let proposal = ScriptSpacingTableProposal::new(
            2,
            vec![
                contextual_range(scalar, scalar, 1, ja_and_ko, both_modes),
                contextual_range(
                    scalar,
                    scalar,
                    2,
                    ProviderRegionMask::one(LanguageRegion::Ko),
                    ProviderWritingModeMask::one(WritingMode::Vertical),
                ),
            ],
            Vec::new(),
        );
        assert_eq!(
            CompiledScriptSpacingTable::compile(proposal),
            Err(ScriptSpacingTableError::OverlappingScalarRanges {
                first_range_index: 0,
                second_range_index: 1,
                region_code: LanguageRegion::Ko.code(),
                writing_mode: WritingMode::Vertical,
            })
        );
    }

    #[test]
    fn 零と未知bitを持つ範囲maskを拒む() {
        let scalar = u32::from('A');
        for (bad_range, expected) in [
            (
                contextual_range(
                    scalar,
                    scalar,
                    1,
                    ProviderRegionMask::from_wire(0),
                    ProviderWritingModeMask::all(),
                ),
                ScriptSpacingTableError::InvalidRangeRegionMask {
                    range_index: 0,
                    mask: 0,
                },
            ),
            (
                contextual_range(
                    scalar,
                    scalar,
                    1,
                    ProviderRegionMask::from_wire(1 << REGION_COUNT),
                    ProviderWritingModeMask::all(),
                ),
                ScriptSpacingTableError::InvalidRangeRegionMask {
                    range_index: 0,
                    mask: 1 << REGION_COUNT,
                },
            ),
            (
                contextual_range(
                    scalar,
                    scalar,
                    1,
                    ProviderRegionMask::all(),
                    ProviderWritingModeMask::from_wire(0),
                ),
                ScriptSpacingTableError::InvalidRangeWritingModeMask {
                    range_index: 0,
                    mask: 0,
                },
            ),
            (
                contextual_range(
                    scalar,
                    scalar,
                    1,
                    ProviderRegionMask::all(),
                    ProviderWritingModeMask::from_wire(1 << WRITING_MODE_COUNT),
                ),
                ScriptSpacingTableError::InvalidRangeWritingModeMask {
                    range_index: 0,
                    mask: 1 << WRITING_MODE_COUNT,
                },
            ),
        ] {
            assert_eq!(
                CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                    1,
                    vec![bad_range],
                    Vec::new(),
                )),
                Err(expected)
            );
        }
    }

    #[test]
    fn 文字クラス数零と上限超過を拒む() {
        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                0,
                Vec::new(),
                Vec::new()
            )),
            Err(ScriptSpacingTableError::EmptyClassSet)
        );
        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                MAX_SCRIPT_SPACING_CLASSES + 1,
                Vec::new(),
                Vec::new()
            )),
            Err(ScriptSpacingTableError::TooManyClasses {
                actual: MAX_SCRIPT_SPACING_CLASSES + 1,
                maximum: MAX_SCRIPT_SPACING_CLASSES,
            })
        );
    }

    #[test]
    fn 範囲数と規則数の上限超過を割当て前に拒む() {
        let ranges = vec![range(0x41, 0x41, 1); MAX_SCRIPT_SPACING_RANGES + 1];
        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                1,
                ranges,
                Vec::new()
            )),
            Err(ScriptSpacingTableError::TooManyRanges {
                actual: MAX_SCRIPT_SPACING_RANGES + 1,
                maximum: MAX_SCRIPT_SPACING_RANGES,
            })
        );

        let rules = vec![
            rule(
                1,
                1,
                LanguageRegion::Und,
                WritingMode::Horizontal,
                ScriptSpacingActionProposal::BuiltInFallback,
            );
            MAX_SCRIPT_SPACING_RULES + 1
        ];
        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                1,
                Vec::new(),
                rules
            )),
            Err(ScriptSpacingTableError::TooManyRules {
                actual: MAX_SCRIPT_SPACING_RULES + 1,
                maximum: MAX_SCRIPT_SPACING_RULES,
            })
        );
    }

    #[test]
    fn 逆転範囲代理対範囲外符号位置を拒む() {
        for (bad_range, expected) in [
            (
                range(0x42, 0x41, 1),
                ScriptSpacingTableError::ReversedScalarRange { range_index: 0 },
            ),
            (
                range(0xd800, 0xd800, 1),
                ScriptSpacingTableError::NonScalarRange { range_index: 0 },
            ),
            (
                range(0xd7ff, 0xe000, 1),
                ScriptSpacingTableError::NonScalarRange { range_index: 0 },
            ),
            (
                range(0x110000, 0x110000, 1),
                ScriptSpacingTableError::NonScalarRange { range_index: 0 },
            ),
        ] {
            assert_eq!(
                CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                    1,
                    vec![bad_range],
                    Vec::new()
                )),
                Err(expected)
            );
        }
    }

    #[test]
    fn 重複範囲と範囲外文字クラスを拒む() {
        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                1,
                vec![range(0x50, 0x60, 1), range(0x40, 0x50, 1)],
                Vec::new()
            )),
            Err(ScriptSpacingTableError::OverlappingScalarRanges {
                first_range_index: 1,
                second_range_index: 0,
                region_code: LanguageRegion::Und.code(),
                writing_mode: WritingMode::Horizontal,
            })
        );
        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                1,
                vec![range(0x41, 0x5a, 2)],
                Vec::new()
            )),
            Err(ScriptSpacingTableError::RangeClassOutOfBounds {
                range_index: 0,
                class: 2,
                class_count: 1,
            })
        );
    }

    #[test]
    fn 範囲外文字クラスと重複する対規則を拒む() {
        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                1,
                Vec::new(),
                vec![rule(
                    1,
                    2,
                    LanguageRegion::Ja,
                    WritingMode::Horizontal,
                    ScriptSpacingActionProposal::NoAutomaticSpace,
                )]
            )),
            Err(ScriptSpacingTableError::RuleClassOutOfBounds {
                rule_index: 0,
                class: 2,
                class_count: 1,
            })
        );

        let duplicate = rule(
            1,
            1,
            LanguageRegion::Ja,
            WritingMode::Horizontal,
            ScriptSpacingActionProposal::KanjiSkip,
        );
        assert_eq!(
            CompiledScriptSpacingTable::compile(ScriptSpacingTableProposal::new(
                1,
                Vec::new(),
                vec![
                    duplicate,
                    ScriptPairRuleProposal {
                        action: ScriptSpacingActionProposal::XKanjiSkip,
                        ..duplicate
                    },
                ]
            )),
            Err(ScriptSpacingTableError::DuplicatePairRules {
                first_rule_index: 0,
                second_rule_index: 1,
            })
        );
    }

    #[test]
    fn 固定糊の次数と寸法範囲を検証する() {
        let bad_order = ScriptSpacingTableProposal::new(
            1,
            Vec::new(),
            vec![rule(
                1,
                1,
                LanguageRegion::Ja,
                WritingMode::Horizontal,
                ScriptSpacingActionProposal::FixedGlue(FixedGlueProposal::new(0, 0, 4, 0, 0)),
            )],
        );
        assert_eq!(
            CompiledScriptSpacingTable::compile(bad_order),
            Err(ScriptSpacingTableError::InvalidGlueOrder {
                rule_index: 0,
                component: GlueComponent::Stretch,
                order: 4,
            })
        );

        let overflow = ScriptSpacingTableProposal::new(
            1,
            Vec::new(),
            vec![rule(
                1,
                1,
                LanguageRegion::Ja,
                WritingMode::Horizontal,
                ScriptSpacingActionProposal::FixedGlue(FixedGlueProposal::new(
                    MAX_DIMEN as i64 + 1,
                    0,
                    0,
                    0,
                    0,
                )),
            )],
        );
        assert_eq!(
            CompiledScriptSpacingTable::compile(overflow),
            Err(ScriptSpacingTableError::GlueComponentOutOfBounds {
                rule_index: 0,
                component: GlueComponent::Width,
                value: MAX_DIMEN as i64 + 1,
            })
        );
    }

    #[test]
    fn 不正提案では使用中の表を部分更新しない() {
        let mut table = CompiledScriptSpacingTable::compile(最小の正しい提案()).unwrap();
        let before = table.clone();
        let bad = ScriptSpacingTableProposal::new(
            1,
            vec![range(0x41, 0x50, 1), range(0x50, 0x60, 1)],
            Vec::new(),
        );
        assert!(table.try_replace(bad).is_err());
        assert_eq!(table, before);
    }
}

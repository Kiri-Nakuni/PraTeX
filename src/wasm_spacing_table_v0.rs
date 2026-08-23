//! ABI 0.0 `SpacingTableUpload` の host-owned domain 境界。
//!
//! wire decoder や Vaak adapter は、外部表現をこの proposal へ写してから同じ validator を
//! 通す。validator は全 record を一時領域で検証・正規化し終えた場合だけ
//! [`ValidatedSpacingProfileProposalV0`] を返すため、途中までの table を公開できない。
//!
//! この型はproductionの[`crate::script_spacing::CompiledScriptSpacingTable`]ではない。W0-Dでは
//! canonical候補をraw proposalへ戻して再検証せず、この型を一方向にconsumeするcompilerへ
//! 現行native tableを合流させる。

use crate::script_spacing::{
    MAX_SCRIPT_SPACING_CLASSES, MAX_SCRIPT_SPACING_RANGES, MAX_SCRIPT_SPACING_RULES,
};
use crate::spacing_table_domain::{
    find_contextual_pair_overlap, find_contextual_scalar_overlap, provider_class_index,
    valid_nonzero_mask, ContextOverlapSearchError, ContextualPairKey, ContextualScalarRange,
    ScalarRangeDomainError, VALID_LANGUAGE_REGION_MASK, VALID_WRITING_MODE_MASK,
};

const LAYOUT_SCHEMA_V0: u32 = 0;
const SPACING_CLASS_RANGE_RECORD_BYTES_V0: u64 = 24;
const SPACING_PAIR_RULE_RECORD_BYTES_V0: u64 = 88;

/// ABI 0.0 が engine-native な固定 tableへ写せる調整優先度。
///
/// wire field は将来の schema 拡張用に `u16` だが、schema 0は小整数tableの indexとして
/// 下位8 bitだけを使う。256以上を黙って別の優先度へ丸めない。
pub(crate) const MAX_ADJUSTMENT_TIER_V0: u16 = u8::MAX as u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpacingTableConfigV0 {
    layout_schema: u32,
    max_classes: u32,
    max_ranges: u32,
    max_rules: u32,
    max_reason_ids: u32,
    allowed_region_mask: u32,
    allowed_writing_mode_mask: u32,
}

impl SpacingTableConfigV0 {
    pub(crate) const fn new(
        layout_schema: u32,
        max_classes: u32,
        max_ranges: u32,
        max_rules: u32,
        max_reason_ids: u32,
        allowed_region_mask: u32,
        allowed_writing_mode_mask: u32,
    ) -> Self {
        Self {
            layout_schema,
            max_classes,
            max_ranges,
            max_rules,
            max_reason_ids,
            allowed_region_mask,
            allowed_writing_mode_mask,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpacingLengthProposalV0 {
    numerator: i64,
    denominator: u32,
    basis: u16,
    flags: u16,
}

impl SpacingLengthProposalV0 {
    pub(crate) const fn new(numerator: i64, denominator: u32, basis: u16, flags: u16) -> Self {
        Self {
            numerator,
            denominator,
            basis,
            flags,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpacingClassRangeProposalV0 {
    first_scalar: u32,
    last_scalar_inclusive: u32,
    class_id: u32,
    region_mask: u32,
    writing_mode_mask: u32,
}

impl SpacingClassRangeProposalV0 {
    pub(crate) const fn new(
        first_scalar: u32,
        last_scalar_inclusive: u32,
        class_id: u32,
        region_mask: u32,
        writing_mode_mask: u32,
    ) -> Self {
        Self {
            first_scalar,
            last_scalar_inclusive,
            class_id,
            region_mask,
            writing_mode_mask,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpacingPairRuleProposalV0 {
    left_class_id: u32,
    right_class_id: u32,
    region_mask: u32,
    writing_mode_mask: u32,
    natural: SpacingLengthProposalV0,
    shrink_limit: SpacingLengthProposalV0,
    stretch_limit: SpacingLengthProposalV0,
    shrink_tier: u16,
    stretch_tier: u16,
    break_rule: u16,
    line_edge_rule: u16,
    penalty: i32,
    reason_id: u32,
    flags: u32,
}

impl SpacingPairRuleProposalV0 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        left_class_id: u32,
        right_class_id: u32,
        region_mask: u32,
        writing_mode_mask: u32,
        natural: SpacingLengthProposalV0,
        shrink_limit: SpacingLengthProposalV0,
        stretch_limit: SpacingLengthProposalV0,
        shrink_tier: u16,
        stretch_tier: u16,
        break_rule: u16,
        line_edge_rule: u16,
        penalty: i32,
        reason_id: u32,
        flags: u32,
    ) -> Self {
        Self {
            left_class_id,
            right_class_id,
            region_mask,
            writing_mode_mask,
            natural,
            shrink_limit,
            stretch_limit,
            shrink_tier,
            stretch_tier,
            break_rule,
            line_edge_rule,
            penalty,
            reason_id,
            flags,
        }
    }
}

/// Vaak・WASM・試験adapterが共有する、外部所有物を含まないspacing proposal。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpacingTableProposalV0 {
    ranges: Vec<SpacingClassRangeProposalV0>,
    rules: Vec<SpacingPairRuleProposalV0>,
}

impl SpacingTableProposalV0 {
    pub(crate) fn new(
        ranges: Vec<SpacingClassRangeProposalV0>,
        rules: Vec<SpacingPairRuleProposalV0>,
    ) -> Self {
        Self { ranges, rules }
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SpacingLengthBasisV0 {
    AbsoluteScaledPoint = 0,
    LeftEm = 1,
    RightEm = 2,
    LeftZw = 3,
    RightZw = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CanonicalSpacingLengthV0 {
    numerator: i64,
    denominator: u32,
    basis: SpacingLengthBasisV0,
}

impl CanonicalSpacingLengthV0 {
    pub(crate) const fn numerator(self) -> i64 {
        self.numerator
    }

    pub(crate) const fn denominator(self) -> u32 {
        self.denominator
    }

    pub(crate) const fn basis(self) -> SpacingLengthBasisV0 {
        self.basis
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SpacingBreakRuleV0 {
    UseBuiltIn = 0,
    Allow = 1,
    Forbid = 2,
    Penalty = 3,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SpacingLineEdgeRuleV0 {
    UseBuiltIn = 0,
    Retain = 1,
    DiscardAtStart = 2,
    DiscardAtEnd = 3,
    DiscardAtBoth = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ValidatedSpacingClassRangeV0 {
    first_scalar: u32,
    last_scalar_inclusive: u32,
    class_id: u32,
    region_mask: u32,
    writing_mode_mask: u32,
}

impl ValidatedSpacingClassRangeV0 {
    pub(crate) const fn first_scalar(self) -> u32 {
        self.first_scalar
    }

    pub(crate) const fn last_scalar_inclusive(self) -> u32 {
        self.last_scalar_inclusive
    }

    pub(crate) const fn class_id(self) -> u32 {
        self.class_id
    }

    pub(crate) const fn region_mask(self) -> u32 {
        self.region_mask
    }

    pub(crate) const fn writing_mode_mask(self) -> u32 {
        self.writing_mode_mask
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ValidatedSpacingPairRuleV0 {
    left_class_id: u32,
    right_class_id: u32,
    region_mask: u32,
    writing_mode_mask: u32,
    natural: CanonicalSpacingLengthV0,
    shrink_limit: CanonicalSpacingLengthV0,
    stretch_limit: CanonicalSpacingLengthV0,
    shrink_tier: u16,
    stretch_tier: u16,
    break_rule: SpacingBreakRuleV0,
    line_edge_rule: SpacingLineEdgeRuleV0,
    penalty: i32,
    reason_id: u32,
}

impl ValidatedSpacingPairRuleV0 {
    pub(crate) const fn class_pair(self) -> (u32, u32) {
        (self.left_class_id, self.right_class_id)
    }

    pub(crate) const fn context_masks(self) -> (u32, u32) {
        (self.region_mask, self.writing_mode_mask)
    }

    pub(crate) const fn lengths(
        self,
    ) -> (
        CanonicalSpacingLengthV0,
        CanonicalSpacingLengthV0,
        CanonicalSpacingLengthV0,
    ) {
        (self.natural, self.shrink_limit, self.stretch_limit)
    }

    pub(crate) const fn tiers(self) -> (u16, u16) {
        (self.shrink_tier, self.stretch_tier)
    }

    pub(crate) const fn break_rule(self) -> SpacingBreakRuleV0 {
        self.break_rule
    }

    pub(crate) const fn line_edge_rule(self) -> SpacingLineEdgeRuleV0 {
        self.line_edge_rule
    }

    pub(crate) const fn penalty(self) -> i32 {
        self.penalty
    }

    pub(crate) const fn reason_id(self) -> u32 {
        self.reason_id
    }
}

/// 全件検証・有理数既約化・canonical整列を終えた場合だけ構築できるtable候補。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedSpacingProfileProposalV0 {
    ranges: Vec<ValidatedSpacingClassRangeV0>,
    rules: Vec<ValidatedSpacingPairRuleV0>,
    record_bytes: u64,
}

impl ValidatedSpacingProfileProposalV0 {
    pub(crate) fn ranges(&self) -> &[ValidatedSpacingClassRangeV0] {
        &self.ranges
    }

    pub(crate) fn rules(&self) -> &[ValidatedSpacingPairRuleV0] {
        &self.rules
    }

    pub(crate) const fn record_bytes(&self) -> u64 {
        self.record_bytes
    }

    /// W0-D compilerへ検証済み値を一方向に渡す。raw proposalを再構築する必要はない。
    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<ValidatedSpacingClassRangeV0>,
        Vec<ValidatedSpacingPairRuleV0>,
        u64,
    ) {
        (self.ranges, self.rules, self.record_bytes)
    }

    /// 新しいproposalを先に全件検証し、成功時だけ現在値と交換する。
    pub(crate) fn try_replace(
        &mut self,
        config: SpacingTableConfigV0,
        max_record_bytes: u64,
        proposal: SpacingTableProposalV0,
    ) -> Result<(), SpacingTableValidationErrorV0> {
        let replacement = validate_spacing_table_proposal_v0(config, max_record_bytes, proposal)?;
        *self = replacement;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpacingTableLimitV0 {
    Classes,
    Ranges,
    Rules,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpacingPairSideV0 {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpacingLengthComponentV0 {
    Natural,
    ShrinkLimit,
    StretchLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpacingTierComponentV0 {
    Shrink,
    Stretch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SpacingTableValidationErrorV0 {
    UnsupportedLayoutSchema {
        actual: u32,
    },
    EmptyClassLimit,
    ConfiguredLimitTooLarge {
        limit: SpacingTableLimitV0,
        actual: u64,
        maximum: u64,
    },
    InvalidAllowedRegionMask {
        mask: u32,
    },
    InvalidAllowedWritingModeMask {
        mask: u32,
    },
    TooManyRanges {
        actual: u64,
        maximum: u32,
    },
    TooManyRules {
        actual: u64,
        maximum: u32,
    },
    RecordBytesOverflow,
    TooManyRecordBytes {
        actual: u64,
        maximum: u64,
    },
    AllocationFailed,
    ReversedScalarRange {
        range_index: usize,
    },
    NonScalarRange {
        range_index: usize,
    },
    RangeClassOutOfBounds {
        range_index: usize,
        class_id: u32,
        maximum: u32,
    },
    InvalidRangeRegionMask {
        range_index: usize,
        mask: u32,
        allowed: u32,
    },
    InvalidRangeWritingModeMask {
        range_index: usize,
        mask: u32,
        allowed: u32,
    },
    OverlappingScalarRanges {
        first_range_index: usize,
        second_range_index: usize,
        region_code: u8,
        writing_mode: u8,
    },
    RuleClassOutOfBounds {
        rule_index: usize,
        side: SpacingPairSideV0,
        class_id: u32,
        maximum: u32,
    },
    InvalidRuleRegionMask {
        rule_index: usize,
        mask: u32,
        allowed: u32,
    },
    InvalidRuleWritingModeMask {
        rule_index: usize,
        mask: u32,
        allowed: u32,
    },
    OverlappingPairRules {
        first_rule_index: usize,
        second_rule_index: usize,
        region_code: u8,
        writing_mode: u8,
    },
    ZeroLengthDenominator {
        rule_index: usize,
        component: SpacingLengthComponentV0,
    },
    UnknownLengthBasis {
        rule_index: usize,
        component: SpacingLengthComponentV0,
        basis: u16,
    },
    NonZeroLengthFlags {
        rule_index: usize,
        component: SpacingLengthComponentV0,
        flags: u16,
    },
    NegativeLengthLimit {
        rule_index: usize,
        component: SpacingLengthComponentV0,
        numerator: i64,
    },
    InvalidTier {
        rule_index: usize,
        component: SpacingTierComponentV0,
        tier: u16,
        maximum: u16,
    },
    UnknownBreakRule {
        rule_index: usize,
        value: u16,
    },
    PenaltyWithoutPenaltyRule {
        rule_index: usize,
        penalty: i32,
    },
    UnknownLineEdgeRule {
        rule_index: usize,
        value: u16,
    },
    ReasonIdOutOfBounds {
        rule_index: usize,
        reason_id: u32,
        maximum_exclusive: u32,
    },
    NonZeroRuleFlags {
        rule_index: usize,
        flags: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexedRangeV0 {
    source_index: usize,
    value: ValidatedSpacingClassRangeV0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexedRuleV0 {
    source_index: usize,
    value: ValidatedSpacingPairRuleV0,
}

/// `SpacingTableUpload` responseのdomain値を一括検証する。
///
/// `max_record_bytes` は `SpacingClassRangeV0` と `SpacingPairRuleV0` sectionの合計である。
/// envelope等を含むmailbox全体の上限はwire validatorが別に先行して検査する。
pub(crate) fn validate_spacing_table_proposal_v0(
    config: SpacingTableConfigV0,
    max_record_bytes: u64,
    proposal: SpacingTableProposalV0,
) -> Result<ValidatedSpacingProfileProposalV0, SpacingTableValidationErrorV0> {
    validate_config(config)?;
    let record_bytes = validate_proposal_size(config, max_record_bytes, &proposal)?;

    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(proposal.ranges.len())
        .map_err(|_| SpacingTableValidationErrorV0::AllocationFailed)?;
    for (range_index, range) in proposal.ranges.into_iter().enumerate() {
        ranges.push(IndexedRangeV0 {
            source_index: range_index,
            value: validate_range(config, range_index, range)?,
        });
    }
    ranges.sort_unstable_by_key(|entry| entry.value);
    validate_range_overlaps(&ranges)?;

    let mut rules = Vec::new();
    rules
        .try_reserve_exact(proposal.rules.len())
        .map_err(|_| SpacingTableValidationErrorV0::AllocationFailed)?;
    for (rule_index, rule) in proposal.rules.into_iter().enumerate() {
        rules.push(IndexedRuleV0 {
            source_index: rule_index,
            value: validate_rule(config, rule_index, rule)?,
        });
    }
    rules.sort_unstable_by_key(|entry| {
        (
            entry.value.left_class_id,
            entry.value.right_class_id,
            entry.value.region_mask,
            entry.value.writing_mode_mask,
        )
    });
    validate_rule_overlaps(&rules)?;

    let mut validated_ranges = Vec::new();
    validated_ranges
        .try_reserve_exact(ranges.len())
        .map_err(|_| SpacingTableValidationErrorV0::AllocationFailed)?;
    validated_ranges.extend(ranges.into_iter().map(|entry| entry.value));
    let mut validated_rules = Vec::new();
    validated_rules
        .try_reserve_exact(rules.len())
        .map_err(|_| SpacingTableValidationErrorV0::AllocationFailed)?;
    validated_rules.extend(rules.into_iter().map(|entry| entry.value));

    Ok(ValidatedSpacingProfileProposalV0 {
        ranges: validated_ranges,
        rules: validated_rules,
        record_bytes,
    })
}

fn validate_config(config: SpacingTableConfigV0) -> Result<(), SpacingTableValidationErrorV0> {
    if config.layout_schema != LAYOUT_SCHEMA_V0 {
        return Err(SpacingTableValidationErrorV0::UnsupportedLayoutSchema {
            actual: config.layout_schema,
        });
    }
    if config.max_classes == 0 {
        return Err(SpacingTableValidationErrorV0::EmptyClassLimit);
    }
    validate_configured_limit(
        SpacingTableLimitV0::Classes,
        u64::from(config.max_classes),
        u64::from(MAX_SCRIPT_SPACING_CLASSES),
    )?;
    validate_configured_limit(
        SpacingTableLimitV0::Ranges,
        u64::from(config.max_ranges),
        MAX_SCRIPT_SPACING_RANGES as u64,
    )?;
    validate_configured_limit(
        SpacingTableLimitV0::Rules,
        u64::from(config.max_rules),
        MAX_SCRIPT_SPACING_RULES as u64,
    )?;
    if !valid_nonzero_mask(config.allowed_region_mask, VALID_LANGUAGE_REGION_MASK) {
        return Err(SpacingTableValidationErrorV0::InvalidAllowedRegionMask {
            mask: config.allowed_region_mask,
        });
    }
    if !valid_nonzero_mask(config.allowed_writing_mode_mask, VALID_WRITING_MODE_MASK) {
        return Err(
            SpacingTableValidationErrorV0::InvalidAllowedWritingModeMask {
                mask: config.allowed_writing_mode_mask,
            },
        );
    }
    Ok(())
}

fn validate_configured_limit(
    limit: SpacingTableLimitV0,
    actual: u64,
    maximum: u64,
) -> Result<(), SpacingTableValidationErrorV0> {
    if actual > maximum {
        Err(SpacingTableValidationErrorV0::ConfiguredLimitTooLarge {
            limit,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn validate_proposal_size(
    config: SpacingTableConfigV0,
    max_record_bytes: u64,
    proposal: &SpacingTableProposalV0,
) -> Result<u64, SpacingTableValidationErrorV0> {
    let range_count = proposal.ranges.len() as u64;
    if range_count > u64::from(config.max_ranges) {
        return Err(SpacingTableValidationErrorV0::TooManyRanges {
            actual: range_count,
            maximum: config.max_ranges,
        });
    }
    let rule_count = proposal.rules.len() as u64;
    if rule_count > u64::from(config.max_rules) {
        return Err(SpacingTableValidationErrorV0::TooManyRules {
            actual: rule_count,
            maximum: config.max_rules,
        });
    }
    let record_bytes = range_count
        .checked_mul(SPACING_CLASS_RANGE_RECORD_BYTES_V0)
        .and_then(|bytes| {
            rule_count
                .checked_mul(SPACING_PAIR_RULE_RECORD_BYTES_V0)
                .and_then(|rule_bytes| bytes.checked_add(rule_bytes))
        })
        .ok_or(SpacingTableValidationErrorV0::RecordBytesOverflow)?;
    if record_bytes > max_record_bytes {
        return Err(SpacingTableValidationErrorV0::TooManyRecordBytes {
            actual: record_bytes,
            maximum: max_record_bytes,
        });
    }
    Ok(record_bytes)
}

fn validate_range(
    config: SpacingTableConfigV0,
    range_index: usize,
    range: SpacingClassRangeProposalV0,
) -> Result<ValidatedSpacingClassRangeV0, SpacingTableValidationErrorV0> {
    match crate::spacing_table_domain::validate_scalar_range(
        range.first_scalar,
        range.last_scalar_inclusive,
    ) {
        Ok(()) => {}
        Err(ScalarRangeDomainError::Reversed) => {
            return Err(SpacingTableValidationErrorV0::ReversedScalarRange { range_index })
        }
        Err(ScalarRangeDomainError::NonScalar) => {
            return Err(SpacingTableValidationErrorV0::NonScalarRange { range_index })
        }
    }
    if provider_class_index(range.class_id, config.max_classes).is_none() {
        return Err(SpacingTableValidationErrorV0::RangeClassOutOfBounds {
            range_index,
            class_id: range.class_id,
            maximum: config.max_classes,
        });
    }
    if !valid_nonzero_mask(range.region_mask, VALID_LANGUAGE_REGION_MASK)
        || range.region_mask & !config.allowed_region_mask != 0
    {
        return Err(SpacingTableValidationErrorV0::InvalidRangeRegionMask {
            range_index,
            mask: range.region_mask,
            allowed: config.allowed_region_mask,
        });
    }
    if !valid_nonzero_mask(range.writing_mode_mask, VALID_WRITING_MODE_MASK)
        || range.writing_mode_mask & !config.allowed_writing_mode_mask != 0
    {
        return Err(SpacingTableValidationErrorV0::InvalidRangeWritingModeMask {
            range_index,
            mask: range.writing_mode_mask,
            allowed: config.allowed_writing_mode_mask,
        });
    }
    Ok(ValidatedSpacingClassRangeV0 {
        first_scalar: range.first_scalar,
        last_scalar_inclusive: range.last_scalar_inclusive,
        class_id: range.class_id,
        region_mask: range.region_mask,
        writing_mode_mask: range.writing_mode_mask,
    })
}

fn validate_range_overlaps(ranges: &[IndexedRangeV0]) -> Result<(), SpacingTableValidationErrorV0> {
    let overlap =
        find_contextual_scalar_overlap(ranges.iter().map(|range| ContextualScalarRange {
            source_index: range.source_index,
            first: range.value.first_scalar,
            last_inclusive: range.value.last_scalar_inclusive,
            region_mask: range.value.region_mask,
            writing_mode_mask: range.value.writing_mode_mask,
        }))
        .map_err(map_overlap_search_error)?;
    if let Some(overlap) = overlap {
        return Err(SpacingTableValidationErrorV0::OverlappingScalarRanges {
            first_range_index: overlap.first_source_index,
            second_range_index: overlap.second_source_index,
            region_code: overlap.region_code,
            writing_mode: overlap.writing_mode.code(),
        });
    }
    Ok(())
}

fn validate_rule(
    config: SpacingTableConfigV0,
    rule_index: usize,
    rule: SpacingPairRuleProposalV0,
) -> Result<ValidatedSpacingPairRuleV0, SpacingTableValidationErrorV0> {
    validate_rule_class(
        config,
        rule_index,
        SpacingPairSideV0::Left,
        rule.left_class_id,
    )?;
    validate_rule_class(
        config,
        rule_index,
        SpacingPairSideV0::Right,
        rule.right_class_id,
    )?;
    if !valid_nonzero_mask(rule.region_mask, VALID_LANGUAGE_REGION_MASK)
        || rule.region_mask & !config.allowed_region_mask != 0
    {
        return Err(SpacingTableValidationErrorV0::InvalidRuleRegionMask {
            rule_index,
            mask: rule.region_mask,
            allowed: config.allowed_region_mask,
        });
    }
    if !valid_nonzero_mask(rule.writing_mode_mask, VALID_WRITING_MODE_MASK)
        || rule.writing_mode_mask & !config.allowed_writing_mode_mask != 0
    {
        return Err(SpacingTableValidationErrorV0::InvalidRuleWritingModeMask {
            rule_index,
            mask: rule.writing_mode_mask,
            allowed: config.allowed_writing_mode_mask,
        });
    }
    let natural = validate_length(rule_index, SpacingLengthComponentV0::Natural, rule.natural)?;
    let shrink_limit = validate_length(
        rule_index,
        SpacingLengthComponentV0::ShrinkLimit,
        rule.shrink_limit,
    )?;
    let stretch_limit = validate_length(
        rule_index,
        SpacingLengthComponentV0::StretchLimit,
        rule.stretch_limit,
    )?;
    validate_tier(rule_index, SpacingTierComponentV0::Shrink, rule.shrink_tier)?;
    validate_tier(
        rule_index,
        SpacingTierComponentV0::Stretch,
        rule.stretch_tier,
    )?;
    let break_rule = match rule.break_rule {
        0 => SpacingBreakRuleV0::UseBuiltIn,
        1 => SpacingBreakRuleV0::Allow,
        2 => SpacingBreakRuleV0::Forbid,
        3 => SpacingBreakRuleV0::Penalty,
        value => return Err(SpacingTableValidationErrorV0::UnknownBreakRule { rule_index, value }),
    };
    if break_rule != SpacingBreakRuleV0::Penalty && rule.penalty != 0 {
        return Err(SpacingTableValidationErrorV0::PenaltyWithoutPenaltyRule {
            rule_index,
            penalty: rule.penalty,
        });
    }
    let line_edge_rule = match rule.line_edge_rule {
        0 => SpacingLineEdgeRuleV0::UseBuiltIn,
        1 => SpacingLineEdgeRuleV0::Retain,
        2 => SpacingLineEdgeRuleV0::DiscardAtStart,
        3 => SpacingLineEdgeRuleV0::DiscardAtEnd,
        4 => SpacingLineEdgeRuleV0::DiscardAtBoth,
        value => {
            return Err(SpacingTableValidationErrorV0::UnknownLineEdgeRule { rule_index, value })
        }
    };
    if rule.reason_id >= config.max_reason_ids {
        return Err(SpacingTableValidationErrorV0::ReasonIdOutOfBounds {
            rule_index,
            reason_id: rule.reason_id,
            maximum_exclusive: config.max_reason_ids,
        });
    }
    if rule.flags != 0 {
        return Err(SpacingTableValidationErrorV0::NonZeroRuleFlags {
            rule_index,
            flags: rule.flags,
        });
    }
    Ok(ValidatedSpacingPairRuleV0 {
        left_class_id: rule.left_class_id,
        right_class_id: rule.right_class_id,
        region_mask: rule.region_mask,
        writing_mode_mask: rule.writing_mode_mask,
        natural,
        shrink_limit,
        stretch_limit,
        shrink_tier: rule.shrink_tier,
        stretch_tier: rule.stretch_tier,
        break_rule,
        line_edge_rule,
        penalty: rule.penalty,
        reason_id: rule.reason_id,
    })
}

fn validate_rule_class(
    config: SpacingTableConfigV0,
    rule_index: usize,
    side: SpacingPairSideV0,
    class_id: u32,
) -> Result<(), SpacingTableValidationErrorV0> {
    if provider_class_index(class_id, config.max_classes).is_none() {
        Err(SpacingTableValidationErrorV0::RuleClassOutOfBounds {
            rule_index,
            side,
            class_id,
            maximum: config.max_classes,
        })
    } else {
        Ok(())
    }
}

fn validate_rule_overlaps(rules: &[IndexedRuleV0]) -> Result<(), SpacingTableValidationErrorV0> {
    let overlap = find_contextual_pair_overlap(rules.iter().map(|rule| ContextualPairKey {
        source_index: rule.source_index,
        left_class_id: rule.value.left_class_id,
        right_class_id: rule.value.right_class_id,
        region_mask: rule.value.region_mask,
        writing_mode_mask: rule.value.writing_mode_mask,
    }))
    .map_err(map_overlap_search_error)?;
    if let Some(overlap) = overlap {
        return Err(SpacingTableValidationErrorV0::OverlappingPairRules {
            first_rule_index: overlap.first_source_index,
            second_rule_index: overlap.second_source_index,
            region_code: overlap.region_code,
            writing_mode: overlap.writing_mode.code(),
        });
    }
    Ok(())
}

fn map_overlap_search_error(error: ContextOverlapSearchError) -> SpacingTableValidationErrorV0 {
    match error {
        ContextOverlapSearchError::AllocationFailed => {
            SpacingTableValidationErrorV0::AllocationFailed
        }
    }
}

fn validate_length(
    rule_index: usize,
    component: SpacingLengthComponentV0,
    length: SpacingLengthProposalV0,
) -> Result<CanonicalSpacingLengthV0, SpacingTableValidationErrorV0> {
    if length.denominator == 0 {
        return Err(SpacingTableValidationErrorV0::ZeroLengthDenominator {
            rule_index,
            component,
        });
    }
    let basis = match length.basis {
        0 => SpacingLengthBasisV0::AbsoluteScaledPoint,
        1 => SpacingLengthBasisV0::LeftEm,
        2 => SpacingLengthBasisV0::RightEm,
        3 => SpacingLengthBasisV0::LeftZw,
        4 => SpacingLengthBasisV0::RightZw,
        basis => {
            return Err(SpacingTableValidationErrorV0::UnknownLengthBasis {
                rule_index,
                component,
                basis,
            })
        }
    };
    if length.flags != 0 {
        return Err(SpacingTableValidationErrorV0::NonZeroLengthFlags {
            rule_index,
            component,
            flags: length.flags,
        });
    }
    if component != SpacingLengthComponentV0::Natural && length.numerator < 0 {
        return Err(SpacingTableValidationErrorV0::NegativeLengthLimit {
            rule_index,
            component,
            numerator: length.numerator,
        });
    }
    let divisor =
        greatest_common_divisor(length.numerator.unsigned_abs(), length.denominator.into());
    let divisor_i64 = divisor as i64;
    Ok(CanonicalSpacingLengthV0 {
        numerator: length.numerator / divisor_i64,
        denominator: length.denominator / divisor as u32,
        basis,
    })
}

fn validate_tier(
    rule_index: usize,
    component: SpacingTierComponentV0,
    tier: u16,
) -> Result<(), SpacingTableValidationErrorV0> {
    if tier > MAX_ADJUSTMENT_TIER_V0 {
        Err(SpacingTableValidationErrorV0::InvalidTier {
            rule_index,
            component,
            tier,
            maximum: MAX_ADJUSTMENT_TIER_V0,
        })
    } else {
        Ok(())
    }
}

fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_RECORD_BYTES: u64 = 1_000_000;

    fn config() -> SpacingTableConfigV0 {
        SpacingTableConfigV0::new(
            0,
            4,
            8,
            8,
            4,
            VALID_LANGUAGE_REGION_MASK,
            VALID_WRITING_MODE_MASK,
        )
    }

    fn length(numerator: i64, denominator: u32) -> SpacingLengthProposalV0 {
        SpacingLengthProposalV0::new(numerator, denominator, 0, 0)
    }

    fn range(scalar: u32, class_id: u32) -> SpacingClassRangeProposalV0 {
        SpacingClassRangeProposalV0::new(scalar, scalar, class_id, 1, 1)
    }

    fn rule(left: u32, right: u32) -> SpacingPairRuleProposalV0 {
        SpacingPairRuleProposalV0::new(
            left,
            right,
            1,
            1,
            length(0, 1),
            length(0, 1),
            length(0, 1),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        )
    }

    fn validate(
        ranges: Vec<SpacingClassRangeProposalV0>,
        rules: Vec<SpacingPairRuleProposalV0>,
    ) -> Result<ValidatedSpacingProfileProposalV0, SpacingTableValidationErrorV0> {
        validate_spacing_table_proposal_v0(
            config(),
            ALL_RECORD_BYTES,
            SpacingTableProposalV0::new(ranges, rules),
        )
    }

    #[test]
    fn unicode_scalarの境界とsurrogate全域を区別する() {
        assert!(validate(
            vec![range(0, 1), range(0xd7ff, 1), range(0xe000, 1)],
            vec![]
        )
        .is_ok());
        assert!(validate(vec![range(0x10_ffff, 1)], vec![]).is_ok());
        for (first, last) in [
            (0xd800, 0xd800),
            (0xdfff, 0xdfff),
            (0xd7ff, 0xe000),
            (0x11_0000, 0x11_0000),
        ] {
            let result = validate(
                vec![SpacingClassRangeProposalV0::new(first, last, 1, 1, 1)],
                vec![],
            );
            assert!(matches!(
                result,
                Err(SpacingTableValidationErrorV0::NonScalarRange { .. })
            ));
        }
        assert!(matches!(
            validate(
                vec![SpacingClassRangeProposalV0::new(2, 1, 1, 1, 1)],
                vec![]
            ),
            Err(SpacingTableValidationErrorV0::ReversedScalarRange { .. })
        ));
    }

    #[test]
    fn classとmaskの上下限をhost設定内に閉じる() {
        assert!(validate(
            vec![SpacingClassRangeProposalV0::new(
                0x41,
                0x41,
                4,
                VALID_LANGUAGE_REGION_MASK,
                VALID_WRITING_MODE_MASK,
            )],
            vec![rule(4, 4)],
        )
        .is_ok());
        for class_id in [0, 5] {
            assert!(matches!(
                validate(vec![range(0x41, class_id)], vec![]),
                Err(SpacingTableValidationErrorV0::RangeClassOutOfBounds { .. })
            ));
            assert!(matches!(
                validate(vec![], vec![rule(class_id, 1)]),
                Err(SpacingTableValidationErrorV0::RuleClassOutOfBounds { .. })
            ));
        }
        for mask in [
            0,
            VALID_LANGUAGE_REGION_MASK | (1 << crate::spacing_table_domain::LANGUAGE_REGION_COUNT),
        ] {
            assert!(matches!(
                validate(
                    vec![SpacingClassRangeProposalV0::new(0x41, 0x41, 1, mask, 1)],
                    vec![]
                ),
                Err(SpacingTableValidationErrorV0::InvalidRangeRegionMask { .. })
            ));
        }
        let restricted = SpacingTableConfigV0::new(0, 4, 8, 8, 4, 1, 1);
        assert!(matches!(
            validate_spacing_table_proposal_v0(
                restricted,
                ALL_RECORD_BYTES,
                SpacingTableProposalV0::new(
                    vec![SpacingClassRangeProposalV0::new(0x41, 0x41, 1, 2, 1)],
                    vec![],
                ),
            ),
            Err(SpacingTableValidationErrorV0::InvalidRangeRegionMask { .. })
        ));

        let mut invalid_rule_mask = rule(1, 1);
        invalid_rule_mask.writing_mode_mask = 0b10;
        assert!(matches!(
            validate_spacing_table_proposal_v0(
                restricted,
                ALL_RECORD_BYTES,
                SpacingTableProposalV0::new(vec![], vec![invalid_rule_mask]),
            ),
            Err(SpacingTableValidationErrorV0::InvalidRuleWritingModeMask { .. })
        ));
    }

    #[test]
    fn schema設定自体もengineのbounded上限で拒否する() {
        let empty = || SpacingTableProposalV0::new(vec![], vec![]);
        let invalid_configs = [
            (SpacingTableConfigV0::new(1, 1, 0, 0, 0, 1, 1), "schema"),
            (
                SpacingTableConfigV0::new(0, 0, 0, 0, 0, 1, 1),
                "classes zero",
            ),
            (
                SpacingTableConfigV0::new(0, MAX_SCRIPT_SPACING_CLASSES + 1, 0, 0, 0, 1, 1),
                "classes maximum",
            ),
            (
                SpacingTableConfigV0::new(0, 1, 0, 0, 0, 0, 1),
                "region zero",
            ),
            (
                SpacingTableConfigV0::new(
                    0,
                    1,
                    0,
                    0,
                    0,
                    1,
                    1 << crate::spacing_table_domain::WRITING_MODE_COUNT,
                ),
                "writing unknown",
            ),
        ];
        for (invalid, label) in invalid_configs {
            assert!(
                validate_spacing_table_proposal_v0(invalid, 0, empty()).is_err(),
                "{label} must be rejected",
            );
        }
    }

    #[test]
    fn scalar範囲はregionと方向の積が交わる時だけ曖昧になる() {
        let first = SpacingClassRangeProposalV0::new(0x40, 0x50, 1, 0b001, 0b01);
        let disjoint_region = SpacingClassRangeProposalV0::new(0x48, 0x60, 2, 0b010, 0b01);
        let disjoint_mode = SpacingClassRangeProposalV0::new(0x48, 0x60, 2, 0b001, 0b10);
        assert!(validate(vec![first, disjoint_region, disjoint_mode], vec![]).is_ok());

        let ambiguous = SpacingClassRangeProposalV0::new(0x48, 0x60, 2, 0b011, 0b11);
        assert!(matches!(
            validate(vec![first, ambiguous], vec![]),
            Err(SpacingTableValidationErrorV0::OverlappingScalarRanges { .. })
        ));
    }

    #[test]
    fn pair_keyはcontextが一部でも重なる二規則を拒否する() {
        let mut first = rule(1, 2);
        first.region_mask = 0b001;
        first.writing_mode_mask = 0b11;
        let mut disjoint = rule(1, 2);
        disjoint.region_mask = 0b010;
        assert!(validate(vec![], vec![first, disjoint]).is_ok());

        let mut overlapping = rule(1, 2);
        overlapping.region_mask = 0b011;
        overlapping.writing_mode_mask = 0b10;
        assert!(matches!(
            validate(vec![], vec![first, overlapping]),
            Err(SpacingTableValidationErrorV0::OverlappingPairRules { .. })
        ));
    }

    #[test]
    fn lengthは浮動小数点を使わず全境界で既約化する() {
        for numerator in [i64::MIN, -12, -1, 0, 1, 12, i64::MAX] {
            for denominator in [1, 2, 3, u32::MAX] {
                let canonical = validate_length(
                    0,
                    SpacingLengthComponentV0::Natural,
                    length(numerator, denominator),
                )
                .unwrap();
                assert_eq!(
                    i128::from(numerator) * i128::from(canonical.denominator()),
                    i128::from(canonical.numerator()) * i128::from(denominator),
                );
                assert_eq!(
                    greatest_common_divisor(
                        canonical.numerator().unsigned_abs(),
                        canonical.denominator().into(),
                    ),
                    1,
                );
            }
        }
        assert_eq!(
            validate_length(0, SpacingLengthComponentV0::Natural, length(0, u32::MAX),)
                .unwrap()
                .denominator(),
            1,
        );
        assert!(matches!(
            validate_length(0, SpacingLengthComponentV0::Natural, length(1, 0),),
            Err(SpacingTableValidationErrorV0::ZeroLengthDenominator { .. })
        ));
        assert!(matches!(
            validate_length(
                0,
                SpacingLengthComponentV0::Natural,
                SpacingLengthProposalV0::new(1, 1, 5, 0),
            ),
            Err(SpacingTableValidationErrorV0::UnknownLengthBasis { .. })
        ));
        assert!(matches!(
            validate_length(
                0,
                SpacingLengthComponentV0::Natural,
                SpacingLengthProposalV0::new(1, 1, 0, 1),
            ),
            Err(SpacingTableValidationErrorV0::NonZeroLengthFlags { .. })
        ));
        for basis in 0..=4 {
            let canonical = validate_length(
                0,
                SpacingLengthComponentV0::Natural,
                SpacingLengthProposalV0::new(1, 1, basis, 0),
            )
            .unwrap();
            assert_eq!(canonical.basis() as u16, basis);
        }
    }

    #[test]
    fn tierとbreakとedgeとpenaltyとreasonの境界を検証する() {
        let mut boundary = rule(1, 1);
        boundary.shrink_tier = MAX_ADJUSTMENT_TIER_V0;
        boundary.stretch_tier = MAX_ADJUSTMENT_TIER_V0;
        boundary.break_rule = 3;
        boundary.penalty = i32::MIN;
        boundary.line_edge_rule = 4;
        boundary.reason_id = 3;
        assert!(validate(vec![], vec![boundary]).is_ok());

        let mut invalid = boundary;
        invalid.shrink_tier = MAX_ADJUSTMENT_TIER_V0 + 1;
        assert!(matches!(
            validate(vec![], vec![invalid]),
            Err(SpacingTableValidationErrorV0::InvalidTier { .. })
        ));
        invalid = boundary;
        invalid.break_rule = 4;
        assert!(matches!(
            validate(vec![], vec![invalid]),
            Err(SpacingTableValidationErrorV0::UnknownBreakRule { .. })
        ));
        invalid = boundary;
        invalid.break_rule = 2;
        assert!(matches!(
            validate(vec![], vec![invalid]),
            Err(SpacingTableValidationErrorV0::PenaltyWithoutPenaltyRule { .. })
        ));
        invalid = boundary;
        invalid.line_edge_rule = 5;
        assert!(matches!(
            validate(vec![], vec![invalid]),
            Err(SpacingTableValidationErrorV0::UnknownLineEdgeRule { .. })
        ));
        invalid = boundary;
        invalid.reason_id = 4;
        assert!(matches!(
            validate(vec![], vec![invalid]),
            Err(SpacingTableValidationErrorV0::ReasonIdOutOfBounds { .. })
        ));
        invalid = boundary;
        invalid.flags = 1;
        assert!(matches!(
            validate(vec![], vec![invalid]),
            Err(SpacingTableValidationErrorV0::NonZeroRuleFlags { .. })
        ));
    }

    #[test]
    fn naturalは負値を保ち調整limitは非負に限定する() {
        let mut signed_natural = rule(1, 1);
        signed_natural.natural = length(-1, 3);
        assert!(validate(vec![], vec![signed_natural]).is_ok());

        for component in [
            SpacingLengthComponentV0::ShrinkLimit,
            SpacingLengthComponentV0::StretchLimit,
        ] {
            let mut invalid = rule(1, 1);
            match component {
                SpacingLengthComponentV0::ShrinkLimit => invalid.shrink_limit = length(-1, 3),
                SpacingLengthComponentV0::StretchLimit => invalid.stretch_limit = length(-1, 3),
                SpacingLengthComponentV0::Natural => unreachable!(),
            }
            assert!(matches!(
                validate(vec![], vec![invalid]),
                Err(SpacingTableValidationErrorV0::NegativeLengthLimit {
                    component: actual,
                    ..
                }) if actual == component
            ));
        }
    }

    #[test]
    fn 件数とrecord_byte上限は境界値だけを通す() {
        let small = SpacingTableConfigV0::new(0, 2, 1, 1, 1, 1, 1);
        let proposal = SpacingTableProposalV0::new(vec![range(0x41, 1)], vec![rule(1, 1)]);
        let validated =
            validate_spacing_table_proposal_v0(small, 24 + 88, proposal.clone()).unwrap();
        assert_eq!(validated.record_bytes(), 112);
        assert!(matches!(
            validate_spacing_table_proposal_v0(small, 111, proposal),
            Err(SpacingTableValidationErrorV0::TooManyRecordBytes { .. })
        ));
        assert!(matches!(
            validate_spacing_table_proposal_v0(
                small,
                ALL_RECORD_BYTES,
                SpacingTableProposalV0::new(vec![range(0x41, 1), range(0x42, 1)], vec![]),
            ),
            Err(SpacingTableValidationErrorV0::TooManyRanges { .. })
        ));
        assert!(validate_spacing_table_proposal_v0(
            small,
            0,
            SpacingTableProposalV0::new(vec![], vec![]),
        )
        .is_ok());
    }

    #[test]
    fn 最後の不正recordでもactive候補を部分交換しない() {
        let mut active = validate(vec![range(0x41, 1)], vec![rule(1, 1)]).unwrap();
        let before = active.clone();
        let mut invalid_last = rule(2, 2);
        invalid_last.stretch_limit = length(1, 0);
        let result = active.try_replace(
            config(),
            ALL_RECORD_BYTES,
            SpacingTableProposalV0::new(vec![range(0x42, 2)], vec![rule(1, 1), invalid_last]),
        );
        assert!(matches!(
            result,
            Err(SpacingTableValidationErrorV0::ZeroLengthDenominator { rule_index: 1, .. })
        ));
        assert_eq!(active, before);
    }

    #[test]
    fn record順の全順列は同じcanonical候補になる() {
        let ranges = vec![range(0x41, 1), range(0x42, 2), range(0x43, 3)];
        let rules = vec![rule(1, 2), rule(2, 3), rule(3, 1)];
        let expected = validate(ranges.clone(), rules.clone()).unwrap();
        for range_order in permutations(ranges) {
            for rule_order in permutations(rules.clone()) {
                assert_eq!(validate(range_order.clone(), rule_order).unwrap(), expected);
            }
        }
    }

    fn permutations<T: Clone>(values: Vec<T>) -> Vec<Vec<T>> {
        fn visit<T: Clone>(values: &mut [T], start: usize, output: &mut Vec<Vec<T>>) {
            if start == values.len() {
                output.push(values.to_vec());
                return;
            }
            for index in start..values.len() {
                values.swap(start, index);
                visit(values, start + 1, output);
                values.swap(start, index);
            }
        }

        let mut values = values;
        let mut output = Vec::new();
        visit(&mut values, 0, &mut output);
        output
    }
}

//! ABI 0.0 spacing candidateからnative compiled tableへの一方向bridge。
//!
//! このmoduleはwire/raw proposalを受け取らない。W0-Aが全件検証してcanonical化した
//! [`CanonicalSpacingTableCandidateV0`]だけをconsumeし、共有domain codecでdense class IDへ写した後、
//! native側の唯一のtable compilerへ渡す。runtime/provider registration自体はまだ後段だが、
//! 検証済みcandidateをrun-local active dispatcherへ原子的に公開する最終host入口もここに置く。

use crate::eqtb::Eqtb;
use crate::script_spacing::{
    CanonicalScriptSpacingRange, CanonicalScriptSpacingRule, CanonicalScriptSpacingTableCandidate,
    CompiledScriptSpacingTable, ContextualSpacingLength, ContextualSpacingLengthBasis,
    ProviderRegionMask, ProviderWritingModeMask, ScriptClassId, ScriptSpacingAction,
    ScriptSpacingActivationId, ScriptSpacingBoundaryRule, ScriptSpacingBreakRule,
    ScriptSpacingLineEdgeRule, ScriptSpacingTableError,
};
use crate::spacing_table_domain::provider_class_index;
use crate::wasm_spacing_table_v0::{
    CanonicalSpacingLengthV0, CanonicalSpacingTableCandidateV0, SpacingBreakRuleV0,
    SpacingLengthBasisV0, SpacingLineEdgeRuleV0,
};

use std::num::NonZeroU32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpacingCandidateInvariantV0 {
    ClassCapacity,
    ClassId { class_id: u32 },
    LengthDenominator,
    AdjustmentTier { tier: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SpacingTableCompileErrorV0 {
    CandidateInvariant(SpacingCandidateInvariantV0),
    NativeTable(ScriptSpacingTableError),
}

/// Consumes one W0-A candidate and builds a native table without reconstructing a raw proposal.
pub(crate) fn compile_spacing_table_candidate_v0(
    candidate: CanonicalSpacingTableCandidateV0,
) -> Result<CompiledScriptSpacingTable, SpacingTableCompileErrorV0> {
    let candidate = into_native_candidate(candidate)?;
    CompiledScriptSpacingTable::compile_canonical(candidate)
        .map_err(SpacingTableCompileErrorV0::NativeTable)
}

/// Compiles a fully validated candidate before publishing it to this run's list dispatcher.
/// Neither the provider handle nor the activation generation is written to fmt or a node.
pub(crate) fn install_spacing_table_candidate_v0(
    eqtb: &mut Eqtb,
    candidate: CanonicalSpacingTableCandidateV0,
) -> Result<ScriptSpacingActivationId, SpacingTableCompileErrorV0> {
    let table = compile_spacing_table_candidate_v0(candidate)?;
    Ok(eqtb.install_script_spacing_table(table))
}

/// Revokes the active capability-owned table for this run and invalidates open lists.
pub(crate) fn revoke_spacing_table_candidate_v0(eqtb: &mut Eqtb) {
    eqtb.clear_script_spacing_table();
}

/// Compiles completely before publishing the replacement.
pub(crate) fn try_replace_spacing_table_candidate_v0(
    active: &mut CompiledScriptSpacingTable,
    candidate: CanonicalSpacingTableCandidateV0,
) -> Result<(), SpacingTableCompileErrorV0> {
    let replacement = compile_spacing_table_candidate_v0(candidate)?;
    *active = replacement;
    Ok(())
}

fn into_native_candidate(
    candidate: CanonicalSpacingTableCandidateV0,
) -> Result<CanonicalScriptSpacingTableCandidate, SpacingTableCompileErrorV0> {
    let (provider_class_capacity, ranges, rules, _record_bytes) = candidate.into_parts();
    let class_capacity = u16::try_from(provider_class_capacity).map_err(|_| {
        SpacingTableCompileErrorV0::CandidateInvariant(SpacingCandidateInvariantV0::ClassCapacity)
    })?;

    let mut native_ranges = Vec::new();
    native_ranges
        .try_reserve_exact(ranges.len())
        .map_err(|_| native_table_too_large())?;
    for range in ranges {
        native_ranges.push(CanonicalScriptSpacingRange::from_validated_parts(
            range.first_scalar(),
            range.last_scalar_inclusive(),
            dense_class_id(range.class_id(), provider_class_capacity)?,
            ProviderRegionMask::from_wire(range.region_mask()),
            ProviderWritingModeMask::from_wire(range.writing_mode_mask()),
        ));
    }

    let mut native_rules = Vec::new();
    native_rules
        .try_reserve_exact(rules.len())
        .map_err(|_| native_table_too_large())?;
    for rule in rules {
        let (left_class_id, right_class_id) = rule.class_pair();
        let (region_mask, writing_mode_mask) = rule.context_masks();
        let (natural, shrink_limit, stretch_limit) = rule.lengths();
        let (shrink_tier, stretch_tier) = rule.tiers();
        let action =
            ScriptSpacingAction::BoundaryRule(ScriptSpacingBoundaryRule::from_canonical_parts(
                native_length(natural)?,
                native_length(shrink_limit)?,
                native_length(stretch_limit)?,
                u8::try_from(shrink_tier).map_err(|_| {
                    SpacingTableCompileErrorV0::CandidateInvariant(
                        SpacingCandidateInvariantV0::AdjustmentTier { tier: shrink_tier },
                    )
                })?,
                u8::try_from(stretch_tier).map_err(|_| {
                    SpacingTableCompileErrorV0::CandidateInvariant(
                        SpacingCandidateInvariantV0::AdjustmentTier { tier: stretch_tier },
                    )
                })?,
                native_break_rule(rule.break_rule()),
                native_line_edge_rule(rule.line_edge_rule()),
                rule.penalty(),
                rule.reason_id(),
            ));
        native_rules.push(CanonicalScriptSpacingRule::from_validated_parts(
            dense_class_id(left_class_id, provider_class_capacity)?,
            dense_class_id(right_class_id, provider_class_capacity)?,
            ProviderRegionMask::from_wire(region_mask),
            ProviderWritingModeMask::from_wire(writing_mode_mask),
            action,
        ));
    }

    Ok(CanonicalScriptSpacingTableCandidate::from_validated_parts(
        class_capacity,
        native_ranges,
        native_rules,
    ))
}

fn dense_class_id(
    class_id: u32,
    provider_class_capacity: u32,
) -> Result<ScriptClassId, SpacingTableCompileErrorV0> {
    let index = provider_class_index(class_id, provider_class_capacity).ok_or(
        SpacingTableCompileErrorV0::CandidateInvariant(SpacingCandidateInvariantV0::ClassId {
            class_id,
        }),
    )?;
    ScriptClassId::from_validated_dense_index(index).ok_or(
        SpacingTableCompileErrorV0::CandidateInvariant(SpacingCandidateInvariantV0::ClassId {
            class_id,
        }),
    )
}

fn native_length(
    length: CanonicalSpacingLengthV0,
) -> Result<ContextualSpacingLength, SpacingTableCompileErrorV0> {
    let denominator = NonZeroU32::new(length.denominator()).ok_or(
        SpacingTableCompileErrorV0::CandidateInvariant(
            SpacingCandidateInvariantV0::LengthDenominator,
        ),
    )?;
    let basis = match length.basis() {
        SpacingLengthBasisV0::AbsoluteScaledPoint => {
            ContextualSpacingLengthBasis::AbsoluteScaledPoint
        }
        SpacingLengthBasisV0::LeftEm => ContextualSpacingLengthBasis::LeftEm,
        SpacingLengthBasisV0::RightEm => ContextualSpacingLengthBasis::RightEm,
        SpacingLengthBasisV0::LeftZw => ContextualSpacingLengthBasis::LeftZw,
        SpacingLengthBasisV0::RightZw => ContextualSpacingLengthBasis::RightZw,
    };
    Ok(ContextualSpacingLength::from_canonical_parts(
        length.numerator(),
        denominator,
        basis,
    ))
}

const fn native_break_rule(rule: SpacingBreakRuleV0) -> ScriptSpacingBreakRule {
    match rule {
        SpacingBreakRuleV0::UseBuiltIn => ScriptSpacingBreakRule::UseBuiltIn,
        SpacingBreakRuleV0::Allow => ScriptSpacingBreakRule::Allow,
        SpacingBreakRuleV0::Forbid => ScriptSpacingBreakRule::Forbid,
        SpacingBreakRuleV0::Penalty => ScriptSpacingBreakRule::Penalty,
    }
}

const fn native_line_edge_rule(rule: SpacingLineEdgeRuleV0) -> ScriptSpacingLineEdgeRule {
    match rule {
        SpacingLineEdgeRuleV0::UseBuiltIn => ScriptSpacingLineEdgeRule::UseBuiltIn,
        SpacingLineEdgeRuleV0::Retain => ScriptSpacingLineEdgeRule::Retain,
        SpacingLineEdgeRuleV0::DiscardAtStart => ScriptSpacingLineEdgeRule::DiscardAtStart,
        SpacingLineEdgeRuleV0::DiscardAtEnd => ScriptSpacingLineEdgeRule::DiscardAtEnd,
        SpacingLineEdgeRuleV0::DiscardAtBoth => ScriptSpacingLineEdgeRule::DiscardAtBoth,
    }
}

fn native_table_too_large() -> SpacingTableCompileErrorV0 {
    SpacingTableCompileErrorV0::NativeTable(ScriptSpacingTableError::CompiledTableTooLarge)
}

#[cfg(test)]
fn try_replace_with_max_dense_slots(
    active: &mut CompiledScriptSpacingTable,
    candidate: CanonicalSpacingTableCandidateV0,
    max_dense_slots: usize,
) -> Result<(), SpacingTableCompileErrorV0> {
    let candidate = into_native_candidate(candidate)?;
    active
        .try_replace_canonical_with_max_dense_slots(candidate, max_dense_slots)
        .map_err(SpacingTableCompileErrorV0::NativeTable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dimension::MAX_DIMEN;
    use crate::eqtb::LanguageRegion;
    use crate::nodes::DimensionOrder;
    use crate::script_spacing::{
        BoundaryMetricSnapshot, ContextualSpacingResolutionError, ScriptSpacingProfileRef,
        WritingMode,
    };
    use crate::spacing_table_domain::{VALID_LANGUAGE_REGION_MASK, VALID_WRITING_MODE_MASK};
    use crate::wasm_spacing_table_v0::{
        validate_spacing_table_proposal_v0, SpacingClassRangeProposalV0, SpacingLengthProposalV0,
        SpacingPairRuleProposalV0, SpacingTableConfigV0, SpacingTableProposalV0,
    };

    const ALL_RECORD_BYTES: u64 = 1_000_000;

    fn config() -> SpacingTableConfigV0 {
        SpacingTableConfigV0::new(
            0,
            3,
            8,
            8,
            4,
            VALID_LANGUAGE_REGION_MASK,
            VALID_WRITING_MODE_MASK,
        )
    }

    fn length(numerator: i64, denominator: u32, basis: u16) -> SpacingLengthProposalV0 {
        SpacingLengthProposalV0::new(numerator, denominator, basis, 0)
    }

    fn external_proposal() -> SpacingTableProposalV0 {
        let contexts = (1 << LanguageRegion::Ja.code()) | (1 << LanguageRegion::Ko.code());
        SpacingTableProposalV0::new(
            vec![
                SpacingClassRangeProposalV0::new(
                    u32::from('A'),
                    u32::from('A'),
                    1,
                    contexts,
                    VALID_WRITING_MODE_MASK,
                ),
                SpacingClassRangeProposalV0::new(
                    u32::from('漢'),
                    u32::from('漢'),
                    2,
                    contexts,
                    VALID_WRITING_MODE_MASK,
                ),
            ],
            vec![SpacingPairRuleProposalV0::new(
                2,
                1,
                contexts,
                VALID_WRITING_MODE_MASK,
                length(1, 2, SpacingLengthBasisV0::LeftEm as u16),
                length(1, 4, SpacingLengthBasisV0::RightZw as u16),
                length(3, 2, SpacingLengthBasisV0::AbsoluteScaledPoint as u16),
                u8::MAX as u16,
                u8::MAX as u16,
                SpacingBreakRuleV0::Penalty as u16,
                SpacingLineEdgeRuleV0::DiscardAtBoth as u16,
                i32::MIN,
                3,
                0,
            )],
        )
    }

    fn validated_external_candidate() -> CanonicalSpacingTableCandidateV0 {
        validate_spacing_table_proposal_v0(config(), ALL_RECORD_BYTES, external_proposal()).unwrap()
    }

    #[test]
    fn provider_class上限も一始まりから零始まりへ一度だけ写す() {
        let capacity = crate::script_spacing::MAX_SCRIPT_SPACING_CLASSES;
        let config = SpacingTableConfigV0::new(
            0,
            capacity,
            2,
            1,
            1,
            VALID_LANGUAGE_REGION_MASK,
            VALID_WRITING_MODE_MASK,
        );
        let proposal = SpacingTableProposalV0::new(
            vec![
                SpacingClassRangeProposalV0::new(u32::from('A'), u32::from('A'), 1, 1, 1),
                SpacingClassRangeProposalV0::new(u32::from('Z'), u32::from('Z'), capacity, 1, 1),
            ],
            vec![SpacingPairRuleProposalV0::new(
                capacity,
                1,
                1,
                1,
                length(0, 1, SpacingLengthBasisV0::AbsoluteScaledPoint as u16),
                length(0, 1, SpacingLengthBasisV0::AbsoluteScaledPoint as u16),
                length(0, 1, SpacingLengthBasisV0::AbsoluteScaledPoint as u16),
                0,
                0,
                SpacingBreakRuleV0::UseBuiltIn as u16,
                SpacingLineEdgeRuleV0::UseBuiltIn as u16,
                0,
                0,
                0,
            )],
        );
        let table = compile_spacing_table_candidate_v0(
            validate_spacing_table_proposal_v0(config, ALL_RECORD_BYTES, proposal).unwrap(),
        )
        .unwrap();

        assert_eq!(u32::from(table.class_count()), capacity);
        let first = table
            .classify_scalar('A', LanguageRegion::Und, WritingMode::Horizontal)
            .unwrap();
        let last = table
            .classify_scalar('Z', LanguageRegion::Und, WritingMode::Horizontal)
            .unwrap();
        assert_eq!(first, ScriptClassId::from_validated_dense_index(0).unwrap());
        assert_eq!(
            last,
            ScriptClassId::from_validated_dense_index(capacity - 1).unwrap()
        );
        assert!(matches!(
            table.action_for(last, first, LanguageRegion::Und, WritingMode::Horizontal),
            ScriptSpacingAction::BoundaryRule(_)
        ));
    }

    fn native_api_candidate() -> CanonicalScriptSpacingTableCandidate {
        let contexts = (1 << LanguageRegion::Ja.code()) | (1 << LanguageRegion::Ko.code());
        let class = |index| ScriptClassId::from_validated_dense_index(index).unwrap();
        let native_length = |numerator, denominator, basis| {
            ContextualSpacingLength::from_canonical_parts(
                numerator,
                NonZeroU32::new(denominator).unwrap(),
                basis,
            )
        };
        let action =
            ScriptSpacingAction::BoundaryRule(ScriptSpacingBoundaryRule::from_canonical_parts(
                native_length(1, 2, ContextualSpacingLengthBasis::LeftEm),
                native_length(1, 4, ContextualSpacingLengthBasis::RightZw),
                native_length(3, 2, ContextualSpacingLengthBasis::AbsoluteScaledPoint),
                u8::MAX,
                u8::MAX,
                ScriptSpacingBreakRule::Penalty,
                ScriptSpacingLineEdgeRule::DiscardAtBoth,
                i32::MIN,
                3,
            ));
        CanonicalScriptSpacingTableCandidate::from_validated_parts(
            3,
            vec![
                CanonicalScriptSpacingRange::from_validated_parts(
                    u32::from('A'),
                    u32::from('A'),
                    class(0),
                    ProviderRegionMask::from_wire(contexts),
                    ProviderWritingModeMask::all(),
                ),
                CanonicalScriptSpacingRange::from_validated_parts(
                    u32::from('漢'),
                    u32::from('漢'),
                    class(1),
                    ProviderRegionMask::from_wire(contexts),
                    ProviderWritingModeMask::all(),
                ),
            ],
            vec![CanonicalScriptSpacingRule::from_validated_parts(
                class(1),
                class(0),
                ProviderRegionMask::from_wire(contexts),
                ProviderWritingModeMask::all(),
                action,
            )],
        )
    }

    #[test]
    fn native_apiとwasm候補は同じcompiled表意味になる() {
        let native = CompiledScriptSpacingTable::compile_canonical(native_api_candidate()).unwrap();
        let wasm = compile_spacing_table_candidate_v0(validated_external_candidate()).unwrap();
        assert_eq!(native, wasm);
        assert_eq!(wasm.class_count(), 3, "未使用class IDを含むcapacityを保つ");

        for (region, mode) in [
            (LanguageRegion::Ja, WritingMode::Horizontal),
            (LanguageRegion::Ja, WritingMode::Vertical),
            (LanguageRegion::Ko, WritingMode::Horizontal),
            (LanguageRegion::Ko, WritingMode::Vertical),
        ] {
            let left = wasm.classify_scalar('漢', region, mode).unwrap();
            let right = wasm.classify_scalar('A', region, mode).unwrap();
            let ScriptSpacingAction::BoundaryRule(rule) =
                wasm.action_for(left, right, region, mode)
            else {
                panic!("contextual boundary ruleでなければならない");
            };
            let (natural, shrink_limit, stretch_limit) = rule.lengths();
            assert_eq!(
                (natural.numerator(), natural.denominator(), natural.basis()),
                (1, 2, ContextualSpacingLengthBasis::LeftEm),
                "context長を登録時spへ近似しない"
            );
            assert_eq!(
                (
                    shrink_limit.numerator(),
                    shrink_limit.denominator(),
                    shrink_limit.basis()
                ),
                (1, 4, ContextualSpacingLengthBasis::RightZw)
            );
            assert_eq!(
                (
                    stretch_limit.numerator(),
                    stretch_limit.denominator(),
                    stretch_limit.basis()
                ),
                (3, 2, ContextualSpacingLengthBasis::AbsoluteScaledPoint)
            );
            let metrics = BoundaryMetricSnapshot::new(11, 13, 17, 10).unwrap();
            let resolved = rule.resolve(metrics).unwrap();
            assert_eq!(resolved.glue().width(), 6);
            assert_eq!(resolved.glue().shrink().value, 3);
            assert_eq!(resolved.glue().stretch().value, 2);
            assert_eq!(resolved.glue().shrink().order, DimensionOrder::Normal);
            assert_eq!(resolved.glue().stretch().order, DimensionOrder::Normal);
            assert_eq!(resolved.tiers(), (u8::MAX, u8::MAX));
            assert_eq!(resolved.break_rule(), ScriptSpacingBreakRule::Penalty);
            assert_eq!(
                resolved.line_edge_rule(),
                ScriptSpacingLineEdgeRule::DiscardAtBoth
            );
            assert_eq!(resolved.penalty(), i32::MIN);
            assert_eq!(resolved.reason_id(), 3);
        }
    }

    #[test]
    fn wasm候補はcompile完了後だけrun_local_dispatcherへ公開する() {
        let mut eqtb = Eqtb::new();
        let activation =
            install_spacing_table_candidate_v0(&mut eqtb, validated_external_candidate()).unwrap();
        assert_eq!(eqtb.script_spacing_activation_id(), Some(activation));
        assert!(matches!(
            eqtb.select_script_spacing_profile(Some(activation), LanguageRegion::Ja),
            ScriptSpacingProfileRef::CompiledTable { .. }
        ));

        revoke_spacing_table_candidate_v0(&mut eqtb);
        assert_eq!(eqtb.script_spacing_activation_id(), None);
        assert!(matches!(
            eqtb.select_script_spacing_profile(Some(activation), LanguageRegion::Ja),
            ScriptSpacingProfileRef::BuiltIn
        ));
    }

    #[test]
    fn contextual有理長は全basisを同じchecked丸めで解決する() {
        let metrics = BoundaryMetricSnapshot::new(11, 13, 17, 19).unwrap();
        for (basis, basis_value) in [
            (ContextualSpacingLengthBasis::AbsoluteScaledPoint, 1),
            (ContextualSpacingLengthBasis::LeftEm, 11),
            (ContextualSpacingLengthBasis::RightEm, 13),
            (ContextualSpacingLengthBasis::LeftZw, 17),
            (ContextualSpacingLengthBasis::RightZw, 19),
        ] {
            for numerator in [-3, -1, 0, 1, 3] {
                for denominator in [1, 2, 3] {
                    let value = ContextualSpacingLength::from_canonical_parts(
                        numerator,
                        NonZeroU32::new(denominator).unwrap(),
                        basis,
                    );
                    let actual = value.resolve(metrics).unwrap();
                    assert_eq!(
                        actual,
                        reference_round(
                            i128::from(numerator) * i128::from(basis_value),
                            i128::from(denominator)
                        ) as i32
                    );
                }
            }
        }
        assert_eq!(
            ContextualSpacingLength::from_canonical_parts(
                -1,
                NonZeroU32::new(2).unwrap(),
                ContextualSpacingLengthBasis::AbsoluteScaledPoint,
            )
            .resolve(metrics),
            Ok(-1),
            "half tieは零から遠い側へ丸める"
        );
        assert!(BoundaryMetricSnapshot::new(-1, 1, 1, 1).is_err());
        assert!(BoundaryMetricSnapshot::new(MAX_DIMEN + 1, 1, 1, 1).is_err());
    }

    #[test]
    fn 寸法範囲外は近似せずactive表も変更しない() {
        let mut active = CompiledScriptSpacingTable::compile_canonical(
            CanonicalScriptSpacingTableCandidate::from_validated_parts(1, vec![], vec![]),
        )
        .unwrap();
        try_replace_spacing_table_candidate_v0(&mut active, validated_external_candidate())
            .unwrap();
        assert_eq!(active.class_count(), 3);
        let before = active.clone();
        let result = try_replace_with_max_dense_slots(
            &mut active,
            validated_external_candidate(),
            3 * 3 * 6 * 2 - 1,
        );
        assert_eq!(
            result,
            Err(SpacingTableCompileErrorV0::NativeTable(
                ScriptSpacingTableError::CompiledTableTooLarge
            ))
        );
        assert_eq!(active, before);

        let too_large = ContextualSpacingLength::from_canonical_parts(
            i64::MAX,
            NonZeroU32::new(1).unwrap(),
            ContextualSpacingLengthBasis::LeftEm,
        );
        assert!(matches!(
            too_large.resolve(BoundaryMetricSnapshot::new(MAX_DIMEN, 1, 1, 1).unwrap()),
            Err(ContextualSpacingResolutionError::DimensionOutOfBounds { .. })
        ));
        assert_eq!(active, before, "解決失敗もcompiled tableを変更しない");
    }

    #[test]
    fn record順序はbridge後のnative表意味を変えない() {
        let expected = compile_spacing_table_candidate_v0(validated_external_candidate()).unwrap();
        let (mut ranges, mut rules) = external_proposal().into_parts();
        ranges.reverse();
        rules.reverse();
        let candidate = validate_spacing_table_proposal_v0(
            config(),
            ALL_RECORD_BYTES,
            SpacingTableProposalV0::new(ranges, rules),
        )
        .unwrap();
        assert_eq!(
            compile_spacing_table_candidate_v0(candidate).unwrap(),
            expected
        );
    }

    fn reference_round(numerator: i128, denominator: i128) -> i128 {
        let quotient = numerator / denominator;
        let remainder = numerator % denominator;
        if remainder.abs() * 2 >= denominator {
            quotient + numerator.signum()
        } else {
            quotient
        }
    }
}

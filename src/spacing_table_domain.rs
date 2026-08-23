//! Native tableと外向きadapterが共有するscript-spacing domain検査。
//!
//! ここでは外部IDを内部nodeへ変換せず、Unicode scalar、公開mask、class ID、context付き
//! range/pair keyの曖昧性だけを一度決める。ABI固有のlengthやactionは各schema側で検証する。

use crate::eqtb::LanguageRegion;

pub(crate) const LANGUAGE_REGION_COUNT: usize = LanguageRegion::MAX_CODE as usize + 1;
pub(crate) const WRITING_MODE_COUNT: usize = 2;
const CONTEXT_COUNT: usize = LANGUAGE_REGION_COUNT * WRITING_MODE_COUNT;
pub(crate) const VALID_LANGUAGE_REGION_MASK: u32 = (1_u32 << LANGUAGE_REGION_COUNT) - 1;
pub(crate) const VALID_WRITING_MODE_MASK: u32 = (1_u32 << WRITING_MODE_COUNT) - 1;

const UNICODE_MAX: u32 = 0x10_ffff;
const SURROGATE_START: u32 = 0xd800;
const SURROGATE_END: u32 = 0xdfff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarRangeDomainError {
    Reversed,
    NonScalar,
}

pub(crate) const fn validate_scalar_range(
    first: u32,
    last_inclusive: u32,
) -> Result<(), ScalarRangeDomainError> {
    if first > last_inclusive {
        return Err(ScalarRangeDomainError::Reversed);
    }
    if last_inclusive > UNICODE_MAX || (first <= SURROGATE_END && last_inclusive >= SURROGATE_START)
    {
        return Err(ScalarRangeDomainError::NonScalar);
    }
    Ok(())
}

pub(crate) const fn valid_nonzero_mask(mask: u32, known_mask: u32) -> bool {
    mask != 0 && mask & !known_mask == 0
}

/// Provider-local IDは1 originで、0は予約する。
pub(crate) const fn provider_class_index(class_id: u32, class_count: u32) -> Option<u32> {
    if class_id == 0 || class_id > class_count {
        None
    } else {
        Some(class_id - 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContextualScalarRange {
    pub(crate) source_index: usize,
    pub(crate) first: u32,
    pub(crate) last_inclusive: u32,
    pub(crate) region_mask: u32,
    pub(crate) writing_mode_mask: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContextualPairKey {
    pub(crate) source_index: usize,
    pub(crate) left_class_id: u32,
    pub(crate) right_class_id: u32,
    pub(crate) region_mask: u32,
    pub(crate) writing_mode_mask: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DomainWritingMode {
    Horizontal,
    Vertical,
}

impl DomainWritingMode {
    const ALL: [Self; WRITING_MODE_COUNT] = [Self::Horizontal, Self::Vertical];

    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Horizontal => 0,
            Self::Vertical => 1,
        }
    }

    const fn index(self) -> usize {
        self.code() as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContextOverlap {
    pub(crate) first_source_index: usize,
    pub(crate) second_source_index: usize,
    pub(crate) region_code: u8,
    pub(crate) writing_mode: DomainWritingMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextOverlapSearchError {
    AllocationFailed,
}

/// 入力順ではなくscalar順で、context積が重なる最初のrange対を探す。
pub(crate) fn find_contextual_scalar_overlap(
    ranges: impl IntoIterator<Item = ContextualScalarRange>,
) -> Result<Option<ContextOverlap>, ContextOverlapSearchError> {
    let mut ranges = collect_checked(ranges)?;
    ranges.sort_unstable_by_key(|range| (range.first, range.last_inclusive, range.source_index));
    let mut previous_by_context: [Option<(u32, usize)>; CONTEXT_COUNT] = [None; CONTEXT_COUNT];
    for range in ranges {
        let mut overlap = None;
        for_each_context(
            range.region_mask,
            range.writing_mode_mask,
            |context, region_code, writing_mode| {
                if overlap.is_some() {
                    return;
                }
                if let Some((previous_end, previous_index)) = previous_by_context[context] {
                    if previous_end >= range.first {
                        overlap = Some(ContextOverlap {
                            first_source_index: previous_index,
                            second_source_index: range.source_index,
                            region_code,
                            writing_mode,
                        });
                        return;
                    }
                }
                previous_by_context[context] = Some((range.last_inclusive, range.source_index));
            },
        );
        if overlap.is_some() {
            return Ok(overlap);
        }
    }
    Ok(None)
}

/// 同じclass対についてcontext積が一つでも重なるrule対を探す。
pub(crate) fn find_contextual_pair_overlap(
    rules: impl IntoIterator<Item = ContextualPairKey>,
) -> Result<Option<ContextOverlap>, ContextOverlapSearchError> {
    let mut rules = collect_checked(rules)?;
    rules.sort_unstable_by_key(|rule| {
        (
            rule.left_class_id,
            rule.right_class_id,
            rule.region_mask,
            rule.writing_mode_mask,
            rule.source_index,
        )
    });
    let mut current_pair = None;
    let mut previous_by_context: [Option<usize>; CONTEXT_COUNT] = [None; CONTEXT_COUNT];
    for rule in rules {
        let pair = (rule.left_class_id, rule.right_class_id);
        if current_pair != Some(pair) {
            current_pair = Some(pair);
            previous_by_context.fill(None);
        }
        let mut overlap = None;
        for_each_context(
            rule.region_mask,
            rule.writing_mode_mask,
            |context, region_code, writing_mode| {
                if overlap.is_some() {
                    return;
                }
                if let Some(previous_index) = previous_by_context[context] {
                    overlap = Some(ContextOverlap {
                        first_source_index: previous_index,
                        second_source_index: rule.source_index,
                        region_code,
                        writing_mode,
                    });
                    return;
                }
                previous_by_context[context] = Some(rule.source_index);
            },
        );
        if overlap.is_some() {
            return Ok(overlap);
        }
    }
    Ok(None)
}

fn collect_checked<T>(
    values: impl IntoIterator<Item = T>,
) -> Result<Vec<T>, ContextOverlapSearchError> {
    let iterator = values.into_iter();
    let mut output = Vec::new();
    output
        .try_reserve_exact(iterator.size_hint().0)
        .map_err(|_| ContextOverlapSearchError::AllocationFailed)?;
    for value in iterator {
        if output.len() == output.capacity() {
            output
                .try_reserve(1)
                .map_err(|_| ContextOverlapSearchError::AllocationFailed)?;
        }
        output.push(value);
    }
    Ok(output)
}

fn for_each_context(
    region_mask: u32,
    writing_mode_mask: u32,
    mut callback: impl FnMut(usize, u8, DomainWritingMode),
) {
    for writing_mode in DomainWritingMode::ALL {
        if writing_mode_mask & (1 << writing_mode.index()) == 0 {
            continue;
        }
        for region_code in 0..LANGUAGE_REGION_COUNT {
            if region_mask & (1 << region_code) != 0 {
                callback(
                    writing_mode.index() * LANGUAGE_REGION_COUNT + region_code,
                    region_code as u8,
                    writing_mode,
                );
            }
        }
    }
}

//! 横組hlistへplannerの決定を一度だけmaterializeする中央finalizer。
//!
//! ASCII-only listは`ScriptSpacingListState`でこのmoduleへ入らない。日本語listの
//! 再finalize時は、このmoduleが付けたprovenanceだけを除去して元のglyph境界を再評価する。

use super::planner::{
    BoundaryAtom, BoundaryContext, CompiledJfmPairSpacingTable, JapaneseBoundary,
    JapaneseSpacingPlanner, JfmFontId, LatinBoundary, LayoutCharacterCode, PlannedSpacingAction,
    PlannerJfmClassId, PtexSpacingState, ScriptSpacingListState,
};
use super::FixedGlue;
use crate::eqtb::{Eqtb, SkipVariable};
use crate::japanese_fonts::JapaneseFontIndex;
use crate::nodes::{
    AutomaticJapaneseGlue, GlueNode, GlueSpec, GlueType, KernNode, KernSubtype, LigatureNode, Node,
    PenaltyNode, PenaltySubtype, WideCharNode,
};

use std::rc::Rc;

#[derive(Clone, Copy)]
struct ObservedBoundary {
    atom: BoundaryAtom,
    japanese_font: Option<JapaneseFontIndex>,
}

/// list終端のK/X snapshotを使い、JFM/禁則/K/Xを同じ元境界へ一回だけ適用する。
pub(crate) fn finalize_horizontal_list_if_needed(
    list: &mut Vec<Node>,
    list_state: ScriptSpacingListState,
    eqtb: &Eqtb,
) {
    let _ = list_state.finalize_if_needed(|| finalize_horizontal_list(list, eqtb));
}

fn finalize_horizontal_list(list: &mut Vec<Node>, eqtb: &Eqtb) {
    let state = PtexSpacingState::built_in_minimal_with_controls(
        FixedGlue::snapshot(eqtb.skips.get(SkipVariable::KanjiSkip)),
        FixedGlue::snapshot(eqtb.skips.get(SkipVariable::XKanjiSkip)),
        eqtb.auto_spacing_state(),
        eqtb.xsp_codes(),
        eqtb.inhibit_xsp_codes(),
    );
    let planner = JapaneseSpacingPlanner::built_in_ptex();
    let original = std::mem::take(list);
    let mut rebuilt = Vec::with_capacity(original.len());
    let mut previous = None;

    for node in original {
        if is_automatic_spacing(&node) {
            continue;
        }

        if let Some(current) = observe_boundary(&node, eqtb) {
            if let Some(left) = previous {
                let jfm_pairs = pair_table_for_boundary(left, current, eqtb);
                let plan = planner.plan_boundary(
                    left.atom,
                    current.atom,
                    BoundaryContext::DEFAULT,
                    &state,
                    jfm_pairs,
                );
                rebuilt.extend(plan.actions().map(materialize_action));
            }
            previous = Some(current);
            rebuilt.push(node);
        } else if matches!(node, Node::Penalty(_)) {
            // 明示penaltyは文字境界を保つ。planner actionはこのnodeの後ろへ置かれる。
            rebuilt.push(node);
        } else {
            // 明示glue/kern/math/whatsit/list/rule/disc等を越えて自動間隔を作らない。
            previous = None;
            rebuilt.push(node);
        }
    }

    *list = rebuilt;
}

fn observe_boundary(node: &Node, eqtb: &Eqtb) -> Option<ObservedBoundary> {
    match node {
        Node::Char(character) => Some(ObservedBoundary {
            atom: BoundaryAtom::Latin(LatinBoundary::new(
                LayoutCharacterCode::from_public_integer(u32::from(character.character)).ok()?,
            )),
            japanese_font: None,
        }),
        Node::Ligature(ligature) => observe_ligature(ligature),
        Node::WideChar(WideCharNode {
            font_index,
            character,
            class,
            ..
        }) => {
            let font = eqtb.japanese_fonts.get(font_index.position())?;
            Some(ObservedBoundary {
                atom: BoundaryAtom::Japanese(JapaneseBoundary::new(
                    LayoutCharacterCode::from_public_integer(*character).ok()?,
                    JfmFontId::new(font_index.position() as u32),
                    font.metric_id(),
                    PlannerJfmClassId::new(class.number()),
                )),
                japanese_font: Some(*font_index),
            })
        }
        _ => None,
    }
}

fn observe_ligature(ligature: &LigatureNode) -> Option<ObservedBoundary> {
    let leading = ligature.lig.first().copied().unwrap_or(ligature.character);
    let trailing = ligature.lig.last().copied().unwrap_or(ligature.character);
    Some(ObservedBoundary {
        atom: BoundaryAtom::Latin(LatinBoundary::ligature(
            LayoutCharacterCode::from_public_integer(u32::from(leading)).ok()?,
            LayoutCharacterCode::from_public_integer(u32::from(trailing)).ok()?,
        )),
        japanese_font: None,
    })
}

fn pair_table_for_boundary<'a>(
    left: ObservedBoundary,
    right: ObservedBoundary,
    eqtb: &'a Eqtb,
) -> Option<&'a CompiledJfmPairSpacingTable> {
    let left_font = left.japanese_font?;
    let right_font = right.japanese_font?;
    if left_font != right_font {
        return None;
    }
    eqtb.japanese_fonts
        .get(left_font.position())
        .map(|font| font.pair_spacing())
}

fn is_automatic_spacing(node: &Node) -> bool {
    matches!(
        node,
        Node::Glue(GlueNode {
            subtype: GlueType::AutomaticJapanese(_),
            ..
        }) | Node::Kern(KernNode {
            subtype: KernSubtype::AutomaticJapaneseJfm,
            ..
        }) | Node::Penalty(PenaltyNode {
            subtype: PenaltySubtype::AutomaticJapaneseKinsoku,
            ..
        })
    )
}

fn materialize_action(action: PlannedSpacingAction) -> Node {
    match action {
        PlannedSpacingAction::KinsokuPenalty { value } => {
            Node::Penalty(PenaltyNode::new_automatic_japanese(value))
        }
        PlannedSpacingAction::JfmGlue { glue } => automatic_glue(AutomaticJapaneseGlue::Jfm, glue),
        PlannedSpacingAction::JfmKern { width } => Node::Kern(KernNode {
            subtype: KernSubtype::AutomaticJapaneseJfm,
            width,
        }),
        PlannedSpacingAction::ImplicitKanjiSkip { glue, .. } => {
            automatic_glue(AutomaticJapaneseGlue::VirtualKanjiSkip, glue)
        }
        PlannedSpacingAction::MaterialXKanjiSkip { glue, .. } => {
            automatic_glue(AutomaticJapaneseGlue::XKanjiSkip, glue)
        }
    }
}

fn automatic_glue(kind: AutomaticJapaneseGlue, glue: FixedGlue) -> Node {
    Node::Glue(GlueNode::new_automatic_japanese(
        kind,
        Rc::new(GlueSpec {
            width: glue.width(),
            stretch: glue.stretch(),
            shrink: glue.shrink(),
        }),
    ))
}

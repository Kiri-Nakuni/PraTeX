//! 横組hlistへplannerの決定を一度だけmaterializeする中央finalizer。
//!
//! ASCII-only listは`ScriptSpacingListState`でこのmoduleへ入らない。日本語listの
//! 再finalize時は、このmoduleが付けたprovenanceだけを除去して元のglyph境界を再評価する。

use super::planner::{
    BoundaryAtom, BoundaryContext, CompiledJfmPairSpacingTable, JapaneseBoundary,
    JapaneseSpacingPlanner, JfmFontId, JfmGlueControl, JfmPairContinuity, LatinBoundary,
    LayoutCharacterCode, PlannedSpacingAction, PlannerJfmClassId, PtexSpacingState,
    ScriptSpacingListState, SpacingActionPhase,
};
use super::FixedGlue;
use crate::eqtb::{Eqtb, SkipVariable};
use crate::japanese_fonts::JapaneseFontIndex;
use crate::nodes::{
    AutomaticJapaneseGlue, GlueNode, GlueSpec, GlueType, HlistOrVlist, KernNode, KernSubtype,
    LigatureNode, ListNode, Node, PenaltyNode, PenaltySubtype, WideCharNode,
};

use std::rc::Rc;

#[derive(Clone, Copy)]
struct ObservedBoundary {
    atom: BoundaryAtom,
    japanese_font: Option<JapaneseFontIndex>,
}

#[derive(Clone, Copy)]
struct BoundaryEndpoint {
    boundary: ObservedBoundary,
    hbox_edge: bool,
}

#[derive(Clone, Copy)]
struct ObservedHboxEdges {
    first: Option<ObservedBoundary>,
    last: Option<ObservedBoundary>,
}

enum HboxEdgeScan {
    Found(ObservedBoundary),
    Empty,
    Blocked,
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

        if let Some(current) = observe_glyph_boundary(&node, eqtb) {
            let current = BoundaryEndpoint {
                boundary: current,
                hbox_edge: false,
            };
            if let Some(left) = previous {
                append_boundary_spacing(&mut rebuilt, left, current, &planner, &state, eqtb);
            }
            previous = Some(current);
            rebuilt.push(node);
        } else if let Some(edges) = observe_unshifted_hbox_edges(&node, eqtb) {
            if let (Some(left), Some(first)) = (previous, edges.first) {
                append_boundary_spacing(
                    &mut rebuilt,
                    left,
                    BoundaryEndpoint {
                        boundary: first,
                        hbox_edge: true,
                    },
                    &planner,
                    &state,
                    eqtb,
                );
            }
            rebuilt.push(node);
            previous = edges.last.map(|boundary| BoundaryEndpoint {
                boundary,
                hbox_edge: true,
            });
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

fn append_boundary_spacing(
    rebuilt: &mut Vec<Node>,
    left: BoundaryEndpoint,
    right: BoundaryEndpoint,
    planner: &JapaneseSpacingPlanner,
    state: &PtexSpacingState,
    eqtb: &Eqtb,
) {
    let hbox_edge = left.hbox_edge || right.hbox_edge;
    let context = if hbox_edge {
        BoundaryContext {
            jfm_continuity: JfmPairContinuity::Broken,
            jfm_glue: JfmGlueControl::Inhibit,
        }
    } else {
        BoundaryContext::DEFAULT
    };
    let jfm_pairs = (!hbox_edge)
        .then(|| pair_table_for_boundary(left.boundary, right.boundary, eqtb))
        .flatten();
    let plan = planner.plan_boundary(
        left.boundary.atom,
        right.boundary.atom,
        context,
        state,
        jfm_pairs,
    );
    if hbox_edge {
        // 箱edgeで公式に確認できたのはK/Xだけ。JFM pairと禁則を推測で越境させない。
        rebuilt.extend(
            plan.actions_for_phase(SpacingActionPhase::ListFinalizer)
                .map(|action| materialize_action(action, true)),
        );
    } else {
        rebuilt.extend(
            plan.actions()
                .map(|action| materialize_action(action, false)),
        );
    }
}

fn observe_glyph_boundary(node: &Node, eqtb: &Eqtb) -> Option<ObservedBoundary> {
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

/// unshifted hboxだけを外側のK/X境界として要約する。
///
/// 公式binaryで、先頭の空hboxは読み飛ばす一方、先頭kernはedgeを遮ることを確認した。
/// 未観測nodeを推測で透明にはせず、glyphまたはunshifted hboxだけを再帰する。
fn observe_unshifted_hbox_edges(node: &Node, eqtb: &Eqtb) -> Option<ObservedHboxEdges> {
    let Node::List(ListNode {
        shift_amount: 0,
        list: HlistOrVlist::Hlist(nodes),
        ..
    }) = node
    else {
        return None;
    };
    Some(ObservedHboxEdges {
        first: match scan_hbox_edge(nodes, eqtb, false) {
            HboxEdgeScan::Found(boundary) => Some(boundary),
            HboxEdgeScan::Empty | HboxEdgeScan::Blocked => None,
        },
        last: match scan_hbox_edge(nodes, eqtb, true) {
            HboxEdgeScan::Found(boundary) => Some(boundary),
            HboxEdgeScan::Empty | HboxEdgeScan::Blocked => None,
        },
    })
}

fn scan_hbox_edge(nodes: &[Node], eqtb: &Eqtb, reverse: bool) -> HboxEdgeScan {
    let visit = |node: &Node| {
        if let Some(boundary) = observe_glyph_boundary(node, eqtb) {
            return HboxEdgeScan::Found(boundary);
        }
        match node {
            Node::List(ListNode {
                shift_amount: 0,
                list: HlistOrVlist::Hlist(nested),
                ..
            }) => scan_hbox_edge(nested, eqtb, reverse),
            _ => HboxEdgeScan::Blocked,
        }
    };

    if reverse {
        for node in nodes.iter().rev() {
            match visit(node) {
                HboxEdgeScan::Empty => {}
                result => return result,
            }
        }
    } else {
        for node in nodes {
            match visit(node) {
                HboxEdgeScan::Empty => {}
                result => return result,
            }
        }
    }
    HboxEdgeScan::Empty
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

fn materialize_action(action: PlannedSpacingAction, material_kanji_skip: bool) -> Node {
    match action {
        PlannedSpacingAction::KinsokuPenalty { value } => {
            Node::Penalty(PenaltyNode::new_automatic_japanese(value))
        }
        PlannedSpacingAction::JfmGlue { glue } => automatic_glue(AutomaticJapaneseGlue::Jfm, glue),
        PlannedSpacingAction::JfmKern { width } => Node::Kern(KernNode {
            subtype: KernSubtype::AutomaticJapaneseJfm,
            width,
        }),
        PlannedSpacingAction::ImplicitKanjiSkip { glue, .. } => automatic_glue(
            if material_kanji_skip {
                AutomaticJapaneseGlue::MaterialKanjiSkip
            } else {
                AutomaticJapaneseGlue::VirtualKanjiSkip
            },
            glue,
        ),
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

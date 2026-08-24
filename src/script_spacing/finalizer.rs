//! 横組hlistへplannerの決定を一度だけmaterializeする中央finalizer。
//!
//! ASCII-only listは`ScriptSpacingListState`でこのmoduleへ入らない。日本語listの
//! 再finalize時はclose-time K/Xだけを除去して元のglyph境界を再評価する。main-loopで
//! 挿入され、利用者が観測・除去できるJFM/禁則はそのまま保持する。

use super::planner::{
    BoundaryAtom, BoundaryContext, CompiledJfmPairSpacingTable, JapaneseBoundary,
    JapaneseSpacingPlanner, JfmFontId, JfmGlueControl, JfmPairContinuity, LatinBoundary,
    LayoutCharacterCode, MainLoopBoundaryEvent, PlannedSpacingAction, PlannerJfmClassId,
    PtexSpacingState, ScriptSpacingListState, SpacingActionPhase,
};
use super::FixedGlue;
use crate::eqtb::{Eqtb, SkipVariable};
use crate::japanese_fonts::JapaneseFontIndex;
use crate::nodes::{
    AutomaticJapaneseGlue, GlueNode, GlueSpec, GlueType, HlistOrVlist, JfmBoundaryBefore, KernNode,
    KernSubtype, LigatureNode, ListNode, Node, PenaltyNode, PenaltySubtype, WideCharNode,
};

use std::rc::Rc;

#[derive(Clone, Copy)]
struct ObservedBoundary {
    atom: BoundaryAtom,
    japanese_font: Option<JapaneseFontIndex>,
    jfm_boundary_before: JfmBoundaryBefore,
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

#[derive(Clone, Copy)]
enum KanjiSkipProvenance {
    Virtual,
    Material,
}

/// 通常のhorizontal appendを、JFM/禁則のmain-loop phaseへ一箇所で接続する。
pub(crate) fn append_node_with_main_loop_spacing(
    list: &mut Vec<Node>,
    list_state: &mut ScriptSpacingListState,
    mut node: Node,
    eqtb: &Eqtb,
) {
    if let Some(current) = observe_glyph_boundary(&node, eqtb) {
        list_state.observe(current.atom);
        let previous = list_state.observe_main_loop_boundary(current.atom);
        if let Some((left, continuity)) = previous {
            if !left.is_japanese() && !current.atom.is_japanese() {
                list.push(node);
                return;
            }
            let left_half_has_jfm =
                continuity == JfmPairContinuity::Broken && trailing_main_loop_jfm_survives(list);
            let event = match (left, current.atom, continuity) {
                (
                    BoundaryAtom::Japanese(logical_left),
                    BoundaryAtom::Japanese(right),
                    JfmPairContinuity::Broken,
                ) => MainLoopBoundaryEvent::ResumeBeforeJapanese {
                    logical_left,
                    right,
                },
                (left, right, _) => MainLoopBoundaryEvent::Direct { left, right },
            };
            let right_half_has_jfm = append_main_loop_event(list, event, eqtb);
            if continuity == JfmPairContinuity::Broken {
                let Node::WideChar(ref mut wide) = node else {
                    unreachable!("Japanese boundary is represented by a WideChar node")
                };
                wide.jfm_boundary_before = if left_half_has_jfm || right_half_has_jfm {
                    JfmBoundaryBefore::ReplacedByMainLoopJfm
                } else {
                    JfmBoundaryBefore::BrokenNeedsKanjiSkip
                };
            }
        }
        list.push(node);
        return;
    }

    if node.contains_horizontal_japanese_glyph() {
        list_state.observe_japanese();
    }
    if !is_main_loop_spacing(&node) && !matches!(node, Node::Penalty(_)) {
        list_state.reset_main_loop_boundary();
    }
    list.push(node);
}

/// 空group、`\relax`、node除去・観測commandがJFM pairだけを切る。
pub(crate) fn break_main_loop_jfm_continuity(
    list: &mut Vec<Node>,
    list_state: &mut ScriptSpacingListState,
    eqtb: &Eqtb,
) {
    let Some(left) = list_state.break_after_japanese() else {
        return;
    };
    append_main_loop_event(
        list,
        MainLoopBoundaryEvent::BreakAfterJapanese { left },
        eqtb,
    );
}

fn append_main_loop_event(list: &mut Vec<Node>, event: MainLoopBoundaryEvent, eqtb: &Eqtb) -> bool {
    let jfm_pairs = pair_table_for_main_loop_event(event, eqtb);
    let plan = JapaneseSpacingPlanner::built_in_ptex().plan_main_loop_event(event, jfm_pairs);
    let has_jfm_spacing = plan.has_jfm_spacing();
    list.extend(
        plan.actions_for_phase(SpacingActionPhase::MainLoop)
            .map(|action| materialize_action(action, KanjiSkipProvenance::Virtual)),
    );
    has_jfm_spacing
}

fn pair_table_for_main_loop_event<'a>(
    event: MainLoopBoundaryEvent,
    eqtb: &'a Eqtb,
) -> Option<&'a CompiledJfmPairSpacingTable> {
    let (left, right) = match event {
        MainLoopBoundaryEvent::Direct {
            left: BoundaryAtom::Japanese(left),
            right: BoundaryAtom::Japanese(right),
        } => (left, right),
        MainLoopBoundaryEvent::BreakAfterJapanese { left } => (left, left.default_class_endpoint()),
        MainLoopBoundaryEvent::ResumeBeforeJapanese {
            logical_left,
            right,
        } => (logical_left.default_class_endpoint(), right),
        MainLoopBoundaryEvent::Direct { .. } => return None,
    };
    pair_table_for_japanese_pair(left, right, eqtb)
}

/// list終端のK/X snapshotを使い、JFM/禁則/K/Xを同じ元境界へ一回だけ適用する。
pub(crate) fn finalize_horizontal_list_if_needed(
    list: &mut Vec<Node>,
    list_state: ScriptSpacingListState,
    eqtb: &Eqtb,
) {
    let _ = list_state.finalize_if_needed(|| finalize_horizontal_list(list, eqtb));
}

fn spacing_state_snapshot(eqtb: &Eqtb) -> PtexSpacingState {
    PtexSpacingState::built_in_minimal_with_controls(
        FixedGlue::snapshot(eqtb.skips.get(SkipVariable::KanjiSkip)),
        FixedGlue::snapshot(eqtb.skips.get(SkipVariable::XKanjiSkip)),
        eqtb.auto_spacing_state(),
        eqtb.xsp_codes(),
        eqtb.inhibit_xsp_codes(),
    )
}

fn finalize_horizontal_list(list: &mut Vec<Node>, eqtb: &Eqtb) {
    let state = spacing_state_snapshot(eqtb);
    let planner = JapaneseSpacingPlanner::built_in_ptex();
    let original = std::mem::take(list);
    let mut rebuilt = Vec::with_capacity(original.len());
    let mut previous = None;

    for node in original {
        if is_list_finalizer_spacing(&node) {
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
        } else if is_main_loop_spacing(&node) {
            // JFM/禁則は利用者が途中で観測・除去できる。closeでは保持し、元glyphの
            // K/X境界だけを連続させる。
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
        BoundaryContext {
            jfm_continuity: match right.boundary.jfm_boundary_before {
                JfmBoundaryBefore::Continuous => JfmPairContinuity::Continuous,
                JfmBoundaryBefore::BrokenNeedsKanjiSkip => JfmPairContinuity::Broken,
                JfmBoundaryBefore::ReplacedByMainLoopJfm => {
                    JfmPairContinuity::ReplacedByMainLoopJfm
                }
            },
            jfm_glue: JfmGlueControl::Allow,
        }
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
    // MainLoop由来nodeは既にlistにあり、削除済みなら復活させない。closeは終端
    // snapshotを必要とするK/Xだけを再生成する。
    rebuilt.extend(
        plan.actions_for_phase(SpacingActionPhase::ListFinalizer)
            .map(|action| {
                materialize_action(
                    action,
                    if hbox_edge {
                        KanjiSkipProvenance::Material
                    } else {
                        KanjiSkipProvenance::Virtual
                    },
                )
            }),
    );
}

fn observe_glyph_boundary(node: &Node, eqtb: &Eqtb) -> Option<ObservedBoundary> {
    match node {
        Node::Char(character) => Some(ObservedBoundary {
            atom: BoundaryAtom::Latin(LatinBoundary::new(
                LayoutCharacterCode::from_public_integer(u32::from(character.character)).ok()?,
            )),
            japanese_font: None,
            jfm_boundary_before: JfmBoundaryBefore::Continuous,
        }),
        Node::Ligature(ligature) => observe_ligature(ligature),
        Node::WideChar(WideCharNode {
            font_index,
            character,
            class,
            jfm_boundary_before,
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
                jfm_boundary_before: *jfm_boundary_before,
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
        jfm_boundary_before: JfmBoundaryBefore::Continuous,
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

fn pair_table_for_japanese_pair<'a>(
    left: JapaneseBoundary,
    right: JapaneseBoundary,
    eqtb: &'a Eqtb,
) -> Option<&'a CompiledJfmPairSpacingTable> {
    let left_position = left.font_position()?;
    let right_position = right.font_position()?;
    if left_position != right_position {
        return None;
    }
    eqtb.japanese_fonts
        .get(left_position)
        .map(|font| font.pair_spacing())
}

fn is_list_finalizer_spacing(node: &Node) -> bool {
    matches!(
        node,
        Node::Glue(GlueNode {
            subtype: GlueType::AutomaticJapanese(
                AutomaticJapaneseGlue::VirtualKanjiSkip
                    | AutomaticJapaneseGlue::MaterialKanjiSkip
                    | AutomaticJapaneseGlue::XKanjiSkip
            ),
            ..
        })
    )
}

fn is_main_loop_spacing(node: &Node) -> bool {
    matches!(
        node,
        Node::Glue(GlueNode {
            subtype: GlueType::AutomaticJapanese(AutomaticJapaneseGlue::Jfm),
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

/// 直前の実glyphより後ろに、node-less境界の左側JFM spacingがまだ残るか。
/// `\unskip`/`\unkern`で除かれたJFMは置換根拠に数えない。
fn trailing_main_loop_jfm_survives(nodes: &[Node]) -> bool {
    nodes
        .iter()
        .rev()
        .take_while(|node| !matches!(node, Node::Char(_) | Node::Ligature(_) | Node::WideChar(_)))
        .any(|node| {
            matches!(
                node,
                Node::Glue(GlueNode {
                    subtype: GlueType::AutomaticJapanese(AutomaticJapaneseGlue::Jfm),
                    ..
                }) | Node::Kern(KernNode {
                    subtype: KernSubtype::AutomaticJapaneseJfm,
                    ..
                })
            )
        })
}

fn materialize_action(action: PlannedSpacingAction, kanji_skip: KanjiSkipProvenance) -> Node {
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
            match kanji_skip {
                KanjiSkipProvenance::Virtual => AutomaticJapaneseGlue::VirtualKanjiSkip,
                KanjiSkipProvenance::Material => AutomaticJapaneseGlue::MaterialKanjiSkip,
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

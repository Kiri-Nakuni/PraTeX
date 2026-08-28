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
use super::{
    BoundaryMetricSnapshot, FixedGlue, ScriptSpacingAction, ScriptSpacingBreakRule,
    ScriptSpacingLineEdgeRule, ScriptSpacingProfileRef, WritingMode,
};
use crate::eqtb::{Eqtb, FontIndex, SkipVariable};
use crate::japanese_fonts::JapaneseFontIndex;
use crate::nodes::{
    AutomaticJapaneseGlue, GlueNode, GlueSpec, GlueType, HlistOrVlist, JfmBoundaryBefore, KernNode,
    KernSubtype, LigatureNode, Node, PenaltyNode, PenaltySubtype, WideCharNode,
};

use std::rc::Rc;

#[derive(Clone, Copy)]
struct ObservedBoundary {
    atom: BoundaryAtom,
    latin_font: Option<FontIndex>,
    japanese_font: Option<JapaneseFontIndex>,
    jfm_boundary_before: JfmBoundaryBefore,
}

#[derive(Clone, Copy)]
struct BoundaryEndpoint {
    boundary: ObservedBoundary,
    indirect_edge: bool,
}

#[derive(Clone, Copy)]
struct ObservedHboxEdges {
    first: Option<ObservedBoundary>,
    last: Option<ObservedBoundary>,
}

#[derive(Clone, Copy)]
struct ObservedDiscRightEdges {
    no_break: Option<ObservedBoundary>,
    post_break: Option<ObservedBoundary>,
}

#[derive(Clone, Copy)]
struct PendingDiscRightEdges {
    rebuilt_index: usize,
    edges: ObservedDiscRightEdges,
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

#[derive(Clone, Copy, Default)]
struct MainLoopJfmOutcome {
    materialized: bool,
    inhibited: bool,
}

/// 通常のhorizontal appendを、JFM/禁則のmain-loop phaseへ一箇所で接続する。
pub(crate) fn append_node_with_main_loop_spacing(
    list: &mut Vec<Node>,
    list_state: &mut ScriptSpacingListState,
    mut node: Node,
    eqtb: &Eqtb,
) {
    // `\inhibitglue`はnode一個で消費する。実glyph間のJFM計画だけは、消費前の値を使う。
    let jfm_glue = list_state.take_pending_jfm_glue();
    if let Some(current) = observe_glyph_boundary(&node, eqtb) {
        list_state.observe_with_profile(
            current.atom,
            eqtb.script_spacing_activation_id(),
            eqtb.language_region(),
        );
        let previous = list_state.observe_main_loop_boundary(current.atom);
        if let Some((left, continuity, broken_left_was_inhibited)) = previous {
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
            let right_half = append_main_loop_event(list, event, jfm_glue, eqtb);
            if continuity == JfmPairContinuity::Broken {
                let Node::WideChar(ref mut wide) = node else {
                    unreachable!("Japanese boundary is represented by a WideChar node")
                };
                wide.jfm_boundary_before = if broken_left_was_inhibited || right_half.inhibited {
                    JfmBoundaryBefore::InhibitedByMainLoop
                } else if left_half_has_jfm || right_half.materialized {
                    JfmBoundaryBefore::ReplacedByMainLoopJfm
                } else {
                    JfmBoundaryBefore::BrokenNeedsKanjiSkip
                };
            } else if right_half.inhibited {
                let Node::WideChar(ref mut wide) = node else {
                    unreachable!("JFM inhibition can only belong to a WideChar boundary")
                };
                wide.jfm_boundary_before = JfmBoundaryBefore::InhibitedByMainLoop;
            }
        }
        list.push(node);
        return;
    }

    if node.contains_horizontal_japanese_glyph() {
        list_state.observe_japanese();
    }
    if node.is_automatic_script_spacing() {
        list_state.observe_existing_compiled_spacing();
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
    let outcome = append_main_loop_event(
        list,
        MainLoopBoundaryEvent::BreakAfterJapanese { left },
        list_state.pending_jfm_glue(),
        eqtb,
    );
    list_state.record_broken_left_jfm_inhibition(outcome.inhibited);
}

fn append_main_loop_event(
    list: &mut Vec<Node>,
    event: MainLoopBoundaryEvent,
    jfm_glue: JfmGlueControl,
    eqtb: &Eqtb,
) -> MainLoopJfmOutcome {
    let jfm_pairs = pair_table_for_main_loop_event(event, eqtb);
    let plan = JapaneseSpacingPlanner::built_in_ptex()
        .plan_main_loop_event(event, jfm_glue, jfm_pairs);
    let outcome = MainLoopJfmOutcome {
        materialized: plan.has_jfm_spacing(),
        inhibited: plan.jfm_spacing_inhibited(),
    };
    list.extend(
        plan.actions_for_phase(SpacingActionPhase::MainLoop)
            .map(|action| materialize_action(action, KanjiSkipProvenance::Virtual)),
    );
    outcome
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
    let _ = list_state.finalize_if_needed(|| {
        let (activation, region) = list_state
            .compiled_profile_context()
            .map_or((None, Default::default()), |(activation, region)| {
                (Some(activation), region)
            });
        let profile = eqtb.select_script_spacing_profile(activation, region);
        finalize_horizontal_list(list, eqtb, profile);
    });
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

fn finalize_horizontal_list(
    list: &mut Vec<Node>,
    eqtb: &Eqtb,
    profile: ScriptSpacingProfileRef<'_>,
) {
    let state = spacing_state_snapshot(eqtb);
    let planner = JapaneseSpacingPlanner::built_in_ptex();
    let original = std::mem::take(list);
    let mut rebuilt = Vec::with_capacity(original.len());
    let mut previous = None;
    let mut pending_disc = None;

    for mut node in original {
        if is_list_finalizer_spacing(&node) {
            continue;
        }

        if let Some(current) = observe_glyph_boundary(&node, eqtb) {
            let current = BoundaryEndpoint {
                boundary: current,
                indirect_edge: false,
            };
            if let Some(pending) = pending_disc.take() {
                append_disc_right_spacing(
                    &mut rebuilt,
                    pending,
                    current,
                    &planner,
                    &state,
                    eqtb,
                    profile,
                );
            }
            if let Some(left) = previous {
                append_boundary_spacing(
                    &mut rebuilt,
                    left,
                    current,
                    &planner,
                    &state,
                    eqtb,
                    profile,
                );
            }
            previous = Some(current);
            rebuilt.push(node);
        } else if let Some(edges) = observe_unshifted_hbox_edges(&node, eqtb) {
            if let (Some(pending), Some(first)) = (pending_disc.take(), edges.first) {
                append_disc_right_spacing(
                    &mut rebuilt,
                    pending,
                    BoundaryEndpoint {
                        boundary: first,
                        indirect_edge: true,
                    },
                    &planner,
                    &state,
                    eqtb,
                    profile,
                );
            }
            if let (Some(left), Some(first)) = (previous, edges.first) {
                append_boundary_spacing(
                    &mut rebuilt,
                    left,
                    BoundaryEndpoint {
                        boundary: first,
                        indirect_edge: true,
                    },
                    &planner,
                    &state,
                    eqtb,
                    profile,
                );
            }
            rebuilt.push(node);
            previous = edges.last.map(|boundary| BoundaryEndpoint {
                boundary,
                indirect_edge: true,
            });
        } else if let Node::Disc(disc) = &mut node {
            // 右側K/Xは外側nodeにせず、実際に選ばれる枝へ保持する。再finalizeでは
            // 前回の条件付き末尾だけを除き、枝内のVirtual K/Xは残す。
            remove_trailing_disc_right_spacing(&mut disc.no_break);
            remove_trailing_disc_right_spacing(&mut disc.post_break);
            let edges = observe_disc_right_edges(disc, eqtb);
            previous = None;
            let rebuilt_index = rebuilt.len();
            rebuilt.push(node);
            pending_disc = Some(PendingDiscRightEdges {
                rebuilt_index,
                edges,
            });
        } else if is_main_loop_spacing(&node) {
            // JFM/禁則は利用者が途中で観測・除去できる。closeでは保持し、元glyphの
            // K/X境界だけを連続させる。
            rebuilt.push(node);
        } else if matches!(node, Node::Penalty(_)) {
            // 明示penaltyは文字境界を保つ。planner actionはこのnodeの後ろへ置かれる。
            rebuilt.push(node);
        } else {
            // 明示glue/kern/math/whatsit/list/rule等を越えて自動間隔を作らない。
            previous = None;
            pending_disc = None;
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
    profile: ScriptSpacingProfileRef<'_>,
) {
    let indirect_edge = left.indirect_edge || right.indirect_edge;
    let jfm_pairs = (!indirect_edge)
        .then(|| pair_table_for_boundary(left.boundary, right.boundary, eqtb))
        .flatten();
    if let ScriptSpacingProfileRef::CompiledTable { table, region } = profile {
        // Region provenance for indirect box/discretionary edges is not yet stored in nodes.
        // Applying a close-time region there would be silently wrong, so keep the complete
        // boundary on the native path until the typed region marker checkpoint lands.
        if !indirect_edge {
            let left_scalar =
                char::from_u32(left.boundary.atom.trailing_character().to_public_integer());
            let right_scalar =
                char::from_u32(right.boundary.atom.leading_character().to_public_integer());
            if let (Some(left_scalar), Some(right_scalar)) = (left_scalar, right_scalar) {
                if let (Some(left_class), Some(right_class)) = (
                    table.classify_scalar(left_scalar, region, WritingMode::Horizontal),
                    table.classify_scalar(right_scalar, region, WritingMode::Horizontal),
                ) {
                    let action =
                        table.action_for(left_class, right_class, region, WritingMode::Horizontal);
                    if append_compiled_boundary_action(
                        rebuilt,
                        action,
                        left.boundary,
                        right.boundary,
                        state,
                        eqtb,
                    ) {
                        return;
                    }
                }
            }
        }
    }

    append_built_in_boundary_spacing(rebuilt, left, right, planner, state, jfm_pairs);
}

fn append_built_in_boundary_spacing(
    rebuilt: &mut Vec<Node>,
    left: BoundaryEndpoint,
    right: BoundaryEndpoint,
    planner: &JapaneseSpacingPlanner,
    state: &PtexSpacingState,
    jfm_pairs: Option<&CompiledJfmPairSpacingTable>,
) {
    let indirect_edge = left.indirect_edge || right.indirect_edge;
    let context = if indirect_edge {
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
                JfmBoundaryBefore::InhibitedByMainLoop => {
                    JfmPairContinuity::ReplacedByMainLoopJfm
                }
            },
            jfm_glue: JfmGlueControl::Allow,
        }
    };
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
                    if indirect_edge {
                        KanjiSkipProvenance::Material
                    } else {
                        KanjiSkipProvenance::Virtual
                    },
                )
            }),
    );
}

/// Returns `true` when the compiled table made a complete decision for this boundary. A
/// context-dependent length that cannot be resolved falls back before any node is appended.
fn append_compiled_boundary_action(
    rebuilt: &mut Vec<Node>,
    action: ScriptSpacingAction,
    left: ObservedBoundary,
    right: ObservedBoundary,
    state: &PtexSpacingState,
    eqtb: &Eqtb,
) -> bool {
    match action {
        ScriptSpacingAction::BuiltInFallback => false,
        ScriptSpacingAction::NoAutomaticSpace => true,
        ScriptSpacingAction::KanjiSkip => {
            let glue = if state.auto().kanji() {
                state.kanji_skip()
            } else {
                FixedGlue::ZERO
            };
            rebuilt.push(automatic_script_spacing_glue(glue));
            true
        }
        ScriptSpacingAction::XKanjiSkip => {
            let glue = if state.auto().xkanji() {
                state.xkanji_skip()
            } else {
                FixedGlue::ZERO
            };
            rebuilt.push(automatic_script_spacing_glue(glue));
            true
        }
        ScriptSpacingAction::FixedGlue(glue) => {
            rebuilt.push(automatic_script_spacing_glue(glue));
            true
        }
        ScriptSpacingAction::BoundaryRule(rule) => {
            let Some(metrics) = boundary_metric_snapshot(left, right, eqtb) else {
                return false;
            };
            let Ok(resolved) = rule.resolve(metrics) else {
                return false;
            };
            // Adjustment tiers and discard-at-line-edge need the later line-breaking slice. Do
            // not silently erase their meaning merely because the glue dimensions resolved.
            if resolved.tiers() != (0, 0)
                || matches!(
                    resolved.line_edge_rule(),
                    ScriptSpacingLineEdgeRule::DiscardAtStart
                        | ScriptSpacingLineEdgeRule::DiscardAtEnd
                        | ScriptSpacingLineEdgeRule::DiscardAtBoth
                )
            {
                return false;
            }
            if resolved.break_rule() != ScriptSpacingBreakRule::UseBuiltIn {
                remove_current_boundary_built_in_kinsoku(rebuilt);
            }
            let penalty = match resolved.break_rule() {
                ScriptSpacingBreakRule::UseBuiltIn | ScriptSpacingBreakRule::Allow => None,
                ScriptSpacingBreakRule::Forbid => Some(10_000),
                ScriptSpacingBreakRule::Penalty => Some(resolved.penalty()),
            };
            if let Some(penalty) = penalty {
                rebuilt.push(Node::Penalty(PenaltyNode::new_automatic_script_spacing(
                    penalty,
                )));
            }
            rebuilt.push(automatic_script_spacing_glue(resolved.glue()));
            true
        }
    }
}

fn remove_current_boundary_built_in_kinsoku(rebuilt: &mut Vec<Node>) {
    let boundary_start = rebuilt
        .iter()
        .rposition(|node| matches!(node, Node::Char(_) | Node::Ligature(_) | Node::WideChar(_)))
        .map_or(0, |index| index + 1);
    let mut index = rebuilt.len();
    while index > boundary_start {
        index -= 1;
        if matches!(
            rebuilt[index],
            Node::Penalty(PenaltyNode {
                subtype: PenaltySubtype::AutomaticJapaneseKinsoku,
                ..
            })
        ) {
            rebuilt.remove(index);
        }
    }
}

fn boundary_metric_snapshot(
    left: ObservedBoundary,
    right: ObservedBoundary,
    eqtb: &Eqtb,
) -> Option<BoundaryMetricSnapshot> {
    let (left_em, left_zw) = boundary_metrics(left, eqtb)?;
    let (right_em, right_zw) = boundary_metrics(right, eqtb)?;
    BoundaryMetricSnapshot::new(left_em, right_em, left_zw, right_zw).ok()
}

fn boundary_metrics(boundary: ObservedBoundary, eqtb: &Eqtb) -> Option<(i32, i32)> {
    if let Some(font) = boundary.japanese_font {
        let zw = eqtb.japanese_fonts.get(font.position())?.zw();
        // A JFM has no independent TeX fontdimen quad. Its scaled class-0 advance is the stable
        // em-like metric available at this host-owned boundary.
        return Some((zw, zw));
    }
    let font = boundary.latin_font?;
    let em = eqtb.fonts.get(font as usize)?.quad();
    // Latin TFM has no JFM zw; the existing PraTeX `zw` fallback is the current TFM em.
    Some((em, em))
}

fn append_disc_right_spacing(
    rebuilt: &mut [Node],
    pending: PendingDiscRightEdges,
    right: BoundaryEndpoint,
    planner: &JapaneseSpacingPlanner,
    state: &PtexSpacingState,
    eqtb: &Eqtb,
    profile: ScriptSpacingProfileRef<'_>,
) {
    let Node::Disc(disc) = &mut rebuilt[pending.rebuilt_index] else {
        unreachable!("pending discretionary index must name a DiscNode")
    };
    if let Some(left) = pending.edges.no_break {
        append_boundary_spacing(
            &mut disc.no_break,
            BoundaryEndpoint {
                boundary: left,
                indirect_edge: true,
            },
            right,
            planner,
            state,
            eqtb,
            profile,
        );
    }
    if let Some(left) = pending.edges.post_break {
        append_boundary_spacing(
            &mut disc.post_break,
            BoundaryEndpoint {
                boundary: left,
                indirect_edge: true,
            },
            right,
            planner,
            state,
            eqtb,
            profile,
        );
    }
}

fn remove_trailing_disc_right_spacing(nodes: &mut Vec<Node>) {
    while matches!(
        nodes.last(),
        Some(Node::Glue(GlueNode {
            subtype: GlueType::AutomaticJapanese(
                AutomaticJapaneseGlue::MaterialKanjiSkip | AutomaticJapaneseGlue::XKanjiSkip
            ),
            ..
        }))
    ) {
        nodes.pop();
    }
}

fn observe_disc_right_edges(disc: &crate::nodes::DiscNode, eqtb: &Eqtb) -> ObservedDiscRightEdges {
    let last = |nodes: &[Node]| match scan_hbox_edge(nodes, eqtb, true) {
        HboxEdgeScan::Found(boundary) => Some(boundary),
        HboxEdgeScan::Empty | HboxEdgeScan::Blocked => None,
    };
    ObservedDiscRightEdges {
        no_break: last(&disc.no_break),
        post_break: last(&disc.post_break),
    }
}

fn observe_glyph_boundary(node: &Node, eqtb: &Eqtb) -> Option<ObservedBoundary> {
    match node {
        Node::Char(character) => Some(ObservedBoundary {
            atom: BoundaryAtom::Latin(LatinBoundary::new(
                LayoutCharacterCode::from_public_integer(u32::from(character.character)).ok()?,
            )),
            latin_font: Some(character.font_index),
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
                latin_font: None,
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
    let Node::List(list_node) = node else {
        return None;
    };
    let (0, HlistOrVlist::Hlist(nodes)) = (list_node.shift_amount, &list_node.list) else {
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
            Node::List(list_node) => match (list_node.shift_amount, &list_node.list) {
                (0, HlistOrVlist::Hlist(nested)) => scan_hbox_edge(nested, eqtb, reverse),
                _ => HboxEdgeScan::Blocked,
            },
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
        latin_font: Some(ligature.font_index),
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
        }) | Node::Glue(GlueNode {
            subtype: GlueType::AutomaticScriptSpacing,
            ..
        }) | Node::Penalty(PenaltyNode {
            subtype: PenaltySubtype::AutomaticScriptSpacing,
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

fn automatic_script_spacing_glue(glue: FixedGlue) -> Node {
    Node::Glue(GlueNode::new_automatic_script_spacing(Rc::new(GlueSpec {
        width: glue.width(),
        stretch: glue.stretch(),
        shrink: glue.shrink(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nodes::CharNode;
    use crate::script_spacing::{
        FixedGlueProposal, ProviderRegionMask, ProviderScriptClassId, ProviderWritingModeMask,
        ScriptPairRuleProposal, ScriptSpacingActionProposal, ScriptSpacingTableProposal,
        UnicodeScalarRangeProposal,
    };

    fn character(value: u8) -> Node {
        Node::Char(CharNode {
            font_index: 0,
            character: value,
            width: 0,
            height: 0,
            depth: 0,
            italic: 0,
        })
    }

    fn fixed_table(width: i64) -> ScriptSpacingTableProposal {
        let class = |value| ProviderScriptClassId::from_wire(value);
        ScriptSpacingTableProposal::new(
            2,
            vec![
                UnicodeScalarRangeProposal::new(
                    u32::from('A'),
                    u32::from('A'),
                    class(1),
                    ProviderRegionMask::all(),
                    ProviderWritingModeMask::all(),
                ),
                UnicodeScalarRangeProposal::new(
                    u32::from('B'),
                    u32::from('B'),
                    class(2),
                    ProviderRegionMask::all(),
                    ProviderWritingModeMask::all(),
                ),
            ],
            vec![ScriptPairRuleProposal::new(
                class(1),
                class(2),
                Default::default(),
                WritingMode::Horizontal,
                ScriptSpacingActionProposal::FixedGlue(FixedGlueProposal::new(width, 0, 0, 0, 0)),
            )],
        )
    }

    #[test]
    fn 登録済み表はlist終端で固定糊を実nodeへする() {
        let mut eqtb = Eqtb::new();
        eqtb.try_install_script_spacing_table(fixed_table(123))
            .unwrap();
        let mut state = ScriptSpacingListState::default();
        let mut list = Vec::new();
        append_node_with_main_loop_spacing(&mut list, &mut state, character(b'A'), &eqtb);
        append_node_with_main_loop_spacing(&mut list, &mut state, character(b'B'), &eqtb);

        finalize_horizontal_list_if_needed(&mut list, state, &eqtb);

        assert_eq!(list.len(), 3);
        let Node::Glue(glue) = &list[1] else {
            panic!("文字対の間に糊が必要");
        };
        assert!(matches!(glue.subtype, GlueType::AutomaticScriptSpacing));
        assert_eq!(glue.glue_spec.width, 123);
        assert_eq!(crate::packaging::measure_hlist(&list).width, 123);
    }

    #[test]
    fn list途中の表交換は新旧規則を混ぜず全体を組込みへ戻す() {
        let mut eqtb = Eqtb::new();
        eqtb.try_install_script_spacing_table(fixed_table(123))
            .unwrap();
        let mut state = ScriptSpacingListState::default();
        let mut list = Vec::new();
        append_node_with_main_loop_spacing(&mut list, &mut state, character(b'A'), &eqtb);
        eqtb.try_install_script_spacing_table(fixed_table(456))
            .unwrap();
        append_node_with_main_loop_spacing(&mut list, &mut state, character(b'B'), &eqtb);

        finalize_horizontal_list_if_needed(&mut list, state, &eqtb);

        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|node| !matches!(
            node,
            Node::Glue(GlueNode {
                subtype: GlueType::AutomaticScriptSpacing,
                ..
            })
        )));
    }

    #[test]
    fn list途中のregion変更は終端値で過去文字を再分類しない() {
        let mut eqtb = Eqtb::new();
        eqtb.try_install_script_spacing_table(fixed_table(123))
            .unwrap();
        let mut state = ScriptSpacingListState::default();
        let mut list = Vec::new();
        append_node_with_main_loop_spacing(&mut list, &mut state, character(b'A'), &eqtb);
        eqtb.language_region_define(crate::eqtb::LanguageRegion::Ja, true);
        append_node_with_main_loop_spacing(&mut list, &mut state, character(b'B'), &eqtb);

        finalize_horizontal_list_if_needed(&mut list, state, &eqtb);

        assert_eq!(list.len(), 2);
    }

    #[test]
    fn fmt後にproviderが無いunboxは古いcompiled糊を残さない() {
        let mut source_eqtb = Eqtb::new();
        source_eqtb
            .try_install_script_spacing_table(fixed_table(123))
            .unwrap();
        let mut source_state = ScriptSpacingListState::default();
        let mut list = Vec::new();
        append_node_with_main_loop_spacing(
            &mut list,
            &mut source_state,
            character(b'A'),
            &source_eqtb,
        );
        append_node_with_main_loop_spacing(
            &mut list,
            &mut source_state,
            character(b'B'),
            &source_eqtb,
        );
        finalize_horizontal_list_if_needed(&mut list, source_state, &source_eqtb);
        assert!(list.iter().any(Node::is_automatic_script_spacing));

        let restored_eqtb = Eqtb::new();
        let mut restored_state = ScriptSpacingListState::default();
        restored_state.observe_existing_compiled_spacing();
        finalize_horizontal_list_if_needed(&mut list, restored_state, &restored_eqtb);

        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|node| !node.is_automatic_script_spacing()));
    }
}

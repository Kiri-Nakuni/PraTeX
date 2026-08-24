use crate::eqtb::Eqtb;
use crate::logger::Logger;
use crate::nodes::Node;
use crate::print::Printer;
use crate::script_spacing::finalizer::{
    append_node_with_main_loop_spacing, break_main_loop_jfm_continuity,
    finalize_horizontal_list_if_needed,
};
use crate::script_spacing::planner::ScriptSpacingListState;

#[derive(Debug)]
pub struct HorizontalMode {
    pub list: Vec<Node>,
    pub subtype: HorizontalModeType,
    pub space_factor: u16,
    pub(crate) script_spacing: ScriptSpacingListState,
    /// 明示`\hbox`だけが方向境界を直接保持できる最初sliceかどうか。
    ///
    /// discretionary枝とalignment spanもTeX上はrestricted horizontal modeだが、
    /// それらは独立したhlistとしてshipoutされないので別扱いにする。
    pub(crate) accepts_text_direction_boundaries: bool,
}

impl HorizontalMode {
    pub fn new_restricted() -> Self {
        Self {
            list: Vec::new(),
            subtype: HorizontalModeType::Restricted,
            space_factor: 1000,
            script_spacing: ScriptSpacingListState::default(),
            accepts_text_direction_boundaries: false,
        }
    }

    pub fn new_directional_hbox() -> Self {
        Self {
            accepts_text_direction_boundaries: true,
            ..Self::new_restricted()
        }
    }

    /// See 214.
    pub fn append_node(&mut self, node: Node, eqtb: &mut Eqtb) {
        append_node_with_main_loop_spacing(&mut self.list, &mut self.script_spacing, node, eqtb);
        self.update_last_node_info(eqtb);
    }

    pub(crate) fn observe_appended_nodes(&mut self, nodes: &[Node]) {
        if nodes.iter().any(Node::contains_horizontal_japanese_glyph) {
            self.script_spacing.observe_japanese();
        }
        if nodes.iter().any(Node::is_automatic_script_spacing) {
            self.script_spacing.observe_existing_compiled_spacing();
        }
        if !nodes.is_empty() {
            self.script_spacing.take_pending_jfm_glue();
            self.script_spacing.reset_main_loop_boundary();
        }
    }

    /// 次の実nodeまでJFM metric空白だけを抑止するone-shot状態を切り替える。
    pub(crate) fn set_inhibit_glue(&mut self, inhibit: bool) {
        self.script_spacing.set_jfm_glue_inhibited(inhibit);
    }

    /// nodeを作らないcommandの直前に、左JFMをclass 0へ一度だけ閉じる。
    pub(crate) fn break_jfm_pair_continuity(&mut self, eqtb: &mut Eqtb) {
        break_main_loop_jfm_continuity(&mut self.list, &mut self.script_spacing, eqtb);
        self.update_last_node_info(eqtb);
    }

    pub(crate) fn finalize_script_spacing(&mut self, eqtb: &mut Eqtb) {
        let list_state = std::mem::take(&mut self.script_spacing);
        finalize_horizontal_list_if_needed(&mut self.list, list_state, eqtb);
        self.update_last_node_info(eqtb);
    }

    /// See 219.
    pub fn show_fields(&self, logger: &mut Logger) {
        logger.print_nl_str("spacefactor ");
        logger.print_int(self.space_factor as i32);
        if let HorizontalModeType::Unrestricted { clang, .. } = self.subtype {
            if clang > 0 {
                logger.print_str(", current language ");
                logger.print_int(clang as i32);
            }
        }
    }

    pub fn set_space_factor(&mut self, value: u16, eqtb: &mut Eqtb) {
        self.space_factor = value;
        // Keep copy in Eqtb updated
        eqtb.space_factor = self.space_factor;
    }

    /// Updates the condensed info about the last node of the current list that we keep in the
    /// Eqtb.
    pub fn update_last_node_info(&mut self, eqtb: &mut Eqtb) {
        eqtb.update_last_node_info(Node::last_tex_observable(&self.list));
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HorizontalModeType {
    Restricted,
    Unrestricted { clang: u16, lang_data: LanguageData },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageData {
    pub left_hyphen_min: usize,
    pub right_hyphen_min: usize,
    pub language: usize,
}

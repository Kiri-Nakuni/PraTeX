use crate::eqtb::Eqtb;
use crate::logger::Logger;
use crate::nodes::Node;
use crate::print::Printer;
use crate::script_spacing::finalizer::finalize_horizontal_list_if_needed;
use crate::script_spacing::planner::ScriptSpacingListState;

#[derive(Debug)]
pub struct HorizontalMode {
    pub list: Vec<Node>,
    pub subtype: HorizontalModeType,
    pub space_factor: u16,
    pub(crate) script_spacing: ScriptSpacingListState,
}

impl HorizontalMode {
    pub fn new_restricted() -> Self {
        Self {
            list: Vec::new(),
            subtype: HorizontalModeType::Restricted,
            space_factor: 1000,
            script_spacing: ScriptSpacingListState::default(),
        }
    }

    /// See 214.
    pub fn append_node(&mut self, node: Node, eqtb: &mut Eqtb) {
        if node.contains_horizontal_japanese_glyph() {
            self.script_spacing.observe_japanese();
        }
        self.list.push(node);
        self.update_last_node_info(eqtb);
    }

    pub(crate) fn observe_appended_nodes(&mut self, nodes: &[Node]) {
        if nodes.iter().any(Node::contains_horizontal_japanese_glyph) {
            self.script_spacing.observe_japanese();
        }
    }

    pub(crate) fn finalize_script_spacing(&mut self, eqtb: &mut Eqtb) {
        finalize_horizontal_list_if_needed(&mut self.list, self.script_spacing, eqtb);
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

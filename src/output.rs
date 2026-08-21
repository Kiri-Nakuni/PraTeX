use crate::dimension::{is_running, Dimension, MAX_DIMEN};
use crate::eqtb::{
    ControlSequence, DimensionVariable, Eqtb, FontIndex, IntegerVariable, RegisterIndex, NULL_FONT,
};
use crate::input::token_source::TokenSourceType;
use crate::input::Scanner;
use crate::logger::Logger;
use crate::nodes::{
    show_box, CharNode, DimensionOrder, GlueNode, GlueSign, GlueType, HlistOrVlist, LeaderKind,
    LigatureNode, ListNode, Node, OpenNode, RuleNode, SpecialNode, WhatsitNode, WriteNode,
};
use crate::print::stream::StreamPrinter;
use crate::print::string::StringPrinter;
use crate::print::{Printer, MAX_PRINT_LINE};
use crate::run_options::OutputFormat;
use crate::scaled::Scaled;
use crate::token::Token;
use crate::token_lists::{show_token_list, token_show};
use crate::{open_out, round};

use std::ffi::OsString;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

#[path = "output_backend.rs"]
mod output_backend;
#[path = "pdf_backend.rs"]
mod pdf_backend;

use output_backend::{DviBackend, ShipoutBackend};
use pdf_backend::PdfBackend;

type DviFileBackend = DviBackend<BufWriter<File>>;
type DviDocument = Document<DviFileBackend>;
type PdfFileBackend = PdfBackend<BufWriter<File>>;
type PdfShipoutDocument = Document<PdfFileBackend>;

/// 出力形式の分岐はpage単位だけで行い、node走査内へ持ち込まない。
enum OutputDocument {
    Dvi(DviDocument),
    Pdf(PdfShipoutDocument),
}

impl OutputDocument {
    fn ship_box_out(&mut self, list_node: &ListNode, eqtb: &mut Eqtb) -> Vec<WhatsitNode> {
        match self {
            Self::Dvi(document) => document.ship_box_out(list_node, eqtb),
            Self::Pdf(document) => document.ship_box_out(list_node, eqtb),
        }
    }

    fn finish(self) -> (OsString, usize, usize) {
        match self {
            Self::Dvi(document) => document.finish(),
            Self::Pdf(document) => document.finish(),
        }
    }
}

const END_WRITE_TOKEN: Token = Token::CSToken {
    cs: ControlSequence::EndWrite,
};

pub struct Output {
    output_format: OutputFormat,
    document: Option<OutputDocument>,

    /// 1342.
    pub write_files: [Option<BufWriter<File>>; 16],
}

impl Output {
    pub const fn new(output_format: OutputFormat) -> Self {
        Self {
            output_format,
            document: None,
            write_files: [
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None, None,
            ],
        }
    }

    /// See 638.
    pub fn ship_out(
        &mut self,
        list_node: ListNode,
        scanner: &mut Scanner,
        eqtb: &mut Eqtb,
        logger: &mut Logger,
    ) {
        if eqtb.tracing_output() > 0 {
            logger.print_nl_str("");
            logger.print_ln();
            logger.print_str("Completed box being shipped out");
        }
        if logger.term_offset > MAX_PRINT_LINE - 9 {
            logger.print_ln();
        } else if logger.term_offset > 0 || logger.file_offset > 0 {
            logger.print_char(b' ');
        }
        logger.print_char(b'[');
        let mut j = 9;
        while (eqtb.integer(IntegerVariable::Count(j)) == 0) && (j > 0) {
            j -= 1;
        }
        for k in 0..=j {
            logger.print_int(eqtb.integer(IntegerVariable::Count(k)));
            if k < j {
                logger.print_char(b'.');
            }
        }
        logger.update_terminal();
        if eqtb.tracing_output() > 0 {
            logger.print_char(b']');
            logger.begin_diagnostic(eqtb.tracing_online());
            show_box(&list_node, eqtb, logger);
            logger.end_diagnostic(true);
        }

        if check_page_dimensions(&list_node, scanner, eqtb, logger) {
            let mut document = ensure_output_open(
                self.document.take(),
                self.output_format,
                scanner,
                eqtb,
                logger,
            );
            let whatsit_list = document.ship_box_out(&list_node, eqtb);
            for whatsit_node in whatsit_list {
                self.out_what(&whatsit_node, scanner, eqtb, logger);
            }
            self.document = Some(document);
        }

        if eqtb.tracing_output() <= 0 {
            logger.print_char(b']');
        }
        eqtb.dead_cycles = 0;
        logger.update_terminal();
        if cfg!(feature = "stats") {
            if eqtb.integer(IntegerVariable::TracingStats) > 1 {
                logger.print_nl_str("Memory usage display not supported");
                logger.print_ln();
            }
        }
    }

    /// See 642.
    pub fn finish_output_file(self, logger: &mut Logger) {
        if let Some(document) = self.document {
            let (output_file_name, page_count, byte_count) = document.finish();
            logger.print_nl_str("Output written on ");
            logger.slow_print_str(output_file_name.as_encoded_bytes());
            logger.print_str(" (");
            logger.print_int(page_count as i32);
            logger.print_str(" page");
            if page_count != 1 {
                logger.print_char(b's');
            }
            logger.print_str(", ");
            logger.print_int(byte_count as i32);
            logger.print_str(" bytes).");
        } else {
            logger.print_nl_str("No pages of output.");
        }
    }

    /// See 1373. and 1374.
    pub fn out_what(
        &mut self,
        node: &WhatsitNode,
        scanner: &mut Scanner,
        eqtb: &mut Eqtb,
        logger: &mut Logger,
    ) {
        match node {
            WhatsitNode::Open(node) => {
                open_write_file(node, self, scanner, eqtb, logger);
            }
            WhatsitNode::Write(node) => {
                write_out(node, self, scanner, eqtb, logger);
            }
            WhatsitNode::Close(close_node) => {
                if close_node.write_stream < 16 {
                    self.write_files[close_node.write_stream] = None;
                }
            }
            WhatsitNode::Special(_) | WhatsitNode::Language(_) => {}
        }
    }
}

/// Returns false it for goto done, true otherwise.
/// The last part has been movoed to DviWriter.
/// See 641.
fn check_page_dimensions(
    list_node: &ListNode,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> bool {
    if list_node.height > MAX_DIMEN
        || list_node.depth > MAX_DIMEN
        || list_node.height + list_node.depth + eqtb.dimen(DimensionVariable::VOffset) > MAX_DIMEN
        || list_node.width + eqtb.dimen(DimensionVariable::HOffset) > MAX_DIMEN
    {
        logger.print_err("Huge page cannot be shipped out");
        let help = &[
            "The page just created is more than 18 feet tall or",
            "more than 18 feet wide, so I suspect something went wrong.",
        ];
        logger.error(help, scanner, eqtb);
        // If \tracingoutput is positive, we have already printed a description of the box,
        // which is why we only printed it here for non-positive values of \tracingoutput.
        if eqtb.tracing_output() <= 0 {
            logger.begin_diagnostic(eqtb.tracing_online());
            logger.print_nl_str("The following box has been deleted:");
            show_box(list_node, eqtb, logger);
            logger.end_diagnostic(true);
        }
        false
    } else {
        true
    }
}

/// See 532. and 617.
fn ensure_output_open(
    document: Option<OutputDocument>,
    output_format: OutputFormat,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> OutputDocument {
    match document {
        Some(output) => output,
        None => {
            let extension = match output_format {
                OutputFormat::Dvi => "dvi",
                OutputFormat::Pdf => "pdf",
            };
            let (output_file_name, output_file) =
                open_output_file(extension, scanner, eqtb, logger);
            // TeX82 と同じく log/output file を開いた後で mag を検査し、診断を transcript に残す。
            eqtb.prepare_mag(scanner, logger);
            match output_format {
                OutputFormat::Dvi => {
                    OutputDocument::Dvi(Document::create_dvi(output_file_name, output_file, eqtb))
                }
                OutputFormat::Pdf => {
                    OutputDocument::Pdf(Document::create_pdf(output_file_name, output_file, eqtb))
                }
            }
        }
    }
}

fn open_output_file(
    extension: &str,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> (OsString, BufWriter<File>) {
    if logger.job_name.is_none() {
        logger.open_log_file(&scanner.input_stack, eqtb);
    }
    let mut initial_name = logger.job_name.as_ref().unwrap().clone();
    initial_name.push(".");
    initial_name.push(extension);
    let mut path = PathBuf::from(initial_name);
    let output_file = loop {
        match open_out(&path) {
            Ok(file) => break BufWriter::new(file),
            Err(_) => {
                path = logger.prompt_file_name(
                    &path,
                    "file name for output",
                    extension,
                    &scanner.input_stack,
                    eqtb,
                );
            }
        }
    };
    (path.into_os_string(), output_file)
}

struct Document<B: ShipoutBackend> {
    backend: B,
    /// See 532.
    output_file_name: OsString,

    /// See 616.
    cur_v: Dimension,
    /// See 616.
    cur_h: Dimension,
    /// See 616.
    dvi_h: Dimension,
    /// See 616.
    dvi_v: Dimension,
    /// See 616.
    /// backendで現在選択中のfont。
    dvi_f: FontIndex,
    /// このbackendへ定義済みのfont。fmtに保存されたDVI固有flagには依存しない。
    defined_fonts: Vec<bool>,
    /// The current level of nesting. Starts at -1.
    /// See 616.
    cur_s: i32,
}

impl Document<DviFileBackend> {
    fn create_dvi(output_file_name: OsString, dvi_file: BufWriter<File>, eqtb: &Eqtb) -> Self {
        let comment = format!(
            " rtex output {}.{:02}.{:02}:{:02}{:02}",
            eqtb.integer(IntegerVariable::Year),
            eqtb.integer(IntegerVariable::Month),
            eqtb.integer(IntegerVariable::Day),
            eqtb.integer(IntegerVariable::Time) / 60,
            eqtb.integer(IntegerVariable::Time) % 60
        );

        let backend = DviBackend::new(
            dvi_file,
            eqtb.integer(IntegerVariable::Mag),
            comment.as_bytes(),
        )
        .unwrap();
        Self::new(backend, output_file_name)
    }
}

impl Document<PdfFileBackend> {
    fn create_pdf(output_file_name: OsString, pdf_file: BufWriter<File>, eqtb: &Eqtb) -> Self {
        let backend = PdfBackend::new(pdf_file, eqtb.integer(IntegerVariable::Mag)).unwrap();
        Self::new(backend, output_file_name)
    }
}

impl<B: ShipoutBackend> Document<B> {
    fn new(backend: B, output_file_name: OsString) -> Self {
        Self {
            backend,
            output_file_name,
            cur_h: 0,
            cur_v: 0,
            dvi_h: 0,
            dvi_v: 0,
            dvi_f: NULL_FONT,
            defined_fonts: Vec::new(),
            cur_s: -1,
        }
    }

    fn finish(self) -> (OsString, usize, usize) {
        let page_count = self.backend.page_count();
        let byte_count = self.backend.finish().unwrap();
        (self.output_file_name, page_count, byte_count)
    }

    /// See 617. and 640.
    fn ship_box_out(&mut self, list_node: &ListNode, eqtb: &mut Eqtb) -> Vec<WhatsitNode> {
        self.dvi_h = 0;
        self.dvi_v = 0;
        self.cur_h = eqtb.dimen(DimensionVariable::HOffset);
        self.dvi_f = NULL_FONT;

        let page_height =
            list_node.height + list_node.depth + eqtb.dimen(DimensionVariable::VOffset);
        let page_width = list_node.width + eqtb.dimen(DimensionVariable::HOffset);

        let mut counts = [0; 10];
        for k in 0..10 {
            counts[k] = eqtb.integer(IntegerVariable::Count(k as RegisterIndex));
        }
        self.backend
            .start_page(&counts, page_height, page_width)
            .unwrap();
        self.cur_v = list_node.height + eqtb.dimen(DimensionVariable::VOffset);
        let mut whatsit_list = Vec::new();
        match &list_node.list {
            HlistOrVlist::Vlist(_) => self.vlist_out(list_node, &mut whatsit_list, eqtb),
            HlistOrVlist::Hlist(_) => self.hlist_out(list_node, &mut whatsit_list, eqtb),
        };
        self.backend.end_page().unwrap();
        self.cur_s = -1;
        whatsit_list
    }

    /// See 616.
    fn synch_h(&mut self) {
        if self.cur_h != self.dvi_h {
            self.backend.move_right(self.cur_h - self.dvi_h).unwrap();
            self.dvi_h = self.cur_h;
        }
    }

    /// See 616.
    fn synch_v(&mut self) {
        if self.cur_v != self.dvi_v {
            self.backend.move_down(self.cur_v - self.dvi_v).unwrap();
            self.dvi_v = self.cur_v;
        }
    }

    /// See 619.
    fn hlist_out(
        &mut self,
        this_box: &ListNode,
        whatsit_list: &mut Vec<WhatsitNode>,
        eqtb: &mut Eqtb,
    ) {
        let mut cur_g = 0;
        let mut cur_glue = 0.0;
        let g_order = this_box.glue_order;
        let g_sign = this_box.glue_sign;
        let hlist = match &this_box.list {
            HlistOrVlist::Hlist(hlist) => hlist,
            _ => panic!("Should not happen"),
        };
        self.cur_s += 1;
        if self.cur_s > 0 {
            self.backend.push().unwrap();
        }
        let base_line = self.cur_v;
        let left_edge = self.cur_h;
        for node in hlist {
            self.output_node_p_for_hlist_out(
                node,
                this_box,
                base_line,
                left_edge,
                &mut cur_g,
                &mut cur_glue,
                g_order,
                g_sign,
                whatsit_list,
                eqtb,
            );
        }
        if self.cur_s > 0 {
            self.backend.pop().unwrap();
        }
        self.cur_s -= 1;
    }

    /// See 620. and 622.
    fn output_node_p_for_hlist_out(
        &mut self,
        node: &Node,
        this_box: &ListNode,
        base_line: Scaled,
        left_edge: Scaled,
        cur_g: &mut Scaled,
        cur_glue: &mut f64,
        g_order: DimensionOrder,
        g_sign: GlueSign,
        whatsit_list: &mut Vec<WhatsitNode>,
        eqtb: &mut Eqtb,
    ) {
        match node {
            &Node::Char(CharNode {
                font_index,
                character,
                width,
                ..
            })
            | &Node::Ligature(LigatureNode {
                font_index,
                character,
                width,
                ..
            }) => {
                self.synch_h();
                self.synch_v();
                self.output_char(font_index, character, width, eqtb);
                self.dvi_h = self.cur_h;
            }
            Node::Glue(glue_node) => {
                self.move_right_or_output_leaders(
                    glue_node, this_box, base_line, left_edge, cur_g, cur_glue, g_order, g_sign,
                    eqtb,
                );
            }
            Node::Kern(kern_node) => {
                self.cur_h += kern_node.width;
            }
            Node::List(list_node) => {
                self.output_box_in_hlist(list_node, base_line, whatsit_list, eqtb)
            }
            Node::Math(math_node) => {
                self.cur_h += math_node.width;
            }
            Node::Disc(disc_node) => {
                for node in &disc_node.no_break {
                    self.output_node_p_for_hlist_out(
                        node,
                        this_box,
                        base_line,
                        left_edge,
                        cur_g,
                        cur_glue,
                        g_order,
                        g_sign,
                        whatsit_list,
                        eqtb,
                    );
                }
            }
            Node::Rule(rule_node) => {
                self.output_rule_in_hlist(rule_node, rule_node.width, this_box, base_line);
            }
            Node::Whatsit(whatsit_node) => {
                if let WhatsitNode::Special(special_node) = whatsit_node {
                    self.special_out(special_node, eqtb);
                } else {
                    whatsit_list.push(whatsit_node.clone());
                }
            }
            Node::Penalty(_) => {}

            Node::Ins(_)
            | Node::Mark(_)
            | Node::Adjust(_)
            | Node::Unset(_)
            | Node::Noad(_)
            | Node::Choice(_)
            | Node::Style(_) => {}
        }
    }

    /// See 620.
    fn output_char(&mut self, font_index: FontIndex, chr: u8, width: Dimension, eqtb: &mut Eqtb) {
        if font_index != self.dvi_f {
            self.change_font(font_index, eqtb);
        }
        self.backend.set_char(chr, width).unwrap();
        self.cur_h += width;
    }

    /// See 621.
    fn change_font(&mut self, font_index: FontIndex, eqtb: &mut Eqtb) {
        let Some(font_number) = font_index.checked_sub(1).map(u32::from) else {
            panic!("Null font cannot be selected for output");
        };
        let font_position = usize::from(font_index);
        if self.defined_fonts.len() <= font_position {
            self.defined_fonts.resize(font_position + 1, false);
        }
        if !self.defined_fonts[font_position] {
            let checksum = eqtb.fonts[font_position].check;
            let at_size = eqtb.fonts[font_position].size;
            let design_size = eqtb.fonts[font_position].dsize;
            let area = &eqtb.fonts[font_position].area;
            let name = &eqtb.fonts[font_position].name;
            self.backend
                .define_font(font_number, checksum, at_size, design_size, area, name)
                .unwrap();
            self.defined_fonts[font_position] = true;
        }
        self.backend.set_font(font_number).unwrap();
        self.dvi_f = font_index;
    }

    /// See 623.
    fn output_box_in_hlist(
        &mut self,
        node: &ListNode,
        base_line: Scaled,
        whatsit_list: &mut Vec<WhatsitNode>,
        eqtb: &mut Eqtb,
    ) {
        match &node.list {
            HlistOrVlist::Vlist(vlist) if vlist.is_empty() => {
                self.cur_h += node.width;
                return;
            }
            HlistOrVlist::Hlist(hlist) if hlist.is_empty() => {
                self.cur_h += node.width;
                return;
            }
            _ => {}
        }
        let save_h = self.dvi_h;
        let save_v = self.dvi_v;
        self.cur_v = base_line + node.shift_amount;
        let edge = self.cur_h;
        match node.list {
            HlistOrVlist::Vlist(_) => {
                self.vlist_out(node, whatsit_list, eqtb);
            }
            HlistOrVlist::Hlist(_) => {
                self.hlist_out(node, whatsit_list, eqtb);
            }
        }
        self.dvi_h = save_h;
        self.dvi_v = save_v;
        self.cur_h = edge + node.width;
        self.cur_v = base_line;
    }

    /// See 624.
    fn output_rule_in_hlist(
        &mut self,
        rule_node: &RuleNode,
        width: Dimension,
        this_box: &ListNode,
        base_line: Scaled,
    ) {
        let mut height = if is_running(rule_node.height) {
            this_box.height
        } else {
            rule_node.height
        };
        let depth = if is_running(rule_node.depth) {
            this_box.depth
        } else {
            rule_node.depth
        };
        height += depth;
        if (height > 0) && (width > 0) {
            self.synch_h();
            self.cur_v = base_line + depth;
            self.synch_v();
            self.backend.set_rule(height, width).unwrap();
            self.cur_v = base_line;
            self.dvi_h += width;
        }
        // move_past:
        self.cur_h += width;
    }

    /// Return false for goto move_past, true for goto next_p.
    /// See 625.
    fn move_right_or_output_leaders(
        &mut self,
        glue_node: &GlueNode,
        this_box: &ListNode,
        base_line: Scaled,
        left_edge: Scaled,
        cur_g: &mut Scaled,
        cur_glue: &mut f64,
        g_order: DimensionOrder,
        g_sign: GlueSign,
        eqtb: &mut Eqtb,
    ) {
        let glue_spec = &glue_node.glue_spec;
        let prev_cur_g = *cur_g;
        if g_sign != GlueSign::Normal {
            if g_sign == GlueSign::Stretching {
                if glue_spec.stretch.order == g_order {
                    *cur_glue += glue_spec.stretch.value as f64;
                    let glue_temp = vet_glue(this_box.glue_set * *cur_glue);
                    *cur_g = round(glue_temp);
                }
            } else {
                if glue_spec.shrink.order == g_order {
                    *cur_glue -= glue_spec.shrink.value as f64;
                    let glue_temp = vet_glue(this_box.glue_set * *cur_glue);
                    *cur_g = round(glue_temp);
                }
            }
        }
        let width = glue_spec.width + (*cur_g - prev_cur_g);
        if let GlueType::Leaders {
            leader_kind,
            ref leader_node,
        } = glue_node.subtype
        {
            self.output_leaders_in_hlist(
                leader_kind,
                leader_node,
                width,
                this_box,
                base_line,
                left_edge,
                eqtb,
            );
        } else {
            self.cur_h += width;
        }
    }

    /// Return false for goto move_past, true for goto next_p.
    /// See 626., 627. and 628.
    fn output_leaders_in_hlist(
        &mut self,
        leader_kind: LeaderKind,
        leader_node: &Node,
        mut width: Dimension,
        this_box: &ListNode,
        base_line: Scaled,
        left_edge: Scaled,
        eqtb: &mut Eqtb,
    ) {
        match leader_node {
            Node::Rule(ref rule_node) => {
                // Corresponds to goto fin_rule
                self.output_rule_in_hlist(rule_node, width, this_box, base_line);
            }
            Node::List(ref list_node) => {
                let leader_wd = list_node.width;
                if (leader_wd > 0) && (width > 0) {
                    // Compensate for f32 rounding
                    width += 10;
                    let edge = self.cur_h + width;
                    let mut lx = 0;
                    // From 627.
                    if leader_kind == LeaderKind::ALeaders {
                        let save_h = self.cur_h;
                        self.cur_h = left_edge + leader_wd * ((self.cur_h - left_edge) / leader_wd);
                        if self.cur_h < save_h {
                            self.cur_h += leader_wd;
                        }
                    } else {
                        let lq = width / leader_wd;
                        let lr = width % leader_wd;
                        if leader_kind == LeaderKind::CLeaders {
                            self.cur_h += lr / 2;
                        } else {
                            lx = lr / (lq + 1);
                            self.cur_h += (lr - (lq - 1) * lx) / 2;
                        }
                    }
                    while self.cur_h + leader_wd <= edge {
                        // From 628.
                        self.cur_v = base_line + list_node.shift_amount;
                        self.synch_v();
                        let save_v = self.dvi_v;
                        self.synch_h();
                        let save_h = self.dvi_h;
                        // We ignore Whatits in leaders (see 1374.).
                        let mut ignored_whatsits = Vec::new();
                        match list_node.list {
                            HlistOrVlist::Vlist(_) => {
                                self.vlist_out(list_node, &mut ignored_whatsits, eqtb)
                            }
                            HlistOrVlist::Hlist(_) => {
                                self.hlist_out(list_node, &mut ignored_whatsits, eqtb)
                            }
                        }
                        self.dvi_v = save_v;
                        self.dvi_h = save_h;
                        self.cur_v = base_line;
                        self.cur_h = save_h + leader_wd + lx;
                    }
                    self.cur_h = edge - 10;
                } else {
                    self.cur_h += width;
                }
            }
            _ => panic!("Should not happen"),
        }
    }

    /// See 629.
    fn vlist_out(
        &mut self,
        this_box: &ListNode,
        whatsit_list: &mut Vec<WhatsitNode>,
        eqtb: &mut Eqtb,
    ) {
        let mut cur_g = 0;
        let mut cur_glue = 0.0;
        let g_order = this_box.glue_order;
        let g_sign = this_box.glue_sign;
        let vlist = match &this_box.list {
            HlistOrVlist::Vlist(vlist) => vlist,
            _ => panic!("Should not happen"),
        };
        self.cur_s += 1;
        if self.cur_s > 0 {
            self.backend.push().unwrap();
        }
        let left_edge = self.cur_h;
        self.cur_v -= this_box.height;
        let top_edge = self.cur_v;
        for node in vlist {
            self.output_node_for_vlist(
                node,
                this_box,
                left_edge,
                top_edge,
                &mut cur_g,
                &mut cur_glue,
                g_order,
                g_sign,
                whatsit_list,
                eqtb,
            );
        }
        if self.cur_s > 0 {
            self.backend.pop().unwrap();
        }
        self.cur_s -= 1;
    }

    /// See 630. and 631.
    fn output_node_for_vlist(
        &mut self,
        node: &Node,
        this_box: &ListNode,
        left_edge: Scaled,
        top_edge: Scaled,
        cur_g: &mut Scaled,
        cur_glue: &mut f64,
        g_order: DimensionOrder,
        g_sign: GlueSign,
        whatsit_list: &mut Vec<WhatsitNode>,
        eqtb: &mut Eqtb,
    ) {
        match node {
            Node::Char(_) => panic!("No CharNode should ever appear here"),
            Node::List(list_node) => {
                self.output_box_in_vlist(list_node, left_edge, whatsit_list, eqtb)
            }
            Node::Rule(rule_node) => {
                let height = rule_node.height + rule_node.depth;
                self.output_rule_in_vlist(rule_node, height, this_box);
            }
            Node::Whatsit(whatsit_node) => {
                if let WhatsitNode::Special(special_node) = whatsit_node {
                    self.special_out(special_node, eqtb);
                } else {
                    whatsit_list.push(whatsit_node.clone());
                }
            }
            Node::Glue(glue_node) => {
                self.move_down_or_output_leaders(
                    glue_node, this_box, left_edge, top_edge, cur_g, cur_glue, g_order, g_sign,
                    eqtb,
                );
            }
            Node::Kern(kern_node) => {
                self.cur_v += kern_node.width;
            }
            _ => {}
        }
    }

    /// See 632.
    fn output_box_in_vlist(
        &mut self,
        list_node: &ListNode,
        left_edge: Scaled,
        whatsit_list: &mut Vec<WhatsitNode>,
        eqtb: &mut Eqtb,
    ) {
        match &list_node.list {
            HlistOrVlist::Vlist(vlist) if vlist.is_empty() => {
                self.cur_v += list_node.height + list_node.depth;
                return;
            }
            HlistOrVlist::Hlist(hlist) if hlist.is_empty() => {
                self.cur_v += list_node.height + list_node.depth;
                return;
            }
            _ => {}
        }

        self.cur_v += list_node.height;
        self.synch_v();
        let save_h = self.dvi_h;
        let save_v = self.dvi_v;
        self.cur_h = left_edge + list_node.shift_amount;
        match &list_node.list {
            HlistOrVlist::Vlist(_) => self.vlist_out(list_node, whatsit_list, eqtb),
            HlistOrVlist::Hlist(_) => self.hlist_out(list_node, whatsit_list, eqtb),
        }
        self.dvi_h = save_h;
        self.dvi_v = save_v;
        self.cur_v = save_v + list_node.depth;
        self.cur_h = left_edge;
    }

    /// See 633.
    fn output_rule_in_vlist(
        &mut self,
        rule_node: &RuleNode,
        height: Dimension,
        this_box: &ListNode,
    ) {
        let width = if is_running(rule_node.width) {
            this_box.width
        } else {
            rule_node.width
        };
        self.cur_v += height;
        if (height > 0) && (width > 0) {
            self.synch_h();
            self.synch_v();
            self.backend.put_rule(height, width).unwrap();
        }
    }

    /// Return false for goto move_past, true for goto next_p.
    /// See 634.
    fn move_down_or_output_leaders(
        &mut self,
        glue_node: &GlueNode,
        this_box: &ListNode,
        left_edge: Scaled,
        top_edge: Scaled,
        cur_g: &mut Scaled,
        cur_glue: &mut f64,
        g_order: DimensionOrder,
        g_sign: GlueSign,
        eqtb: &mut Eqtb,
    ) {
        let spec = &glue_node.glue_spec;
        let prev_cur_g = *cur_g;
        if g_sign != GlueSign::Normal {
            if g_sign == GlueSign::Stretching {
                if spec.stretch.order == g_order {
                    *cur_glue += spec.stretch.value as f64;
                    let glue_temp = vet_glue(this_box.glue_set * *cur_glue);
                    *cur_g = round(glue_temp);
                }
            } else {
                if spec.shrink.order == g_order {
                    *cur_glue -= spec.shrink.value as f64;
                    let glue_temp = vet_glue(this_box.glue_set * *cur_glue);
                    *cur_g = round(glue_temp);
                }
            }
        }
        let height = spec.width + (*cur_g - prev_cur_g);
        if let GlueType::Leaders {
            leader_kind,
            ref leader_node,
        } = glue_node.subtype
        {
            self.output_leaders_in_vlist(
                leader_kind,
                leader_node,
                height,
                this_box,
                left_edge,
                top_edge,
                eqtb,
            );
        } else {
            self.cur_v += height;
        }
    }

    /// Return false for goto move_past, true for goto next_p.
    /// See 635., 636. and 637.
    fn output_leaders_in_vlist(
        &mut self,
        leader_kind: LeaderKind,
        leader_node: &Node,
        mut height: Dimension,
        this_box: &ListNode,
        left_edge: Scaled,
        top_edge: Scaled,
        eqtb: &mut Eqtb,
    ) {
        match leader_node {
            Node::Rule(rule_node) => {
                self.output_rule_in_vlist(rule_node, height, this_box);
            }
            Node::List(list_node) => {
                let leader_ht = list_node.height + list_node.depth;
                if (leader_ht > 0) && (height > 0) {
                    // Compensate for f32 rounding
                    height += 10;
                    let edge = self.cur_v + height;
                    let mut lx = 0;
                    // From 636.
                    if leader_kind == LeaderKind::ALeaders {
                        let save_v = self.cur_v;
                        self.cur_v = top_edge + leader_ht * ((self.cur_v - top_edge) / leader_ht);
                        if self.cur_v < save_v {
                            self.cur_v += leader_ht;
                        }
                    } else {
                        let lq = height / leader_ht;
                        let lr = height % leader_ht;
                        if leader_kind == LeaderKind::CLeaders {
                            self.cur_v += lr / 2;
                        } else {
                            lx = lr / (lq + 1);
                            self.cur_v += (lr - (lq - 1) * lx) / 2;
                        }
                    }
                    while self.cur_v + leader_ht <= edge {
                        // From 637.
                        self.cur_h = left_edge + list_node.shift_amount;
                        self.synch_h();
                        let save_h = self.dvi_h;
                        self.cur_v += list_node.height;
                        self.synch_v();
                        let save_v = self.dvi_v;
                        // We ignore Whatits in leaders (see 1374.).
                        let mut ignored_whatsits = Vec::new();
                        match list_node.list {
                            HlistOrVlist::Vlist(_) => {
                                self.vlist_out(list_node, &mut ignored_whatsits, eqtb)
                            }
                            HlistOrVlist::Hlist(_) => {
                                self.hlist_out(list_node, &mut ignored_whatsits, eqtb)
                            }
                        }
                        self.dvi_v = save_v;
                        self.dvi_h = save_h;
                        self.cur_h = left_edge;
                        self.cur_v = save_v - list_node.height + leader_ht + lx;
                    }
                    self.cur_v = edge - 10;
                } else {
                    self.cur_v += height;
                }
            }
            _ => panic!("Should not happen"),
        }
    }

    /// See 1368.
    fn special_out(&mut self, special_node: &SpecialNode, eqtb: &Eqtb) {
        self.synch_h();
        self.synch_v();
        let mut string_printer = StringPrinter::new(eqtb.get_current_escape_character());
        // We set the length limit here to a somewhat arbitrary large value. In
        // TeX82 the limit is given by the remaining space in the string pool which we
        // don't use anymore.
        show_token_list(&special_node.tokens, 1_000_000, &mut string_printer, eqtb);
        let s = string_printer.into_string();
        self.backend.write_special(&s).unwrap();
    }
}

/// See 625.
const BILLION: f64 = 1_000_000_000.0;

/// See 625.
fn vet_glue(g: f64) -> f64 {
    g.clamp(-BILLION, BILLION)
}

/// See 1370.
pub fn write_out(
    node: &WriteNode,
    output: &mut Output,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    let token_list = expand_macros_in_token_list(node, scanner, eqtb, logger);
    let j = node.write_stream;
    // If j corresponds to an open stream file, write to it.
    if j < 16 {
        if let Some(file) = &mut output.write_files[j] {
            let mut stream_printer = StreamPrinter::from_file(
                file,
                eqtb.get_current_newline_character(),
                eqtb.get_current_escape_character(),
            );
            token_show(&token_list, &mut stream_printer, eqtb);
            stream_printer.print_ln();
            return;
        }
    }
    // Alternatively write to the log file and potentially the terminal.
    let old_setting = logger.terminal_logging;
    // Print to only log file if the number is 17.
    if j == 17 && logger.terminal_logging {
        logger.terminal_logging = false;
    }
    logger.print_nl_str("");
    token_show(&token_list, logger, eqtb);
    logger.print_ln();
    logger.terminal_logging = old_setting;
}

/// See 1371.
fn expand_macros_in_token_list(
    write_node: &WriteNode,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> Vec<Token> {
    let token_list = vec![Token::RIGHT_BRACE_TOKEN, END_WRITE_TOKEN];
    scanner.ins_list(token_list, eqtb, logger);
    let source_type = TokenSourceType::WriteText;
    scanner
        .input_stack
        .begin_token_list(write_node.tokens.clone(), source_type, eqtb, logger);
    let token_list = vec![Token::LEFT_BRACE_TOKEN];
    scanner.ins_list(token_list, eqtb, logger);
    scanner.scanning_write_tokens = true;
    let cs = eqtb.write_cs;
    let token_list = scanner.scan_toks(cs, true, eqtb, logger);
    let token = scanner.get_token(eqtb, logger);
    if token != END_WRITE_TOKEN {
        recover_from_unbalanced_write_command(scanner, eqtb, logger);
    }
    scanner.scanning_write_tokens = false;
    // Remove the finished token list inserted above to conserve stack space
    // in case of a sequence of \write commands.
    scanner.end_token_list(eqtb, logger);
    token_list
}

/// See 1372.
fn recover_from_unbalanced_write_command(
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    logger.print_err("Unbalanced write command");
    let help = &[
        "On this page there's a \\write with fewer real {'s than }'s.",
        "I can't handle that very well; good luck.",
    ];
    logger.error(help, scanner, eqtb);
    loop {
        let token = scanner.get_token(eqtb, logger);
        if token == END_WRITE_TOKEN {
            break;
        }
    }
}

/// See 1374.
pub fn open_write_file(
    node: &OpenNode,
    output: &mut Output,
    scanner: &Scanner,
    eqtb: &Eqtb,
    logger: &mut Logger,
) {
    let stream = node.write_stream;
    if stream < 16 {
        let mut path = node.path.clone();
        if path.extension().is_none() {
            path.set_extension("tex");
        }
        loop {
            match open_out(&path) {
                Ok(file) => {
                    output.write_files[stream] = Some(BufWriter::new(file));
                    break;
                }
                Err(_) => {
                    path = logger.prompt_file_name(
                        &path,
                        "output file name",
                        "tex",
                        &scanner.input_stack,
                        eqtb,
                    );
                }
            }
        }
    }
}

use crate::command::UnexpandableCommand;
use crate::eqtb::{Eqtb, IntegerVariable, MAX_LATIN_UCS_CODE};
use crate::format::{Dumpable, FormatError};
use crate::input::expansion::get_x_token;
use crate::input::Scanner;
use crate::logger::Logger;
use crate::print::Printer;
use crate::token::Token;

use std::collections::{HashMap, VecDeque};
use std::io::Write;

/// The 8-bit hyphenation-code snapshot defined by e-TeX manual section 3.10.
/// The dense byte table keeps the active snapshot lookup out of an allocation
/// or binary search while preserving the TeX82 byte domain.
struct EtexHyphenationCodes([u8; 256]);

/// PraTeX's Latin-UCS extension to e-TeX's byte-domain snapshot.
///
/// This is deliberately a separate type: it must not silently widen the
/// externally observable 8-bit e-TeX contract. Entries live here when either
/// the input code point or its hyphenation code does not fit in one byte.
struct LatinUcsHyphenationCodes(Vec<(u16, u16)>);

struct SavedHyphenationCodes {
    etex: EtexHyphenationCodes,
    latin_ucs: LatinUcsHyphenationCodes,
}

impl SavedHyphenationCodes {
    fn capture(eqtb: &Eqtb) -> Self {
        let mut etex = [0; 256];
        let mut latin_ucs = Vec::new();
        for character in 0..=MAX_LATIN_UCS_CODE as usize {
            let code = eqtb.lc_code(character);
            if character <= u8::MAX as usize && (1..=u8::MAX as i32).contains(&code) {
                etex[character] = code as u8;
            } else if (1..=MAX_LATIN_UCS_CODE as i32).contains(&code) {
                latin_ucs.push((character as u16, code as u16));
            }
        }
        Self {
            etex: EtexHyphenationCodes(etex),
            latin_ucs: LatinUcsHyphenationCodes(latin_ucs),
        }
    }

    fn get(&self, character: u32) -> i32 {
        let Ok(character) = u16::try_from(character) else {
            return 0;
        };
        if let Ok(etex_character) = u8::try_from(character) {
            let code = self.etex.0[usize::from(etex_character)];
            if code != 0 {
                return i32::from(code);
            }
        }
        if let Ok(index) = self
            .latin_ucs
            .0
            .binary_search_by_key(&character, |&(key, _)| key)
        {
            return i32::from(self.latin_ucs.0[index].1);
        }
        0
    }

    fn is_valid(&self) -> bool {
        let latin_ucs_is_valid = self.latin_ucs.0.iter().all(|&(key, value)| {
            u32::from(key) <= MAX_LATIN_UCS_CODE
                && (1..=MAX_LATIN_UCS_CODE as u16).contains(&value)
                && (key > u8::MAX as u16 || value > u8::MAX as u16)
                && !(key <= u8::MAX as u16 && self.etex.0[usize::from(key)] != 0)
        }) && keys_are_strictly_increasing(&self.latin_ucs.0);
        latin_ucs_is_valid
    }
}

fn keys_are_strictly_increasing<K: Ord + Copy, V>(entries: &[(K, V)]) -> bool {
    entries.windows(2).all(|pair| pair[0].0 < pair[1].0)
}

/// Stores all the hyphenation related information.
pub struct Hyphenator {
    /// Stores for each language the hyphenation exceptions.
    /// The information is given as a list of the hyphen positions.
    /// See 926.
    pub exceptions: [HashMap<Vec<u16>, Vec<usize>>; 256],

    pre_trie: PreTrie,

    /// See 950.
    pub trie: Option<Trie>,

    /// Language-local snapshots created by positive `\savinghyphcodes`.
    /// They become active only after the pattern trie has been compressed.
    saved_hyphenation_codes: [Option<Box<SavedHyphenationCodes>>; 256],
    /// Keeps the overwhelmingly common no-snapshot word scan to one
    /// predictable branch before the ordinary `\lccode` lookup.
    has_saved_hyphenation_codes: bool,

    pub cur_lang: usize,
    /// Minimum number of characters before the first hyphen when hyphenating.
    pub l_hyf: usize,
    /// Minimum number of characters after the last hyphen when hyphenating.
    pub r_hyf: usize,

    /// The values of cur_lang, l_hyf, and r_hyf at the start of a paragraph.
    pub init_cur_lang: usize,
    pub init_l_hyf: usize,
    pub init_r_hyf: usize,
}

impl Hyphenator {
    pub fn new() -> Self {
        Self {
            exceptions: std::array::from_fn(|_| HashMap::new()),
            pre_trie: PreTrie::new(),
            trie: None,
            saved_hyphenation_codes: std::array::from_fn(|_| None),
            has_saved_hyphenation_codes: false,
            cur_lang: 0,
            l_hyf: 0,
            r_hyf: 0,
            init_cur_lang: 0,
            init_l_hyf: 0,
            init_r_hyf: 0,
        }
    }

    /// See 934.
    pub fn set_cur_lang(&mut self, eqtb: &Eqtb) {
        let language = eqtb.integer(IntegerVariable::Language);
        self.cur_lang = if language <= 0 || language > 255 {
            0
        } else {
            language as usize
        };
    }

    pub(super) fn hyphenation_code(&self, character: u32, eqtb: &Eqtb) -> i32 {
        if self.has_saved_hyphenation_codes && self.trie.is_some() {
            if let Some(codes) = &self.saved_hyphenation_codes[self.cur_lang] {
                return codes.get(character);
            }
        }
        usize::try_from(character)
            .ok()
            .filter(|&character| character <= MAX_LATIN_UCS_CODE as usize)
            .map_or(0, |character| eqtb.lc_code(character))
    }

    /// Returns false for Return, else true.
    /// See 923.
    pub fn find_hyphen_locations(&self, word: &[u16], pattern: &mut Vec<u8>) -> bool {
        if word.len() < 2 {
            pattern.clear();
            return false;
        }
        pattern.clear();
        pattern.resize(word.len() - 1, 0);
        // First look for the pattern in the exception table.
        if !self.find_pattern_in_exceptions(&word[1..word.len() - 1], pattern) {
            // Alternatively look for the pattern in the trie.
            let Some(trie) = self.trie.as_ref() else {
                return false;
            };
            if !trie.determine_hyph_pattern(word, self.cur_lang, pattern) {
                // Pattern was not found in the trie either.
                return false;
            }
        }

        // found:
        for j in 0..self.l_hyf {
            if let Some(value) = pattern.get_mut(j) {
                *value = 0;
            }
        }
        for j in 0..self.r_hyf {
            let Some(index) = word.len().checked_sub(2 + j) else {
                break;
            };
            if let Some(value) = pattern.get_mut(index) {
                *value = 0;
            }
        }
        true
    }

    /// If the word is in the exception table, store the corresponding pattern in `pattern` and
    /// return true, else return false.
    /// See 930., 931. and 932.
    fn find_pattern_in_exceptions(&self, word: &[u16], pattern: &mut Vec<u8>) -> bool {
        let Some(exceptions) = self.exceptions.get(self.cur_lang) else {
            return false;
        };
        if let Some(hyphen_positions) = exceptions.get(word) {
            for &pos in hyphen_positions {
                if let Some(value) = pattern.get_mut(pos) {
                    *value = 1;
                } else {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    /// See 939., 940. and 941.
    fn enter_hyphenation_exception(&mut self, word: Vec<u16>, hyphen_positions: Vec<usize>) {
        self.exceptions[self.cur_lang].insert(word, hyphen_positions);
    }

    fn format_exceptions_are_valid(&self) -> bool {
        self.exceptions.iter().all(|map| {
            map.iter().all(|(word, positions)| {
                (2..=63).contains(&word.len())
                    && word
                        .iter()
                        .all(|&code| (1..=MAX_LATIN_UCS_CODE as u16).contains(&code))
                    && positions.iter().all(|&position| position <= word.len())
            })
        })
    }
    fn saved_hyphenation_codes_are_valid(&self) -> bool {
        self.saved_hyphenation_codes
            .iter()
            .flatten()
            .all(|codes| codes.is_valid())
    }
}

/// See 920. and 921.
pub struct Trie {
    pub nodes: Vec<TrieNode>,
    hyf_ops: [Vec<HyfOp>; 256],
    op_code_hash: [HashMap<HyfOp, usize>; 256],
}

/// See 920. and 921.
#[derive(Clone, Copy)]
pub struct TrieNode {
    link: Option<usize>,
    chr: Option<u16>,
    op: Option<usize>,
}

impl Trie {
    fn format_state_is_valid(&self) -> bool {
        if self.nodes.len() < 257
            || self.nodes[0].chr.is_some()
            || self.nodes[0].link.is_some()
            || self.nodes[0].op.is_some()
            || !hyf_operations_are_valid(&self.hyf_ops, &self.op_code_hash)
        {
            return false;
        }

        let mut families = vec![Vec::new(); self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            if node.chr.is_some_and(|c| u32::from(c) > MAX_LATIN_UCS_CODE)
                || node.link.is_some_and(|link| link >= self.nodes.len())
                || (node.chr.is_none() && (node.link.is_some() || node.op.is_some()))
            {
                return false;
            }
            if let Some(chr) = node.chr {
                let chr = usize::from(chr);
                if index <= chr {
                    return false;
                }
                families[index - chr].push(index);
            }
        }
        for node in &self.nodes {
            if node
                .link
                .is_some_and(|base| base == 0 || families[base].is_empty())
            {
                return false;
            }
        }

        for lang in 0..256 {
            match self.nodes[lang + 1].chr {
                None => {}
                Some(c) => {
                    if usize::from(c) != lang
                        || !self.language_patterns_are_valid(lang + 1, lang, &families)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn language_patterns_are_valid(
        &self,
        language_node: usize,
        lang: usize,
        families: &[Vec<usize>],
    ) -> bool {
        let mut maximum_depth_seen = HashMap::new();
        let mut stack = vec![(language_node, 0usize)];
        while let Some((index, depth)) = stack.pop() {
            if depth > 63
                || self.nodes[index]
                    .op
                    .is_some_and(|op| op >= self.hyf_ops[lang].len())
            {
                return false;
            }
            if maximum_depth_seen
                .get(&index)
                .is_some_and(|&previous| previous >= depth)
            {
                continue;
            }
            maximum_depth_seen.insert(index, depth);
            let Some(base) = self.nodes[index].link else {
                continue;
            };
            let Some(child_depth) = depth.checked_add(1) else {
                return false;
            };
            for &child in &families[base] {
                stack.push((child, child_depth));
            }
        }
        true
    }

    /// See 923.
    fn determine_hyph_pattern(&self, word: &[u16], lang: usize, pattern: &mut Vec<u8>) -> bool {
        if lang >= 256 {
            return false;
        }
        let Some(language_node) = self.nodes.get(lang + 1) else {
            return false;
        };
        match language_node.chr {
            None => return false,
            Some(chr) => {
                if chr as usize != lang {
                    return false;
                }
            }
        }

        for j in 0..word.len() {
            let Some(mut base) = language_node.link else {
                continue;
            };
            for l in j..word.len() {
                // Unicode対応後は、各familyを「実在する最大の兄弟」までしか
                // 確保しない。未登録の高位文字を引いたときは単に不一致であり、
                // 圧縮表の外を添字にしてはならない。
                let Some(index) = base.checked_add(usize::from(word[l])) else {
                    break;
                };
                let Some(node) = self.nodes.get(index).copied() else {
                    break;
                };

                if node.chr != Some(word[l]) {
                    break;
                }

                if let Some(op) = node.op {
                    self.store_maximum_values_in_pattern(op, l, pattern, lang);
                }
                base = match node.link {
                    Some(base) => base,
                    None => break,
                }
            }
        }
        true
    }

    /// See 924.
    fn store_maximum_values_in_pattern(
        &self,
        op: usize,
        l: usize,
        pattern: &mut Vec<u8>,
        lang: usize,
    ) {
        let mut v = op;
        // A valid op chain always points to an earlier entry. Keep a step
        // bound as defense in depth for a format that escaped validation.
        for _ in 0..self.hyf_ops[lang].len() {
            let Some(&hyf_op) = self.hyf_ops[lang].get(v) else {
                break;
            };
            let Some(i) = l.checked_sub(hyf_op.distance) else {
                break;
            };
            let Some(value) = pattern.get_mut(i) else {
                break;
            };
            if hyf_op.num > *value {
                *value = hyf_op.num;
            }
            v = match hyf_op.next {
                Some(index) => index,
                None => break,
            }
        }
    }
}

/// See 920. and 921.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct HyfOp {
    distance: usize,
    num: u8,
    next: Option<usize>,
}

impl Hyphenator {
    /// See 934.
    pub fn new_hyph_exceptions(
        &mut self,
        scanner: &mut Scanner,
        eqtb: &mut Eqtb,
        logger: &mut Logger,
    ) {
        scanner.scan_left_brace(eqtb, logger);
        self.set_cur_lang(eqtb);
        self.enter_as_many_hyphenations_exception_as_are_listed(scanner, eqtb, logger);
    }

    /// See 935.
    fn enter_as_many_hyphenations_exception_as_are_listed(
        &mut self,
        scanner: &mut Scanner,
        eqtb: &mut Eqtb,
        logger: &mut Logger,
    ) {
        let mut word = Vec::new();
        let mut hyphen_positions = Vec::new();
        loop {
            let (unexpandable_command, _) = get_x_token(scanner, eqtb, logger);
            match unexpandable_command {
                UnexpandableCommand::Letter(c)
                | UnexpandableCommand::Other(c)
                | UnexpandableCommand::CharGiven(c) => {
                    let character = u32::from(c);
                    append_new_letter_or_hyphen(
                        character,
                        self.hyphenation_code(character, eqtb),
                        &mut word,
                        &mut hyphen_positions,
                        scanner,
                        eqtb,
                        logger,
                    )
                }
                UnexpandableCommand::CharNum => {
                    let c = scanner.scan_char_num(eqtb, logger);
                    let character = u32::from(c);
                    append_new_letter_or_hyphen(
                        character,
                        self.hyphenation_code(character, eqtb),
                        &mut word,
                        &mut hyphen_positions,
                        scanner,
                        eqtb,
                        logger,
                    );
                }
                UnexpandableCommand::LatinUcsChar(token) => {
                    let character = token.code_point();
                    append_new_letter_or_hyphen(
                        character,
                        self.hyphenation_code(character, eqtb),
                        &mut word,
                        &mut hyphen_positions,
                        scanner,
                        eqtb,
                        logger,
                    )
                }
                UnexpandableCommand::RightBrace(_)
                | UnexpandableCommand::LatinUcsRightBrace(_)
                | UnexpandableCommand::Spacer => {
                    if word.len() > 1 {
                        self.enter_hyphenation_exception(word, hyphen_positions);
                    }
                    if matches!(
                        unexpandable_command,
                        UnexpandableCommand::RightBrace(_)
                            | UnexpandableCommand::LatinUcsRightBrace(_)
                    ) {
                        return;
                    }
                    word = Vec::new();
                    hyphen_positions = Vec::new();
                }
                _ => give_improper_hyphenation_error(scanner, eqtb, logger),
            }
        }
    }
}

/// See 936.
fn give_improper_hyphenation_error(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    logger.print_err("Improper ");
    logger.print_esc_str(b"hyphenation");
    logger.print_str(" will be flushed");
    let help = &[
        "Hyphenation exceptions must contain only letters",
        "and hyphens. But continue; I'll forgive and forget.",
    ];
    logger.error(help, scanner, eqtb)
}

/// See 937. and 938.
fn append_new_letter_or_hyphen(
    chr: u32,
    hyphenation_code: i32,
    word: &mut Vec<u16>,
    hyphen_positions: &mut Vec<usize>,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    if chr == u32::from(b'-') {
        if word.len() < 63 {
            hyphen_positions.push(word.len());
        }
    } else if !is_valid_hyphenation_code(hyphenation_code) {
        logger.print_err("Not a letter");
        let help = &[
            "Letters in \\hyphenation words must have a usable \\lccode.",
            "Proceed; I'll ignore the character I just read.",
        ];
        logger.error(help, scanner, eqtb);
    } else if word.len() < 63 {
        word.push(hyphenation_code as u16);
    }
}

fn hyf_operations_are_valid(
    hyf_ops: &[Vec<HyfOp>; 256],
    op_code_hash: &[HashMap<HyfOp, usize>; 256],
) -> bool {
    for lang in 0..256 {
        let ops = &hyf_ops[lang];
        let hash = &op_code_hash[lang];
        if hash.len() != ops.len() {
            return false;
        }
        for (index, op) in ops.iter().enumerate() {
            if !(1..=9).contains(&op.num)
                || op.distance > 63
                || op.next.is_some_and(|next| next >= index)
                || hash.get(op) != Some(&index)
            {
                return false;
            }
        }
    }
    true
}

fn is_valid_hyphenation_code(code: i32) -> bool {
    (1..=MAX_LATIN_UCS_CODE as i32).contains(&code)
}

/// See 947.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
struct PreTrieNode {
    c: u16,
    op: Option<usize>,
    first_child: Option<usize>,
    next_sibling: Option<usize>,
}

/// A Trie that can accept new entries.
/// Keys are sequences of bytes, values are `usize`.
/// The very first node corresponds to the root (corresponding to an empty key).
/// See 947.
#[derive(Debug)]
struct PreTrie {
    nodes: Vec<PreTrieNode>,
    subtrie_hash: HashMap<PreTrieNode, usize>,
    hyf_ops: [Vec<HyfOp>; 256],
    op_code_hash: [HashMap<HyfOp, usize>; 256],
}

impl PreTrie {
    fn new() -> Self {
        Self {
            // We insert the root node corresponding to an empty key.
            nodes: vec![PreTrieNode {
                c: 0,
                op: None,
                first_child: None,
                next_sibling: None,
            }],
            subtrie_hash: HashMap::new(),
            hyf_ops: std::array::from_fn(|_| Vec::new()),
            op_code_hash: std::array::from_fn(|_| HashMap::new()),
        }
    }

    fn format_state_is_valid(&self) -> bool {
        let Some(root) = self.nodes.first() else {
            return false;
        };
        if root.c != 0
            || root.op.is_some()
            || root.next_sibling.is_some()
            || !hyf_operations_are_valid(&self.hyf_ops, &self.op_code_hash)
        {
            return false;
        }

        for node in &self.nodes {
            if u32::from(node.c) > MAX_LATIN_UCS_CODE
                || node
                    .first_child
                    .is_some_and(|index| index >= self.nodes.len())
                || node
                    .next_sibling
                    .is_some_and(|index| index >= self.nodes.len())
            {
                return false;
            }
        }

        let Some(reachable) = self.reachable_nodes_without_cycles() else {
            return false;
        };
        if !self.sibling_families_are_strictly_increasing(&reachable) {
            return false;
        }

        let mut language_node = root.first_child;
        while let Some(index) = language_node {
            let lang = usize::from(self.nodes[index].c);
            if lang >= 256 || !self.language_subtrie_is_valid(index, lang) {
                return false;
            }
            language_node = self.nodes[index].next_sibling;
        }
        true
    }

    /// Validate the reachable child/sibling DAG without using the process
    /// stack. Shared reduced subtries are valid; directed cycles are not.
    fn reachable_nodes_without_cycles(&self) -> Option<Vec<bool>> {
        let mut reachable = vec![false; self.nodes.len()];
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            if reachable[index] {
                continue;
            }
            reachable[index] = true;
            if let Some(child) = self.nodes[index].first_child {
                stack.push(child);
            }
            if let Some(sibling) = self.nodes[index].next_sibling {
                stack.push(sibling);
            }
        }

        let mut indegree = vec![0usize; self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            if !reachable[index] {
                continue;
            }
            for target in [node.first_child, node.next_sibling].into_iter().flatten() {
                indegree[target] = indegree[target].checked_add(1)?;
            }
        }
        let mut queue = VecDeque::new();
        for (index, &is_reachable) in reachable.iter().enumerate() {
            if is_reachable && indegree[index] == 0 {
                queue.push_back(index);
            }
        }
        let mut visited = 0usize;
        while let Some(index) = queue.pop_front() {
            visited += 1;
            let node = self.nodes[index];
            for target in [node.first_child, node.next_sibling].into_iter().flatten() {
                indegree[target] -= 1;
                if indegree[target] == 0 {
                    queue.push_back(target);
                }
            }
        }
        (visited == reachable.iter().filter(|&&value| value).count()).then_some(reachable)
    }

    fn sibling_families_are_strictly_increasing(&self, reachable: &[bool]) -> bool {
        for (index, node) in self.nodes.iter().enumerate() {
            if !reachable[index] {
                continue;
            }
            let mut child = node.first_child;
            let mut previous = None;
            while let Some(child_index) = child {
                let c = self.nodes[child_index].c;
                if previous.is_some_and(|previous| previous >= c) {
                    return false;
                }
                previous = Some(c);
                child = self.nodes[child_index].next_sibling;
            }
        }
        true
    }

    fn language_subtrie_is_valid(&self, language_node: usize, lang: usize) -> bool {
        if self.nodes[language_node]
            .op
            .is_some_and(|op| op >= self.hyf_ops[lang].len())
        {
            return false;
        }

        let mut maximum_depth_seen = vec![0usize; self.nodes.len()];
        let mut stack = Vec::new();
        if let Some(child) = self.nodes[language_node].first_child {
            stack.push((child, 1usize));
        }
        while let Some((index, depth)) = stack.pop() {
            if depth > 63 {
                return false;
            }
            if self.nodes[index]
                .op
                .is_some_and(|op| op >= self.hyf_ops[lang].len())
            {
                return false;
            }
            if maximum_depth_seen[index] >= depth {
                continue;
            }
            maximum_depth_seen[index] = depth;
            if let Some(child) = self.nodes[index].first_child {
                let Some(child_depth) = depth.checked_add(1) else {
                    return false;
                };
                stack.push((child, child_depth));
            }
            if let Some(sibling) = self.nodes[index].next_sibling {
                stack.push((sibling, depth));
            }
        }
        true
    }

    /// See 963.
    fn insert(&mut self, word: &[u16], op: Option<usize>) -> Result<(), ()> {
        // We start with the parent being the root.
        let mut parent = 0;
        for &c in word {
            let mut prev_child = None;
            let mut child = self.nodes[parent].first_child;
            while let Some(index) = child {
                if self.nodes[index].c >= c {
                    break;
                }
                prev_child = Some(index);
                child = self.nodes[index].next_sibling;
            }
            let pos = match child {
                None => self.push_new_trie_node(c, None),
                Some(index) => {
                    if self.nodes[index].c > c {
                        self.push_new_trie_node(c, Some(index))
                    } else {
                        index
                    }
                }
            };
            let prev_link = match prev_child {
                None => &mut self.nodes[parent].first_child,
                Some(prev_index) => &mut self.nodes[prev_index].next_sibling,
            };
            *prev_link = Some(pos);
            parent = pos;
        }
        if self.nodes[parent].op.is_some() {
            self.nodes[parent].op = op;
            Err(())
        } else {
            self.nodes[parent].op = op;
            Ok(())
        }
    }

    /// See 963.
    fn insert_new_pattern_into_linked_trie(
        &mut self,
        mut word: Vec<u16>,
        pattern: Vec<u8>,
        lang: usize,
        scanner: &mut Scanner,
        eqtb: &mut Eqtb,
        logger: &mut Logger,
    ) {
        let v = self.compute_trie_op_code(&word, pattern, lang);
        // Use the language as the letter at index zero.
        word.insert(0, lang as u16);
        match self.insert(&word, v) {
            Ok(()) => {}
            Err(()) => {
                logger.print_err("Duplicate pattern");
                let help = &["(See Appendix H.)"];
                logger.error(help, scanner, eqtb);
            }
        }
    }

    /// Creates a new TrieNode and sets the given next sibling. Returns the
    /// corresponding index in the node list.
    /// See 964.
    fn push_new_trie_node(&mut self, c: u16, next_sibling: Option<usize>) -> usize {
        self.nodes.push(PreTrieNode {
            c,
            op: None,
            first_child: None,
            next_sibling,
        });
        self.nodes.len() - 1
    }

    /// See 965.
    fn compute_trie_op_code(
        &mut self,
        word: &[u16],
        mut pattern: Vec<u8>,
        lang: usize,
    ) -> Option<usize> {
        // We don't allow hyphenation before or after the virtual beginning-of-word and
        // ending-of-word marks.
        if let Some(0) = word.first() {
            pattern[0] = 0;
        }
        if let Some(0) = word.last() {
            pattern[word.len()] = 0;
        }
        let mut l = word.len();
        let mut v = None;
        loop {
            if pattern[l] != 0 {
                let hyf_op = HyfOp {
                    distance: word.len() - l,
                    num: pattern[l],
                    next: v,
                };
                v = Some(self.new_trie_op(hyf_op, lang));
            }
            if l > 0 {
                l -= 1;
            } else {
                return v;
            }
        }
    }

    /// See 944.
    fn new_trie_op(&mut self, hyf_op: HyfOp, lang: usize) -> usize {
        match self.op_code_hash[lang].get(&hyf_op) {
            None => {
                let u = self.hyf_ops[lang].len();
                // Add the opcode.
                self.hyf_ops[lang].push(hyf_op);
                // Associate this index with the hash entry.
                self.op_code_hash[lang].insert(hyf_op, u);
                u
            }
            Some(&u) => u,
        }
    }

    /// Removes reduncancy by using only one representative of each class of
    /// equivalent subtries.
    /// See 952.
    fn reduce(&mut self) {
        self.subtrie_hash = HashMap::new();

        // Deduplicate the node trie.
        self.nodes[0].first_child = self.reduce_subtrie(self.nodes[0].first_child);
    }

    /// See 949.
    fn reduce_subtrie(&mut self, subtrie_root: Option<usize>) -> Option<usize> {
        // A Unicode family can legally have one sibling for every latin_ucs
        // code point. Recursing through that sibling list would make valid
        // input consume O(U+2E7F) call frames. Collect one level iteratively;
        // recursion is then only over pattern depth (at most 64 including the
        // language node).
        let mut siblings = Vec::new();
        let mut current = subtrie_root;
        while let Some(index) = current {
            siblings.push(index);
            current = self.nodes[index].next_sibling;
        }

        let mut reduced_next = None;
        for index in siblings.into_iter().rev() {
            let first_child = self.nodes[index].first_child;
            self.nodes[index].first_child = self.reduce_subtrie(first_child);
            self.nodes[index].next_sibling = reduced_next;
            reduced_next = Some(self.subtrie_representative(index));
        }
        reduced_next
    }

    /// Return the unique representative of equal subtries.
    /// See 948.
    fn subtrie_representative(&mut self, node_index: usize) -> usize {
        *self
            .subtrie_hash
            .entry(self.nodes[node_index])
            .or_insert(node_index)
    }

    /// See 947.
    fn trie_root(&self) -> Option<usize> {
        self.nodes[0].first_child
    }

    /// See 966.
    pub fn to_trie_mut(&mut self) -> Trie {
        // Remove redundancy in trie.
        self.reduce();

        // This stores for each smallest child, the corresponding base index in the compressed trie.
        let mut trie_ref = HashMap::new();
        // Is used to determine the layout of how to fit the trie into the compressed array.
        // The first node remains always available.
        let mut allocations = vec![AllocationCell {
            is_base: false,
            is_taken: false,
            prev: 0,
            next: 1,
        }];
        if let Some(index) = self.trie_root() {
            self.first_fit(index, &mut allocations, &mut trie_ref);
            debug_assert_eq!(trie_ref.get(&index), Some(&1));
            Self::reserve_unused_language_slots(&mut allocations);
            self.trie_pack(index, &mut allocations, &mut trie_ref);
        }
        self.move_data_into_trie(allocations.len(), trie_ref)
    }

    /// See 953.
    fn first_fit(
        &self,
        p: usize,
        allocations: &mut Vec<AllocationCell>,
        trie_ref: &mut HashMap<usize, usize>,
    ) {
        let c = self.nodes[p].c;
        // Unicode欧文の符号位置までfree-listを歩く前に、添字そのものを確保する。
        Self::ensure_allocation_extent(0, c as usize + 2, allocations);
        let mut z = 0;
        // Ensure that the base will be at index 1 or higher.
        while z <= c as usize {
            z = allocations[z].next;
        }
        let mut h;
        let family_extent = self.maximum_sibling_code(p) + 2;
        loop {
            h = z - c as usize;
            Self::ensure_allocation_extent(h, family_extent, allocations);
            if allocations[h].is_base {
                // not_found:
                z = allocations[z].next;
                continue;
            }
            if self.all_characters_of_familiy_fit(p, h, allocations) {
                break;
            }

            // not_found:
            z = allocations[z].next;
        }
        // found:
        self.pack_family_into_trie(h, p, allocations, trie_ref);
    }

    /// See 954.
    fn ensure_allocation_extent(
        h: usize,
        required_extent: usize,
        allocations: &mut Vec<AllocationCell>,
    ) {
        if allocations.len() - h < required_extent {
            loop {
                let pos = allocations.len();
                allocations.push(AllocationCell {
                    is_base: false,
                    is_taken: false,
                    next: pos + 1,
                    prev: pos - 1,
                });
                if allocations.len() - h == required_extent {
                    break;
                }
            }
        }
    }

    /// Slots 1 through 256 are addressed directly by `language + 1`.
    /// After fitting the root language family at base 1, keep its unused
    /// members out of the free list so descendant families cannot masquerade
    /// as another language root.
    fn reserve_unused_language_slots(allocations: &mut Vec<AllocationCell>) {
        Self::ensure_allocation_extent(0, 256 + 2, allocations);
        for index in 1..=256 {
            if allocations[index].is_taken {
                continue;
            }
            let previous = allocations[index].prev;
            let next = allocations[index].next;
            allocations[previous].next = next;
            allocations[next].prev = previous;
            allocations[index].is_taken = true;
        }
    }

    fn maximum_sibling_code(&self, mut p: usize) -> usize {
        let mut maximum = 0;
        loop {
            maximum = maximum.max(self.nodes[p].c as usize);
            match self.nodes[p].next_sibling {
                Some(next) => p = next,
                None => return maximum,
            }
        }
    }

    /// Returns false for goto not_found and true for goto found.
    /// See 955.
    fn all_characters_of_familiy_fit(
        &self,
        p: usize,
        h: usize,
        allocations: &[AllocationCell],
    ) -> bool {
        let mut q = self.nodes[p].next_sibling;
        while let Some(index) = q {
            // If this spot is already in use, we need to find another hole.
            if allocations[h + self.nodes[index].c as usize].is_taken {
                return false;
            }
            q = self.nodes[index].next_sibling;
        }
        true
    }

    /// See 956.
    fn pack_family_into_trie(
        &self,
        h: usize,
        p: usize,
        allocations: &mut Vec<AllocationCell>,
        trie_ref: &mut HashMap<usize, usize>,
    ) {
        allocations[h].is_base = true;
        trie_ref.insert(p, h);
        let mut q = p;
        loop {
            let z = h + self.nodes[q].c as usize;

            // Reconnect the double linked list of unused spots.
            let l = allocations[z].prev;
            let r = allocations[z].next;
            allocations[r].prev = l;
            allocations[l].next = r;

            // Mark this spot as used.
            allocations[z].is_taken = true;

            match self.nodes[q].next_sibling {
                None => break,
                Some(index) => {
                    q = index;
                }
            }
        }
    }

    /// Recursively store the trie with root node p in the table trie.
    /// See 957.
    fn trie_pack(
        &self,
        mut p: usize,
        allocations: &mut Vec<AllocationCell>,
        trie_ref: &mut HashMap<usize, usize>,
    ) {
        loop {
            // Get first child of p.
            let q = self.nodes[p].first_child;
            // If this trie exists and if it has not yet been added, find
            // a fit and store it.
            if let Some(index) = q {
                if !trie_ref.contains_key(&index) {
                    self.first_fit(index, allocations, trie_ref);
                    self.trie_pack(index, allocations, trie_ref);
                }
            }
            // Move on to next sibling node.
            match self.nodes[p].next_sibling {
                None => break,
                Some(index) => {
                    p = index;
                }
            }
        }
    }

    /// See 958.
    fn move_data_into_trie(&self, len: usize, trie_ref: HashMap<usize, usize>) -> Trie {
        // We use this to "zero out" the unused spots.
        let unused_node = TrieNode {
            link: None,
            chr: None,
            op: None,
        };
        // If the trie is empty, zero out the first 257 entries.
        let mut nodes = match self.trie_root() {
            None => {
                vec![unused_node; 256 + 1]
            }
            Some(index) => {
                // The first 256 occupied positions are the fixed language
                // slots even when the only loaded patterns use low languages
                // and a small alphabet.
                let mut nodes = vec![unused_node; len.max(256 + 1)];
                // Write the actual trie data into the table trie.
                self.trie_fix(index, &mut nodes, &trie_ref);
                nodes
            }
        };
        nodes[0].chr = None;
        Trie {
            nodes,
            hyf_ops: self.hyf_ops.clone(),
            op_code_hash: self.op_code_hash.clone(),
        }
    }

    /// Write the data from the node trie to the table trie.
    /// See 959.
    fn trie_fix(&self, mut p: usize, nodes: &mut Vec<TrieNode>, trie_ref: &HashMap<usize, usize>) {
        // Get the reference index for p and its siblings.
        let z = trie_ref[&p];
        // Iterate through the siblings.
        loop {
            // The first child of p.
            let q = self.nodes[p].first_child;
            // The character of p.
            let c = self.nodes[p].c;
            // Set the reference index for p's child nodes.
            nodes[z + c as usize].link = q.map(|index| trie_ref[&index]);
            // Store the character and opcode.
            nodes[z + c as usize].chr = Some(c);
            nodes[z + c as usize].op = self.nodes[p].op;
            // If p has child nodes, recursively add their data.
            if let Some(index) = q {
                self.trie_fix(index, nodes, trie_ref);
            }
            // Move on to the next sibling.
            match self.nodes[p].next_sibling {
                None => break,
                Some(index) => {
                    p = index;
                }
            }
        }
    }
}

/// See 950.
struct AllocationCell {
    is_base: bool,
    is_taken: bool,
    next: usize,
    prev: usize,
}

impl Hyphenator {
    /// See 960.
    pub fn new_patterns(
        &mut self,
        token: Token,
        scanner: &mut Scanner,
        eqtb: &mut Eqtb,
        logger: &mut Logger,
    ) {
        if self.trie.is_none() {
            self.set_cur_lang(eqtb);
            if eqtb.integer(IntegerVariable::SavingHyphCodes) > 0 {
                // e-TeX manual 3.10: every positive \patterns execution
                // replaces the current language's snapshot. A later
                // non-positive execution leaves an earlier snapshot intact.
                self.saved_hyphenation_codes[self.cur_lang] =
                    Some(Box::new(SavedHyphenationCodes::capture(eqtb)));
                self.has_saved_hyphenation_codes = true;
            }
            scanner.scan_left_brace(eqtb, logger);
            self.enter_all_patterns_into_linked_trie(scanner, eqtb, logger)
        } else {
            logger.print_err("Too late for ");
            logger.print_esc_str(b"patterns");
            let help = &["All patterns must be given before typesetting begins."];
            logger.error(help, scanner, eqtb);
            // Scan and ignore.
            let Token::CSToken { cs } = token else {
                panic!("Impossible")
            };
            scanner.scan_toks(cs, false, eqtb, logger);
        }
    }

    /// See 961.
    fn enter_all_patterns_into_linked_trie(
        &mut self,
        scanner: &mut Scanner,
        eqtb: &mut Eqtb,
        logger: &mut Logger,
    ) {
        let mut word = Vec::new();
        let mut pattern = vec![0];
        let mut digit_sensed = false;
        loop {
            let (unexpandable_command, _) = get_x_token(scanner, eqtb, logger);
            match unexpandable_command {
                UnexpandableCommand::Letter(c) | UnexpandableCommand::Other(c) => {
                    append_new_letter_or_hyphen_level(
                        u32::from(c),
                        &mut digit_sensed,
                        &mut word,
                        &mut pattern,
                        scanner,
                        eqtb,
                        logger,
                    )
                }
                UnexpandableCommand::LatinUcsChar(token) => append_new_letter_or_hyphen_level(
                    token.code_point(),
                    &mut digit_sensed,
                    &mut word,
                    &mut pattern,
                    scanner,
                    eqtb,
                    logger,
                ),
                UnexpandableCommand::RightBrace(_)
                | UnexpandableCommand::LatinUcsRightBrace(_)
                | UnexpandableCommand::Spacer => {
                    // If there is at least one character in current pattern
                    if !word.is_empty() {
                        self.pre_trie.insert_new_pattern_into_linked_trie(
                            word,
                            pattern,
                            self.cur_lang,
                            scanner,
                            eqtb,
                            logger,
                        );
                    }
                    if matches!(
                        unexpandable_command,
                        UnexpandableCommand::RightBrace(_)
                            | UnexpandableCommand::LatinUcsRightBrace(_)
                    ) {
                        return;
                    }
                    // Reset for next pattern.
                    word = Vec::new();
                    pattern = vec![0];
                    digit_sensed = false;
                }
                _ => {
                    logger.print_err("Bad ");
                    logger.print_esc_str(b"patterns");
                    let help = &["(See Appendix H.)"];
                    logger.error(help, scanner, eqtb);
                }
            }
        }
    }
}

/// See 962.
fn append_new_letter_or_hyphen_level(
    mut chr: u32,
    digit_sensed: &mut bool,
    word: &mut Vec<u16>,
    pattern: &mut Vec<u8>,
    scanner: &mut Scanner,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) {
    if *digit_sensed || chr < u32::from(b'0') || chr > u32::from(b'9') {
        if chr == u32::from(b'.') {
            chr = 0;
        } else {
            let lc_code = eqtb.lc_code(chr as usize);
            if !is_valid_hyphenation_code(lc_code) {
                logger.print_err("Nonletter");
                let help = &["(See Appendix H.)"];
                logger.error(help, scanner, eqtb);
                // U+2E80 is accepted as an upTeX case-table sentinel, but it
                // is not a tokenizable latin_ucs character and must never be
                // installed in the trie. The character has already been
                // consumed, so ignoring it also guarantees forward progress.
                return;
            }
            chr = lc_code as u32;
        }
        if word.len() < 63 {
            word.push(chr as u16);
            pattern.push(0);
            *digit_sensed = false;
        }
    } else if word.len() < 63 {
        pattern[word.len()] = (chr - u32::from(b'0')) as u8;
        *digit_sensed = true;
    }
}

impl Hyphenator {
    /// See 966.
    pub fn init_trie(&mut self) {
        self.trie = Some(self.pre_trie.to_trie_mut());
    }
}

impl Dumpable for EtexHyphenationCodes {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self(Dumpable::undump(lines)?))
    }
}

impl Dumpable for LatinUcsHyphenationCodes {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.0.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        Ok(Self(Vec::undump(lines)?))
    }
}

impl Dumpable for SavedHyphenationCodes {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.etex.dump(target)?;
        self.latin_ucs.dump(target)
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let codes = Self {
            etex: EtexHyphenationCodes::undump(lines)?,
            latin_ucs: LatinUcsHyphenationCodes::undump(lines)?,
        };
        if codes.is_valid() {
            Ok(codes)
        } else {
            Err(FormatError::ParseError)
        }
    }
}

impl Dumpable for Hyphenator {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        for map in &self.exceptions {
            map.dump(target)?;
        }

        self.saved_hyphenation_codes.dump(target)?;
        self.pre_trie.dump(target)?;
        self.trie.dump(target)?;

        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let mut hyphenator = Hyphenator::new();

        for i in 0..hyphenator.exceptions.len() {
            let map = HashMap::undump(lines)?;
            hyphenator.exceptions[i] = map;
        }

        hyphenator.saved_hyphenation_codes = Dumpable::undump(lines)?;
        hyphenator.has_saved_hyphenation_codes = hyphenator
            .saved_hyphenation_codes
            .iter()
            .any(Option::is_some);
        hyphenator.pre_trie = PreTrie::undump(lines)?;
        hyphenator.trie = Option::undump(lines)?;

        if !hyphenator.format_exceptions_are_valid()
            || !hyphenator.saved_hyphenation_codes_are_valid()
        {
            return Err(FormatError::ParseError);
        }

        Ok(hyphenator)
    }
}

impl Dumpable for HyfOp {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.distance.dump(target)?;
        self.num.dump(target)?;
        self.next.dump(target)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let distance = usize::undump(lines)?;
        let num = u8::undump(lines)?;
        let next = Option::undump(lines)?;
        Ok(Self {
            distance,
            num,
            next,
        })
    }
}

impl Dumpable for PreTrie {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.nodes.dump(target)?;
        self.subtrie_hash.dump(target)?;
        for ops in &self.hyf_ops {
            ops.dump(target)?;
        }
        for map in &self.op_code_hash {
            map.dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let mut pre_trie = PreTrie::new();
        pre_trie.nodes = Vec::undump(lines)?;
        // Representatives are a build-time cache. The reachable node graph is
        // authoritative, so do not trust stale or forged cache entries from a
        // format file.
        let _: HashMap<PreTrieNode, usize> = HashMap::undump(lines)?;
        pre_trie.subtrie_hash = HashMap::new();
        for i in 0..pre_trie.hyf_ops.len() {
            let ops = Vec::undump(lines)?;
            pre_trie.hyf_ops[i] = ops;
        }
        for i in 0..pre_trie.op_code_hash.len() {
            let map = HashMap::undump(lines)?;
            pre_trie.op_code_hash[i] = map;
        }
        pre_trie
            .format_state_is_valid()
            .then_some(pre_trie)
            .ok_or(FormatError::ParseError)
    }
}

impl Dumpable for PreTrieNode {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.c.dump(target)?;
        self.op.dump(target)?;
        self.first_child.dump(target)?;
        self.next_sibling.dump(target)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let c = u16::undump(lines)?;
        if u32::from(c) > MAX_LATIN_UCS_CODE {
            return Err(FormatError::ParseError);
        }
        let op = Option::undump(lines)?;
        let first_child = Option::undump(lines)?;
        let next_sibling = Option::undump(lines)?;
        Ok(Self {
            c,
            op,
            first_child,
            next_sibling,
        })
    }
}

impl Dumpable for Trie {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.nodes.dump(target)?;
        for ops in &self.hyf_ops {
            ops.dump(target)?;
        }
        for map in &self.op_code_hash {
            map.dump(target)?;
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let nodes = Vec::undump(lines)?;
        let mut hyf_ops: [_; 256] = std::array::from_fn(|_| Vec::new());
        for i in 0..hyf_ops.len() {
            let ops = Vec::undump(lines)?;
            hyf_ops[i] = ops;
        }
        let mut op_code_hash: [_; 256] = std::array::from_fn(|_| HashMap::new());
        for i in 0..op_code_hash.len() {
            let map = HashMap::undump(lines)?;
            op_code_hash[i] = map;
        }
        let trie = Self {
            nodes,
            hyf_ops,
            op_code_hash,
        };
        trie.format_state_is_valid()
            .then_some(trie)
            .ok_or(FormatError::ParseError)
    }
}

impl Dumpable for TrieNode {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.link.dump(target)?;
        self.chr.dump(target)?;
        self.op.dump(target)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let link = Option::undump(lines)?;
        let chr: Option<u16> = Option::undump(lines)?;
        if chr.is_some_and(|c| u32::from(c) > MAX_LATIN_UCS_CODE) {
            return Err(FormatError::ParseError);
        }
        let op = Option::undump(lines)?;
        Ok(Self { link, chr, op })
    }
}

#[cfg(test)]
mod latin_ucs_tests {
    use super::*;

    fn dump_to_string(value: &impl Dumpable) -> String {
        let mut bytes = Vec::new();
        value.dump(&mut bytes).unwrap();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn 未登録の高位unicode欧文は圧縮trieの範囲外を読まない() {
        let empty = TrieNode {
            link: None,
            chr: None,
            op: None,
        };
        let mut nodes = vec![empty; 257];
        nodes[1] = TrieNode {
            link: Some(1),
            chr: Some(0),
            op: None,
        };
        let trie = Trie {
            nodes,
            hyf_ops: std::array::from_fn(|_| Vec::new()),
            op_code_hash: std::array::from_fn(|_| HashMap::new()),
        };
        let mut pattern = vec![0; 2];

        assert!(trie.determine_hyph_pattern(&[0, MAX_LATIN_UCS_CODE as u16, 0], 0, &mut pattern));
        assert_eq!(pattern, vec![0; 2]);
    }

    #[test]
    fn 範囲外のlanguageは圧縮trieを読まない() {
        let trie = Trie {
            nodes: vec![
                TrieNode {
                    link: None,
                    chr: None,
                    op: None,
                };
                257
            ],
            hyf_ops: std::array::from_fn(|_| Vec::new()),
            op_code_hash: std::array::from_fn(|_| HashMap::new()),
        };
        let mut pattern = vec![0; 2];

        assert!(!trie.determine_hyph_pattern(&[0, b'a' as u16, 0], usize::MAX, &mut pattern,));
        assert_eq!(pattern, vec![0; 2]);
    }

    #[test]
    fn unicode欧文の長い兄弟列は再帰せずに縮約する() {
        let last_code = MAX_LATIN_UCS_CODE as u16;
        let mut nodes = Vec::with_capacity(usize::from(last_code) + 3);
        nodes.push(PreTrieNode {
            c: 0,
            op: None,
            first_child: Some(1),
            next_sibling: None,
        });
        nodes.push(PreTrieNode {
            c: 0,
            op: None,
            first_child: Some(2),
            next_sibling: None,
        });
        for c in 0..=last_code {
            let index = nodes.len();
            nodes.push(PreTrieNode {
                c,
                op: None,
                first_child: None,
                next_sibling: (c < last_code).then_some(index + 1),
            });
        }
        let mut pre_trie = PreTrie::new();
        pre_trie.nodes = nodes;

        assert!(pre_trie.format_state_is_valid());
        pre_trie.reduce();
        assert!(pre_trie.format_state_is_valid());
    }

    #[test]
    fn unicode欧文patternの事前trieと圧縮trieをformat往復する() {
        let mut pre_trie = PreTrie::new();
        let op = pre_trie.new_trie_op(
            HyfOp {
                distance: 0,
                num: 3,
                next: None,
            },
            0,
        );
        pre_trie
            .insert(&[0, u16::from(b'b'), 0x00DF], Some(op))
            .unwrap();
        let trie = pre_trie.to_trie_mut();

        let pre_input = dump_to_string(&pre_trie);
        assert!(PreTrie::undump(&mut pre_input.lines()).is_ok());
        let trie_input = dump_to_string(&trie);
        assert!(Trie::undump(&mut trie_input.lines()).is_ok());
    }

    #[test]
    fn 循環する事前trieをformatから読まない() {
        let mut pre_trie = PreTrie::new();
        pre_trie.nodes[0].first_child = Some(1);
        pre_trie.nodes.push(PreTrieNode {
            c: 0,
            op: None,
            first_child: Some(1),
            next_sibling: None,
        });
        let input = dump_to_string(&pre_trie);

        assert!(matches!(
            PreTrie::undump(&mut input.lines()),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 範囲外のハイフン操作をformatから読まない() {
        let mut pre_trie = PreTrie::new();
        let bad_op = HyfOp {
            distance: 64,
            num: 1,
            next: None,
        };
        pre_trie.hyf_ops[0].push(bad_op);
        pre_trie.op_code_hash[0].insert(bad_op, 0);
        let input = dump_to_string(&pre_trie);

        assert!(matches!(
            PreTrie::undump(&mut input.lines()),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 圧縮trieの範囲外参照をformatから読まない() {
        let empty = TrieNode {
            link: None,
            chr: None,
            op: None,
        };
        let mut trie = Trie {
            nodes: vec![empty; 257],
            hyf_ops: std::array::from_fn(|_| Vec::new()),
            op_code_hash: std::array::from_fn(|_| HashMap::new()),
        };
        trie.nodes[1] = TrieNode {
            link: Some(258),
            chr: Some(0),
            op: None,
        };
        let input = dump_to_string(&trie);

        assert!(matches!(
            Trie::undump(&mut input.lines()),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 圧縮trieの言語別範囲外操作をformatから読まない() {
        let empty = TrieNode {
            link: None,
            chr: None,
            op: None,
        };
        let mut trie = Trie {
            nodes: vec![empty; 258],
            hyf_ops: std::array::from_fn(|_| Vec::new()),
            op_code_hash: std::array::from_fn(|_| HashMap::new()),
        };
        trie.nodes[1] = TrieNode {
            link: Some(257),
            chr: Some(0),
            op: None,
        };
        trie.nodes[257] = TrieNode {
            link: None,
            chr: Some(0),
            op: Some(0),
        };
        let input = dump_to_string(&trie);

        assert!(matches!(
            Trie::undump(&mut input.lines()),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 例外単語の範囲外位置をformatから読まない() {
        let mut hyphenator = Hyphenator::new();
        hyphenator.exceptions[0].insert(vec![u16::from(b'a'), u16::from(b'b')], vec![3]);
        let input = dump_to_string(&hyphenator);

        assert!(matches!(
            Hyphenator::undump(&mut input.lines()),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 保存hyphenation_codeの壊れた値をformatから読まない() {
        let mut hyphenator = Hyphenator::new();
        let mut etex = [0; 256];
        etex[usize::from(b'A')] = b'a';
        hyphenator.saved_hyphenation_codes[0] = Some(Box::new(SavedHyphenationCodes {
            etex: EtexHyphenationCodes(etex),
            latin_ucs: LatinUcsHyphenationCodes(vec![(256, 256)]),
        }));
        let input = dump_to_string(&hyphenator);
        let mut lines: Vec<_> = input.lines().map(str::to_owned).collect();
        // 256 empty exception maps and language 0's `Some` precede its dense
        // byte table. A value outside u8 must fail before any allocation.
        lines[257 + usize::from(b'A')] = "256".to_owned();

        assert!(matches!(
            Hyphenator::undump(&mut lines.join("\n").lines()),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 保存hyphenation_codeの重複keyとlatin_ucs範囲外を拒否する() {
        let duplicated = SavedHyphenationCodes {
            etex: EtexHyphenationCodes([0; 256]),
            latin_ucs: LatinUcsHyphenationCodes(vec![(256, 256), (256, 257)]),
        };
        assert!(!duplicated.is_valid());

        let outside = SavedHyphenationCodes {
            etex: EtexHyphenationCodes([0; 256]),
            latin_ucs: LatinUcsHyphenationCodes(vec![(256, MAX_LATIN_UCS_CODE as u16 + 1)]),
        };
        assert!(!outside.is_valid());
    }
}

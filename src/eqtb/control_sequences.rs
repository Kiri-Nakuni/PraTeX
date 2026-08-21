use super::{CatCode, Eqtb, FontIndex, Variable, VariableLevels};
use crate::command::{Command, ExpandableCommand, MacroCall};
use crate::format::{Dumpable, FormatError};
use crate::macros::Macro;
use crate::print::Printer;

use std::collections::HashMap;
use std::io::Write;

type CommandStoreEntry = (Command, Vec<u8>);
pub type ControlSequenceId = u16;

/// 名前空間の番号。**名前そのものは持ち回らない。**
///
/// `\namespace foo\csname bar\endcsname` の `foo` を一度だけ番号に直し、
/// 以後は番号で引く。
pub type NamespaceId = u16;

/// 制御綴の名前を、呼び出し側の `&[u8]` のまま引く表。
///
/// 通常の制御綴と活性文字を分けることで、複合キーを作るための
/// `Vec<u8>` の一時確保を検索経路から外す。
#[derive(Default)]
struct ControlSequenceHash {
    normal: HashMap<Vec<u8>, ControlSequenceId>,
    active: HashMap<Vec<u8>, ControlSequenceId>,
}

impl ControlSequenceHash {
    fn get(&self, active: bool, key: &[u8]) -> Option<ControlSequenceId> {
        let hash = if active { &self.active } else { &self.normal };
        hash.get(key).copied()
    }

    fn insert(&mut self, active: bool, key: Vec<u8>, id: ControlSequenceId) {
        let hash = if active {
            &mut self.active
        } else {
            &mut self.normal
        };
        hash.insert(key, id);
    }

    fn len(&self) -> usize {
        self.normal.len() + self.active.len()
    }
}

/// Specifies a control sequence in the Eqtb.
/// See 222.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlSequence {
    Active(u8),
    Single(u8),
    NullCs,
    Escaped(ControlSequenceId),
    FrozenProtection,
    FrozenCr,
    FrozenEndGroup,
    FrozenRight,
    FrozenFi,
    FrozenEndTemplate,
    FrozenEndv,
    FrozenRelax,
    EndWrite,
    FontId(FontIndex),
    Undefined,
}

/// 名前空間の印と名前を出す。**返り値は活性文字なら その文字。**
///
/// `namespacechar` が範囲外（既定は −1）なら**何も出さない**——
/// 名前空間を使わない文書では見えない。
fn print_namespace_prefix(
    cs: ControlSequence,
    eqtb: &Eqtb,
    printer: &mut impl Printer,
) -> Option<u8> {
    if let Some(ns) = eqtb.control_sequences.namespace_of(cs) {
        let nc = eqtb.integer(crate::eqtb::IntegerVariable::NamespaceChar);
        if (0..=255).contains(&nc) {
            printer.print(nc as u8);
            // **名前を写さずに出す。** 借りたまま印字する
            let n = eqtb.control_sequences.namespace_name(ns).to_vec();
            for b in n {
                printer.print(b);
            }
        }
    }
    eqtb.control_sequences.active_char(cs).filter(|_| {
        matches!(cs, ControlSequence::Escaped(_))
    })
}

impl ControlSequence {
    /// See 262.
    pub fn print_cs(self, eqtb: &Eqtb, printer: &mut impl Printer) {
        match self {
            ControlSequence::Active(c) => {
                printer.print(c);
            }
            ControlSequence::Single(c) => {
                printer.print_esc_str(std::slice::from_ref(&c));
                if eqtb.cat_code(c) == CatCode::Letter {
                    printer.print_char(b' ');
                }
            }
            ControlSequence::NullCs => {
                printer.print_esc_str(b"csname");
                printer.print_esc_str(b"endcsname");
                printer.print_char(b' ');
            }
            _ => {
                let active = print_namespace_prefix(self, eqtb, printer);
                match active {
                    // 名前空間つきの活性文字。**`escapechar` を挟まない**
                    Some(c) => printer.print(c),
                    None => printer.print_esc_str(eqtb.control_sequences.text(self)),
                }
                printer.print_char(b' ');
            }
        }
    }

    /// The same as `print_cs` but without trailing whitespaces.
    /// See 263.
    pub fn sprint_cs(self, eqtb: &Eqtb, printer: &mut impl Printer) {
        match self {
            ControlSequence::Active(c) => {
                printer.print(c);
            }
            ControlSequence::Single(c) => {
                printer.print_esc_str(std::slice::from_ref(&c));
            }
            ControlSequence::NullCs => {
                printer.print_esc_str(b"csname");
                printer.print_esc_str(b"endcsname");
            }
            _ => {
                let active = print_namespace_prefix(self, eqtb, printer);
                if let Some(c) = active {
                    printer.print(c);
                    return;
                }
                printer.print_esc_str(eqtb.control_sequences.text(self));
            }
        }
    }

    pub fn to_variable(self) -> Variable {
        Variable::ControlSequence(self)
    }
}

/// Store for each ControlSequence the corresponding command, level, and name.
/// See 222.
pub struct ControlSequenceStore {
    active: Vec<(Command, Vec<u8>)>,
    single: Vec<(Command, Vec<u8>)>,
    null_cs: (Command, Vec<u8>),

    /// global の制御綴。名前空間を使わない通常経路はここを一度だけ引く。
    ///
    /// 通常名と活性文字を分けるのは、**`*lib\~` と `*lib~` が衝突する**からである。
    /// どちらも名前が一文字の `~` になる。
    global_hash: ControlSequenceHash,
    /// 名前空間の番号を添字にする。番号をもう一度ハッシュせず、名前だけを借用して引く。
    namespace_hashes: Vec<ControlSequenceHash>,
    escaped: Vec<(Command, Vec<u8>)>,
    /// `escaped` と並ぶ。**どの名前空間の出自か。** `None` が global
    escaped_ns: Vec<Option<NamespaceId>>,
    /// `escaped` と並ぶ。**名前空間つきの active char なら、その文字。**
    ///
    /// 名前空間つきの active char も同じ番号空間に載せる——
    /// そうすると save stack も群も `\global` も**そのまま付いてくる**
    escaped_active: Vec<Option<u8>>,
    /// 名前空間の名前。番号が添字
    namespaces: Vec<Vec<u8>>,
    ns_index: HashMap<Vec<u8>, NamespaceId>,
    pub cs_count: usize,

    frozen_protection: (Command, Vec<u8>),
    frozen_cr: (Command, Vec<u8>),
    frozen_end_group: (Command, Vec<u8>),
    frozen_right: (Command, Vec<u8>),
    frozen_fi: (Command, Vec<u8>),
    frozen_end_template: (Command, Vec<u8>),
    frozen_endv: (Command, Vec<u8>),
    frozen_relax: (Command, Vec<u8>),
    end_write: (Command, Vec<u8>),
    font_id: Vec<(Command, Vec<u8>)>,
    undefined: (Command, Vec<u8>),
}

impl ControlSequenceStore {
    /// All control sequence are initiated as undefined.
    /// See 222.
    pub fn new() -> Self {
        let mut active = Vec::new();
        for _ in 0..256 {
            active.push((
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ));
        }
        let mut single = Vec::new();
        for _ in 0..256 {
            single.push((
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ));
        }
        let mut font_id = Vec::new();
        for _ in 0..=FontIndex::MAX {
            font_id.push((
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ));
        }
        Self {
            active,
            single,
            escaped_ns: Vec::new(),
            escaped_active: Vec::new(),
            namespaces: Vec::new(),
            ns_index: HashMap::new(),
            null_cs: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),

            global_hash: ControlSequenceHash::default(),
            namespace_hashes: Vec::new(),
            escaped: Vec::new(),
            cs_count: 0,

            frozen_protection: (
                // See 1216.
                Command::Expandable(ExpandableCommand::Undefined),
                b"inaccessible".to_vec(),
            ),
            frozen_cr: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),
            frozen_end_group: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),
            frozen_right: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),
            frozen_fi: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),
            frozen_end_template: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),
            frozen_endv: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),
            frozen_relax: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),
            end_write: (
                // See 1369.
                Command::Expandable(ExpandableCommand::Macro(MacroCall {
                    long: false,
                    outer: true,
                    protected: false,
                    macro_def: std::rc::Rc::new(Macro::default()),
                })),
                b"endwrite".to_vec(),
            ),
            font_id,
            undefined: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),
        }
    }

    /// See 259.
    pub fn id_lookup(&self, key: &[u8]) -> Option<ControlSequenceId> {
        self.id_lookup_ns(None, false, key)
    }

    /// 名前空間つきで引く。`None` は global。
    pub fn id_lookup_ns(
        &self,
        ns: Option<NamespaceId>,
        active: bool,
        key: &[u8],
    ) -> Option<ControlSequenceId> {
        match ns {
            None => self.global_hash.get(active, key),
            Some(ns) => self.namespace_hashes.get(ns as usize)?.get(active, key),
        }
    }

    /// 名前空間の名前を番号に直す。**同じ名前なら同じ番号。**
    pub fn intern_namespace(&mut self, name: &[u8]) -> NamespaceId {
        if let Some(&id) = self.ns_index.get(name) {
            return id;
        }
        let id = self.namespaces.len() as NamespaceId;
        self.namespaces.push(name.to_vec());
        self.namespace_hashes.push(ControlSequenceHash::default());
        self.ns_index.insert(name.to_vec(), id);
        id
    }

    pub fn namespace_name(&self, id: NamespaceId) -> &[u8] {
        self.namespaces.get(id as usize).map(|v| &v[..]).unwrap_or(b"")
    }

    /// **この制御綴はどの名前空間のものか。** global なら `None`。
    pub fn namespace_of(&self, cs: ControlSequence) -> Option<NamespaceId> {
        match cs {
            ControlSequence::Escaped(n) => {
                self.escaped_ns.get(n as usize).copied().flatten()
            }
            _ => None,
        }
    }

    /// **これは active char か。** そうならその文字。
    ///
    /// `ControlSequence::Active(c)` を分解する代わりにここへ尋ねる——
    /// 名前空間つきの active char は `Escaped` の番号空間に載っているので、
    /// **分解では見つからない**（Phase 5）。
    pub fn active_char(&self, cs: ControlSequence) -> Option<u8> {
        match cs {
            ControlSequence::Active(c) => Some(c),
            ControlSequence::Escaped(n) => {
                self.escaped_active.get(n as usize).copied().flatten()
            }
            _ => None,
        }
    }

    /// See 259.
    pub fn add_command(
        &mut self,
        key: &[u8],
        variable_levels: &mut VariableLevels,
    ) -> Result<ControlSequenceId, ()> {
        self.add_command_ns(None, key, None, variable_levels)
    }

    /// 名前空間つきで作る。`active` はその文字（名前空間つき active char のとき）。
    ///
    /// **同じ番号空間に載せるのが肝である。** `escaped` が伸びれば
    /// `VariableLevels` も伸びるので、**save stack と群と `\global` がただで付いてくる。**
    pub fn add_command_ns(
        &mut self,
        ns: Option<NamespaceId>,
        key: &[u8],
        active: Option<u8>,
        variable_levels: &mut VariableLevels,
    ) -> Result<ControlSequenceId, ()> {
        if ns.is_some_and(|ns| ns as usize >= self.namespace_hashes.len()) {
            return Err(());
        }
        let Ok(n) = ControlSequenceId::try_from(self.cs_count) else {
            return Err(());
        };
        self.cs_count += 1;
        let hash = match ns {
            None => &mut self.global_hash,
            Some(ns) => &mut self.namespace_hashes[ns as usize],
        };
        hash.insert(active.is_some(), key.to_vec(), n);
        let cmd = Command::Expandable(ExpandableCommand::Undefined);
        self.escaped.push((cmd, key.to_vec()));
        self.escaped_ns.push(ns);
        self.escaped_active.push(active);
        // NOTE: We need to extend the memory slots for levels as well.
        variable_levels.add_new_escaped_command();
        Ok(n)
    }

    fn index(&self, index: ControlSequence) -> &(Command, Vec<u8>) {
        match index {
            ControlSequence::Active(c) => &self.active[c as usize],
            ControlSequence::Single(c) => &self.single[c as usize],
            ControlSequence::NullCs => &self.null_cs,
            ControlSequence::Escaped(n) => &self.escaped[n as usize],
            ControlSequence::FrozenProtection => &self.frozen_protection,
            ControlSequence::FrozenCr => &self.frozen_cr,
            ControlSequence::FrozenEndGroup => &self.frozen_end_group,
            ControlSequence::FrozenRight => &self.frozen_right,
            ControlSequence::FrozenFi => &self.frozen_fi,
            ControlSequence::FrozenEndTemplate => &self.frozen_end_template,
            ControlSequence::FrozenEndv => &self.frozen_endv,
            ControlSequence::FrozenRelax => &self.frozen_relax,
            ControlSequence::EndWrite => &self.end_write,
            ControlSequence::FontId(n) => &self.font_id[n as usize],
            ControlSequence::Undefined => &self.undefined,
        }
    }

    fn index_mut(&mut self, index: ControlSequence) -> &mut (Command, Vec<u8>) {
        match index {
            ControlSequence::Active(c) => &mut self.active[c as usize],
            ControlSequence::Single(c) => &mut self.single[c as usize],
            ControlSequence::NullCs => &mut self.null_cs,
            ControlSequence::Escaped(n) => &mut self.escaped[n as usize],
            ControlSequence::FrozenProtection => &mut self.frozen_protection,
            ControlSequence::FrozenCr => &mut self.frozen_cr,
            ControlSequence::FrozenEndGroup => &mut self.frozen_end_group,
            ControlSequence::FrozenRight => &mut self.frozen_right,
            ControlSequence::FrozenFi => &mut self.frozen_fi,
            ControlSequence::FrozenEndTemplate => &mut self.frozen_end_template,
            ControlSequence::FrozenEndv => &mut self.frozen_endv,
            ControlSequence::FrozenRelax => &mut self.frozen_relax,
            ControlSequence::EndWrite => &mut self.end_write,
            ControlSequence::FontId(n) => &mut self.font_id[n as usize],
            ControlSequence::Undefined => &mut self.undefined,
        }
    }

    pub fn text(&self, cs: ControlSequence) -> &[u8] {
        &self.index(cs).1
    }

    pub fn set_text(&mut self, cs: ControlSequence, text: &[u8]) {
        self.index_mut(cs).1 = text.to_vec();
    }

    pub fn get(&self, cs: ControlSequence) -> &Command {
        &self.index(cs).0
    }

    pub fn set(&mut self, cs: ControlSequence, new_command: Command) -> Command {
        std::mem::replace(&mut self.index_mut(cs).0, new_command)
    }

    /// fmt は従来どおり `(名前空間, 活性か, 名前)` の順で書く。
    /// 実行時の表だけを分け、保存済み fmt の読み書きは変えない。
    fn dump_hash(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        let hash_len = self.global_hash.len()
            + self
                .namespace_hashes
                .iter()
                .map(ControlSequenceHash::len)
                .sum::<usize>();
        writeln!(target, "{hash_len}")?;
        Self::dump_hash_entries(None, false, &self.global_hash.normal, target)?;
        Self::dump_hash_entries(None, true, &self.global_hash.active, target)?;
        for (ns, hash) in self.namespace_hashes.iter().enumerate() {
            let ns = Some(ns as NamespaceId);
            Self::dump_hash_entries(ns, false, &hash.normal, target)?;
            Self::dump_hash_entries(ns, true, &hash.active, target)?;
        }
        Ok(())
    }

    fn dump_hash_entries(
        ns: Option<NamespaceId>,
        active: bool,
        hash: &HashMap<Vec<u8>, ControlSequenceId>,
        target: &mut impl Write,
    ) -> Result<(), std::io::Error> {
        for (key, id) in hash {
            ns.dump(target)?;
            active.dump(target)?;
            key.dump(target)?;
            id.dump(target)?;
        }
        Ok(())
    }
}

impl Dumpable for ControlSequenceStore {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.active.dump(target)?;
        self.single.dump(target)?;
        self.null_cs.dump(target)?;
        self.dump_hash(target)?;
        self.escaped.dump(target)?;
        self.escaped_ns.dump(target)?;
        self.escaped_active.dump(target)?;
        self.namespaces.dump(target)?;
        self.cs_count.dump(target)?;
        self.frozen_protection.dump(target)?;
        self.frozen_cr.dump(target)?;
        self.frozen_end_group.dump(target)?;
        self.frozen_right.dump(target)?;
        self.frozen_fi.dump(target)?;
        self.frozen_end_template.dump(target)?;
        self.frozen_endv.dump(target)?;
        self.frozen_relax.dump(target)?;
        self.end_write.dump(target)?;
        self.font_id.dump(target)?;
        self.undefined.dump(target)?;
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let active = Vec::undump(lines)?;
        let single = Vec::undump(lines)?;
        let null_cs = CommandStoreEntry::undump(lines)?;
        let hash: HashMap<(Option<NamespaceId>, bool, Vec<u8>), ControlSequenceId> =
            HashMap::undump(lines)?;
        let escaped = Vec::undump(lines)?;
        let escaped_ns: Vec<Option<NamespaceId>> = Vec::undump(lines)?;
        let escaped_active: Vec<Option<u8>> = Vec::undump(lines)?;
        let namespaces: Vec<Vec<u8>> = Vec::undump(lines)?;
        // **番号から名前を引く表は書き出す。逆は組み直す**——写す意味が無い
        let ns_index = namespaces
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), i as NamespaceId))
            .collect();
        let mut global_hash = ControlSequenceHash::default();
        let mut namespace_hashes = (0..namespaces.len())
            .map(|_| ControlSequenceHash::default())
            .collect::<Vec<_>>();
        for ((ns, active, key), id) in hash {
            match ns {
                None => global_hash.insert(active, key, id),
                Some(ns) => {
                    let Some(hash) = namespace_hashes.get_mut(ns as usize) else {
                        return Err(FormatError::ParseError);
                    };
                    hash.insert(active, key, id);
                }
            }
        }
        let cs_count = usize::undump(lines)?;
        let frozen_protection = CommandStoreEntry::undump(lines)?;
        let frozen_cr = CommandStoreEntry::undump(lines)?;
        let frozen_end_group = CommandStoreEntry::undump(lines)?;
        let frozen_right = CommandStoreEntry::undump(lines)?;
        let frozen_fi = CommandStoreEntry::undump(lines)?;
        let frozen_end_template = CommandStoreEntry::undump(lines)?;
        let frozen_endv = CommandStoreEntry::undump(lines)?;
        let frozen_relax = CommandStoreEntry::undump(lines)?;
        let end_write = CommandStoreEntry::undump(lines)?;
        let font_id = Vec::undump(lines)?;
        let undefined = CommandStoreEntry::undump(lines)?;
        Ok(Self {
            active,
            single,
            null_cs,
            global_hash,
            namespace_hashes,
            escaped,
            escaped_ns,
            escaped_active,
            namespaces,
            ns_index,
            cs_count,
            frozen_protection,
            frozen_cr,
            frozen_end_group,
            frozen_right,
            frozen_fi,
            frozen_end_template,
            frozen_endv,
            frozen_relax,
            end_write,
            font_id,
            undefined,
        })
    }
}

impl Dumpable for ControlSequence {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Active(c) => {
                writeln!(target, "Active")?;
                c.dump(target)?;
            }
            Self::Single(c) => {
                writeln!(target, "Single")?;
                c.dump(target)?;
            }
            Self::NullCs => {
                writeln!(target, "NullCs")?;
            }
            Self::Escaped(n) => {
                writeln!(target, "Escaped")?;
                n.dump(target)?;
            }
            Self::FrozenProtection => {
                writeln!(target, "FrozenProtection")?;
            }
            Self::FrozenCr => {
                writeln!(target, "FrozenCr")?;
            }
            Self::FrozenEndGroup => {
                writeln!(target, "FrozenEndGroup")?;
            }
            Self::FrozenRight => {
                writeln!(target, "FrozenRight")?;
            }
            Self::FrozenFi => {
                writeln!(target, "FrozenFi")?;
            }
            Self::FrozenEndTemplate => {
                writeln!(target, "FrozenEndTemplate")?;
            }
            Self::FrozenEndv => {
                writeln!(target, "FrozenEndv")?;
            }
            Self::FrozenRelax => {
                writeln!(target, "FrozenRelax")?;
            }
            Self::EndWrite => {
                writeln!(target, "EndWrite")?;
            }
            Self::FontId(n) => {
                writeln!(target, "FontId")?;
                n.dump(target)?;
            }
            Self::Undefined => {
                writeln!(target, "Undefined")?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        let variant = lines.next().ok_or(FormatError::IncompleteFile)?;
        match variant {
            "Active" => {
                let c = u8::undump(lines)?;
                Ok(Self::Active(c))
            }
            "Single" => {
                let c = u8::undump(lines)?;
                Ok(Self::Single(c))
            }
            "NullCs" => Ok(Self::NullCs),
            "Escaped" => {
                let n = ControlSequenceId::undump(lines)?;
                Ok(Self::Escaped(n))
            }
            "FrozenProtection" => Ok(Self::FrozenProtection),
            "FrozenCr" => Ok(Self::FrozenCr),
            "FrozenEndGroup" => Ok(Self::FrozenEndGroup),
            "FrozenRight" => Ok(Self::FrozenRight),
            "FrozenFi" => Ok(Self::FrozenFi),
            "FrozenEndTemplate" => Ok(Self::FrozenEndTemplate),
            "FrozenEndv" => Ok(Self::FrozenEndv),
            "FrozenRelax" => Ok(Self::FrozenRelax),
            "EndWrite" => Ok(Self::EndWrite),
            "FontId" => {
                let n = FontIndex::undump(lines)?;
                Ok(Self::FontId(n))
            }
            "Undefined" => Ok(Self::Undefined),
            _ => Err(FormatError::ParseError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_control_sequence() {
        let active = ControlSequence::Active(1);
        let single = ControlSequence::Single(2);
        let null_cs = ControlSequence::NullCs;
        let escaped = ControlSequence::Escaped(3);
        let frozen_protection = ControlSequence::FrozenProtection;
        let frozen_cr = ControlSequence::FrozenCr;
        let frozen_end_group = ControlSequence::FrozenEndGroup;
        let frozen_right = ControlSequence::FrozenRight;
        let frozen_fi = ControlSequence::FrozenFi;
        let frozen_end_template = ControlSequence::FrozenEndTemplate;
        let frozen_endv = ControlSequence::FrozenEndv;
        let frozen_relax = ControlSequence::FrozenRelax;
        let end_write = ControlSequence::EndWrite;
        let font_id = ControlSequence::FontId(4);
        let undefined = ControlSequence::Undefined;

        let mut file = Vec::new();
        active.dump(&mut file).unwrap();
        single.dump(&mut file).unwrap();
        null_cs.dump(&mut file).unwrap();
        escaped.dump(&mut file).unwrap();
        frozen_protection.dump(&mut file).unwrap();
        frozen_cr.dump(&mut file).unwrap();
        frozen_end_group.dump(&mut file).unwrap();
        frozen_right.dump(&mut file).unwrap();
        frozen_fi.dump(&mut file).unwrap();
        frozen_end_template.dump(&mut file).unwrap();
        frozen_endv.dump(&mut file).unwrap();
        frozen_relax.dump(&mut file).unwrap();
        end_write.dump(&mut file).unwrap();
        font_id.dump(&mut file).unwrap();
        undefined.dump(&mut file).unwrap();

        let input = String::from_utf8(file).unwrap();
        let mut lines = input.lines();
        let active_undumped = ControlSequence::undump(&mut lines).unwrap();
        let single_undumped = ControlSequence::undump(&mut lines).unwrap();
        let null_cs_undumped = ControlSequence::undump(&mut lines).unwrap();
        let escaped_undumped = ControlSequence::undump(&mut lines).unwrap();
        let frozen_protection_undumped = ControlSequence::undump(&mut lines).unwrap();
        let frozen_cr_undumped = ControlSequence::undump(&mut lines).unwrap();
        let frozen_end_group_undumped = ControlSequence::undump(&mut lines).unwrap();
        let frozen_right_undumped = ControlSequence::undump(&mut lines).unwrap();
        let frozen_fi_undumped = ControlSequence::undump(&mut lines).unwrap();
        let frozen_end_template_undumped = ControlSequence::undump(&mut lines).unwrap();
        let frozen_endv_undumped = ControlSequence::undump(&mut lines).unwrap();
        let frozen_relax_undumped = ControlSequence::undump(&mut lines).unwrap();
        let end_write_undumped = ControlSequence::undump(&mut lines).unwrap();
        let font_id_undumped = ControlSequence::undump(&mut lines).unwrap();
        let undefined_undumped = ControlSequence::undump(&mut lines).unwrap();

        assert_eq!(active, active_undumped);
        assert_eq!(single, single_undumped);
        assert_eq!(null_cs, null_cs_undumped);
        assert_eq!(escaped, escaped_undumped);
        assert_eq!(frozen_protection, frozen_protection_undumped);
        assert_eq!(frozen_cr, frozen_cr_undumped);
        assert_eq!(frozen_end_group, frozen_end_group_undumped);
        assert_eq!(frozen_right, frozen_right_undumped);
        assert_eq!(frozen_fi, frozen_fi_undumped);
        assert_eq!(frozen_end_template, frozen_end_template_undumped);
        assert_eq!(frozen_endv, frozen_endv_undumped);
        assert_eq!(frozen_relax, frozen_relax_undumped);
        assert_eq!(end_write, end_write_undumped);
        assert_eq!(font_id, font_id_undumped);
        assert_eq!(undefined, undefined_undumped);
    }
}

#[cfg(test)]
mod namespace_tests {
    use super::*;
    use crate::eqtb::Eqtb;

    fn e() -> Eqtb {
        Eqtb::new()
    }

    #[test]
    fn 名前空間の番号は名前で共有される() {
        let mut e = e();
        let a = e.control_sequences.intern_namespace(b"foo");
        let b = e.control_sequences.intern_namespace(b"foo");
        let c = e.control_sequences.intern_namespace(b"bar");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(e.control_sequences.namespace_name(a), b"foo");
    }

    #[test]
    fn 名前空間が違えば別の制御綴になる() {
        let mut e = e();
        let foo = e.control_sequences.intern_namespace(b"foo");
        let bar = e.control_sequences.intern_namespace(b"bar");
        let a = e.lookup_or_create_ns(Some(foo), b"x", None).unwrap();
        let b = e.lookup_or_create_ns(Some(bar), b"x", None).unwrap();
        let g = e.lookup_or_create(b"x").unwrap();
        assert_ne!(a, b);
        assert_ne!(a, g);
        assert_ne!(b, g);
    }

    #[test]
    fn 同じ名前空間の同じ名前は同じ制御綴() {
        let mut e = e();
        let foo = e.control_sequences.intern_namespace(b"foo");
        let a = e.lookup_or_create_ns(Some(foo), b"x", None).unwrap();
        let b = e.lookup_or_create_ns(Some(foo), b"x", None).unwrap();
        assert_eq!(a, b);
        assert_eq!(e.lookup_ns(Some(foo), b"x"), Some(a));
    }

    #[test]
    fn 一文字も名前空間に入る() {
        // **一文字の短絡は名前空間版では行わない。** `Single` は global 専用である
        let mut e = e();
        let foo = e.control_sequences.intern_namespace(b"foo");
        let a = e.lookup_or_create_ns(Some(foo), b"x", None).unwrap();
        assert!(matches!(a, ControlSequence::Escaped(_)));
        assert_eq!(e.lookup_or_create(b"x").unwrap(), ControlSequence::Single(b'x'));
    }

    #[test]
    fn 空の名前はグローバルへ落ちる() {
        // **「空は global へ落ちる」を統一規則とする。** エラーにしない
        let mut e = e();
        let foo = e.control_sequences.intern_namespace(b"foo");
        assert_eq!(
            e.lookup_or_create_ns(Some(foo), b"", None).unwrap(),
            ControlSequence::NullCs
        );
        assert_eq!(e.lookup_ns(Some(foo), b""), Some(ControlSequence::NullCs));
    }

    #[test]
    fn 出自を問い合わせられる() {
        let mut e = e();
        let foo = e.control_sequences.intern_namespace(b"foo");
        let a = e.lookup_or_create_ns(Some(foo), b"x", None).unwrap();
        let g = e.lookup_or_create(b"yy").unwrap();
        assert_eq!(e.control_sequences.namespace_of(a), Some(foo));
        assert_eq!(e.control_sequences.namespace_of(g), None);
    }

    #[test]
    fn 名前空間つきの活性文字は種別を答える() {
        // **`ControlSequence::Active(c)` の分解では見つからない**（Phase 5）
        let mut e = e();
        let foo = e.control_sequences.intern_namespace(b"foo");
        let a = e.lookup_or_create_ns(Some(foo), b"~", Some(b'~')).unwrap();
        assert_eq!(e.control_sequences.active_char(a), Some(b'~'));
        let n = e.lookup_or_create_ns(Some(foo), b"x", None).unwrap();
        assert_eq!(e.control_sequences.active_char(n), None);
        assert_eq!(
            e.control_sequences.active_char(ControlSequence::Active(b'~')),
            Some(b'~')
        );
    }

    #[test]
    fn 制御綴の索引はfmtを往復しても区別を保つ() {
        let mut e = e();
        let global = e.lookup_or_create(b"same").unwrap();
        let foo = e.control_sequences.intern_namespace(b"foo");
        let bar = e.control_sequences.intern_namespace(b"bar");
        let foo_normal = e.lookup_or_create_ns(Some(foo), b"same", None).unwrap();
        let bar_normal = e.lookup_or_create_ns(Some(bar), b"same", None).unwrap();
        let foo_active = e
            .lookup_or_create_ns(Some(foo), b"same", Some(b'~'))
            .unwrap();

        let mut dumped = Vec::new();
        e.control_sequences.dump(&mut dumped).unwrap();
        let dumped = String::from_utf8(dumped).unwrap();
        let mut lines = dumped.lines();
        let loaded = ControlSequenceStore::undump(&mut lines).unwrap();

        let id = |cs| match cs {
            ControlSequence::Escaped(id) => id,
            _ => panic!("escaped control sequence expected"),
        };
        assert_eq!(loaded.id_lookup(b"same"), Some(id(global)));
        assert_eq!(
            loaded.id_lookup_ns(Some(foo), false, b"same"),
            Some(id(foo_normal))
        );
        assert_eq!(
            loaded.id_lookup_ns(Some(bar), false, b"same"),
            Some(id(bar_normal))
        );
        assert_eq!(
            loaded.id_lookup_ns(Some(foo), true, b"same"),
            Some(id(foo_active))
        );
    }

    // **群を抜けたら戻ること**は Phase 3 で TeX を走らせて確かめる——
    // `Logger` を組み立てるより、`\namespace` を書いた方が早い
}

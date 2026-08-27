use super::{CatCode, Eqtb, FontIndex, KCatCode, Variable, VariableLevels};
use crate::command::{Command, ExpandableCommand, MacroCall};
use crate::format::{Dumpable, FormatError};
use crate::macros::Macro;
use crate::print::Printer;
use crate::token::{print_uptex_code_point, push_uptex_utf8};

use crate::fx_hash::FxHashMap;
use std::collections::HashMap;
use std::io::Write;

type CommandStoreEntry = (Command, Vec<u8>);
pub type ControlSequenceId = u16;
const MAX_UNICODE_CODE_POINT: u32 = 0x10_FFFF;

/// 制御綴名を構成する一単位。
///
/// `Byte` は従来の8 bit TeX経路、`Unicode` はupTeXのUTF-8入力経路から
/// 来た文字である。表示bytesが同じでも、この種別が違えば別の制御綴である。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlSequenceNameUnit {
    Byte(u8),
    Unicode(u32),
}

impl ControlSequenceNameUnit {
    fn is_valid(self) -> bool {
        match self {
            Self::Byte(_) => true,
            // upTeXの入力はsurrogateも文字コードとして運ぶ。
            Self::Unicode(code_point) => code_point <= MAX_UNICODE_CODE_POINT,
        }
    }
}

impl Dumpable for ControlSequenceNameUnit {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Byte(byte) => {
                writeln!(target, "Byte")?;
                byte.dump(target)?;
            }
            Self::Unicode(code_point) => {
                if *code_point > MAX_UNICODE_CODE_POINT {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "control sequence Unicode unit is out of range",
                    ));
                }
                writeln!(target, "Unicode")?;
                code_point.dump(target)?;
            }
        }
        Ok(())
    }

    fn undump<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, FormatError> {
        match lines.next().ok_or(FormatError::IncompleteFile)? {
            "Byte" => Ok(Self::Byte(u8::undump(lines)?)),
            "Unicode" => {
                let code_point = u32::undump(lines)?;
                if code_point <= MAX_UNICODE_CODE_POINT {
                    Ok(Self::Unicode(code_point))
                } else {
                    Err(FormatError::ParseError)
                }
            }
            _ => Err(FormatError::ParseError),
        }
    }
}

fn is_valid_wide_name(name: &[ControlSequenceNameUnit]) -> bool {
    name.iter().copied().all(ControlSequenceNameUnit::is_valid)
        && name
            .iter()
            .any(|unit| matches!(unit, ControlSequenceNameUnit::Unicode(_)))
}

fn wide_name_to_display_bytes(name: &[ControlSequenceNameUnit]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(name.len());
    for unit in name {
        match *unit {
            ControlSequenceNameUnit::Byte(byte) => bytes.push(byte),
            ControlSequenceNameUnit::Unicode(code_point) => push_uptex_utf8(code_point, &mut bytes),
        }
    }
    bytes
}

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
    normal: FxHashMap<Vec<u8>, ControlSequenceId>,
    /// `Unicode`単位を一つ以上含む名前。byte名とはidentityを共有しない。
    /// byte engineと通常の名前空間はmap本体を持たない。
    wide: Option<Box<FxHashMap<Vec<ControlSequenceNameUnit>, ControlSequenceId>>>,
    /// Unicode活性文字。通常の一文字制御記号とは同じ符号位置でも別identity。
    wide_active: Option<Box<FxHashMap<Vec<ControlSequenceNameUnit>, ControlSequenceId>>>,
    active: FxHashMap<Vec<u8>, ControlSequenceId>,
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

    fn insert_checked(&mut self, active: bool, key: Vec<u8>, id: ControlSequenceId) -> bool {
        let hash = if active {
            &mut self.active
        } else {
            &mut self.normal
        };
        hash.insert(key, id).is_none()
    }

    fn get_wide(&self, active: bool, key: &[ControlSequenceNameUnit]) -> Option<ControlSequenceId> {
        let hash = if active {
            self.wide_active.as_deref()?
        } else {
            self.wide.as_deref()?
        };
        hash.get(key).copied()
    }

    fn insert_wide(
        &mut self,
        active: bool,
        key: Vec<ControlSequenceNameUnit>,
        id: ControlSequenceId,
    ) -> Option<ControlSequenceId> {
        let hash = if active {
            &mut self.wide_active
        } else {
            &mut self.wide
        };
        hash.get_or_insert_with(|| Box::new(FxHashMap::default()))
            .insert(key, id)
    }

    fn len(&self) -> usize {
        self.normal.len() + self.active.len()
    }

    fn wide_len(&self) -> usize {
        self.wide.as_deref().map_or(0, FxHashMap::len)
            + self.wide_active.as_deref().map_or(0, FxHashMap::len)
    }
}

/// global名は最終表へ直接入れ、namespace数がまだ読めない名前だけを疎な中間表へ置く。
/// 通常のLaTeX fmtで同じ名前を二度hashする必要をなくしつつ、不正namespaceから
/// 大量の空`ControlSequenceHash`を先に作らない。
#[derive(Default)]
struct UndumpedControlSequenceHashes {
    global: ControlSequenceHash,
    namespaced: FxHashMap<(NamespaceId, bool, Vec<u8>), ControlSequenceId>,
    namespaced_wide:
        FxHashMap<(NamespaceId, bool, Vec<ControlSequenceNameUnit>), ControlSequenceId>,
    declared_entry_count: usize,
}

impl UndumpedControlSequenceHashes {
    fn accept_declared_entries(&mut self, count: usize) -> Result<(), FormatError> {
        let total = self
            .declared_entry_count
            .checked_add(count)
            .ok_or(FormatError::ParseError)?;
        if total > ControlSequenceId::MAX as usize + 1 {
            return Err(FormatError::ParseError);
        }
        self.declared_entry_count = total;
        Ok(())
    }

    fn insert(
        &mut self,
        namespace: Option<NamespaceId>,
        active: bool,
        key: Vec<u8>,
        id: ControlSequenceId,
    ) -> Result<(), FormatError> {
        let inserted = match namespace {
            None => self.global.insert_checked(active, key, id),
            Some(namespace) => self
                .namespaced
                .insert((namespace, active, key), id)
                .is_none(),
        };
        if inserted {
            Ok(())
        } else {
            Err(FormatError::ParseError)
        }
    }

    fn insert_wide(
        &mut self,
        namespace: Option<NamespaceId>,
        active: bool,
        key: Vec<ControlSequenceNameUnit>,
        id: ControlSequenceId,
    ) -> Result<(), FormatError> {
        let inserted = match namespace {
            None => self.global.insert_wide(active, key, id).is_none(),
            Some(namespace) => self
                .namespaced_wide
                .insert((namespace, active, key), id)
                .is_none(),
        };
        if inserted {
            Ok(())
        } else {
            Err(FormatError::ParseError)
        }
    }

    fn into_final(
        self,
        namespace_count: usize,
    ) -> Result<(ControlSequenceHash, Vec<ControlSequenceHash>), FormatError> {
        let mut namespace_hashes = (0..namespace_count)
            .map(|_| ControlSequenceHash::default())
            .collect::<Vec<_>>();
        for ((namespace, active, key), id) in self.namespaced {
            let target = namespace_hashes
                .get_mut(namespace as usize)
                .ok_or(FormatError::ParseError)?;
            if !target.insert_checked(active, key, id) {
                return Err(FormatError::ParseError);
            }
        }
        for ((namespace, active, key), id) in self.namespaced_wide {
            let target = namespace_hashes
                .get_mut(namespace as usize)
                .ok_or(FormatError::ParseError)?;
            if target.insert_wide(active, key, id).is_some() {
                return Err(FormatError::ParseError);
            }
        }
        Ok((self.global, namespace_hashes))
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
) -> Option<ControlSequenceNameUnit> {
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
    if !matches!(cs, ControlSequence::Escaped(_)) {
        return None;
    }
    eqtb.control_sequences
        .active_char(cs)
        .map(ControlSequenceNameUnit::Byte)
        .or_else(|| {
            eqtb.control_sequences
                .wide_active_char(cs)
                .map(ControlSequenceNameUnit::Unicode)
        })
}

fn print_active_unit(unit: ControlSequenceNameUnit, printer: &mut impl Printer) {
    match unit {
        ControlSequenceNameUnit::Byte(byte) => printer.print(byte),
        ControlSequenceNameUnit::Unicode(code_point) => print_uptex_code_point(code_point, printer),
    }
}

fn print_escaped_name(cs: ControlSequence, eqtb: &Eqtb, printer: &mut impl Printer) {
    let Some(name) = eqtb.control_sequences.wide_name(cs) else {
        printer.print_esc_str(eqtb.control_sequences.text(cs));
        return;
    };
    if let Some(escape) = printer.current_escape_character() {
        printer.print(escape);
    }
    for unit in name {
        match *unit {
            ControlSequenceNameUnit::Byte(byte) => printer.print(byte),
            ControlSequenceNameUnit::Unicode(code_point) => {
                print_uptex_code_point(code_point, printer)
            }
        }
    }
}

fn wide_name_needs_separator(name: &[ControlSequenceNameUnit], eqtb: &Eqtb) -> bool {
    match name {
        // A multi-unit name is printed as a control word regardless of the
        // current categories of its individual units.
        [_, _, ..] => true,
        [ControlSequenceNameUnit::Unicode(code_point)] => !matches!(
            eqtb.kcat_code(*code_point),
            KCatCode::LatinUcs | KCatCode::OtherKChar
        ),
        _ => false,
    }
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
                    Some(unit) => print_active_unit(unit, printer),
                    None => print_escaped_name(self, eqtb, printer),
                }
                if active.is_some()
                    || eqtb
                        .control_sequences
                        .wide_name(self)
                        .map_or(true, |name| wide_name_needs_separator(name, eqtb))
                {
                    printer.print_char(b' ');
                }
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
                if let Some(unit) = active {
                    print_active_unit(unit, printer);
                    return;
                }
                print_escaped_name(self, eqtb, printer);
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
    /// typed名だけを番号から引く疎な逆表。表示と一文字定数で使う。
    /// byte engineではmap本体を確保しない。
    wide_names: Option<Box<FxHashMap<ControlSequenceId, Vec<ControlSequenceNameUnit>>>>,
    /// `escaped` と並ぶ。**どの名前空間の出自か。** `None` が global
    escaped_ns: Vec<Option<NamespaceId>>,
    /// `escaped` と並ぶ。**名前空間つきの active char なら、その文字。**
    ///
    /// 名前空間つきの active char も同じ番号空間に載せる——
    /// そうすると save stack も群も `\global` も**そのまま付いてくる**
    escaped_active: Vec<Option<u8>>,
    /// `escaped` と並ぶ。Unicode活性文字なら、その符号位置。
    escaped_wide_active: Vec<Option<u32>>,
    /// 名前空間の名前。番号が添字
    namespaces: Vec<Vec<u8>>,
    ns_index: FxHashMap<Vec<u8>, NamespaceId>,
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
            escaped_wide_active: Vec::new(),
            namespaces: Vec::new(),
            ns_index: FxHashMap::default(),
            null_cs: (
                Command::Expandable(ExpandableCommand::Undefined),
                Vec::new(),
            ),

            global_hash: ControlSequenceHash::default(),
            namespace_hashes: Vec::new(),
            escaped: Vec::new(),
            wide_names: None,
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

    /// Unicode単位を含む制御綴をglobalから引く。
    pub fn id_lookup_wide(&self, key: &[ControlSequenceNameUnit]) -> Option<ControlSequenceId> {
        self.id_lookup_ns_wide(None, false, key)
    }

    /// Unicode活性文字をglobalから引く。
    pub fn id_lookup_wide_active(
        &self,
        key: &[ControlSequenceNameUnit],
    ) -> Option<ControlSequenceId> {
        self.id_lookup_ns_wide(None, true, key)
    }

    /// Unicode単位を含む制御綴を名前空間つきで引く。`None` はglobal。
    pub fn id_lookup_ns_wide(
        &self,
        ns: Option<NamespaceId>,
        active: bool,
        key: &[ControlSequenceNameUnit],
    ) -> Option<ControlSequenceId> {
        if !is_valid_wide_name(key) {
            return None;
        }
        match ns {
            None => self.global_hash.get_wide(active, key),
            Some(ns) => self
                .namespace_hashes
                .get(ns as usize)?
                .get_wide(active, key),
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
        self.namespaces
            .get(id as usize)
            .map(|v| &v[..])
            .unwrap_or(b"")
    }

    /// **この制御綴はどの名前空間のものか。** global なら `None`。
    pub fn namespace_of(&self, cs: ControlSequence) -> Option<NamespaceId> {
        match cs {
            ControlSequence::Escaped(n) => self.escaped_ns.get(n as usize).copied().flatten(),
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
            ControlSequence::Escaped(n) => self.escaped_active.get(n as usize).copied().flatten(),
            _ => None,
        }
    }

    /// Unicode活性文字なら、その符号位置を返す。
    pub fn wide_active_char(&self, cs: ControlSequence) -> Option<u32> {
        let ControlSequence::Escaped(n) = cs else {
            return None;
        };
        self.escaped_wide_active.get(n as usize).copied().flatten()
    }

    /// Unicode単位を含む元の制御綴名を返す。
    pub fn wide_name(&self, cs: ControlSequence) -> Option<&[ControlSequenceNameUnit]> {
        let ControlSequence::Escaped(id) = cs else {
            return None;
        };
        self.wide_names.as_deref()?.get(&id).map(Vec::as_slice)
    }

    /// alphabetic constantとして使える一文字wide制御綴なら符号位置を返す。
    pub fn single_wide_code_point(&self, cs: ControlSequence) -> Option<u32> {
        match self.wide_name(cs)? {
            [ControlSequenceNameUnit::Unicode(code_point)] => Some(*code_point),
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
        self.escaped_wide_active.push(None);
        // NOTE: We need to extend the memory slots for levels as well.
        variable_levels.add_new_escaped_command();
        Ok(n)
    }

    /// Unicode単位を含む制御綴をglobalに作る。
    pub fn add_wide_command(
        &mut self,
        key: &[ControlSequenceNameUnit],
        variable_levels: &mut VariableLevels,
    ) -> Result<ControlSequenceId, ()> {
        self.add_wide_command_ns(None, key, None, variable_levels)
    }

    /// Unicode活性文字をglobalに作る。
    pub fn add_wide_active_command(
        &mut self,
        code_point: u32,
        variable_levels: &mut VariableLevels,
    ) -> Result<ControlSequenceId, ()> {
        let key = [ControlSequenceNameUnit::Unicode(code_point)];
        self.add_wide_command_ns(None, &key, Some(code_point), variable_levels)
    }

    /// Unicode単位を含む制御綴を名前空間つきで作る。
    pub fn add_wide_command_ns(
        &mut self,
        ns: Option<NamespaceId>,
        key: &[ControlSequenceNameUnit],
        active: Option<u32>,
        variable_levels: &mut VariableLevels,
    ) -> Result<ControlSequenceId, ()> {
        if !is_valid_wide_name(key)
            || ns.is_some_and(|ns| ns as usize >= self.namespace_hashes.len())
            || active
                .is_some_and(|code_point| key != [ControlSequenceNameUnit::Unicode(code_point)])
        {
            return Err(());
        }
        let hash = match ns {
            None => &self.global_hash,
            Some(ns) => &self.namespace_hashes[ns as usize],
        };
        if hash.get_wide(active.is_some(), key).is_some() {
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
        let previous = hash.insert_wide(active.is_some(), key.to_vec(), n);
        debug_assert!(previous.is_none());
        let previous = self
            .wide_names
            .get_or_insert_with(|| Box::new(FxHashMap::default()))
            .insert(n, key.to_vec());
        debug_assert!(previous.is_none());
        let cmd = Command::Expandable(ExpandableCommand::Undefined);
        self.escaped.push((cmd, wide_name_to_display_bytes(key)));
        self.escaped_ns.push(ns);
        self.escaped_active.push(None);
        self.escaped_wide_active.push(active);
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
        hash: &FxHashMap<Vec<u8>, ControlSequenceId>,
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

    fn undump_hash<'a>(
        lines: &mut impl Iterator<Item = &'a str>,
        hashes: &mut UndumpedControlSequenceHashes,
    ) -> Result<(), FormatError> {
        let count = usize::undump(lines)?;
        hashes.accept_declared_entries(count)?;
        for _ in 0..count {
            let ns = Option::<NamespaceId>::undump(lines)?;
            let active = bool::undump(lines)?;
            let key = Vec::<u8>::undump(lines)?;
            let id = ControlSequenceId::undump(lines)?;
            hashes.insert(ns, active, key, id)?;
        }
        Ok(())
    }

    /// typed nameは従来のbyte hashと混ぜず、独立したblockへ保存する。
    fn dump_wide_hash(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        let hash_len = self.global_hash.wide_len()
            + self
                .namespace_hashes
                .iter()
                .map(ControlSequenceHash::wide_len)
                .sum::<usize>();
        writeln!(target, "{hash_len}")?;
        if let Some(hash) = self.global_hash.wide.as_deref() {
            Self::dump_wide_hash_entries(None, false, hash, target)?;
        }
        if let Some(hash) = self.global_hash.wide_active.as_deref() {
            Self::dump_wide_hash_entries(None, true, hash, target)?;
        }
        for (ns, hash) in self.namespace_hashes.iter().enumerate() {
            if let Some(hash) = hash.wide.as_deref() {
                Self::dump_wide_hash_entries(Some(ns as NamespaceId), false, hash, target)?;
            }
            if let Some(hash) = hash.wide_active.as_deref() {
                Self::dump_wide_hash_entries(Some(ns as NamespaceId), true, hash, target)?;
            }
        }
        Ok(())
    }

    fn dump_wide_hash_entries(
        ns: Option<NamespaceId>,
        active: bool,
        hash: &FxHashMap<Vec<ControlSequenceNameUnit>, ControlSequenceId>,
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

    fn undump_wide_hash<'a>(
        lines: &mut impl Iterator<Item = &'a str>,
        hashes: &mut UndumpedControlSequenceHashes,
    ) -> Result<(), FormatError> {
        let count = usize::undump(lines)?;
        hashes.accept_declared_entries(count)?;
        for _ in 0..count {
            let ns = Option::<NamespaceId>::undump(lines)?;
            let active = bool::undump(lines)?;
            let key = Vec::<ControlSequenceNameUnit>::undump(lines)?;
            let id = ControlSequenceId::undump(lines)?;
            if !is_valid_wide_name(&key)
                || active && !matches!(key.as_slice(), [ControlSequenceNameUnit::Unicode(_)])
            {
                return Err(FormatError::ParseError);
            }
            hashes.insert_wide(ns, active, key, id)?;
        }
        Ok(())
    }

    fn validate_undumped_byte_entries(
        namespace: Option<NamespaceId>,
        active: bool,
        entries: &FxHashMap<Vec<u8>, ControlSequenceId>,
        escaped: &[CommandStoreEntry],
        escaped_ns: &[Option<NamespaceId>],
        escaped_active: &[Option<u8>],
        escaped_wide_active: &[Option<u32>],
        seen_hash_ids: &mut [bool],
    ) -> Result<(), FormatError> {
        for (key, &id) in entries {
            let id = id as usize;
            if id >= escaped.len()
                || seen_hash_ids[id]
                || escaped_ns[id] != namespace
                || escaped_active[id].is_some() != active
                || escaped_wide_active[id].is_some()
                || escaped[id].1.as_slice() != key.as_slice()
            {
                return Err(FormatError::ParseError);
            }
            seen_hash_ids[id] = true;
        }
        Ok(())
    }

    fn validate_undumped_wide_entries(
        namespace: Option<NamespaceId>,
        active: bool,
        entries: &FxHashMap<Vec<ControlSequenceNameUnit>, ControlSequenceId>,
        escaped: &[CommandStoreEntry],
        escaped_ns: &[Option<NamespaceId>],
        escaped_active: &[Option<u8>],
        escaped_wide_active: &[Option<u32>],
        seen_hash_ids: &mut [bool],
        wide_names: &mut Option<Box<FxHashMap<ControlSequenceId, Vec<ControlSequenceNameUnit>>>>,
    ) -> Result<(), FormatError> {
        for (key, &id) in entries {
            let position = id as usize;
            let active_code_point = match key.as_slice() {
                [ControlSequenceNameUnit::Unicode(code_point)] if active => Some(*code_point),
                _ => None,
            };
            if position >= escaped.len()
                || seen_hash_ids[position]
                || escaped_ns[position] != namespace
                || escaped_active[position].is_some()
                || escaped_wide_active[position] != active_code_point
                || escaped[position].1 != wide_name_to_display_bytes(key)
            {
                return Err(FormatError::ParseError);
            }
            if wide_names
                .get_or_insert_with(|| Box::new(FxHashMap::default()))
                .insert(id, key.clone())
                .is_some()
            {
                return Err(FormatError::ParseError);
            }
            seen_hash_ids[position] = true;
        }
        Ok(())
    }

    fn validate_undumped_hash(
        namespace: Option<NamespaceId>,
        hash: &ControlSequenceHash,
        escaped: &[CommandStoreEntry],
        escaped_ns: &[Option<NamespaceId>],
        escaped_active: &[Option<u8>],
        escaped_wide_active: &[Option<u32>],
        seen_hash_ids: &mut [bool],
        wide_names: &mut Option<Box<FxHashMap<ControlSequenceId, Vec<ControlSequenceNameUnit>>>>,
    ) -> Result<(), FormatError> {
        Self::validate_undumped_byte_entries(
            namespace,
            false,
            &hash.normal,
            escaped,
            escaped_ns,
            escaped_active,
            escaped_wide_active,
            seen_hash_ids,
        )?;
        Self::validate_undumped_byte_entries(
            namespace,
            true,
            &hash.active,
            escaped,
            escaped_ns,
            escaped_active,
            escaped_wide_active,
            seen_hash_ids,
        )?;
        if let Some(entries) = hash.wide.as_deref() {
            Self::validate_undumped_wide_entries(
                namespace,
                false,
                entries,
                escaped,
                escaped_ns,
                escaped_active,
                escaped_wide_active,
                seen_hash_ids,
                wide_names,
            )?;
        }
        if let Some(entries) = hash.wide_active.as_deref() {
            Self::validate_undumped_wide_entries(
                namespace,
                true,
                entries,
                escaped,
                escaped_ns,
                escaped_active,
                escaped_wide_active,
                seen_hash_ids,
                wide_names,
            )?;
        }
        Ok(())
    }

    fn rebuild_namespace_index(
        namespaces: &[Vec<u8>],
    ) -> Result<FxHashMap<Vec<u8>, NamespaceId>, FormatError> {
        if namespaces.len() > usize::from(NamespaceId::MAX) + 1 {
            return Err(FormatError::ParseError);
        }
        let mut index = FxHashMap::default();
        for (id, name) in namespaces.iter().enumerate() {
            if index.insert(name.clone(), id as NamespaceId).is_some() {
                return Err(FormatError::ParseError);
            }
        }
        Ok(index)
    }
}

impl Dumpable for ControlSequenceStore {
    fn dump(&self, target: &mut impl Write) -> Result<(), std::io::Error> {
        self.active.dump(target)?;
        self.single.dump(target)?;
        self.null_cs.dump(target)?;
        self.dump_hash(target)?;
        self.dump_wide_hash(target)?;
        self.escaped.dump(target)?;
        self.escaped_ns.dump(target)?;
        self.escaped_active.dump(target)?;
        self.escaped_wide_active.dump(target)?;
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
        if active.len() != 256 || single.len() != 256 {
            return Err(FormatError::ParseError);
        }
        let null_cs = CommandStoreEntry::undump(lines)?;
        let mut undumped_hashes = UndumpedControlSequenceHashes::default();
        Self::undump_hash(lines, &mut undumped_hashes)?;
        Self::undump_wide_hash(lines, &mut undumped_hashes)?;
        let escaped: Vec<CommandStoreEntry> = Vec::undump(lines)?;
        let escaped_ns: Vec<Option<NamespaceId>> = Vec::undump(lines)?;
        let escaped_active: Vec<Option<u8>> = Vec::undump(lines)?;
        let escaped_wide_active: Vec<Option<u32>> = Vec::undump(lines)?;
        let namespaces: Vec<Vec<u8>> = Vec::undump(lines)?;
        // **番号から名前を引く表は書き出す。逆は組み直す**——写す意味が無い。
        // 同名slotを許すと番号からは見えるのに名前から到達できない制御綴が残る。
        let ns_index = Self::rebuild_namespace_index(&namespaces)?;
        let (global_hash, namespace_hashes) = undumped_hashes.into_final(namespaces.len())?;
        if escaped_ns.len() != escaped.len()
            || escaped_active.len() != escaped.len()
            || escaped_wide_active.len() != escaped.len()
        {
            return Err(FormatError::ParseError);
        }
        let mut seen_hash_ids = vec![false; escaped.len()];
        let mut wide_names: Option<Box<FxHashMap<ControlSequenceId, Vec<ControlSequenceNameUnit>>>> =
            None;
        Self::validate_undumped_hash(
            None,
            &global_hash,
            &escaped,
            &escaped_ns,
            &escaped_active,
            &escaped_wide_active,
            &mut seen_hash_ids,
            &mut wide_names,
        )?;
        for (namespace, hash) in namespace_hashes.iter().enumerate() {
            Self::validate_undumped_hash(
                Some(namespace as NamespaceId),
                hash,
                &escaped,
                &escaped_ns,
                &escaped_active,
                &escaped_wide_active,
                &mut seen_hash_ids,
                &mut wide_names,
            )?;
        }
        if seen_hash_ids.iter().any(|seen| !seen) {
            return Err(FormatError::ParseError);
        }
        let cs_count = usize::undump(lines)?;
        if cs_count != escaped.len() || cs_count > ControlSequenceId::MAX as usize + 1 {
            return Err(FormatError::ParseError);
        }
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
        if font_id.len() != FontIndex::MAX as usize + 1 {
            return Err(FormatError::ParseError);
        }
        let undefined = CommandStoreEntry::undump(lines)?;
        Ok(Self {
            active,
            single,
            null_cs,
            global_hash,
            namespace_hashes,
            escaped,
            wide_names,
            escaped_ns,
            escaped_active,
            escaped_wide_active,
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

    #[test]
    fn 固定幅制御綴表の短いformatを拒否する() {
        for store in {
            let mut active = ControlSequenceStore::new();
            active.active.pop();
            let mut single = ControlSequenceStore::new();
            single.single.pop();
            let mut font_id = ControlSequenceStore::new();
            font_id.font_id.pop();
            [active, single, font_id]
        } {
            let mut bytes = Vec::new();
            store.dump(&mut bytes).unwrap();
            let input = String::from_utf8(bytes).unwrap();
            assert!(matches!(
                ControlSequenceStore::undump(&mut input.lines()),
                Err(FormatError::ParseError)
            ));
        }
    }
}

#[cfg(test)]
mod namespace_tests {
    use super::*;
    use crate::eqtb::Eqtb;

    fn e() -> Eqtb {
        Eqtb::new()
    }

    fn escaped_id(cs: ControlSequence) -> ControlSequenceId {
        match cs {
            ControlSequence::Escaped(id) => id,
            _ => panic!("escaped control sequence expected"),
        }
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
        assert_eq!(
            e.lookup_or_create(b"x").unwrap(),
            ControlSequence::Single(b'x')
        );
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
            e.control_sequences
                .active_char(ControlSequence::Active(b'~')),
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

        assert_eq!(loaded.id_lookup(b"same"), Some(escaped_id(global)));
        assert_eq!(
            loaded.id_lookup_ns(Some(foo), false, b"same"),
            Some(escaped_id(foo_normal))
        );
        assert_eq!(
            loaded.id_lookup_ns(Some(bar), false, b"same"),
            Some(escaped_id(bar_normal))
        );
        assert_eq!(
            loaded.id_lookup_ns(Some(foo), true, b"same"),
            Some(escaped_id(foo_active))
        );
    }

    #[test]
    fn 表示bytesが同じbyte名とunicode名を区別する() {
        let mut e = e();
        let byte_name = e.lookup_or_create(&[0xE3, 0x81, 0x82]).unwrap();
        let unicode_name = e
            .lookup_or_create_wide(&[ControlSequenceNameUnit::Unicode(0x3042)])
            .unwrap();

        assert_ne!(byte_name, unicode_name);
        assert_eq!(e.control_sequences.text(byte_name), [0xE3, 0x81, 0x82]);
        assert_eq!(e.control_sequences.text(unicode_name), [0xE3, 0x81, 0x82]);
        assert_eq!(e.lookup(&[0xE3, 0x81, 0x82]), Some(byte_name));
        assert_eq!(
            e.lookup_wide(&[ControlSequenceNameUnit::Unicode(0x3042)]),
            Some(unicode_name)
        );
    }

    #[test]
    fn unicode名のidentityはcategoryを含まない() {
        // 字句解析時のcategoryが違ってもstoreへ渡すkeyは同じcode point列である。
        let mut e = e();
        let letter = [ControlSequenceNameUnit::Unicode(0x3042)];
        let other = [ControlSequenceNameUnit::Unicode(0x3042)];
        let first = e.lookup_or_create_wide(&letter).unwrap();
        let second = e.lookup_or_create_wide(&other).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn unicode名も名前空間ごとに区別する() {
        let mut e = e();
        let foo = e.control_sequences.intern_namespace(b"foo");
        let bar = e.control_sequences.intern_namespace(b"bar");
        let name = [
            ControlSequenceNameUnit::Byte(b'x'),
            ControlSequenceNameUnit::Unicode(0x3042),
        ];
        let global = e.lookup_or_create_wide(&name).unwrap();
        let in_foo = e.lookup_or_create_ns_wide(Some(foo), &name).unwrap();
        let in_bar = e.lookup_or_create_ns_wide(Some(bar), &name).unwrap();

        assert_ne!(global, in_foo);
        assert_ne!(in_foo, in_bar);
        assert_eq!(e.lookup_ns_wide(None, &name), Some(global));
        assert_eq!(e.lookup_ns_wide(Some(foo), &name), Some(in_foo));
        assert_eq!(e.lookup_ns_wide(Some(bar), &name), Some(in_bar));
    }

    #[test]
    fn unicode活性文字と一文字制御記号はfmt往復後も別identityを保つ() {
        use crate::print::string::StringPrinter;

        let mut e = e();
        let name = [ControlSequenceNameUnit::Unicode(0x00DF)];
        let symbol = e.lookup_or_create_wide(&name).unwrap();
        let active = e.lookup_or_create_wide_active(0x00DF).unwrap();

        assert_ne!(symbol, active);
        assert_eq!(e.lookup_wide(&name), Some(symbol));
        assert_eq!(e.lookup_wide_active(0x00DF), Some(active));
        assert_eq!(e.control_sequences.wide_active_char(symbol), None);
        assert_eq!(e.control_sequences.wide_active_char(active), Some(0x00DF));

        let mut symbol_printer = StringPrinter::new(Some(b'\\'));
        symbol.sprint_cs(&e, &mut symbol_printer);
        assert_eq!(symbol_printer.into_string(), "\\ß".as_bytes());
        let mut active_printer = StringPrinter::new(Some(b'\\'));
        active.sprint_cs(&e, &mut active_printer);
        assert_eq!(active_printer.into_string(), "ß".as_bytes());

        let mut dumped = Vec::new();
        e.control_sequences.dump(&mut dumped).unwrap();
        let dumped = String::from_utf8(dumped).unwrap();
        let loaded = ControlSequenceStore::undump(&mut dumped.lines()).unwrap();
        assert_eq!(loaded.id_lookup_wide(&name), Some(escaped_id(symbol)));
        assert_eq!(
            loaded.id_lookup_wide_active(&name),
            Some(escaped_id(active))
        );
        assert_eq!(loaded.wide_active_char(active), Some(0x00DF));
    }

    #[test]
    fn unicode名はfmt往復でsurrogateを含むidentityを保つ() {
        let mut e = e();
        let foo = e.control_sequences.intern_namespace(b"foo");
        let global_name = [ControlSequenceNameUnit::Unicode(0x3042)];
        let namespace_name = [
            ControlSequenceNameUnit::Byte(b'x'),
            ControlSequenceNameUnit::Unicode(0xD800),
        ];
        let global = e.lookup_or_create_wide(&global_name).unwrap();
        let namespaced = e
            .lookup_or_create_ns_wide(Some(foo), &namespace_name)
            .unwrap();
        assert_eq!(
            e.control_sequences.single_wide_code_point(global),
            Some(0x3042)
        );
        assert_eq!(e.control_sequences.single_wide_code_point(namespaced), None);

        let mut dumped = Vec::new();
        e.control_sequences.dump(&mut dumped).unwrap();
        let dumped = String::from_utf8(dumped).unwrap();
        let loaded = ControlSequenceStore::undump(&mut dumped.lines()).unwrap();

        assert_eq!(
            loaded.id_lookup_wide(&global_name),
            Some(escaped_id(global))
        );
        assert_eq!(
            loaded.id_lookup_ns_wide(Some(foo), false, &namespace_name),
            Some(escaped_id(namespaced))
        );
        assert_eq!(
            loaded.text(ControlSequence::Escaped(escaped_id(namespaced))),
            [b'x', 0xED, 0xA0, 0x80]
        );
        assert_eq!(loaded.single_wide_code_point(global), Some(0x3042));
        assert_eq!(loaded.single_wide_code_point(namespaced), None);
    }

    #[test]
    fn wide_apiはbyteだけの名前と範囲外unicodeを拒む() {
        let mut e = e();
        let boundaries = e
            .lookup_or_create_wide(&[
                ControlSequenceNameUnit::Unicode(0),
                ControlSequenceNameUnit::Unicode(0x10_FFFF),
            ])
            .unwrap();
        assert_eq!(
            e.control_sequences.text(boundaries),
            [0, 0xF4, 0x8F, 0xBF, 0xBF]
        );
        assert!(e
            .lookup_or_create_wide(&[ControlSequenceNameUnit::Byte(b'x')])
            .is_err());
        assert!(e
            .lookup_wide(&[ControlSequenceNameUnit::Byte(b'x')])
            .is_none());
        assert!(e
            .lookup_or_create_wide(&[ControlSequenceNameUnit::Unicode(0x11_0000)])
            .is_err());
        assert!(e
            .lookup_wide(&[ControlSequenceNameUnit::Unicode(0x11_0000)])
            .is_none());
        assert!(ControlSequenceNameUnit::Unicode(0x11_0000)
            .dump(&mut Vec::new())
            .is_err());
    }

    #[test]
    fn 壊れたunicode名を含むfmtを拒む() {
        let mut e = e();
        e.lookup_or_create_wide(&[ControlSequenceNameUnit::Unicode(0x3042)])
            .unwrap();
        let mut dumped = Vec::new();
        e.control_sequences.dump(&mut dumped).unwrap();
        let dumped = String::from_utf8(dumped).unwrap();

        let out_of_range = dumped.replacen("Unicode\n12354\n", "Unicode\n1114112\n", 1);
        assert!(matches!(
            ControlSequenceStore::undump(&mut out_of_range.lines()),
            Err(FormatError::ParseError)
        ));

        // hash側だけを別の有効なcode pointへ変えても、sidecarとの不一致で拒む。
        let mismatched = dumped.replacen("Unicode\n12354\n", "Unicode\n12355\n", 1);
        assert!(matches!(
            ControlSequenceStore::undump(&mut mismatched.lines()),
            Err(FormatError::ParseError)
        ));

        assert!(matches!(
            ControlSequenceNameUnit::undump(&mut "Unicode".lines()),
            Err(FormatError::IncompleteFile)
        ));
    }

    #[test]
    fn 壊れたfmtの重複制御綴索引を拒む() {
        let duplicate = "2\nNone\nfalse\n1\n120\n0\nNone\nfalse\n1\n120\n1\n";
        let mut hashes = UndumpedControlSequenceHashes::default();
        assert!(matches!(
            ControlSequenceStore::undump_hash(&mut duplicate.lines(), &mut hashes),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 壊れたfmtの名前空間つき重複制御綴索引を拒む() {
        let duplicate = "2\nSome\n0\nfalse\n1\n120\n0\nSome\n0\nfalse\n1\n120\n1\n";
        let mut hashes = UndumpedControlSequenceHashes::default();
        assert!(matches!(
            ControlSequenceStore::undump_hash(&mut duplicate.lines(), &mut hashes),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 壊れたfmtのwide重複制御綴索引を全domainで拒む() {
        for (label, namespace, active) in [
            ("global通常", "None\n", false),
            ("global活性", "None\n", true),
            ("名前空間通常", "Some\n0\n", false),
            ("名前空間活性", "Some\n0\n", true),
        ] {
            let entry = format!("{namespace}{active}\n1\nUnicode\n12354\n");
            let duplicate = format!("2\n{entry}0\n{entry}1\n");
            let mut hashes = UndumpedControlSequenceHashes::default();
            assert!(
                matches!(
                    ControlSequenceStore::undump_wide_hash(&mut duplicate.lines(), &mut hashes),
                    Err(FormatError::ParseError)
                ),
                "{label}"
            );
        }
    }

    #[test]
    fn hash二blockの宣言数合計が制御綴id範囲を越えれば本文前に拒む() {
        let capacity = ControlSequenceId::MAX as usize + 1;
        let too_many = format!("{}\n", capacity + 1);
        let mut hashes = UndumpedControlSequenceHashes::default();
        assert!(matches!(
            ControlSequenceStore::undump_hash(&mut too_many.lines(), &mut hashes),
            Err(FormatError::ParseError)
        ));

        let one_byte_entry = "1\nNone\nfalse\n1\n120\n0\n";
        let mut hashes = UndumpedControlSequenceHashes::default();
        ControlSequenceStore::undump_hash(&mut one_byte_entry.lines(), &mut hashes).unwrap();
        let overflowing_total = format!("{capacity}\n");
        assert!(matches!(
            ControlSequenceStore::undump_wide_hash(&mut overflowing_total.lines(), &mut hashes),
            Err(FormatError::ParseError)
        ));
    }

    #[test]
    fn 壊れたfmtの重複名前空間を拒む() {
        let names = vec![b"same".to_vec(), b"same".to_vec()];
        assert!(matches!(
            ControlSequenceStore::rebuild_namespace_index(&names),
            Err(FormatError::ParseError)
        ));
    }

    // **群を抜けたら戻ること**は Phase 3 で TeX を走らせて確かめる——
    // `Logger` を組み立てるより、`\namespace` を書いた方が早い
}

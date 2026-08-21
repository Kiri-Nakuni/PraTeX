//! `\directvaak{…}` — Vaak を走らせる。
//!
//! # 何をするか
//!
//! 1. `{…}` の中身を**一般テキストとして走査**し、バイト列にする（`\message` と同じ）
//! 2. **全ての数値レジスタへの別名を持たせて** Vaak を起動する
//! 3. 走らせる
//! 4. 変わったレジスタを**書き戻す**
//! 5. **終了コードを10進で展開する**
//!
//! # なぜ展開可能か
//!
//! **数値の走査中に呼ばれうる**からである。
//!
//! ```tex
//! \count0=\directvaak{ count[1] + count[2] }
//! ```
//!
//! 展開が空だと TeX の数値走査が壊れる。**だから必ず数字を出す**——
//! エラーが起きても `0` を出し、エラーは別に報告する。
//!
//! # 終了コード
//!
//! 最上位の外界面は**言語の意味論ではない**（Vaak の C-31）。ホストが決めてよい。
//!
//! | 最上位の外界面 | 展開されるもの |
//! |---|---|
//! | 整数の値 | その10進表記 |
//! | 中身が空（アーカーシャ・paradox） | `0`。**エラーではない** |
//! | 実行時エラー | エラーを報告して `0` |
//! | 静的エラー・構文エラー | 同上 |
//!
//! # なぜレジスタを `i32` で見せるか
//!
//! **TeX の `Integer` は `i32`。** `i64` で見せるのは嘘であり、
//! 書き戻すときに変換が要る——**変換が要らなければ、変換の誤りも起きない。**
//! 溢れは Vaak も TeX も 2^32 を法として折り返す。
//!
//! # 覚え書き（キャッシュ）
//!
//! **同じ `\directvaak{…}` が何度も呼ばれる。** マクロの中に書かれるからである。
//! 解析・検査・組み立ては**ソースが同じなら同じ結果**なので、一度だけ行う。
//!
//! 鍵はソースのバイト列。**ホストが見せる名前と型は常に同じ**（`count` と `dimen`）
//! なので、鍵に含めなくてよい。
//!
//! # なぜエラー文が英語か
//!
//! **rtex は 7bit。** 多バイト文字は `^^xx` に展開されて読めなくなる。
//! Vaak の説明（日本語）は落とし、**種別と位置だけ残す。**

use crate::command::{Command, ExpandableCommand};
use crate::eqtb::{DimensionVariable, Eqtb, IntegerVariable, RegisterIndex};
use crate::input::Scanner;
use crate::logger::Logger;
use crate::print::string::StringPrinter;
use crate::print::Printer;
use crate::token::Token;
use crate::token_lists::{str_toks, token_show};

use std::rc::Rc;
use std::sync::OnceLock;
use std::cell::RefCell;
use std::collections::HashMap;

use vaak::ast::ValueType;
use vaak::value::Value;
use vaak::vm::Program2;

/// レジスタの数。TeX82 は 256 個（`RegisterIndex = u8`）。
const N_REGS: usize = 256;

/// 組み上がったものの覚え書き。**ソースが同じなら同じ結果。**
///
/// 一つの文書で `\directvaak` は何百回も呼ばれうるが、**書かれている文字列は
/// たいてい少ない**——マクロの中に一度書かれ、そこから呼ばれるので。
/// 覚えておくもの。**組んだ結果と、どの名前を使うか。**
///
/// `host_used()` は命令列を全部見るので、**呼び出しのたびにやってはいけない。**
#[derive(Clone)]
struct Built {
    program: Program2,
    /// **どの添字を見ているか。**
    ///
    /// 「別名として見えている」ことと「実際に見ている」ことは別である——
    /// `count[5] * 2` は `count` が見えているが、**見ているのは 5 番だけ。**
    ///
    /// - `Touch::None` — 触らない
    /// - `Touch::Some(v)` — その添字だけ
    /// - `Touch::All` — 全部（動く添字、丸ごとの用途、別名で受け直し）
    count: Touch,
    dimen: Touch,
}

#[derive(Clone)]
enum Touch {
    None,
    Some(Vec<usize>),
    All,
}

impl Touch {
    fn of(p: &Program2, i: usize, used: bool) -> Self {
        if !used {
            return Touch::None;
        }
        match p.host_touched(i) {
            None => Touch::All,
            Some(v) if v.is_empty() => Touch::None,
            Some(v) => Touch::Some(
                v.into_iter().filter(|n| *n >= 0 && (*n as usize) < N_REGS).map(|n| n as usize).collect(),
            ),
        }
    }
}

/// **貸すのであって、渡すのではない。**
///
/// 組み上がったものを呼び出しのたびに複製していた。命令列も定数表も位置情報も、
/// **一度作れば二度と変わらない**のに、毎回写していた——`Rc` で貸せば済む。
type Cached = Rc<Result<Built, Vec<String>>>;

thread_local! {
    static CACHE: RefCell<HashMap<Vec<u8>, Cached>> = RefCell::new(HashMap::new());
    /// 名前の付いた本体。**番号は本体の内容で共有される。**
    ///
    /// 同じ本体に二つの名前を付ければ同じ番号になり、`\ifx` が等しいと言う。
    /// これは `\def` の意味論（本体が同じなら等しい）に合わせたものである。
    static NAMED: RefCell<(Vec<(Vec<u8>, Cached)>, HashMap<Vec<u8>, u32>)> =
        RefCell::new((Vec::new(), HashMap::new()));
    /// 見せる値の入れ物を**使い回す**。
    ///
    /// 呼び出しのたびに 512 個の `Value` を作り直すのは無駄である——
    /// **レジスタはたいてい変わっていない。**
    /// 前回の入れ物を持っておき、**変わった分だけ書き換えて**渡す。
    ///
    /// `run_program_with_host` は使い終わった入れ物を返すので、それを取っておく。
    static HOST_BUF: RefCell<Option<Vec<Value>>> = const { RefCell::new(None) };
    /// 走らせる前のレジスタを控える場所。**呼び出しごとに作り直さない。**
    ///
    /// `[i32; 256]` を二つ、呼び出しのたびに零で埋めて値で返していた——
    /// **一回あたり 4 KB を写していた**ことになる。
    /// 見ている添字しか書かないし、見ている添字しか読まないので、
    /// **残っている古い値は誰にも観測されない。**
    static BEFORE: RefCell<[[i32; N_REGS]; 2]> = const { RefCell::new([[0; N_REGS]; 2]) };
    /// 実行の入れ物。**場・積み・枠を呼び出しをまたいで使い回す。**
    static RUNNER: RefCell<vaak::vm::Runner> = RefCell::new(vaak::vm::Runner::new());
}

/// 本体を名前表に入れ、番号を返す。**同じ本体は同じ番号。**
pub fn intern(source: &[u8]) -> u32 {
    if let Some(id) = NAMED.with(|n| n.borrow().1.get(source).copied()) {
        return id;
    }
    // **組むのは表の外で。** 借りを持ったまま組み立てを走らせない
    let built = compile_cached(source);
    NAMED.with(|n| {
        let mut n = n.borrow_mut();
        let id = n.0.len() as u32;
        n.0.push((source.to_vec(), built));
        n.1.insert(source.to_vec(), id);
        id
    })
}

/// 番号から本体を引く。`\show` と書き出しに使う。
pub fn source_of(id: u32) -> Vec<u8> {
    NAMED.with(|n| n.borrow().0.get(id as usize).map(|e| e.0.clone()).unwrap_or_default())
}

/// `\vaakdef\名前{本体}` — **定義の時点で組み立てる。**
///
/// `\directvaak{…}` との違いは、**呼ぶときに本体を見ないこと**だけである。
/// 意味論は同じ——同じ本体なら同じ結果を出し、同じ終了コードを展開する。
///
/// 組み立ての誤りは**ここで報告する。** 呼ぶたびに同じ誤りを言わない。
pub fn vaak_def(global: bool, scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    let cs = crate::command::prefixable::get_r_token(scanner, eqtb, logger);
    let def_ref = scanner.scan_toks(cs, true, eqtb, logger);
    let mut printer = StringPrinter::new(eqtb.get_current_escape_character());
    token_show(&def_ref, &mut printer, eqtb);
    let source = printer.into_string();

    // **今のうちに組む。** 誤りがあれば、使われる前に分かる
    let id = intern(&source);
    let errs = NAMED.with(|n| match &*n.borrow().0[id as usize].1 {
        Err(e) => e.len(),
        Ok(_) => 0,
    });
    if errs > 0 {
        report_error(&format!("{errs} static error(s) at definition"), scanner, eqtb, logger);
    }
    let command = Command::Expandable(ExpandableCommand::VaakCall(id));
    eqtb.cs_define(cs, command, global);
}

/// `\vaakdef` で定義された名前を呼ぶ。**本体は見ない。**
pub fn vaak_call(id: u32, scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    static NULL2: OnceLock<bool> = OnceLock::new();
    if *NULL2.get_or_init(|| std::env::var_os("VAAK_NULL").is_some()) {
        let toks = str_toks(b"0");
        scanner.ins_list(toks, eqtb, logger);
        return;
    }
    // **番号で直に引く。** 本体は写さないし、鍵を作って表を引き直しもしない
    let cached = NAMED.with(|n| n.borrow().0.get(id as usize).map(|e| Rc::clone(&e.1)));
    let Some(cached) = cached else { return };
    let (code, error) = run_cached(&cached, id, eqtb, logger);
    if let Some(msg) = error {
        report_error(&msg, scanner, eqtb, logger);
    }
    let mut buf = [0u8; 12];
    let toks = str_toks(itoa(code, &mut buf));
    scanner.ins_list(toks, eqtb, logger);
}

/// 十進に直す。**確保しない**——`format!` は一回あたり一度の確保である。
fn itoa(mut n: i32, buf: &mut [u8; 12]) -> &[u8] {
    let neg = n < 0;
    let mut i = 12;
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while n != 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10).unsigned_abs() as u8;
        n /= 10;
    }
    if neg {
        i -= 1;
        buf[i] = b'-';
    }
    &buf[i..]
}

/// ホストが見せる名前と型。**常に同じ**なので鍵に含めない。
fn exposed() -> Vec<(String, ValueType)> {
    let arr = ValueType::Array(Box::new(ValueType::I32));
    vec![("count".to_string(), arr.clone()), ("dimen".to_string(), arr)]
}

/// 解析・検査・組み立て。**一度だけ行い、覚えておく。**
fn compile_cached(source: &[u8]) -> Cached {
    // **環境変数は一度だけ読む。** 呼び出しのたびに環境全体を走査する理由が無い
    static NO_CACHE: OnceLock<bool> = OnceLock::new();
    let no_cache = *NO_CACHE.get_or_init(|| std::env::var_os("VAAK_NO_CACHE").is_some());

    CACHE.with(|c| {
        if !no_cache {
            if let Some(hit) = c.borrow().get(source) {
                return Rc::clone(hit);
            }
        }
        let built = Rc::new(build(source));
        c.borrow_mut().insert(source.to_vec(), Rc::clone(&built));
        built
    })
}

fn build(source: &[u8]) -> Result<Built, Vec<String>> {
    let src = String::from_utf8_lossy(source);
    let prog = match vaak::parser::parse(&src) {
        Ok(p) => p,
        Err(e) => return Err(vec![e.msg]),
    };
    let ex = exposed();
    let mut errs: Vec<String> =
        vaak::check::check_with_host(&prog, &ex).into_iter().map(|e| e.msg).collect();
    errs.extend(vaak::types::check_types_with_host(&prog, &ex).into_iter().map(|e| e.msg));
    if !errs.is_empty() {
        return Err(errs);
    }
    let program = vaak::vm::compile_with_host(&prog, &ex).map_err(|e| vec![e.msg])?;
    // **一度だけ調べる。** 命令列を全部見るので、呼び出しのたびにはできない
    let used = program.host_used();
    Ok(Built {
        count: Touch::of(&program, 0, used.first().copied().unwrap_or(false)),
        dimen: Touch::of(&program, 1, used.get(1).copied().unwrap_or(false)),
        program,
    })
}

/// `\vaakinput 名前.vaak` — **ファイルを読んで走らせる。**
///
/// # なぜ別の命令なのか
///
/// `\directvaak{…}` は**一般テキスト**を取る（`\directlua` と同じ契約）。
/// マクロで組み立てられるし、`\edef` の中でも働く——**それは機能である。**
///
/// だが字句器を通るので、Vaak のソースとしては壊れる:
///
/// | | `\directvaak{…}` | `\vaakinput` |
/// |---|---|---|
/// | 改行 | **空白に潰れる**（`%` 行注釈が残り全部を食う） | **残る** |
/// | `%` | TeX の注釈。**閉じ括弧まで食う** | **Vaak の注釈** |
/// | `#` `$` `~` | TeX の意味で解釈される | **ただの文字** |
/// | 名前空間の印 | **本体の中で発火する** | しない |
/// | マクロで組み立てる | **できる** | できない |
///
/// LuaTeX のマニュアルは「長い Lua は別ファイルに置け」と言う。同じ分担である——
/// **ただしこちらは字句器を通さないので、本当に元のまま読める。**
///
/// # 綴り
///
/// `\input` と同じで、括弧を取らない。空白で終わる。
///
/// ```tex
/// \vaakinput setup.vaak
/// \count0=\vaakinput compute.vaak
/// ```
///
/// 拡張子が無ければ `.vaak` を足す。
pub fn vaak_input(scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    let name = scanner.scan_file_name(eqtb, logger);
    let mut path = std::path::PathBuf::from(String::from_utf8_lossy(&name).into_owned());
    if path.extension().is_none() {
        path.set_extension("vaak");
    }

    let (code, error) = match std::fs::read(&path) {
        // **生のまま渡す。** 字句器を通さないので、注釈も改行もそのまま
        Ok(src) => run_vaak(&src, eqtb, logger),
        Err(_) => (0, Some(format!("cannot read {}", path.display()))),
    };
    if let Some(msg) = error {
        report_error(&msg, scanner, eqtb, logger);
    }
    let mut buf = [0u8; 12];
    let toks = str_toks(itoa(code, &mut buf));
    scanner.ins_list(toks, eqtb, logger);
}

/// `\directvaak{…}` を実行し、終了コードを展開する。
pub fn direct_vaak(token: Token, scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    let Token::CSToken { cs } = token else {
        panic!("Impossible")
    };
    // `{…}` を一般テキストとして走査する。`\message` と同じ（See 1279.）
    let def_ref = scanner.scan_toks(cs, true, eqtb, logger);
    let mut printer = StringPrinter::new(eqtb.get_current_escape_character());
    token_show(&def_ref, &mut printer, eqtb);
    let source = printer.into_string();

    // 測定用の底。**TeX 側だけの費用**を見るために Vaak を飛ばす
    static NULL: OnceLock<bool> = OnceLock::new();
    if *NULL.get_or_init(|| std::env::var_os("VAAK_NULL").is_some()) {
        let toks = str_toks(b"0");
        scanner.ins_list(toks, eqtb, logger);
        return;
    }
    let (code, error) = run_vaak(&source, eqtb, logger);

    if let Some(msg) = error {
        report_error(&msg, scanner, eqtb, logger);
    }

    // **必ず数字を出す。** 展開が空だと TeX の数値走査が壊れる
    let mut buf = [0u8; 12];
    let toks = str_toks(itoa(code, &mut buf));
    scanner.ins_list(toks, eqtb, logger);
}

/// Vaak を走らせ、(終了コード, エラー文) を返す。
fn run_vaak(source: &[u8], eqtb: &mut Eqtb, logger: &mut Logger) -> (i32, Option<String>) {
    // **組み上がったものは覚えてある。** 二度目からは解析も検査もしない
    let cached = compile_cached(source);
    run_built(&cached, Src::Bytes(source), eqtb, logger)
}

/// 本体の在り処。**誤りを報告するときにしか要らない。**
enum Src<'a> {
    Bytes(&'a [u8]),
    /// 名前表の番号。引くのは誤りが起きたときだけ
    Named(u32),
}

impl Src<'_> {
    fn line_col(&self, offset: u32) -> (usize, usize) {
        match self {
            Src::Bytes(b) => line_col(b, offset),
            Src::Named(id) => line_col(&source_of(*id), offset),
        }
    }
}

fn run_cached(
    cached: &Cached,
    id: u32,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> (i32, Option<String>) {
    run_built(cached, Src::Named(id), eqtb, logger)
}

fn run_built(
    cached: &Cached,
    src: Src,
    eqtb: &mut Eqtb,
    logger: &mut Logger,
) -> (i32, Option<String>) {
    let built = match &**cached {
        Ok(p) => p,
        Err(errs) => {
            // **数しか出せない**——rtex の記録は 7 ビットで、Vaak の文言は日本語である。
            // 中身を見たいときは `VAAK_DEBUG=1` を立てると標準エラーへ出る
            if std::env::var_os("VAAK_DEBUG").is_some() {
                for e in errs {
                    eprintln!("vaak: {e}");
                }
            }
            return (0, Some(format!("{} static error(s) before running", errs.len())));
        }
    };

    // **見ている添字だけ用意する。**
    // `count[5] * 2` なら 5 番だけ——256 個を見る必要が無い
    let mut host = HOST_BUF.with(|b| b.borrow_mut().take()).unwrap_or_else(|| {
        vec![regs_to_value(&[0; N_REGS]), regs_to_value(&[0; N_REGS])]
    });

    BEFORE.with(|b| {
        let mut b = b.borrow_mut();
        let (c, d) = b.split_at_mut(1);
        snapshot_counts(&built.count, eqtb, &mut c[0]);
        snapshot_dimens(&built.dimen, eqtb, &mut d[0]);
        sync(&mut host[0], &built.count, &c[0]);
        sync(&mut host[1], &built.dimen, &d[0]);
    });

    let (ev, after) = match RUNNER.with(|r| r.borrow_mut().run(&built.program, host)) {
        Ok(x) => x,
        Err(e) => {
            let (line, col) = src.line_col(e.span.start);
            return (0, Some(format!("{line}:{col}: the run did not finish")));
        }
    };

    // 変わった分だけ書き戻す。**`int_define` を通す**——保存スタックと `\global` のため
    BEFORE.with(|b| {
        let b = b.borrow();
        each(&built.count, |n| {
            if let Some(x) = elem(&after[0], n) {
                if x != b[0][n] {
                    eqtb.int_define(IntegerVariable::Count(n as RegisterIndex), x, false, logger);
                }
            }
        });
        each(&built.dimen, |n| {
            if let Some(x) = elem(&after[1], n) {
                if x != b[1][n] {
                    eqtb.dimen_define(DimensionVariable::Dimen(n as RegisterIndex), x, false);
                }
            }
        });
    });

    // 入れ物を取っておく。次の呼び出しで使い回す
    HOST_BUF.with(|b| *b.borrow_mut() = Some(after));

    // **最上位の外界面は言語の意味論ではない**（C-31）。ここで決める
    match ev {
        vaak::interp::Eval::Value(v) => match v.as_int() {
            Some(n) => (n as i32, None),
            None => (0, Some("the result is not an integer".to_string())),
        },
        // **中身が空で終わればホストに委ねる**（C-31）。エラーではない
        vaak::interp::Eval::Paradox(_) | vaak::interp::Eval::Akasha => (0, None),
        vaak::interp::Eval::Escape(x) => {
            let (line, col) = src.line_col(x.span.start);
            (0, Some(format!("{line}:{col}: the run did not finish")))
        }
    }
}

/// 位置を行と桁に。エラーの表示にだけ使う。
fn line_col(src: &[u8], offset: u32) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for b in src.iter().take(offset as usize) {
        if *b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// 見ている添字を順に。
fn each(t: &Touch, mut f: impl FnMut(usize)) {
    match t {
        Touch::None => {}
        Touch::Some(v) => v.iter().for_each(|n| f(*n)),
        Touch::All => (0..N_REGS).for_each(f),
    }
}

/// 見ている添字だけ控える。**返さない**——渡された場所に書く。
fn snapshot_counts(t: &Touch, eqtb: &Eqtb, out: &mut [i32; N_REGS]) {
    each(t, |n| out[n] = eqtb.integer(IntegerVariable::Count(n as RegisterIndex)));
}

fn snapshot_dimens(t: &Touch, eqtb: &Eqtb, out: &mut [i32; N_REGS]) {
    each(t, |n| out[n] = eqtb.dimen(DimensionVariable::Dimen(n as RegisterIndex)));
}

fn elem(v: &Value, n: usize) -> Option<i32> {
    let Value::Array(a) = v else { return None };
    a.items.get(n).and_then(|x| x.as_int()).map(|x| x as i32)
}

/// 入れ物の中身を、**見ている添字だけ**いまのレジスタに合わせる。
fn sync(v: &mut Value, t: &Touch, now: &[i32; N_REGS]) {
    let Value::Array(a) = v else { return };
    if a.items.len() != N_REGS {
        a.items.resize(N_REGS, Value::I32(0));
    }
    each(t, |n| a.items[n] = Value::I32(now[n]));
}

/// 入れ物の中身を、いまのレジスタに合わせる。**作り直さない。**
///
/// レジスタはたいてい変わっていないので、**512 回の比較**で済む——
/// 512 個の `Value` を作るより桁違いに安い。
fn sync_regs(v: &mut Value, now: &[i32; N_REGS]) {
    let Value::Array(a) = v else {
        *v = regs_to_value(now);
        return;
    };
    // スクリプトが伸ばしたり縮めたりした場合に備える
    if a.items.len() != N_REGS {
        a.items.resize(N_REGS, Value::I32(0));
    }
    for (slot, n) in a.items.iter_mut().zip(now.iter()) {
        match slot {
            Value::I32(x) if *x == *n => {}
            _ => *slot = Value::I32(*n),
        }
    }
}

/// レジスタの束を `i32 array` として作る。
fn regs_to_value(before: &[i32; N_REGS]) -> Value {
    Value::array(ValueType::I32, before.iter().map(|v| Value::I32(*v)).collect())
}

/// 走った後の値を取り出す。足りない分は元のままとする。
fn value_to_regs(v: &Value, before: &[i32; N_REGS]) -> [i32; N_REGS] {
    let mut out = *before;
    let Value::Array(a) = v else {
        return out;
    };
    for (n, slot) in out.iter_mut().enumerate() {
        if let Some(x) = a.items.get(n).and_then(|x| x.as_int()) {
            *slot = x as i32;
        }
    }
    out
}

/// エラーを報告する。**`\directlua` と見た目を揃える。**
///
/// ```text
/// ! Vaak interpreter error [\directvaak]:1:5: the run did not finish.
/// ```
fn report_error(msg: &str, scanner: &mut Scanner, eqtb: &mut Eqtb, logger: &mut Logger) {
    logger.print_err("Vaak interpreter error [");
    logger.print_esc_str(b"directvaak");
    logger.print_str("]:");
    logger.slow_print_str(msg.as_bytes());
    logger.error(
        &[
            "The Vaak code you gave could not be run to completion.",
            "I have expanded it to 0 and will carry on.",
        ],
        scanner,
        eqtb,
    );
}

/// **API を叩かずにレジスタへ書けるか** — 答え：**書ける。しかし書いてはいけない。**
///
/// `Eqtb::integers` は `pub` で、`IntegerParameters::set` も `pub` である。
/// したがって safe-Rust のまま、`int_define` を通さずに書ける:
///
/// ```ignore
/// eqtb.integers.set(IntegerVariable::Count(5), 42);
/// ```
///
/// **だが保存スタックを迂回する。**
/// `int_define` → `Eqtb::define` は `variable_levels` を見て、
/// **現在のグループで初めて触るなら前の値を積む。** これを飛ばすと:
///
/// ```tex
/// \count5=1
/// {\directvaak{ count[5] := 2; }}
/// % \count5 は 1 に戻るべきだが、2 のまま
/// ```
///
/// **速さのためにこれを選ぶ理由も無い。** `define` が積むのは
/// 「そのグループで初めて触る変数」だけで、二度目からは積まない。
/// **触るレジスタの数が上限**である。
fn _why_not_direct_write() {}

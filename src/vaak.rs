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

use crate::eqtb::{DimensionVariable, Eqtb, IntegerVariable, RegisterIndex};
use crate::input::Scanner;
use crate::logger::Logger;
use crate::print::string::StringPrinter;
use crate::print::Printer;
use crate::token::Token;
use crate::token_lists::{str_toks, token_show};

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

type Cached = Result<Built, Vec<String>>;

thread_local! {
    static CACHE: RefCell<HashMap<Vec<u8>, Cached>> = RefCell::new(HashMap::new());
    /// 見せる値の入れ物を**使い回す**。
    ///
    /// 呼び出しのたびに 512 個の `Value` を作り直すのは無駄である——
    /// **レジスタはたいてい変わっていない。**
    /// 前回の入れ物を持っておき、**変わった分だけ書き換えて**渡す。
    ///
    /// `run_program_with_host` は使い終わった入れ物を返すので、それを取っておく。
    static HOST_BUF: RefCell<Option<Vec<Value>>> = const { RefCell::new(None) };
}

/// ホストが見せる名前と型。**常に同じ**なので鍵に含めない。
fn exposed() -> Vec<(String, ValueType)> {
    let arr = ValueType::Array(Box::new(ValueType::I32));
    vec![("count".to_string(), arr.clone()), ("dimen".to_string(), arr)]
}

/// 解析・検査・組み立て。**一度だけ行い、覚えておく。**
fn compile_cached(source: &[u8]) -> Cached {
    CACHE.with(|c| {
        if std::env::var_os("VAAK_NO_CACHE").is_none() {
            if let Some(hit) = c.borrow().get(source) {
                return hit.clone();
            }
        }
        let built = build(source);
        c.borrow_mut().insert(source.to_vec(), built.clone());
        built
    })
}

fn build(source: &[u8]) -> Cached {
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

    let (code, error) = run_vaak(&source, eqtb, logger);

    if let Some(msg) = error {
        report_error(&msg, scanner, eqtb, logger);
    }

    // **必ず数字を出す。** 展開が空だと TeX の数値走査が壊れる
    let digits = format!("{code}");
    let toks = str_toks(digits.as_bytes());
    scanner.ins_list(toks, eqtb, logger);
}

/// Vaak を走らせ、(終了コード, エラー文) を返す。
fn run_vaak(source: &[u8], eqtb: &mut Eqtb, logger: &mut Logger) -> (i32, Option<String>) {
    // **組み上がったものは覚えてある。** 二度目からは解析も検査もしない
    let built = match compile_cached(source) {
        Ok(p) => p,
        Err(errs) => {
            return (0, Some(format!("{} static error(s) before running", errs.len())));
        }
    };

    // **見ている添字だけ用意する。**
    // `count[5] * 2` なら 5 番だけ——256 個を見る必要が無い
    let mut host = HOST_BUF.with(|b| b.borrow_mut().take()).unwrap_or_else(|| {
        vec![regs_to_value(&[0; N_REGS]), regs_to_value(&[0; N_REGS])]
    });

    let counts_before = snapshot_counts(&built.count, eqtb);
    let dimens_before = snapshot_dimens(&built.dimen, eqtb);
    sync(&mut host[0], &built.count, &counts_before);
    sync(&mut host[1], &built.dimen, &dimens_before);

    let (ev, after) = match vaak::vm::run_program_with_host(&built.program, host) {
        Ok(x) => x,
        Err(e) => {
            let (line, col) = line_col(source, e.span.start);
            return (0, Some(format!("{line}:{col}: the run did not finish")));
        }
    };

    // 変わった分だけ書き戻す。**`int_define` を通す**——保存スタックと `\global` のため
    each(&built.count, |n| {
        if let Some(x) = elem(&after[0], n) {
            if x != counts_before[n] {
                eqtb.int_define(IntegerVariable::Count(n as RegisterIndex), x, false, logger);
            }
        }
    });
    each(&built.dimen, |n| {
        if let Some(x) = elem(&after[1], n) {
            if x != dimens_before[n] {
                eqtb.dimen_define(DimensionVariable::Dimen(n as RegisterIndex), x, false);
            }
        }
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
            let (line, col) = line_col(source, x.span.start);
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

fn snapshot_counts(t: &Touch, eqtb: &Eqtb) -> [i32; N_REGS] {
    let mut out = [0i32; N_REGS];
    each(t, |n| out[n] = eqtb.integer(IntegerVariable::Count(n as RegisterIndex)));
    out
}

fn snapshot_dimens(t: &Touch, eqtb: &Eqtb) -> [i32; N_REGS] {
    let mut out = [0i32; N_REGS];
    each(t, |n| out[n] = eqtb.dimen(DimensionVariable::Dimen(n as RegisterIndex)));
    out
}

fn elem(v: &Value, n: usize) -> Option<i32> {
    let Value::Array { items, .. } = v else { return None };
    items.get(n).and_then(|x| x.as_int()).map(|x| x as i32)
}

/// 入れ物の中身を、**見ている添字だけ**いまのレジスタに合わせる。
fn sync(v: &mut Value, t: &Touch, now: &[i32; N_REGS]) {
    let Value::Array { items, .. } = v else { return };
    if items.len() != N_REGS {
        items.resize(N_REGS, Value::I32(0));
    }
    each(t, |n| items[n] = Value::I32(now[n]));
}

/// 入れ物の中身を、いまのレジスタに合わせる。**作り直さない。**
///
/// レジスタはたいてい変わっていないので、**512 回の比較**で済む——
/// 512 個の `Value` を作るより桁違いに安い。
fn sync_regs(v: &mut Value, now: &[i32; N_REGS]) {
    let Value::Array { items, .. } = v else {
        *v = regs_to_value(now);
        return;
    };
    // スクリプトが伸ばしたり縮めたりした場合に備える
    if items.len() != N_REGS {
        items.resize(N_REGS, Value::I32(0));
    }
    for (slot, n) in items.iter_mut().zip(now.iter()) {
        match slot {
            Value::I32(x) if *x == *n => {}
            _ => *slot = Value::I32(*n),
        }
    }
}

/// レジスタの束を `i32 array` として作る。
fn regs_to_value(before: &[i32; N_REGS]) -> Value {
    Value::Array {
        elem: ValueType::I32,
        items: before.iter().map(|v| Value::I32(*v)).collect(),
    }
}

/// 走った後の値を取り出す。足りない分は元のままとする。
fn value_to_regs(v: &Value, before: &[i32; N_REGS]) -> [i32; N_REGS] {
    let mut out = *before;
    let Value::Array { items, .. } = v else {
        return out;
    };
    for (n, slot) in out.iter_mut().enumerate() {
        if let Some(x) = items.get(n).and_then(|x| x.as_int()) {
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

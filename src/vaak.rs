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

use vaak::ast::ValueType;
use vaak::host::{Host, Outcome};
use vaak::value::Value;

/// レジスタの数。TeX82 は 256 個（`RegisterIndex = u8`）。
const N_REGS: usize = 256;

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
    let src = String::from_utf8_lossy(source).into_owned();

    // 走らせる前の値を控える。**変わった分だけ書き戻す**ため
    let mut counts_before = [0i32; N_REGS];
    for (n, slot) in counts_before.iter_mut().enumerate() {
        *slot = eqtb.integer(IntegerVariable::Count(n as RegisterIndex));
    }
    let mut dimens_before = [0i32; N_REGS];
    for (n, slot) in dimens_before.iter_mut().enumerate() {
        *slot = eqtb.dimen(DimensionVariable::Dimen(n as RegisterIndex));
    }

    let mut host = Host::new();
    // **起動時点で全ての数値レジスタへの別名を持たせる。**
    // スクリプトは `&=` を書かなくてよい
    host.expose_value("count", regs_to_value(&counts_before));
    host.expose_value("dimen", regs_to_value(&dimens_before));

    let outcome = host.run(&src);

    // 変わった分だけ書き戻す。**`int_define` を通す**——保存スタックと `\global` のため
    if let Some(v) = host.get("count").map(|b| b.read()) {
        let after = value_to_regs(&v, &counts_before);
        for n in 0..N_REGS {
            if counts_before[n] != after[n] {
                eqtb.int_define(
                    IntegerVariable::Count(n as RegisterIndex),
                    after[n],
                    false,
                    logger,
                );
            }
        }
    }
    if let Some(v) = host.get("dimen").map(|b| b.read()) {
        let after = value_to_regs(&v, &dimens_before);
        for n in 0..N_REGS {
            if dimens_before[n] != after[n] {
                eqtb.dimen_define(DimensionVariable::Dimen(n as RegisterIndex), after[n], false);
            }
        }
    }

    // **最上位の外界面は言語の意味論ではない**（C-31）。ここで決める
    match outcome {
        Outcome::Value(v) => match v.as_int() {
            Some(n) => (n as i32, None),
            None => (0, Some("the result is not an integer".to_string())),
        },
        // **中身が空で終わればホストに委ねる**（C-31）。エラーではない
        Outcome::Empty | Outcome::Paradox { .. } => (0, None),
        Outcome::Runtime { line, col, .. } => {
            (0, Some(format!("{line}:{col}: the run did not finish")))
        }
        Outcome::Static(errs) => {
            (0, Some(format!("{} static error(s) before running", errs.len())))
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

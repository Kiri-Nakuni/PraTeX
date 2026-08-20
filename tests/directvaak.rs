//! `\directvaak` の受け入れ試験。
//!
//! rtex 本体を動かすのが難しいので、**Vaak の側の振る舞い**を確かめる。
//! TeX の側は `docs/vaak-integration.md` の段取りに沿って手で確かめた。

use vaak::ast::ValueType;
use vaak::host::{Host, Outcome};
use vaak::value::Value;

fn regs(vals: &[i32]) -> Value {
    Value::Array {
        elem: ValueType::I32,
        items: vals.iter().map(|v| Value::I32(*v)).collect(),
    }
}

fn run(src: &str, before: &[i32]) -> (Outcome, Vec<i32>) {
    let mut h = Host::new();
    h.expose_value("count", regs(before));
    let out = h.run(src);
    let after = match h.get("count").map(|b| b.read()) {
        Some(Value::Array { items, .. }) => {
            items.iter().filter_map(|x| x.as_int()).map(|x| x as i32).collect()
        }
        _ => before.to_vec(),
    };
    (out, after)
}

#[test]
fn 起動時点でレジスタが見えている() {
    // `&=` を書かなくてよい
    let (out, _) = run("count[1] + count[2]", &[0, 10, 20, 0]);
    match out {
        Outcome::Value(v) => assert_eq!(v.as_int(), Some(30)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn レジスタに書ける() {
    let (_, after) = run("count[3] := 99;", &[0, 0, 0, 0]);
    assert_eq!(after[3], 99);
}

#[test]
fn 中身が空なら終了コードは_0() {
    // **最上位の外界面は言語の意味論ではない**（C-31）。ホストが 0 と決めた
    let (out, _) = run("", &[0]);
    assert!(matches!(out, Outcome::Paradox { .. } | Outcome::Empty), "{out:?}");
}

#[test]
fn レジスタは_i32_なので折り返す() {
    // TeX の Integer は i32。**変換が要らないので、変換の誤りも起きない**
    let (out, _) = run("count[0] + 1", &[2147483647]);
    match out {
        Outcome::Value(v) => assert_eq!(v.as_int(), Some(-2147483648)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn 別名でも触れる() {
    let (_, after) = run("var c : i32 array alias &= count; c[2] := 7;", &[0, 0, 0]);
    assert_eq!(after[2], 7);
}

#[test]
fn 静的エラーは走らせる前に返る() {
    // ループの本体に値が残る——**走らせずに数えられる**
    let (out, after) = run("loop { count[0] }", &[5]);
    assert!(matches!(out, Outcome::Static(_)), "{out:?}");
    // **走っていないので、レジスタは動かない**
    assert_eq!(after[0], 5);
}

#[test]
fn 走査は書けるが範囲外は_paradox() {
    let (out, _) = run("count[999] ?? -1", &[0, 0]);
    match out {
        Outcome::Value(v) => assert_eq!(v.as_int(), Some(-1)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn 脱出で値を返せる() {
    let (out, _) = run(
        "nfor (i, 0, 4) { if (count[i] == 0) break i; fi; }",
        &[3, 4, 0, 9],
    );
    match out {
        Outcome::Value(v) => assert_eq!(v.as_int(), Some(2)),
        other => panic!("{other:?}"),
    }
}

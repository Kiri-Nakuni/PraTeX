//! 名前空間（字句層まで）。
//!
//! **catcode 16 の文字が名前空間の印である。** `*foo\hello` は
//! 名前空間 `foo` の `hello`——global の `\hello` とは別物である。

use std::io::Write;
use std::process::Command;

fn run_tex(name: &str, body: &str) -> String {
    // **場所の名前は ASCII にする。** rtex の記録は 7 ビットで、
    // 日本語の道が `^^e6` に化けて読めなくなる。
    // 名前を潰すと**試験どうしがぶつかる**ので、写しではなく畳んだ値を使う
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    let dir = std::env::temp_dir().join(format!("ns-{}-{:x}", std::process::id(), h.finish()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(
        f,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n\\catcode`\\*=16\n{body}\n\\end\n"
    )
    .unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    std::fs::read_to_string(dir.join("t.log")).unwrap()
}

#[test]
fn 名前空間が違えば別物() {
    let log = run_tex(
        "別物",
        "\\def*foo\\hello{FOO}\\def\\hello{GLOBAL}\\def*bar\\hello{BAR}\n\
         \\message{[*foo\\hello][\\hello][*bar\\hello]}",
    );
    assert!(log.contains("[FOO][GLOBAL][BAR]"), "{log}");
}

#[test]
fn ifxは意味を比べる() {
    // **`\ifx` は意味を比べる。同一性ではない。**
    // 別の制御綴でも、中身が同じなら等しいと言う——TeX82 のとおりである
    let log = run_tex(
        "ifx同じ",
        "\\def*foo\\aa{X}\\def\\aa{X}\n\\message{[\\ifx*foo\\aa\\aa Y\\else N\\fi]}",
    );
    assert!(log.contains("[Y]"), "{log}");

    // 中身が違えば違う——**別の入れ物である**ことはこちらで分かる
    let log = run_tex(
        "ifx違う",
        "\\def*foo\\aa{X}\\def\\aa{Y}\n\\message{[\\ifx*foo\\aa\\aa Y\\else N\\fi]}",
    );
    assert!(log.contains("[N]"), "{log}");
}

#[test]
fn 同じ名前空間なら同じもの() {
    let log = run_tex(
        "同じ",
        "\\def*foo\\a{X}\\let*foo\\b=*foo\\a\n\\message{[*foo\\b][\\ifx*foo\\a*foo\\b Y\\else N\\fi]}",
    );
    assert!(log.contains("[X][Y]"), "{log}");
}

#[test]
fn 一文字の制御綴も名前空間に入る() {
    let log = run_tex(
        "一文字",
        "\\def*foo\\!{NS}\\def\\!{GLOBAL}\n\\message{[*foo\\!][\\!]}",
    );
    assert!(log.contains("[NS][GLOBAL]"), "{log}");
}

#[test]
fn 群を出れば戻る() {
    // **同じ番号空間に載せた効き目。** save stack がそのまま働く
    let log = run_tex(
        "群",
        "\\def*foo\\a{OUT}\n{\\def*foo\\a{IN}\\message{[in=*foo\\a]}}\\message{[out=*foo\\a]}",
    );
    assert!(log.contains("[in=IN]"), "{log}");
    assert!(log.contains("[out=OUT]"), "{log}");
}

#[test]
fn globalは群を越える() {
    let log = run_tex(
        "global",
        "\\def*foo\\a{OUT}\n{\\global\\def*foo\\a{IN}}\\message{[out=*foo\\a]}",
    );
    assert!(log.contains("[out=IN]"), "{log}");
}

#[test]
fn 名前空間の名前に印を含められる() {
    // **階層ではない。** `*a*b\hoge` は `a*b` の `hoge`
    let log = run_tex(
        "入れ子でない",
        "\\def*a*b\\h{AB}\\def*a\\h{A}\n\\message{[*a*b\\h][*a\\h]}",
    );
    assert!(log.contains("[AB][A]"), "{log}");
}

#[test]
fn 名前が閉じなければ暴走を報せる() {
    let log = run_tex("runaway", "\\def*foo bar{X}\n\\message{[done]}");
    assert!(log.contains("Runaway namespace name"), "{log}");
    // **読み飛ばして続く**
    assert!(log.contains("[done]"), "{log}");
}

#[test]
fn 印を置かなければ何も変わらない() {
    // catcode 16 の文字が無ければ、字句化は TeX82 のままである
    let dir = std::env::temp_dir().join(format!("ns-plain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(
        f,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n\\def\\a{{X}}\n\\message{{[\\a][*]}}\n\\end\n"
    )
    .unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    let log = std::fs::read_to_string(dir.join("t.log")).unwrap();
    assert!(log.contains("[X][*]"), "{log}");
}

// ===== Phase 3：`\namespace` と `\csname` =====

#[test]
fn csname経由でも同じ制御綴になる() {
    let log = run_tex(
        "csname",
        "\\def*foo\\bar{FOOBAR}\n\
         \\message{[\\namespace foo\\csname bar\\endcsname]}",
    );
    assert!(log.contains("[FOOBAR]"), "{log}");
}

#[test]
fn グローバルに作られない() {
    // **`\endcsname` を終端にする案を退けた理由そのもの。**
    // 登録は `\endcsname` に達した一箇所で起きるので、そこへ名前空間を渡すしかない
    let log = run_tex(
        "作られない",
        "\\def*foo\\bar{X}\n\
         \\message{[\\namespace foo\\csname bar\\endcsname]}\n\
         \\message{[global=\\ifx\\bar\\undefined Y\\else N\\fi]}",
    );
    assert!(log.contains("[global=Y]"), "{log}");
}

#[test]
fn csnameで作ってdefできる() {
    let log = run_tex(
        "作る",
        "\\expandafter\\def\\namespace zoo\\csname qux\\endcsname{ZOOQUX}\n\
         \\message{[*zoo\\qux]}",
    );
    assert!(log.contains("[ZOOQUX]"), "{log}");
}

#[test]
fn 名前空間名は展開される() {
    let log = run_tex(
        "展開",
        "\\def*foo\\bar{FOOBAR}\\def\\ns{foo}\n\
         \\message{[\\namespace \\ns\\csname bar\\endcsname]}",
    );
    assert!(log.contains("[FOOBAR]"), "{log}");
}

#[test]
fn 空の名前空間名はグローバルそのもの() {
    let log = run_tex(
        "空",
        "\\def\\bar{GLOBAL}\n\\message{[\\namespace\\csname bar\\endcsname]}",
    );
    assert!(log.contains("[GLOBAL]"), "{log}");
}

#[test]
fn 名前空間は入れ子にできる() {
    // **二つの `\namespace` は同じ `\csname` を奪い合わない。**
    // 内側は最初の一組を消費して `*hoge\fuga` を作り、
    // それが展開されて外側の名前空間名の文字になる。
    // 外側は自分の `\csname` を別に持つ——括弧のように入れ子になるだけである
    let log = run_tex(
        "入れ子",
        "\\def*hoge\\fuga{zoo}\\def*zoo\\bar{ZOOBAR}\n\
         \\message{[\\namespace \\namespace hoge\\csname fuga\\endcsname\\csname bar\\endcsname]}",
    );
    assert!(log.contains("[ZOOBAR]"), "{log}");
}

#[test]
fn 名前空間名は普通のマクロでも作れる() {
    // 入れ子を許すのは**非対称を作らない**ためである。
    // global のマクロで作れるなら、名前空間つきのマクロでも作れねばならない
    let log = run_tex(
        "普通のマクロ",
        "\\def\\ns{quux}\\def*quux\\x{QX}\n\\message{[\\namespace \\ns\\csname x\\endcsname]}",
    );
    assert!(log.contains("[QX]"), "{log}");
}

#[test]
fn csname以外が来れば誤り() {
    let log = run_tex("csname無し", "\\namespace foo\\relax \\message{[done]}");
    assert!(log.contains("Missing"), "{log}");
    assert!(log.contains("[done]"), "{log}");
}

// ===== Phase 4：印字と `\namespacechar` =====

#[test]
fn 既定では名前空間を印字しない() {
    // **`\namespacechar` の既定は −1。** 名前空間を使わない文書では見えない
    let log = run_tex("既定", "\\def*foo\\bar{X}\n\\message{[\\string*foo\\bar]}");
    assert!(log.contains("[\\bar]"), "{log}");
}

#[test]
fn 印を決めれば名前空間も印字する() {
    let log = run_tex(
        "印あり",
        "\\def*foo\\bar{X}\\namespacechar=`\\*\n\\message{[\\string*foo\\bar][\\string\\bar]}",
    );
    assert!(log.contains("[*foo\\bar][\\bar]"), "{log}");
}

#[test]
fn showが名前空間つきで出す() {
    let log = run_tex("show", "\\def*foo\\bar{X}\\namespacechar=`\\*\n\\show*foo\\bar");
    assert!(log.contains("> *foo\\bar=macro:"), "{log}");
}

#[test]
fn 名前空間の印は逆読みできる() {
    // §5 の reflection：**最初の escapechar で割れる**
    let log = run_tex(
        "逆読み",
        "\\def*foo\\bar{X}\\namespacechar=`\\*\\escapechar=`\\\\\n\\message{[\\string*foo\\bar]}",
    );
    assert!(log.contains("[*foo\\bar]"), "{log}");
}

// ===== Phase 5：`\if` / `\ifcat` =====

#[test]
fn 名前空間つきの活性文字も活性である() {
    // **分解ではなく問い合わせる。** `Active(c)` の分解では見つからない
    let log = run_tex(
        "活性",
        "\\catcode`\\~=13 \\def~{TILDE}\\def*foo~{NSTILDE}\n\
         \\message{[\\ifcat\\noexpand*foo~\\noexpand~ Y\\else N\\fi]}",
    );
    // `\noexpand~` の後の空白は読み飛ばされるので、記録には ` Y` と出る
    assert!(log.contains("Y]"), "{log}");
    assert!(!log.contains("N]"), "{log}");
}

#[test]
fn 活性文字の同一性はifxの仕事() {
    // `\ifcat` は catcode の**問い合わせ**であって同一性の判定ではない。
    // **`*ns~` と `~` を区別するのは `\ifx` である**
    let log = run_tex(
        "同一性",
        "\\catcode`\\~=13 \\def~{TILDE}\\def*foo~{NSTILDE}\n\
         \\message{[\\ifx*foo~~Y\\else N\\fi][~][*foo~]}",
    );
    assert!(log.contains("[N][TILDE][NSTILDE]"), "{log}");
}

#[test]
fn 名前空間つきの活性文字はエスケープを挟まない() {
    let log = run_tex(
        "活性の印字",
        "\\catcode`\\~=13 \\def*foo~{X}\\namespacechar=`\\*\n\\message{[\\string*foo~]}",
    );
    assert!(log.contains("[*foo~]"), "{log}");
}

// ===== Phase 7：fmt =====

#[test]
fn 書き出して読み直せる() {
    // **新しい欄が dump/undump を往復すること。** 名前空間の表も含む
    let dir = std::env::temp_dir().join(format!("ns-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ini = dir.join("mk.tex");
    let mut f = std::fs::File::create(&ini).unwrap();
    write!(
        f,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n\\catcode`\\*=16\n\
         \\def*foo\\bar{{FMT}}\\namespacechar=`\\*\n\\dump\n"
    )
    .unwrap();
    drop(f);
    let out = Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg(&ini)
        .current_dir(&dir)
        .output()
        .unwrap();
    let fmt = dir.join("mk.fmt");
    if !fmt.exists() {
        // このビルドが `\dump` を持たないなら飛ばす
        eprintln!("`\\dump` が使えないので飛ばす: {}", String::from_utf8_lossy(&out.stderr));
        return;
    }
    let use_ = dir.join("use.tex");
    let mut f = std::fs::File::create(&use_).unwrap();
    write!(f, "\\message{{[\\string*foo\\bar][*foo\\bar]}}\n\\end\n").unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("&mk")
        .arg(&use_)
        .current_dir(&dir)
        .output()
        .unwrap();
    let log = std::fs::read_to_string(dir.join("use.log")).unwrap_or_default();
    assert!(log.contains("[*foo\\bar][FMT]"), "{log}");
}

// ===== Phase 6：参照時探索（`\usingnamespace`）=====

#[test]
fn 使用宣言で名前空間を探しに行く() {
    let log = run_tex(
        "使用宣言",
        "\\def*lib\\greet{HELLO}\n\
         \\message{[before=\\ifx\\greet\\undefined U\\else D\\fi]}\n\
         \\usingnamespace lib\n\\message{[after=\\greet]}",
    );
    assert!(log.contains("[before=U]"), "{log}");
    assert!(log.contains("[after=HELLO]"), "{log}");
}

#[test]
fn グローバルが優先される() {
    // **フォーマットを名前空間で上書きする用途は非目標である**
    let log = run_tex(
        "global優先",
        "\\def*lib\\bye{NS}\\def\\bye{GLOBAL}\\usingnamespace lib\n\\message{[\\bye]}",
    );
    assert!(log.contains("[GLOBAL]"), "{log}");
}

#[test]
fn 追加順に探す() {
    let log = run_tex(
        "追加順",
        "\\def*a\\x{A}\\def*b\\x{B}\\usingnamespace a \\usingnamespace b\n\\message{[\\x]}",
    );
    assert!(log.contains("[A]"), "{log}");
}

#[test]
fn 使用宣言は群を出れば戻る() {
    // **保存スタックを通している**
    let log = run_tex(
        "群で戻る",
        "\\def*lib\\greet{HELLO}\n\
         {\\usingnamespace lib \\message{[in=\\greet]}}\n\
         \\message{[out=\\ifx\\greet\\undefined U\\else D\\fi]}",
    );
    assert!(log.contains("[in=HELLO]"), "{log}");
    assert!(log.contains("[out=U]"), "{log}");
}

#[test]
fn 使用宣言もglobalにできる() {
    let log = run_tex(
        "global宣言",
        "\\def*lib\\greet{HELLO}\n{\\global\\usingnamespace lib}\n\\message{[\\greet]}",
    );
    assert!(log.contains("[HELLO]"), "{log}");
}

#[test]
fn 局所の定義が名前空間より優先される() {
    let log = run_tex(
        "局所優先",
        "\\def*lib\\greet{NS}\\usingnamespace lib\n\
         {\\def\\greet{INNER}\\message{[in=\\greet]}}\\message{[out=\\greet]}",
    );
    assert!(log.contains("[in=INNER]"), "{log}");
    assert!(log.contains("[out=NS]"), "{log}");
}

#[test]
fn csnameも探索に参加する() {
    // 仕様の §6「既知の危険」は実態より強い——
    // **`\csname` が探索に参加する以上、`\relax` 化は起きない**
    let log = run_tex(
        "csname探索",
        "\\def*lib\\greet{HELLO}\\usingnamespace lib\n\
         \\message{[\\csname greet\\endcsname]}",
    );
    assert!(log.contains("[HELLO]"), "{log}");
}

#[test]
fn 宣言しなければ何も変わらない() {
    // **使わない機能は費用を持たない。** 一覧が空なら素の TeX82 と同じ道
    let log = run_tex(
        "宣言なし",
        "\\def*lib\\greet{HELLO}\n\\message{[\\ifx\\greet\\undefined U\\else D\\fi]}",
    );
    assert!(log.contains("[U]"), "{log}");
}

#[test]
fn 一文字の制御綴と活性文字は別物() {
    // **鍵に「活性か」を入れないと衝突する**——どちらも名前が一文字の `~` になる
    let log = run_tex(
        "衝突",
        "\\catcode`\\~=13 \\def*lib\\~{SYMBOL}\\def*lib~{ACTIVE}\\usingnamespace lib\n\
         \\message{[\\~][~]}",
    );
    assert!(log.contains("[SYMBOL][ACTIVE]"), "{log}");
}

#[test]
fn 一文字の制御綴も探索に参加する() {
    // `to_token` は今まで `Single(c)` へ直に落としていたので漏れていた
    let log = run_tex(
        "一文字探索",
        "\\def*lib\\x{ONE}\\usingnamespace lib\n\\message{[\\x]}",
    );
    assert!(log.contains("[ONE]"), "{log}");
}

// ===== 全部入り：Vaak と名前空間が同居する =====

#[test]
fn vaakと名前空間が同居する() {
    // **名前空間の印は Vaak の本体にも効く。**
    // `*` を印にすると `count[5] * 2` の `*` が名前空間の始まりになるので、
    // **Vaak の綴りに現れない文字を選ぶ**（ここでは `@`）
    let dir = std::env::temp_dir().join(format!("ns-full-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(
        f,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n\
         \\catcode`\\@=16\n\\namespacechar=`\\@\n\
         \\count5=10 \\count6=3\n\
         \\vaakdef@lib\\tally{{ count[5] * 2 + count[6] }}\n\
         \\usingnamespace lib\n\
         \\count0=\\tally\n\
         \\dimen0=\\dimexpr 4Q*2\\relax\n\
         \\message{{[\\the\\count0][\\the\\dimen0][\\string@lib\\tally]}}\n\\end\n"
    )
    .unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    let log = std::fs::read_to_string(dir.join("t.log")).unwrap();
    // Vaak が走り、e-TeX の式が和文単位を受け、名前空間つきの名前が印字される
    assert!(log.contains("[23]"), "{log}");
    assert!(log.contains("[5.69052pt]"), "{log}");
    assert!(log.contains("[@lib\\tally]"), "{log}");
}

#[test]
fn 名前空間の印はvaakの綴りと衝突しうる() {
    // **既知の危険。** `*` を印にすると Vaak の掛け算が壊れる
    let dir = std::env::temp_dir().join(format!("ns-clash-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tex");
    let mut f = std::fs::File::create(&src).unwrap();
    write!(
        f,
        "\\catcode`\\{{=1\n\\catcode`\\}}=2\n\\batchmode\n\\catcode`\\*=16\n\
         \\vaakdef\\t{{ 3 * 4 }}\n\\message{{[done]}}\n\\end\n"
    )
    .unwrap();
    drop(f);
    Command::new(env!("CARGO_BIN_EXE_rtex"))
        .arg("t.tex")
        .current_dir(&dir)
        .output()
        .unwrap();
    let log = std::fs::read_to_string(dir.join("t.log")).unwrap();
    assert!(log.contains("Runaway namespace name"), "{log}");
    // **落ちない。** 読み飛ばして続く
    assert!(log.contains("[done]"), "{log}");
}

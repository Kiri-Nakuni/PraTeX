# 生文字列レジスタと `\showthe`

## 保存値

生文字列レジスタはtoken listやRustの`String`ではなく、任意のbyte列を保存する。
初期実装は `Rc<Vec<u8>>` とし、0..32767の低位dense・高位sparse storageを既存の
e-TeX拡張レジスタと同じ規則で持つ。NUL、改行、不正UTF-8を失ってはならない。

代入・別名の規則は次のとおり。

- `\rawstringdef\a=7` のような別名はslot 7を指し、`\let`した別名も同じslotを指す。
- レジスタ間の値代入は現在の`Rc`をO(1)でcopyする。その後のslot代入は互いに独立する。
- group、`\global`、`\globaldefs`、fmt往復は既存`Eqtb::define`の規則へ載せる。
- token listとはstorageもaliasも共有しない。

真のraw値をbrace内のtoken列から復元することはできない。最初のproducerは明示delimiterを
TextSourceから直接読む入力、file、Vaak/WASMの長さ付きbyte bufferとする。token列を
detokenizeして格納する便利機能を足す場合は、raw source captureとは別名・別契約にする。

## 三つのconsumer

値の取得は `InternalValue::RawString(Rc<Vec<u8>>)` まで共通化し、その後は混ぜない。

### `\the\rawstring<n>`

展開時点のcatcode/kcatcodeで入力断片を字句化し、得たtoken列をその場へ挿入する。
字句化はsnapshotであり、同じrawから作ったtoken列は後のcatcode変更で変わらない。rawを
もう一度 `\the` した時だけ新しい分類が効く。保存中のLF/CRLFはsynthetic line boundary
として扱い、通常入力と同じspace/comment/control-sequence/CJK規則を各行へ適用する。
暗黙のendlinecharは追加しない。

`\edef`、`\message`、`\write`などの展開走査では、通常の`\the`と同じく生成tokenを結果へ
直接追加し、もう一度展開しない。途中のtoken実行で同じrawの後半の分類が変わるlive
pseudo-file semanticsは採用しない。必要なら将来、別の入力source primitiveとして設計する。

### `\therawstring\rawstring<n>`

escape、comment、active、`^^`、改行を解釈しない。初期のbyte token表現では、空白、NUL、
LF/CR、UTF-8を構成するbyteも含め、各byteを`OtherChar`として直接返す。したがってLFも
line boundaryやspaceにはならない。将来Unicode Other tokenを導入しても、byte列の往復を
失わないことを契約にする。

### `\showthe\rawstring<n>`

現行の通常値は `the_toks -> token_show` で表示できるが、生文字列には使わない。raw内容を
LineLexer、入力stack、control-sequence lookupへ一度も渡さず、専用診断printerで表示する。
これにより、格納値が `\global\count0=...`、`\end`、unmatched brace、`%`、active文字でも
実行・欠落・制御綴作成を起こさない。

表示規則は次のとおり。

- canonical UTF-8だけをUnicode一文字単位でatomicに表示する。
- overlong、surrogate、truncatedを含む不正列は先頭1byteずつ `^^hh` にする。
- NUL、LF、CRは `^^@`、`^^J`、`^^M` とし、`newlinechar`と一致しても物理改行にしない。
- 長行折返しはUTF-8列や`^^hh`の途中で切らず、既存token表示相当の上限を持つ。
- `\meaning` / `\show` は内容でなく `\rawstring7` 相当のvariable designatorを表示し、
  `\showthe`だけが値を表示する。

## fmtと破損入力

fmtはraw byteを行へ直書きしない。storage header/versionと長さ付きu8数値列、またはhexで
保存し、長さ上限、truncated値、0..255外の値、範囲外slot、sparse重複をundump時に拒否する。
`Rc`共有構造自体は復元対象にせず、bytesとslot aliasだけを意味として保持する。

## 必須試験

- `theは参照時のcatcodeとkcatcodeで字句化する`
- `therawstringは空白とnulとutf八byteも全てotherにする`
- `showtheは格納した制御綴や代入を実行せずcs表も増やさない`
- `showtheは改行nul不正utf八を安全に逃がす`
- `toksへ写した時点でcatcodeを固定する`
- `letした別名は同じslotを指し値copyは後の代入から独立する`
- `局所代入をgroup終了で戻しglobaldefsを守る`
- `fmt往復でnul改行不正utf八と高位registerを保つ`
- 巨大長、truncated、範囲外byte、duplicate sparseを含むmalformed fmtを拒否する

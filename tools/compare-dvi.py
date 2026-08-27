#!/usr/bin/env python3
"""二つの DVI を意味で比べる。

engine 名が `TeX` と `rtex` で長さが違うため、preamble の comment 長が
1 byte ずれ、以降の byte 位置がすべてずれる。したがってクロスエンジンでは
byte 一致は原理的に成立しない。

ここでは命令列を取り出して比べる。位置に依存する値、すなわち
- preamble の comment
- `bop` が持つ前頁への後方 pointer
- `post` / `post_post` が持つ pointer
- font 定義の area と name
は正規化して除く。それ以外の命令と作用対象が完全に一致すれば、
同じ組版結果である。

使い方:
    python3 tools/compare-dvi.py a.dvi b.dvi
"""
import sys

# 命令ごとの引数 byte 数。負値は「符号つき」を意味しない。ここでは長さだけ使う。
SET1, SET_RULE = 128, 132
PUT1, PUT_RULE = 133, 137
NOP, BOP, EOP, PUSH, POP = 138, 139, 140, 141, 142
RIGHT1, W0, W1, X0, X1 = 143, 147, 148, 152, 153
DOWN1, Y0, Y1, Z0, Z1 = 157, 161, 162, 166, 167
FNT_NUM_0, FNT1 = 171, 235
XXX1, FNT_DEF1, PRE, POST, POST_POST = 239, 243, 247, 248, 249


def parse(data):
    """DVI を (命令名, 引数...) の列にする。位置依存の値は落とす。"""
    out = []
    i = 0
    n = len(data)

    def num(k, signed=False):
        nonlocal i
        v = int.from_bytes(data[i:i + k], "big", signed=signed)
        i += k
        return v

    while i < n:
        op = data[i]
        i += 1
        if op < 128:
            out.append(("set_char", op))
        elif op == PRE:
            num(1)                      # version
            num(4); num(4); num(4)      # num, den, mag
            k = num(1)
            i += k                      # comment は engine 名を含むので落とす
            out.append(("pre",))
        elif op in (POST, POST_POST):
            # 末尾は pointer と後始末だけなので、ここで打ち切る。
            out.append((("post" if op == POST else "post_post"),))
            break
        elif op == BOP:
            counts = [num(4, True) for _ in range(10)]
            num(4, True)                # 前頁への pointer は位置依存
            out.append(("bop", *counts))
        elif op == EOP:
            out.append(("eop",))
        elif op == PUSH:
            out.append(("push",))
        elif op == POP:
            out.append(("pop",))
        elif op == NOP:
            pass
        elif SET1 <= op <= SET1 + 3:
            out.append(("set", num(op - SET1 + 1)))
        elif op == SET_RULE:
            out.append(("set_rule", num(4, True), num(4, True)))
        elif PUT1 <= op <= PUT1 + 3:
            out.append(("put", num(op - PUT1 + 1)))
        elif op == PUT_RULE:
            out.append(("put_rule", num(4, True), num(4, True)))
        elif RIGHT1 <= op <= RIGHT1 + 3:
            out.append(("right", num(op - RIGHT1 + 1, True)))
        elif op == W0:
            out.append(("w0",))
        elif W1 <= op <= W1 + 3:
            out.append(("w", num(op - W1 + 1, True)))
        elif op == X0:
            out.append(("x0",))
        elif X1 <= op <= X1 + 3:
            out.append(("x", num(op - X1 + 1, True)))
        elif DOWN1 <= op <= DOWN1 + 3:
            out.append(("down", num(op - DOWN1 + 1, True)))
        elif op == Y0:
            out.append(("y0",))
        elif Y1 <= op <= Y1 + 3:
            out.append(("y", num(op - Y1 + 1, True)))
        elif op == Z0:
            out.append(("z0",))
        elif Z1 <= op <= Z1 + 3:
            out.append(("z", num(op - Z1 + 1, True)))
        elif FNT_NUM_0 <= op < FNT1:
            out.append(("fnt", op - FNT_NUM_0))
        elif FNT1 <= op <= FNT1 + 3:
            out.append(("fnt", num(op - FNT1 + 1)))
        elif XXX1 <= op <= XXX1 + 3:
            k = num(op - XXX1 + 1)
            out.append(("xxx", bytes(data[i:i + k])))
            i += k
        elif FNT_DEF1 <= op <= FNT_DEF1 + 3:
            f = num(op - FNT_DEF1 + 1)
            checksum = num(4)
            at = num(4, True)
            design = num(4, True)
            a = num(1); l = num(1)
            name = bytes(data[i + a:i + a + l])   # area は探索経路なので落とす
            i += a + l
            out.append(("fnt_def", f, checksum, at, design, name))
        else:
            out.append(("unknown", op))
    return out


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    a = parse(open(sys.argv[1], "rb").read())
    b = parse(open(sys.argv[2], "rb").read())
    if a == b:
        print(f"意味は一致した。命令 {len(a):,} 個。")
        return 0
    print(f"命令数 {len(a):,} 対 {len(b):,}")
    for k, (x, y) in enumerate(zip(a, b)):
        if x != y:
            print(f"最初の差 命令 {k}:")
            print(f"  {sys.argv[1]}: {x}")
            print(f"  {sys.argv[2]}: {y}")
            lo = max(0, k - 3)
            print(f"  直前: {a[lo:k]}")
            return 1
    print("片方が長い。短い側の末尾まではすべて一致した。")
    return 1


if __name__ == "__main__":
    sys.exit(main())

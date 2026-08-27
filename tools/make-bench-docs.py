#!/usr/bin/env python3
"""plain TeX ベンチ用の、本文量だけが違う文書を作る。

起動固定費と組版スループットを分けるため、同じ本文を n 回入力するだけの
文書を複数の n で用意する。回帰直線の切片が起動固定費、傾きが真の
組版スループットになる。
"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BENCH = ROOT / "bench"
BENCH.mkdir(exist_ok=True)

SIZES = [1, 2, 5, 10, 20]

for n in SIZES:
    lines = ["\\input body"] * n + ["\\bye", ""]
    (BENCH / f"doc{n}.tex").write_text("\n".join(lines))
    print(f"bench/doc{n}.tex: {n} 本文")

# 起動のみ
(BENCH / "doc0.tex").write_text("\\bye\n")
print("bench/doc0.tex: 起動のみ")

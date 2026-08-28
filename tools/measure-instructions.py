#!/usr/bin/env python3
"""A/B 反復用の高速計測。命令数と DVI の指紋だけを見る。

`bench-document-throughput-linux.sh` は三 engine の end-to-end wall time を測る
正式な gate だが、一回あたりが重い。変更を採るかどうかの判定にはここを使う。

命令数は CPU 周波数にも他 process の負荷にも依存しないので、laptop でも
そのまま比較できる。DVI の指紋が変われば意味が変わっているので採らない。

使い方:
    python3 tools/measure-instructions.py --save base --label "分岐点"
    python3 tools/measure-instructions.py --against base
"""
import argparse
import hashlib
import json
import pathlib
import statistics
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
# 配布する実行 file は `dist` profile で作る。`release` は gate の
# `cargo test --release` を壊さないために `panic = "abort"` を持たない。
BIN = ROOT / "target" / "dist" / "rtex"
RUN = pathlib.Path("/tmp/claude-1000/prun")
STORE = ROOT / "bench-results"
FIXTURE = "bench"

ARGS = [
    "--quiet", "-interaction=batchmode", "-halt-on-error", "-no-shell-escape",
    "-output-comment=bench", "--", "&latex", f"{FIXTURE}.tex",
]


def one_run():
    cmd = ["perf", "stat", "-e", "instructions,task-clock", "-x,", str(BIN), *ARGS]
    p = subprocess.run(cmd, cwd=RUN, stdin=subprocess.DEVNULL,
                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    ins = ms = None
    for line in p.stderr.splitlines():
        f = line.split(",")
        if len(f) >= 3 and f[2] == "instructions":
            ins = float(f[0])
        elif len(f) >= 3 and f[2] == "task-clock":
            ms = float(f[0])
    if ins is None:
        sys.exit(f"perf の出力を読めない:\n{p.stderr}")
    return ins, ms


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repeat", type=int, default=3)
    ap.add_argument("--save")
    ap.add_argument("--against")
    ap.add_argument("--label", default="")
    args = ap.parse_args()

    STORE.mkdir(exist_ok=True)
    one_run()  # warm-up
    runs = [one_run() for _ in range(args.repeat)]
    ins = statistics.median(r[0] for r in runs)
    ms = statistics.median(r[1] for r in runs)
    spread = (max(r[0] for r in runs) - min(r[0] for r in runs)) / ins

    dvi = (RUN / f"{FIXTURE}.dvi").read_bytes()
    digest = hashlib.sha256(dvi).hexdigest()[:16]

    print(f"命令数   {ins:>15,.0f}   (ばらつき {spread*100:.3f}%)")
    print(f"時間     {ms:>15,.1f} ms")
    print(f"DVI      {digest}  ({len(dvi):,} bytes)")

    if args.against:
        base = json.loads((STORE / f"{args.against}.json").read_text())
        print()
        print(f"基準 {args.against}: {base['instructions']:,.0f}  ({base.get('label','')})")
        print(f"命令数比 {ins/base['instructions']:.4f}"
              f"   ({(ins/base['instructions']-1)*100:+.2f}%)")
        if digest != base["dvi"]:
            print("DVI が違う。意味が変わっているので採らない。")
        else:
            print("DVI 一致。")

    if args.save:
        (STORE / f"{args.save}.json").write_text(json.dumps(
            {"instructions": ins, "ms": ms, "dvi": digest, "label": args.label},
            indent=2, ensure_ascii=False))
        print(f"\n保存: bench-results/{args.save}.json")


if __name__ == "__main__":
    main()

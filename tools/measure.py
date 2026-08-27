#!/usr/bin/env python3
"""A/B 反復用の高速計測。命令数だけを見る。

命令数は CPU 周波数にも他 process の負荷にも依存しないので、この laptop でも
そのまま判定に使える。壁時計時間は参考として併記するだけにする。

DVI の byte 一致も同時に確かめる。rtex 対 rtex なので、意味が変わっていなければ
byte まで一致するはずである。一致しない変更は、速くなっていても採用しない。

使い方:
    python3 tools/measure.py                 # 現在の binary を測る
    python3 tools/measure.py --save base     # 結果を bench/base.json へ保存
    python3 tools/measure.py --against base  # 保存した結果と比較
"""
import argparse
import json
import pathlib
import statistics
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RUN = ROOT / "run-rtex"
BIN = ROOT / "target" / "release" / "rtex"
BENCH = ROOT / "bench"

DOC = "doc20"


def one_run():
    cmd = ["perf", "stat", "-e", "instructions,task-clock", "-x,",
           str(BIN), f"&plain {DOC}.tex"]
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
    ap.add_argument("--repeat", type=int, default=5)
    ap.add_argument("--save")
    ap.add_argument("--against")
    ap.add_argument("--label", default="")
    args = ap.parse_args()

    one_run()  # warm-up
    runs = [one_run() for _ in range(args.repeat)]
    ins = statistics.median(r[0] for r in runs)
    ms = statistics.median(r[1] for r in runs)
    spread = (max(r[0] for r in runs) - min(r[0] for r in runs)) / ins

    dvi = (RUN / f"{DOC}.dvi").read_bytes()
    digest = __import__("hashlib").sha256(dvi).hexdigest()[:16]

    print(f"命令数   {ins:>15,.0f}   (ばらつき {spread*100:.3f}%)")
    print(f"時間     {ms:>15,.1f} ms")
    print(f"DVI      {digest}  ({len(dvi):,} bytes)")

    if args.against:
        base = json.loads((BENCH / f"{args.against}.json").read_text())
        d = ins / base["instructions"] - 1
        print()
        print(f"基準 {args.against}: {base['instructions']:,.0f}  "
              f"({base.get('label','')})")
        print(f"命令数比 {ins/base['instructions']:.4f}   ({d*100:+.2f}%)")
        if digest != base["dvi"]:
            print("DVI が違う。意味が変わっているので採用しない。")
        else:
            print("DVI 一致。")

    if args.save:
        (BENCH / f"{args.save}.json").write_text(json.dumps(
            {"instructions": ins, "ms": ms, "dvi": digest, "label": args.label},
            indent=2, ensure_ascii=False))
        print(f"\n保存: bench/{args.save}.json")


if __name__ == "__main__":
    main()

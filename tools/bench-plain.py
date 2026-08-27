#!/usr/bin/env python3
"""rtex と Knuth TeX を同じ plain 文書で測り、起動固定費と組版スループットを分ける。

このマシンは laptop でサーマルスロットリングと他 process の負荷があるため、
壁時計時間だけでは判断しない。`perf stat` の命令数を主指標にする。命令数は
周波数にも他 process にも依存しないので、A/B の判定に使える。

時間も併記するが、両 engine を交互に走らせた対 (paired) の比だけを見る。
交互にすることで、ゆっくりした drift (発熱、周波数低下) は両者に等しく効き、
比からほぼ消える。

使い方:
    python3 tools/bench-plain.py [--repeat N] [--sizes 0,1,2,5,10,20]
"""
import argparse
import json
import pathlib
import re
import shutil
import statistics
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RTEX_BIN = ROOT / "target" / "release" / "rtex"
RUN_RTEX = ROOT / "run-rtex"
RUN_KNUTH = ROOT / "run-knuth"

PERF_EVENTS = "instructions,cycles,task-clock"


def prepare_dirs(sizes):
    """両 engine 用の実行 directory を作る。入力は同一 byte にする。"""
    for d in (RUN_RTEX, RUN_KNUTH):
        d.mkdir(exist_ok=True)
        shutil.copy(ROOT / "body.tex", d / "body.tex")
        for n in sizes:
            shutil.copy(ROOT / "bench" / f"doc{n}.tex", d / f"doc{n}.tex")
    # rtex は kpathsea を持たないので、format を cwd に置く
    shutil.copy(ROOT / "plain.fmt", RUN_RTEX / "plain.fmt")
    fonts = RUN_RTEX / "fonts"
    if not fonts.exists():
        shutil.copytree(ROOT / "fonts", fonts)


def run_perf(cmd, cwd):
    """perf stat 付きで一回走らせ、(instructions, cycles, task_clock_ms) を返す。"""
    full = ["perf", "stat", "-e", PERF_EVENTS, "-x,", *cmd]
    proc = subprocess.run(
        full, cwd=cwd, stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
    )
    vals = {}
    for line in proc.stderr.splitlines():
        parts = line.split(",")
        if len(parts) < 3:
            continue
        raw, _unit, event = parts[0], parts[1], parts[2]
        try:
            vals[event] = float(raw)
        except ValueError:
            continue
    missing = {"instructions", "cycles", "task-clock"} - vals.keys()
    if missing:
        sys.exit(f"perf の出力に {missing} がない:\n{proc.stderr}")
    return vals["instructions"], vals["cycles"], vals["task-clock"]


def engines():
    return {
        "rtex": (lambda n: [str(RTEX_BIN), f"&plain doc{n}.tex"], RUN_RTEX),
        "knuth": (lambda n: ["tex", f"doc{n}.tex"], RUN_KNUTH),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repeat", type=int, default=7)
    ap.add_argument("--sizes", default="0,1,2,5,10,20")
    ap.add_argument("--warmup", type=int, default=2)
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    sizes = [int(s) for s in args.sizes.split(",")]
    prepare_dirs(sizes)
    eng = engines()

    # warm-up: file cache と分岐予測を温める。結果には使わない。
    for _ in range(args.warmup):
        for name, (mk, cwd) in eng.items():
            run_perf(mk(sizes[-1]), cwd)

    samples = {name: {n: [] for n in sizes} for name in eng}
    for _ in range(args.repeat):
        for n in sizes:
            # 交互に走らせて drift を対で打ち消す
            for name, (mk, cwd) in eng.items():
                samples[name][n].append(run_perf(mk(n), cwd))

    med = {}
    for name in eng:
        med[name] = {}
        for n in sizes:
            ins = statistics.median(s[0] for s in samples[name][n])
            cyc = statistics.median(s[1] for s in samples[name][n])
            ms = statistics.median(s[2] for s in samples[name][n])
            med[name][n] = (ins, cyc, ms)

    print(f"# 標本 {args.repeat} 回 (warm-up {args.warmup})、値は中央値\n")
    hdr = f"{'本文数':>6} | {'rtex 命令':>14} {'Knuth 命令':>14} {'比':>6}"
    hdr += f" | {'rtex ms':>9} {'Knuth ms':>9} {'比':>6}"
    print(hdr)
    print("-" * len(hdr))
    for n in sizes:
        ri, _rc, rm = med["rtex"][n]
        ki, _kc, km = med["knuth"][n]
        print(
            f"{n:>6} | {ri:>14,.0f} {ki:>14,.0f} {ri/ki:>6.3f}"
            f" | {rm:>9.1f} {km:>9.1f} {rm/km:>6.3f}"
        )

    # 起動固定費 (切片) と組版スループット (傾き) を最小二乗で分ける
    print("\n## 起動固定費と組版スループットの分離 (最小二乗)\n")
    fit = {}
    for name in eng:
        xs = [float(n) for n in sizes]
        for idx, metric in enumerate(("命令", "ms")):
            col = 0 if idx == 0 else 2
            ys = [med[name][n][col] for n in sizes]
            mx, my = statistics.fmean(xs), statistics.fmean(ys)
            denom = sum((x - mx) ** 2 for x in xs)
            slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / denom
            intercept = my - slope * mx
            fit[(name, metric)] = (intercept, slope)

    for metric, unit in (("命令", ""), ("ms", " ms")):
        ri, rs = fit[("rtex", metric)]
        ki, ks = fit[("knuth", metric)]
        print(f"### {metric}")
        print(f"  起動固定費   rtex {ri:>14,.1f}{unit}   Knuth {ki:>14,.1f}{unit}"
              f"   比 {ri/ki:.3f}")
        print(f"  本文1つあたり rtex {rs:>14,.1f}{unit}   Knuth {ks:>14,.1f}{unit}"
              f"   比 {rs/ks:.3f}")
        print()

    if args.json:
        out = {
            "repeat": args.repeat,
            "median": {name: {str(n): med[name][n] for n in sizes} for name in eng},
            "fit": {f"{k[0]}/{k[1]}": v for k, v in fit.items()},
        }
        pathlib.Path(args.json).write_text(json.dumps(out, indent=2, ensure_ascii=False))
        print(f"JSON: {args.json}")


if __name__ == "__main__":
    main()

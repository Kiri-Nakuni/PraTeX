#!/usr/bin/env python3
"""`bench-document-throughput-linux.sh` の `runs.tsv` を集計する。

対 (paired) の比を使う。同じ round で並べて比を取り、その中央値を見る。
発熱や他 process による drift は同じ round の両者へほぼ等しく効くので、
比を取ると打ち消される。warm-up は捨てる。

使い方:
    python3 tools/summarize-throughput.py <result ディレクトリ>
"""
import csv
import pathlib
import statistics
import sys


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    path = pathlib.Path(sys.argv[1]) / "runs.tsv"
    rows = [r for r in csv.DictReader(path.open(), delimiter="\t")
            if not r["round"].startswith("warmup")]

    by_fixture = {}
    for r in rows:
        by_fixture.setdefault(r["fixture"], {}).setdefault(r["engine"], {})[r["round"]] = r

    for fixture, engines in sorted(by_fixture.items()):
        print(f"## {fixture}\n")
        names = sorted(engines)
        width = max(len(n) for n in names)
        print(f"{'engine':<{width}}  {'wall 中央値':>12}  {'user 中央値':>12}  {'peak RSS':>10}")
        print("-" * (width + 42))
        for name in names:
            walls = [float(r["wall_s"]) for r in engines[name].values()]
            users = [float(r["user_s"]) for r in engines[name].values()]
            rss = [int(r["peak_rss_kb"]) for r in engines[name].values()]
            print(f"{name:<{width}}  {statistics.median(walls):>10.4f} s"
                  f"  {statistics.median(users):>10.4f} s"
                  f"  {statistics.median(rss):>8,} KiB")

        if "pratex" in engines and "uplatex" in engines:
            rounds = sorted(set(engines["pratex"]) & set(engines["uplatex"]))
            ratios = [float(engines["pratex"][k]["wall_s"]) / float(engines["uplatex"][k]["wall_s"])
                      for k in rounds]
            geo = 1.0
            for x in ratios:
                geo *= x
            geo **= 1.0 / len(ratios)
            print(f"\n  PraTeX / upLaTeX  paired wall 比"
                  f"  中央値 {statistics.median(ratios):.4f}"
                  f"  幾何平均 {geo:.4f}"
                  f"  (標本 {len(ratios)})")
        print()


if __name__ == "__main__":
    main()

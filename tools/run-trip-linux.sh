#!/usr/bin/env bash
# `tools/run-trip.ps1` と同じ手順を Linux で走らせる。
#
# この環境には PowerShell 7 がないため、runner に固定された byte 入力と引数を
# 同じ順で実行する。第三者資材は版方へ入れず、隔離した作業領域へ置く。
# 取り出す十ファイルの SHA-256 は `tests-support/trip/assets.json` と照合する。
#
# 使い方:
#     bash tools/run-trip-linux.sh [作業領域]
set -euo pipefail

for command_name in cargo curl python3 sha256sum unzip pltotf tftopl; do
    command -v "$command_name" >/dev/null || {
        printf '必要な command がない: %s\n' "$command_name" >&2
        exit 1
    }
done

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
work_root=${1:-$(mktemp -d "${TMPDIR:-/tmp}/pratex-trip.XXXXXXXX")}
mkdir -p "$work_root"

case "$work_root" in
    "$repo_root"|"$repo_root"/*)
        printf '作業領域を版方の中へ置かない: %s\n' "$work_root" >&2
        exit 1
        ;;
esac

reference_dir=$work_root/reference
run_dir=$work_root/run
actual_dir=$work_root/actual
target_dir=$work_root/target
mkdir -p "$reference_dir" "$run_dir" "$actual_dir" "$target_dir"

archive=$work_root/tex.zip
manifest=$repo_root/tests-support/trip/assets.json

printf '== 資材の取得と検証 ==\n'
if [ ! -f "$archive" ]; then
    url=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['archive_url'])" "$manifest")
    curl -fsSL -o "$archive" "$url"
fi
printf 'archive SHA-256: %s\n' "$(sha256sum "$archive" | cut -d' ' -f1)"

names=$(python3 -c "
import json, sys
for f in json.load(open(sys.argv[1]))['files']:
    print(f['name'])
" "$manifest")

for name in $names; do
    unzip -o -j -q "$archive" "*/$name" -d "$reference_dir" 2>/dev/null \
        || unzip -o -j -q "$archive" "$name" -d "$reference_dir"
done

python3 - "$manifest" "$reference_dir" <<'PY'
import hashlib, json, pathlib, sys
manifest, reference = sys.argv[1], pathlib.Path(sys.argv[2])
bad = []
for entry in json.load(open(manifest))["files"]:
    path = reference / entry["name"]
    if not path.exists():
        bad.append(f"{entry['name']}: 取り出せていない")
        continue
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != entry["sha256"]:
        bad.append(f"{entry['name']}: {digest} != {entry['sha256']}")
if bad:
    print("SHA-256 が合わない:"); [print(" ", b) for b in bad]; sys.exit(1)
print(f"十資材すべて SHA-256 一致")
PY

printf '\n== trip feature の build ==\n'
CARGO_TARGET_DIR=$target_dir CARGO_BUILD_JOBS=${TRIP_BUILD_JOBS:-1} \
    cargo build --release --features trip --locked --manifest-path "$repo_root/Cargo.toml" >/dev/null
rtex=$target_dir/release/rtex
test -x "$rtex" || { printf 'build できていない\n' >&2; exit 1; }
printf '実行 file SHA-256: %s\n' "$(sha256sum "$rtex" | cut -d' ' -f1)"

printf '\n== Appendix A step 1: PLtoTF -> TFtoPL の往復 ==\n'
(cd "$run_dir" && pltotf "$reference_dir/trip.pl" trip.tfm >/dev/null)
(cd "$run_dir" && tftopl trip.tfm "$actual_dir/trip.roundtrip.pl" >/dev/null)
if diff -q <(tr -d '\r' <"$reference_dir/trip.pl") <(tr -d '\r' <"$actual_dir/trip.roundtrip.pl") >/dev/null; then
    printf '往復 PL は公式 trip.pl と一致\n'
else
    printf '往復 PL が公式 trip.pl と一致しない\n' >&2
    exit 1
fi
printf '生成 TFM SHA-256: %s\n' "$(sha256sum "$run_dir/trip.tfm" | cut -d' ' -f1)"

cp -- "$reference_dir/trip.tex" "$run_dir/trip.tex"

printf '\n== 1段目 (INITEX) ==\n'
printf '\n\\input trip\n' >"$actual_dir/stage1.stdin"
set +e
(cd "$run_dir" && "$rtex" <"$actual_dir/stage1.stdin" \
    >"$actual_dir/tripin.fot" 2>"$actual_dir/stage1.stderr")
stage1_exit=$?
set -e
printf 'exit %s\n' "$stage1_exit"
for required in trip.log trip.fmt; do
    test -f "$run_dir/$required" || { printf '%s ができていない\n' "$required" >&2; exit 1; }
done
cp -- "$run_dir/trip.log" "$actual_dir/tripin.log"

printf '\n== 2段目 ==\n'
printf ' &trip  trip \n' >"$actual_dir/stage2.stdin"
set +e
(cd "$run_dir" && "$rtex" <"$actual_dir/stage2.stdin" \
    >"$actual_dir/trip.fot" 2>"$actual_dir/stage2.stderr")
stage2_exit=$?
set -e
printf 'exit %s\n' "$stage2_exit"
for required in trip.log trip.dvi tripos.tex 8terminal.tex; do
    test -f "$run_dir/$required" || { printf '%s ができていない\n' "$required" >&2; exit 1; }
    cp -- "$run_dir/$required" "$actual_dir/$required"
done
printf '8terminal.tex は %s byte\n' "$(stat -c %s "$actual_dir/8terminal.tex")"
printf 'DVI SHA-256: %s\n' "$(sha256sum "$actual_dir/trip.dvi" | cut -d' ' -f1)"

printf '\n== 比較 ==\n'
for name in tripos.tex; do
    if diff -q <(tr -d '\r' <"$reference_dir/$name") <(tr -d '\r' <"$actual_dir/$name") >/dev/null; then
        printf '%-12s 一致\n' "$name"
    else
        printf '%-12s 差分あり\n' "$name"
    fi
done

printf '\n作業領域: %s\n' "$work_root"
printf 'DVI の意味比較は tools/compare-dvi-semantics.py で行う。\n'

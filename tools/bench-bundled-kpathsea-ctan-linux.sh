#!/usr/bin/env bash

# Measure PraTeX on a pinned, externally prepared flat CTAN runtime without
# mixing Windows/WSL filesystem latency into the engine timings.
#
# The input runtime is copied to an ext4 temporary directory in two forms:
#
#   local: every runtime file is in the document working directory;
#   tree:  the document and PraTeX-private fmt are local; source and metric
#          dependencies are found through a minimal texmf.cnf + ls-R tree by
#          the bundled Kpathsea resolver.  PraTeX intentionally does not load
#          its incompatible fmt format from a general TeX Live tree.
#
# The script never downloads or vendors CTAN material.  A caller must supply
# the verified flat runtime produced by tools/test-prjsarticle.ps1 (or an
# equivalent external fixture) and the release PraTeX binary to measure.

set -euo pipefail

for command_name in awk cat cp find grep install mkdir mktemp rm sha256sum sort strace tail time wc; do
    command -v "$command_name" >/dev/null || {
        printf 'required command is missing: %s\n' "$command_name" >&2
        exit 1
    }
done

pratex=${PRATEX_PERF_BINARY:?set PRATEX_PERF_BINARY to a release PraTeX binary}
flat_runtime=${PRATEX_PERF_RUNTIME:?set PRATEX_PERF_RUNTIME to a verified flat CTAN runtime}
runs=${PRATEX_PERF_RUNS:-7}
document=${PRATEX_PERF_DOCUMENT:-prjsarticle-sample.tex}
source_date_epoch=${SOURCE_DATE_EPOCH:-1709210096}

test -x "$pratex" || {
    printf 'PraTeX binary is not executable: %s\n' "$pratex" >&2
    exit 1
}
test -d "$flat_runtime" || {
    printf 'flat CTAN runtime does not exist: %s\n' "$flat_runtime" >&2
    exit 1
}
test -f "$flat_runtime/$document" || {
    printf 'benchmark document is missing from the flat runtime: %s\n' "$document" >&2
    exit 1
}
case "$runs" in
    '' | *[!0-9]* | 0)
        printf 'PRATEX_PERF_RUNS must be a positive integer: %s\n' "$runs" >&2
        exit 1
        ;;
esac

if test -n "${PRATEX_PERF_WORK_ROOT:-}"; then
    work_root=$PRATEX_PERF_WORK_ROOT
    mkdir -p "$work_root"
    keep_work=1
else
    work_root=$(mktemp -d "${TMPDIR:-/tmp}/pratex-ctan-perf.XXXXXXXX")
    keep_work=${PRATEX_KEEP_PERF_WORK:-0}
fi

marker=$work_root/.pratex-ctan-perf-v1
if test -e "$marker"; then
    printf 'refusing to reuse a populated performance work root: %s\n' "$work_root" >&2
    exit 1
fi
if test "$(find "$work_root" -mindepth 1 -maxdepth 1 | wc -l)" -ne 0; then
    printf 'performance work root must be empty: %s\n' "$work_root" >&2
    exit 1
fi
printf 'pratex-ctan-perf-v1\n' >"$marker"

cleanup() {
    status=$?
    trap - EXIT
    if test "$status" -ne 0 && test -n "${result_dir:-}" && test -d "$result_dir"; then
        printf 'benchmark failed; captured stderr tails follow\n' >&2
        while IFS= read -r error_file; do
            printf '%s:\n' "$error_file" >&2
            tail -n 40 "$error_file" >&2
        done < <(find "$result_dir" -maxdepth 1 -type f -name '*.stderr' | sort)
        if test -n "${PRATEX_PERF_RESULT_DIR:-}"; then
            mkdir -p "$PRATEX_PERF_RESULT_DIR"
            if test "$(find "$PRATEX_PERF_RESULT_DIR" -mindepth 1 -maxdepth 1 | wc -l)" -eq 0; then
                cp -a "$result_dir/." "$PRATEX_PERF_RESULT_DIR/"
            fi
        fi
    fi
    if test "$keep_work" = 1; then
        printf 'performance work tree retained: %s\n' "$work_root"
    else
        rm -rf -- "$work_root"
    fi
    exit "$status"
}
trap cleanup EXIT
if test -n "${PRATEX_PERF_RESULT_DIR:-}"; then
    mkdir -p "$PRATEX_PERF_RESULT_DIR"
    if test "$(find "$PRATEX_PERF_RESULT_DIR" -mindepth 1 -maxdepth 1 | wc -l)" -ne 0; then
        printf 'PRATEX_PERF_RESULT_DIR must be empty: %s\n' "$PRATEX_PERF_RESULT_DIR" >&2
        exit 1
    fi
fi

bin_dir=$work_root/bin
local_run=$work_root/local
tree_run=$work_root/tree-run
format_run=$work_root/format-run
texmf_dist=$work_root/texmf-dist
tex_dir=$texmf_dist/tex/pratex
tfm_dir=$texmf_dist/fonts/tfm/public/pratex
cnf_dir=$texmf_dist/web2c
result_dir=$work_root/result
mkdir -p \
    "$bin_dir" "$local_run" "$tree_run" "$format_run" \
    "$tex_dir" "$tfm_dir" "$cnf_dir" "$result_dir"
install -m 755 "$pratex" "$bin_dir/pratex"
pratex=$bin_dir/pratex

is_tex_runtime_file() {
    case "$1" in
        *.tex | *.ltx | *.cfg | *.def | *.fd | *.cls | *.clo | *.sty | *.txt | *.dat)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

while IFS= read -r -d '' source; do
    name=${source##*/}
    case "$name" in
        *.tfm)
            cp -- "$source" "$local_run/$name"
            cp -- "$source" "$tfm_dir/$name"
            ;;
        *)
            if is_tex_runtime_file "$name"; then
                cp -- "$source" "$local_run/$name"
                cp -- "$source" "$tex_dir/$name"
            fi
            ;;
    esac
done < <(find "$flat_runtime" -maxdepth 1 -type f -print0)
cp -- "$flat_runtime/$document" "$tree_run/$document"

printf '%s\n' \
    "TEXMFROOT = $texmf_dist" \
    'TEXMF = $TEXMFROOT' \
    'TEXMFDBS = $TEXMFROOT' >"$cnf_dir/texmf.cnf"

write_ls_r() {
    {
        printf '%% ls-R -- filename database for kpathsea; do not change this line.\n'
        printf './tex/pratex:\n'
        find "$tex_dir" -maxdepth 1 -type f -printf '%f\n' | sort
        printf '\n./fonts/tfm/public/pratex:\n'
        find "$tfm_dir" -maxdepth 1 -type f -printf '%f\n' | sort
    } >"$texmf_dist/ls-R"
}
write_ls_r

common_env=(
    "SOURCE_DATE_EPOCH=$source_date_epoch"
    "TEXMFCNF=$cnf_dir"
    "TEXMFDBS=$texmf_dist"
    "TEXINPUTS=!!$texmf_dist/tex//"
    "TEXINPUTS.pratex=!!$texmf_dist/tex//"
    "TFMFONTS=!!$texmf_dist/fonts/tfm//"
)

(
    cd "$format_run"
    env "${common_env[@]}" \
        /usr/bin/time -f 'format\t%e\t%U\t%S\t%M' \
        -o "$result_dir/format.tsv" \
        "$pratex" --quiet -- latex.ltx \
        >"$result_dir/format.stdout" 2>"$result_dir/format.stderr"
)
test -s "$format_run/latex.fmt" || {
    printf 'PraTeX did not produce latex.fmt\n' >&2
    exit 1
}
cp -- "$format_run/latex.fmt" "$local_run/latex.fmt"
cp -- "$format_run/latex.fmt" "$tree_run/latex.fmt"
{
    printf 'key\tvalue\n'
    printf 'pratex_sha256\t%s\n' "$(sha256sum "$pratex" | awk '{print $1}')"
    printf 'format_sha256\t%s\n' "$(sha256sum "$format_run/latex.fmt" | awk '{print $1}')"
    printf 'format_bytes\t%s\n' "$(wc -c <"$format_run/latex.fmt")"
    printf 'document_sha256\t%s\n' "$(sha256sum "$flat_runtime/$document" | awk '{print $1}')"
    printf 'runtime_file_count\t%s\n' "$(find "$flat_runtime" -maxdepth 1 -type f | wc -l)"
    printf 'source_date_epoch\t%s\n' "$source_date_epoch"
} >"$result_dir/provenance.tsv"

run_document() {
    local case_name=$1
    local round=$2
    local directory=$3
    (
        cd "$directory"
        env "${common_env[@]}" \
            /usr/bin/time -f "$case_name\t$round\t%e\t%U\t%S\t%M" \
            -a -o "$result_dir/runs.tsv" \
            "$pratex" --quiet -- '&latex' "$document" \
            >"$result_dir/$case_name.stdout" 2>"$result_dir/$case_name.stderr"
        test -s "${document%.tex}.dvi"
        printf '%s\t%s\t%s\n' \
            "$case_name" "$round" \
            "$(sha256sum "${document%.tex}.dvi" | awk '{print $1}')" \
            >>"$result_dir/dvi-sha256.tsv"
    )
}

# Stabilize LaTeX auxiliary files before recording alternating warm samples.
run_document local warmup "$local_run"
run_document tree warmup "$tree_run"
printf 'case\tround\twall_s\tuser_s\tsys_s\tpeak_rss_kb\n' >"$result_dir/runs.header.tsv"

round=1
while test "$round" -le "$runs"; do
    if test $((round % 2)) -eq 1; then
        run_document local "$round" "$local_run"
        run_document tree "$round" "$tree_run"
    else
        run_document tree "$round" "$tree_run"
        run_document local "$round" "$local_run"
    fi
    round=$((round + 1))
done

(
    cd "$tree_run"
    strace -f -qq -e trace=process -o "$result_dir/process.trace" \
        /usr/bin/env "${common_env[@]}" \
        "$pratex" --quiet -- '&latex' "$document" \
        >"$result_dir/trace.stdout" 2>"$result_dir/trace.stderr"
)
if grep -Eq 'clone3?\(|fork\(|vfork\(' "$result_dir/process.trace"; then
    printf 'PraTeX spawned a child process during the CTAN tree benchmark\n' >&2
    grep -E 'clone3?\(|fork\(|vfork\(' "$result_dir/process.trace" >&2
    exit 1
fi

for case_name in local tree; do
    unique_hashes=$(awk -F '\t' -v c="$case_name" '$1 == c && $2 != "warmup" {print $3}' \
        "$result_dir/dvi-sha256.tsv" | sort -u | wc -l)
    test "$unique_hashes" -eq 1 || {
        printf '%s DVI hash changed across measured rounds\n' "$case_name" >&2
        exit 1
    }
done
local_hash=$(awk -F '\t' '$1 == "local" && $2 != "warmup" {print $3; exit}' \
    "$result_dir/dvi-sha256.tsv")
tree_hash=$(awk -F '\t' '$1 == "tree" && $2 != "warmup" {print $3; exit}' \
    "$result_dir/dvi-sha256.tsv")
test "$local_hash" = "$tree_hash" || {
    printf 'local and tree DVI hashes differ: %s != %s\n' "$local_hash" "$tree_hash" >&2
    exit 1
}

cp -- "$result_dir/runs.header.tsv" "$result_dir/all-runs.tsv"
cat "$result_dir/runs.tsv" >>"$result_dir/all-runs.tsv"
published_result=$result_dir
if test -n "${PRATEX_PERF_RESULT_DIR:-}"; then
    published_result=$PRATEX_PERF_RESULT_DIR
    cp -a "$result_dir/." "$published_result/"
fi
printf 'PraTeX CTAN benchmark passed: rounds=%s child_processes=0 DVI_SHA256=%s\n' \
    "$runs" "$local_hash"
printf 'raw measurements: %s\n' "$published_result/all-runs.tsv"

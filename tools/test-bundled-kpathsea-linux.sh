#!/usr/bin/env bash

# Linux default-feature gate for PraTeX's statically bundled Kpathsea 6.4.2.
# The tree is synthetic and repository-external: no TeX Live font asset is
# copied or vendored. Resolver hit/miss coverage does not parse the fake metric
# bytes; the end-to-end DVI document deliberately needs no font.

set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$repo_root"
cargo_target=${PRATEX_BUNDLED_KPATHSEA_TARGET:-"${TMPDIR:-/tmp}/pratex-bundled-kpathsea-target"}

for command_name in cargo cmp cut grep ldd sha256sum sort strace wc; do
    command -v "$command_name" >/dev/null || {
        printf 'required command is missing: %s\n' "$command_name" >&2
        exit 1
    }
done

gate_root=$(mktemp -d "${TMPDIR:-/tmp}/pratex-bundled-kpathsea.XXXXXXXX")
cleanup() {
    if test "${PRATEX_KEEP_BUNDLED_KPATHSEA_GATE:-0}" = 1; then
        printf 'gate work tree retained: %s\n' "$gate_root"
    else
        rm -rf -- "$gate_root"
    fi
}
trap cleanup EXIT

texmf_dist="$gate_root/texmf-dist"
texmf_cnf="$texmf_dist/web2c"
tex_dir="$texmf_dist/tex/pratex"
tfm_dir="$texmf_dist/fonts/tfm/public/pratex"
vf_dir="$texmf_dist/fonts/vf/public/pratex"
mkdir -p \
    "$texmf_cnf" \
    "$tex_dir" \
    "$tfm_dir" \
    "$vf_dir" \
    "$gate_root/native" \
    "$gate_root/local" \
    "$gate_root/trace" \
    "$gate_root/no-tools"

tex_fixture="$tex_dir/bundled-kpathsea-gate.tex"
tfm_fixture="$tfm_dir/synthetic-latin.tfm"
jfm_fixture="$tfm_dir/synthetic-jfm.tfm"
vf_fixture="$vf_dir/synthetic-jfm.vf"

printf '%s\n' \
    '\catcode123=1' \
    '\catcode125=2' \
    '\batchmode' \
    '\shipout\vbox{\hrule width 10pt height 2pt}' \
    '\end' >"$tex_fixture"
printf 'synthetic latin metric\n' >"$tfm_fixture"
printf 'synthetic Japanese metric\n' >"$jfm_fixture"
printf 'synthetic virtual font\n' >"$vf_fixture"

printf '%s\n' \
    "TEXMFROOT = $texmf_dist" \
    'TEXMF = $TEXMFROOT' \
    'TEXMFDBS = $TEXMFROOT' >"$texmf_cnf/texmf.cnf"

printf '%s\n' \
    '% ls-R -- filename database for kpathsea; do not change this line.' \
    './tex/pratex:' \
    'bundled-kpathsea-gate.tex' \
    '' \
    './fonts/tfm/public/pratex:' \
    'synthetic-latin.tfm' \
    'synthetic-jfm.tfm' \
    '' \
    './fonts/vf/public/pratex:' \
    'synthetic-jfm.vf' >"$texmf_dist/ls-R"

common_env=(
    "TEXMFCNF=$texmf_cnf"
    "TEXMFDBS=$texmf_dist"
    "TEXINPUTS=$gate_root/no-generic-tex"
    "TEXINPUTS.pratex=!!$texmf_dist/tex//"
    "TFMFONTS=!!$texmf_dist/fonts/tfm//"
    "VFFONTS=!!$texmf_dist/fonts/vf//"
    "SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-1709210096}"
)

CARGO_TARGET_DIR="$cargo_target" \
    cargo build --manifest-path "$repo_root/Cargo.toml" --release --locked

pratex="$cargo_target/release/pratex"
test -x "$pratex"
if ldd "$pratex" | grep -F 'libkpathsea' >/dev/null; then
    printf 'default PraTeX unexpectedly depends on a shared libkpathsea\n' >&2
    ldd "$pratex" >&2
    exit 1
fi

env \
    "${common_env[@]}" \
    "CARGO_TARGET_DIR=$cargo_target" \
    PRATEX_TEST_BUNDLED_KPATHSEA=1 \
    "PRATEX_KPATHSEA_EXPECT_TEX=$tex_fixture" \
    "PRATEX_KPATHSEA_EXPECT_TFM=$tfm_fixture" \
    "PRATEX_KPATHSEA_EXPECT_JFM=$jfm_fixture" \
    "PRATEX_KPATHSEA_EXPECT_VF=$vf_fixture" \
    cargo test --manifest-path "$repo_root/Cargo.toml" --release --locked --lib \
    'file_search::in_process::tests::組込みkpathseaがpratex名でtexとtfmとjfmとvfのhit_missを分ける' \
    -- --ignored --exact

(
    cd "$gate_root/native"
    env "${common_env[@]}" "PATH=$gate_root/no-tools" \
        "$pratex" bundled-kpathsea-gate.tex
)

cp "$tex_fixture" "$gate_root/local/"
(
    cd "$gate_root/local"
    env "${common_env[@]}" "PATH=$gate_root/no-tools" \
        "$pratex" bundled-kpathsea-gate.tex
)

native_dvi="$gate_root/native/bundled-kpathsea-gate.dvi"
local_dvi="$gate_root/local/bundled-kpathsea-gate.dvi"
cmp "$native_dvi" "$local_dvi"

(
    cd "$gate_root/trace"
    strace -f -qq -e trace=process -o process.trace \
        /usr/bin/env "${common_env[@]}" "PATH=$gate_root/no-tools" \
        "$pratex" bundled-kpathsea-gate.tex
)

process_trace="$gate_root/trace/process.trace"
if grep -Eq 'clone3?\(|fork\(|vfork\(' "$process_trace"; then
    printf 'PraTeX spawned a child process during the bundled lookup gate:\n' >&2
    grep -E 'clone3?\(|fork\(|vfork\(' "$process_trace" >&2
    exit 1
fi
pid_count=$(cut -d ' ' -f 1 "$process_trace" | sort -u | wc -l)
test "$pid_count" -eq 1 || {
    printf 'expected one traced PID, got %s\n' "$pid_count" >&2
    exit 1
}

dvi_sha256=$(sha256sum "$native_dvi" | cut -d ' ' -f 1)
expected_dvi_sha256=658ec798192d67c3a067b8296a3300e580b2aaf7ba8b4fcc04dab78022848993
test "$dvi_sha256" = "$expected_dvi_sha256" || {
    printf 'unexpected bundled Kpathsea gate DVI SHA-256: %s\n' \
        "$dvi_sha256" >&2
    exit 1
}
printf 'Bundled Kpathsea gate passed: child processes 0; DVI SHA-256 %s\n' \
    "$dvi_sha256"

#!/usr/bin/env bash

# Explicit system-library Kpathsea gate for PraTeX on Linux.
#
# This runner does not download or build Kpathsea. Point it at an external
# prefix made from the recorded TeX Live 2026 source and at a TeX tree with
# the official CM and uptex-fonts assets listed in docs/kpathsea-port-notes.md.

set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$repo_root"

kpathsea_prefix=${PRATEX_KPATHSEA_PREFIX:?set PRATEX_KPATHSEA_PREFIX to the external Kpathsea 6.4.2 prefix}
texmf_dist=${PRATEX_KPATHSEA_TEXMF_DIST:-"$kpathsea_prefix/share/texmf-dist"}
cargo_target=${PRATEX_KPATHSEA_CARGO_TARGET:-"${TMPDIR:-/tmp}/pratex-system-kpathsea-cargo-target"}
build_jobs=${PRATEX_KPATHSEA_BUILD_JOBS:-1}

for command_name in cargo cmp cut grep ldd pkg-config sha256sum sort strace wc; do
    command -v "$command_name" >/dev/null || {
        printf 'required command is missing: %s\n' "$command_name" >&2
        exit 1
    }
done

kpathsea_lib="$kpathsea_prefix/lib"
pkg_config_path="$kpathsea_lib/pkgconfig"
texmf_cnf="$texmf_dist/web2c"
tex_fixture="$repo_root/tests/fixtures/japanese-live-two-jfm.tex"
cm_tfm="$texmf_dist/fonts/tfm/public/cm/cmr10.tfm"
jfm_mincho="$texmf_dist/fonts/tfm/uptex-fonts/jis/upjisr-h.tfm"
jfm_gothic="$texmf_dist/fonts/tfm/uptex-fonts/jis/upjisg-h.tfm"
vf_mincho="$texmf_dist/fonts/vf/uptex-fonts/jis/upjisr-h.vf"

for required_file in \
    "$texmf_cnf/texmf.cnf" \
    "$tex_fixture" \
    "$cm_tfm" \
    "$jfm_mincho" \
    "$jfm_gothic" \
    "$vf_mincho"
do
    test -f "$required_file" || {
        printf 'required gate fixture is missing: %s\n' "$required_file" >&2
        exit 1
    }
done

kpathsea_version=$(PKG_CONFIG_PATH="$pkg_config_path" pkg-config --modversion kpathsea)
test "$kpathsea_version" = 6.4.2 || {
    printf 'expected Kpathsea 6.4.2, got %s\n' "$kpathsea_version" >&2
    exit 1
}

gate_root=$(mktemp -d "${TMPDIR:-/tmp}/pratex-system-kpathsea.XXXXXXXX")
cleanup() {
    if test "${PRATEX_KEEP_KPATHSEA_GATE:-0}" = 1; then
        printf 'gate work tree retained: %s\n' "$gate_root"
    else
        rm -rf -- "$gate_root"
    fi
}
trap cleanup EXIT

mkdir -p "$gate_root/native" "$gate_root/local" "$gate_root/trace"
runtime_library_path="$kpathsea_lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
tex_inputs="$repo_root/tests/fixtures//"
tfm_fonts="$texmf_dist/fonts/tfm//"
vf_fonts="$texmf_dist/fonts/vf//"
source_date_epoch=${SOURCE_DATE_EPOCH:-1709210096}

PKG_CONFIG_PATH="$pkg_config_path" \
    CARGO_TARGET_DIR="$cargo_target" \
    CARGO_BUILD_JOBS="$build_jobs" \
    cargo build --manifest-path "$repo_root/Cargo.toml" --release --locked \
        --no-default-features --features stats,system-kpathsea

pratex="$cargo_target/release/pratex"
test -x "$pratex"
LD_LIBRARY_PATH="$runtime_library_path" ldd "$pratex" \
    | grep -F "$kpathsea_lib/libkpathsea.so.6" >/dev/null || {
    printf 'PraTeX is not linked to the requested libkpathsea prefix\n' >&2
    LD_LIBRARY_PATH="$runtime_library_path" ldd "$pratex" >&2
    exit 1
}

common_env=(
    "TEXMFCNF=$texmf_cnf"
    "TEXINPUTS=$gate_root/no-generic-tex"
    "TEXINPUTS.pratex=$tex_inputs"
    "TFMFONTS=$tfm_fonts"
    "VFFONTS=$vf_fonts"
    "LD_LIBRARY_PATH=$runtime_library_path"
    "SOURCE_DATE_EPOCH=$source_date_epoch"
)

env \
    "${common_env[@]}" \
    "PKG_CONFIG_PATH=$pkg_config_path" \
    "CARGO_TARGET_DIR=$cargo_target" \
    "CARGO_BUILD_JOBS=$build_jobs" \
    PRATEX_TEST_SYSTEM_KPATHSEA=1 \
    "PRATEX_KPATHSEA_EXPECT_TEX=$tex_fixture" \
    "PRATEX_KPATHSEA_EXPECT_TFM=$cm_tfm" \
    "PRATEX_KPATHSEA_EXPECT_JFM=$jfm_mincho" \
    "PRATEX_KPATHSEA_EXPECT_VF=$vf_mincho" \
    cargo test --manifest-path "$repo_root/Cargo.toml" --release --locked \
        --no-default-features --features stats,system-kpathsea --lib \
        'file_search::in_process::tests::配布側libkpathseaがpratex名でtexとtfmとjfmとvfのhit_missを分ける' \
        -- --ignored --exact

(
    cd "$gate_root/native"
    env "${common_env[@]}" "$pratex" japanese-live-two-jfm.tex
)

cp "$tex_fixture" "$jfm_mincho" "$jfm_gothic" "$gate_root/local/"
(
    cd "$gate_root/local"
    env \
        "TEXMFCNF=$texmf_cnf" \
        "LD_LIBRARY_PATH=$runtime_library_path" \
        "SOURCE_DATE_EPOCH=$source_date_epoch" \
        "$pratex" japanese-live-two-jfm.tex
)

native_dvi="$gate_root/native/japanese-live-two-jfm.dvi"
local_dvi="$gate_root/local/japanese-live-two-jfm.dvi"
cmp "$native_dvi" "$local_dvi"

(
    cd "$gate_root/trace"
    strace -f -qq -e trace=process -o process.trace \
        env "${common_env[@]}" "$pratex" japanese-live-two-jfm.tex
)

process_trace="$gate_root/trace/process.trace"
if grep -Eq 'clone3?\(|fork\(|vfork\(' "$process_trace"; then
    printf 'PraTeX spawned a child process during the linked lookup gate:\n' >&2
    grep -E 'clone3?\(|fork\(|vfork\(' "$process_trace" >&2
    exit 1
fi
pid_count=$(cut -d ' ' -f 1 "$process_trace" | sort -u | wc -l)
test "$pid_count" -eq 1 || {
    printf 'expected one traced PID, got %s\n' "$pid_count" >&2
    exit 1
}

dvi_sha256=$(sha256sum "$native_dvi" | cut -d ' ' -f 1)
expected_dvi_sha256=49bd1e1cd78832c970e7d6283cee99213cb6e21e8a628fe299484e11d1eb81f9
test "$dvi_sha256" = "$expected_dvi_sha256" || {
    printf 'DVI meaning changed: expected %s, got %s\n' \
        "$expected_dvi_sha256" "$dvi_sha256" >&2
    exit 1
}
printf 'system Kpathsea %s gate passed: child processes 0; DVI SHA-256 %s\n' \
    "$kpathsea_version" "$dvi_sha256"

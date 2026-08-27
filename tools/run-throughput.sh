#!/usr/bin/env bash
# `bench-document-throughput-linux.sh` を、この作業枝の binary と fmt で走らせる。
# LuaLaTeX は別 workload 列なので、ここでは呼ばない。
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

export PRATEX_DOCUMENT_BINARY="${PRATEX_DOCUMENT_BINARY:-$repo_root/target/release/rtex}"
export PRATEX_DOCUMENT_FORMAT="${PRATEX_DOCUMENT_FORMAT:-/tmp/claude-1000/prun/latex.fmt}"
export PRATEX_DOCUMENT_RUNS="${PRATEX_DOCUMENT_RUNS:-9}"
export PRATEX_DOCUMENT_WARMUPS="${PRATEX_DOCUMENT_WARMUPS:-3}"

exec bash "$repo_root/tools/bench-document-throughput-linux.sh" "$@"

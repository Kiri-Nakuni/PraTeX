#!/usr/bin/env bash
# 機械が静かになるまで待つ。強い競合下では engine 間の比そのものが歪むので、
# 正式な wall 比はここを通してから測る。
set -euo pipefail
limit=${1:-2}
while :; do
    load=$(cut -d' ' -f1 /proc/loadavg)
    whole=${load%%.*}
    if [ "$whole" -lt "$limit" ]; then
        printf '負荷が下がった: %s\n' "$(cat /proc/loadavg)"
        exit 0
    fi
    sleep 60
done

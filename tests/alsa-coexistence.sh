#!/usr/bin/env bash
# Run while playback and capture are active on AudioFuse 8Pre (1c75:af0e).
# Verifies real native hardware read & live VU meter coexistence alongside snd-usb-audio / PipeWire.
set -euo pipefail
cli=${AUDIOFUSE_CLI:-target/release/audiofuse-cli}
scratch=$(mktemp -d "${TMPDIR:-/tmp}/audiofuse-coexistence.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
snapshot() {
    local out=$1 node found=0
    mkdir -p "$out"
    amixer -c A8Pre contents > "$out/controls" 2>"$out/amixer-errors"
    test ! -s "$out/amixer-errors"
    awk '/Status:|Interface =|Altset =|Momentary freq =/{print}' /proc/asound/A8Pre/stream0 > "$out/streams"
    test "$(grep -c 'Status: Running' "$out/streams")" -eq 2
    for node in /sys/bus/usb/devices/*; do
        [[ -r "$node/idVendor" && -r "$node/idProduct" ]] || continue
        [[ $(<"$node/idVendor") == 1c75 && $(<"$node/idProduct") == af0e ]] || continue
        for i in 0 1 2; do
            readlink -e "$node:1.$i/driver" >> "$out/drivers"
        done
        found=1
    done
    test "$found" = 1
}
snapshot "$scratch/before"
for action in status clock inputs mixer routing prefs meters dfu; do
    case $action in
        dfu)
            if "$cli" firmware inspect >/dev/null 2>&1; then
                echo "Expected live DFU refusal" >&2
                exit 1
            fi
            ;;
        meters)
            "$cli" --json meters --count 2 --interval-ms 50 > "$scratch/meters.json"
            grep -q '"payload_size_bytes": 152' "$scratch/meters.json"
            grep -q '"vu_unit_id": "0x11"' "$scratch/meters.json"
            ;;
        *)
            "$cli" --json "$action" > "$scratch/$action.json"
            ;;
    esac
    snapshot "$scratch/$action"
    diff -ru "$scratch/before" "$scratch/$action"
    echo "$action: ALSA controls, running streams and driver bindings match"
done

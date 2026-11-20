#!/usr/bin/env bash
# Exercise packaging/audiofuse8pre-dkms against mock dkms/modprobe/uname/systemd-detect-virt
# and a fake module tree. Never touches the host's DKMS state or kernel modules.
set -euo pipefail
cd "$(dirname "$0")/.."
hook="$PWD/packaging/audiofuse8pre-dkms"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
bin="$work/bin"
mkdir -p "$bin"
for k in 6.1.0-old 7.3.4-running; do
    mkdir -p "$work/modules/$k/build"
    : > "$work/modules/$k/build/Makefile"
done
cat > "$bin/dkms" <<'EOF'
#!/bin/sh
echo "dkms $*" >> "$MOCK_LOG"
case "$1" in
    status) [ -n "${MOCK_REGISTERED:-}" ] && echo "audiofuse8pre-ctl/0.2.1: added"; exit 0 ;;
    build) for k in ${MOCK_FAIL_KERNELS:-}; do [ "$5" = "$k" ] && exit 10; done; exit 0 ;;
esac
exit 0
EOF
cat > "$bin/modprobe" <<'EOF'
#!/bin/sh
echo "modprobe $*" >> "$MOCK_LOG"
if [ -n "${MOCK_MODPROBE_REJECT:-}" ]; then
    echo "modprobe: ERROR: could not insert 'audiofuse8pre_ctl': Key was rejected by service" >&2
    exit 1
fi
EOF
printf '#!/bin/sh\necho 7.3.4-running\n' > "$bin/uname"
printf '#!/bin/sh\nexit 1\n' > "$bin/systemd-detect-virt"
chmod +x "$bin"/*

pass=0
run_case() {
    local name=$1
    shift
    : > "$work/log"
    set +e
    env PATH="$bin:$PATH" MOCK_LOG="$work/log" AUDIOFUSE_MODULES_DIR="$work/modules" "$@" \
        sh "$hook" install 2> "$work/err"
    status=$?
    set -e
    echo "$name: exit $status"
}
expect() {
    if ! eval "$1"; then
        echo "FAIL: $1" >&2
        cat "$work/log" "$work/err" >&2
        exit 1
    fi
    pass=$((pass + 1))
}

run_case "clean install"
expect '[ "$status" -eq 0 ]'
expect 'head -2 "$work/log" | grep -q "dkms add"'
expect '[ "$(grep -m1 "dkms build" "$work/log")" = "dkms build --force audiofuse8pre-ctl/0.2.1 -k 7.3.4-running" ]'
expect 'grep -q "modprobe audiofuse8pre_ctl" "$work/log"'
expect '[ ! -s "$work/err" ]'

run_case "secure boot rejects modprobe" MOCK_REGISTERED=1 MOCK_MODPROBE_REJECT=1
expect '[ "$status" -eq 0 ]'
expect '! grep -q "dkms add" "$work/log"'
expect 'grep -q "Secure Boot" "$work/err"'

run_case "older kernel build fails" MOCK_REGISTERED=1 MOCK_FAIL_KERNELS=6.1.0-old
expect '[ "$status" -eq 0 ]'
expect 'grep -q "install --force audiofuse8pre-ctl/0.2.1 -k 7.3.4-running" "$work/log"'
expect 'grep -q "modprobe audiofuse8pre_ctl" "$work/log"'
expect 'grep -q "kernel 6.1.0-old failed" "$work/err"'

run_case "running kernel build fails" MOCK_REGISTERED=1 MOCK_FAIL_KERNELS=7.3.4-running
expect '[ "$status" -eq 0 ]'
expect 'grep -q "build --force audiofuse8pre-ctl/0.2.1 -k 6.1.0-old" "$work/log"'
expect '! grep -q "modprobe" "$work/log"'
expect 'grep -q "kernel 7.3.4-running failed" "$work/err"'

set +e
env PATH="$bin:$PATH" MOCK_LOG="$work/log" sh "$hook" bogus 2>/dev/null
expect '[ "$?" -eq 2 ]'
set -e

: > "$work/log"
env PATH="$bin:$PATH" MOCK_LOG="$work/log" MOCK_REGISTERED=1 sh "$hook" remove
expect 'grep -q "dkms remove audiofuse8pre-ctl/0.2.1 --all" "$work/log"'

echo "dkms-hook: $pass/$pass checks passed"

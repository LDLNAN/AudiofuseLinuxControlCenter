#!/usr/bin/env bash
# Run against the unpacked release source or the worktree; stage sources only.
set -euo pipefail
cd "$(dirname "$0")/.."
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
make install-helper DESTDIR="$stage"
kernelver=$(uname -r)
source kmod/dkms.conf
helper="$stage/usr/src/$PACKAGE_NAME-$PACKAGE_VERSION"
for f in Makefile dkms.conf audiofuse8pre_ctl.c audiofuse8pre_ctl.h audiofuse8pre_policy.h; do
    cmp "kmod/$f" "$helper/$f"
done
test -f tests/alsa-coexistence.sh
test -f kmod/test_policy.c
grep -Fq "MODULE_VERSION(\"$PACKAGE_VERSION\")" "$helper/audiofuse8pre_ctl.c"
grep -Fq '.mode = 0660' "$helper/audiofuse8pre_ctl.c"
grep -F 'KERNEL=="audiofuse8pre-ctl"' udev/70-arturia-audiofuse.rules |
    grep -Fq 'MODE="0660", GROUP="audio", TAG+="uaccess"'
for recipe in packaging/debian/build-deb.sh packaging/rpm/*.spec packaging/arch/PKGBUILD; do
    grep -q 'install-helper' "$recipe"
    grep -q 'alsa-utils' "$recipe"
done
test "$(cat "$stage/usr/lib/modules-load.d/audiofuse8pre-ctl.conf")" = audiofuse8pre_ctl
test -x "$stage/usr/lib/audiofuse-control-center/audiofuse8pre-dkms"
for hook in packaging/debian/build-deb.sh packaging/rpm/*.spec packaging/arch/*.install; do
    grep -Fq '/usr/lib/audiofuse-control-center/audiofuse8pre-dkms install' "$hook"
    grep -Fq '/usr/lib/audiofuse-control-center/audiofuse8pre-dkms remove' "$hook"
done

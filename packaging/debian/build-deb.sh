#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${VERSION:-1.0.0}"
ARCH="${ARCH:-amd64}"
PKG_NAME="audiofuse-control-center_${VERSION}_${ARCH}"
STAGE_DIR="${ROOT_DIR}/target/debian/${PKG_NAME}"

echo "[deb] Building release workspace..."
cargo build --locked --release --workspace --manifest-path "${ROOT_DIR}/Cargo.toml"

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/DEBIAN"
mkdir -p "${STAGE_DIR}/usr/bin"
mkdir -p "${STAGE_DIR}/lib/udev/rules.d"
mkdir -p "${STAGE_DIR}/usr/share/applications"
mkdir -p "${STAGE_DIR}/usr/share/icons/hicolor/scalable/apps"

make -C "${ROOT_DIR}" install-helper DESTDIR="${STAGE_DIR}"
install -Dm755 "${CARGO_TARGET_DIR:-${ROOT_DIR}/target}/release/audiofuse-control-center" "${STAGE_DIR}/usr/bin/audiofuse-control-center"
install -Dm755 "${CARGO_TARGET_DIR:-${ROOT_DIR}/target}/release/audiofuse-cli" "${STAGE_DIR}/usr/bin/audiofuse-cli"
install -Dm644 "${ROOT_DIR}/udev/70-arturia-audiofuse.rules" "${STAGE_DIR}/lib/udev/rules.d/70-arturia-audiofuse.rules"
install -Dm644 "${ROOT_DIR}/packaging/audiofuse-control-center.desktop" "${STAGE_DIR}/usr/share/applications/audiofuse-control-center.desktop"
install -Dm644 "${ROOT_DIR}/packaging/audiofuse-control-center.svg" "${STAGE_DIR}/usr/share/icons/hicolor/scalable/apps/audiofuse-control-center.svg"

glibc_min=$(readelf --version-info "${STAGE_DIR}"/usr/bin/* | grep -oE 'GLIBC_[0-9]+(\.[0-9]+)+' | sed 's/GLIBC_//' | sort -Vu | tail -1)
test -n "$glibc_min"
cat > "${STAGE_DIR}/DEBIAN/control" << EOF
Package: audiofuse-control-center
Version: ${VERSION}
Section: sound
Priority: optional
Architecture: ${ARCH}
Depends: libc6 (>= ${glibc_min}), libusb-1.0-0, libasound2, alsa-utils, dkms, udev, libx11-6, libx11-xcb1, libxcursor1, libxi6, libxkbcommon-x11-0, libgl1, libegl1, libwayland-client0
Maintainer: AudioFuse Linux Control Center Contributors <maintainers@localhost>
Description: Native Linux Control Center and CLI for Arturia AudioFuse 8Pre (1c75:af0e)
 Full studio channel strip, 24-channel low-latency hardware cue mixer,
 output/loopback routing matrix, 152-byte VU meter polling, and udev rules
 for the Arturia AudioFuse 8Pre USB Audio Class 2.0 interface.
EOF

cat > "${STAGE_DIR}/DEBIAN/postinst" << 'EOF'
#!/bin/sh
set -e
if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
    udevadm trigger --subsystem-match=usb --attr-match=idVendor=1c75 --attr-match=idProduct=af0e || true
fi
if [ "$1" = configure ]; then /usr/lib/audiofuse-control-center/audiofuse8pre-dkms install; fi
EOF
cat > "${STAGE_DIR}/DEBIAN/prerm" << 'EOF'
#!/bin/sh
set -e
if [ "$1" = remove ]; then /usr/lib/audiofuse-control-center/audiofuse8pre-dkms remove; fi
EOF
chmod 0755 "${STAGE_DIR}/DEBIAN/prerm"
chmod 0755 "${STAGE_DIR}/DEBIAN/postinst"

if command -v dpkg-deb >/dev/null 2>&1; then
    dpkg-deb --build --root-owner-group "${STAGE_DIR}" "${ROOT_DIR}/${PKG_NAME}.deb"
    echo "[deb] Built ${ROOT_DIR}/${PKG_NAME}.deb"
else
    echo "Missing dpkg-deb" >&2
    exit 1
fi

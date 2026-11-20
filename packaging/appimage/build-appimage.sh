#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APPDIR="${ROOT_DIR}/AppDir"
VERSION="${VERSION:-1.0.0}"
ARCH="${ARCH:-x86_64}"

echo "[AppImage] Building release binaries..."
cargo build --locked --release --workspace --manifest-path "${ROOT_DIR}/Cargo.toml"

rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin"
mkdir -p "${APPDIR}/usr/lib/udev/rules.d"
mkdir -p "${APPDIR}/usr/share/applications"
mkdir -p "${APPDIR}/usr/share/icons/hicolor/scalable/apps"

make -C "${ROOT_DIR}" install-helper DESTDIR="${APPDIR}"
install -Dm755 "${CARGO_TARGET_DIR:-${ROOT_DIR}/target}/release/audiofuse-control-center" "${APPDIR}/usr/bin/audiofuse-control-center"
install -Dm755 "${CARGO_TARGET_DIR:-${ROOT_DIR}/target}/release/audiofuse-cli" "${APPDIR}/usr/bin/audiofuse-cli"
install -Dm644 "${ROOT_DIR}/udev/70-arturia-audiofuse.rules" "${APPDIR}/usr/lib/udev/rules.d/70-arturia-audiofuse.rules"
install -Dm644 "${ROOT_DIR}/packaging/audiofuse-control-center.desktop" "${APPDIR}/usr/share/applications/audiofuse-control-center.desktop"
install -Dm644 "${ROOT_DIR}/packaging/audiofuse-control-center.svg" "${APPDIR}/usr/share/icons/hicolor/scalable/apps/audiofuse-control-center.svg"

libusb=$(pkg-config --variable=libdir libusb-1.0)/libusb-1.0.so.0
install -Dm755 "$libusb" "${APPDIR}/usr/lib/libusb-1.0.so.0"
cp "${ROOT_DIR}/packaging/audiofuse-control-center.desktop" "${APPDIR}/audiofuse-control-center.desktop"
cp "${ROOT_DIR}/packaging/audiofuse-control-center.svg" "${APPDIR}/audiofuse-control-center.svg"

cat > "${APPDIR}/AppRun" << 'EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
if [[ "${1:-}" == "--cli" ]]; then
    shift
    exec "${HERE}/usr/bin/audiofuse-cli" "$@"
fi
exec "${HERE}/usr/bin/audiofuse-control-center" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

if command -v appimagetool >/dev/null 2>&1; then
    ARCH="${ARCH}" appimagetool "${APPDIR}" "${ROOT_DIR}/AudioFuse-Control-Center-${VERSION}-${ARCH}.AppImage"
    echo "[AppImage] Built AudioFuse-Control-Center-${VERSION}-${ARCH}.AppImage"
else
    echo "Missing appimagetool; staged AppDir at ${APPDIR}" >&2
    exit 1
fi

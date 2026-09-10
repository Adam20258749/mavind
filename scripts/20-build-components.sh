#!/usr/bin/env bash
# Stage 20 — compile the Mavind components and install them into the rootfs,
# plus the Windows-apps (Wine) integration files.
set -euo pipefail
# shellcheck source=scripts/lib/common.sh
source "$(dirname "$0")/lib/common.sh"
need_root

[ -d "${ROOTFS}" ] || die "no rootfs — run stages 00–10 first"
need_cmd cargo rustc

CARGO_TARGET_DIR="${CACHE_DIR}/cargo-target"
export CARGO_TARGET_DIR
export CARGO_HOME="${CACHE_DIR}/cargo-home"
mkdir -p "${CARGO_TARGET_DIR}" "${CARGO_HOME}"

step "cargo build --release (workspace, opt-level=z)"
# Built on the host/container, which is the same Debian release as the target,
# so the dynamic libs match. Stage 30 verifies with ldd against the rootfs.
( cd "${REPO_ROOT}" && cargo build --release --locked 2>/dev/null || cargo build --release )

BIN="${CARGO_TARGET_DIR}/release"
install_bin() {
  local name="$1"
  [ -f "${BIN}/${name}" ] || die "component not built: ${name}"
  install -Dm755 "${BIN}/${name}" "${ROOTFS}/usr/bin/${name}"
  strip --strip-unneeded "${ROOTFS}/usr/bin/${name}" 2>/dev/null || true
  log "installed /usr/bin/${name} ($(du -h "${ROOTFS}/usr/bin/${name}" | cut -f1))"
}

for c in mavind-shell mavind-system-monitor mavind-windows-apps minder mavind-settings \
         mavind-installer mavind-oobe mpk; do
  install_bin "$c"
done
# CLI alias for the wine core
ln -sf mavind-windows-apps "${ROOTFS}/usr/bin/mavind-wine"

# ---------------------------------------------------------------------------
step "desktop helper scripts"
for s in "${REPO_ROOT}"/desktop/bin/*; do
  [ -e "$s" ] || continue
  install -Dm755 "$s" "${ROOTFS}/usr/bin/$(basename "$s")"
  log "installed /usr/bin/$(basename "$s")"
done

# --------------------------------------------------------------------------
step "installer + OOBE glue"
# privileged backend (called by mavind-installer via pkexec / by hand via sudo)
install -Dm755 "${REPO_ROOT}/installer/mavind-install" "${ROOTFS}/usr/bin/mavind-install"
# OOBE: session bring-up + the apply helper + the service unit
install -Dm755 "${REPO_ROOT}/system/oobe/session"          "${ROOTFS}/usr/lib/mavind/oobe/session"
install -Dm755 "${REPO_ROOT}/system/oobe/mavind-oobe-apply" "${ROOTFS}/usr/bin/mavind-oobe-apply"
install -Dm644 "${REPO_ROOT}/system/oobe/mavind-oobe.service" \
  "${ROOTFS}/usr/lib/systemd/system/mavind-oobe.service"
# GUI launcher for the installer (shown on the live desktop)
install -Dm644 /dev/stdin "${ROOTFS}/usr/share/applications/mavind-installer.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Install Mavind
GenericName=Installer
Comment=Install Mavind on this PC
Exec=mavind-installer
Icon=drive-harddisk
Terminal=false
Categories=System;
StartupNotify=true
EOF
# polkit: let the "sudo" group run the install backend without a password prompt
install -Dm644 /dev/stdin "${ROOTFS}/usr/share/polkit-1/rules.d/20-mavind-installer.rules" <<'EOF'
// mavind-installer calls: pkexec mavind-install --plan <file>
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.policykit.exec" &&
        action.lookup("program") == "/usr/bin/mavind-install" &&
        subject.isInGroup("sudo")) {
        return polkit.Result.YES;
    }
});
EOF

# ---------------------------------------------------------------------------
step "desktop entries, icons, polkit"
install -d "${ROOTFS}/usr/share/applications" \
          "${ROOTFS}/usr/share/icons/hicolor/scalable/apps" \
          "${ROOTFS}/usr/share/mavind"

for d in "${REPO_ROOT}"/desktop/applications/*.desktop \
         "${REPO_ROOT}"/apps/*/data/*.desktop; do
  [ -e "$d" ] || continue
  install -Dm644 "$d" "${ROOTFS}/usr/share/applications/$(basename "$d")"
done

for i in "${REPO_ROOT}"/desktop/assets/icons/*.svg \
         "${REPO_ROOT}"/apps/*/data/icons/*.svg; do
  [ -e "$i" ] || continue
  install -Dm644 "$i" "${ROOTFS}/usr/share/icons/hicolor/scalable/apps/$(basename "$i")"
done

for p in "${REPO_ROOT}"/apps/*/data/*.rules "${REPO_ROOT}"/system/polkit/*.rules; do
  [ -e "$p" ] || continue
  install -Dm644 "$p" "${ROOTFS}/usr/share/polkit-1/rules.d/$(basename "$p")"
done

# ---------------------------------------------------------------------------
step "Windows-apps (Wine) integration"
WINE_SRC="${REPO_ROOT}/compatibility/wine"
# MIME types so file managers show "Open with Mavind Windows Apps"
install -Dm644 "${WINE_SRC}/mime/mavind-windows-apps.xml" \
  "${ROOTFS}/usr/share/mime/packages/mavind-windows-apps.xml"
install -Dm644 "${WINE_SRC}/mime/mavind-windows-apps.desktop" \
  "${ROOTFS}/usr/share/applications/mavind-windows-apps.desktop"
# helper scripts + data used at runtime by mavind-wine
install -d "${ROOTFS}/usr/lib/mavind/wine"
install -Dm755 "${WINE_SRC}/install-dxvk.sh"  "${ROOTFS}/usr/lib/mavind/wine/install-dxvk.sh"
install -Dm755 "${WINE_SRC}/install-vkd3d.sh" "${ROOTFS}/usr/lib/mavind/wine/install-vkd3d.sh"
install -Dm644 "${WINE_SRC}/hints.tsv"        "${ROOTFS}/usr/lib/mavind/wine/hints.tsv"
install -Dm644 "${WINE_SRC}/dxvk.version"     "${ROOTFS}/usr/lib/mavind/wine/dxvk.version"
install -Dm644 "${WINE_SRC}/vkd3d.version"    "${ROOTFS}/usr/lib/mavind/wine/vkd3d.version"
install -Dm644 "${WINE_SRC}/prefix-baseline.txt" \
  "${ROOTFS}/usr/lib/mavind/wine/prefix-baseline.txt"

# default MIME association
install -d "${ROOTFS}/etc/xdg"
cat > "${ROOTFS}/etc/xdg/mimeapps.list" <<'EOF'
[Default Applications]
application/x-ms-dos-executable=mavind-windows-apps.desktop
application/x-msi=mavind-windows-apps.desktop
application/vnd.microsoft.portable-executable=mavind-windows-apps.desktop
inode/directory=minder.desktop
EOF

# refresh caches inside the image
trap 'chroot_umount "${ROOTFS}"' EXIT
chroot_mount "${ROOTFS}"
in_chroot "${ROOTFS}" bash -c '
  update-mime-database /usr/share/mime  >/dev/null 2>&1 || true
  update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
  gtk-update-icon-cache -qtf /usr/share/icons/hicolor >/dev/null 2>&1 || true
'
chroot_umount "${ROOTFS}"
trap - EXIT

log "stage 20 complete — components in place"

#!/usr/bin/env bash
# Stage 10 — turn the plain Debian rootfs into Mavind:
# branding, users, systemd trimming, session, sysctl/zram, skel, config drop-in.
set -euo pipefail
# shellcheck source=scripts/lib/common.sh
source "$(dirname "$0")/lib/common.sh"
need_root

SYS="${REPO_ROOT}/system"
DESK="${REPO_ROOT}/desktop"

[ -d "${ROOTFS}" ] || die "no rootfs — run stage 00 first"
trap 'chroot_umount "${ROOTFS}"' EXIT
chroot_mount "${ROOTFS}"

# ---------------------------------------------------------------------------
step "branding"
# render os-release with the version stamped by build-iso.sh
_vid="${MAVIND_VERSION_ID:-1.0.2}"
_ver="${MAVIND_VERSION:-1.0.2}"
_bid="${MAVIND_BUILD_ID:-unknown}"
sed -e "s|@VERSION_ID@|${_vid}|g" -e "s|@VERSION@|${_ver}|g" -e "s|@BUILD_ID@|${_bid}|g" \
    "${SYS}/os-release" > "${ROOTFS}/usr/lib/os-release"
ln -sf ../usr/lib/os-release              "${ROOTFS}/etc/os-release"
# machine-readable release info for the installer's "choose version" page
install -d "${ROOTFS}/usr/lib/mavind"
cat > "${ROOTFS}/usr/lib/mavind/release.json" <<EOF
{ "version_id": "${_vid}", "pretty": "Mavind ${_vid}", "build_id": "${_bid}",
  "channel": "${MAVIND_PROFILE:-core}", "arch": "${MAVIND_ARCH:-amd64}" }
EOF
install -Dm644 "${SYS}/issue"             "${ROOTFS}/etc/issue"
install -Dm644 "${SYS}/issue"             "${ROOTFS}/etc/issue.net"
echo "mavind"                            > "${ROOTFS}/etc/hostname"
cat > "${ROOTFS}/etc/hosts" <<'EOF'
127.0.0.1   localhost
127.0.1.1   mavind
::1         localhost ip6-localhost ip6-loopback
EOF
install -Dm644 "${SYS}/motd"              "${ROOTFS}/etc/motd" 2>/dev/null || true

# tier lists for `mpk` (mpk size / mpk install-tier / mpk list --tier)
install -d "${ROOTFS}/usr/lib/mavind/packages"
install -Dm644 "${SYS}/packages/core.list"     "${ROOTFS}/usr/lib/mavind/packages/core.list"
install -Dm644 "${SYS}/packages/compat.list"   "${ROOTFS}/usr/lib/mavind/packages/compat.list"
install -Dm644 "${SYS}/packages/optional.list" "${ROOTFS}/usr/lib/mavind/packages/optional.list"

# ---------------------------------------------------------------------------
step "locale + time"
in_chroot "${ROOTFS}" bash -c '
  echo "LANG=C.UTF-8" > /etc/default/locale
  echo "en_US.UTF-8 UTF-8" > /etc/locale.gen
  command -v locale-gen >/dev/null && locale-gen || true
  ln -sf /usr/share/zoneinfo/UTC /etc/localtime
  echo "UTC" > /etc/timezone
'

# ---------------------------------------------------------------------------
step "users"
# root locked; live user "mavind" / password "mavind" (installer forces a change).
in_chroot "${ROOTFS}" bash -c '
  set -e
  passwd -l root || true
  if ! id mavind >/dev/null 2>&1; then
    useradd --create-home --shell /bin/bash mavind
  fi
  # add to whatever optional groups actually exist (some are package-created)
  for g in sudo audio video input render netdev plugdev bluetooth lp scanner; do
    getent group "$g" >/dev/null 2>&1 && usermod -aG "$g" mavind || true
  done
  echo "mavind:mavind" | chpasswd
  install -d -m0750 -o mavind -g mavind /home/mavind
  # passwordless sudo for the live session only; installer removes this file
  printf "mavind ALL=(ALL) NOPASSWD: ALL\n" > /etc/sudoers.d/90-mavind-live
  chmod 0440 /etc/sudoers.d/90-mavind-live
'

# ---------------------------------------------------------------------------
step "systemd: set target + apply mask-list"
in_chroot "${ROOTFS}" systemctl set-default graphical.target

# Mask units we never want running (RAM + boot time). List lives in the repo.
while read -r unit; do
  [ -n "${unit}" ] || continue
  in_chroot "${ROOTFS}" systemctl mask "${unit}" 2>/dev/null \
    || warn "could not mask ${unit} (not present?)"
done < <(read_list "${SYS}/systemd/mask.list")

# Enable what we do want.
while read -r unit; do
  [ -n "${unit}" ] || continue
  in_chroot "${ROOTFS}" systemctl enable "${unit}" 2>/dev/null \
    || warn "could not enable ${unit}"
done < <(read_list "${SYS}/systemd/enable.list")

# journald: volatile + tiny
install -Dm644 "${SYS}/systemd/journald.conf.d/00-mavind.conf" \
  "${ROOTFS}/etc/systemd/journald.conf.d/00-mavind.conf"
# logind: handle lid/power sanely on old laptops
install -Dm644 "${SYS}/systemd/logind.conf.d/00-mavind.conf" \
  "${ROOTFS}/etc/systemd/logind.conf.d/00-mavind.conf"
# use dbus-broker if present
in_chroot "${ROOTFS}" systemctl enable dbus-broker.service 2>/dev/null || true

# ---------------------------------------------------------------------------
step "kernel cmdline defaults + zram + sysctl"
install -Dm644 "${SYS}/sysctl.d/99-mavind.conf" \
  "${ROOTFS}/etc/sysctl.d/99-mavind.conf"
install -Dm644 "${SYS}/zram/zram-generator.conf" \
  "${ROOTFS}/etc/systemd/zram-generator.conf"
install -Dm644 "${SYS}/modprobe.d/mavind.conf" \
  "${ROOTFS}/etc/modprobe.d/mavind.conf" 2>/dev/null || true
install -Dm644 "${SYS}/modules-load.d/mavind-vm.conf" \
  "${ROOTFS}/etc/modules-load.d/mavind-vm.conf" 2>/dev/null || true

# ---------------------------------------------------------------------------
step "boot splash (Plymouth 'M' theme)"
install -d "${ROOTFS}/usr/share/plymouth/themes/mavind"
install -Dm644 "${SYS}/plymouth/mavind/mavind.plymouth" \
  "${ROOTFS}/usr/share/plymouth/themes/mavind/mavind.plymouth"
install -Dm644 "${SYS}/plymouth/mavind/mavind.script" \
  "${ROOTFS}/usr/share/plymouth/themes/mavind/mavind.script"
# Render the M from SVG (vector, no font dependency -> reliable in the
# initramfs) rather than drawing text with Image.Text, which needs a font the
# initramfs may not have.
if command -v rsvg-convert >/dev/null 2>&1; then
  rsvg-convert -w 512 -h 512 "${SYS}/plymouth/mavind/logo.svg" \
    -o "${ROOTFS}/usr/share/plymouth/themes/mavind/logo.png"
  log "rendered logo.png from logo.svg"
else
  warn "rsvg-convert not found — boot splash will have no image (install librsvg2-bin)"
fi
# keep the last frame on screen until labwc paints (no black flash)
install -Dm644 "${SYS}/plymouth/plymouth-quit.service.d/retain.conf" \
  "${ROOTFS}/etc/systemd/system/plymouth-quit.service.d/retain.conf"
# make it the default theme (stage 40's update-initramfs bakes it in)
in_chroot "${ROOTFS}" plymouth-set-default-theme mavind 2>/dev/null \
  || echo "Theme=mavind" > "${ROOTFS}/etc/plymouth/plymouthd.conf.d/mavind.conf" 2>/dev/null \
  || { install -d "${ROOTFS}/etc/plymouth"; printf '[Daemon]\nTheme=mavind\n' > "${ROOTFS}/etc/plymouth/plymouthd.conf"; }

# Plymouth needs a framebuffer *before* systemd starts. modules-load.d is too
# late (that's systemd-time); force the common VM display drivers straight
# into the initramfs so there is something to draw the splash on.
install -d "${ROOTFS}/etc/initramfs-tools/conf.d"
printf 'FRAMEBUFFER=y\n' > "${ROOTFS}/etc/initramfs-tools/conf.d/mavind-splash.conf"
{
  echo "# Mavind: load early so Plymouth has a framebuffer on VMs"
  cat "${SYS}/modules-load.d/mavind-vm.conf" 2>/dev/null | grep -vE '^\s*#|^\s*$'
} >> "${ROOTFS}/etc/initramfs-tools/modules"

# ---------------------------------------------------------------------------
step "session: greetd + labwc + mavind-session"
# Live ISO autologins; installed system uses greetd (installer flips this).
install -Dm755 "${DESK}/mavind-session"       "${ROOTFS}/usr/bin/mavind-session"
install -Dm644 "${DESK}/mavind-session.desktop" \
  "${ROOTFS}/usr/share/wayland-sessions/mavind.desktop"

# labwc config -> /etc/xdg/labwc (system defaults; user can override in ~/.config)
install -d "${ROOTFS}/etc/xdg/labwc"
install -Dm644 "${DESK}/labwc/rc.xml"      "${ROOTFS}/etc/xdg/labwc/rc.xml"
install -Dm644 "${DESK}/labwc/menu.xml"    "${ROOTFS}/etc/xdg/labwc/menu.xml"
install -Dm644 "${DESK}/labwc/autostart"   "${ROOTFS}/etc/xdg/labwc/autostart"
install -Dm644 "${DESK}/labwc/environment" "${ROOTFS}/etc/xdg/labwc/environment"

# greetd config is staged for the INSTALLED system, but greetd must NOT run on
# the live ISO — it would show a login prompt instead of the desktop. The
# installer enables greetd itself.
install -Dm644 "${SYS}/greetd/config.toml" "${ROOTFS}/etc/greetd/config.toml"
in_chroot "${ROOTFS}" systemctl disable greetd.service 2>/dev/null || true

# Live: autologin mavind on tty1, which execs mavind-session from ~/.bash_profile.
install -Dm644 "${SYS}/systemd/getty-autologin.conf" \
  "${ROOTFS}/etc/systemd/system/getty@tty1.service.d/autologin.conf"
in_chroot "${ROOTFS}" systemctl enable getty@tty1.service 2>/dev/null || true
cat > "${ROOTFS}/home/mavind/.bash_profile" <<'EOF'
# Mavind live: start the desktop on the first console login — but only once.
# MAVIND_SESSION_TRIED is exported before exec, so if mavind-session falls back
# to a shell, this does not re-launch it (which would loop).
if [ -z "${WAYLAND_DISPLAY:-}" ] && [ -z "${SSH_TTY:-}" ] && [ -z "${MAVIND_SESSION_TRIED:-}" ]; then
  case "$(tty)" in
    /dev/tty1)
      export MAVIND_SESSION_TRIED=1
      exec /usr/bin/mavind-session
      ;;
  esac
fi
EOF
in_chroot "${ROOTFS}" chown mavind:mavind /home/mavind/.bash_profile

# ---------------------------------------------------------------------------
step "skel + wallpaper + xdg dirs"
cp -aT "${SYS}/skel" "${ROOTFS}/etc/skel"
install -Dm644 "${DESK}/assets/wallpaper.png" \
  "${ROOTFS}/usr/share/backgrounds/mavind/wallpaper.png" 2>/dev/null || \
  warn "no wallpaper.png yet — shell will use a solid colour"
# make sure the live user gets the skel we just wrote
in_chroot "${ROOTFS}" bash -c 'cp -aT /etc/skel /home/mavind && chown -R mavind:mavind /home/mavind'

# ---------------------------------------------------------------------------
step "firstboot oneshot (regen machine-id, ssh keys, resize, etc.)"
install -Dm755 "${SYS}/firstboot/mavind-firstboot.sh" \
  "${ROOTFS}/usr/lib/mavind/firstboot.sh"
install -Dm644 "${SYS}/firstboot/mavind-firstboot.service" \
  "${ROOTFS}/etc/systemd/system/mavind-firstboot.service"
in_chroot "${ROOTFS}" systemctl enable mavind-firstboot.service
# blank machine-id => systemd regenerates on first boot
: > "${ROOTFS}/etc/machine-id"

# ---------------------------------------------------------------------------
step "cleanup"
rm -f "${ROOTFS}/etc/resolv.conf"        # firstboot/NM will provide it
ln -sf /run/systemd/resolve/stub-resolv.conf "${ROOTFS}/etc/resolv.conf" 2>/dev/null || \
  ln -sf /run/NetworkManager/resolv.conf "${ROOTFS}/etc/resolv.conf" 2>/dev/null || true
rm -rf "${ROOTFS}/var/log/"* "${ROOTFS}/tmp/"* "${ROOTFS}/root/".bash_history 2>/dev/null || true

chroot_umount "${ROOTFS}"
trap - EXIT
log "stage 10 complete — rootfs $(du -sh "${ROOTFS}" | cut -f1)"

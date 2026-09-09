#!/bin/sh
# Runs once on the very first boot of an installed (or dd'd) Mavind system.
set -eu

STAMP=/var/lib/mavind/.firstboot-done
[ -e "$STAMP" ] && exit 0
mkdir -p /var/lib/mavind

echo "mavind: first boot setup"

# 1. Fresh machine-id (stage 10 blanked it).
if [ ! -s /etc/machine-id ]; then
    systemd-machine-id-setup || true
fi

# 2. Grow the root filesystem to fill its partition (installer leaves it exact,
#    but dd-to-USB users benefit).
ROOT_SRC=$(findmnt -no SOURCE / || true)
case "$ROOT_SRC" in
    /dev/*)
        DISK=$(lsblk -no PKNAME "$ROOT_SRC" 2>/dev/null || true)
        PART=$(echo "$ROOT_SRC" | grep -o '[0-9]*$' || true)
        if [ -n "$DISK" ] && [ -n "$PART" ] && command -v growpart >/dev/null 2>&1; then
            growpart "/dev/$DISK" "$PART" || true
            resize2fs "$ROOT_SRC" || true
        fi
        ;;
esac

# 3. Regenerate SSH host keys if the server is ever installed.
[ -x /usr/sbin/sshd ] && rm -f /etc/ssh/ssh_host_* && \
    dpkg-reconfigure openssh-server >/dev/null 2>&1 || true

# 4. Seed a working resolv.conf until NetworkManager takes over.
[ -e /etc/resolv.conf ] || ln -sf /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf || true

# 5. Rebuild caches that may be stale after image assembly.
command -v update-mime-database >/dev/null 2>&1 && update-mime-database /usr/share/mime || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database || true
ldconfig || true

touch "$STAMP"
echo "mavind: first boot setup complete"

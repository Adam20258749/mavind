#!/usr/bin/env bash
# Stage 50 — assemble the bootable hybrid ISO (UEFI + BIOS) with GRUB + live-boot.
set -euo pipefail
# shellcheck source=scripts/lib/common.sh
source "$(dirname "$0")/lib/common.sh"
need_cmd xorriso grub-mkstandalone mformat mmd mcopy
OUT_ISO="${MAVIND_OUT_ISO:-${OUT_DIR}/Mavind.iso}"

LIVE="${WORK_DIR}/live"
[ -f "${LIVE}/filesystem.squashfs" ] || die "no squashfs — run stage 40 first"

rm -rf "${ISO_TREE}"
mkdir -p "${ISO_TREE}/live" "${ISO_TREE}/boot/grub" "${ISO_TREE}/EFI/BOOT"

step "lay out ISO tree"
cp "${LIVE}/filesystem.squashfs"     "${ISO_TREE}/live/"
cp "${LIVE}/filesystem.size"         "${ISO_TREE}/live/" 2>/dev/null || true
cp "${LIVE}/vmlinuz"                 "${ISO_TREE}/live/vmlinuz"
cp "${LIVE}/initrd.img"              "${ISO_TREE}/live/initrd.img"
echo "Mavind $(. "${REPO_ROOT}/system/os-release"; echo "${VERSION_ID:-1.0.2}")" \
  > "${ISO_TREE}/.disk/info" 2>/dev/null || { mkdir -p "${ISO_TREE}/.disk"; echo "Mavind" > "${ISO_TREE}/.disk/info"; }

step "grub menu (mavind.cfg)"
# The real menu is mavind.cfg, NOT grub.cfg. The embedded stub in the standalone
# GRUB binary lives at (memdisk)/boot/grub/grub.cfg; if the real menu were also
# named grub.cfg, `search --file /boot/grub/grub.cfg` would match the memdisk and
# `configfile` would recurse into the stub forever ("maximum recursion depth
# exceeded"). Distinct name + search-by-label avoids that.
sed -e "s/@VOLID@/${MAVIND_VOLID}/g" \
    -e "s/@PROFILE@/${MAVIND_PROFILE}/g" \
    "${REPO_ROOT}/boot/grub/grub.cfg.in" > "${ISO_TREE}/boot/grub/mavind.cfg"
# Fallback grub.cfg on the ISO for any GRUB that auto-loads $prefix/grub.cfg.
printf 'configfile ${prefix}/mavind.cfg\n' > "${ISO_TREE}/boot/grub/grub.cfg"
cp "${REPO_ROOT}/boot/grub/theme.txt"  "${ISO_TREE}/boot/grub/theme.txt" 2>/dev/null || true
cp "${REPO_ROOT}/boot/splash.png"      "${ISO_TREE}/boot/grub/splash.png" 2>/dev/null || true

# Tiny startup config baked into the standalone GRUB (both BIOS + UEFI). It finds
# the live medium by volume label, then loads the real menu.
EMBED="$(mktemp)"
cat > "${EMBED}" <<EOF
set pager=1
search --no-floppy --set=root --label ${MAVIND_VOLID}
if [ ! -f (\$root)/boot/grub/mavind.cfg ]; then
  search --no-floppy --set=root --file /boot/grub/mavind.cfg
fi
if [ -f (\$root)/boot/grub/mavind.cfg ]; then
  set prefix=(\$root)/boot/grub
  configfile (\$root)/boot/grub/mavind.cfg
else
  echo ""
  echo "Mavind: could not find /boot/grub/mavind.cfg (label ${MAVIND_VOLID})."
  echo "Known devices:"
  ls
  echo ""
  echo "Dropping to the GRUB shell in 30s."
  sleep 30
fi
EOF

GRUB_MODS_COMMON="normal linux search search_label search_fs_file iso9660 configfile \
  echo test all_video gfxterm gfxmenu png loadenv part_gpt part_msdos fat ext2 \
  loopback probe cat halt reboot sleep videoinfo"

step "build standalone GRUB (UEFI x86_64)"
grub-mkstandalone \
  --format=x86_64-efi \
  --modules="${GRUB_MODS_COMMON} efi_gop efi_uga" \
  --locales="" --fonts="" --themes="" \
  --output="${ISO_TREE}/EFI/BOOT/BOOTX64.EFI" \
  "boot/grub/grub.cfg=${EMBED}"

step "build GRUB BIOS El Torito image (i386-pc)"
# grub-mkimage is the correct tool for a CD BIOS core; concatenate cdboot.img in
# front to make the bootable El Torito image. Same embedded stub as UEFI: find
# the medium by label, then configfile /boot/grub/mavind.cfg.
# shellcheck disable=SC2086
grub-mkimage \
  --format=i386-pc \
  --output="${WORK_DIR}/core.img" \
  --prefix='/boot/grub' \
  --config="${EMBED}" \
  biosdisk iso9660 ${GRUB_MODS_COMMON}
cat /usr/lib/grub/i386-pc/cdboot.img "${WORK_DIR}/core.img" > "${ISO_TREE}/boot/grub/bios.img"

step "build FAT ESP image for the UEFI El Torito entry"
ESP="${ISO_TREE}/EFI/BOOT/efiboot.img"
ESP_KB=$(( ( $(stat -c%s "${ISO_TREE}/EFI/BOOT/BOOTX64.EFI") / 1024 ) + 128 ))
rm -f "${ESP}"
mformat -i "${ESP}" -C -f "${ESP_KB}" -N 0 ::  2>/dev/null || \
  { dd if=/dev/zero of="${ESP}" bs=1024 count="${ESP_KB}" status=none; mformat -i "${ESP}" -N 0 :: ; }
mmd    -i "${ESP}" ::/EFI ::/EFI/BOOT
mcopy  -i "${ESP}" "${ISO_TREE}/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/BOOTX64.EFI

step "xorriso: hybrid ISO"
rm -f "${OUT_ISO}"
xorriso -as mkisofs \
  -iso-level 3 -full-iso9660-filenames -joliet -rational-rock \
  -volid "${MAVIND_VOLID}" \
  -partition_offset 16 \
  --grub2-mbr /usr/lib/grub/i386-pc/boot_hybrid.img \
  -b boot/grub/bios.img \
    -no-emul-boot -boot-load-size 4 -boot-info-table --grub2-boot-info \
  -eltorito-alt-boot \
  -e EFI/BOOT/efiboot.img \
    -no-emul-boot \
  -append_partition 2 0xef "${ISO_TREE}/EFI/BOOT/efiboot.img" \
  -appended_part_as_gpt \
  ${SOURCE_DATE_EPOCH:+--modification-date="$(date -u -d "@${SOURCE_DATE_EPOCH}" +%Y%m%d%H%M%S00)"} \
  -o "${OUT_ISO}" \
  "${ISO_TREE}"

rm -f "${EMBED}" "${WORK_DIR}/core.img"

sz="$(stat -c%s "${OUT_ISO}")"
log "ISO: ${OUT_ISO}  $(human "${sz}")"
command -v isohybrid >/dev/null && isohybrid --uefi "${OUT_ISO}" 2>/dev/null || true
( cd "$(dirname "${OUT_ISO}")" && sha256sum "$(basename "${OUT_ISO}")" > "$(basename "${OUT_ISO}").sha256" )
log "stage 50 complete"

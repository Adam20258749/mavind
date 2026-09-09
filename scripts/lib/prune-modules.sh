#!/usr/bin/env bash
# Prune a kernel module tree to a keep-list, preserving dependencies.
#   prune-modules.sh <MODDIR> <keeplist>
# <MODDIR> = /usr/lib/modules/<kver>
# keeplist entries are module names (no .ko), one per line; '#' comments ok.
# A trailing '*' globs (e.g. 'r8*' keeps all Realtek eth variants).
set -euo pipefail

MODDIR="${1:?module dir}"
KEEP="${2:?keep list}"
[ -d "${MODDIR}" ] || { echo "no such module dir: ${MODDIR}" >&2; exit 1; }

modules_dep="${MODDIR}/modules.dep"
[ -f "${modules_dep}" ] || { echo "no modules.dep in ${MODDIR}" >&2; exit 1; }

tmp="$(mktemp -d)"; trap 'rm -rf "${tmp}"' EXIT
wanted="${tmp}/wanted"; : > "${wanted}"

# Expand keep-list patterns against actual module basenames.
mapfile -t ALLMODS < <(find "${MODDIR}" -name '*.ko*' -printf '%f\n' | sed 's/\.ko.*$//' | sort -u)
while read -r pat; do
  pat="${pat%%#*}"; pat="$(echo "$pat" | tr -d '[:space:]')"
  [ -n "${pat}" ] || continue
  case "${pat}" in
    *\*) for m in "${ALLMODS[@]}"; do [[ "$m" == ${pat} ]] && echo "$m" >> "${wanted}"; done;;
    *)   echo "${pat}" >> "${wanted}";;
  esac
done < "${KEEP}"

# Always keep the essentials so the machine can boot at all.
cat >> "${wanted}" <<'EOF'
ext4 vfat fat nls_cp437 nls_iso8859-1 nls_ascii isofs overlay squashfs loop
crc32c_intel crc32_pclmul nvme nvme_core sd_mod sr_mod ahci libahci
virtio virtio_pci virtio_blk virtio_scsi virtio_net virtio_gpu virtio_input virtio_ring
xhci_pci xhci_hcd ehci_pci ehci_hcd ohci_pci ohci_hcd uhci_hcd usbhid hid_generic
usb_storage uas evdev
i915 amdgpu radeon nouveau drm drm_kms_helper
efivarfs vivaldi_fmap
dm_mod dm_crypt
zram zstd_compress
EOF
sort -u "${wanted}" -o "${wanted}"

# Resolve dependency closure from modules.dep.
# Each line: path/to/mod.ko: dep1.ko dep2.ko ...
closure="${tmp}/closure"; : > "${closure}"
name_of() { basename "$1" | sed 's/\.ko.*$//'; }

declare -A DEPS
while IFS= read -r line; do
  key="${line%%:*}"
  rest="${line#*:}"
  DEPS["$(name_of "${key}")"]="${rest}"
done < "${modules_dep}"

resolve() {
  local m="$1"
  grep -qxF "$m" "${closure}" && return 0
  echo "$m" >> "${closure}"
  local d
  for d in ${DEPS[$m]:-}; do
    resolve "$(name_of "$d")"
  done
}
while read -r m; do resolve "$m"; done < "${wanted}"
sort -u "${closure}" -o "${closure}"

kept=0; removed=0
while IFS= read -r ko; do
  m="$(name_of "${ko}")"
  if grep -qxF "$m" "${closure}"; then
    kept=$((kept+1))
  else
    rm -f "${ko}"; removed=$((removed+1))
  fi
done < <(find "${MODDIR}" -name '*.ko*')

# drop now-empty dirs
find "${MODDIR}" -type d -empty -delete 2>/dev/null || true
echo "prune-modules: ${MODDIR##*/}  kept=${kept}  removed=${removed}"

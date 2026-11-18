#!/bin/bash
# Boot NowWrite in QEMU from a writable FAT16 disk image (Linux version of run-qemu.ps1).
#
# Your test documents live in output/drive/ (created on first run). Each run:
#   1. copies output/drive/ plus output/NowWrite.efi into output/nowwrite-disk.img,
#   2. boots it in QEMU (the image is a normal writable disk),
#   3. after QEMU exits, copies everything from the image back into output/drive/,
#      so saved documents and .nowwrite/ backups show up on the host.
#
# Uses the system OVMF (edk2) firmware unless --ovmf points at another x64
# firmware image. Needs Python 3 for scripts/fatimg.py.
# Install QEMU + OVMF with e.g.:
#   Arch:          sudo pacman -S qemu-desktop edk2-ovmf
#   Debian/Ubuntu: sudo apt install qemu-system-x86 ovmf
#   Fedora:        sudo dnf install qemu-system-x86 edk2-ovmf
#
# Usage: ./scripts/run-qemu.sh [--resolution PRESET|WxH] [--ovmf FILE] [--size-mb N] [-- extra qemu args...]
#   e.g. ./scripts/run-qemu.sh --resolution fhd
#        ./scripts/run-qemu.sh -r 1600x900
#        ./scripts/run-qemu.sh -- -display none -monitor tcp:127.0.0.1:4444,server,nowait
#
# --resolution sets the screen size the firmware starts in (QEMU's default
# is 1280x800). Presets (--resolution list prints them):
#   vga 640x480, svga 800x600   below 1024x768: NowWrite switches to a larger mode
#   xga 1024x768                smallest size NowWrite keeps as is
#   wxga 1280x800               QEMU's default
#   hd 1360x768                 budget laptop panel (1366x768 ones report 1360)
#   fhd 1920x1080, wuxga 1920x1200
#   qhd 2560x1440               above NowWrite's 1920x1200 switch limit
set -euo pipefail

OVMF=""
SIZE_MB=64
RESOLUTION=""
QEMU_ARGS=()

PRESETS="vga=640x480 svga=800x600 xga=1024x768 wxga=1280x800 hd=1360x768 fhd=1920x1080 wuxga=1920x1200 qhd=2560x1440"

list_presets() {
    for p in $PRESETS; do printf '  %-6s %s\n' "${p%%=*}" "${p#*=}"; done
}

while [ $# -gt 0 ]; do
    case "$1" in
        --ovmf) OVMF="$2"; shift 2 ;;
        --size-mb) SIZE_MB="$2"; shift 2 ;;
        -r|--resolution) RESOLUTION="$2"; shift 2 ;;
        --) shift; QEMU_ARGS=("$@"); break ;;
        -h|--help) sed -n '2,31p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown argument: $1 (put QEMU arguments after --)" >&2; exit 1 ;;
    esac
done

# Resolve --resolution to WIDTHxHEIGHT and the matching VGA device.
DISPLAY_ARGS=()
if [ -n "$RESOLUTION" ]; then
    if [ "$RESOLUTION" = "list" ]; then
        echo "Resolution presets (or pass any WIDTHxHEIGHT):"
        list_presets
        exit 0
    fi
    for p in $PRESETS; do
        if [ "${p%%=*}" = "$RESOLUTION" ]; then RESOLUTION="${p#*=}"; fi
    done
    if ! [[ "$RESOLUTION" =~ ^([0-9]+)x([0-9]+)$ ]]; then
        echo "Unknown resolution '$RESOLUTION'. Use WIDTHxHEIGHT or one of:" >&2
        list_presets >&2
        exit 1
    fi
    XRES="${BASH_REMATCH[1]}"
    YRES="${BASH_REMATCH[2]}"
    # OVMF rounds the width down to a multiple of 8 but QEMU keeps the
    # requested row length, which skews the picture. Round it ourselves.
    if (( XRES % 8 != 0 )); then
        echo "Note: using width $(( XRES / 8 * 8 )) instead of $XRES (QEMU/OVMF need a multiple of 8)."
        XRES=$(( XRES / 8 * 8 ))
    fi
    RESOLUTION="${XRES}x${YRES}"
    # The firmware starts in this mode (from the VGA device's EDID). 32 MiB
    # of video memory covers everything up to 2560x1600 with room to spare.
    DISPLAY_ARGS=(-vga none -device "VGA,vgamem_mb=32,xres=$XRES,yres=$YRES")
fi

EFI="output/NowWrite.efi"
DRIVE="output/drive"
IMAGE="output/nowwrite-disk.img"
VARS="output/qemu-vars.fd"
FATIMG="$(dirname "$0")/fatimg.py"

if [ ! -f "$EFI" ]; then
    echo "Run ./scripts/build-uefi.sh first." >&2
    exit 1
fi

PYTHON="$(command -v python3 || command -v python || true)"
if [ -z "$PYTHON" ]; then
    echo "Python 3 is required (for scripts/fatimg.py)." >&2
    exit 1
fi

QEMU="$(command -v qemu-system-x86_64 || true)"
if [ -z "$QEMU" ]; then
    echo "qemu-system-x86_64 not found." >&2
    exit 1
fi

if [ -n "$OVMF" ]; then
    FIRMWARE=(-bios "$OVMF")
else
    # Code/vars pairs used by common distributions; the first that exists wins.
    CANDIDATES=(
        "/usr/share/edk2/x64/OVMF_CODE.4m.fd:/usr/share/edk2/x64/OVMF_VARS.4m.fd"      # Arch
        "/usr/share/OVMF/OVMF_CODE_4M.fd:/usr/share/OVMF/OVMF_VARS_4M.fd"              # Debian/Ubuntu
        "/usr/share/OVMF/OVMF_CODE.fd:/usr/share/OVMF/OVMF_VARS.fd"                    # older Debian/Ubuntu
        "/usr/share/edk2/ovmf/OVMF_CODE.fd:/usr/share/edk2/ovmf/OVMF_VARS.fd"          # Fedora
        "/usr/share/qemu/edk2-x86_64-code.fd:/usr/share/qemu/edk2-i386-vars.fd"        # QEMU-bundled
    )
    CODE=""
    VARS_SRC=""
    for pair in "${CANDIDATES[@]}"; do
        if [ -f "${pair%%:*}" ] && [ -f "${pair##*:}" ]; then
            CODE="${pair%%:*}"
            VARS_SRC="${pair##*:}"
            break
        fi
    done
    if [ -z "$CODE" ]; then
        echo "No OVMF firmware found; install edk2-ovmf/ovmf or pass --ovmf <path to OVMF.fd>." >&2
        exit 1
    fi
    # Writable copy of the variable store so boot settings can be saved.
    # Recreate it if it doesn't match the firmware's size (e.g. firmware changed).
    if [ ! -f "$VARS" ] || [ "$(stat -c %s "$VARS")" != "$(stat -c %s "$VARS_SRC")" ]; then
        cp "$VARS_SRC" "$VARS"
        chmod u+w "$VARS"
    fi
    FIRMWARE=(
        -drive "if=pflash,format=raw,readonly=on,file=$CODE"
        -drive "if=pflash,format=raw,file=$VARS"
    )
fi

# Use KVM when available; it's much faster than TCG emulation.
ACCEL=()
if [ -w /dev/kvm ]; then
    ACCEL=(-accel kvm)
fi

mkdir -p "$DRIVE"

# If a previous run didn't sync back (e.g. QEMU was killed), recover its changes first.
if [ -f "$IMAGE" ]; then
    "$PYTHON" "$FATIMG" extract "$IMAGE" "$DRIVE" --skip EFI
fi

"$PYTHON" "$FATIMG" create "$IMAGE" "$DRIVE" --efi "$EFI" --size-mb "$SIZE_MB"

echo "Booting NowWrite${RESOLUTION:+ at $RESOLUTION}; documents are synced from/to $DRIVE"
# Don't let a non-zero QEMU exit skip the sync back.
"$QEMU" -machine q35 -m 256M "${ACCEL[@]}" "${FIRMWARE[@]}" "${DISPLAY_ARGS[@]}" \
    -drive "format=raw,file=$IMAGE" -net none "${QEMU_ARGS[@]}" || true

"$PYTHON" "$FATIMG" extract "$IMAGE" "$DRIVE" --skip EFI

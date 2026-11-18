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
# Usage: ./scripts/run-qemu.sh [--ovmf FILE] [--size-mb N] [-- extra qemu args...]
#   e.g. ./scripts/run-qemu.sh -- -display none -monitor tcp:127.0.0.1:4444,server,nowait
set -euo pipefail

OVMF=""
SIZE_MB=64
QEMU_ARGS=()

while [ $# -gt 0 ]; do
    case "$1" in
        --ovmf) OVMF="$2"; shift 2 ;;
        --size-mb) SIZE_MB="$2"; shift 2 ;;
        --) shift; QEMU_ARGS=("$@"); break ;;
        -h|--help) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown argument: $1 (put QEMU arguments after --)" >&2; exit 1 ;;
    esac
done

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

echo "Booting NowWrite; documents are synced from/to $DRIVE"
# Don't let a non-zero QEMU exit skip the sync back.
"$QEMU" -machine q35 -m 256M "${ACCEL[@]}" "${FIRMWARE[@]}" \
    -drive "format=raw,file=$IMAGE" -net none "${QEMU_ARGS[@]}" || true

"$PYTHON" "$FATIMG" extract "$IMAGE" "$DRIVE" --skip EFI

# Boot NowWrite in QEMU from a writable FAT16 disk image.
#
# Your test documents live in output\drive\ (created on first run). Each run:
#   1. copies output\drive\ plus output\NowWrite.efi into output\nowwrite-disk.img,
#   2. boots it in QEMU (the image is a normal writable disk),
#   3. after QEMU exits, copies everything from the image back into output\drive\,
#      so saved documents and .nowwrite\ backups show up on the host.
#
# Uses the edk2 (OVMF) firmware bundled with QEMU for Windows unless -Ovmf
# points at another x64 firmware image. Needs Python 3 for scripts\fatimg.py.
# Install QEMU with: winget install SoftwareFreedomConservancy.QEMU
#
# Extra QEMU arguments can be passed with -QemuArgs, e.g. -QemuArgs '-display','none'.
param(
    [string]$Ovmf,
    [int]$SizeMb = 64,
    [string[]]$QemuArgs = @()
)

$efi = "output\NowWrite.efi"
$drive = "output\drive"
$image = "output\nowwrite-disk.img"
$fatimg = Join-Path $PSScriptRoot "fatimg.py"

if (-not (Test-Path $efi)) {
    Write-Error "Run .\scripts\build-uefi.ps1 first."
    exit 1
}

$python = (Get-Command python -ErrorAction SilentlyContinue).Source
if (-not $python) { $python = (Get-Command py -ErrorAction SilentlyContinue).Source }
if (-not $python) {
    Write-Error "Python 3 is required (for scripts\fatimg.py)."
    exit 1
}

$qemu = (Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue).Source
if (-not $qemu) { $qemu = "C:\Program Files\qemu\qemu-system-x86_64.exe" }
if (-not (Test-Path $qemu)) {
    Write-Error "qemu-system-x86_64 not found."
    exit 1
}
$share = Join-Path (Split-Path $qemu) "share"

if ($Ovmf) {
    $firmware = @("-bios", $Ovmf)
} else {
    $code = Join-Path $share "edk2-x86_64-code.fd"
    if (-not (Test-Path $code)) {
        Write-Error "No bundled firmware at $code; pass -Ovmf <path to OVMF.fd>."
        exit 1
    }
    # Writable copy of the variable store so boot settings can be saved.
    if (-not (Test-Path output\qemu-vars.fd)) {
        Copy-Item (Join-Path $share "edk2-i386-vars.fd") output\qemu-vars.fd
    }
    $firmware = @(
        "-drive", "if=pflash,format=raw,readonly=on,file=$code",
        "-drive", "if=pflash,format=raw,file=output/qemu-vars.fd"
    )
}

New-Item -ItemType Directory -Force $drive | Out-Null

# If a previous run didn't sync back (e.g. QEMU was killed), recover its changes first.
if (Test-Path $image) {
    & $python $fatimg extract $image $drive --skip EFI
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

& $python $fatimg create $image $drive --efi $efi --size-mb $SizeMb
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Booting NowWrite; documents are synced from/to $drive"
& $qemu -machine q35 -m 256M @firmware -drive "format=raw,file=$image" -net none @QemuArgs

& $python $fatimg extract $image $drive --skip EFI
exit $LASTEXITCODE
